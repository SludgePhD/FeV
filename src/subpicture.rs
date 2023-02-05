//! Subpictures and surface blending.
//!
//! (TODO)

bitflags! {
    pub struct SubpictureFlags: u32 {
        const CHROMA_KEYING = 0x0001;
        const GLOBAL_ALPHA  = 0x0002;
        const DESTINATION_IS_SCREEN_COORD = 0x0004;
    }
}
