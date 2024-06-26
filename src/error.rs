//! Defines the [`Error`] type used throughout the library.

use core::fmt;
use std::{
    ffi::{c_int, CStr},
    num::TryFromIntError,
    str::Utf8Error,
};

use crate::dlopen::libva;

ffi_enum! {
    pub(crate) enum VAStatus: c_int {
        SUCCESS                        = 0x00000000,
        // Other allowed values are in `VAError`.
    }
}

ffi_enum! {
    /// An error code returned by *libva*.
    pub enum VAError: c_int {
        ERROR_OPERATION_FAILED         = 0x00000001,
        ERROR_ALLOCATION_FAILED        = 0x00000002,
        ERROR_INVALID_DISPLAY          = 0x00000003,
        ERROR_INVALID_CONFIG           = 0x00000004,
        ERROR_INVALID_CONTEXT          = 0x00000005,
        ERROR_INVALID_SURFACE          = 0x00000006,
        ERROR_INVALID_BUFFER           = 0x00000007,
        ERROR_INVALID_IMAGE            = 0x00000008,
        ERROR_INVALID_SUBPICTURE       = 0x00000009,
        ERROR_ATTR_NOT_SUPPORTED       = 0x0000000a,
        ERROR_MAX_NUM_EXCEEDED         = 0x0000000b,
        ERROR_UNSUPPORTED_PROFILE      = 0x0000000c,
        ERROR_UNSUPPORTED_ENTRYPOINT   = 0x0000000d,
        ERROR_UNSUPPORTED_RT_FORMAT    = 0x0000000e,
        ERROR_UNSUPPORTED_BUFFERTYPE   = 0x0000000f,
        ERROR_SURFACE_BUSY             = 0x00000010,
        ERROR_FLAG_NOT_SUPPORTED       = 0x00000011,
        ERROR_INVALID_PARAMETER        = 0x00000012,
        ERROR_RESOLUTION_NOT_SUPPORTED = 0x00000013,
        ERROR_UNIMPLEMENTED            = 0x00000014,
        ERROR_SURFACE_IN_DISPLAYING    = 0x00000015,
        ERROR_INVALID_IMAGE_FORMAT     = 0x00000016,
        ERROR_DECODING_ERROR           = 0x00000017,
        ERROR_ENCODING_ERROR           = 0x00000018,
        ERROR_INVALID_VALUE            = 0x00000019,
        ERROR_UNSUPPORTED_FILTER       = 0x00000020,
        ERROR_INVALID_FILTER_CHAIN     = 0x00000021,
        ERROR_HW_BUSY                  = 0x00000022,
        ERROR_UNSUPPORTED_MEMORY_TYPE  = 0x00000024,
        ERROR_NOT_ENOUGH_BUFFER        = 0x00000025,
        ERROR_TIMEDOUT                 = 0x00000026,
        #[allow(overflowing_literals)]
        ERROR_UNKNOWN                  = 0xFFFFFFFF,
    }
}

impl From<VAError> for VAStatus {
    #[inline]
    fn from(e: VAError) -> Self {
        Self(e.0)
    }
}

impl PartialEq<VAError> for VAStatus {
    #[inline]
    fn eq(&self, other: &VAError) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<VAStatus> for VAError {
    #[inline]
    fn eq(&self, other: &VAStatus) -> bool {
        self.0 == other.0
    }
}

impl VAError {
    pub fn to_str(self) -> Result<&'static str, Error> {
        unsafe {
            let cstr = &CStr::from_ptr(libva::get()?.vaErrorStr(self.into()));
            Ok(cstr.to_str().map_err(Error::from)?)
        }
    }
}

pub(crate) enum Repr {
    Libva(&'static str, VAError),
    Libloading {
        inner: libloading::Error,
        libname: String,
        funcname: Option<&'static str>,
    },
    Utf8Error(Utf8Error),
    TryFromIntError(TryFromIntError),
    HandleError(raw_window_handle::HandleError),
    Other(String),
    Static(&'static Error),
}

impl From<raw_window_handle::HandleError> for Repr {
    fn from(v: raw_window_handle::HandleError) -> Self {
        Self::HandleError(v)
    }
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

impl<'a> From<&'a str> for Repr {
    fn from(value: &'a str) -> Self {
        Self::Other(value.into())
    }
}

impl From<Utf8Error> for Repr {
    fn from(v: Utf8Error) -> Self {
        Self::Utf8Error(v)
    }
}

/// The main error type used by this library.
pub struct Error {
    repr: Repr,
}

impl Error {
    /// If this [`Error`] was returned by a *libva* function, returns the corresponding [`VAError`]
    /// code.
    pub fn as_libva(&self) -> Option<VAError> {
        match &self.repr {
            Repr::Libva(_, e) => Some(*e),
            _ => None,
        }
    }

    pub(crate) fn from(e: impl Into<Repr>) -> Self {
        Self { repr: e.into() }
    }

    pub(crate) fn from_va(location: &'static str, error: VAError) -> Self {
        Self {
            repr: Repr::Libva(location, error),
        }
    }

    pub(crate) fn dlopen(libname: &str, error: libloading::Error) -> Self {
        Self {
            repr: Repr::Libloading {
                inner: error,
                libname: libname.to_string(),
                funcname: None,
            },
        }
    }

    pub(crate) fn dlsym(libname: &str, funcname: &'static str, error: libloading::Error) -> Self {
        Self {
            repr: Repr::Libloading {
                inner: error,
                libname: libname.to_string(),
                funcname: Some(funcname),
            },
        }
    }

    pub(crate) fn statik(error: &'static Self) -> Self {
        Self {
            repr: Repr::Static(error),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Libva(loc, e) => write!(f, "{loc}: {e:?}"),
            Repr::Libloading {
                inner,
                libname,
                funcname,
            } => {
                write!(f, "{libname}")?;
                if let Some(name) = funcname {
                    write!(f, "/{name}")?;
                }
                write!(f, ": {inner:?}")
            }
            Repr::Utf8Error(e) => e.fmt(f),
            Repr::TryFromIntError(e) => e.fmt(f),
            Repr::HandleError(e) => e.fmt(f),
            Repr::Other(s) => s.fmt(f),
            Repr::Static(e) => e.fmt(f),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            Repr::Libva(loc, e) => match e.to_str() {
                Ok(s) => write!(f, "{loc}: {s} ({e:?})"),
                Err(_) => fmt::Debug::fmt(e, f),
            },
            Repr::Libloading {
                inner,
                libname,
                funcname,
            } => {
                write!(f, "{libname}")?;
                if let Some(name) = funcname {
                    write!(f, "/{name}")?;
                }
                write!(f, ": {inner}")
            }
            Repr::Utf8Error(e) => e.fmt(f),
            Repr::TryFromIntError(e) => e.fmt(f),
            Repr::HandleError(e) => e.fmt(f),
            Repr::Other(e) => e.fmt(f),
            Repr::Static(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
