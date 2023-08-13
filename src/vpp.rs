//! Wraps the Video Processing API (`va_vpp.h`).
//!
//! To perform video processing, create a [`Context`] with [`Profile::None`][crate::Profile::None]
//! and [`Entrypoint::VideoProc`][crate::Entrypoint::VideoProc], and submit a
//! [`ProcPipelineParameterBuffer`] as a
//! [`BufferType::ProcPipelineParameter`][crate::buffer::BufferType::ProcPipelineParameter].

use std::{ffi::c_uint, marker::PhantomData, mem, slice, vec};

use crate::{
    buffer::{Buffer, RawBuffer},
    check,
    context::Context,
    pixelformat::PixelFormat,
    raw::{Rectangle, VABufferID, VASurfaceID, VA_PADDING_HIGH, VA_PADDING_LARGE, VA_PADDING_LOW},
    surface::Surface,
    Mirror, Result, Rotation,
};

impl Context {
    /// Fetches the list of supported video processing filter types.
    pub fn query_video_processing_filters(&self) -> Result<FilterTypes> {
        // The docs of `vaQueryVideoProcFilters` clearly state that the number of filters will be
        // returned in `num_filters`, if it is higher than what we pass to it (and the function will
        // return a `MAX_NUM_EXCEEDED` error).
        // This, however, is a lie. The function does no such thing (it succeeds and returns a
        // truncated list, at least on Intel's impl), so we just preallocate a "large" array and
        // shrink it later.

        const PREALLOC: usize = 512;

        let mut num_filters = PREALLOC as c_uint;
        let mut filters = vec![FilterType::None; PREALLOC];
        unsafe {
            check(
                "vaQueryVideoProcFilters",
                self.d.libva.vaQueryVideoProcFilters(
                    self.d.raw,
                    self.id,
                    filters.as_mut_ptr(),
                    &mut num_filters,
                ),
            )?;
        }

        assert_ne!(
            num_filters as usize, PREALLOC,
            "nothing should support this many filters"
        );

        filters.truncate(num_filters as usize);
        filters.shrink_to_fit();

        Ok(FilterTypes { filters })
    }

    pub fn query_video_processing_pipeline_caps(
        &self,
        filters: &mut Filters,
    ) -> Result<ProcPipelineCaps> {
        let mut input_color_standards = vec![ColorStandardType(0); 32];
        let mut output_color_standards = vec![ColorStandardType(0); 32];
        let mut input_pixel_formats = vec![PixelFormat::from_u32_le(0); 32];
        let mut output_pixel_formats = vec![PixelFormat::from_u32_le(0); 32];

        unsafe {
            let mut caps: RawProcPipelineCaps = mem::zeroed();
            caps.input_color_standards = input_color_standards.as_mut_ptr();
            caps.num_input_color_standards = input_color_standards.len() as _;
            caps.output_color_standards = output_color_standards.as_mut_ptr();
            caps.num_output_color_standards = output_color_standards.len() as _;
            caps.input_pixel_format = input_pixel_formats.as_mut_ptr();
            caps.num_input_pixel_formats = input_pixel_formats.len() as _;
            caps.output_pixel_format = output_pixel_formats.as_mut_ptr();
            caps.num_output_pixel_formats = output_pixel_formats.len() as _;
            check(
                "vaQueryVideoProcPipelineCaps",
                self.d.libva.vaQueryVideoProcPipelineCaps(
                    self.d.raw,
                    self.id,
                    filters.as_mut_ptr(),
                    filters.len().try_into().unwrap(),
                    &mut caps,
                ),
            )?;

            // Intel's and Mesa's implementation doesn't use the user-provided buffers, but changes
            // the pointer to point to static data, despite the `va_vpp.h` docs implying otherwise.
            // Support both by copying to our buffer.
            let num_in_color_stds = caps.num_input_color_standards as usize;
            let num_out_color_stds = caps.num_output_color_standards as usize;
            if caps.input_color_standards != input_color_standards.as_mut_ptr() {
                input_color_standards[..num_in_color_stds].copy_from_slice(slice::from_raw_parts(
                    caps.input_color_standards,
                    num_in_color_stds,
                ));
            }
            if caps.output_color_standards != output_color_standards.as_mut_ptr() {
                output_color_standards[..num_out_color_stds].copy_from_slice(
                    slice::from_raw_parts(caps.output_color_standards, num_out_color_stds),
                );
            }

            // Aaaand of course nobody actually supports the pixel format part of the interface.
            // So when the number is unchanged, assume the data is too, and clear it.
            if caps.num_input_pixel_formats == input_pixel_formats.len() as _ {
                input_pixel_formats.clear();
            }
            if caps.num_output_pixel_formats == output_pixel_formats.len() as _ {
                output_pixel_formats.clear();
            }

            input_color_standards.truncate(caps.num_input_color_standards as _);
            output_color_standards.truncate(caps.num_output_color_standards as _);
            input_pixel_formats.truncate(caps.num_input_pixel_formats as _);
            output_pixel_formats.truncate(caps.num_output_pixel_formats as _);

            Ok(ProcPipelineCaps {
                raw: caps,
                input_color_standards,
                output_color_standards,
                input_pixel_formats,
                output_pixel_formats,
            })
        }
    }
}

ffi_enum! {
    /// Enumeration of all known filter types.
    pub enum FilterType: u32 {
        None = 0,
        NoiseReduction = 1,
        Deinterlacing = 2,
        Sharpening = 3,
        ColorBalance = 4,
        SkinToneEnhancement = 5,
        TotalColorCorrection = 6,
        HVSNoiseReduction = 7,
        HighDynamicRangeToneMapping = 8,
        LUT3D = 9,
    }
}

ffi_enum! {
    /// Enumeration of the deinterlacing algorithm types.
    pub enum DeinterlacingType: u32 {
        None = 0,
        Bob = 1,
        Weave = 2,
        MotionAdaptive = 3,
        MotionCompensated = 4,
    }
}

ffi_enum! {
    pub enum ColorBalanceType: u32 {
        None = 0,
        Hue = 1,
        Saturation = 2,
        Brightness = 3,
        Contrast = 4,
        AutoSaturation = 5,
        AutoBrightness = 6,
        AutoContrast = 7,
    }
}

ffi_enum! {
    /// Enumeration of the known color standards.
    pub enum ColorStandardType: u32 {
        /// Unknown/Arbitrary.
        None = 0,
        /// ITU-R BT.601. The color standard used by JPEG/JFIF images.
        BT601 = 1,
        /// ITU-R BT.709.
        BT709 = 2,
        /// ITU-R BT.470-2 System M.
        BT470M = 3,
        /// ITU-R BT.470-2 System B, G.
        BT470BG = 4,
        /// SMPTE-170M.
        SMPTE170M = 5,
        /// SMPTE-240M.
        SMPTE240M = 6,
        GenericFilm = 7,
        SRGB = 8,
        STRGB = 9,
        XVYCC601 = 10,
        XVYCC709 = 11,
        /// ITU-R BT.2020.
        BT2020 = 12,
        Explicit = 13,
    }
}

ffi_enum! {
    pub enum TotalColorCorrectionType: u32 {
        None = 0,
        Red = 1,
        Green = 2,
        Blue = 3,
        Cyan = 4,
        Magenta = 5,
        Yellow = 6,
    }
}

ffi_enum! {
    pub enum HighDynamicRangeMetadataType: u32 {
        None = 0,
        HDR10 = 1,
    }
}

ffi_enum! {
    /// Video processing performance mode.
    pub enum ProcMode: u32 {
        DefaultMode = 0,
        PowerSavingMode = 1,
        PerformanceMode = 2,
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlendFlags: u32 {
        const GLOBAL_ALPHA        = 0x0001;
        const PREMULTIPLIED_ALPHA = 0x0002;
        const LUMA_KEY            = 0x0010;
    }
}

bitflags! {
    /// Flags that may be applied to a video processing pipeline.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PipelineFlags: u32 {
        const SUBPICTURES = 0x00000001;
        const FAST        = 0x00000002;
        const END         = 0x00000004;
    }
}

bitflags! {
    /// Flags and properties that may be applied to each individual filter stage.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilterFlags: u32 {
        const MANDATORY     = 0x00000001;

        const FRAME_PICTURE = 0x00000000;
        const TOP_FIELD     = 0x00000001;
        const BOTTOM_FIELD  = 0x00000002;

        const SRC_BT601     = 0x00000010;
        const SRC_BT709     = 0x00000020;
        const SRC_SMPTE_240 = 0x00000040;

        const FILTER_SCALING_DEFAULT       = 0x00000000;
        const FILTER_SCALING_FAST          = 0x00000100;
        const FILTER_SCALING_HQ            = 0x00000200;
        const FILTER_SCALING_NL_ANAMORPHIC = 0x00000300;

        const FILTER_INTERPOLATION_DEFAULT          = 0x00000000;
        const FILTER_INTERPOLATION_NEAREST_NEIGHBOR = 0x00001000;
        const FILTER_INTERPOLATION_BILINEAR         = 0x00002000;
        const FILTER_INTERPOLATION_ADVANCED         = 0x00003000;
    }
}

bitflags! {
    /// The supported [`Rotation`]s.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RotationFlags: u32 {
        const R90 = 1 << Rotation::R90.0;
        const R180 = 1 << Rotation::R180.0;
        const R270 = 1 << Rotation::R270.0;
    }
}

ffi_enum! {
    /// The location of the chroma samples.
    pub enum ChromaSiting: u8 {
        UNKNOWN           = 0x00,
        VERTICAL_TOP      = 0x01,
        VERTICAL_CENTER   = 0x02,
        VERTICAL_BOTTOM   = 0x03,
        HORIZONTAL_LEFT   = 0x04,
        HORIZONTAL_CENTER = 0x08,
    }
}

ffi_enum! {
    /// Input or output color range.
    pub enum SourceRange: u8 {
        /// Unknown, Arbitrary color range.
        UNKNOWN = 0,
        /// Color components use a limited range.
        ///
        /// Y is in range 16-235, Cb/Cr are in range 16-240.
        REDUCED = 1,
        /// Color components use the full 0-255 range.
        ///
        /// This is used, among other things, in JPEG images.
        FULL = 2,
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ToneMapping: u16 {
        const HDR_TO_HDR = 0x0001;
        const HDR_TO_SDR = 0x0002;
        const HDR_TO_EDR = 0x0004;
        const SDR_TO_HDR = 0x0008;
    }
}

/// Color-related properties of a video processing pipeline.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct ColorProperties {
    chroma_sample_location: ChromaSiting,
    color_range: SourceRange,
    colour_primaries: u8,
    transfer_characteristics: u8,
    matrix_coefficients: u8,
    reserved: [u8; 3],
}

impl ColorProperties {
    pub fn new() -> Self {
        unsafe { mem::zeroed() }
    }

    #[inline]
    pub fn chroma_sample_location(&self) -> ChromaSiting {
        self.chroma_sample_location
    }

    #[inline]
    pub fn set_chroma_sample_location(&mut self, chroma_sample_location: ChromaSiting) {
        self.chroma_sample_location = chroma_sample_location;
    }

    #[inline]
    pub fn color_range(&self) -> SourceRange {
        self.color_range
    }

    #[inline]
    pub fn set_color_range(&mut self, color_range: SourceRange) {
        self.color_range = color_range;
    }

    #[inline]
    pub fn with_chroma_sample_location(mut self, chroma_sample_location: ChromaSiting) -> Self {
        self.chroma_sample_location = chroma_sample_location;
        self
    }

    #[inline]
    pub fn with_color_range(mut self, color_range: SourceRange) -> Self {
        self.color_range = color_range;
        self
    }
}

/// Collection of supported [`FilterType`]s.
pub struct FilterTypes {
    filters: Vec<FilterType>,
}

impl FilterTypes {
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

impl IntoIterator for FilterTypes {
    type Item = FilterType;
    type IntoIter = vec::IntoIter<FilterType>;

    fn into_iter(self) -> Self::IntoIter {
        self.filters.into_iter()
    }
}

/// Configuration for a video processing pipeline.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct ProcPipelineParameterBuffer<'a> {
    surface: VASurfaceID,
    surface_region: *const Rectangle,
    surface_color_standard: ColorStandardType,
    output_region: *const Rectangle,
    output_background_color: u32,
    output_color_standard: ColorStandardType,
    pipeline_flags: PipelineFlags,
    filter_flags: FilterFlags,
    filters: *mut VABufferID,
    num_filters: u32,
    forward_references: *mut VASurfaceID,
    num_forward_references: u32,
    backward_references: *mut VASurfaceID,
    num_backward_references: u32,
    rotation_state: Rotation,
    blend_state: *const BlendState, // may be NULL
    mirror_state: Mirror,
    additional_outputs: *mut VASurfaceID,
    num_additional_outputs: u32,
    input_surface_flag: u32,
    output_surface_flag: u32,
    input_color_properties: ColorProperties,
    output_color_properties: ColorProperties,
    processing_mode: ProcMode,
    output_hdr_metadata: *const u64, // TODO port struct

    va_reserved: [u32; if cfg!(target_pointer_width = "64") {
        VA_PADDING_LARGE - 16
    } else {
        VA_PADDING_LARGE - 13
    }],

    _p: PhantomData<&'a ()>,
}

impl<'a> ProcPipelineParameterBuffer<'a> {
    /// Creates default processing pipeline parameters using the given [`Surface`] as the input
    /// image.
    ///
    /// The destination [`Surface`] is the surface passed to [`Context::begin_picture`].
    pub fn new(source: &'a Surface) -> Self {
        let mut this: Self = unsafe { mem::zeroed() };
        this.surface = source.id();
        this
    }

    #[inline]
    pub fn set_filters(&mut self, filters: &'a mut Filters) {
        self.filters = filters.as_mut_ptr();
        self.num_filters = filters.len().try_into().unwrap();
    }

    #[inline]
    pub fn input_color_standard(&self) -> ColorStandardType {
        self.surface_color_standard
    }

    #[inline]
    pub fn set_input_color_standard(&mut self, std: ColorStandardType) {
        self.surface_color_standard = std;
    }

    #[inline]
    pub fn output_color_standard(&self) -> ColorStandardType {
        self.output_color_standard
    }

    #[inline]
    pub fn set_output_color_standard(&mut self, std: ColorStandardType) {
        self.output_color_standard = std;
    }

    #[inline]
    pub fn input_color_properties(&self) -> ColorProperties {
        self.input_color_properties
    }

    #[inline]
    pub fn set_input_color_properties(&mut self, props: ColorProperties) {
        self.input_color_properties = props;
    }

    #[inline]
    pub fn output_color_properties(&self) -> ColorProperties {
        self.output_color_properties
    }

    #[inline]
    pub fn set_output_color_properties(&mut self, props: ColorProperties) {
        self.output_color_properties = props;
    }

    #[inline]
    pub fn set_filter_flags(&mut self, flags: FilterFlags) {
        self.filter_flags = flags;
    }

    #[inline]
    pub fn set_rotation(&mut self, rot: Rotation) {
        self.rotation_state = rot;
    }
}

/// A collection of video processing filters, applied in sequence.
pub struct Filters {
    buffers: Vec<RawBuffer>,
    ids: Vec<VABufferID>,
}

impl Filters {
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            ids: Vec::new(),
        }
    }

    pub fn push<T: 'static>(&mut self, buffer: Buffer<T>) {
        // FIXME: once we have types for filter parameters, this should use a trait bound restricting them
        let id = buffer.id();
        self.buffers.push(buffer.into());
        self.ids.push(id);
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    fn as_mut_ptr(&mut self) -> *mut VABufferID {
        self.ids.as_mut_ptr()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FilterValueRange {
    min_value: f32,
    max_value: f32,
    default_value: f32,
    step: f32,
    va_reserved: [u32; VA_PADDING_LOW],
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BlendState {
    flags: c_uint,
    global_alpha: f32,
    min_luma: f32,
    max_luma: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FilterParameterBufferBase {
    type_: FilterType,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct FilterParameterBuffer {
    type_: FilterType,
    value: f32,
    va_reserved: [u32; VA_PADDING_LOW],
}

/// Capabilities of a video processing pipeline.
///
/// Returned by [`Context::query_video_processing_pipeline_caps`].
pub struct ProcPipelineCaps {
    raw: RawProcPipelineCaps,
    input_color_standards: Vec<ColorStandardType>,
    output_color_standards: Vec<ColorStandardType>,
    input_pixel_formats: Vec<PixelFormat>,
    output_pixel_formats: Vec<PixelFormat>,
}

impl ProcPipelineCaps {
    #[inline]
    pub fn pipeline_flags(&self) -> PipelineFlags {
        self.raw.pipeline_flags
    }

    #[inline]
    pub fn filter_flags(&self) -> FilterFlags {
        self.raw.filter_flags
    }

    #[inline]
    pub fn num_forward_references(&self) -> u32 {
        self.raw.num_forward_references
    }

    #[inline]
    pub fn num_backward_references(&self) -> u32 {
        self.raw.num_backward_references
    }

    #[inline]
    pub fn input_color_standards(&self) -> &[ColorStandardType] {
        &self.input_color_standards
    }

    #[inline]
    pub fn output_color_standards(&self) -> &[ColorStandardType] {
        &self.output_color_standards
    }

    #[inline]
    pub fn input_pixel_formats(&self) -> &[PixelFormat] {
        &self.input_pixel_formats
    }

    #[inline]
    pub fn output_pixel_formats(&self) -> &[PixelFormat] {
        &self.output_pixel_formats
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub(crate) struct RawProcPipelineCaps {
    pipeline_flags: PipelineFlags,
    filter_flags: FilterFlags,
    num_forward_references: u32,
    num_backward_references: u32,
    input_color_standards: *const ColorStandardType,
    num_input_color_standards: u32,
    output_color_standards: *const ColorStandardType,
    num_output_color_standards: u32,
    rotation_flags: RotationFlags,
    blend_flags: BlendFlags,
    mirror_flags: Mirror,
    num_additional_outputs: u32,

    num_input_pixel_formats: u32,
    input_pixel_format: *const PixelFormat,
    num_output_pixel_formats: u32,
    output_pixel_format: *const PixelFormat,

    max_input_width: u32,
    max_input_height: u32,
    min_input_width: u32,
    min_input_height: u32,

    max_output_width: u32,
    max_output_height: u32,
    min_output_width: u32,
    min_output_height: u32,

    va_reserved: [u32; if cfg!(target_pointer_width = "64") {
        VA_PADDING_HIGH - 2
    } else {
        VA_PADDING_HIGH
    }],
}

#[cfg(test)]
mod tests {
    use crate::{
        buffer::BufferType,
        config::Config,
        image::{Image, ImageFormat},
        test::*,
        Entrypoint, Profile,
    };

    use super::*;

    #[test]
    #[ignore] // FIXME: fails on AMD/Mesa
    fn vpp_copy() {
        let display = test_display();

        // Surface with test data.
        let input_surface = test_surface(&display);

        let mut output_surface =
            Surface::new(&display, TEST_WIDTH, TEST_HEIGHT, TEST_RTFORMAT).unwrap();

        let config = Config::new(&display, Profile::None, Entrypoint::VideoProc).unwrap();
        let mut context = Context::new(&config, TEST_WIDTH, TEST_HEIGHT).unwrap();

        let mut pppbuf = ProcPipelineParameterBuffer::new(&input_surface);
        let props = ColorProperties::new().with_color_range(SourceRange::FULL);
        pppbuf.set_input_color_properties(props);
        pppbuf.set_input_color_standard(ColorStandardType::BT601);
        pppbuf.set_output_color_properties(props);
        pppbuf.set_output_color_standard(ColorStandardType::SRGB);

        let mut params =
            Buffer::new_param(&context, BufferType::ProcPipelineParameter, pppbuf).unwrap();

        let mut picture = context.begin_picture(&mut output_surface).unwrap();
        picture.render_picture(&mut params).unwrap();
        unsafe {
            picture.end_picture().unwrap();
        }

        let mut output_image = Image::new(
            &display,
            ImageFormat::new(TEST_PIXELFORMAT),
            TEST_WIDTH,
            TEST_HEIGHT,
        )
        .expect("failed to create output image");

        output_surface.sync().expect("sync failed");
        // TODO: the following unwrap fails on AMD/Mesa for seemingly no reason
        output_surface.copy_to_image(&mut output_image).unwrap();

        output_surface.sync().unwrap();
        let map = output_image.map().expect("failed to map output image");
        assert_eq!(&map[..TEST_DATA.len()], TEST_DATA);
    }
}
