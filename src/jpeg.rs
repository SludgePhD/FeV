//! Buffer types specific to JPEG decoding and encoding.

use std::mem;

pub use crate::shared::jpeg::*;

use crate::{raw::jpeg, Mapping, Rotation, SliceParameterBufferBase};

/// Stores up to 4 quantizer tables and remembers which ones have been modified and need reloading.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct IQMatrixBuffer {
    raw: jpeg::IQMatrixBuffer,
}

impl IQMatrixBuffer {
    pub fn new() -> Self {
        Self {
            raw: unsafe { std::mem::zeroed() },
        }
    }

    pub fn set_quantization_table(&mut self, index: u8, table_data: &[u8; 64]) {
        assert!(index <= 3, "index {index} out of bounds");
        let index = usize::from(index);
        self.raw.load_quantiser_table[index] = 1;
        self.raw.quantiser_table[index] = *table_data;
    }

    pub fn submit(&mut self, dest: &mut Mapping<'_, Self>) {
        dest.write(0, *self);
        self.raw.load_quantiser_table = [0; 4];
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PictureParameterBuffer {
    raw: jpeg::PictureParameterBuffer,
}

impl PictureParameterBuffer {
    pub fn new(picture_width: u16, picture_height: u16, color_space: ColorSpace) -> Self {
        unsafe {
            let mut raw: jpeg::PictureParameterBuffer = mem::zeroed();
            raw.picture_width = picture_width;
            raw.picture_height = picture_height;
            raw.color_space = color_space;
            Self { raw }
        }
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.raw.rotation = rotation;
    }

    #[allow(non_snake_case)]
    pub fn push_component(&mut self, Ci: u8, Hi: u8, Vi: u8, Tqi: u8) {
        let index = usize::from(self.raw.num_components);
        self.raw.num_components = self
            .raw
            .num_components
            .checked_add(1)
            .expect("maximum number of frame components reached");

        self.raw.components[index].component_id = Ci;
        self.raw.components[index].h_sampling_factor = Hi;
        self.raw.components[index].v_sampling_factor = Vi;
        self.raw.components[index].quantiser_table_selector = Tqi;
    }

    pub fn submit(&mut self, dest: &mut Mapping<'_, Self>) {
        dest.write(0, *self);
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct SliceParameterBuffer {
    raw: jpeg::SliceParameterBuffer,
}

impl SliceParameterBuffer {
    /// Creates a new JPEG slice parameter structure.
    ///
    /// # Parameters
    ///
    /// - `base`: codec-independent slice parameters
    /// - `Ri`: number of MCUs per restart interval
    /// - `num_mcus`: number of MCUs per scan - this is `Ri` times the number of restart intervals
    ///   per scan, presumably
    #[allow(non_snake_case)]
    pub fn new(base: SliceParameterBufferBase, Ri: u16, num_mcus: u32) -> Self {
        unsafe {
            let mut raw: jpeg::SliceParameterBuffer = mem::zeroed();
            raw.slice_data_size = base.slice_data_size();
            raw.slice_data_offset = base.slice_data_offset();
            raw.slice_data_flag = base.slice_data_flags();
            raw.restart_interval = Ri;
            raw.num_mcus = num_mcus;
            Self { raw }
        }
    }

    #[allow(non_snake_case)]
    pub fn push_component(&mut self, Csj: u8, Tdj: u8, Taj: u8) {
        let index = usize::from(self.raw.num_components);
        self.raw.num_components = self
            .raw
            .num_components
            .checked_add(1)
            .expect("maximum number of scan components reached");

        self.raw.components[index].component_selector = Csj;
        self.raw.components[index].dc_table_selector = Tdj;
        self.raw.components[index].ac_table_selector = Taj;
    }

    pub fn submit(&mut self, dest: &mut Mapping<'_, Self>) {
        dest.write(0, *self);
    }
}

/// Stores up to 2 [`HuffmanTable`]s and remembers which have been modified and need reloading.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct HuffmanTableBuffer {
    raw: jpeg::HuffmanTableBuffer,
}

impl HuffmanTableBuffer {
    pub fn new() -> Self {
        unsafe { Self { raw: mem::zeroed() } }
    }

    pub fn set_huffman_table(&mut self, index: u8, tbl: &HuffmanTable) {
        assert!(index <= 1, "huffman table index {index} out of bounds");
        let index = usize::from(index);
        self.raw.huffman_table[index] = tbl.raw;
        self.raw.load_huffman_table[index] = 1; // mark as modified
    }

    pub fn submit(&mut self, dest: &mut Mapping<'_, Self>) {
        dest.write(0, *self);
        self.raw.load_huffman_table = [0; 2];
    }
}

#[derive(Clone, Copy)]
pub struct HuffmanTable {
    raw: jpeg::HuffmanTable,
}

impl HuffmanTable {
    pub fn new() -> Self {
        unsafe { Self { raw: mem::zeroed() } }
    }

    #[allow(non_snake_case)]
    pub fn set_dc_table(&mut self, Li: &[u8], Vij: &[u8]) {
        assert!(
            Li.len() <= 16,
            "DC huffman table code count {} exceeds maximum",
            Li.len(),
        );
        assert!(
            Vij.len() <= 12,
            "DC huffman table value count {} exceeds maximum",
            Vij.len(),
        );

        self.raw.num_dc_codes[..Li.len()].copy_from_slice(Li);
        self.raw.dc_values[..Vij.len()].copy_from_slice(Vij);
    }

    #[allow(non_snake_case)]
    pub fn set_ac_table(&mut self, Li: &[u8], Vij: &[u8]) {
        assert!(
            Li.len() <= 16,
            "AC huffman table code count {} exceeds maximum",
            Li.len(),
        );
        assert!(
            Vij.len() <= 162,
            "AC huffman table value count {} exceeds maximum",
            Vij.len(),
        );

        self.raw.num_ac_codes[..Li.len()].copy_from_slice(Li);
        self.raw.ac_values[..Vij.len()].copy_from_slice(Vij);
    }
}
