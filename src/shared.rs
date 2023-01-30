//! FFI-compatible type definitions that may be directly exposed to Rust.

#![allow(non_upper_case_globals)]

pub mod jpeg;
pub mod vpp;

use std::{
    ffi::CStr,
    mem,
    os::raw::{c_int, c_uint},
};

use crate::{dlopen::libva, pixelformat::PixelFormat, raw::VA_PADDING_LOW, Error};

ffi_enum! {
    pub enum VAStatus: c_int {
        SUCCESS                        = 0x00000000,
        // Other allowed values are in `VAError`.
    }
}

ffi_enum! {
    /// An error code returned by *libva*.
    pub enum VAError: c_int {
        ERROR_OPERATION_FAILED         = 0x00000001,
        ERROR_ALLOCATION_FAILED        = 0x00000002,
        ERROR_INVALID_DISPLAY          = 0x00000003,
        ERROR_INVALID_CONFIG           = 0x00000004,
        ERROR_INVALID_CONTEXT          = 0x00000005,
        ERROR_INVALID_SURFACE          = 0x00000006,
        ERROR_INVALID_BUFFER           = 0x00000007,
        ERROR_INVALID_IMAGE            = 0x00000008,
        ERROR_INVALID_SUBPICTURE       = 0x00000009,
        ERROR_ATTR_NOT_SUPPORTED       = 0x0000000a,
        ERROR_MAX_NUM_EXCEEDED         = 0x0000000b,
        ERROR_UNSUPPORTED_PROFILE      = 0x0000000c,
        ERROR_UNSUPPORTED_ENTRYPOINT   = 0x0000000d,
        ERROR_UNSUPPORTED_RT_FORMAT    = 0x0000000e,
        ERROR_UNSUPPORTED_BUFFERTYPE   = 0x0000000f,
        ERROR_SURFACE_BUSY             = 0x00000010,
        ERROR_FLAG_NOT_SUPPORTED       = 0x00000011,
        ERROR_INVALID_PARAMETER        = 0x00000012,
        ERROR_RESOLUTION_NOT_SUPPORTED = 0x00000013,
        ERROR_UNIMPLEMENTED            = 0x00000014,
        ERROR_SURFACE_IN_DISPLAYING    = 0x00000015,
        ERROR_INVALID_IMAGE_FORMAT     = 0x00000016,
        ERROR_DECODING_ERROR           = 0x00000017,
        ERROR_ENCODING_ERROR           = 0x00000018,
        ERROR_INVALID_VALUE            = 0x00000019,
        ERROR_UNSUPPORTED_FILTER       = 0x00000020,
        ERROR_INVALID_FILTER_CHAIN     = 0x00000021,
        ERROR_HW_BUSY                  = 0x00000022,
        ERROR_UNSUPPORTED_MEMORY_TYPE  = 0x00000024,
        ERROR_NOT_ENOUGH_BUFFER        = 0x00000025,
        ERROR_TIMEDOUT                 = 0x00000026,
        #[allow(overflowing_literals)]
        ERROR_UNKNOWN                  = 0xFFFFFFFF,
    }
}

impl From<VAError> for VAStatus {
    #[inline]
    fn from(e: VAError) -> Self {
        Self(e.0)
    }
}

impl PartialEq<VAError> for VAStatus {
    #[inline]
    fn eq(&self, other: &VAError) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<VAStatus> for VAError {
    #[inline]
    fn eq(&self, other: &VAStatus) -> bool {
        self.0 == other.0
    }
}

impl VAError {
    pub fn to_str(self) -> Result<&'static str, Error> {
        unsafe {
            let cstr = &CStr::from_ptr(libva::get().map_err(Error::from)?.vaErrorStr(self.into()));
            Ok(cstr.to_str().map_err(Error::from)?)
        }
    }
}

ffi_enum! {
    pub enum Profile: c_int {
        /// "Misc" profile for format-independent operations.
        None = -1,
        MPEG2Simple = 0,
        MPEG2Main = 1,
        MPEG4Simple = 2,
        MPEG4AdvancedSimple = 3,
        MPEG4Main = 4,
        H264Baseline = 5,
        H264Main = 6,
        H264High = 7,
        VC1Simple = 8,
        VC1Main = 9,
        VC1Advanced = 10,
        H263Baseline = 11,
        JPEGBaseline = 12,
        H264ConstrainedBaseline = 13,
        VP8Version0_3 = 14,
        H264MultiviewHigh = 15,
        H264StereoHigh = 16,
        HEVCMain = 17,
        HEVCMain10 = 18,
        VP9Profile0 = 19,
        VP9Profile1 = 20,
        VP9Profile2 = 21,
        VP9Profile3 = 22,
        HEVCMain12 = 23,
        HEVCMain422_10 = 24,
        HEVCMain422_12 = 25,
        HEVCMain444 = 26,
        HEVCMain444_10 = 27,
        HEVCMain444_12 = 28,
        HEVCSccMain = 29,
        HEVCSccMain10 = 30,
        HEVCSccMain444 = 31,
        AV1Profile0 = 32,
        AV1Profile1 = 33,
        HEVCSccMain444_10 = 34,
        Protected = 35,
    }
}

ffi_enum! {
    pub enum Entrypoint: c_int {
        /// Variable-length decoding.
        VLD         = 1,
        IZZ         = 2,
        IDCT        = 3,
        MoComp      = 4,
        Deblocking  = 5,
        EncSlice    = 6,    /* slice level encode */
        EncPicture  = 7,    /* pictuer encode, JPEG, etc */
        EncSliceLP  = 8,    /* low-power variant */
        VideoProc   = 10,
        /// Flexible Encoding Infrastructure
        FEI         = 11,
        Stats       = 12,
        ProtectedTEEComm = 13,
        ProtectedContent = 14,
    }
}

ffi_enum! {
    pub enum ConfigAttribType: c_int {
        RTFormat          = 0,
        SpatialResidual   = 1,
        SpatialClipping   = 2,
        IntraResidual     = 3,
        Encryption        = 4,
        RateControl       = 5,
        DecSliceMode      = 6,
        DecJPEG           = 7,
        DecProcessing     = 8,
        EncPackedHeaders  = 10,
        EncInterlaced     = 11,
        EncMaxRefFrames   = 13,
        EncMaxSlices      = 14,
        EncSliceStructure = 15,
        EncMacroblockInfo = 16,
        MaxPictureWidth   = 18,
        MaxPictureHeight  = 19,
        EncJPEG             = 20,
        EncQualityRange     = 21,
        EncQuantization     = 22,
        EncIntraRefresh     = 23,
        EncSkipFrame        = 24,
        EncROI              = 25,
        EncRateControlExt   = 26,
        ProcessingRate      = 27,
        EncDirtyRect        = 28,
        EncParallelRateControl    = 29,
        EncDynamicScaling         = 30,
        FrameSizeToleranceSupport = 31,
        FEIFunctionType       = 32,
        FEIMVPredictors       = 33,
        Stats                 = 34,
        EncTileSupport        = 35,
        CustomRoundingControl = 36,
        QPBlockSize           = 37,
        MaxFrameSize          = 38,
        PredictionDirection   = 39,
        MultipleFrame         = 40,
        ContextPriority       = 41,
        DecAV1Features        = 42,
        TEEType               = 43,
        TEETypeClient         = 44,
        ProtectedContentCipherAlgorithm  = 45,
        ProtectedContentCipherBlockSize  = 46,
        ProtectedContentCipherMode       = 47,
        ProtectedContentCipherSampleType = 48,
        ProtectedContentUsage = 49,
        EncHEVCFeatures       = 50,
        EncHEVCBlockSizes     = 51,
        EncAV1                = 52,
        EncAV1Ext1            = 53,
        EncAV1Ext2            = 54,
        EncPerBlockControl    = 55,
    }
}

ffi_enum! {
    pub enum VAGenericValueType: c_int {
        Integer = 1,      /**< 32-bit signed integer. */
        Float = 2,            /**< 32-bit floating-point value. */
        Pointer = 3,          /**< Generic pointer type */
        Func = 4,
    }
}

ffi_enum! {
    pub enum VASurfaceAttribType: c_int {
        None = 0,
        PixelFormat = 1,
        MinWidth = 2,
        MaxWidth = 3,
        MinHeight = 4,
        MaxHeight = 5,
        MemoryType = 6,
        ExternalBufferDescriptor = 7,
        UsageHint = 8,
        DRMFormatModifiers = 9,
    }
}

ffi_enum! {
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

ffi_enum! {
    pub enum SurfaceStatus: c_int {
        Rendering = 1,
        Displaying = 2,
        Ready = 4,
        Skipped = 8,
    }
}

ffi_enum! {
    pub enum VADecodeErrorType: c_int {
        SliceMissing = 0,
        MBError = 1,
    }
}

ffi_enum! {
    pub enum ByteOrder: u32 {
        None = 0,
        LsbFirst = 1,
        MsbFirst = 2,
    }
}

ffi_enum! {
    pub enum VADisplayAttribType: c_int {
        Brightness          = 0,
        Contrast            = 1,
        Hue                 = 2,
        Saturation          = 3,
        BackgroundColor     = 4,
        DirectSurface       = 5,
        Rotation            = 6,
        OutofLoopDeblock    = 7,
        BLEBlackMode        = 8,
        BLEWhiteMode        = 9,
        BlueStretch         = 10,
        SkinColorCorrection = 11,
        CSCMatrix           = 12,
        BlendColor          = 13,
        OverlayAutoPaintColorKey = 14,
        OverlayColorKey     = 15,
        RenderMode          = 16,
        RenderDevice        = 17,
        RenderRect          = 18,
        SubDevice           = 19,
        Copy                = 20,
        PCIID               = 21,
    }
}

ffi_enum! {
    pub enum Rotation: u32 {
        NONE = 0x00000000,
        R90  = 0x00000001,
        R180 = 0x00000002,
        R270 = 0x00000003,
    }
}

bitflags! {
    pub struct Mirror: u32 {
        const NONE = 0;
        const HORIZONTAL = 0x00000001;
        const VERTICAL   = 0x00000002;
    }
}

bitflags! {
    pub struct VASurfaceAttribFlags: c_int {
        const GETTABLE = 0x00000001;
        const SETTABLE = 0x00000002;
    }
}

bitflags! {
    pub struct VASurfaceAttribMemoryType: u32 {
        // Generic types
        const VA       = 0x00000001;
        const V4L2     = 0x00000002;
        const USER_PTR = 0x00000004;

        // DRM types
        const KERNEL_DRM  = 0x10000000;
        const DRM_PRIME   = 0x20000000;
        const DRM_PRIME_2 = 0x40000000;
    }
}

bitflags! {
    pub struct VAExportSurface: u32 {
        const READ_ONLY = 0x0001;
        const WRITE_ONLY = 0x0002;
        const READ_WRITE = 0x0003;
        const SEPARATE_LAYERS = 0x0004;
        const COMPOSED_LAYERS = 0x0008;
    }
}

bitflags! {
    /// Surface pixel formats.
    pub struct RTFormat: c_uint {
        const YUV420    = 0x00000001;
        const YUV422    = 0x00000002;
        const YUV444    = 0x00000004;
        const YUV411    = 0x00000008;
        const YUV400    = 0x00000010;
        const YUV420_10 = 0x00000100;
        const YUV422_10 = 0x00000200;
        const YUV444_10 = 0x00000400;
        const YUV420_12 = 0x00001000;
        const YUV422_12 = 0x00002000;
        const YUV444_12 = 0x00004000;
        const RGB16     = 0x00010000;
        const RGB32     = 0x00020000;
        const RGBP      = 0x00100000;
        const RGB32_10  = 0x00200000;
        const PROTECTED = 0x80000000;
    }
}

impl Default for RTFormat {
    #[inline]
    fn default() -> Self {
        Self::YUV420
    }
}

bitflags! {
    pub struct VADisplayAttribFlags: u32 {
        const GETTABLE = 0x0001;
        const SETTABLE = 0x0002;
    }
}

bitflags! {
    pub struct VASubpictureFlags: u32 {
        const CHROMA_KEYING = 0x0001;
        const GLOBAL_ALPHA  = 0x0002;
        const DESTINATION_IS_SCREEN_COORD = 0x0004;
    }
}

bitflags! {
    pub struct SliceDataFlags: u32 {
        const ALL    = 0x00;
        const BEGIN  = 0x01;
        const MIDDLE = 0x02;
        const END    = 0x04;
    }
}

/// Codec-independent slice parameters.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SliceParameterBufferBase {
    slice_data_size: u32,
    slice_data_offset: u32,
    slice_data_flags: SliceDataFlags,
}

impl SliceParameterBufferBase {
    #[inline]
    pub fn new(slice_data_size: u32) -> Self {
        Self {
            slice_data_size,
            slice_data_offset: 0,
            slice_data_flags: SliceDataFlags::ALL,
        }
    }

    #[inline]
    pub fn slice_data_size(&self) -> u32 {
        self.slice_data_size
    }

    #[inline]
    pub fn slice_data_offset(&self) -> u32 {
        self.slice_data_offset
    }

    #[inline]
    pub fn set_slice_data_offset(&mut self, slice_data_offset: u32) {
        self.slice_data_offset = slice_data_offset;
    }

    #[inline]
    pub fn slice_data_flags(&self) -> SliceDataFlags {
        self.slice_data_flags
    }

    #[inline]
    pub fn set_slice_data_flags(&mut self, flags: SliceDataFlags) {
        self.slice_data_flags = flags;
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ImageFormat {
    pub(crate) fourcc: PixelFormat,
    pub(crate) byte_order: ByteOrder,
    pub(crate) bits_per_pixel: u32,
    pub(crate) depth: u32,
    pub(crate) red_mask: u32,
    pub(crate) green_mask: u32,
    pub(crate) blue_mask: u32,
    pub(crate) alpha_mask: u32,
    va_reserved: [u32; VA_PADDING_LOW],
}

impl ImageFormat {
    pub fn new(pixel_format: PixelFormat) -> Self {
        Self {
            fourcc: pixel_format,
            ..unsafe { mem::zeroed() }
        }
    }

    #[inline]
    pub fn pixel_format(&self) -> PixelFormat {
        self.fourcc
    }

    #[inline]
    pub fn set_pixel_format(&mut self, fmt: PixelFormat) {
        self.fourcc = fmt;
    }

    #[inline]
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }
    #[inline]
    pub fn set_byte_order(&mut self, byte_order: ByteOrder) {
        self.byte_order = byte_order;
    }

    #[inline]
    pub fn bits_per_pixel(&self) -> u32 {
        self.bits_per_pixel
    }

    #[inline]
    pub fn set_bits_per_pixel(&mut self, bits_per_pixel: u32) {
        self.bits_per_pixel = bits_per_pixel;
    }

    #[inline]
    pub fn depth(&self) -> u32 {
        self.depth
    }

    #[inline]
    pub fn set_depth(&mut self, depth: u32) {
        self.depth = depth;
    }

    #[inline]
    pub fn red_mask(&self) -> u32 {
        self.red_mask
    }

    #[inline]
    pub fn set_red_mask(&mut self, red_mask: u32) {
        self.red_mask = red_mask;
    }

    #[inline]
    pub fn green_mask(&self) -> u32 {
        self.green_mask
    }

    #[inline]
    pub fn set_green_mask(&mut self, green_mask: u32) {
        self.green_mask = green_mask;
    }

    #[inline]
    pub fn blue_mask(&self) -> u32 {
        self.blue_mask
    }

    #[inline]
    pub fn set_blue_mask(&mut self, blue_mask: u32) {
        self.blue_mask = blue_mask;
    }

    #[inline]
    pub fn alpha_mask(&self) -> u32 {
        self.alpha_mask
    }

    #[inline]
    pub fn set_alpha_mask(&mut self, alpha_mask: u32) {
        self.alpha_mask = alpha_mask;
    }
}

/// The default image format uses the [`PixelFormat::NV12`] format.
impl Default for ImageFormat {
    fn default() -> Self {
        Self::new(PixelFormat::NV12)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ConfigAttrib {
    type_: ConfigAttribType,
    value: u32,
}

impl ConfigAttrib {
    pub fn zeroed() -> Self {
        unsafe { mem::zeroed() }
    }

    #[inline]
    pub fn attrib_type(&self) -> ConfigAttribType {
        self.type_
    }

    #[inline]
    pub fn raw_value(&self) -> u32 {
        self.value
    }
}
