#![allow(non_snake_case)]

use bytemuck::{Pod, Zeroable};

pub struct JpegParser<'a> {
    buf: &'a [u8],
}

impl<'a> JpegParser<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }
}

pub struct Segment<'a> {
    /// Offset of the segment's marker in the input buffer.
    pub pos: usize,
    pub kind: SegmentKind<'a>,
}

pub enum SegmentKind<'a> {
    Dqt(Dqt<'a>),
    Dht(Dht<'a>),
    Dri(Dri),
    Sof(Sof<'a>),
    Sos(Sos<'a>),
    Eoi,
    Other { marker: u8, data: &'a [u8] },
}

#[non_exhaustive]
pub struct Dqt<'a> {
    pub Pq: u8,
    pub Dq: u8,
    pub Qk: &'a [u8; 64],
}

#[non_exhaustive]
pub struct Dht<'a> {
    /// Table class (0 = DC table/lossless table, 1 = AC table).
    pub Tc: u8,
    pub Th: u8,
    pub Li: &'a [u8; 16],
    pub Vij: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct Dri {
    Ri: u16,
}

impl Dri {
    #[inline]
    pub fn restart_interval(&self) -> u16 {
        self.Ri
    }
}

pub struct Sof<'a> {
    pub sof: u8,
    /// Sample precision in bits.
    pub P: u8,
    pub Y: u16,
    pub X: u16,
    pub components: &'a [FrameComponent],
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct FrameComponent {
    Ci: u8,
    HiVi: u8,
    Tqi: u8,
}

impl FrameComponent {
    #[inline]
    pub fn id(&self) -> u8 {
        self.Ci
    }

    #[inline]
    pub fn horizontal_sampling_factor(&self) -> u8 {
        self.HiVi >> 4
    }

    #[inline]
    pub fn vertical_sampling_factor(&self) -> u8 {
        self.HiVi & 0xf
    }
}

pub struct Sos<'a> {
    pub components: &'a [ScanComponent],
    pub Ss: u8,
    pub Se: u8,
    pub AhAl: u8,
    pub data_start: usize,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct ScanComponent {
    Csj: u8,
    TdjTaj: u8,
}

impl ScanComponent {
    #[inline]
    pub fn id(&self) -> u8 {
        self.Csj
    }
}
