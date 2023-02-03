#![allow(bad_style)]

use std::{
    ffi::c_void,
    os::raw::{c_char, c_float, c_int, c_uchar, c_uint},
};

use crate::raw::{vpp::VAProcPipelineCaps, *};
use crate::shared::{vpp::*, *};

use once_cell::sync::OnceCell;

/// `dylib! {}`
macro_rules! dylib {
    (
        pub struct $strukt:ident;

        $(
            fn $func:ident( $( $name:ident : $t:ty ),* $(,)? ) $( -> $ret:ty )?;
        )+
    ) => {
        $(
            pub type $func = unsafe extern "C" fn( $( $name : $t ),* ) $( -> $ret )?;
        )+

        pub struct $strukt {
            $(
                $func: $func,
            )+
        }

        #[allow(unused)]
        impl $strukt {
            fn load() -> Result<Self, libloading::Error> {
                unsafe {
                    let libname = concat!(stringify!($strukt), ".so").replace('_', "-");
                    let lib = libloading::Library::new(&libname)?;

                    let this = Self {
                        $(
                            $func: *lib.get(concat!(stringify!($func), "\0").as_bytes())?,
                        )+
                    };

                    // Ensure the library is never unloaded.
                    std::mem::forget(lib);

                    Ok(this)
                }
            }

            pub fn get() -> Result<&'static Self, libloading::Error> {
                static CELL: OnceCell<$strukt> = OnceCell::new();
                CELL.get_or_try_init(Self::load)
            }

            $(
                pub unsafe fn $func( &self, $( $name : $t ),* ) $( -> $ret )? {
                    (self.$func)($($name),*)
                }
            )+
        }
    };
}

dylib! {
    pub struct libva;

    fn vaErrorStr(error_status: VAStatus) -> *const c_char;
    fn vaSetErrorCallback(dpy: VADisplay, callback: VAMessageCallback, user_context: *mut c_void);
    fn vaSetInfoCallback(dpy: VADisplay, callback: VAMessageCallback, user_context: *mut c_void);
    fn vaDisplayIsValid(dpy: VADisplay) -> c_int;
    fn vaSetDriverName(dpy: VADisplay, driver_name: *mut c_char) -> VAStatus;
    fn vaInitialize(dpy: VADisplay, major_version: *mut c_int, minor_version: *mut c_int) -> VAStatus;
    fn vaTerminate(dpy: VADisplay) -> VAStatus;
    fn vaQueryVendorString(dpy: VADisplay) -> *const c_char;
    fn vaGetLibFunc(dpy: VADisplay, func: *const c_char) -> VAPrivFunc;
    fn vaMaxNumProfiles(dpy: VADisplay) -> c_int;
    fn vaMaxNumEntrypoints(dpy: VADisplay) -> c_int;
    fn vaMaxNumConfigAttributes(dpy: VADisplay) -> c_int;
    fn vaQueryConfigProfiles(dpy: VADisplay, profile_list: *mut Profile, num_profiles: *mut c_int) -> VAStatus;
    fn vaQueryConfigEntrypoints(dpy: VADisplay, profile: Profile, entrypoint_list: *mut Entrypoint, num_entrypoints: *mut c_int) -> VAStatus;
    fn vaGetConfigAttributes(dpy: VADisplay, profile: Profile, entrypoint: Entrypoint, attrib_list: *mut ConfigAttrib, num_attribs: c_int) -> VAStatus;
    fn vaCreateConfig(dpy: VADisplay, profile: Profile, entrypoint: Entrypoint, attrib_list: *mut ConfigAttrib, num_attribs: c_int, config_id: *mut VAConfigID) -> VAStatus;
    fn vaDestroyConfig(dpy: VADisplay, config_id: VAConfigID) -> VAStatus;
    fn vaQueryConfigAttributes(dpy: VADisplay, config_id: VAConfigID, profile: *mut Profile, entrypoint: *mut Entrypoint, attrib_list: *mut ConfigAttrib, num_attribs: *mut c_int) -> VAStatus;
    fn vaQuerySurfaceAttributes(dpy: VADisplay, config: VAConfigID, attrib_list: *mut SurfaceAttrib, num_attribs: *mut c_uint) -> VAStatus;
    fn vaCreateSurfaces(dpy: VADisplay, format: RTFormat, width: c_uint, height: c_uint, surfaces: *mut VASurfaceID, num_surfaces: c_uint, attrib_list: *mut SurfaceAttrib, num_attribs: c_uint) -> VAStatus;
    fn vaDestroySurfaces(dpy: VADisplay, surfaces: *mut VASurfaceID, num_surfaces: c_int) -> VAStatus;
    fn vaCreateContext(dpy: VADisplay, config_id: VAConfigID, picture_width: c_int, picture_height: c_int, flag: c_int, render_targets: *mut VASurfaceID, num_render_targets: c_int, context: *mut VAContextID) -> VAStatus;
    fn vaDestroyContext(dpy: VADisplay, context: VAContextID) -> VAStatus;
    fn vaCreateMFContext(dpy: VADisplay, mf_context: *mut VAMFContextID) -> VAStatus;
    // (some missing)
    fn vaQueryProcessingRate(dpy: VADisplay, config: VAConfigID, proc_buf: *mut VAProcessingRateParameter, processing_rate: *mut c_uint) -> VAStatus;
    fn vaCreateBuffer(dpy: VADisplay, context: VAContextID, type_: BufferType, size: c_uint, num_elements: c_uint, data: *mut c_void, buf_id: *mut VABufferID) -> VAStatus;
    fn vaCreateBuffer2(dpy: VADisplay, context: VAContextID, type_: BufferType, width: c_uint, height: c_uint, unit_size: *mut c_uint, pitch: *mut c_uint, buf_id: *mut VABufferID) -> VAStatus;
    fn vaBufferSetNumElements(dpy: VADisplay, buf_id: VABufferID, num_elements: c_uint) -> VAStatus;
    fn vaMapBuffer(dpy: VADisplay, buf_id: VABufferID, pbuf: *mut *mut c_void) -> VAStatus;
    fn vaUnmapBuffer(dpy: VADisplay, buf_id: VABufferID) -> VAStatus;
    fn vaDestroyBuffer(dpy: VADisplay, buffer_id: VABufferID) -> VAStatus;
    fn vaAcquireBufferHandle(dpy: VADisplay, buf_id: VABufferID, buf_info: *mut VABufferInfo) -> VAStatus;
    fn vaReleaseBufferHandle(dpy: VADisplay, buf_id: VABufferID) -> VAStatus;
    fn vaExportSurfaceHandle(dpy: VADisplay, surface_id: VASurfaceID, mem_type: SurfaceAttribMemoryType, flags: VAExportSurface, descriptor: *mut c_void) -> VAStatus;
    fn vaBeginPicture(dpy: VADisplay, context: VAContextID, render_target: VASurfaceID) -> VAStatus;
    fn vaRenderPicture(dpy: VADisplay, context: VAContextID, buffers: *mut VABufferID, num_buffers: c_int) -> VAStatus;
    fn vaEndPicture(dpy: VADisplay, context: VAContextID) -> VAStatus;
    fn vaMFSubmit(dpy: VADisplay, mf_context: VAMFContextID, contexts: *mut VAContextID, num_contexts: c_int) -> VAStatus;
    fn vaSyncSurface(dpy: VADisplay, render_target: VASurfaceID) -> VAStatus;
    fn vaSyncSurface2(dpy: VADisplay, surface: VASurfaceID, timeout_ns: u64) -> VAStatus;
    fn vaQuerySurfaceStatus(dpy: VADisplay, render_target: VASurfaceID, status: *mut SurfaceStatus) -> VAStatus;
    fn vaQuerySurfaceError(dpy: VADisplay, surface: VASurfaceID, error_status: VAStatus, error_info: *mut *mut c_void) -> VAStatus;
    fn vaSyncBuffer(dpy: VADisplay, buf_id: VABufferID, timeout_ns: u64) -> VAStatus;
    fn vaMaxNumImageFormats(dpy: VADisplay) -> c_int;
    fn vaQueryImageFormats(dpy: VADisplay, format_list: *mut ImageFormat, num_formats: *mut c_int) -> VAStatus;
    fn vaCreateImage(dpy: VADisplay, format: *mut ImageFormat, width: c_int, height: c_int, image: *mut VAImage) -> VAStatus;
    fn vaDestroyImage(dpy: VADisplay, image: VAImageID) -> VAStatus;
    fn vaSetImagePalette(dpy: VADisplay, image: VAImageID, palette: *mut c_uchar) -> VAStatus;
    fn vaGetImage(dpy: VADisplay, surface: VASurfaceID, x: c_int, y: c_int, width: c_uint, height: c_uint, image: VAImageID) -> VAStatus;
    fn vaPutImage(dpy: VADisplay, surface: VASurfaceID, image: VAImageID, src_x: c_int, src_y: c_int, src_width: c_uint, src_height: c_uint, dest_x: c_int, dest_y: c_int, dest_width: c_uint, dest_height: c_uint) -> VAStatus;
    fn vaDeriveImage(dpy: VADisplay, surface: VASurfaceID, image: *mut VAImage) -> VAStatus;
    fn vaMaxNumSubpictureFormats(dpy: VADisplay) -> c_int;
    fn vaQuerySubpictureFormats(dpy: VADisplay, format_list: *mut ImageFormat, flags: *mut c_uint, num_formats: *mut c_uint) -> VAStatus;
    fn vaCreateSubpicture(dpy: VADisplay, image: VAImageID, subpicture: *mut VASubpictureID) -> VAStatus;
    fn vaDestroySubpicture(dpy: VADisplay, subpicture: VASubpictureID) -> VAStatus;
    fn vaSetSubpictureImage(dpy: VADisplay, subpicture: VASubpictureID, image: VAImageID) -> VAStatus;
    fn vaSetSubpictureChromakey(dpy: VADisplay, subpicture: VASubpictureID, chromakey_min: c_uint, chromakey_max: c_uint, chromakey_mask: c_uint) -> VAStatus;
    fn vaSetSubpictureGlobalAlpha(dpy: VADisplay, subpicture: VASubpictureID, global_alpha: c_float) -> VAStatus;
    fn vaAssociateSubpicture(dpy: VADisplay, subpicture: VASubpictureID, target_surfaces: *mut VASurfaceID, num_surfaces: c_int, src_x: i32, src_y: i32, src_width: u16, src_height: u16, dest_x: i16, dest_y: i16, dest_width: u16, dest_height: u16, flags: VASubpictureFlags) -> VAStatus;
    fn vaDeassociateSubpicture(dpy: VADisplay, subpicture: VASubpictureID, target_surfaces: *mut VASurfaceID, num_surfaces: c_int) -> VAStatus;
    fn vaMaxNumDisplayAttributes(dpy: VADisplay) -> c_int;
    fn vaQueryDisplayAttributes(dpy: VADisplay, attr_list: *mut VADisplayAttribute, num_attributes: *mut c_int) -> VAStatus;
    fn vaGetDisplayAttributes(dpy: VADisplay, attr_list: *mut VADisplayAttribute, num_attributes: c_int) -> VAStatus;
    fn vaSetDisplayAttributes(dpy: VADisplay, attr_list: *mut VADisplayAttribute, num_attributes: c_int) -> VAStatus;

    fn vaQueryVideoProcFilters(dpy: VADisplay, context: VAContextID, filters: *mut FilterType, num_filters: *mut c_uint) -> VAStatus;
    fn vaQueryVideoProcFilterCaps(dpy: VADisplay, context: VAContextID, type_: FilterType, filter_caps: *mut c_void, num_filter_caps: *mut c_uint) -> VAStatus;
    fn vaQueryVideoProcPipelineCaps(dpy: VADisplay, context: VAContextID, filters: *mut VABufferID, num_filters: c_uint, pipeline_caps: *mut VAProcPipelineCaps) -> VAStatus;
}

dylib! {
    pub struct libva_x11;

    fn vaGetDisplay(dpy: *mut Display) -> VADisplay;
}

dylib! {
    pub struct libva_wayland;

    fn vaGetDisplayWl(display: *mut wl_display) -> VADisplay;
}

dylib! {
    pub struct libva_drm;

    fn vaGetDisplayDRM(fd: c_int) -> VADisplay;
}

pub struct wl_display;

/// Opaque type representing the Xlib X11 `Display` type.
pub struct Display;
