use core::{marker::PhantomData, ptr::NonNull};

use crate::{
    bindings::{self, GBitmapFormat, GRect, GSize},
    colour::GColor,
    resources::ResourceId,
};

pub struct GBitmap<'bitmap> {
    pub(crate) inner: NonNull<bindings::GBitmap>,

    _phantom: PhantomData<&'bitmap ()>,
}

impl<'a> GBitmap<'a> {
    pub(crate) fn from_ptr(ptr: NonNull<bindings::GBitmap>) -> Self {
        Self {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    pub fn from_resource(resource_id: ResourceId) -> Option<GBitmap<'static>> {
        let ptr = unsafe { bindings::gbitmap_create_with_resource(resource_id) };
        NonNull::new(ptr).map(GBitmap::from_ptr)
    }

    pub unsafe fn from_data<'data>(data: &'data [u8]) -> Option<GBitmap<'data>> {
        let ptr = unsafe { bindings::gbitmap_create_with_data(data.as_ptr()) };
        NonNull::new(ptr).map(GBitmap::from_ptr)
    }

    pub fn sub(&self, sub_rect: GRect) -> Option<GBitmap<'_>> {
        let ptr = unsafe { bindings::gbitmap_create_as_sub_bitmap(self.inner.as_ptr(), sub_rect) };
        NonNull::new(ptr).map(GBitmap::from_ptr)
    }

    pub fn from_png_data(data: &[u8]) -> Option<GBitmap<'static>> {
        let ptr = unsafe { bindings::gbitmap_create_from_png_data(data.as_ptr(), data.len()) };
        NonNull::new(ptr).map(GBitmap::from_ptr)
    }

    pub fn blank(size: GSize, format: GBitmapFormat) -> Option<GBitmap<'static>> {
        let ptr = unsafe { bindings::gbitmap_create_blank(size, format) };
        NonNull::new(ptr).map(GBitmap::from_ptr)
    }

    pub fn blank_with_palette(
        size: GSize,
        format: GBitmapFormat,
        palette: &[GColor],
    ) -> Option<GBitmap<'_>> {
        let ptr = unsafe {
            bindings::gbitmap_create_blank_with_palette(
                size,
                format,
                palette.as_ptr() as *mut _,
                false,
            )
        };
        NonNull::new(ptr).map(GBitmap::from_ptr)
    }
}

impl<'a> Drop for GBitmap<'a> {
    fn drop(&mut self) {
        unsafe {
            bindings::gbitmap_destroy(self.inner.as_ptr());
        }
    }
}
