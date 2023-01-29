use core::fmt;
use std::{
    ffi::{c_void, CStr},
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{Deref, DerefMut},
    os::raw::{c_char, c_int, c_uint},
    panic::catch_unwind,
    ptr,
    sync::Arc,
    time::Instant,
    vec,
};

use bytemuck::{AnyBitPattern, NoUninit, Pod};
use dlopen::{libva, libva_drm, libva_wayland, libva_x11};
use raw_window_handle::RawDisplayHandle;

#[macro_use]
mod macros;
mod dlopen;
mod error;
pub mod jpeg;
mod raw;
mod shared;
pub mod vpp;

pub use error::Error;
pub use pixelformat::PixelFormat;
pub use shared::*;

use raw::{
    VABufferID, VAConfigID, VAContextID, VADisplay, VADisplayAttribute, VAGenericFunc,
    VAGenericValue, VAImage, VASurfaceAttrib, VASurfaceID, VA_TIMEOUT_INFINITE,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DisplayApi {
    Xlib,
    Wayland,
    Drm,
}

/// Owns a VADisplay and destroys it on drop.
struct DisplayOwner {
    raw: VADisplay,
    libva: &'static libva,
}

impl fmt::Debug for DisplayOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisplayOwner")
            .field("raw", &self.raw)
            .finish()
    }
}

impl Drop for DisplayOwner {
    fn drop(&mut self) {
        unsafe {
            check_log(self.libva.vaTerminate(self.raw), "vaTerminate call in drop");
        }
    }
}

/// Main entry point into the library.
pub struct Display {
    d: Arc<DisplayOwner>,
    libva: &'static libva,
    api: DisplayApi,
    major: u32,
    minor: u32,
}

impl Display {
    pub fn new(handle: RawDisplayHandle) -> Result<Self> {
        unsafe {
            let raw: VADisplay;
            let api = match handle {
                RawDisplayHandle::Xlib(d) => {
                    raw = libva_x11::get()
                        .map_err(Error::from)?
                        .vaGetDisplay(d.display.cast());
                    DisplayApi::Xlib
                }
                RawDisplayHandle::Wayland(d) => {
                    raw = libva_wayland::get()
                        .map_err(Error::from)?
                        .vaGetDisplayWl(d.display.cast());
                    DisplayApi::Wayland
                }
                RawDisplayHandle::Drm(d) => {
                    raw = libva_drm::get().map_err(Error::from)?.vaGetDisplayDRM(d.fd);
                    DisplayApi::Drm
                }
                _ => {
                    return Err(Error::from(format!(
                        "unsupported display handle type: {:?}",
                        handle
                    )));
                }
            };

            let libva = libva::get().map_err(Error::from)?;
            let valid = libva.vaDisplayIsValid(raw);
            if valid == 0 {
                return Err(Error::from(format!(
                    "failed to create VADisplay from window handle {:?}",
                    handle
                )));
            }

            libva.vaSetErrorCallback(raw, error_callback, ptr::null_mut());
            libva.vaSetInfoCallback(raw, info_callback, ptr::null_mut());

            let mut major = 0;
            let mut minor = 0;
            check(libva.vaInitialize(raw, &mut major, &mut minor))?;

            log::info!("initialized libva {major}.{minor}");

            Ok(Self {
                d: Arc::new(DisplayOwner { raw, libva }),
                libva,
                api,
                major: major as _,
                minor: minor as _,
            })
        }
    }

    #[inline]
    pub fn version_major(&self) -> u32 {
        self.major
    }

    #[inline]
    pub fn version_minor(&self) -> u32 {
        self.minor
    }

    #[inline]
    pub fn display_api(&self) -> DisplayApi {
        self.api
    }

    pub fn query_vendor_string(&self) -> Result<&str> {
        unsafe {
            let cstr = CStr::from_ptr(self.libva.vaQueryVendorString(self.d.raw));
            cstr.to_str().map_err(Error::from)
        }
    }

    pub fn query_profiles(&self) -> Result<Profiles> {
        unsafe {
            let max = self.libva.vaMaxNumProfiles(self.d.raw) as usize;
            let mut profiles = Vec::with_capacity(max);
            let mut num = 0;
            check(
                self.libva
                    .vaQueryConfigProfiles(self.d.raw, profiles.as_mut_ptr(), &mut num),
            )?;
            profiles.set_len(num as usize);
            Ok(Profiles { vec: profiles })
        }
    }

    pub fn query_entrypoints(&self, profile: Profile) -> Result<Entrypoints> {
        unsafe {
            let max = self.libva.vaMaxNumEntrypoints(self.d.raw) as usize;
            let mut entrypoints = Vec::with_capacity(max);
            let mut num = 0;
            check(self.libva.vaQueryConfigEntrypoints(
                self.d.raw,
                profile,
                entrypoints.as_mut_ptr(),
                &mut num,
            ))?;
            entrypoints.set_len(num as usize);
            Ok(Entrypoints { vec: entrypoints })
        }
    }

    pub fn query_image_formats(&self) -> Result<ImageFormats> {
        unsafe {
            let max = self.libva.vaMaxNumImageFormats(self.d.raw) as usize;
            let mut formats = Vec::with_capacity(max);
            let mut num = 0;
            check(
                self.libva
                    .vaQueryImageFormats(self.d.raw, formats.as_mut_ptr(), &mut num),
            )?;
            formats.set_len(num as usize);
            Ok(ImageFormats { vec: formats })
        }
    }

    pub fn query_display_attributes(&self) -> Result<DisplayAttributes> {
        unsafe {
            let max = self.libva.vaMaxNumDisplayAttributes(self.d.raw) as usize;
            let mut attribs = Vec::with_capacity(max);
            let mut num = 0;
            check(
                self.libva
                    .vaQueryDisplayAttributes(self.d.raw, attribs.as_mut_ptr(), &mut num),
            )?;
            attribs.set_len(num as usize);
            Ok(DisplayAttributes { vec: attribs })
        }
    }

    pub fn create_default_config(
        &self,
        profile: Profile,
        entrypoint: Entrypoint,
    ) -> Result<Config> {
        unsafe {
            let mut config_id = 0;
            check(self.libva.vaCreateConfig(
                self.d.raw,
                profile,
                entrypoint,
                ptr::null_mut(),
                0,
                &mut config_id,
            ))?;
            Ok(Config {
                d: self.d.clone(),
                id: config_id,
            })
        }
    }

    pub fn create_surface(
        &self,
        format: RTFormat,
        width: u32,
        height: u32,
        attribs: &mut [VASurfaceAttrib],
    ) -> Result<Surface> {
        let mut id = 0;
        unsafe {
            check(self.d.libva.vaCreateSurfaces(
                self.d.raw,
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
            d: self.d.clone(),
            id,
        })
    }

    pub fn create_image(&self, mut format: ImageFormat, width: u32, height: u32) -> Result<Image> {
        let width: c_int = width.try_into().map_err(Error::from)?;
        let height: c_int = height.try_into().map_err(Error::from)?;
        let mut image = MaybeUninit::uninit();
        unsafe {
            check(self.libva.vaCreateImage(
                self.d.raw,
                &mut format,
                width,
                height,
                image.as_mut_ptr(),
            ))?;
            Ok(Image {
                d: self.d.clone(),
                raw: image.assume_init(),
            })
        }
    }

    pub fn set_driver_name(&mut self, name: &str) -> Result<()> {
        let mut buf;
        let mut name = name.as_bytes();
        if name.last() != Some(&0) {
            buf = Vec::with_capacity(name.len() + 1);
            buf.extend_from_slice(name);
            buf.push(0);
            name = &buf;
        }
        unsafe {
            // NB: casting to a mutable pointer - libva doesn't modify the string, and other code
            // would probably break if it did
            check(
                self.libva
                    .vaSetDriverName(self.d.raw, name.as_ptr() as *mut c_char),
            )
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
    /// Creates a surface and image of the given resolution and pixel formats.
    pub fn with_default_format(display: &Display, width: u32, height: u32) -> Result<Self> {
        let mut surface = display.create_surface(RTFormat::default(), width, height, &mut [])?;

        // Try to use `vaDeriveImage` first, fall back if that fails.
        match surface.derive_image() {
            Ok(image) => {
                log::trace!("using vaDeriveImage for fast surface access");

                Ok(Self {
                    surface,
                    image,
                    derived: true,
                })
            }
            Err(e) if e.as_libva() == Some(VAError::ERROR_OPERATION_FAILED) => {
                log::trace!("vaDeriveImage not supported, using vaGetImage");

                let image = display.create_image(ImageFormat::default(), width, height)?;
                Ok(Self {
                    surface,
                    image,
                    derived: false,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn with_format(
        display: &Display,
        width: u32,
        height: u32,
        surface_format: RTFormat,
        image_format: PixelFormat,
    ) -> Result<Self> {
        // `vaDeriveImage` gives us an arbitrary image format, so we don't use that here.
        let surface = display.create_surface(surface_format, width, height, &mut [])?;

        let image = display.create_image(ImageFormat::new(image_format), width, height)?;
        Ok(Self {
            surface,
            image,
            derived: false,
        })
    }

    #[inline]
    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn map_sync(&mut self) -> Result<Mapping<'_, u8>> {
        if !self.derived {
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
pub struct Surface {
    d: Arc<DisplayOwner>,
    id: VASurfaceID,
}

impl Surface {
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
    pub fn copy_to_image(&mut self, image: &mut Image) -> Result<()> {
        self.sync()?;

        let start = Instant::now();

        unsafe {
            check(self.d.libva.vaGetImage(
                self.d.raw,
                self.id,
                0,
                0,
                image.raw.width.into(),
                image.raw.height.into(),
                image.raw.image_id,
            ))?;
        }

        log::trace!("vaGetImage took {:?}", start.elapsed());

        Ok(())
    }

    /// Creates an [`Image`] that allow direct access to the surface's image data.
    ///
    /// Only supported by some drivers. Will return [`VAError::ERROR_OPERATION_FAILED`] if it's not
    /// supported.
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

#[derive(Debug)]
pub struct Image {
    d: Arc<DisplayOwner>,
    raw: VAImage,
}

impl Image {
    #[inline]
    pub fn pixelformat(&self) -> PixelFormat {
        self.raw.format.fourcc
    }

    /// Maps the [`Buffer`] storing the backing data of this [`Image`].
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

pub struct Config {
    d: Arc<DisplayOwner>,
    id: VAConfigID,
}

impl Config {
    pub fn query_surface_attributes(&self) -> Result<SurfaceAttributes> {
        unsafe {
            let mut num_attribs = 0;
            let status = self.d.libva.vaQuerySurfaceAttributes(
                self.d.raw,
                self.id,
                ptr::null_mut(),
                &mut num_attribs,
            );
            if status != VAStatus::SUCCESS && status != VAError::ERROR_MAX_NUM_EXCEEDED {
                return Err(check(status).unwrap_err());
            }

            let mut attribs = Vec::with_capacity(num_attribs as usize);
            check(self.d.libva.vaQuerySurfaceAttributes(
                self.d.raw,
                self.id,
                attribs.as_mut_ptr(),
                &mut num_attribs,
            ))?;
            attribs.set_len(num_attribs as usize);
            Ok(SurfaceAttributes { vec: attribs })
        }
    }

    pub fn create_default_context(
        &self,
        picture_width: u32,
        picture_height: u32,
    ) -> Result<Context> {
        unsafe {
            let mut context_id = 0;
            check(self.d.libva.vaCreateContext(
                self.d.raw,
                self.id,
                picture_width as _,
                picture_height as _,
                0,
                ptr::null_mut(),
                0,
                &mut context_id,
            ))?;
            Ok(Context {
                d: self.d.clone(),
                id: context_id,
            })
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyConfig(self.d.raw, self.id),
                "vaDestroyConfig call in drop",
            );
        }
    }
}

pub struct Context {
    d: Arc<DisplayOwner>,
    id: VAContextID,
}

impl Context {
    pub fn create_empty_buffer<T: NoUninit>(
        &self,
        buf_ty: BufferType,
        num_elements: usize,
    ) -> Result<Buffer<T>> {
        let mut buf_id = 0;
        unsafe {
            check(self.d.libva.vaCreateBuffer(
                self.d.raw,
                self.id,
                buf_ty,
                mem::size_of::<T>() as c_uint,
                c_uint::try_from(num_elements).unwrap(),
                ptr::null_mut(),
                &mut buf_id,
            ))?;
        }
        Ok(Buffer {
            d: self.d.clone(),
            id: buf_id,
            capacity: num_elements,
            _p: PhantomData,
        })
    }

    /// Creates a [`Buffer`] of the specified [`BufferType`], containing raw data bytes.
    pub fn create_data_buffer(&self, buf_ty: BufferType, data: &[u8]) -> Result<Buffer<u8>> {
        let mut buf_id = 0;
        unsafe {
            check(self.d.libva.vaCreateBuffer(
                self.d.raw,
                self.id,
                buf_ty,
                c_uint::try_from(data.len()).unwrap(),
                1,
                data.as_ptr() as *mut _,
                &mut buf_id,
            ))?;
        }
        Ok(Buffer {
            d: self.d.clone(),
            id: buf_id,
            capacity: data.len(),
            _p: PhantomData,
        })
    }

    /// Creates a [`Buffer`] of the specified [`BufferType`], containing an instance of `T`.
    ///
    /// This is primarily used to pass individual parameter structures to libva.
    pub fn create_param_buffer<T: Copy>(
        &self,
        buf_ty: BufferType,
        mut content: T,
    ) -> Result<Buffer<T>> {
        let mut buf_id = 0;
        unsafe {
            check(self.d.libva.vaCreateBuffer(
                self.d.raw,
                self.id,
                buf_ty,
                mem::size_of::<T>() as c_uint,
                1,
                &mut content as *mut _ as *mut c_void,
                &mut buf_id,
            ))?;
        }
        Ok(Buffer {
            d: self.d.clone(),
            id: buf_id,
            capacity: 1,
            _p: PhantomData,
        })
    }

    pub fn begin_picture<'a>(
        &'a mut self,
        target: &'a mut Surface,
    ) -> Result<InProgressPicture<'a>> {
        unsafe {
            check(self.d.libva.vaBeginPicture(self.d.raw, self.id, target.id))?;
        }

        Ok(InProgressPicture {
            d: self.d.clone(),
            context: self,
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyContext(self.d.raw, self.id),
                "vaDestroyContext call in drop",
            );
        }
    }
}

pub struct InProgressPicture<'a> {
    d: Arc<DisplayOwner>,
    context: &'a mut Context,
}

impl<'a> InProgressPicture<'a> {
    /// Submits a [`Buffer`] as part of this libva operation.
    ///
    /// Typically, libva does not document which buffer types are required for any given entry
    /// point, so good luck!
    pub fn render_picture<T>(&mut self, buffer: &mut Buffer<T>) -> Result<()> {
        unsafe {
            check(
                self.d
                    .libva
                    .vaRenderPicture(self.d.raw, self.context.id, &mut buffer.id, 1),
            )
        }
    }

    /// Finishes submitting buffers, and begins the libva operation (encode, decode, etc.).
    ///
    /// # Safety
    ///
    /// libva does not specify when Undefined Behavior occurs, and in practice at least some
    /// implementations exhibit UB-like behavior when buffers where submitted incorrectly (or when
    /// not all buffers required by the operation were submitted).
    ///
    /// So, basically, the safety invariant of this method is "fuck if I know". Good luck, Loser.
    pub unsafe fn end_picture(self) -> Result<()> {
        check(self.d.libva.vaEndPicture(self.d.raw, self.context.id))
    }
}

/// A buffer that holds elements of type `T`.
pub struct Buffer<T: 'static> {
    d: Arc<DisplayOwner>,
    id: VABufferID,
    capacity: usize,
    _p: PhantomData<T>,
}

impl<T: 'static> Buffer<T> {
    pub fn map(&mut self) -> Result<Mapping<'_, T>> {
        let mut ptr = ptr::null_mut();
        unsafe {
            check(self.d.libva.vaMapBuffer(self.d.raw, self.id, &mut ptr))?;
        }
        Ok(Mapping {
            d: &self.d,
            id: self.id,
            ptr: ptr.cast(),
            capacity: self.capacity,
        })
    }

    pub fn sync(&mut self) -> Result<()> {
        unsafe {
            check(
                self.d
                    .libva
                    .vaSyncBuffer(self.d.raw, self.id, VA_TIMEOUT_INFINITE),
            )
        }
    }
}

impl<T: 'static> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyBuffer(self.d.raw, self.id),
                "vaDestroyBuffer call in drop",
            );
        }
    }
}

/// A handle to the memory-mapped data of a [`Buffer`].
///
/// A [`Mapping`] can be accessed in 3 ways:
///
/// - [`Deref`] allows read access and is implemented if `T` implements [`AnyBitPattern`].
/// - [`DerefMut`] allows read and write access and is implemented if `T` implements [`Pod`].
/// - [`Mapping::write`] is implemented if `T` implements [`Copy`], but only allows storing a value
///   in the buffer.
pub struct Mapping<'a, T> {
    d: &'a DisplayOwner,
    id: VABufferID,
    ptr: *mut T,
    capacity: usize,
}

impl<'a, T: Copy> Mapping<'a, T> {
    pub fn write(&mut self, index: usize, value: T) {
        assert!(index < self.capacity && index < isize::MAX as usize);
        unsafe {
            self.ptr.offset(index as isize).write(value);
        }
    }
}

impl<'a, T: AnyBitPattern> Deref for Mapping<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr.cast(), self.capacity) }
    }
}

impl<'a, T: Pod> DerefMut for Mapping<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.cast(), self.capacity) }
    }
}

impl<'a, T> Drop for Mapping<'a, T> {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaUnmapBuffer(self.d.raw, self.id),
                "vaUnmapBuffer call in drop",
            );
        }
    }
}

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

#[derive(Clone)]
pub struct DisplayAttributes {
    vec: Vec<VADisplayAttribute>,
}

impl DisplayAttributes {
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
}

impl IntoIterator for DisplayAttributes {
    type Item = VADisplayAttribute;
    type IntoIter = vec::IntoIter<VADisplayAttribute>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

impl VADisplayAttribute {
    pub fn ty(&self) -> VADisplayAttribType {
        self.type_
    }

    pub fn min_value(&self) -> i32 {
        self.min_value
    }

    pub fn max_value(&self) -> i32 {
        self.max_value
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn flags(&self) -> VADisplayAttribFlags {
        self.flags
    }
}

#[derive(Clone)]
pub struct ImageFormats {
    vec: Vec<ImageFormat>,
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
    type IntoIter = vec::IntoIter<ImageFormat>; // TODO wrap C type

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

#[derive(Clone)]
pub struct SurfaceAttributes {
    vec: Vec<VASurfaceAttrib>,
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
        self.vec.iter().filter_map(|attr| match attr.as_enum()? {
            SurfaceAttribEnum::PixelFormat(f) => Some(f),
            _ => None,
        })
    }
}

impl IntoIterator for SurfaceAttributes {
    type Item = VASurfaceAttrib;
    type IntoIter = vec::IntoIter<VASurfaceAttrib>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

impl VASurfaceAttrib {
    #[inline]
    pub fn ty(&self) -> VASurfaceAttribType {
        self.type_
    }

    #[inline]
    pub fn flags(&self) -> VASurfaceAttribFlags {
        self.flags
    }

    pub fn is_readable(&self) -> bool {
        self.flags.contains(VASurfaceAttribFlags::GETTABLE)
    }

    pub fn is_writable(&self) -> bool {
        self.flags.contains(VASurfaceAttribFlags::SETTABLE)
    }

    pub fn as_enum(&self) -> Option<SurfaceAttribEnum> {
        Some(match self.type_ {
            VASurfaceAttribType::PixelFormat => {
                SurfaceAttribEnum::PixelFormat(PixelFormat::from_u32_le(self.as_int()? as u32))
            }
            VASurfaceAttribType::MemoryType => unsafe {
                SurfaceAttribEnum::MemoryType(VASurfaceAttribMemoryType::from_bits_unchecked(
                    self.as_int()? as u32,
                ))
            },
            _ => return None,
        })
    }

    pub fn raw_value(&self) -> Option<GenericValue> {
        unsafe { GenericValue::from_raw(self.value) }
    }

    pub fn as_int(&self) -> Option<i32> {
        self.raw_value().and_then(GenericValue::as_int)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum SurfaceAttribEnum {
    PixelFormat(PixelFormat),
    MemoryType(VASurfaceAttribMemoryType),
}

mod pixelformat;

#[derive(Debug, Clone, Copy)]
pub enum GenericValue {
    Int(i32),
    Float(f32),
    Pointer(*mut c_void),
    Func(VAGenericFunc),
}

impl GenericValue {
    unsafe fn from_raw(raw: VAGenericValue) -> Option<Self> {
        Some(match raw.type_ {
            VAGenericValueType::Integer => Self::Int(raw.value.i),
            VAGenericValueType::Float => Self::Float(raw.value.f),
            VAGenericValueType::Pointer => Self::Pointer(raw.value.p),
            VAGenericValueType::Func => Self::Func(raw.value.func),
            _ => return None,
        })
    }

    pub fn as_int(self) -> Option<i32> {
        match self {
            Self::Int(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_float(self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_ptr(self) -> Option<*mut c_void> {
        match self {
            Self::Pointer(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_func(self) -> Option<VAGenericFunc> {
        match self {
            Self::Func(v) => Some(v),
            _ => None,
        }
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

extern "C" fn error_callback(_ctx: *mut c_void, message: *const c_char) {
    catch_unwind(|| unsafe {
        let cstr = CStr::from_ptr(message);
        match cstr.to_str() {
            Ok(s) => {
                log::error!("libva: {}", s.trim());
            }
            Err(e) => {
                log::error!("failed to decode libva error: {e}");
            }
        }
    })
    .ok();
}

extern "C" fn info_callback(_ctx: *mut c_void, message: *const c_char) {
    catch_unwind(|| unsafe {
        let cstr = CStr::from_ptr(message);
        match cstr.to_str() {
            Ok(s) => {
                log::info!("libva: {}", s.trim());
            }
            Err(e) => {
                log::error!("failed to decode libva info message: {e}");
            }
        }
    })
    .ok();
}
