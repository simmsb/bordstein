use core::ptr::NonNull;

use crate::bindings;

pub struct Resource(pub(crate) NonNull<core::ffi::c_void>);

impl Resource {
    pub fn get_handle(resource_id: u32) -> Option<Self> {
        let ptr = unsafe { bindings::resource_get_handle(resource_id) };
        NonNull::new(ptr).map(Resource)
    }

    pub fn load(self, buf: &mut [u8]) -> usize {
        unsafe { bindings::resource_load(self.0.as_ptr(), buf.as_mut_ptr(), buf.len()) }
    }

    pub fn load_range(self, offset: u32, buf: &mut [u8]) -> usize {
        unsafe {
            bindings::resource_load_byte_range(self.0.as_ptr(), offset, buf.as_mut_ptr(), buf.len())
        }
    }
}
