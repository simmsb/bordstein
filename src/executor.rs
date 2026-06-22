use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;

use crate::single_core_cell::SingleCoreCell;

static EXECUTOR: SingleCoreCell<Option<Executor>> = SingleCoreCell::new(None);
static SIGNAL_WORK_THREAD_MODE: AtomicBool = AtomicBool::new(false);
static EXECUTOR_IN_POLL: AtomicBool = AtomicBool::new(false);

#[unsafe(export_name = "__pender")]
fn __pender(_context: *mut ()) {
    SIGNAL_WORK_THREAD_MODE.store(true, Ordering::SeqCst);
}

pub struct Executor {
    inner: embassy_executor::raw::Executor,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
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
        if EXECUTOR_IN_POLL.swap(true, Ordering::SeqCst) {
            panic!("Executor polled recursively");
        }

        loop {
            unsafe { self.inner.poll() };

            if !SIGNAL_WORK_THREAD_MODE.swap(false, Ordering::SeqCst) {
                break;
            }
        }

        EXECUTOR_IN_POLL.store(false, Ordering::SeqCst);
    }

    pub fn run() {
        // do one poll to kick things off
        unsafe {
            poll_executor();
        }
        // we don't actually run the executor, instead any registered callbacks will poll the executor for us.
        unsafe {
            crate::bindings::app_event_loop();
        }
    }
}

pub fn init() {
    unsafe { EXECUTOR.with_mut(|e| *e = Some(Executor::new())) };
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

// unsafe, caller must ensure that this is not called re-entrantly
#[inline(never)]
pub unsafe fn poll_executor() {
    if EXECUTOR_IN_POLL.load(Ordering::SeqCst) {
        SIGNAL_WORK_THREAD_MODE.store(true, Ordering::SeqCst);
    } else {
        unsafe {
            EXECUTOR.with_mut(|e| {
                crate::trace!("Executor poll, addr: {:?}", e as *mut _);
                let Some(e) = e.as_mut() else { return };
                let s: &mut Executor = make_static(e);
                make_static(s).poll();
            });
        }
    }
}

// pub fn waker_as_ptr(waker: &Waker) -> NonNull<core::ffi::c_void> {
//     // TaskRef doesn't expose as_ptr/from_ptr publicly, so we have to be evil.
//     //
//     // This is certainly a bomb waiting to go off, TaskRef isn't even
//     // #[repr(transparent)]
//     let task_ref = task_from_waker(waker);

//     unsafe { core::mem::transmute(task_ref) }
// }

// pub fn wake_from_ptr(ptr: NonNull<core::ffi::c_void>) {
//     // TaskRef doesn't expose as_ptr/from_ptr publicly, so we have to be evil.
//     //
//     // This is certainly a bomb waiting to go off, TaskRef isn't even
//     // #[repr(transparent)]
//     let task_ref: TaskRef = unsafe { core::mem::transmute(ptr) };
//     wake_task(task_ref);
// }
