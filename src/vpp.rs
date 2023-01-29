use std::{ffi::c_uint, marker::PhantomData, mem, vec};

use crate::{
    check,
    raw::{
        vpp::{VAProcPipelineCaps, VAProcPipelineParameterBuffer},
        VABufferID,
    },
    Buffer, Context, RawBuffer, Result, Rotation, Surface,
};

pub use crate::shared::vpp::*;

impl Context {
    /// Fetches the list of supported video processing filter types.
    pub fn query_video_processing_filters(&self) -> Result<FilterTypes> {
        // The docs of `vaQueryVideoProcFilters` clearly state that the number of filters will be
        // returned in `num_filters`, if it is higher than what we pass to it (and the function will
        // return a `MAX_NUM_EXCEEDED` error).
        // This, however, is a lie. The function does no such thing (it succeeds and returns a
        // truncated list), so we just preallocate a "large" array and shrink it later.

        const PREALLOC: usize = 512;

        let mut num_filters = PREALLOC as c_uint;
        let mut filters = vec![FilterType::None; PREALLOC];
        unsafe {
            check(self.d.libva.vaQueryVideoProcFilters(
                self.d.raw,
                self.id,
                filters.as_mut_ptr(),
                &mut num_filters,
            ))?;
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
        // TODO: also query color standards, pixel formats, etc.
        unsafe {
            let mut caps: VAProcPipelineCaps = mem::zeroed();
            check(self.d.libva.vaQueryVideoProcPipelineCaps(
                self.d.raw,
                self.id,
                filters.as_mut_ptr(),
                filters.len().try_into().unwrap(),
                &mut caps,
            ))?;
            Ok(ProcPipelineCaps { raw: caps })
        }
    }
}

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

#[derive(Clone, Copy)]
pub struct ProcPipelineParameterBuffer<'a> {
    raw: VAProcPipelineParameterBuffer,
    _p: PhantomData<&'a ()>,
}

impl<'a> ProcPipelineParameterBuffer<'a> {
    pub fn new(source: &'a Surface) -> Self {
        let mut raw: VAProcPipelineParameterBuffer = unsafe { mem::zeroed() };
        raw.surface = source.id;
        Self {
            raw,
            _p: PhantomData,
        }
    }

    pub fn set_filters(&mut self, filters: &'a mut Filters) {
        self.raw.filters = filters.as_mut_ptr();
        self.raw.num_filters = filters.len().try_into().unwrap();
    }

    #[inline]
    pub fn input_color_standard(&self) -> ColorStandardType {
        self.raw.surface_color_standard
    }

    #[inline]
    pub fn set_input_color_standard(&mut self, std: ColorStandardType) {
        self.raw.surface_color_standard = std;
    }

    #[inline]
    pub fn output_color_standard(&self) -> ColorStandardType {
        self.raw.output_color_standard
    }

    #[inline]
    pub fn set_output_color_standard(&mut self, std: ColorStandardType) {
        self.raw.output_color_standard = std;
    }

    pub fn input_color_properties(&self) -> ColorProperties {
        self.raw.input_color_properties
    }

    pub fn set_input_color_properties(&mut self, props: ColorProperties) {
        self.raw.input_color_properties = props;
    }

    pub fn output_color_properties(&self) -> ColorProperties {
        self.raw.output_color_properties
    }

    pub fn set_output_color_properties(&mut self, props: ColorProperties) {
        self.raw.output_color_properties = props;
    }

    #[inline]
    pub fn set_filter_flags(&mut self, flags: FilterFlags) {
        self.raw.filter_flags = flags;
    }

    pub fn set_rotation(&mut self, rot: Rotation) {
        self.raw.rotation_state = rot;
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

    pub fn push<T>(&mut self, buffer: Buffer<T>) {
        // FIXME: once we have types for filter parameters, this should use a trait bound restricting them
        let id = buffer.raw.id;
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

pub struct ProcPipelineCaps {
    raw: VAProcPipelineCaps,
}

impl ProcPipelineCaps {
    #[inline]
    pub fn filter_flags(&self) -> FilterFlags {
        self.raw.filter_flags
    }

    // TODO: fill this in
}
