use std::fmt;

use crate::surface::RTFormat;

/// A FourCC code identifying a pixel format.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PixelFormat(u32);

impl PixelFormat {
    /// Planar YUV 4:2:0 standard pixel format.
    ///
    /// All samples are 8 bits in size. The plane containing Y samples comes first, followed by a
    /// plane storing packed U and V samples (with U samples in the first byte and V samples in the
    /// second byte).
    ///
    /// This format is widely supported by hardware codecs (and often the *only* supported format),
    /// so it should be supported by all software, and may be used as the default format.
    pub const NV12: Self = f(b"NV12");

    /// Planar YUV 4:2:0 pixel format, with U and V swapped compared to `NV12`.
    pub const NV21: Self = f(b"NV21");

    /// Interleaved YUV 4:2:2, stored in memory as `yyyyyyyy uuuuuuuu YYYYYYYY vvvvvvvv`.
    ///
    /// `uuuuuuuu` and `vvvvvvvv` are shared by 2 neighboring pixels.
    pub const YUY2: Self = f(b"YUY2");

    /// Interleaved YUV 4:2:2, stored in memory as `uuuuuuuu yyyyyyyy vvvvvvvv YYYYYYYY`.
    ///
    /// `uuuuuuuu` and `vvvvvvvv` are shared by 2 neighboring pixels.
    pub const UYVY: Self = f(b"UVYV");

    /// `RGBA`: Packed 8-bit RGBA, stored in memory as `aaaaaaaa bbbbbbbb gggggggg rrrrrrrr`.
    pub const RGBA: Self = f(b"RGBA");

    /// `ARGB`: Packed 8-bit RGBA, stored in memory as `bbbbbbbb gggggggg rrrrrrrr aaaaaaaa`.
    pub const ARGB: Self = f(b"ARGB");

    /// Packed 8-bit RGBX.
    ///
    /// The X channel has unspecified values.
    pub const RGBX: Self = f(b"RGBX");

    /// Packed 8-bit BGRA.
    pub const BGRA: Self = f(b"BGRA");

    /// Packed 8-bit BGRX.
    ///
    /// The X channel has unspecified values.
    pub const BGRX: Self = f(b"BGRX");

    pub const fn from_bytes(fourcc: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(fourcc))
    }

    pub const fn from_u32_le(fourcc: u32) -> Self {
        Self(fourcc)
    }

    pub const fn to_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    pub const fn to_u32_le(self) -> u32 {
        self.0
    }

    /// Returns a surface [`RTFormat`] compatible with this [`PixelFormat`].
    ///
    /// Returns [`None`] when `self` is an unknown or unhandled [`PixelFormat`].
    pub fn to_rtformat(self) -> Option<RTFormat> {
        Some(match self {
            Self::NV12 | Self::NV21 => RTFormat::YUV420,
            Self::YUY2 | Self::UYVY => RTFormat::YUV422,
            Self::RGBA | Self::RGBX | Self::ARGB | Self::BGRA | Self::BGRX => RTFormat::RGB32,
            _ => return None,
        })
    }
}

const fn f(fourcc: &[u8; 4]) -> PixelFormat {
    PixelFormat::from_bytes(*fourcc)
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0.to_le_bytes();
        let [a, b, c, d] = bytes.map(|b| (b as char).escape_default());
        write!(f, "{}{}{}{}", a, b, c, d)
    }
}

impl fmt::Debug for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}
