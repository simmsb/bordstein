use core::{marker::PhantomData, ptr::NonNull};

use crate::{
    bindings::{self, GAlign, GColor, GCompOp, GRect},
    bitmap::GBitmap,
};

use super::{AsChildLayer, IsLayer, LayerMut, LayerRef};

pub struct BitmapLayer<'layer> {
    pub(crate) inner: NonNull<bindings::BitmapLayer>,

    pub(crate) _phantom: PhantomData<&'layer ()>,
}

impl<'layer> BitmapLayer<'layer> {
    pub(crate) fn new(frame: GRect) -> Option<Self> {
        let ptr = unsafe { bindings::bitmap_layer_create(frame) };
        NonNull::new(ptr).map(Self::from_ptr)
    }

    pub(crate) fn from_ptr(ptr: NonNull<bindings::BitmapLayer>) -> Self {
        Self {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    /// Set contents of this bitmap layer. The layer doesn't copy the bitmap so the
    /// lifetime of the bitmap must be greater than or equal to the layer.
    #[must_use = "Content is set back to none when the returned guard is dropped"]
    pub fn set_bitmap<'bitmap, 'a>(
        &'a mut self,
        bitmap: &'bitmap GBitmap,
    ) -> SetBitmapGuard<'bitmap, 'a> {
        unsafe {
            bindings::bitmap_layer_set_bitmap(self.inner.as_ptr(), bitmap.inner.as_ptr());
        }

        SetBitmapGuard {
            layer: self.inner,
            _phantom: PhantomData,
        }
    }

    pub fn set_alignment(&mut self, alignment: GAlign) {
        unsafe {
            bindings::bitmap_layer_set_alignment(self.inner.as_ptr(), alignment);
        }
    }

    pub fn set_background_colour(&mut self, background_colour: GColor) {
        unsafe {
            bindings::bitmap_layer_set_background_color(self.inner.as_ptr(), background_colour);
        }
    }

    pub fn set_compositing_mode(&mut self, compositing_mode: GCompOp) {
        unsafe {
            bindings::bitmap_layer_set_compositing_mode(self.inner.as_ptr(), compositing_mode);
        }
    }
}

impl<'layer> Drop for BitmapLayer<'layer> {
    fn drop(&mut self) {
        unsafe {
            bindings::bitmap_layer_destroy(self.inner.as_ptr());
        }
    }
}

/// A guard that represents the lifetime of a string passed to [BitmapLayer::set_bitmap].
///
/// Once dropped, the bitmap in the bitmap layer is set to `""` and the `'bitmap` lifetime is freed up.
#[must_use = "Content is set back to an empty string when the returned guard is dropped"]
pub struct SetBitmapGuard<'bitmap, 'layer> {
    pub(crate) layer: NonNull<bindings::BitmapLayer>,
    pub(crate) _phantom: PhantomData<(&'bitmap (), &'layer ())>,
}

impl<'bitmap, 'layer> Drop for SetBitmapGuard<'bitmap, 'layer> {
    fn drop(&mut self) {
        unsafe {
            bindings::bitmap_layer_set_bitmap(self.layer.as_ptr(), core::ptr::null_mut());
        }
    }
}

impl<'a> AsChildLayer<'a> for BitmapLayer<'a> {
    type Parameters = GRect;

    fn new_unparented(create_params: Self::Parameters) -> Option<Self> {
        Self::new(create_params)
    }
}

impl<'a> IsLayer for BitmapLayer<'a> {
    fn layer(&self) -> LayerRef<'a> {
        let ptr = unsafe { bindings::bitmap_layer_get_layer(self.inner.as_ptr()) };
        LayerRef::from_ptr(NonNull::new(ptr).unwrap())
    }

    fn layer_mut(&mut self) -> LayerMut<'a> {
        let ptr = unsafe { bindings::bitmap_layer_get_layer(self.inner.as_ptr()) };
        LayerMut::from_ptr(NonNull::new(ptr).unwrap())
    }
}
