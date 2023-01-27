use bytemuck::{AnyBitPattern, Pod, Zeroable};

use crate::{jpeg, Rotation, SliceDataFlags};

use super::{VA_PADDING_LOW, VA_PADDING_MEDIUM};

#[derive(Clone, Copy, AnyBitPattern)]
#[repr(C)]
pub struct HuffmanTableBuffer {
    pub load_huffman_table: [u8; 2],
    pub huffman_table: [HuffmanTable; 2],
    va_reserved: [u32; VA_PADDING_LOW],
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct HuffmanTable {
    pub num_dc_codes: [u8; 16],
    pub dc_values: [u8; 12],
    pub num_ac_codes: [u8; 16],
    pub ac_values: [u8; 162],
    pad: [u8; 2],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct PictureParameterBuffer {
    pub picture_width: u16,
    pub picture_height: u16,
    pub components: [Component; 255],
    pub num_components: u8,
    pub color_space: jpeg::ColorSpace,
    pub rotation: Rotation,
    va_reserved: [u32; VA_PADDING_MEDIUM - 1],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Component {
    pub component_id: u8,
    pub h_sampling_factor: u8,
    pub v_sampling_factor: u8,
    pub quantiser_table_selector: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct IQMatrixBuffer {
    pub load_quantiser_table: [u8; 4],
    /// 4 quantization tables, indexed by the `Tqi` field of the color component.
    pub quantiser_table: [[u8; 64]; 4],
    va_reserved: [u32; VA_PADDING_LOW],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SliceParameterBuffer {
    pub slice_data_size: u32,
    pub slice_data_offset: u32,
    pub slice_data_flag: SliceDataFlags,

    pub slice_horizontal_position: u32,
    pub slice_vertical_position: u32,

    pub components: [ScanComponent; 4],
    pub num_components: u8,

    pub restart_interval: u16,
    pub num_mcus: u32,

    va_reserved: [u32; VA_PADDING_LOW],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ScanComponent {
    pub component_selector: u8,
    pub dc_table_selector: u8,
    pub ac_table_selector: u8,
}
