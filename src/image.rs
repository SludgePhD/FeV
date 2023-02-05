//! [`Image`] creation and mapping.

use std::{
    ffi::c_int,
    mem::{self, MaybeUninit},
    ptr,
    sync::Arc,
    time::Instant,
    vec,
};

use crate::{
    buffer::Mapping,
    check, check_log,
    display::{Display, DisplayOwner},
    pixelformat::PixelFormat,
    raw::{VABufferID, VAImageID, VA_PADDING_LOW},
    Error, Result,
};

ffi_enum! {
    pub enum ByteOrder: u32 {
        None = 0,
        LsbFirst = 1,
        MsbFirst = 2,
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ImageFormat {
    pub(crate) fourcc: PixelFormat,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bits_per_pixel: u32,
    pub(crate) depth: u32,
    pub(crate) red_mask: u32,
    pub(crate) green_mask: u32,
    pub(crate) blue_mask: u32,
    pub(crate) alpha_mask: u32,
    va_reserved: [u32; VA_PADDING_LOW],
}

impl ImageFormat {
    pub(crate) fn zeroed() -> Self {
        unsafe { mem::zeroed() }
    }

    pub fn new(pixel_format: PixelFormat) -> Self {
        Self {
            fourcc: pixel_format,
            ..unsafe { mem::zeroed() }
        }
    }

    #[inline]
    pub fn pixel_format(&self) -> PixelFormat {
        self.fourcc
    }

    #[inline]
    pub fn set_pixel_format(&mut self, fmt: PixelFormat) {
        self.fourcc = fmt;
    }

    #[inline]
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }
    #[inline]
    pub fn set_byte_order(&mut self, byte_order: ByteOrder) {
        self.byte_order = byte_order;
    }

    #[inline]
    pub fn bits_per_pixel(&self) -> u32 {
        self.bits_per_pixel
    }

    #[inline]
    pub fn set_bits_per_pixel(&mut self, bits_per_pixel: u32) {
        self.bits_per_pixel = bits_per_pixel;
    }

    #[inline]
    pub fn depth(&self) -> u32 {
        self.depth
    }

    #[inline]
    pub fn set_depth(&mut self, depth: u32) {
        self.depth = depth;
    }

    #[inline]
    pub fn red_mask(&self) -> u32 {
        self.red_mask
    }

    #[inline]
    pub fn set_red_mask(&mut self, red_mask: u32) {
        self.red_mask = red_mask;
    }

    #[inline]
    pub fn green_mask(&self) -> u32 {
        self.green_mask
    }

    #[inline]
    pub fn set_green_mask(&mut self, green_mask: u32) {
        self.green_mask = green_mask;
    }

    #[inline]
    pub fn blue_mask(&self) -> u32 {
        self.blue_mask
    }

    #[inline]
    pub fn set_blue_mask(&mut self, blue_mask: u32) {
        self.blue_mask = blue_mask;
    }

    #[inline]
    pub fn alpha_mask(&self) -> u32 {
        self.alpha_mask
    }

    #[inline]
    pub fn set_alpha_mask(&mut self, alpha_mask: u32) {
        self.alpha_mask = alpha_mask;
    }
}

impl From<PixelFormat> for ImageFormat {
    fn from(value: PixelFormat) -> Self {
        ImageFormat::new(value)
    }
}

#[derive(Clone)]
pub struct ImageFormats {
    pub(crate) vec: Vec<ImageFormat>,
}

impl ImageFormats {
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
}

impl IntoIterator for ImageFormats {
    type Item = ImageFormat;
    type IntoIter = vec::IntoIter<ImageFormat>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

#[derive(Debug)]
#[repr(C)]
pub(crate) struct VAImage {
    pub image_id: VAImageID,
    pub format: ImageFormat,
    pub buf: VABufferID,
    pub width: u16,
    pub height: u16,
    pub data_size: u32,
    pub num_planes: u32,
    pub pitches: [u32; 3],
    pub offsets: [u32; 3],
    pub num_palette_entries: i32,
    pub entry_bytes: i32,
    pub component_order: [i8; 4],
    va_reserved: [u32; VA_PADDING_LOW],
}

/// An [`Image`] is a mappable [`Buffer`][crate::buffer::Buffer] storing image data.
#[derive(Debug)]
pub struct Image {
    pub(crate) d: Arc<DisplayOwner>,
    pub(crate) raw: VAImage,
}

impl Image {
    pub fn new(
        display: &Display,
        mut format: ImageFormat,
        width: u32,
        height: u32,
    ) -> Result<Image> {
        let width: c_int = width.try_into().map_err(Error::from)?;
        let height: c_int = height.try_into().map_err(Error::from)?;
        let mut image = MaybeUninit::uninit();
        unsafe {
            check(display.d.libva.vaCreateImage(
                display.d.raw,
                &mut format,
                width,
                height,
                image.as_mut_ptr(),
            ))?;
            Ok(Image {
                d: display.d.clone(),
                raw: image.assume_init(),
            })
        }
    }

    #[inline]
    pub(crate) fn id(&self) -> VAImageID {
        self.raw.image_id
    }

    #[inline]
    pub fn width(&self) -> u16 {
        self.raw.width
    }

    #[inline]
    pub fn height(&self) -> u16 {
        self.raw.height
    }

    #[inline]
    pub fn image_format(&self) -> &ImageFormat {
        &self.raw.format
    }

    #[inline]
    pub fn pixel_format(&self) -> PixelFormat {
        self.raw.format.fourcc
    }

    /// Maps the [`Buffer`][crate::buffer::Buffer] storing the backing data of this [`Image`].
    pub fn map(&mut self) -> Result<Mapping<'_, u8>> {
        let start = Instant::now();

        let mut ptr = ptr::null_mut();
        unsafe {
            check(self.d.libva.vaMapBuffer(self.d.raw, self.raw.buf, &mut ptr))?;
        }

        log::trace!("vaMapBuffer for VAImage took {:?}", start.elapsed());

        Ok(Mapping {
            d: &self.d,
            id: self.raw.buf,
            ptr: ptr.cast(),
            capacity: self.raw.data_size as usize,
        })
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyImage(self.d.raw, self.raw.image_id),
                "vaDestroyImage call in drop",
            );
        }
    }
}
