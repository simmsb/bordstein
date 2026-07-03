use core::{cell::Cell, marker::PhantomPinned, pin::Pin, ptr::NonNull, task::Poll};

use cordyceps::{Linked, List, list::Links};
use embassy_sync::waitqueue::AtomicWaker;
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::{
    bindings::{self},
    single_core_cell::SingleCoreCell,
};

pub fn peek_pebble_app_connection() -> bool {
    unsafe { bindings::connection_service_peek_pebble_app_connection() }
}

struct ConnectionServiceEntry {
    links: Links<ConnectionServiceEntry>,

    callback: *mut ConnectionServiceHandlerVTable,
}

unsafe impl Linked<Links<ConnectionServiceEntry>> for ConnectionServiceEntry {
    type Handle = NonNull<ConnectionServiceEntry>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(
        ptr: core::ptr::NonNull<Self>,
    ) -> core::ptr::NonNull<Links<ConnectionServiceEntry>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = core::ptr::addr_of_mut!((*target).links);

            NonNull::new_unchecked(links)
        }
    }
}

pub trait ConnectionServiceHandler<'env> = FnMut(bool) + 'env;

pub(crate) type ConnectionServiceHandlerVTable = dyn ConnectionServiceHandler<'static>;

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Callback is deregistered and dropped when [ConnectionServiceHandle] is dropped"]
#[pin_data(PinnedDrop)]
pub struct ConnectionServiceHandle<F> {
    #[pin]
    callback: F,

    entry: ConnectionServiceEntry,

    #[pin]
    _pin_phantom: PhantomPinned,
}

static LIST: SingleCoreCell<List<ConnectionServiceEntry>> = SingleCoreCell::new(List::new());

/// Listen to connection events
///
/// When the returned [ConnectionServiceHandle] is dropped, the callback will be
/// deregistered and the closure dropped.
///
/// NOTE: You can create multiple connection event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [ConnectionServiceHandle] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [ConnectionServiceHandle] is dropped"]
pub fn listen<F>(callback: F) -> impl PinInit<ConnectionServiceHandle<F>>
where
    F: FnMut(bool),
{
    pin_init::pin_init!(&this in ConnectionServiceHandle {
        callback,

        entry: ConnectionServiceEntry {
            links: Links::default(),
            callback: unsafe { core::mem::transmute::<_, *mut ConnectionServiceHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn ConnectionServiceHandler<'_>) },
        },

        _pin_phantom: PhantomPinned,
    }).pin_chain(|p| {
        let project = p.project();

        unsafe {
            LIST.with_mut(|l| {
                l.push_front(NonNull::from_mut(project.entry));

                re_register_callback(l);
            });
        }

        Ok(())
    })
}

type ConnectionServiceStreamHandler = impl FnMut(bool);

#[pin_data]
pub struct ConnectionServiceStream {
    #[pin]
    handle: ConnectionServiceHandle<ConnectionServiceStreamHandler>,

    waker: AtomicWaker,

    value: Cell<Option<bool>>,
}

impl futures::Stream for ConnectionServiceStream {
    type Item = bool;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let value = self.value.take();

        if let Some(value) = value {
            Poll::Ready(Some(value))
        } else {
            self.waker.register(cx.waker());
            Poll::Pending
        }
    }
}

#[define_opaque(ConnectionServiceStreamHandler)]
fn stream_closure(ptr: NonNull<ConnectionServiceStream>) -> ConnectionServiceStreamHandler {
    move |evt| unsafe {
        let waker_ptr = &raw mut (*ptr.as_ptr()).waker;
        let value_ptr = &raw mut (*ptr.as_ptr()).value;
        (*value_ptr).set(Some(evt));
        (*waker_ptr).wake();
    }
}

/// Similar to [listen], this returns a [futures::Stream] of [bool].
///
/// NOTE: You can create multiple connection event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [ConnectionServiceStream] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [ConnectionServiceStream] is dropped"]
pub fn stream() -> impl PinInit<ConnectionServiceStream> {
    pin_init::pin_init!(&this in ConnectionServiceStream {
        handle <- listen(stream_closure(this)),
        waker: AtomicWaker::new(),
        value: Cell::new(None),
    })
}

unsafe fn re_register_callback(list: &mut List<ConnectionServiceEntry>) {
    if list.is_empty() {
        unsafe {
            bindings::connection_service_unsubscribe();

            return;
        }
    }

    unsafe {
        bindings::connection_service_subscribe(bindings::ConnectionHandlers {
            // ignore for now, might make this into App/Pebblekit
            // connected/disconnected events if needed.
            pebblekit_connection_handler: None,
            pebble_app_connection_handler: Some(connection_service_callback),
        });
    }
}

#[pinned_drop]
impl<F> PinnedDrop for ConnectionServiceHandle<F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            LIST.with_mut(|l| {
                l.remove(NonNull::from_mut(self.project().entry));

                re_register_callback(l);
            });
        }
    }
}

unsafe extern "C" fn connection_service_callback(connected: bool) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.callback)(connected);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
