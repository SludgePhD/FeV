//! JPEG-related types and utilities.

mod parser;

#[cfg(test)]
mod tests;

use std::{cmp, mem};

use bytemuck::{AnyBitPattern, Pod, Zeroable};

use crate::{
    buffer::{Buffer, BufferType},
    config::Config,
    context::Context,
    display::Display,
    error::Error,
    raw::{Rectangle, VA_PADDING_LOW, VA_PADDING_MEDIUM},
    surface::{RTFormat, Surface},
    vpp::{ColorProperties, ColorStandardType, ProcPipelineParameterBuffer, SourceRange},
    Entrypoint, PixelFormat, Profile, Result, Rotation, SliceParameterBufferBase,
};

use self::parser::{JpegParser, SegmentKind, SofMarker};

ffi_enum! {
    pub enum ColorSpace: u8 {
        YUV = 0,
        RGB = 1,
        BGR = 2,
    }
}

/// Stores up to 4 quantizer tables and remembers which ones have been modified and need reloading.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct IQMatrixBuffer {
    load_quantiser_table: [u8; 4],
    /// 4 quantization tables, indexed by the `Tqi` field of the color component.
    quantiser_table: [[u8; 64]; 4],
    va_reserved: [u32; VA_PADDING_LOW],
}

impl IQMatrixBuffer {
    pub fn new() -> Self {
        unsafe { mem::zeroed() }
    }

    pub fn set_quantization_table(&mut self, index: u8, table_data: &[u8; 64]) {
        assert!(index <= 3, "index {index} out of bounds");
        let index = usize::from(index);
        self.load_quantiser_table[index] = 1;
        self.quantiser_table[index] = *table_data;
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct PictureParameterBuffer {
    picture_width: u16,
    picture_height: u16,
    components: [Component; 255],
    num_components: u8,
    color_space: ColorSpace,
    rotation: Rotation,
    crop_rectangle: Rectangle,
    va_reserved: [u32; VA_PADDING_MEDIUM - 3],
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Component {
    component_id: u8,
    h_sampling_factor: u8,
    v_sampling_factor: u8,
    quantiser_table_selector: u8,
}

impl PictureParameterBuffer {
    pub fn new(picture_width: u16, picture_height: u16, color_space: ColorSpace) -> Self {
        unsafe {
            let mut this: Self = mem::zeroed();
            this.picture_width = picture_width;
            this.picture_height = picture_height;
            this.color_space = color_space;
            this
        }
    }

    #[inline]
    pub fn picture_width(&self) -> u16 {
        self.picture_width
    }

    #[inline]
    pub fn picture_height(&self) -> u16 {
        self.picture_height
    }

    #[inline]
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
    }

    /// Adds a frame component.
    ///
    /// # Parameters
    ///
    /// - `Ci`: component identifier.
    /// - `Hi`: horizontal sampling factor.
    /// - `Vi`: vertical sampling factor.
    /// - `Tqi`: quantization table destination selector.
    #[allow(non_snake_case)]
    pub fn push_component(&mut self, Ci: u8, Hi: u8, Vi: u8, Tqi: u8) {
        let index = usize::from(self.num_components);
        self.num_components = self
            .num_components
            .checked_add(1)
            .expect("maximum number of frame components reached");

        self.components[index].component_id = Ci;
        self.components[index].h_sampling_factor = Hi;
        self.components[index].v_sampling_factor = Vi;
        self.components[index].quantiser_table_selector = Tqi;
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SliceParameterBuffer {
    base: SliceParameterBufferBase,

    slice_horizontal_position: u32,
    slice_vertical_position: u32,

    components: [ScanComponent; 4],
    num_components: u8,

    restart_interval: u16,
    num_mcus: u32,

    va_reserved: [u32; VA_PADDING_LOW],
}

#[derive(Clone, Copy)]
#[repr(C)]
struct ScanComponent {
    component_selector: u8,
    dc_table_selector: u8,
    ac_table_selector: u8,
}

impl SliceParameterBuffer {
    /// Creates a new JPEG slice parameter structure.
    ///
    /// # Parameters
    ///
    /// - `base`: codec-independent slice parameters
    /// - `Ri`: number of MCUs per restart interval
    /// - `num_mcus`: total number of MCUs in this scan
    #[allow(non_snake_case)]
    pub fn new(base: SliceParameterBufferBase, Ri: u16, num_mcus: u32) -> Self {
        unsafe {
            let mut this: Self = mem::zeroed();
            this.base = base;
            this.restart_interval = Ri;
            this.num_mcus = num_mcus;
            this
        }
    }

    #[allow(non_snake_case)]
    pub fn push_component(&mut self, Csj: u8, Tdj: u8, Taj: u8) {
        let index = usize::from(self.num_components);
        self.num_components = self
            .num_components
            .checked_add(1)
            .expect("maximum number of scan components reached");

        self.components[index].component_selector = Csj;
        self.components[index].dc_table_selector = Tdj;
        self.components[index].ac_table_selector = Taj;
    }
}

/// Stores up to 2 [`HuffmanTable`]s and remembers which have been modified and need reloading.
#[derive(Clone, Copy, AnyBitPattern)]
#[repr(C)]
pub struct HuffmanTableBuffer {
    load_huffman_table: [u8; 2],
    huffman_table: [HuffmanTable; 2],
    va_reserved: [u32; VA_PADDING_LOW],
}

impl HuffmanTableBuffer {
    pub fn default_tables() -> Self {
        let mut this = Self::zeroed();
        this.set_huffman_table(0, &HuffmanTable::default_luminance());
        this.set_huffman_table(1, &HuffmanTable::default_chrominance());
        this
    }

    pub fn zeroed() -> Self {
        unsafe { mem::zeroed() }
    }

    pub fn set_huffman_table(&mut self, index: u8, tbl: &HuffmanTable) {
        assert!(index <= 1, "huffman table index {index} out of bounds");
        let index = usize::from(index);
        self.huffman_table[index] = *tbl;
        self.load_huffman_table[index] = 1; // mark as modified
    }

    pub fn huffman_table_mut(&mut self, index: u8) -> &mut HuffmanTable {
        assert!(index <= 1, "huffman table index {index} out of bounds");
        let index = usize::from(index);
        self.load_huffman_table[index] = 1; // mark as modified
        &mut self.huffman_table[index]
    }

    pub fn clear_modified(&mut self) {
        self.load_huffman_table = [0; 2];
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct HuffmanTable {
    num_dc_codes: [u8; 16],
    dc_values: [u8; 12],
    num_ac_codes: [u8; 16],
    ac_values: [u8; 162],
    pad: [u8; 2],
}

impl HuffmanTable {
    /// Returns the default [`HuffmanTable`] to use for luminance data.
    #[rustfmt::skip]
    pub fn default_luminance() -> Self {
        let mut this = Self::zeroed();
        this.set_dc_table(
            &[0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0],
            &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b],
        );
        this.set_ac_table(
            &[0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125],
            &[
                0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12,
                0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
                0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
                0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
                0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16,
                0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
                0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
                0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
                0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
                0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
                0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
                0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
                0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
                0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
                0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
                0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
                0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4,
                0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
                0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
                0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
                0xf9, 0xfa,
            ]
        );
        this
    }

    /// Returns the default [`HuffmanTable`] to use for chrominance data.
    #[rustfmt::skip]
    pub fn default_chrominance() -> Self {
        let mut this = Self::zeroed();
        this.set_dc_table(
            &[0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0],
            &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b]
        );
        this.set_ac_table(
            &[0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119],
            &[
                0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21,
                0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
                0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91,
                0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
                0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34,
                0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
                0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38,
                0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
                0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
                0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
                0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
                0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
                0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96,
                0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
                0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
                0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
                0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2,
                0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
                0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9,
                0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
                0xf9, 0xfa,
            ]
        );
        this
    }

    pub fn zeroed() -> Self {
        unsafe { mem::zeroed() }
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

        self.num_dc_codes[..Li.len()].copy_from_slice(Li);
        self.dc_values[..Vij.len()].copy_from_slice(Vij);
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

        self.num_ac_codes[..Li.len()].copy_from_slice(Li);
        self.ac_values[..Vij.len()].copy_from_slice(Vij);
    }
}

/// JPEG metadata required to create a VA-API JPEG decoding session.
#[derive(Debug, Clone, Copy)]
pub struct JpegInfo {
    width: u16,
    height: u16,
}

impl JpegInfo {
    /// Parses the given JPEG image.
    ///
    /// # Errors
    ///
    /// If this returns an error, the JPEG image is either malformed, or of an incompatible format
    /// that is not supported by VA-API. In that case, the caller should fall back to software
    /// decoding.
    pub fn new(jpeg: &[u8]) -> Result<Self> {
        let mut parser = JpegParser::new(&jpeg);
        let segment = parser
            .next_segment()?
            .ok_or_else(|| Error::from("missing SOI segment"))?;
        if !matches!(segment.kind, parser::SegmentKind::Soi) {
            return Err(Error::from("missing SOI segment"));
        }

        let sof = loop {
            let segment = parser
                .next_segment()?
                .ok_or_else(|| Error::from("missing SOF segment"))?;
            match segment.kind {
                SegmentKind::Sof(sof) => break sof,
                _ => {}
            }
        };

        if sof.sof() != SofMarker::SOF0 {
            return Err(Error::from(format!(
                "not a baseline JPEG ({:?})",
                sof.sof()
            )));
        }
        if sof.P() != 8 {
            return Err(Error::from(format!(
                "unsupported sample precision of {} bits (only 8-bit samples are supported)",
                sof.P()
            )));
        }

        Ok(Self {
            width: sof.X(),
            height: sof.Y(),
        })
    }

    #[inline]
    pub fn width(&self) -> u16 {
        self.width
    }

    #[inline]
    pub fn height(&self) -> u16 {
        self.height
    }
}

/// A VA-API JPEG decoding session.
///
/// This type encapsulates [`Surface`]s and [`Context`]s for decoding baseline JPEG files of a
/// particular size. It will also convert the JPEG to standard sRGB color space.
///
/// [`Surface`]: crate::surface::Surface
pub struct JpegDecodeSession {
    width: u32,
    height: u32,

    jpeg_surface: Surface,
    vpp_surface: Surface,

    jpeg_context: Context,
    vpp_context: Context,
}

impl JpegDecodeSession {
    /// Creates [`Surface`]s and [`Context`]s to decode JPEG images of the given size.
    ///
    /// # Errors
    ///
    /// This function will return an error if VA-API object creation fails. This typically means
    /// that the implementation does not support JPEG decoding, but it can also indicate that the
    /// JPEG is simply too large and smaller ones would work.
    ///
    /// [`Surface`]: crate::surface::Surface
    pub fn new(display: &Display, width: u16, height: u16) -> Result<Self> {
        let width = u32::from(width);
        let height = u32::from(height);

        let config = Config::new(&display, Profile::JPEGBaseline, Entrypoint::VLD)?;
        let jpeg_context = Context::new(&config, width, height)?;
        let config = Config::new(&display, Profile::None, Entrypoint::VideoProc)?;
        let vpp_context = Context::new(&config, width, height)?;

        let jpeg_surface = Surface::new(&display, width, height, RTFormat::YUV420)?;
        let vpp_surface = Surface::with_pixel_format(&display, width, height, PixelFormat::RGBA)?;

        Ok(Self {
            width,
            height,
            jpeg_surface,
            vpp_surface,
            jpeg_context,
            vpp_context,
        })
    }

    #[inline]
    pub fn surface(&mut self) -> &mut Surface {
        &mut self.jpeg_surface
    }

    /// Decodes a baseline JPEG, returning a [`Surface`] containing the decoded image.
    ///
    /// The decoded image is in the JPEG's native color space and uses an unspecified pixel format.
    ///
    /// # Errors
    ///
    /// This method returns an error when the JPEG is malformed or VA-API returns an error during
    /// decoding.
    pub fn decode(&mut self, jpeg: &[u8]) -> Result<&mut Surface> {
        // TODO make this more flexible and move to `error` module
        macro_rules! bail {
            ($($args:tt)*) => {
                return Err(Error::from(format!(
                    $($args)*
                )))
            };
        }

        let mut dhtbuf = HuffmanTableBuffer::zeroed();
        let mut max_h_factor = 0;
        let mut max_v_factor = 0;
        let mut restart_interval = 0;
        let mut ppbuf = None;
        let mut slice = None;
        let mut iqbuf = IQMatrixBuffer::new();

        let mut parser = JpegParser::new(&jpeg);
        while let Some(segment) = parser.next_segment()? {
            match segment.kind {
                SegmentKind::Dqt(dqt) => {
                    for dqt in dqt.tables() {
                        if dqt.Pq() != 0 {
                            bail!("unexpected value `{}` for DQT Pq", dqt.Pq());
                        }
                        iqbuf.set_quantization_table(dqt.Tq(), &dqt.Qk());
                    }
                }
                SegmentKind::Dht(dht) => {
                    for table in dht.tables() {
                        if table.Th() > 1 {
                            bail!(
                                "invalid DHT destination slot {} (expected 0 or 1)",
                                table.Th()
                            );
                        }
                        let tbl = dhtbuf.huffman_table_mut(table.Th());
                        match table.Tc() {
                            0 => tbl.set_dc_table(table.Li(), table.Vij()),
                            1 => tbl.set_ac_table(table.Li(), table.Vij()),
                            _ => bail!("invalid DHT class {}", table.Tc()),
                        }
                    }
                }
                SegmentKind::Dri(dri) => restart_interval = dri.Ri(),
                SegmentKind::Sof(sof) => {
                    if sof.sof() != SofMarker::SOF0 {
                        bail!("not a baseline JPEG (SOF={:?})", sof.sof());
                    }

                    if sof.P() != 8 {
                        bail!("sample precision of {} bits is not supported", sof.P());
                    }

                    if u32::from(sof.Y()) != self.height || u32::from(sof.X()) != self.width {
                        bail!(
                            "image dimension {}x{} does not match context dimention {}x{}",
                            sof.X(),
                            sof.Y(),
                            self.width,
                            self.height
                        );
                    }

                    let mut buf = PictureParameterBuffer::new(sof.X(), sof.Y(), ColorSpace::YUV);
                    for component in sof.components() {
                        buf.push_component(
                            component.Ci(),
                            component.Hi(),
                            component.Vi(),
                            component.Tqi(),
                        );
                        max_h_factor = cmp::max(u32::from(component.Hi()), max_h_factor);
                        max_v_factor = cmp::max(u32::from(component.Vi()), max_v_factor);
                    }
                    ppbuf = Some(buf);
                }
                SegmentKind::Sos(sos) => {
                    if sos.Ss() != 0 || sos.Se() != 63 {
                        // Baseline JPEGs always use 0...63
                        bail!(
                            "invalid SOS header: Ss={}, Se={} (expected 0...63)",
                            sos.Ss(),
                            sos.Se(),
                        );
                    }

                    if sos.Ah() != 0 || sos.Al() != 0 {
                        // Baseline JPEGs always use 0...0
                        bail!("invalid SOS header: Ah={}, Al={}", sos.Ah(), sos.Al());
                    }

                    let slice_data = sos.data();
                    let num_mcus = ((self.width + max_h_factor * 8 - 1) / (max_h_factor * 8))
                        * ((self.height + max_v_factor * 8 - 1) / (max_v_factor * 8));
                    let mut slice_params = SliceParameterBuffer::new(
                        SliceParameterBufferBase::new(slice_data.len().try_into().unwrap()),
                        restart_interval,
                        num_mcus,
                    );
                    for component in sos.components() {
                        slice_params.push_component(
                            component.Csj(),
                            component.Tdj(),
                            component.Taj(),
                        );
                    }
                    slice = Some((slice_params, slice_data));
                }
                SegmentKind::Eoi => break,
                _ => {}
            }
        }

        let Some(ppbuf) = ppbuf else {
            bail!("file is missing SOI segment")
        };
        let Some((slice_params, slice_data)) = slice else {
            bail!("file is missing SOS header")
        };

        let mut buf_dht = Buffer::new_param(&self.jpeg_context, BufferType::HuffmanTable, dhtbuf)?;
        let mut buf_iq = Buffer::new_param(&self.jpeg_context, BufferType::IQMatrix, iqbuf)?;
        let mut buf_pp =
            Buffer::new_param(&self.jpeg_context, BufferType::PictureParameter, ppbuf)?;
        let mut buf_slice_param =
            Buffer::new_param(&self.jpeg_context, BufferType::SliceParameter, slice_params)?;
        let mut buf_slice_data =
            Buffer::new_data(&self.jpeg_context, BufferType::SliceData, &slice_data)?;

        let mut picture = self.jpeg_context.begin_picture(&mut self.jpeg_surface)?;
        picture.render_picture(&mut buf_dht)?;
        picture.render_picture(&mut buf_iq)?;
        picture.render_picture(&mut buf_pp)?;
        picture.render_picture(&mut buf_slice_param)?;
        picture.render_picture(&mut buf_slice_data)?;
        unsafe { picture.end_picture()? }

        Ok(&mut self.jpeg_surface)
    }

    pub fn decode_and_convert(&mut self, jpeg: &[u8]) -> Result<&mut Surface> {
        self.decode(jpeg)?;

        let mut pppbuf = ProcPipelineParameterBuffer::new(&self.jpeg_surface);
        // The input color space is the JPEG color space
        let input_props = ColorProperties::new().with_color_range(SourceRange::FULL);
        pppbuf.set_input_color_properties(input_props);
        pppbuf.set_input_color_standard(ColorStandardType::BT601);
        // The output color space is 8-bit non-linear sRGB
        let output_props = ColorProperties::new().with_color_range(SourceRange::FULL);
        pppbuf.set_output_color_properties(output_props);
        pppbuf.set_output_color_standard(ColorStandardType::SRGB);

        let mut pppbuf =
            Buffer::new_param(&self.vpp_context, BufferType::ProcPipelineParameter, pppbuf)?;

        let mut picture = self.vpp_context.begin_picture(&mut self.vpp_surface)?;
        picture.render_picture(&mut pppbuf)?;
        unsafe { picture.end_picture()? }

        drop(pppbuf);

        Ok(&mut self.vpp_surface)
    }
}
