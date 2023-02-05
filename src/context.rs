//! Codec contexts.

use std::{ptr, sync::Arc};

use crate::{
    buffer::Buffer, check, check_log, config::Config, display::DisplayOwner, raw::VAContextID,
    surface::Surface, Result,
};

/// A codec, configured for a video operation.
///
/// Submit work to a context by calling [`Context::begin_picture`].
pub struct Context {
    pub(crate) d: Arc<DisplayOwner>,
    pub(crate) id: VAContextID,
}

impl Context {
    pub fn new(config: &Config, picture_width: u32, picture_height: u32) -> Result<Self> {
        unsafe {
            let mut context_id = 0;
            check(config.d.libva.vaCreateContext(
                config.d.raw,
                config.id,
                picture_width as _,
                picture_height as _,
                0,
                ptr::null_mut(),
                0,
                &mut context_id,
            ))?;
            Ok(Context {
                d: config.d.clone(),
                id: context_id,
            })
        }
    }

    /// Begins a libva operation that will render to (or encode from) the given [`Surface`].
    pub fn begin_picture<'a>(
        &'a mut self,
        target: &'a mut Surface,
    ) -> Result<InProgressPicture<'a>> {
        unsafe {
            check(
                self.d
                    .libva
                    .vaBeginPicture(self.d.raw, self.id, target.id()),
            )?;
        }

        Ok(InProgressPicture {
            d: self.d.clone(),
            context: self,
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            check_log(
                self.d.libva.vaDestroyContext(self.d.raw, self.id),
                "vaDestroyContext call in drop",
            );
        }
    }
}

/// An operation whose submission is still in progress.
pub struct InProgressPicture<'a> {
    d: Arc<DisplayOwner>,
    context: &'a mut Context,
}

impl<'a> InProgressPicture<'a> {
    /// Submits a [`Buffer`] as part of this libva operation.
    ///
    /// Typically, libva does not document which buffer types are required for any given entry
    /// point, so good luck!
    pub fn render_picture<T>(&mut self, buffer: &mut Buffer<T>) -> Result<()> {
        unsafe {
            check(
                self.d
                    .libva
                    .vaRenderPicture(self.d.raw, self.context.id, &mut buffer.id(), 1),
            )
        }
    }

    /// Finishes submitting buffers, and begins the libva operation (encode, decode, etc.).
    ///
    /// # Safety
    ///
    /// libva does not specify when Undefined Behavior occurs, and in practice at least some
    /// implementations exhibit UB-like behavior when buffers where submitted incorrectly (or when
    /// not all buffers required by the operation were submitted). It also does not document which
    /// buffer types must be submitted (or must not be submitted) for any given entry point.
    ///
    /// So, basically, the safety invariant of this method is "fuck if I know". Good luck, Loser.
    pub unsafe fn end_picture(self) -> Result<()> {
        check(self.d.libva.vaEndPicture(self.d.raw, self.context.id))
    }
}
