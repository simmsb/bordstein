use core::{ffi::CStr, marker::PhantomData, mem::ManuallyDrop, ptr::NonNull};

use crate::{
    bindings::{
        self, GCompOp, GCornerMask, GPoint, GRect, GSize, GTextAlignment, GTextAttributes,
        GTextOverflowMode,
    },
    bitmap::GBitmap,
    colour::GColor,
    font::Font,
};

pub struct GContext<'a> {
    inner: NonNull<bindings::GContext>,

    _phantom: PhantomData<&'a ()>,
}

impl<'a> GContext<'a> {
    pub(crate) fn new(raw: NonNull<bindings::GContext>) -> Self {
        Self {
            inner: raw,
            _phantom: PhantomData,
        }
    }

    pub fn set_stroke_colour(&mut self, colour: GColor) {
        unsafe {
            bindings::graphics_context_set_stroke_color(self.inner.as_ptr(), colour);
        }
    }

    pub fn set_fill_colour(&mut self, colour: GColor) {
        unsafe {
            bindings::graphics_context_set_fill_color(self.inner.as_ptr(), colour);
        }
    }

    pub fn set_text_colour(&mut self, colour: GColor) {
        unsafe {
            bindings::graphics_context_set_text_color(self.inner.as_ptr(), colour);
        }
    }

    pub fn set_compositing_mode(&mut self, comp_mode: GCompOp) {
        unsafe {
            bindings::graphics_context_set_compositing_mode(self.inner.as_ptr(), comp_mode);
        }
    }

    pub fn set_antialiased(&mut self, enable: bool) {
        unsafe {
            bindings::graphics_context_set_antialiased(self.inner.as_ptr(), enable);
        }
    }

    pub fn set_stroke_width(&mut self, width: u8) {
        unsafe {
            bindings::graphics_context_set_stroke_width(self.inner.as_ptr(), width);
        }
    }

    pub fn draw_pixel(&mut self, point: GPoint) {
        unsafe {
            bindings::graphics_draw_pixel(self.inner.as_ptr(), point);
        }
    }

    pub fn draw_line(&mut self, p0: GPoint, p1: GPoint) {
        unsafe {
            bindings::graphics_draw_line(self.inner.as_ptr(), p0, p1);
        }
    }

    pub fn draw_rect(&mut self, rect: GRect) {
        unsafe {
            bindings::graphics_draw_rect(self.inner.as_ptr(), rect);
        }
    }

    pub fn draw_round_rect(&mut self, rect: GRect, radius: u16) {
        unsafe {
            bindings::graphics_draw_round_rect(self.inner.as_ptr(), rect, radius);
        }
    }

    pub fn fill_rect(&mut self, rect: GRect, corner_radius: u16, corner_mask: GCornerMask) {
        unsafe {
            bindings::graphics_fill_rect(self.inner.as_ptr(), rect, corner_radius, corner_mask);
        }
    }

    pub fn draw_circle(&mut self, center: GPoint, radius: u16) {
        unsafe {
            bindings::graphics_draw_circle(self.inner.as_ptr(), center, radius);
        }
    }

    pub fn fill_circle(&mut self, center: GPoint, radius: u16) {
        unsafe {
            bindings::graphics_fill_circle(self.inner.as_ptr(), center, radius);
        }
    }

    pub fn capture_frame_buffer(&mut self) -> Option<CapturedFramebuffer<'_>> {
        unsafe {
            let bitmap_ptr = bindings::graphics_capture_frame_buffer(self.inner.as_ptr());
            let bitmap = GBitmap::from_ptr(NonNull::new(bitmap_ptr)?);

            Some(CapturedFramebuffer {
                inner: ManuallyDrop::new(bitmap),
                ctx: self.inner,
            })
        }
    }

    pub fn capture_frame_buffer_format(
        &mut self,
        format: bindings::GBitmapFormat,
    ) -> Option<CapturedFramebuffer<'_>> {
        unsafe {
            let bitmap_ptr =
                bindings::graphics_capture_frame_buffer_format(self.inner.as_ptr(), format);
            let bitmap = GBitmap::from_ptr(NonNull::new(bitmap_ptr)?);

            Some(CapturedFramebuffer {
                inner: ManuallyDrop::new(bitmap),
                ctx: self.inner,
            })
        }
    }
}

pub struct CapturedFramebuffer<'capture> {
    inner: ManuallyDrop<GBitmap<'capture>>,
    ctx: NonNull<bindings::GContext>,
}

impl<'capture> Drop for CapturedFramebuffer<'capture> {
    fn drop(&mut self) {
        unsafe {
            bindings::graphics_release_frame_buffer(self.ctx.as_ptr(), self.inner.inner.as_ptr());
        }
    }
}

pub struct TextAttributes {
    inner: NonNull<GTextAttributes>,
}

impl<'context> Drop for TextAttributes {
    fn drop(&mut self) {
        unsafe {
            bindings::graphics_text_attributes_destroy(self.inner.as_ptr());
        }
    }
}

impl<'context> TextAttributes {
    pub fn enable_screen_text_flow(&mut self, inset: u8) {
        unsafe {
            bindings::graphics_text_attributes_enable_screen_text_flow(self.inner.as_ptr(), inset);
        }
    }

    pub fn enable_paging(&mut self, content_origin_on_screen: GPoint, paging_on_screen: GRect) {
        unsafe {
            bindings::graphics_text_attributes_enable_paging(
                self.inner.as_ptr(),
                content_origin_on_screen,
                paging_on_screen,
            );
        }
    }

    pub fn restore_default_text_flow(&mut self) {
        unsafe {
            bindings::graphics_text_attributes_restore_default_text_flow(self.inner.as_ptr());
        }
    }

    pub fn restore_default_paging(&mut self) {
        unsafe {
            bindings::graphics_text_attributes_restore_default_paging(self.inner.as_ptr());
        }
    }
}

impl<'a> GContext<'a> {
    pub fn text_attributes(&self) -> TextAttributes {
        let ptr = unsafe { bindings::graphics_text_attributes_create() };

        TextAttributes {
            inner: NonNull::new(ptr).unwrap(),
        }
    }

    pub fn draw_text(
        &mut self,
        text: &CStr,
        font: Font,
        bounding_box: GRect,
        overflow_mode: GTextOverflowMode,
        alignment: GTextAlignment,
        attributes: Option<&TextAttributes>,
    ) {
        unsafe {
            bindings::graphics_draw_text(
                self.inner.as_ptr(),
                text.as_ptr(),
                font.0,
                bounding_box,
                overflow_mode,
                alignment,
                attributes
                    .map(|a| a.inner.as_ptr())
                    .unwrap_or(core::ptr::null_mut()),
            )
        }
    }

    pub fn get_text_content_size_with_attributes(
        text: &CStr,
        font: Font,
        bounding_box: GRect,
        overflow_mode: GTextOverflowMode,
        alignment: GTextAlignment,
        attributes: &TextAttributes,
    ) -> GSize {
        unsafe {
            bindings::graphics_text_layout_get_content_size_with_attributes(
                text.as_ptr(),
                font.0,
                bounding_box,
                overflow_mode,
                alignment,
                attributes.inner.as_ptr(),
            )
        }
    }

    pub fn get_text_content_size(
        text: &CStr,
        font: Font,
        bounding_box: GRect,
        overflow_mode: GTextOverflowMode,
        alignment: GTextAlignment,
    ) -> GSize {
        unsafe {
            bindings::graphics_text_layout_get_content_size(
                text.as_ptr(),
                font.0,
                bounding_box,
                overflow_mode,
                alignment,
            )
        }
    }
}
