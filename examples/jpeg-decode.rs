use anyhow::{anyhow, bail};
use jfifdump::SegmentKind;
use raw_window_handle::HasRawDisplayHandle;
use softbuffer::GraphicsContext;
use v_ayylmao::{
    jpeg, BufferType, Display, Entrypoint, Profile, SliceParameterBufferBase, SurfaceWithImage,
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_module(env!("CARGO_PKG_NAME"), log::LevelFilter::Trace)
        .init();

    let jpeg = match std::env::args_os().skip(1).next() {
        Some(file) => std::fs::read(file)?,
        None => bail!("usage: jpeg-decode <file>"),
    };
    let mut read = &*jpeg;
    let mut dec = jpeg_decoder::Decoder::new(&mut read);
    let control_data = dec.decode()?;
    let control_data = control_data
        .chunks(3)
        .map(|pix| {
            let [r, g, b] = [pix[0], pix[1], pix[2]].map(u32::from);
            r << 16 | g << 8 | b
        })
        .collect::<Vec<_>>();
    let info = dec.info().unwrap();

    let ev = EventLoop::new();
    let win = Window::new(&ev)?;
    win.set_inner_size(PhysicalSize::new(info.width, info.height));
    win.set_resizable(false);
    let handle = win.raw_display_handle();

    let mut graphics_context = unsafe { GraphicsContext::new(win) }.unwrap();

    let display = Display::new(handle)?;
    let config = display.create_default_config(Profile::JPEGBaseline, Entrypoint::VLD)?;
    let mut context = config.create_default_context(info.width.into(), info.height.into())?;

    let mut surface =
        SurfaceWithImage::new_default_format(&display, info.width.into(), info.height.into())?;

    let mut ppbuf = None;
    let mut slice_params = None;
    let mut slice_data = None;
    let mut iqbuf = jpeg::IQMatrixBuffer::new();
    let mut tbls = [jpeg::HuffmanTable::new(), jpeg::HuffmanTable::new()];
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
                }
                ppbuf = Some(buf);
            }
            SegmentKind::Scan(scan) => {
                slice_params = Some(jpeg::SliceParameterBuffer::new(
                    SliceParameterBufferBase::new(scan.data.len().try_into().unwrap()),
                    restart_interval,
                    1,
                ));
                slice_data = Some(scan.data);
            }
            SegmentKind::Eoi => break,
            SegmentKind::Unknown { marker, .. } => {
                eprintln!("unknown segment marker: {:#04x}", marker);
            }
            SegmentKind::Dac(_)
            | SegmentKind::Rst(_)
            | SegmentKind::Comment(_)
            | SegmentKind::App { .. }
            | SegmentKind::App0Jfif(_) => {}
        }
    }

    let Some(ppbuf) = ppbuf else { bail!("file is missing SOI segment") };
    let Some(slice_data) = slice_data else { bail!("missing SOS segment") };
    let Some(slice_params) = slice_params else { bail!("missing SOS segment") };

    let mut dhtbuf = jpeg::HuffmanTableBuffer::new();
    for (index, table) in tbls.iter().enumerate() {
        dhtbuf.set_huffman_table(index as _, table);
    }

    let mut buf_dht = context.create_param_buffer(BufferType::HuffmanTable, dhtbuf)?;
    let mut buf_iq = context.create_param_buffer(BufferType::IQMatrix, iqbuf)?;
    let mut buf_pp = context.create_param_buffer(BufferType::PictureParameter, ppbuf)?;
    let mut buf_slice_param =
        context.create_param_buffer(BufferType::SliceParameter, slice_params)?;
    let mut buf_slice_data = context.create_data_buffer(BufferType::SliceData, &slice_data)?;

    let mut picture = context.begin_picture(surface.surface_mut())?;
    picture.render_picture(&mut buf_dht)?;
    picture.render_picture(&mut buf_iq)?;
    picture.render_picture(&mut buf_pp)?;
    picture.render_picture(&mut buf_slice_param)?;
    picture.render_picture(&mut buf_slice_data)?;
    unsafe { picture.end_picture()? }
    drop(picture);
    println!("submitted render command");

    let status = surface.surface_mut().status()?;
    eprintln!("{status:?}");

    ev.run(move |event, _tgt, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::RedrawRequested(_) => {
                let (width, height) = {
                    let size = graphics_context.window().inner_size();
                    (size.width, size.height)
                };
                graphics_context.set_buffer(&control_data, width as u16, height as u16);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    })
}
