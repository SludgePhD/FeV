use std::{num::NonZeroU32, rc::Rc, time::Instant};

use anyhow::bail;
use fev::{
    buffer::{Buffer, BufferType},
    config::Config,
    context::Context,
    display::Display,
    image::{Image, ImageFormat},
    jpeg::{JpegDecodeSession, JpegInfo},
    surface::{ExportSurfaceFlags, Surface},
    vpp::{ColorProperties, ColorStandardType, ProcPipelineParameterBuffer, SourceRange},
    Entrypoint, PixelFormat, Profile,
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
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

    let ev = EventLoop::new()?;
    let win = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(info.width, info.height))
        .with_resizable(false)
        .build(&ev)?;
    let win = Rc::new(win);

    let graphics_context = softbuffer::Context::new(win.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&graphics_context, win.clone()).unwrap();
    let s = win.inner_size();
    log::info!("window size: {}x{}", s.width, s.height);

    let jpeg_info = JpegInfo::new(&jpeg)?;
    let (width, height) = (jpeg_info.width(), jpeg_info.height());
    surface
        .resize(
            NonZeroU32::new(width.into()).unwrap(),
            NonZeroU32::new(height.into()).unwrap(),
        )
        .unwrap();

    let display = Display::new(win.clone())?;

    let mut context = JpegDecodeSession::new(&display, width, height)?;
    let prime = context
        .surface()
        .export_prime(ExportSurfaceFlags::SEPARATE_LAYERS | ExportSurfaceFlags::READ)?;
    log::debug!("PRIME export: {prime:#?}");

    let config = Config::new(&display, Profile::None, Entrypoint::VideoProc)?;
    let mut vpp_context = Context::new(&config, width.into(), height.into())?;
    let mut vpp_surface =
        Surface::with_pixel_format(&display, width.into(), height.into(), PixelFormat::RGBA)?;

    let mut image = Image::new(
        &display,
        ImageFormat::new(PixelFormat::RGBA),
        jpeg_info.width().into(),
        jpeg_info.height().into(),
    )?;

    log::debug!("<decode>");
    let start = Instant::now();
    let surf = context.decode(&jpeg)?;
    log::debug!("</decode> took {:?}", start.elapsed());

    // Use VPP to convert the surface to RGBA.
    let mut pppbuf = ProcPipelineParameterBuffer::new(surf);

    // The input color space is the JPEG color space
    let input_props = ColorProperties::new().with_color_range(SourceRange::FULL);
    pppbuf.set_input_color_properties(input_props);
    pppbuf.set_input_color_standard(ColorStandardType::BT601);
    // The output color space is 8-bit non-linear sRGB
    let output_props = ColorProperties::new().with_color_range(SourceRange::FULL);
    pppbuf.set_output_color_properties(output_props);
    pppbuf.set_output_color_standard(ColorStandardType::SRGB);
    // NB: not all implementations support converting color standards (eg. Mesa).
    // such implementations  will typically output an image that is brighter than the reference data.

    let mut pppbuf = Buffer::new_param(&vpp_context, BufferType::ProcPipelineParameter, pppbuf)?;

    let mut picture = vpp_context.begin_picture(&mut vpp_surface)?;
    picture.render_picture(&mut pppbuf)?;
    unsafe { picture.end_picture()? }

    drop(pppbuf);

    vpp_surface.copy_to_image(&mut image)?;
    let mapping = image.map()?;

    log::debug!("{} byte output", mapping.len());

    let start = Instant::now();
    let data = mapping.to_vec();
    log::trace!("copy from VABuffer took {:?}", start.elapsed());
    let start = Instant::now();
    let data = data.to_vec();
    log::trace!("vec copy took {:?}", start.elapsed());

    let start = Instant::now();
    let decoded_data: Vec<_> = data
        .chunks(4)
        .take(jpeg_info.width() as usize * jpeg_info.height() as usize) // ignore trailing padding bytes
        .map(|pix| {
            let [r, g, b, _a] = [pix[0], pix[1], pix[2], pix[3]].map(u32::from);
            r << 16 | g << 8 | b
        })
        .collect();
    log::trace!("conversion took {:?}", start.elapsed());

    let mut show_control_data = false;
    ev.run(move |event, tgt| {
        tgt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let data = if show_control_data {
                    &control_data
                } else {
                    &decoded_data
                };
                let mut buffer = surface.buffer_mut().unwrap();
                buffer.copy_from_slice(data);
                buffer.present().unwrap();
                win.set_title(&format!("control={}", show_control_data));
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                tgt.exit();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: Key::Named(NamedKey::Space),
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
    })?;

    Ok(())
}
