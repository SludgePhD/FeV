//! Unit test utilities.

use winit::platform::x11::EventLoopBuilderExtX11;

use crate::{
    display::Display,
    image::{Image, ImageFormat},
    surface::{RTFormat, Surface},
    PixelFormat,
};

pub const TEST_WIDTH: u32 = 16;
pub const TEST_HEIGHT: u32 = 16;
pub const TEST_RTFORMAT: RTFormat = RTFormat::RGB32;
pub const TEST_PIXELFORMAT: PixelFormat = PixelFormat::RGBA;

pub const TEST_DATA: &[u8] = &[
    0xff, 0x00, 0x00, 0xff, // red
    0xff, 0x00, 0xff, 0x00, // green
    0xff, 0xff, 0x00, 0x00, // blue
];

pub fn test_display() -> Display {
    let ev = winit::event_loop::EventLoopBuilder::new()
        .with_any_thread(true)
        .build();
    Display::new(ev).expect("failed to obtain VA-API display")
}

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
