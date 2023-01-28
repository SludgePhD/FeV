use core::fmt;
use std::{num::TryFromIntError, str::Utf8Error};

use crate::VAError;

pub(crate) enum Repr {
    Libva(VAError),
    Libloading(libloading::Error),
    Utf8Error(Utf8Error),
    TryFromIntError(TryFromIntError),
    Other(String),
}

impl From<TryFromIntError> for Repr {
    fn from(v: TryFromIntError) -> Self {
        Self::TryFromIntError(v)
    }
}

impl From<String> for Repr {
    fn from(v: String) -> Self {
        Self::Other(v)
    }
}

impl From<Utf8Error> for Repr {
    fn from(v: Utf8Error) -> Self {
        Self::Utf8Error(v)
    }
}

impl From<libloading::Error> for Repr {
    fn from(v: libloading::Error) -> Self {
        Self::Libloading(v)
    }
}

impl From<VAError> for Repr {
    fn from(v: VAError) -> Self {
        Self::Libva(v)
    }
}

pub struct Error {
    repr: Repr,
}

impl Error {
    /// If this [`Error`] was returned by a *libva* function, returns the corresponding [`VAError`]
    /// code.
    pub fn as_libva(&self) -> Option<VAError> {
        match &self.repr {
            Repr::Libva(e) => Some(*e),
            _ => None,
        }
    }

    pub(crate) fn from(e: impl Into<Repr>) -> Self {
        Self { repr: e.into() }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Libva(e) => e.fmt(f),
            Repr::Libloading(e) => e.fmt(f),
            Repr::Utf8Error(e) => e.fmt(f),
            Repr::TryFromIntError(e) => e.fmt(f),
            Repr::Other(s) => s.fmt(f),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Libva(e) => match e.to_str() {
                Ok(s) => write!(f, "{self:?}: {s}"),
                Err(_) => fmt::Debug::fmt(e, f),
            },
            Repr::Libloading(e) => e.fmt(f),
            Repr::Utf8Error(e) => e.fmt(f),
            Repr::TryFromIntError(e) => e.fmt(f),
            Repr::Other(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
