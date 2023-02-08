//! Display API access and attributes.

use core::fmt;
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    mem,
    panic::catch_unwind,
    ptr,
    sync::Arc,
    vec,
};

use raw_window_handle::{HasRawDisplayHandle, RawDisplayHandle};

use crate::{
    check, check_log,
    dlopen::{libva, libva_drm, libva_wayland, libva_x11},
    image::{ImageFormat, ImageFormats},
    raw::{VADisplay, VA_PADDING_LOW},
    subpicture::{SubpictureFlags, SubpictureFormats},
    Entrypoint, Entrypoints, Error, Profile, Profiles, Result,
};

ffi_enum! {
    pub enum DisplayAttribType: c_int {
        Brightness          = 0,
        Contrast            = 1,
        Hue                 = 2,
        Saturation          = 3,
        BackgroundColor     = 4,
        DirectSurface       = 5,
        Rotation            = 6,
        OutofLoopDeblock    = 7,
        BLEBlackMode        = 8,
        BLEWhiteMode        = 9,
        BlueStretch         = 10,
        SkinColorCorrection = 11,
        CSCMatrix           = 12,
        BlendColor          = 13,
        OverlayAutoPaintColorKey = 14,
        OverlayColorKey     = 15,
        RenderMode          = 16,
        RenderDevice        = 17,
        RenderRect          = 18,
        SubDevice           = 19,
        Copy                = 20,
        PCIID               = 21,
    }
}

bitflags! {
    pub struct DisplayAttribFlags: u32 {
        const GETTABLE = 0x0001;
        const SETTABLE = 0x0002;
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct DisplayAttribute {
    pub(crate) type_: DisplayAttribType,
    pub(crate) min_value: i32,
    pub(crate) max_value: i32,
    pub(crate) value: i32,
    pub(crate) flags: DisplayAttribFlags,
    va_reserved: [u32; VA_PADDING_LOW],
}

impl DisplayAttribute {
    pub(crate) fn zeroed() -> Self {
        unsafe { mem::zeroed() }
    }

    pub fn new(ty: DisplayAttribType, value: i32) -> Self {
        let mut this: Self = unsafe { std::mem::zeroed() };
        this.type_ = ty;
        this.value = value;
        this
    }

    pub fn ty(&self) -> DisplayAttribType {
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

    pub fn flags(&self) -> DisplayAttribFlags {
        self.flags
    }
}

#[derive(Clone)]
pub struct DisplayAttributes {
    pub(crate) vec: Vec<DisplayAttribute>,
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
    type Item = DisplayAttribute;
    type IntoIter = vec::IntoIter<DisplayAttribute>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

/// List of OS APIs that may be used to obtain a libva [`Display`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DisplayApi {
    Xlib,
    Wayland,
    Drm,
}

/// Owns a VADisplay and destroys it on drop.
pub(crate) struct DisplayOwner {
    pub(crate) raw: VADisplay,
    pub(crate) libva: &'static libva,
    #[allow(dead_code)]
    display_handle_owner: Option<Box<dyn HasRawDisplayHandle>>,
}

// Safety: VA-API clearly and unambiguously documents that it is thread-safe.
unsafe impl Send for DisplayOwner {}
unsafe impl Sync for DisplayOwner {}

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

/// The main entry point into the library.
///
/// [`Display`] wraps a native display handle and the corresponding libva implementation. It
/// provides methods for querying implementation capabilities and for creating libva objects. All
/// objects created from a [`Display`] will keep the underlying display handle and libva instance
/// alive even when the [`Display`] itself is dropped.
///
/// Creating a [`Display`] will initialize libva and set up its logging callbacks to forward
/// messages to the Rust `log` crate.
pub struct Display {
    pub(crate) d: Arc<DisplayOwner>,
    api: DisplayApi,
    major: u32,
    minor: u32,
}

impl Display {
    /// Opens a VA-API display from an owned display handle.
    ///
    /// This function takes ownership of `handle` to ensure that the native display handle isn't
    /// closed before the VA-API [`Display`] is dropped.
    pub fn new<H: HasRawDisplayHandle + 'static>(handle: H) -> Result<Self> {
        Self::new_impl(handle.raw_display_handle(), Some(Box::new(handle)))
    }

    /// Opens a VA-API display from a raw, native display handle with unmanaged lifetime.
    ///
    /// # Safety
    ///
    /// It is the user's responsibility to ensure that the native display handle `handle` remains
    /// valid until the last VA-API object created from this [`Display`] (including the [`Display`]
    /// itself) has been destroyed.
    pub unsafe fn new_unmanaged<H: HasRawDisplayHandle>(handle: &H) -> Result<Self> {
        Self::new_impl(handle.raw_display_handle(), None)
    }

    fn new_impl(
        handle: RawDisplayHandle,
        display_handle_owner: Option<Box<dyn HasRawDisplayHandle>>,
    ) -> Result<Self> {
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
                d: Arc::new(DisplayOwner {
                    raw,
                    libva,
                    display_handle_owner,
                }),
                api,
                major: major as _,
                minor: minor as _,
            })
        }
    }

    /// Returns the major part of the libva version.
    #[inline]
    pub fn version_major(&self) -> u32 {
        self.major
    }

    /// Returns the minor part of the libva version.
    #[inline]
    pub fn version_minor(&self) -> u32 {
        self.minor
    }

    /// Returns the [`DisplayApi`] that this [`Display`] is using.
    #[inline]
    pub fn display_api(&self) -> DisplayApi {
        self.api
    }

    /// Queries a string representing the vendor of the libva implementation.
    pub fn query_vendor_string(&self) -> Result<&str> {
        unsafe {
            let cstr = CStr::from_ptr(self.d.libva.vaQueryVendorString(self.d.raw));
            cstr.to_str().map_err(Error::from)
        }
    }

    /// Queries the supported [`Profiles`].
    pub fn query_profiles(&self) -> Result<Profiles> {
        let max = unsafe { self.d.libva.vaMaxNumProfiles(self.d.raw) as usize };
        let mut profiles = vec![Profile(0); max];
        let mut num = 0;
        unsafe {
            check(
                self.d
                    .libva
                    .vaQueryConfigProfiles(self.d.raw, profiles.as_mut_ptr(), &mut num),
            )?;
        }
        profiles.truncate(num as usize);
        Ok(Profiles { vec: profiles })
    }

    /// Queries supported [`Entrypoints`] for the given [`Profile`].
    pub fn query_entrypoints(&self, profile: Profile) -> Result<Entrypoints> {
        let max = unsafe { self.d.libva.vaMaxNumEntrypoints(self.d.raw) as usize };
        let mut entrypoints = vec![Entrypoint(0); max];
        let mut num = 0;
        unsafe {
            check(self.d.libva.vaQueryConfigEntrypoints(
                self.d.raw,
                profile,
                entrypoints.as_mut_ptr(),
                &mut num,
            ))?;
        }
        entrypoints.truncate(num as usize);
        Ok(Entrypoints { vec: entrypoints })
    }

    /// Queries the supported [`ImageFormat`][crate::image::ImageFormat]s.
    pub fn query_image_formats(&self) -> Result<ImageFormats> {
        unsafe {
            let max = self.d.libva.vaMaxNumImageFormats(self.d.raw) as usize;
            let mut formats = vec![ImageFormat::zeroed(); max];
            let mut num = 0;
            check(
                self.d
                    .libva
                    .vaQueryImageFormats(self.d.raw, formats.as_mut_ptr(), &mut num),
            )?;
            formats.truncate(num as usize);
            Ok(ImageFormats { vec: formats })
        }
    }

    pub fn query_subpicture_format(&self) -> Result<SubpictureFormats> {
        unsafe {
            let max = self.d.libva.vaMaxNumSubpictureFormats(self.d.raw) as usize;
            let mut formats = vec![ImageFormat::zeroed(); max];
            let mut flags: Vec<SubpictureFlags> = vec![SubpictureFlags::empty(); max];
            let mut num = 0;
            check(self.d.libva.vaQuerySubpictureFormats(
                self.d.raw,
                formats.as_mut_ptr(),
                flags.as_mut_ptr().cast(),
                &mut num,
            ))?;
            formats.truncate(num as usize);
            flags.truncate(num as usize);

            Ok(SubpictureFormats { formats, flags })
        }
    }

    pub fn query_display_attributes(&self) -> Result<DisplayAttributes> {
        let max = unsafe { self.d.libva.vaMaxNumDisplayAttributes(self.d.raw) as usize };
        let mut attribs = vec![DisplayAttribute::zeroed(); max];
        let mut num = 0;
        unsafe {
            check(self.d.libva.vaQueryDisplayAttributes(
                self.d.raw,
                attribs.as_mut_ptr(),
                &mut num,
            ))?;
        }
        attribs.truncate(num as usize);
        Ok(DisplayAttributes { vec: attribs })
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
                self.d
                    .libva
                    .vaSetDriverName(self.d.raw, name.as_ptr() as *mut c_char),
            )
        }
    }

    pub fn set_attributes(&mut self, attr_list: &mut [DisplayAttribute]) -> Result<()> {
        unsafe {
            check(self.d.libva.vaSetDisplayAttributes(
                self.d.raw,
                attr_list.as_mut_ptr(),
                attr_list.len().try_into().unwrap(),
            ))?;
            Ok(())
        }
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
