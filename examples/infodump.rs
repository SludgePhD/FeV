use std::error::Error;

use raw_window_handle::HasRawDisplayHandle;
use v_ayylmao::Display;
use winit::{event_loop::EventLoop, window::Window};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder()
        .filter_module(env!("CARGO_PKG_NAME"), log::LevelFilter::Trace)
        .init();

    let ev = EventLoop::new();
    let win = Window::new(&ev)?;
    let handle = win.raw_display_handle();

    let display = Display::new(handle)?;
    println!(
        "API Version: {}.{}",
        display.version_major(),
        display.version_minor()
    );
    println!("Display API: {:?}", display.display_api());
    println!("Vendor string: {}", display.query_vendor_string()?);

    let profiles = display.query_profiles()?;
    println!("Supported Profiles:");
    for profile in profiles {
        println!("- {:?}", profile);
        for entrypoint in display.query_entrypoints(profile)? {
            println!("  - Entrypoint {:?}", entrypoint);

            let config = display.create_default_config(profile, entrypoint)?;
            let attribs = match config.query_surface_attributes() {
                Ok(attribs) => attribs,
                Err(e) => {
                    println!("    Could not query surface attributes: {e}");
                    continue;
                },
            };
            println!("    {} surface attributes", attribs.len());
            for attrib in attribs {
                print!("    - {:?} ", attrib.ty());
                if attrib.flags().is_empty() {
                    print!("(not supported)");
                } else {
                    print!("{:?}", attrib.flags());
                    if let Some(value) = attrib.as_enum() {
                        print!(" {:?}", value);
                    } else if let Some(value) = attrib.raw_value() {
                        print!(" {:?}", value);
                    } else {
                        print!(" (failed to decode value)");
                    }
                }
                println!();
            }
        }
    }

    println!("Supported image formats:");
    for format in display.query_image_formats()? {
        println!(
            "- {} {:?}, {} bpp, depth={}, Rm={:#010x}, Gm={:#010x}, Bm={:#010x}, Am={:#010x}",
            format.pixel_format(),
            format.byte_order(),
            format.bits_per_pixel(),
            format.depth(),
            format.red_mask(),
            format.green_mask(),
            format.blue_mask(),
            format.alpha_mask(),
        );
    }

    let display_attributes = display.query_display_attributes()?;
    println!("{} supported display attributes", display_attributes.len());
    for attrib in display_attributes {
        println!(
            "- {:?} {:?} [{}-{}] ({})",
            attrib.ty(),
            attrib.flags(),
            attrib.min_value(),
            attrib.max_value(),
            attrib.value(),
        );
    }

    Ok(())
}
