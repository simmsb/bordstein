use core::sync::atomic::{AtomicPtr, Ordering};

use embassy_time_driver::Driver;
use embassy_time_queue_utils::Queue;

use crate::single_core_cell::SingleCoreCell;

struct TmrDriver {
    timer_handle: AtomicPtr<crate::bindings::AppTimer>,
    queue: SingleCoreCell<Queue>,
}

embassy_time_driver::time_driver_impl!(static DRIVER: TmrDriver = TmrDriver {
    timer_handle: AtomicPtr::new(core::ptr::null_mut()),
    queue: SingleCoreCell::new(Queue::new()),
});

unsafe extern "C" fn timer_callback(_data: *mut core::ffi::c_void) {
    DRIVER.timer_handle.store(core::ptr::null_mut(), Ordering::SeqCst);
    DRIVER.trigger_alarm();

    unsafe {
        crate::executor::poll_executor();
    }
}

impl TmrDriver {
    fn set_alarm(&self, when: u64) -> bool {
        if when == u64::MAX {
            return true;
        }

        let now = self.now();
        let timeout_ms = when.saturating_sub(now).saturating_truncate::<u32>();

        if self.timer_handle.load(Ordering::SeqCst).is_null()
            || unsafe {
                !crate::bindings::app_timer_reschedule(
                    self.timer_handle.load(Ordering::SeqCst),
                    timeout_ms,
                )
            }
        {
            unsafe {
                self.timer_handle.store(
                    crate::bindings::app_timer_register(
                        timeout_ms,
                        Some(timer_callback),
                        core::ptr::null_mut(),
                    ),
                    Ordering::SeqCst,
                );
            }
        }
        if when <= self.now() {
            unsafe {
                crate::bindings::app_timer_cancel(
                    self.timer_handle
                        .swap(core::ptr::null_mut(), Ordering::SeqCst),
                );
            }

            return false;
        }

        true
    }

    fn trigger_alarm(&self) {
        unsafe {
            self.queue.with_mut(|q| {
                let mut when = q.next_expiration(self.now());

                while !self.set_alarm(when) {
                    when = q.next_expiration(self.now());
                }
            });
        }
    }
}

impl Driver for TmrDriver {
    fn now(&self) -> u64 {
        let mut secs: i32 = 0;
        let mut ms: u16 = 0;

        // probably a terrible impl as it's likely not monotonic, but I don't see anything better
        unsafe {
            crate::bindings::time_ms(&raw mut secs, &raw mut ms);
        }

        secs.saturating_cast::<u64>()
            .saturating_mul(1000)
            .saturating_add(ms as u64)
    }

    fn schedule_wake(&self, at: u64, waker: &core::task::Waker) {
        unsafe {
            self.queue.with_mut(|q| {
                if q.schedule_wake(at, waker) {
                    let mut when = q.next_expiration(self.now());

                    while !self.set_alarm(when) {
                        when = q.next_expiration(self.now());
                    }
                }
            });
        }
    }
}
