#![doc(hidden)] // it's not funny or cute!! rustdoc only does this when it's very distressed!

ffi_enum! {
    pub enum ColorSpace: u8 {
        YUV = 0,
        RGB = 1,
        BGR = 2,
    }
}
