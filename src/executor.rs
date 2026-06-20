use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::{Spawner, raw};

use crate::single_core_cell::SingleCoreCell;

static EXECUTOR: SingleCoreCell<Option<Executor>> = SingleCoreCell::new(None);
static SIGNAL_WORK_THREAD_MODE: AtomicBool = AtomicBool::new(false);

#[unsafe(export_name = "__pender")]
fn __pender(_context: *mut ()) {
    SIGNAL_WORK_THREAD_MODE.store(true, Ordering::SeqCst);
}

pub struct Executor {
    inner: embassy_executor::raw::Executor,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            inner: embassy_executor::raw::Executor::new(core::ptr::null_mut()),
        }
    }

    pub fn spawner(&'static mut self) -> Spawner {
        self.inner.spawner()
    }

    unsafe fn poll(&'static mut self) {
        loop {
            unsafe { self.inner.poll() };

            if !SIGNAL_WORK_THREAD_MODE.swap(false, Ordering::SeqCst) {
                continue;
            }
        }
    }

    pub fn run() {
        // we don't actually run the executor, instead any registered callbacks will poll the executor for us.
        unsafe {
            crate::bindings::app_event_loop();
        }
    }
}

pub fn init() {
    EXECUTOR.with_mut(|e| *e = Some(Executor::new()));
}

pub fn run(init: impl FnOnce(Spawner)) {
    unsafe {
        {
            EXECUTOR.with_mut(|e| {
                let s = make_static(e.as_mut().unwrap());
                init(s.spawner());
            });
        }

        Executor::run();
    }
}

unsafe fn make_static<T>(t: &mut T) -> &'static mut T {
    unsafe { ::core::mem::transmute(t) }
}

pub unsafe fn poll_executor() {
    unsafe {
        EXECUTOR.with_mut(|e| {
            let s = make_static(e.as_mut().unwrap());
            make_static(s).poll();
        });
    }
}
