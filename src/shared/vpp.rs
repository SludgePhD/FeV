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
        None = 0,
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
        const MANDATORY = 0x00000001;
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
        UNKNOWN = 0,
        REDUCED = 1,
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
