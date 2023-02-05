//! Subpictures and surface blending.
//!
//! (TODO)

use std::vec;

use crate::image::ImageFormat;

bitflags! {
    pub struct SubpictureFlags: u32 {
        const CHROMA_KEYING = 0x0001;
        const GLOBAL_ALPHA  = 0x0002;
        const DESTINATION_IS_SCREEN_COORD = 0x0004;
    }
}

pub struct SubpictureFormat {
    format: ImageFormat,
    flags: SubpictureFlags,
}

impl SubpictureFormat {
    #[inline]
    pub fn image_format(&self) -> &ImageFormat {
        &self.format
    }

    #[inline]
    pub fn flags(&self) -> SubpictureFlags {
        self.flags
    }
}

pub struct SubpictureFormats {
    pub(crate) formats: Vec<ImageFormat>,
    pub(crate) flags: Vec<SubpictureFlags>,
}

impl SubpictureFormats {
    #[inline]
    pub fn len(&self) -> usize {
        self.formats.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
    }
}

impl IntoIterator for SubpictureFormats {
    type Item = SubpictureFormat;
    type IntoIter = SubpictureFormatIter;

    fn into_iter(self) -> Self::IntoIter {
        SubpictureFormatIter {
            formats: self.formats.into_iter(),
            flags: self.flags.into_iter(),
        }
    }
}

pub struct SubpictureFormatIter {
    formats: vec::IntoIter<ImageFormat>,
    flags: vec::IntoIter<SubpictureFlags>,
}

impl Iterator for SubpictureFormatIter {
    type Item = SubpictureFormat;

    fn next(&mut self) -> Option<Self::Item> {
        Some(SubpictureFormat {
            format: self.formats.next()?,
            flags: self.flags.next()?,
        })
    }
}
