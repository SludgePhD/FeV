//! Buffer creation and mapping.

use std::{
    ffi::{c_int, c_uint, c_void},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr,
    sync::Arc,
};

use bytemuck::{AnyBitPattern, NoUninit, Pod};

use crate::{
    check, check_log,
    display::DisplayOwner,
    raw::{VABufferID, VA_TIMEOUT_INFINITE},
    Context, Result,
};

ffi_enum! {
    /// Enumeration of all the buffer types VA-API understands.
    pub enum BufferType: c_int {
        PictureParameter    = 0,
        IQMatrix            = 1,
        BitPlane            = 2,
        SliceGroupMap       = 3,
        SliceParameter      = 4,
        SliceData           = 5,
        MacroblockParameter = 6,
        ResidualData        = 7,
        DeblockingParameter = 8,
        Image               = 9,
        ProtectedSliceData  = 10,
        QMatrix             = 11,
        HuffmanTable        = 12,
        Probability         = 13,

        /* Following are encode buffer types */
        EncCoded             = 21,
        EncSequenceParameter = 22,
        EncPictureParameter  = 23,
        EncSliceParameter    = 24,
        EncPackedHeaderParameter = 25,
        EncPackedHeaderData     = 26,
        EncMiscParameter        = 27,
        EncMacroblockParameter  = 28,
        EncMacroblockMap        = 29,
        EncQP                   = 30,

        /* Following are video processing buffer types */
        ProcPipelineParameter   = 41,
        ProcFilterParameter     = 42,
        EncFEIMV                = 43,
        EncFEIMBCode            = 44,
        EncFEIDistortion        = 45,
        EncFEIMBControl         = 46,
        EncFEIMVPredictor       = 47,
        StatsStatisticsParameter = 48,
        StatsStatistics         = 49,
        StatsStatisticsBottomField = 50,
        StatsMV                 = 51,
        StatsMVPredictor        = 52,
        EncMacroblockDisableSkipMap = 53,
        EncFEICTBCmd            = 54,
        EncFEICURecord          = 55,
        DecodeStreamout         = 56,
        SubsetsParameter        = 57,
        ContextParameterUpdate  = 58,
        ProtectedSessionExecute = 59,
        EncryptionParameter  = 60,
        EncDeltaQpPerBlock   = 61,
    }
}

/// A buffer that holds arbitrary data.
pub struct RawBuffer {
    d: Arc<DisplayOwner>,
    id: VABufferID,
    #[allow(dead_code)]
    elem_size: usize,
    capacity: usize,
}

impl Drop for RawBuffer {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyBuffer(self.d.raw, self.id),
                "vaDestroyBuffer call in drop",
            );
        }
    }
}

impl<T> From<Buffer<T>> for RawBuffer {
    fn from(buf: Buffer<T>) -> Self {
        buf.raw
    }
}

/// A buffer that holds elements of type `T`.
pub struct Buffer<T> {
    raw: RawBuffer,
    _p: PhantomData<T>,
}

impl Buffer<u8> {
    /// Creates a [`Buffer`] of the specified [`BufferType`], containing raw data bytes.
    pub fn new_data(cx: &Context, buf_ty: BufferType, data: &[u8]) -> Result<Buffer<u8>> {
        let mut buf_id = 0;
        unsafe {
            check(cx.d.libva.vaCreateBuffer(
                cx.d.raw,
                cx.id,
                buf_ty,
                c_uint::try_from(data.len()).unwrap(),
                1,
                data.as_ptr() as *mut _,
                &mut buf_id,
            ))?;
        }
        Ok(Buffer {
            raw: RawBuffer {
                d: cx.d.clone(),
                id: buf_id,
                elem_size: 1,
                capacity: data.len(),
            },
            _p: PhantomData,
        })
    }
}

impl<T> Buffer<T> {
    pub fn new_empty(cx: &Context, buf_ty: BufferType, num_elements: usize) -> Result<Buffer<T>>
    where
        T: NoUninit,
    {
        let mut buf_id = 0;
        unsafe {
            check(cx.d.libva.vaCreateBuffer(
                cx.d.raw,
                cx.id,
                buf_ty,
                mem::size_of::<T>() as c_uint,
                c_uint::try_from(num_elements).unwrap(),
                ptr::null_mut(),
                &mut buf_id,
            ))?;
        }
        Ok(Buffer {
            raw: RawBuffer {
                d: cx.d.clone(),
                id: buf_id,
                elem_size: mem::size_of::<T>(),
                capacity: num_elements,
            },
            _p: PhantomData,
        })
    }

    /// Creates a parameter [`Buffer`] of the specified [`BufferType`], containing an instance of `T`.
    ///
    /// This is primarily used to pass individual parameter structures to libva.
    pub fn new_param(cx: &Context, buf_ty: BufferType, mut content: T) -> Result<Buffer<T>>
    where
        T: Copy,
    {
        let mut buf_id = 0;
        unsafe {
            check(cx.d.libva.vaCreateBuffer(
                cx.d.raw,
                cx.id,
                buf_ty,
                mem::size_of::<T>() as c_uint,
                1,
                &mut content as *mut _ as *mut c_void,
                &mut buf_id,
            ))?;
        }
        Ok(Buffer {
            raw: RawBuffer {
                d: cx.d.clone(),
                id: buf_id,
                elem_size: mem::size_of::<T>(),
                capacity: 1,
            },
            _p: PhantomData,
        })
    }

    #[inline]
    pub(crate) fn id(&self) -> VABufferID {
        self.raw.id
    }

    pub fn map(&mut self) -> Result<Mapping<'_, T>> {
        let mut ptr = ptr::null_mut();
        unsafe {
            check(
                self.raw
                    .d
                    .libva
                    .vaMapBuffer(self.raw.d.raw, self.raw.id, &mut ptr),
            )?;
        }
        Ok(Mapping {
            d: &self.raw.d,
            id: self.raw.id,
            ptr: ptr.cast(),
            capacity: self.raw.capacity,
        })
    }

    pub fn sync(&mut self) -> Result<()> {
        unsafe {
            check(
                self.raw
                    .d
                    .libva
                    .vaSyncBuffer(self.raw.d.raw, self.raw.id, VA_TIMEOUT_INFINITE),
            )
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
    pub(crate) d: &'a DisplayOwner,
    pub(crate) id: VABufferID,
    pub(crate) ptr: *mut T,
    pub(crate) capacity: usize,
}

impl<'a, T: Copy> Mapping<'a, T> {
    /// Casts this [`Mapping`] to one with a different element type (and possibly length).
    pub fn cast<U>(self) -> Mapping<'a, U>
    where
        T: AnyBitPattern + NoUninit,
        U: AnyBitPattern,
    {
        let new_len = bytemuck::cast_slice::<T, U>(&self).len();

        Mapping {
            d: self.d,
            id: self.id,
            ptr: self.ptr.cast(),
            capacity: new_len,
        }
    }

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
