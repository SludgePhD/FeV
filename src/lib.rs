//! VA-API bindings.
//!
//! See [`Display`] for the main entry point into the library.

#[macro_use]
mod macros;
mod dlopen;
mod pixelformat;
mod raw;
mod shared;

pub mod buffer;
pub mod config;
pub mod context;
pub mod display;
pub mod error;
pub mod image;
pub mod jpeg;
pub mod surface;
pub mod vpp;

use std::vec;

use buffer::Mapping;
use error::{VAError, VAStatus};

pub use config::Config;
pub use context::Context;
pub use display::Display;
pub use error::Error;
pub use pixelformat::PixelFormat;
pub use shared::*;
pub use surface::{Surface, SurfaceWithImage};

type Result<T, E = Error> = std::result::Result<T, E>;

/// A list of [`Profile`]s.
#[derive(Clone)]
pub struct Profiles {
    vec: Vec<Profile>,
}

impl Profiles {
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    pub fn contains(&self, profile: Profile) -> bool {
        self.vec.contains(&profile)
    }
}

impl IntoIterator for Profiles {
    type Item = Profile;
    type IntoIter = vec::IntoIter<Profile>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

/// A list of [`Entrypoint`]s.
#[derive(Clone)]
pub struct Entrypoints {
    vec: Vec<Entrypoint>,
}

impl Entrypoints {
    pub fn contains(&self, entrypoint: Entrypoint) -> bool {
        self.vec.contains(&entrypoint)
    }
}

impl IntoIterator for Entrypoints {
    type Item = Entrypoint;
    type IntoIter = vec::IntoIter<Entrypoint>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

fn check(status: VAStatus) -> Result<()> {
    if status == VAStatus::SUCCESS {
        Ok(())
    } else {
        Err(Error::from(VAError(status.0)))
    }
}

fn check_log(status: VAStatus, location: &'static str) {
    match check(status) {
        Ok(()) => {}
        Err(e) => log::error!("ignoring error in {location}: {e}"),
    }
}
