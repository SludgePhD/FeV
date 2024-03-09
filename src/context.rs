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
            check(
                "vaCreateContext",
                config.d.libva.vaCreateContext(
                    config.d.raw,
                    config.id,
                    picture_width as _,
                    picture_height as _,
                    0,
                    ptr::null_mut(),
                    0,
                    &mut context_id,
                ),
            )?;
            Ok(Context {
                d: config.d.clone(),
                id: context_id,
            })
        }
    }

    /// Begins a libva operation that will render to (or encode from) the given [`Surface`].
    ///
    /// Returns an [`InProgressPicture`] that can be used to submit parameter and data buffers to
    /// libva.
    pub fn begin_picture<'a>(
        &'a mut self,
        target: &'a mut Surface,
    ) -> Result<InProgressPicture<'a>> {
        unsafe {
            check(
                "vaBeginPicture",
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
                "vaDestroyContext",
                self.d.libva.vaDestroyContext(self.d.raw, self.id),
            );
        }
    }
}

/// An operation whose submission is still in progress.
///
/// Submit parameter and data buffers with [`InProgressPicture::render_picture`] and finish the
/// operation, kicking off decoding or encoding, by calling [`InProgressPicture::end_picture`].
pub struct InProgressPicture<'a> {
    d: Arc<DisplayOwner>,
    context: &'a mut Context,
}

impl<'a> InProgressPicture<'a> {
    /// Submits a [`Buffer`] as part of this libva operation.
    ///
    /// Typically, libva does not document which buffer types are required for any given entry
    /// point, so good luck!
    ///
    /// # Safety
    ///
    /// Buffers containing metadata structures must contain a valid value of the particular subtype
    /// required by the configured [`Profile`][crate::Profile] and
    /// [`Entrypoint`][crate::Entrypoint].
    ///
    /// For example, when using [`Profile::JPEGBaseline`][crate::Profile::JPEGBaseline] and
    /// [`Entrypoint::VLD`][crate::Entrypoint::VLD], submitting a [`Buffer`] with
    /// [`BufferType::SliceParameter`][crate::buffer::BufferType::SliceParameter] requires that the
    /// [`Buffer`] contains a [`jpeg::SliceParameterBuffer`][crate::jpeg::SliceParameterBuffer],
    /// and submitting only the substructure [`SliceParameterBufferBase`] will cause Undefined
    /// Behavior.
    ///
    /// [`SliceParameterBufferBase`]: crate::SliceParameterBufferBase
    pub unsafe fn render_picture<T>(&mut self, buffer: &mut Buffer<T>) -> Result<()> {
        check(
            "vaRenderPicture",
            self.d
                .libva
                .vaRenderPicture(self.d.raw, self.context.id, &mut buffer.id(), 1),
        )
    }

    /// Finishes submitting buffers, and begins the libva operation (encode, decode, etc.).
    ///
    /// # Safety
    ///
    /// libva does not specify when Undefined Behavior occurs, and in practice at least some
    /// implementations exhibit UB-like behavior when buffers are submitted incorrectly (or when
    /// not all buffers required by the operation were submitted). It also does not document which
    /// buffer types must be submitted (or must not be submitted) for any given entry point.
    ///
    /// So, basically, the safety invariant of this method is "fuck if I know". Good luck, Loser.
    pub unsafe fn end_picture(self) -> Result<()> {
        check(
            "vaEndPicture",
            self.d.libva.vaEndPicture(self.d.raw, self.context.id),
        )
    }
}
