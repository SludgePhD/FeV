use std::error::Error;

use v_ayylmao::Display;
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
    let width = profiles
        .clone()
        .into_iter()
        .map(|prof| format!("{:?}", prof).len())
        .max();

    println!("Supported Profiles:");
    for profile in profiles {
        print!("{:>1$?}: ", profile, width.unwrap());
        for (i, entrypoint) in display.query_entrypoints(profile)?.into_iter().enumerate() {
            if i != 0 {
                print!(" + ");
            }
            print!("{:?}", entrypoint);
        }
        println!();
    }

    Ok(())
}
