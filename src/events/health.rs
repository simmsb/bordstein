use core::{cell::Cell, ffi::c_void, marker::PhantomPinned, pin::Pin, ptr::NonNull, task::Poll};

use cordyceps::{Linked, List, list::Links};
use embassy_sync::waitqueue::AtomicWaker;
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::{
    bindings::{self, HealthEventType},
    single_core_cell::SingleCoreCell,
    time::Timestamp,
};

pub fn health_service_sum(
    metric: bindings::HealthMetric,
    start: Timestamp,
    end: Timestamp,
) -> bindings::HealthValue {
    unsafe { bindings::health_service_sum(metric, start.0, end.0) }
}

pub fn health_service_today(metric: bindings::HealthMetric) -> bindings::HealthValue {
    unsafe { bindings::health_service_sum_today(metric) }
}

pub fn health_service_peek_current_value(metric: bindings::HealthMetric) -> bindings::HealthValue {
    unsafe { bindings::health_service_peek_current_value(metric) }
}

struct HealthServiceEntry {
    links: Links<HealthServiceEntry>,

    callback: *mut HealthServiceHandlerVTable,
}

unsafe impl Linked<Links<HealthServiceEntry>> for HealthServiceEntry {
    type Handle = NonNull<HealthServiceEntry>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(
        ptr: core::ptr::NonNull<Self>,
    ) -> core::ptr::NonNull<Links<HealthServiceEntry>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = core::ptr::addr_of_mut!((*target).links);

            NonNull::new_unchecked(links)
        }
    }
}

pub trait HealthServiceHandler<'env> = FnMut(HealthEventType) + 'env;

pub(crate) type HealthServiceHandlerVTable = dyn HealthServiceHandler<'static>;

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Callback is deregistered and dropped when [HealthServiceHandle] is dropped"]
#[pin_data(PinnedDrop)]
pub struct HealthServiceHandle<F> {
    #[pin]
    callback: F,

    entry: HealthServiceEntry,

    #[pin]
    _pin_phantom: PhantomPinned,
}

static LIST: SingleCoreCell<List<HealthServiceEntry>> = SingleCoreCell::new(List::new());

/// Listen to health events
///
/// When the returned [HealthServiceHandle] is dropped, the callback will be
/// deregistered and the closure dropped.
///
/// NOTE: You can create multiple health event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [HealthServiceHandle] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [HealthServiceHandle] is dropped"]
pub fn listen<F>(callback: F) -> impl PinInit<HealthServiceHandle<F>>
where
    F: FnMut(HealthEventType),
{
    pin_init::pin_init!(&this in HealthServiceHandle {
        callback,

        entry: HealthServiceEntry {
            links: Links::default(),
            callback: unsafe { core::mem::transmute::<_, *mut HealthServiceHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn HealthServiceHandler<'_>) },
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

type HealthServiceStreamHandler = impl FnMut(HealthEventType);

#[pin_data]
pub struct HealthServiceStream {
    #[pin]
    handle: HealthServiceHandle<HealthServiceStreamHandler>,

    waker: AtomicWaker,

    value: Cell<Option<HealthEventType>>,
}

impl futures::Stream for HealthServiceStream {
    type Item = HealthEventType;

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

#[define_opaque(HealthServiceStreamHandler)]
fn stream_closure(ptr: NonNull<HealthServiceStream>) -> HealthServiceStreamHandler {
    move |evt| unsafe {
        let waker_ptr = &raw mut (*ptr.as_ptr()).waker;
        let value_ptr = &raw mut (*ptr.as_ptr()).value;
        (*value_ptr).set(Some(evt));
        (*waker_ptr).wake();
    }
}

/// Similar to [listen], this returns a [futures::Stream] of [HealthEventType].
///
/// NOTE: You can create multiple health event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [HealthServiceStream] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [HealthServiceStream] is dropped"]
pub fn stream() -> impl PinInit<HealthServiceStream> {
    pin_init::pin_init!(&this in HealthServiceStream {
        handle <- listen(stream_closure(this)),
        waker: AtomicWaker::new(),
        value: Cell::new(None),
    })
}

unsafe fn re_register_callback(list: &mut List<HealthServiceEntry>) {
    if list.is_empty() {
        unsafe {
            bindings::health_service_events_unsubscribe();

            return;
        }
    }

    unsafe {
        bindings::health_service_events_subscribe(
            Some(health_service_callback),
            core::ptr::null_mut(),
        );
    }
}

#[pinned_drop]
impl<F> PinnedDrop for HealthServiceHandle<F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            LIST.with_mut(|l| {
                l.remove(NonNull::from_mut(self.project().entry));

                re_register_callback(l);
            });
        }
    }
}

unsafe extern "C" fn health_service_callback(event: HealthEventType, _context: *mut c_void) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.callback)(event);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
