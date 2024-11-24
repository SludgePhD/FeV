//! Unit test utilities.

use std::{any::type_name, sync::OnceLock};

use winit::{event_loop::EventLoop, platform::x11::EventLoopBuilderExtX11};

use crate::{
    display::Display,
    image::{Image, ImageFormat},
    surface::Surface,
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
pub const TEST_PIXELFORMAT: PixelFormat = PixelFormat::RGBA;

pub const TEST_DATA: &[u8] = &{
    // Start with solid white
    let mut data = [0xff; TEST_HEIGHT as usize * TEST_WIDTH as usize];

    // red
    data[0] = 0xff;
    data[1] = 0x00;
    data[2] = 0x00;
    data[3] = 0xff;

    // green
    data[4] = 0xff;
    data[5] = 0x00;
    data[6] = 0xff;
    data[7] = 0x00;

    // blue
    data[8] = 0xff;
    data[9] = 0xff;
    data[10] = 0x00;
    data[11] = 0x00;

    data
};

/// Creates a [`Surface`] and fills its pixels with [`TEST_DATA`].
pub fn test_surface(display: &Display) -> Surface {
    let mut surface =
        Surface::with_pixel_format(&display, TEST_WIDTH, TEST_HEIGHT, TEST_PIXELFORMAT)
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
    EVENT_LOOP.get_or_init(|| {
        let ev = winit::event_loop::EventLoopBuilder::new()
            .with_any_thread(true)
            .build()?;

        Ok(DisplayHandle { event_loop: ev })
    })
}
