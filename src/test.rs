//! Unit test utilities.

use std::{any::type_name, panic::catch_unwind, sync::OnceLock};

use winit::{event_loop::EventLoop, platform::x11::EventLoopBuilderExtX11};

use crate::{
    display::Display,
    image::{Image, ImageFormat},
    surface::{RTFormat, Surface},
    PixelFormat,
};

struct DisplayHandle {
    event_loop: EventLoop<()>,
}

unsafe impl Send for DisplayHandle {}
unsafe impl Sync for DisplayHandle {}

static EVENT_LOOP: OnceLock<anyhow::Result<DisplayHandle>> = OnceLock::new();

pub const TEST_WIDTH: u32 = 16;
pub const TEST_HEIGHT: u32 = 16;
pub const TEST_RTFORMAT: RTFormat = RTFormat::RGB32;
pub const TEST_PIXELFORMAT: PixelFormat = PixelFormat::RGBA;

pub const TEST_DATA: &[u8] = &[
    0xff, 0x00, 0x00, 0xff, // red
    0xff, 0x00, 0xff, 0x00, // green
    0xff, 0xff, 0x00, 0x00, // blue
];

/// Creates a [`Surface`] and fills its pixels with [`TEST_DATA`].
pub fn test_surface(display: &Display) -> Surface {
    let mut surface = Surface::new(&display, TEST_WIDTH, TEST_HEIGHT, TEST_RTFORMAT)
        .expect("failed to create surface");
    let mut input_image = Image::new(
        &display,
        ImageFormat::new(TEST_PIXELFORMAT),
        TEST_WIDTH,
        TEST_HEIGHT,
    )
    .expect("failed to create input image");

    let mut map = input_image.map().expect("failed to map input image");
    map[..TEST_DATA.len()].copy_from_slice(TEST_DATA);
    drop(map);

    surface
        .copy_from_image(&mut input_image)
        .expect("Surface::copy_from_image failed");

    surface.sync().unwrap();

    surface
}

pub fn run_test<'a, T: FnOnce(&Display)>(test: T) {
    let event_loop = match event_loop() {
        Ok(h) => &h.event_loop,
        Err(e) => {
            log::warn!(
                "skipping test '{}' due to error in requirements (event loop): {e}",
                type_name::<T>()
            );
            return;
        }
    };

    let display = match Display::new(event_loop) {
        Ok(d) => d,
        Err(e) => {
            log::warn!(
                "skipping test '{}' due to error in requirements (VADisplay): {e}",
                type_name::<T>()
            );
            return;
        }
    };

    test(&display);
}

fn event_loop() -> &'static anyhow::Result<DisplayHandle> {
    // Frustratingly, winit seems to have no fallible construction methods for its event loop.
    EVENT_LOOP.get_or_init(|| {
        catch_unwind(|| {
            let ev = winit::event_loop::EventLoopBuilder::new()
                .with_any_thread(true)
                .build();

            DisplayHandle { event_loop: ev }
        })
        .map_err(|any| {
            if let Some(s) = any.downcast_ref::<String>() {
                anyhow::anyhow!("{s}")
            } else {
                anyhow::anyhow!("Box<dyn Any>")
            }
        })
    })
}
