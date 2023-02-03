use std::{cmp, rc::Rc, time::Instant};

use anyhow::{anyhow, bail};
use byteorder::{ReadBytesExt, BE};
use jfifdump::SegmentKind;
use softbuffer::GraphicsContext;
use v_ayylmao::{
    jpeg,
    vpp::{ColorProperties, ColorStandardType, ProcPipelineParameterBuffer, SourceRange},
    BufferType, Display, Entrypoint, PixelFormat, Profile, SliceParameterBufferBase,
    SurfaceWithImage,
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
    let config = display.create_default_config(Profile::JPEGBaseline, Entrypoint::VLD)?;
    let mut jpeg_context = config.create_default_context(info.width.into(), info.height.into())?;
    let config = display.create_default_config(Profile::None, Entrypoint::VideoProc)?;
    let mut vpp_context = config.create_default_context(info.width.into(), info.height.into())?;

    let mut surface = SurfaceWithImage::new(
        &display,
        info.width.into(),
        info.height.into(),
        PixelFormat::NV12,
    )?;
    let mut final_surface = SurfaceWithImage::new(
        &display,
        info.width.into(),
        info.height.into(),
        PixelFormat::RGBA,
    )?;

    log::debug!("intermediate image = {:?}", surface.image());
    log::debug!("final image = {:?}", final_surface.image());

    let eoi;
    let mut max_h_factor = 0;
    let mut max_v_factor = 0;
    let mut ppbuf = None;
    let mut scan = None;
    let mut iqbuf = jpeg::IQMatrixBuffer::new();
    let mut tbls = [
        jpeg::HuffmanTable::default_luminance(),
        jpeg::HuffmanTable::default_chrominance(),
    ];
    let mut restart_interval = 0;

    let mut read = &*jpeg;
    let mut jfif = jfifdump::Reader::new(&mut read)?;
    loop {
        let segment = jfif.next_segment()?;
        match segment.kind {
            SegmentKind::Dri(ri) => {
                restart_interval = ri;
            }
            SegmentKind::Dqt(dqts) => {
                for dqt in dqts {
                    iqbuf.set_quantization_table(dqt.dest, &dqt.values);
                }
            }
            SegmentKind::Dht(dhts) => {
                for dht in dhts {
                    let tbl = tbls.get_mut(usize::from(dht.dest)).ok_or_else(|| {
                        anyhow!(
                            "invalid DHT destination slot {} (expected 0 or 1)",
                            dht.dest
                        )
                    })?;
                    match dht.class {
                        0 => tbl.set_dc_table(&dht.code_lengths, &dht.values),
                        1 => tbl.set_ac_table(&dht.code_lengths, &dht.values),
                        _ => bail!("invalid DHT class {}", dht.class),
                    }
                }
            }
            SegmentKind::Frame(frame) => {
                if frame.sof != 0xC0 {
                    bail!(
                        "not a baseline JPEG (SOF={:02x}, {})",
                        frame.sof,
                        frame.get_sof_name()
                    );
                }
                let mut buf = jpeg::PictureParameterBuffer::new(
                    frame.dimension_x,
                    frame.dimension_y,
                    jpeg::ColorSpace::YUV,
                );
                for component in &frame.components {
                    buf.push_component(
                        component.id,
                        component.horizontal_sampling_factor,
                        component.vertical_sampling_factor,
                        component.quantization_table,
                    );
                    max_h_factor = cmp::max(
                        u32::from(component.horizontal_sampling_factor),
                        max_h_factor,
                    );
                    max_v_factor =
                        cmp::max(u32::from(component.vertical_sampling_factor), max_v_factor);
                }
                ppbuf = Some(buf);
            }
            SegmentKind::Scan(s) => {
                // `segment.position` is *after* the segment's marker for some reason
                scan = Some((segment.position - 2, s));
            }
            SegmentKind::Eoi => {
                eoi = segment.position;
                break;
            }
            SegmentKind::Unknown { marker, .. } => {
                log::warn!("unknown segment marker: {:#04x}", marker);
            }
            SegmentKind::Dac(_)
            | SegmentKind::Rst(_)
            | SegmentKind::Comment(_)
            | SegmentKind::App { .. }
            | SegmentKind::App0Jfif(_) => {}
        }
    }

    let Some(ppbuf) = ppbuf else { bail!("file is missing SOI segment") };
    let Some((sos_pos, scan)) = scan else { bail!("missing SOS segment") };

    // NB: the slice data starts at the *data* contained in the SOS segment, and continues until
    // the byte just before the EOI segment.

    let mut sos = &jpeg[sos_pos..];
    assert_eq!(sos.read_u16::<BE>()?, 0xFFDA);
    let sos_len = usize::from(sos.read_u16::<BE>()?);
    // `Ls` field counts its own bytes, but not the preceding marker.
    let slice_data = &jpeg[sos_pos + sos_len + 2..eoi];

    let width = u32::from(ppbuf.picture_width());
    let height = u32::from(ppbuf.picture_height());
    let num_mcus = ((width + max_h_factor * 8 - 1) / (max_h_factor * 8))
        * ((height + max_v_factor * 8 - 1) / (max_v_factor * 8));
    let mut slice_params = jpeg::SliceParameterBuffer::new(
        SliceParameterBufferBase::new(slice_data.len().try_into().unwrap()),
        restart_interval,
        num_mcus,
    );
    for component in &scan.components {
        slice_params.push_component(component.id, component.dc_table, component.ac_table);
    }

    let mut dhtbuf = jpeg::HuffmanTableBuffer::zeroed();
    for (index, table) in tbls.iter().enumerate() {
        dhtbuf.set_huffman_table(index as _, table);
    }

    let mut buf_dht = jpeg_context.create_param_buffer(BufferType::HuffmanTable, dhtbuf)?;
    let mut buf_iq = jpeg_context.create_param_buffer(BufferType::IQMatrix, iqbuf)?;
    let mut buf_pp = jpeg_context.create_param_buffer(BufferType::PictureParameter, ppbuf)?;
    let mut buf_slice_param =
        jpeg_context.create_param_buffer(BufferType::SliceParameter, slice_params)?;
    let mut buf_slice_data = jpeg_context.create_data_buffer(BufferType::SliceData, &slice_data)?;

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

    let mut pppbuf = vpp_context.create_param_buffer(BufferType::ProcPipelineParameter, pppbuf)?;

    let mut picture = vpp_context.begin_picture(&mut final_surface)?;
    picture.render_picture(&mut pppbuf)?;
    unsafe { picture.end_picture()? }
    log::debug!("submitted VPP op");

    final_surface.sync()?;
    log::debug!("synced final surface");

    drop(pppbuf);

    assert_eq!(final_surface.image().pixelformat(), PixelFormat::RGBA);
    let mapping = final_surface.map_sync()?;
    let decoded_data: Vec<_> = mapping
        .chunks(4)
        .take(width as usize * height as usize) // ignore trailing padding bytes
        .map(|pix| {
            let [b, g, r, _a] = [pix[0], pix[1], pix[2], pix[3]].map(u32::from);
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
