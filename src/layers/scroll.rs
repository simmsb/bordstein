use core::{marker::PhantomData, ptr::NonNull};

use crate::{
    bindings::{self, GPoint, GRect, GSize},
    window::WindowHandle,
};

use super::{AsChildLayer, IsLayer, LayerMut, LayerRef};

pub struct ScrollLayer<'layer> {
    pub(crate) inner: NonNull<bindings::ScrollLayer>,

    pub(crate) _phantom: PhantomData<&'layer ()>,
}

impl<'layer> ScrollLayer<'layer> {
    pub(crate) fn new(frame: GRect) -> Option<Self> {
        let ptr = unsafe { bindings::scroll_layer_create(frame) };
        NonNull::new(ptr).map(Self::from_ptr)
    }

    pub(crate) fn from_ptr(ptr: NonNull<bindings::ScrollLayer>) -> Self {
        Self {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    pub fn new_child<'child: 'layer, LayerT>(
        &self,
        create_params: LayerT::Parameters,
    ) -> Option<LayerT>
    where
        LayerT: AsChildLayer<'child>,
    {
        let child = LayerT::new_unparented(create_params)?;
        unsafe {
            bindings::scroll_layer_add_child(
                self.inner.as_ptr(),
                child.layer().inner.inner.as_ptr(),
            );
        }
        Some(child)
    }

    pub fn set_click_config_onto_window(&mut self, window: &mut WindowHandle) {
        unsafe {
            bindings::scroll_layer_set_click_config_onto_window(
                self.inner.as_ptr(),
                window.inner.as_ptr(),
            );
        }
    }

    pub fn set_content_offset(&mut self, offset: GPoint, animate: bool) {
        unsafe {
            bindings::scroll_layer_set_content_offset(self.inner.as_ptr(), offset, animate);
        }
    }

    pub fn get_content_offset(&self) -> GPoint {
        unsafe { bindings::scroll_layer_get_content_offset(self.inner.as_ptr()) }
    }

    pub fn set_content_size(&mut self, size: GSize) {
        unsafe {
            bindings::scroll_layer_set_content_size(self.inner.as_ptr(), size);
        }
    }

    pub fn get_content_size(&self) -> GSize {
        unsafe { bindings::scroll_layer_get_content_size(self.inner.as_ptr()) }
    }

    pub fn set_frame(&mut self, frame: GRect) {
        unsafe {
            bindings::scroll_layer_set_frame(self.inner.as_ptr(), frame);
        }
    }
}

impl<'a> Drop for ScrollLayer<'a> {
    fn drop(&mut self) {
        unsafe {
            bindings::scroll_layer_destroy(self.inner.as_ptr());
        }
    }
}

impl<'a> AsChildLayer<'a> for ScrollLayer<'a> {
    type Parameters = GRect;

    fn new_unparented(create_params: Self::Parameters) -> Option<Self> {
        Self::new(create_params)
    }
}

impl<'a> IsLayer for ScrollLayer<'a> {
    fn layer(&self) -> LayerRef<'a> {
        let ptr = unsafe { bindings::scroll_layer_get_layer(self.inner.as_ptr()) };
        LayerRef::from_ptr(NonNull::new(ptr).unwrap())
    }

    fn layer_mut(&mut self) -> LayerMut<'a> {
        let ptr = unsafe { bindings::scroll_layer_get_layer(self.inner.as_ptr()) };
        LayerMut::from_ptr(NonNull::new(ptr).unwrap())
    }
}
