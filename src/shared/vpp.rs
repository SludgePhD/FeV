use std::mem;

ffi_enum! {
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
    pub enum ColorStandardType: u32 {
        /// Unknown/Arbitrary.
        None = 0,
        /// The color standard used by JPEG/JFIF images.
        BT601 = 1,
        BT709 = 2,
        BT470M = 3,
        BT470BG = 4,
        SMPTE170M = 5,
        SMPTE240M = 6,
        GenericFilm = 7,
        SRGB = 8,
        STRGB = 9,
        XVYCC601 = 10,
        XVYCC709 = 11,
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
    pub enum ProcMode: u32 {
        DefaultMode = 0,
        PowerSavingMode = 1,
        PerformanceMode = 2,
    }
}

bitflags! {
    pub struct BlendFlags: u32 {
        const GLOBAL_ALPHA        = 0x0001;
        const PREMULTIPLIED_ALPHA = 0x0002;
        const LUMA_KEY            = 0x0010;
    }
}

bitflags! {
    pub struct PipelineFlags: u32 {
        const SUBPICTURES = 0x00000001;
        const FAST        = 0x00000002;
        const END         = 0x00000004;
    }
}

bitflags! {
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

ffi_enum! {
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
    pub struct ToneMapping: u16 {
        const HDR_TO_HDR = 0x0001;
        const HDR_TO_SDR = 0x0002;
        const HDR_TO_EDR = 0x0004;
        const SDR_TO_HDR = 0x0008;
    }
}

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
