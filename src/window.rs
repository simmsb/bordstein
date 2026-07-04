use core::{cell::Cell, future::poll_fn, marker::PhantomData, ptr::NonNull, task::Poll};

use embassy_executor::raw::TaskRef;

use crate::{
    bindings::{self, GColor, WindowHandlers},
    layers::LayerRef,
};

pub struct WindowHandle<'active> {
    pub(crate) inner: NonNull<bindings::Window>,
    _phantom: PhantomData<&'active ()>,
}

impl<'active> WindowHandle<'active> {
    pub fn set_background_colour(&mut self, colour: GColor) {
        unsafe {
            bindings::window_set_background_color(self.inner.as_ptr(), colour);
        }
    }

    pub fn root_layer(&mut self) -> LayerRef<'active> {
        let ptr = unsafe { bindings::window_get_root_layer(self.inner.as_ptr()) };

        LayerRef::from_ptr(NonNull::new(ptr).unwrap())
    }

    // TODO: click event subscriptions.
    // It should be possible to offer a `WindowHandle::subscribe_to_single_click(button_id) -> impl Stream<Item = Click>` api.
    //
    // TODO: window appear/ disappear handlers/ event streams
}

struct WindowInfo {
    waker: TaskRef,
    done: Cell<bool>,
}

unsafe extern "C" fn window_handler_wake(window: *mut bindings::Window) {
    let ptr = unsafe { bindings::window_get_user_data(window).cast::<WindowInfo>() };
    if let Some(window_info) = NonNull::new(ptr) {
        unsafe {
            window_info.as_ref().done.set(true);
            embassy_executor::raw::wake_task(window_info.as_ref().waker);
        }
    }

    unsafe {
        crate::executor::poll_executor();
    }
}

unsafe extern "C" fn window_handler_noop(_window: *mut bindings::Window) {
    unsafe {
        crate::executor::poll_executor();
    }
}

/// Create a window, your passed async function will be called once with the
/// handle to the window handle, and will then be polled while the window is
/// active. When the window unloads the future will be dropped.
pub async fn with_window(f: impl for<'active> AsyncFnOnce(WindowHandle<'active>)) -> Option<()> {
    let p = unsafe { bindings::window_create() };
    let p = NonNull::new(p)?;
    let fut = f(WindowHandle {
        inner: p,
        _phantom: PhantomData,
    });

    // probably fine?
    let task_ref =
        poll_fn(|cx| Poll::Ready(embassy_executor::raw::task_from_waker(cx.waker()))).await;

    pin_init::stack_pin_init!(let window_info = WindowInfo {
        waker: task_ref,
        done: Cell::new(false),
    });

    let mut has_started: bool = false;

    // wait for window to start
    poll_fn(|_cx| unsafe {
        if !has_started {
            bindings::window_set_user_data(
                p.as_ptr(),
                window_info.as_ref().get_ref() as *const WindowInfo as *mut core::ffi::c_void,
            );

            bindings::window_set_window_handlers(
                p.as_ptr(),
                WindowHandlers {
                    load: Some(window_handler_wake),
                    appear: Some(window_handler_noop),
                    disappear: Some(window_handler_noop),
                    unload: Some(window_handler_noop),
                },
            );

            bindings::window_stack_push(p.as_ptr(), true);

            has_started = true;

            Poll::Pending
        } else if window_info.done.get() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;

    pin_init::stack_pin_init!(let window_info = WindowInfo {
        waker: task_ref,
        done: Cell::new(false),
    });

    let mut has_started: bool = false;
    let wait_for_stop = poll_fn(|_cx| unsafe {
        if !has_started {
            bindings::window_set_user_data(
                p.as_ptr(),
                window_info.as_ref().get_ref() as *const WindowInfo as *mut core::ffi::c_void,
            );

            bindings::window_set_window_handlers(
                p.as_ptr(),
                WindowHandlers {
                    load: Some(window_handler_noop),
                    appear: Some(window_handler_noop),
                    disappear: Some(window_handler_noop),
                    unload: Some(window_handler_wake),
                },
            );

            has_started = true;

            Poll::Pending
        } else if window_info.done.get() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    });

    embassy_futures::select::select(wait_for_stop, async {
        fut.await;
        unsafe {
            if bindings::window_stack_get_top_window() == p.as_ptr() {
                bindings::window_stack_pop(true);
            }
        }
    })
    .await;

    unsafe {
        bindings::window_destroy(p.as_ptr());
    }

    Some(())
}
