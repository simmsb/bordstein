use core::{ffi::CStr, marker::PhantomData, ptr::NonNull};

use crate::{
    bindings::{self, GColor, GRect, GSize},
    font::Font,
};

use super::{AsChildLayer, IsLayer, LayerMut, LayerRef};

pub struct TextLayer<'layer> {
    pub(crate) inner: NonNull<bindings::TextLayer>,

    pub(crate) _phantom: PhantomData<&'layer ()>,
}

impl<'layer> TextLayer<'layer> {
    pub(crate) fn new(frame: GRect) -> Option<Self> {
        let ptr = unsafe { bindings::text_layer_create(frame) };
        NonNull::new(ptr).map(Self::from_ptr)
    }

    pub(crate) fn from_ptr(ptr: NonNull<bindings::TextLayer>) -> Self {
        Self {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    /// Set contents of this text layer.
    ///
    /// This method returns a guard which replaces the text when dropped, this
    /// ensures that the string reference lives and is unmutated while it is
    /// being used by the text layer.
    #[must_use = "Content is set back to an empty string when the returned guard is dropped"]
    pub fn set_text<'text, 'a>(&'a mut self, text: &'text CStr) -> SetTextGuard<'text, 'a> {
        unsafe {
            bindings::text_layer_set_text(self.inner.as_ptr(), text.as_ptr());
        }

        SetTextGuard {
            layer: self.inner,
            _phantom: PhantomData,
        }
    }

    /// View the text of the text layer.
    pub fn get_text<T>(&self, f: impl for<'text> FnOnce(&'text CStr) -> T) -> Option<T> {
        let ptr = unsafe { bindings::text_layer_get_text(self.inner.as_ptr()) };
        let cstr = unsafe { NonNull::new(ptr.cast_mut()).map(|p| CStr::from_ptr(p.as_ptr())) };

        cstr.map(f)
    }

    pub fn set_background_colour(&mut self, colour: GColor) {
        unsafe {
            bindings::text_layer_set_background_color(self.inner.as_ptr(), colour);
        }
    }

    pub fn set_text_colour(&mut self, colour: GColor) {
        unsafe {
            bindings::text_layer_set_text_color(self.inner.as_ptr(), colour);
        }
    }

    pub fn set_overflow_mode(&mut self, overflow_mode: bindings::GTextOverflowMode) {
        unsafe {
            bindings::text_layer_set_overflow_mode(self.inner.as_ptr(), overflow_mode);
        }
    }

    pub fn set_font(&mut self, font: Font) {
        unsafe {
            bindings::text_layer_set_font(self.inner.as_ptr(), font.0);
        }
    }

    pub fn set_text_alignment(&mut self, text_alignment: bindings::GTextAlignment) {
        unsafe {
            bindings::text_layer_set_text_alignment(self.inner.as_ptr(), text_alignment);
        }
    }

    pub fn enable_screen_text_flow_and_paging(&mut self, inset: u8) {
        unsafe {
            bindings::text_layer_enable_screen_text_flow_and_paging(self.inner.as_ptr(), inset);
        }
    }

    pub fn restore_default_text_flow_and_paging(&mut self) {
        unsafe {
            bindings::text_layer_restore_default_text_flow_and_paging(self.inner.as_ptr());
        }
    }

    pub fn get_content_size(&self) -> GSize {
        unsafe { bindings::text_layer_get_content_size(self.inner.as_ptr()) }
    }

    pub fn set_size(&mut self, max_size: GSize) {
        unsafe {
            bindings::text_layer_set_size(self.inner.as_ptr(), max_size);
        }
    }
}

impl<'layer> Drop for TextLayer<'layer> {
    fn drop(&mut self) {
        unsafe {
            bindings::text_layer_destroy(self.inner.as_ptr());
        }
    }
}

/// A guard that represents the lifetime of a string passed to [TextLayer::set_text].
///
/// Once dropped, the text in the text layer is set to `""` and the `'text` lifetime is freed up.
#[must_use = "Content is set back to an empty string when the returned guard is dropped"]
pub struct SetTextGuard<'text, 'layer> {
    pub(crate) layer: NonNull<bindings::TextLayer>,
    pub(crate) _phantom: PhantomData<(&'text (), &'layer ())>,
}

impl<'text, 'layer> Drop for SetTextGuard<'text, 'layer> {
    fn drop(&mut self) {
        unsafe {
            bindings::text_layer_set_text(self.layer.as_ptr(), c"".as_ptr());
        }
    }
}

impl<'a> AsChildLayer<'a> for TextLayer<'a> {
    type Parameters = GRect;

    fn new_unparented(create_params: Self::Parameters) -> Option<Self> {
        Self::new(create_params)
    }
}

impl<'a> IsLayer for TextLayer<'a> {
    fn layer(&self) -> LayerRef<'a> {
        let ptr = unsafe { bindings::text_layer_get_layer(self.inner.as_ptr()) };
        LayerRef::from_ptr(NonNull::new(ptr).unwrap())
    }

    fn layer_mut(&mut self) -> LayerMut<'a> {
        let ptr = unsafe { bindings::text_layer_get_layer(self.inner.as_ptr()) };
        LayerMut::from_ptr(NonNull::new(ptr).unwrap())
    }
}
