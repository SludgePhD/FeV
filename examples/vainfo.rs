use std::error::Error;

use fev::display::Display;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder()
        .filter_module(env!("CARGO_PKG_NAME"), log::LevelFilter::Trace)
        .init();

    let ev = EventLoop::new()?;

    // Safety: `ev` is dropped after the `display` and all derived resources are dropped.
    // FIXME: use the safe API once winit implements `HasRawDisplayHandle` for `EventLoop`.
    let display = unsafe { Display::new_unmanaged(&*ev)? };
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
