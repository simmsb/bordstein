use core::cell::UnsafeCell;

pub struct SingleCoreCell<T> {
    value: UnsafeCell<T>,
}

impl<T> SingleCoreCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    // pub fn get<'a>(&'a self) -> Ref<'a, T> {
    //     self.value.get()
    // }

    pub unsafe fn with_mut<'a>(&'a self, cb: impl FnOnce(&'a mut T)) {
        unsafe { cb(self.value.get().as_mut_unchecked()) }
    }
}

// pebble apps are single threaded and non-reentrant I hope?
unsafe impl<T> Send for SingleCoreCell<T> {}
unsafe impl<T> Sync for SingleCoreCell<T> {}
