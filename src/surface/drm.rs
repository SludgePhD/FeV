//! DRM PRIME surface export/import.
//!
//! This wraps some of the functionality in `va_drmcommon.h`.
//!
//! Also see [`Surface::export_prime`].

use core::fmt;
use std::{mem::MaybeUninit, os::fd::RawFd};

use crate::{
    check,
    dlopen::{libva_wayland, wl_buffer},
    PixelFormat, Result,
};

use super::{ExportSurfaceFlags, Surface, SurfaceAttribMemoryType};

// TODO: do we need to wrap this in Rust type that owns and releases the fds?
// valgrind seems to indicate no (ie. they're closed automatically when some object is destroyed)

/// Describes how a [`Surface`] was exported to, or should be imported from, a set of DRM PRIME
/// objects.
///
/// Returned by [`Surface::export_prime`].
#[repr(C)]
pub struct PrimeSurfaceDescriptor {
    fourcc: PixelFormat,
    width: u32,
    height: u32,
    num_objects: u32,
    objects: [PrimeObject; 4],
    num_layers: u32,
    layers: [PrimeLayer; 4],
}

impl fmt::Debug for PrimeSurfaceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrimeSurfaceDescriptor")
            .field("fourcc", &self.fourcc)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("objects", &self.objects())
            .field("layers", &self.layers())
            .finish()
    }
}

impl PrimeSurfaceDescriptor {
    /// Returns the FourCC code of the overall PRIME surface (eg. [`PixelFormat::NV12`]).
    #[inline]
    pub fn fourcc(&self) -> PixelFormat {
        self.fourcc
    }

    /// Returns the width of the exported surface in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the exported surface in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the list of PRIME objects that make up the surface.
    ///
    /// There should be at least 1 object in here, but many multiplanar formats like
    /// [`PixelFormat::NV12`] are represented as two separate objects.
    #[inline]
    pub fn objects(&self) -> &[PrimeObject] {
        &self.objects[..self.num_objects as usize]
    }

    /// Returns the PRIME object at `index`.
    ///
    /// `index` is typically taken from [`PrimePlane::object_index`].
    ///
    /// # Panics
    ///
    /// This will panic if `index` is out of bounds.
    pub fn object(&self, index: u32) -> &PrimeObject {
        assert!(index < self.num_objects && index < 4);
        &self.objects[index as usize]
    }

    /// Returns the list of PRIME layers making up the surface.
    ///
    /// If [`ExportSurfaceFlags::COMPOSED_LAYERS`] was used to export the [`Surface`], there will be
    /// exactly one layer (potentially with multiple planes).
    ///
    /// If [`ExportSurfaceFlags::SEPARATE_LAYERS`] was used, each layer will contain a single plane,
    /// and multi-planar formats will have multiple layers.
    #[inline]
    pub fn layers(&self) -> &[PrimeLayer] {
        &self.layers[..self.num_layers as usize]
    }
}

/// Describes a DRM PRIME object, represented as a DMA-BUF file descriptor.
#[derive(Debug)]
#[repr(C)]
pub struct PrimeObject {
    fd: RawFd,
    size: u32,
    drm_format_modifier: u64,
}

impl PrimeObject {
    /// Returns the DMA-BUF file descriptor representing this object.
    #[inline]
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// Returns the size of this object in bytes.
    #[inline]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Returns the DRM format modifier of this object.
    ///
    /// The format modifier is an opaque 64-bit value. A list of them can be found in
    /// `drm_fourcc.h`.
    #[inline]
    pub fn drm_format_modifier(&self) -> u64 {
        self.drm_format_modifier
    }
}

/// Describes how a surface layer maps to a PRIME object.
#[repr(C)]
pub struct PrimeLayer {
    drm_format: PixelFormat,
    num_planes: u32,
    object_index: [u32; 4],
    offset: [u32; 4],
    pitch: [u32; 4],
}

impl fmt::Debug for PrimeLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct PlanesDebug<'a>(&'a PrimeLayer, u32);

        impl<'a> fmt::Debug for PlanesDebug<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut list = f.debug_list();
                for i in 0..self.1 {
                    list.entry(&self.0.plane(i));
                }
                list.finish()
            }
        }

        f.debug_struct("PrimeLayer")
            .field("drm_format", &self.drm_format)
            .field("planes", &PlanesDebug(self, self.num_planes))
            .finish()
    }
}

impl PrimeLayer {
    #[inline]
    pub fn drm_format(&self) -> PixelFormat {
        self.drm_format
    }

    #[inline]
    pub fn num_planes(&self) -> u32 {
        self.num_planes
    }

    pub fn plane(&self, index: u32) -> PrimePlane {
        assert!(index < self.num_planes && index < 4);
        PrimePlane {
            object_index: self.object_index[index as usize],
            offset: self.offset[index as usize],
            pitch: self.pitch[index as usize],
        }
    }

    pub fn planes(&self) -> impl Iterator<Item = PrimePlane> + '_ {
        (0..self.num_planes).map(|i| self.plane(i))
    }
}

/// Describes how a surface plane maps to a DRM PRIME object.
#[derive(Debug)]
pub struct PrimePlane {
    object_index: u32,
    offset: u32,
    pitch: u32,
}

impl PrimePlane {
    /// Returns the index of the [`PrimeObject`] in the [`PrimeSurfaceDescriptor`] that contains the
    /// data of this plane.
    #[inline]
    pub fn object_index(&self) -> u32 {
        self.object_index
    }

    /// Returns the byte offset of this plane in its associated [`PrimeObject`].
    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Returns the row pitch of this plane in bytes.
    #[inline]
    pub fn pitch(&self) -> u32 {
        self.pitch
    }
}

/// Linux-specific surface methods.
impl Surface {
    /// Exports a surface as a set of DRM PRIME objects.
    ///
    /// This should be called right after creating the [`Surface`], before any operations are
    /// performed on it.
    ///
    /// Uses [`SurfaceAttribMemoryType::DRM_PRIME_2`] internally, which must be supported by the
    /// driver in order for this method call to succeed.
    ///
    /// # Errors
    ///
    /// This may return an error of type `ERROR_UNSUPPORTED_MEMORY_TYPE` even though the PRIME
    /// memory type *is* normally supported, if it is called after the [`Surface`] has already been
    /// used by a VA-API operation. The caller should make sure to call this method right after
    /// creating the [`Surface`], before submitting any VA-API operation.
    pub fn export_prime(&mut self, flags: ExportSurfaceFlags) -> Result<PrimeSurfaceDescriptor> {
        unsafe {
            let mut descriptor: MaybeUninit<PrimeSurfaceDescriptor> = MaybeUninit::uninit();
            check(
                "vaExportSurfaceHandle",
                self.d.libva.vaExportSurfaceHandle(
                    self.d.raw,
                    self.id,
                    SurfaceAttribMemoryType::DRM_PRIME_2,
                    flags,
                    descriptor.as_mut_ptr().cast(),
                ),
            )?;
            Ok(descriptor.assume_init())
        }
    }

    /// Returns a pointer to the `wl_buffer` containing this [`Surface`]s pixel data.
    ///
    /// This function will only succeed if the [`Display`][crate::display::Display] this [`Surface`]
    /// was created from is using the Wayland backend. To check the VA-API backend type, use
    /// [`Display::display_api`][crate::display::Display::display_api].
    ///
    /// The returned pointer is valid while the [`Surface`] exists.
    ///
    /// [`Surface::sync`] should be called before using the `wl_buffer`, to ensure that all enqueued
    /// operations have finished.
    ///
    /// **Note**: The underlying function, `vaGetSurfaceBufferWl`, is not implemented on Mesa/AMD,
    /// so this will always return an error there.
    ///
    /// (also note that the `wl_buffer` type in the documentation is deliberately private; cast it
    /// to the desired type to use it)
    pub fn wayland_buffer(&self) -> Result<*mut wl_buffer> {
        unsafe {
            let mut wlbufferptr = MaybeUninit::uninit();
            check(
                "vaGetSurfaceBufferWl",
                libva_wayland::get()?.vaGetSurfaceBufferWl(
                    self.d.raw,
                    self.id,
                    0,
                    wlbufferptr.as_mut_ptr(),
                ),
            )?;
            Ok(wlbufferptr.assume_init())
        }
    }
}
