//! VA-API bindings.
//!
//! See [`Display`][display::Display] for the main entry point into the library.

#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
mod macros;
mod dlopen;
mod pixelformat;
mod raw;

#[cfg(test)]
mod test;

pub mod buffer;
pub mod config;
pub mod context;
pub mod display;
pub mod error;
pub mod image;
pub mod jpeg;
pub mod subpicture;
pub mod surface;
pub mod vpp;

pub use pixelformat::PixelFormat;

use std::{ffi::c_int, vec};

use error::{Error, VAError, VAStatus};

type Result<T, E = Error> = std::result::Result<T, E>;

ffi_enum! {
    /// A codec profile that may be accelerated with libva.
    pub enum Profile: c_int {
        /// "Misc" profile for format-independent operations.
        None = -1,
        MPEG2Simple = 0,
        MPEG2Main = 1,
        MPEG4Simple = 2,
        MPEG4AdvancedSimple = 3,
        MPEG4Main = 4,
        H264Baseline = 5,
        H264Main = 6,
        H264High = 7,
        VC1Simple = 8,
        VC1Main = 9,
        VC1Advanced = 10,
        H263Baseline = 11,
        JPEGBaseline = 12,
        H264ConstrainedBaseline = 13,
        VP8Version0_3 = 14,
        H264MultiviewHigh = 15,
        H264StereoHigh = 16,
        HEVCMain = 17,
        HEVCMain10 = 18,
        VP9Profile0 = 19,
        VP9Profile1 = 20,
        VP9Profile2 = 21,
        VP9Profile3 = 22,
        HEVCMain12 = 23,
        HEVCMain422_10 = 24,
        HEVCMain422_12 = 25,
        HEVCMain444 = 26,
        HEVCMain444_10 = 27,
        HEVCMain444_12 = 28,
        HEVCSccMain = 29,
        HEVCSccMain10 = 30,
        HEVCSccMain444 = 31,
        AV1Profile0 = 32,
        AV1Profile1 = 33,
        HEVCSccMain444_10 = 34,
        Protected = 35,
    }
}

ffi_enum! {
    /// An entrypoint represents a specific operation on image or video data.
    pub enum Entrypoint: c_int {
        /// Variable-length decoding (of video slices or pictures).
        VLD         = 1,
        IZZ         = 2,
        IDCT        = 3,
        MoComp      = 4,
        Deblocking  = 5,
        /// Video slice encoding.
        EncSlice    = 6,
        /// Picture encoding (eg. for JPEGs)
        EncPicture  = 7,
        EncSliceLP  = 8,
        /// The video processing API. See [`crate::vpp`] for more info.
        VideoProc   = 10,
        /// Flexible Encoding Infrastructure
        FEI         = 11,
        Stats       = 12,
        ProtectedTEEComm = 13,
        ProtectedContent = 14,
    }
}

ffi_enum! {
    /// Image rotation values.
    pub enum Rotation: u32 {
        NONE = 0x00000000,
        R90  = 0x00000001,
        R180 = 0x00000002,
        R270 = 0x00000003,
    }
}

bitflags! {
    /// Mirroring directions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Mirror: u32 {
        const NONE = 0;
        const HORIZONTAL = 0x00000001;
        const VERTICAL   = 0x00000002;
    }
}

bitflags! {
    /// Indicates what part of the slice is being submitted.
    ///
    /// Typically, the whole slice is submitted at once ([`SliceDataFlags::ALL`]).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SliceDataFlags: u32 {
        /// The entire slice is being submitted at once.
        const ALL    = 0x00;
        const BEGIN  = 0x01;
        const MIDDLE = 0x02;
        const END    = 0x04;
    }
}

/// Codec-independent slice parameters.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SliceParameterBufferBase {
    slice_data_size: u32,
    slice_data_offset: u32,
    slice_data_flags: SliceDataFlags,
}

impl SliceParameterBufferBase {
    #[inline]
    pub fn new(slice_data_size: u32) -> Self {
        Self {
            slice_data_size,
            slice_data_offset: 0,
            slice_data_flags: SliceDataFlags::ALL,
        }
    }

    #[inline]
    pub fn slice_data_size(&self) -> u32 {
        self.slice_data_size
    }

    #[inline]
    pub fn slice_data_offset(&self) -> u32 {
        self.slice_data_offset
    }

    #[inline]
    pub fn set_slice_data_offset(&mut self, slice_data_offset: u32) {
        self.slice_data_offset = slice_data_offset;
    }

    #[inline]
    pub fn slice_data_flags(&self) -> SliceDataFlags {
        self.slice_data_flags
    }

    #[inline]
    pub fn set_slice_data_flags(&mut self, flags: SliceDataFlags) {
        self.slice_data_flags = flags;
    }
}

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

fn check(location: &'static str, status: VAStatus) -> Result<()> {
    if status == VAStatus::SUCCESS {
        Ok(())
    } else {
        Err(Error::from_va(location, VAError(status.0)))
    }
}

fn check_log(location: &'static str, status: VAStatus) {
    match check(location, status) {
        Ok(()) => {}
        Err(e) => log::error!("ignoring error in destructor: {e}"),
    }
}
