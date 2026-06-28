use core::{marker::PhantomData, ptr::NonNull};

use crate::{bindings, colour::GColor};

use super::{AsChildLayer, IsLayer, LayerMut, LayerRef};

pub struct StatusBarLayer<'layer> {
    pub(crate) inner: NonNull<bindings::StatusBarLayer>,

    pub(crate) _phantom: PhantomData<&'layer ()>,
}

impl<'layer> StatusBarLayer<'layer> {
    pub(crate) fn new() -> Option<Self> {
        let ptr = unsafe { bindings::status_bar_layer_create() };
        NonNull::new(ptr).map(Self::from_ptr)
    }

    pub(crate) fn from_ptr(ptr: NonNull<bindings::StatusBarLayer>) -> Self {
        Self {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    pub fn get_background_colour(&self) -> GColor {
        unsafe { bindings::status_bar_layer_get_background_color(self.inner.as_ptr()) }
    }

    pub fn get_foreground_colour(&self) -> GColor {
        unsafe { bindings::status_bar_layer_get_foreground_color(self.inner.as_ptr()) }
    }

    pub fn set_colours(&mut self, background: GColor, foreground: GColor) {
        unsafe {
            bindings::status_bar_layer_set_colors(self.inner.as_ptr(), background, foreground);
        }
    }

    pub fn set_separator_mode(&mut self, mode: bindings::StatusBarLayerSeparatorMode) {
        unsafe {
            bindings::status_bar_layer_set_separator_mode(self.inner.as_ptr(), mode);
        }
    }
}

impl<'layer> Drop for StatusBarLayer<'layer> {
    fn drop(&mut self) {
        unsafe {
            bindings::status_bar_layer_destroy(self.inner.as_ptr());
        }
    }
}

impl<'layer> AsChildLayer<'layer> for StatusBarLayer<'layer> {
    type Parameters = ();

    fn new_unparented(_create_params: Self::Parameters) -> Option<Self> {
        Self::new()
    }
}

impl<'layer> IsLayer for StatusBarLayer<'layer> {
    fn layer<'a>(&'a self) -> super::LayerRef<'a> {
        let ptr = unsafe { bindings::status_bar_layer_get_layer(self.inner.as_ptr()) };
        LayerRef::from_ptr(NonNull::new(ptr).unwrap())
    }

    fn layer_mut<'a>(&'a mut self) -> super::LayerMut<'a> {
        let ptr = unsafe { bindings::status_bar_layer_get_layer(self.inner.as_ptr()) };
        LayerMut::from_ptr(NonNull::new(ptr).unwrap())
    }
}
