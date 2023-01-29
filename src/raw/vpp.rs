use std::ffi::c_uint;

use crate::{shared::vpp::*, Mirror, PixelFormat, Rotation};

use super::{
    VABufferID, VARectangle, VASurfaceID, VA_PADDING_HIGH, VA_PADDING_LARGE, VA_PADDING_LOW,
};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VAProcPipelineCaps {
    pub pipeline_flags: PipelineFlags,
    pub filter_flags: FilterFlags,
    pub num_forward_references: u32,
    pub num_backward_references: u32,
    pub input_color_standards: *const ColorStandardType,
    pub num_input_color_standards: u32,
    pub output_color_standards: *const ColorStandardType,
    pub num_output_color_standards: u32,
    pub rotation_flags: u32,
    pub blend_flags: BlendFlags,
    pub mirror_flags: Mirror,
    pub num_additional_outputs: u32,

    pub num_input_pixel_formats: u32,
    pub input_pixel_format: *const PixelFormat,
    pub num_output_pixel_formats: u32,
    pub output_pixel_format: *const PixelFormat,

    pub max_input_width: u32,
    pub max_input_height: u32,
    pub min_input_width: u32,
    pub min_input_height: u32,

    pub max_output_width: u32,
    pub max_output_height: u32,
    pub min_output_width: u32,
    pub min_output_height: u32,

    va_reserved: [u32; if cfg!(target_pointer_width = "64") {
        VA_PADDING_HIGH - 2
    } else {
        VA_PADDING_HIGH
    }],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FilterValueRange {
    pub min_value: f32,
    pub max_value: f32,
    pub default_value: f32,
    pub step: f32,
    va_reserve3d: [u32; VA_PADDING_LOW],
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VABlendState {
    pub flags: c_uint,
    pub global_alpha: f32,
    pub min_luma: f32,
    pub max_luma: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct VAProcPipelineParameterBuffer {
    pub surface: VASurfaceID,
    pub surface_region: *const VARectangle,
    pub surface_color_standard: ColorStandardType,
    pub output_region: *const VARectangle,
    pub output_background_color: u32,
    pub output_color_standard: ColorStandardType,
    pub pipeline_flags: PipelineFlags,
    pub filter_flags: FilterFlags,
    pub filters: *mut VABufferID,
    pub num_filters: u32,
    pub forward_references: *mut VASurfaceID,
    pub num_forward_references: u32,
    pub backward_references: *mut VASurfaceID,
    pub num_backward_references: u32,
    pub rotation_state: Rotation,
    pub blend_state: *const VABlendState, // may be NULL
    pub mirror_state: Mirror,
    pub additional_outputs: *mut VASurfaceID,
    pub num_additional_outputs: u32,
    pub input_surface_flag: u32,
    pub output_surface_flag: u32,
    pub input_color_properties: ColorProperties,
    pub output_color_properties: ColorProperties,
    pub processing_mode: ProcMode,
    pub output_hdr_metadata: *const u64, // TODO port struct

    va_reserved: [u32; if cfg!(target_pointer_width = "64") {
        VA_PADDING_LARGE - 16
    } else {
        VA_PADDING_LARGE - 13
    }],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FilterParameterBufferBase {
    pub type_: FilterType,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FilterParameterBuffer {
    pub type_: FilterType,
    pub value: f32,
    va_reserved: [u32; VA_PADDING_LOW],
}
