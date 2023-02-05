use std::{
    ffi::c_void,
    os::raw::{c_char, c_int, c_uint},
};

use crate::buffer::BufferType;

pub const VA_PADDING_LOW: usize = 4;
pub const VA_PADDING_MEDIUM: usize = 8;
pub const VA_PADDING_HIGH: usize = 16;
pub const VA_PADDING_LARGE: usize = 32;
pub const VA_TIMEOUT_INFINITE: u64 = 0xFFFFFFFFFFFFFFFF;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Rectangle {
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VAProcessingRateParameterEnc {
    pub level_idc: u8,
    reserved0: [u8; 3],
    pub quality_level: u32,
    pub intra_period: u32,
    pub ip_period: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VAProcessingRateParameterDec {
    pub level_idc: u8,
    reserved0: [u8; 3],
    reserved: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union VAProcessingRateParameter {
    proc_buf_enc: VAProcessingRateParameterEnc,
    proc_buf_dec: VAProcessingRateParameterDec,
}

#[repr(C)]
pub struct VABufferInfo {
    handle: usize, // uintptr_t
    type_: BufferType,
    mem_type: u32,
    mem_size: usize, // size_t
    va_reserved: [u32; VA_PADDING_LOW],
}

pub type VADisplay = *mut c_void;

pub type VAMessageCallback =
    unsafe extern "C" fn(user_context: *mut c_void, message: *const c_char);
pub type VAPrivFunc = unsafe extern "C" fn() -> c_int;
pub type VAGenericFunc = unsafe extern "C" fn();

pub type VAGenericID = c_uint;
pub type VAConfigID = VAGenericID;
pub type VAContextID = VAGenericID;
pub type VASurfaceID = VAGenericID;
pub type VAMFContextID = VAGenericID;
pub type VABufferID = VAGenericID;
pub type VAImageID = VAGenericID;
pub type VASubpictureID = VAGenericID;
