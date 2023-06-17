use std::{num::NonZeroU32, rc::Rc, time::Instant};

use anyhow::bail;
use fev::{
    display::Display,
    jpeg::{JpegDecodeSession, JpegInfo},
};
use softbuffer::{Context, Surface};
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

    let graphics_context = unsafe { Context::new(&win).unwrap() };
    let mut surface = unsafe { Surface::new(&graphics_context, &win).unwrap() };
    let PhysicalSize { width, height } = win.inner_size();
    surface
        .resize(
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
        )
        .unwrap();

    let display = Display::new(win.clone())?;

    let jpeg_info = JpegInfo::new(&jpeg)?;
    let mut context = JpegDecodeSession::new(&display, jpeg_info.width(), jpeg_info.height())?;
    let mapping = context.decode(&jpeg)?;

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
    ev.run(move |event, _tgt, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::RedrawRequested(_) => {
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
