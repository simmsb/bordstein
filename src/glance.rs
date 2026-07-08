use core::{marker::PhantomData, mem::MaybeUninit, pin::pin, ptr::NonNull};

use crate::bindings;

pub struct AppGlanceReloadSession<'session> {
    inner: NonNull<bindings::AppGlanceReloadSession>,

    _phantom: PhantomData<&'session ()>,
}

impl<'session> AppGlanceReloadSession<'session> {
    pub fn add_slice(
        &mut self,
        slice: bindings::AppGlanceSlice,
    ) -> Result<(), bindings::AppGlanceResult> {
        unsafe { collect_error(bindings::app_glance_add_slice(self.inner.as_ptr(), slice)) }
    }
}

struct AppGlanceHandler<F, T> {
    callback: Option<F>,
    result: NonNull<MaybeUninit<T>>,
}

trait AppGlanceCallback {
    fn run(&mut self, limit: usize, session: AppGlanceReloadSession);
}

impl<F, T> AppGlanceCallback for AppGlanceHandler<F, T>
where
    F: for<'session> FnOnce(usize, AppGlanceReloadSession<'session>) -> T,
{
    fn run(&mut self, limit: usize, session: AppGlanceReloadSession) {
        let r = (self.callback.take().unwrap())(limit, session);
        unsafe { (*self.result.as_ptr()).write(r) };
    }
}

pub fn reload<T>(f: impl for<'session> FnOnce(usize, AppGlanceReloadSession<'session>) -> T) -> T {
    let pinned_result = pin!(MaybeUninit::uninit());
    let result = unsafe { pinned_result.get_unchecked_mut() };
    let pinned_data = pin!(AppGlanceHandler {
        callback: Some(f),
        result: NonNull::from_mut(result)
    });
    let mut dyn_ptr = unsafe { pinned_data.get_unchecked_mut() as *mut dyn AppGlanceCallback };
    let single_width_ptr = &raw mut dyn_ptr;

    unsafe {
        bindings::app_glance_reload(
            Some(app_glance_callback),
            single_width_ptr as *mut core::ffi::c_void,
        );
    }

    unsafe { result.assume_init_read() }
}

unsafe extern "C" fn app_glance_callback(
    session: *mut bindings::AppGlanceReloadSession,
    limit: usize,
    context: *mut core::ffi::c_void,
) {
    let session = AppGlanceReloadSession {
        inner: NonNull::new(session).unwrap(),
        _phantom: PhantomData,
    };

    let cb = context as *mut *mut dyn AppGlanceCallback;
    unsafe {
        let mut p0 = NonNull::new(*cb).unwrap();
        (p0.as_mut()).run(limit, session);
    }
}

fn collect_error(val: bindings::AppGlanceResult) -> Result<(), bindings::AppGlanceResult> {
    if val == bindings::AppGlanceResult::APP_GLANCE_RESULT_SUCCESS {
        Ok(())
    } else {
        Err(val)
    }
}
