//! [`Surface`]s and surface attributes.

#[cfg_attr(docsrs, doc(cfg(target_os = "linux")))]
#[cfg(target_os = "linux")]
pub mod drm;

use core::fmt;
use std::{
    ffi::{c_int, c_uint, c_void},
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Instant,
    vec,
};

use crate::{
    buffer::Mapping,
    check, check_log,
    display::{Display, DisplayOwner},
    error::VAError,
    image::{Image, ImageFormat},
    pixelformat::PixelFormat,
    raw::{VAGenericFunc, VASurfaceID, VA_PADDING_LOW},
    Error, Result,
};

bitflags! {
    /// Flags for configuring how a [`Surface`] should be exported.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExportSurfaceFlags: u32 {
        /// Export the surface to be read by an external consumer.
        const READ = 0x0001;
        /// Export the surface to be written to by an external application.
        const WRITE = 0x0002;
        /// Export the surface's layers separately.
        const SEPARATE_LAYERS = 0x0004;
        /// Export all layers of the surface in one object.
        const COMPOSED_LAYERS = 0x0008;
    }
}

bitflags! {
    /// Surface pixel formats.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RTFormat: c_uint {
        const YUV420    = 0x00000001;
        const YUV422    = 0x00000002;
        const YUV444    = 0x00000004;
        const YUV411    = 0x00000008;
        const YUV400    = 0x00000010;
        const YUV420_10 = 0x00000100;
        const YUV422_10 = 0x00000200;
        const YUV444_10 = 0x00000400;
        const YUV420_12 = 0x00001000;
        const YUV422_12 = 0x00002000;
        const YUV444_12 = 0x00004000;
        const RGB16     = 0x00010000;
        const RGB32     = 0x00020000;
        const RGBP      = 0x00100000;
        const RGB32_10  = 0x00200000;
        const PROTECTED = 0x80000000;
    }
}

/// A [`Surface`] attribute can be used to request a specific property or configuration during
/// [`Surface`] creation.
///
/// It can be created [`From`] a [`SurfaceAttribEnum`], or queried from a [`Config`] via
/// [`Config::query_surface_attributes`].
///
/// [`Config`]: crate::config::Config
/// [`Config::query_surface_attributes`]: crate::config::Config::query_surface_attributes
#[derive(Clone, Copy)]
#[repr(C)]
pub struct SurfaceAttrib {
    pub(crate) type_: SurfaceAttribType,
    pub(crate) flags: SurfaceAttribFlags,
    pub(crate) value: GenericValue,
}

impl SurfaceAttrib {
    #[inline]
    pub fn ty(&self) -> SurfaceAttribType {
        self.type_
    }

    #[inline]
    pub fn flags(&self) -> SurfaceAttribFlags {
        self.flags
    }

    pub fn is_readable(&self) -> bool {
        self.flags.contains(SurfaceAttribFlags::GETTABLE)
    }

    pub fn is_writable(&self) -> bool {
        self.flags.contains(SurfaceAttribFlags::SETTABLE)
    }

    #[inline]
    pub fn raw_value(&self) -> GenericValue {
        self.value
    }

    pub fn as_enum(&self) -> Option<SurfaceAttribEnum> {
        Some(match self.type_ {
            SurfaceAttribType::PixelFormat => SurfaceAttribEnum::PixelFormat(
                PixelFormat::from_u32_le(self.raw_value().as_int()? as u32),
            ),
            SurfaceAttribType::MemoryType => SurfaceAttribEnum::MemoryType(
                SurfaceAttribMemoryType::from_bits_truncate(self.raw_value().as_int()? as u32),
            ),
            _ => return None,
        })
    }
}

/// Collection of supported [`SurfaceAttrib`]s.
#[derive(Clone)]
pub struct SurfaceAttributes {
    pub(crate) vec: Vec<SurfaceAttrib>,
}

impl SurfaceAttributes {
    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    /// Returns an iterator over all [`PixelFormat`] attributes in the attribute list.
    ///
    /// When querying surface attributes, this is the list of supported pixel formats.
    pub fn pixel_formats(&self) -> impl Iterator<Item = PixelFormat> + '_ {
        self.vec.iter().filter_map(|attr| match attr.as_enum() {
            Some(SurfaceAttribEnum::PixelFormat(fmt)) => Some(fmt),
            _ => None,
        })
    }
}

impl IntoIterator for SurfaceAttributes {
    type Item = SurfaceAttrib;
    type IntoIter = vec::IntoIter<SurfaceAttrib>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

ffi_enum! {
    enum VAGenericValueType: c_int {
        Integer = 1,      /**< 32-bit signed integer. */
        Float = 2,            /**< 32-bit floating-point value. */
        Pointer = 3,          /**< Generic pointer type */
        Func = 4,
    }
}

ffi_enum! {
    /// Enumeration of the available surface attribute types.
    pub enum SurfaceAttribType: c_int {
        None = 0,
        PixelFormat = 1,
        MinWidth = 2,
        MaxWidth = 3,
        MinHeight = 4,
        MaxHeight = 5,
        MemoryType = 6,
        ExternalBufferDescriptor = 7,
        UsageHint = 8,
        DRMFormatModifiers = 9,
        AlignmentSize = 10,
    }
}

ffi_enum! {
    /// Surface rendering status.
    pub enum SurfaceStatus: c_int {
        /// The surface is being rendered to (or from).
        Rendering = 1,
        /// The surface is being displayed.
        Displaying = 2,
        /// The surface is unused and idle.
        Ready = 4,
        Skipped = 8,
    }
}

ffi_enum! {
    pub enum DecodeErrorType: c_int {
        SliceMissing = 0,
        /// Macroblock decoding error.
        MBError = 1,
    }
}

#[allow(dead_code)]
#[repr(C)]
pub struct SurfaceDecodeMBErrors {
    status: i32,
    start_mb: u32,
    end_mb: u32,
    decode_error_type: DecodeErrorType,
    num_mb: u32,
    va_reserved: [u32; VA_PADDING_LOW - 1],
}

#[derive(Clone, Copy)]
#[repr(C)]
union VAGenericValueUnion {
    i: i32,
    f: f32,
    p: *mut c_void,
    func: VAGenericFunc,
}

/// Dynamically typed value of a [`SurfaceAttrib`].
#[derive(Clone, Copy)]
#[repr(C)]
pub struct GenericValue {
    type_: VAGenericValueType,
    value: VAGenericValueUnion,
}

impl GenericValue {
    pub fn int(i: i32) -> Self {
        Self {
            type_: VAGenericValueType::Integer,
            value: VAGenericValueUnion { i },
        }
    }

    pub fn float(f: f32) -> Self {
        Self {
            type_: VAGenericValueType::Float,
            value: VAGenericValueUnion { f },
        }
    }

    pub fn as_int(self) -> Option<i32> {
        if self.type_ == VAGenericValueType::Integer {
            unsafe { Some(self.value.i) }
        } else {
            None
        }
    }

    pub fn as_float(self) -> Option<f32> {
        if self.type_ == VAGenericValueType::Float {
            unsafe { Some(self.value.f) }
        } else {
            None
        }
    }

    pub fn as_pointer(self) -> Option<*mut c_void> {
        if self.type_ == VAGenericValueType::Pointer {
            unsafe { Some(self.value.p) }
        } else {
            None
        }
    }

    pub fn as_func(self) -> Option<VAGenericFunc> {
        if self.type_ == VAGenericValueType::Func {
            unsafe { Some(self.value.func) }
        } else {
            None
        }
    }
}

impl fmt::Debug for GenericValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.type_ {
            VAGenericValueType::Integer => {
                f.debug_tuple("Int").field(&self.as_int().unwrap()).finish()
            }
            VAGenericValueType::Float => f
                .debug_tuple("Float")
                .field(&self.as_float().unwrap())
                .finish(),
            VAGenericValueType::Pointer => f
                .debug_tuple("Pointer")
                .field(&self.as_pointer().unwrap())
                .finish(),
            VAGenericValueType::Func => f
                .debug_tuple("Func")
                .field(&self.as_func().unwrap())
                .finish(),
            _ => f
                .debug_struct("GenericValue")
                .field("type", &self.type_)
                .field("value", unsafe { &self.value.p })
                .finish(),
        }
    }
}

bitflags! {
    /// Flags associated with a queried [`SurfaceAttrib`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SurfaceAttribFlags: c_int {
        const GETTABLE = 0x00000001;
        const SETTABLE = 0x00000002;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SurfaceAttribMemoryType: u32 {
        // Generic types
        const VA       = 0x00000001;
        const V4L2     = 0x00000002;
        const USER_PTR = 0x00000004;

        // DRM types
        const KERNEL_DRM  = 0x10000000;
        const DRM_PRIME   = 0x20000000;
        const DRM_PRIME_2 = 0x40000000;
    }
}

/// A graphics surface or texture.
///
/// A [`Surface`] acts as either the input of an encoding operation, or the output of a decoding
/// operation. It can also be used as either the source or target of a VPP operation.
#[derive(Debug)]
pub struct Surface {
    d: Arc<DisplayOwner>,
    id: VASurfaceID,
}

impl Surface {
    /// Creates a [`Surface`] with the given [`RTFormat`].
    pub fn new(display: &Display, width: u32, height: u32, format: RTFormat) -> Result<Self> {
        log::trace!("creating {width}x{height} surface with {format:?}");
        Self::with_attribs(display, width, height, format, &mut [])
    }

    /// Creates a [`Surface`] with the given [`PixelFormat`].
    ///
    /// This will try to derive a matching [`RTFormat`] automatically via
    /// [`PixelFormat::to_rtformat`].
    ///
    /// Note that not all implementations properly validate the [`PixelFormat`] (eg. mesa), so
    /// creating an unsupported surface may appear to work, but result in a surface with an
    /// unexpected pixel format.
    pub fn with_pixel_format(
        display: &Display,
        width: u32,
        height: u32,
        format: PixelFormat,
    ) -> Result<Self> {
        let rtformat = format.to_rtformat().ok_or_else(|| {
            Error::from(format!(
                "no RTFormat to go with the requested pixel format {:?}",
                format
            ))
        })?;

        log::trace!("creating {width}x{height} surface with format {format:?} and {rtformat:?}");
        Self::with_attribs(
            &display,
            width,
            height,
            rtformat,
            &mut [SurfaceAttribEnum::PixelFormat(format).into()],
        )
    }

    /// Creates a [`Surface`] with the given [`RTFormat`] and a list of [`SurfaceAttrib`]utes to
    /// apply.
    pub fn with_attribs(
        display: &Display,
        width: u32,
        height: u32,
        format: RTFormat,
        attribs: &mut [SurfaceAttrib],
    ) -> Result<Self> {
        let mut id = 0;
        unsafe {
            check(
                "vaCreateSurfaces",
                display.d.libva.vaCreateSurfaces(
                    display.d.raw,
                    format,
                    width as c_uint,
                    height as c_uint,
                    &mut id,
                    1,
                    attribs.as_mut_ptr(),
                    attribs.len() as c_uint,
                ),
            )?;
        }
        Ok(Surface {
            d: display.d.clone(),
            id,
        })
    }

    #[inline]
    pub(crate) fn id(&self) -> VASurfaceID {
        self.id
    }

    /// Blocks until all pending operations writing to or reading from the surface have finished.
    pub fn sync(&mut self) -> Result<()> {
        let start = Instant::now();

        unsafe {
            check(
                "vaSyncSurface",
                self.d.libva.vaSyncSurface(self.d.raw, self.id),
            )?
        }

        log::trace!("vaSyncSurface took {:?}", start.elapsed());
        Ok(())
    }

    /// Returns the current [`SurfaceStatus`] of this [`Surface`].
    ///
    /// The [`SurfaceStatus`] indicates whether and how the [`Surface`] is currently being used by a
    /// VA-API operation.
    pub fn status(&self) -> Result<SurfaceStatus> {
        let mut status = SurfaceStatus(0);
        unsafe {
            check(
                "vaQuerySurfaceStatus",
                self.d
                    .libva
                    .vaQuerySurfaceStatus(self.d.raw, self.id, &mut status),
            )?;
        }
        Ok(status)
    }

    /// Copies all pixels from `self` to the given [`Image`].
    ///
    /// This calls `vaGetImage`, which may be expensive on some drivers (eg. Intel). If possible,
    /// [`SurfaceWithImage`] should be used, so that `vaDeriveImage` is used instead if the driver
    /// supports it.
    pub fn copy_to_image(&mut self, image: &mut Image) -> Result<()> {
        self.sync()?;

        let start = Instant::now();

        unsafe {
            check(
                "vaGetImage",
                self.d.libva.vaGetImage(
                    self.d.raw,
                    self.id,
                    0,
                    0,
                    image.width().into(),
                    image.height().into(),
                    image.id(),
                ),
            )?;
        }

        log::trace!("vaGetImage took {:?}", start.elapsed());

        Ok(())
    }

    /// Copies all pixels from the given [`Image`] onto `self`.
    ///
    /// This calls `vaPutImage`, which may be expensive on some drivers. If possible,
    /// [`SurfaceWithImage`] should be used, so that `vaDeriveImage` is used instead if the driver
    /// supports it.
    pub fn copy_from_image(&mut self, image: &mut Image) -> Result<()> {
        self.sync()?;

        let start = Instant::now();

        unsafe {
            check(
                "vaPutImage",
                self.d.libva.vaPutImage(
                    self.d.raw,
                    self.id,
                    image.id(),
                    0,
                    0,
                    image.width().into(),
                    image.height().into(),
                    0,
                    0,
                    image.width().into(),
                    image.height().into(),
                ),
            )?;
        }

        log::trace!("vaPutImage took {:?}", start.elapsed());

        Ok(())
    }

    /// Creates an [`Image`] that allows direct access to the surface's image data.
    ///
    /// Only supported by some drivers, and only for some surface formats. Will return
    /// [`VAError::ERROR_OPERATION_FAILED`] if it's not supported. In that case, the caller should
    /// fall back to creating an [`Image`] manually and using [`Surface::copy_to_image`]. The
    /// [`SurfaceWithImage`] type encapsulates that pattern and should be used for this if possible.
    ///
    /// # Bugs
    ///
    /// - Mesa has a bug in its implementation of `vaDeriveImage` where the resulting [`Image`]
    ///   won't be mapped correctly and appear completely blank if this method is called before a
    ///   decode operation is submitted.
    pub fn derive_image(&mut self) -> Result<Image> {
        unsafe {
            let mut image = MaybeUninit::uninit();
            check(
                "vaDeriveImage",
                self.d
                    .libva
                    .vaDeriveImage(self.d.raw, self.id, image.as_mut_ptr()),
            )?;
            Ok(Image {
                d: self.d.clone(),
                raw: image.assume_init(),
            })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            check_log(
                "vaDestroySurfaces",
                self.d.libva.vaDestroySurfaces(self.d.raw, &mut self.id, 1),
            );
        }
    }
}

/// Bundles a [`Surface`] and [`Image`] with matching formats.
///
/// Allows copying surface contents to the image.
///
/// If the driver supports `vaDeriveImage`, this type can automatically avoid copying between the
/// two.
#[derive(Debug)]
pub struct SurfaceWithImage {
    surface: Surface,
    image: Image,
    derived: bool,
}

impl SurfaceWithImage {
    /// Creates a new [`SurfaceWithImage`] of the given dimensions.
    ///
    /// The [`RTFormat`] of the [`Surface`] will be determined automatically based on the specified
    /// [`PixelFormat`].
    pub fn new(display: &Display, width: u32, height: u32, format: PixelFormat) -> Result<Self> {
        let mut surface = Surface::with_pixel_format(display, width, height, format)?;

        // Try to use `vaDeriveImage` first, fall back if that fails.
        match surface.derive_image() {
            Ok(image) => {
                log::trace!(
                    "using vaDeriveImage for fast surface access \
                    (image format = {:?})",
                    format,
                );

                Ok(Self {
                    surface,
                    image,
                    derived: true,
                })
            }
            Err(e) if e.as_libva() == Some(VAError::ERROR_OPERATION_FAILED) => {
                log::trace!(
                    "vaDeriveImage not supported, using vaGetImage (simage format = {:?})",
                    format
                );

                let image = Image::new(display, ImageFormat::new(format), width, height)?;
                Ok(Self {
                    surface,
                    image,
                    derived: false,
                })
            }
            Err(e) => Err(e),
        }
    }

    #[inline]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[inline]
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// Synchronizes the [`Surface`] and [`Image`] contents and maps the [`Image`] into memory.
    pub fn map_sync(&mut self) -> Result<Mapping<'_, u8>> {
        if self.derived {
            self.surface.sync()?;
        } else {
            // (syncs internally)
            self.surface.copy_to_image(&mut self.image)?;
        }

        self.image.map()
    }
}

impl Deref for SurfaceWithImage {
    type Target = Surface;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

impl DerefMut for SurfaceWithImage {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.surface
    }
}

/// Enumeration of supported [`SurfaceAttrib`]utes.
#[derive(Debug)]
#[non_exhaustive]
pub enum SurfaceAttribEnum {
    PixelFormat(PixelFormat),
    MemoryType(SurfaceAttribMemoryType),
}

impl From<SurfaceAttribEnum> for SurfaceAttrib {
    fn from(value: SurfaceAttribEnum) -> Self {
        let (ty, value) = match value {
            SurfaceAttribEnum::PixelFormat(format) => (
                SurfaceAttribType::PixelFormat,
                GenericValue::int(format.to_u32_le() as i32),
            ),
            SurfaceAttribEnum::MemoryType(ty) => (
                SurfaceAttribType::MemoryType,
                GenericValue::int(ty.bits() as i32),
            ),
        };

        Self {
            type_: ty,
            flags: SurfaceAttribFlags::SETTABLE,
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test::*;

    use super::*;

    #[test]
    fn image_copy() {
        run_test(|display| {
            let mut surface = test_surface(&display);
            let mut output_image = Image::new(
                &display,
                ImageFormat::new(TEST_PIXELFORMAT),
                TEST_WIDTH,
                TEST_HEIGHT,
            )
            .expect("failed to create output image");

            surface
                .copy_to_image(&mut output_image)
                .expect("Surface::copy_to_image failed");

            surface.sync().unwrap();
            let map = output_image.map().expect("failed to map output image");
            assert_eq!(&map[..TEST_DATA.len()], TEST_DATA);
        });
    }
}
