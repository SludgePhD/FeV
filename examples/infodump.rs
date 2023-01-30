use std::error::Error;

use v_ayylmao::{Display, Entrypoint, Profile};
use winit::{event_loop::EventLoop, window::Window};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder()
        .filter_module(env!("CARGO_PKG_NAME"), log::LevelFilter::Trace)
        .init();

    let ev = EventLoop::new();
    let win = Window::new(&ev)?;

    let display = Display::new(win)?;
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
            let attribs = config.query_config_attributes()?;
            println!("    {} config attributes", attribs.len());
            for attrib in attribs {
                println!(
                    "    - {:?}: {:08x}",
                    attrib.attrib_type(),
                    attrib.raw_value(),
                );
            }
            let attribs = match config.query_surface_attributes() {
                Ok(attribs) => attribs,
                Err(e) => {
                    println!("    Could not query surface attributes: {e}");
                    continue;
                }
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

    let formats = display.query_image_formats()?;
    println!("{} supported image formats", formats.len());
    for format in formats {
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

    if display.query_profiles()?.contains(Profile::None)
        && display
            .query_entrypoints(Profile::None)?
            .contains(Entrypoint::VideoProc)
    {
        let config = display.create_default_config(Profile::None, Entrypoint::VideoProc)?;
        let context = config.create_default_context(512, 512)?;
        let proc_filters = context.query_video_processing_filters()?;
        println!("{} supported video processing filters", proc_filters.len());
        for filter in proc_filters {
            println!("- {:?}", filter);
        }
    }

    Ok(())
}
