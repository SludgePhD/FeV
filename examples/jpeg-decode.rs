use std::{cmp, rc::Rc, time::Instant};

use anyhow::{anyhow, bail};
use softbuffer::GraphicsContext;
use v_ayylmao::{
    buffer::{Buffer, BufferType},
    config::Config,
    context::Context,
    display::Display,
    jpeg::{self, parser::SofMarker},
    surface::{Surface, SurfaceWithImage},
    vpp::{ColorProperties, ColorStandardType, ProcPipelineParameterBuffer, SourceRange},
    Entrypoint, PixelFormat, Profile, SliceParameterBufferBase,
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_module(
            &env!("CARGO_PKG_NAME").replace('-', "_"),
            log::LevelFilter::Trace,
        )
        .filter_module(env!("CARGO_CRATE_NAME"), log::LevelFilter::Trace)
        .init();

    let jpeg = match std::env::args_os().skip(1).next() {
        Some(file) => std::fs::read(file)?,
        None => bail!("usage: jpeg-decode <file>"),
    };
    let mut read = &*jpeg;
    let mut dec = jpeg_decoder::Decoder::new(&mut read);
    let start = Instant::now();
    let control_data = dec.decode()?;
    log::info!("jpeg-decoder took {:?}", start.elapsed());
    let control_data = control_data
        .chunks(3)
        .map(|pix| {
            let [r, g, b] = [pix[0], pix[1], pix[2]].map(u32::from);
            r << 16 | g << 8 | b
        })
        .collect::<Vec<_>>();
    let info = dec.info().unwrap();
    log::info!("image size: {}x{}", info.width, info.height);

    let ev = EventLoop::new();
    let win = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(info.width, info.height))
        .with_resizable(false)
        .build(&ev)?;
    let win = Rc::new(win);

    let mut graphics_context = unsafe { GraphicsContext::new(&win, &win) }.unwrap();

    let display = Display::new(win.clone())?;
    let config = Config::new(&display, Profile::JPEGBaseline, Entrypoint::VLD)?;
    let mut jpeg_context = Context::new(&config, info.width.into(), info.height.into())?;
    let config = Config::new(&display, Profile::None, Entrypoint::VideoProc)?;
    let mut vpp_context = Context::new(&config, info.width.into(), info.height.into())?;

    let mut surface = Surface::new(
        &display,
        info.width.into(),
        info.height.into(),
        PixelFormat::NV12.to_rtformat().unwrap(),
    )?;
    let mut final_surface = SurfaceWithImage::new(
        &display,
        info.width.into(),
        info.height.into(),
        PixelFormat::RGBA,
    )?;

    log::debug!("image format = {:?}", final_surface.image());

    let mut max_h_factor = 0;
    let mut max_v_factor = 0;
    let mut ppbuf = None;
    let mut slice = None;
    let mut iqbuf = jpeg::IQMatrixBuffer::new();
    let mut tbls = [
        jpeg::HuffmanTable::default_luminance(),
        jpeg::HuffmanTable::default_chrominance(),
    ];
    let mut restart_interval = 0;

    let mut parser = jpeg::parser::JpegParser::new(&jpeg);
    while let Some(segment) = parser.next_segment()? {
        match segment.kind {
            jpeg::parser::SegmentKind::Dqt(dqt) => {
                for dqt in dqt.tables() {
                    if dqt.Pq() != 0 {
                        bail!("unexpected value `{}` for DQT Pq", dqt.Pq());
                    }
                    iqbuf.set_quantization_table(dqt.Tq(), &dqt.Qk());
                }
            }
            jpeg::parser::SegmentKind::Dht(dht) => {
                for table in dht.tables() {
                    let tbl = tbls.get_mut(usize::from(table.Th())).ok_or_else(|| {
                        anyhow!(
                            "invalid DHT destination slot {} (expected 0 or 1)",
                            table.Th()
                        )
                    })?;
                    match table.Tc() {
                        0 => tbl.set_dc_table(table.Li(), table.Vij()),
                        1 => tbl.set_ac_table(table.Li(), table.Vij()),
                        _ => bail!("invalid DHT class {}", table.Tc()),
                    }
                }
            }
            jpeg::parser::SegmentKind::Dri(dri) => restart_interval = dri.Ri(),
            jpeg::parser::SegmentKind::Sof(sof) => {
                if sof.sof() != SofMarker::SOF0 {
                    bail!("not a baseline JPEG (SOF={:?})", sof.sof());
                }

                if sof.P() != 8 {
                    bail!("sample precision {} bits is not supported", sof.P());
                }

                let mut buf =
                    jpeg::PictureParameterBuffer::new(sof.X(), sof.Y(), jpeg::ColorSpace::YUV);
                for component in sof.components() {
                    buf.push_component(
                        component.Ci(),
                        component.Hi(),
                        component.Vi(),
                        component.Tqi(),
                    );
                    max_h_factor = cmp::max(u32::from(component.Hi()), max_h_factor);
                    max_v_factor = cmp::max(u32::from(component.Vi()), max_v_factor);
                }
                ppbuf = Some(buf);
            }
            jpeg::parser::SegmentKind::Sos(sos) => {
                let Some(ppbuf) = &ppbuf else { continue };
                let slice_data = sos.data();
                let width = u32::from(ppbuf.picture_width());
                let height = u32::from(ppbuf.picture_height());
                let num_mcus = ((width + max_h_factor * 8 - 1) / (max_h_factor * 8))
                    * ((height + max_v_factor * 8 - 1) / (max_v_factor * 8));
                let mut slice_params = jpeg::SliceParameterBuffer::new(
                    SliceParameterBufferBase::new(slice_data.len().try_into().unwrap()),
                    restart_interval,
                    num_mcus,
                );
                for component in sos.components() {
                    slice_params.push_component(component.Csj(), component.Tdj(), component.Taj());
                }
                slice = Some((slice_params, slice_data));
            }
            jpeg::parser::SegmentKind::Eoi => break,
            _ => {}
        }
    }

    let Some(ppbuf) = ppbuf else { bail!("file is missing SOI segment") };
    let Some((slice_params, slice_data)) = slice else { bail!("file is missing SOS header") };

    let width = u32::from(ppbuf.picture_width());
    let height = u32::from(ppbuf.picture_height());

    let mut dhtbuf = jpeg::HuffmanTableBuffer::zeroed();
    for (index, table) in tbls.iter().enumerate() {
        dhtbuf.set_huffman_table(index as _, table);
    }

    let mut buf_dht = Buffer::new_param(&jpeg_context, BufferType::HuffmanTable, dhtbuf)?;
    let mut buf_iq = Buffer::new_param(&jpeg_context, BufferType::IQMatrix, iqbuf)?;
    let mut buf_pp = Buffer::new_param(&jpeg_context, BufferType::PictureParameter, ppbuf)?;
    let mut buf_slice_param =
        Buffer::new_param(&jpeg_context, BufferType::SliceParameter, slice_params)?;
    let mut buf_slice_data = Buffer::new_data(&jpeg_context, BufferType::SliceData, &slice_data)?;

    let mut picture = jpeg_context.begin_picture(&mut surface)?;
    picture.render_picture(&mut buf_dht)?;
    picture.render_picture(&mut buf_iq)?;
    picture.render_picture(&mut buf_pp)?;
    picture.render_picture(&mut buf_slice_param)?;
    picture.render_picture(&mut buf_slice_data)?;
    unsafe { picture.end_picture()? }

    surface.sync()?;
    log::debug!("synced jpeg output surface");

    let mut pppbuf = ProcPipelineParameterBuffer::new(&surface);
    // The input color space is the JPEG color space
    pppbuf.set_input_color_properties(ColorProperties::new().with_color_range(SourceRange::FULL));
    pppbuf.set_input_color_standard(ColorStandardType::BT601);
    // The output color space is 8-bit non-linear sRGB
    pppbuf.set_output_color_properties(ColorProperties::new().with_color_range(SourceRange::FULL));
    pppbuf.set_output_color_standard(ColorStandardType::SRGB);

    let mut pppbuf = Buffer::new_param(&vpp_context, BufferType::ProcPipelineParameter, pppbuf)?;

    let mut picture = vpp_context.begin_picture(&mut final_surface)?;
    picture.render_picture(&mut pppbuf)?;
    unsafe { picture.end_picture()? }
    log::debug!("submitted VPP op");

    final_surface.sync()?;
    log::debug!("synced final surface");

    drop(pppbuf);

    assert_eq!(final_surface.image().pixel_format(), PixelFormat::RGBA);
    let mapping = final_surface.map_sync()?;
    let decoded_data: Vec<_> = mapping
        .chunks(4)
        .take(width as usize * height as usize) // ignore trailing padding bytes
        .map(|pix| {
            let [r, g, b, _a] = [pix[0], pix[1], pix[2], pix[3]].map(u32::from);
            r << 16 | g << 8 | b
        })
        .collect();

    let mut show_control_data = false;
    ev.run(move |event, _tgt, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::RedrawRequested(_) => {
                let (width, height) = {
                    let size = win.inner_size();
                    (size.width, size.height)
                };
                let data = if show_control_data {
                    &control_data
                } else {
                    &decoded_data
                };
                graphics_context.set_buffer(data, width as u16, height as u16);
                win.set_title(&format!("control={}", show_control_data));
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Space),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    }
                    | WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Left,
                        ..
                    },
                ..
            } => {
                show_control_data = !show_control_data;
                win.request_redraw();
            }
            _ => {}
        }
    })
}
