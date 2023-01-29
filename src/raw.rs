#![allow(non_upper_case_globals)]
#![allow(dead_code)]

pub mod jpeg;
pub mod vpp;

use std::{
    ffi::c_void,
    os::raw::{c_char, c_int, c_uint},
};

use crate::shared::*;

pub const VA_PADDING_LOW: usize = 4;
pub const VA_PADDING_MEDIUM: usize = 8;
pub const VA_PADDING_HIGH: usize = 16;
pub const VA_PADDING_LARGE: usize = 32;
pub const VA_TIMEOUT_INFINITE: u64 = 0xFFFFFFFFFFFFFFFF;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VARectangle {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VAConfigAttrib {
    type_: VAConfigAttribType,
    value: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VASurfaceAttrib {
    pub(crate) type_: VASurfaceAttribType,
    pub(crate) flags: VASurfaceAttribFlags,
    pub(crate) value: VAGenericValue,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VAGenericValue {
    pub type_: VAGenericValueType,
    pub value: VAGenericValueUnion,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union VAGenericValueUnion {
    pub i: i32,
    pub f: f32,
    pub p: *mut c_void,
    pub func: VAGenericFunc,
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

#[repr(C)]
pub struct VASurfaceDecodeMBErrors {
    pub status: i32,
    pub start_mb: u32,
    pub end_mb: u32,
    pub decode_error_type: VADecodeErrorType,
    pub num_mb: u32,
    va_reserved: [u32; VA_PADDING_LOW - 1],
}

#[derive(Debug)]
#[repr(C)]
pub struct VAImage {
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

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VADisplayAttribute {
    pub(crate) type_: VADisplayAttribType,
    pub(crate) min_value: i32,
    pub(crate) max_value: i32,
    pub(crate) value: i32,
    pub(crate) flags: VADisplayAttribFlags,
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
