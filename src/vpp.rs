use std::{ffi::c_uint, vec};

use crate::{check, shared::vpp::FilterType, Context, Result};

impl Context {
    /// Fetches the list of supported video processing filter types.
    pub fn query_video_processing_filters(&self) -> Result<VideoProcessingFilters> {
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

        Ok(VideoProcessingFilters { filters })
    }
}

pub struct VideoProcessingFilters {
    filters: Vec<FilterType>,
}

impl VideoProcessingFilters {
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

impl IntoIterator for VideoProcessingFilters {
    type Item = FilterType;
    type IntoIter = vec::IntoIter<FilterType>;

    fn into_iter(self) -> Self::IntoIter {
        self.filters.into_iter()
    }
}
