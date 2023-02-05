//! [`Surface`]s to decode to or encode from.

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
    display::DisplayOwner,
    error::VAError,
    image::{Image, ImageFormat},
    raw::{VAGenericFunc, VASurfaceID},
    Display, Error, PixelFormat, Result,
};

bitflags! {
    pub struct ExportSurface: u32 {
        const READ = 0x0001;
        const WRITE = 0x0002;
        const SEPARATE_LAYERS = 0x0004;
        const COMPOSED_LAYERS = 0x0008;
    }
}

bitflags! {
    /// Surface pixel formats.
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

/// A [`Surface`] attribute.
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
    }
}

ffi_enum! {
    pub enum SurfaceStatus: c_int {
        Rendering = 1,
        Displaying = 2,
        Ready = 4,
        Skipped = 8,
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
union VAGenericValueUnion {
    i: i32,
    f: f32,
    p: *mut c_void,
    func: VAGenericFunc,
}

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
    pub struct SurfaceAttribFlags: c_int {
        const GETTABLE = 0x00000001;
        const SETTABLE = 0x00000002;
    }
}

bitflags! {
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
#[derive(Debug)]
pub struct Surface {
    d: Arc<DisplayOwner>,
    id: VASurfaceID,
}

impl Surface {
    pub fn new(display: &Display, format: RTFormat, width: u32, height: u32) -> Result<Self> {
        Self::with_attribs(display, format, width, height, &mut [])
    }

    pub fn with_attribs(
        display: &Display,
        format: RTFormat,
        width: u32,
        height: u32,
        attribs: &mut [SurfaceAttrib],
    ) -> Result<Self> {
        let mut id = 0;
        unsafe {
            check(display.d.libva.vaCreateSurfaces(
                display.d.raw,
                format,
                width as c_uint,
                height as c_uint,
                &mut id,
                1,
                attribs.as_mut_ptr(),
                attribs.len() as c_uint,
            ))?;
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

        unsafe { check(self.d.libva.vaSyncSurface(self.d.raw, self.id))? }

        log::trace!("vaSyncSurface took {:?}", start.elapsed());
        Ok(())
    }

    pub fn status(&self) -> Result<SurfaceStatus> {
        let mut status = SurfaceStatus(0);
        unsafe {
            check(
                self.d
                    .libva
                    .vaQuerySurfaceStatus(self.d.raw, self.id, &mut status),
            )?;
        }
        Ok(status)
    }

    /// Copies all pixels from `self` to the given [`Image`].
    ///
    /// This calls `vaGetImage`, which may be expensive on some drivers (eg.
    /// Intel). If possible, [`SurfaceWithImage`] should be used, so that
    /// `vaDeriveImage` is used instead if the driver supports it.
    pub fn copy_to_image(&mut self, image: &mut Image) -> Result<()> {
        self.sync()?;

        let start = Instant::now();

        unsafe {
            check(self.d.libva.vaGetImage(
                self.d.raw,
                self.id,
                0,
                0,
                image.width().into(),
                image.height().into(),
                image.id(),
            ))?;
        }

        log::trace!("vaGetImage took {:?}", start.elapsed());

        Ok(())
    }

    /// Creates an [`Image`] that allows direct access to the surface's image data.
    ///
    /// Only supported by some drivers, and only for some surface formats. Will return
    /// [`VAError::ERROR_OPERATION_FAILED`] if it's not supported.
    pub fn derive_image(&mut self) -> Result<Image> {
        unsafe {
            let mut image = MaybeUninit::uninit();
            check(
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
                self.d.libva.vaDestroySurfaces(self.d.raw, &mut self.id, 1),
                "vaDestroySurfaces call in drop",
            );
        }
    }
}

/// Bundles together a [`Surface`] and an [`Image`] with a matching format, and allows transferring
/// data between them.
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
        let rtformat = format.to_rtformat().ok_or_else(|| {
            Error::from(format!(
                "pixel format {:?} is unknown or unimplemented",
                format
            ))
        })?;

        let mut surface = Surface::with_attribs(
            &display,
            rtformat,
            width,
            height,
            &mut [SurfaceAttribEnum::PixelFormat(format).into()],
        )?;

        // Try to use `vaDeriveImage` first, fall back if that fails.
        match surface.derive_image() {
            Ok(image) => {
                log::trace!(
                    "using vaDeriveImage for fast surface access \
                    (surface format = {:?}, image format = {:?})",
                    rtformat,
                    format,
                );

                Ok(Self {
                    surface,
                    image,
                    derived: true,
                })
            }
            Err(e) if e.as_libva() == Some(VAError::ERROR_OPERATION_FAILED) => {
                log::trace!("vaDeriveImage not supported, using vaGetImage (surface format = {:?}, image format = {:?})", rtformat, format);

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
    pub fn image(&self) -> &Image {
        &self.image
    }

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
