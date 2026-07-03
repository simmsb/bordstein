use core::{cell::Cell, marker::PhantomPinned, pin::Pin, ptr::NonNull, task::Poll};

use cordyceps::{Linked, List, list::Links};
use embassy_sync::waitqueue::AtomicWaker;
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::{
    bindings::{self, BatteryChargeState},
    single_core_cell::SingleCoreCell,
};

pub fn battery_service_peek_current_value() -> BatteryChargeState {
    unsafe { bindings::battery_state_service_peek() }
}

struct BatteryServiceEntry {
    links: Links<BatteryServiceEntry>,

    callback: *mut BatteryServiceHandlerVTable,
}

unsafe impl Linked<Links<BatteryServiceEntry>> for BatteryServiceEntry {
    type Handle = NonNull<BatteryServiceEntry>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(
        ptr: core::ptr::NonNull<Self>,
    ) -> core::ptr::NonNull<Links<BatteryServiceEntry>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = core::ptr::addr_of_mut!((*target).links);

            NonNull::new_unchecked(links)
        }
    }
}

pub trait BatteryServiceHandler<'env> = FnMut(BatteryChargeState) + 'env;

pub(crate) type BatteryServiceHandlerVTable = dyn BatteryServiceHandler<'static>;

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Callback is deregistered and dropped when [BatteryServiceHandle] is dropped"]
#[pin_data(PinnedDrop)]
pub struct BatteryServiceHandle<F> {
    #[pin]
    callback: F,

    entry: BatteryServiceEntry,

    #[pin]
    _pin_phantom: PhantomPinned,
}

static LIST: SingleCoreCell<List<BatteryServiceEntry>> = SingleCoreCell::new(List::new());

/// Listen to battery events
///
/// When the returned [BatteryServiceHandle] is dropped, the callback will be
/// deregistered and the closure dropped.
///
/// NOTE: You can create multiple battery event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [BatteryServiceHandle] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [BatteryServiceHandle] is dropped"]
pub fn listen<F>(callback: F) -> impl PinInit<BatteryServiceHandle<F>>
where
    F: FnMut(BatteryChargeState),
{
    pin_init::pin_init!(&this in BatteryServiceHandle {
        callback,

        entry: BatteryServiceEntry {
            links: Links::default(),
            callback: unsafe { core::mem::transmute::<_, *mut BatteryServiceHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn BatteryServiceHandler<'_>) },
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

type BatteryServiceStreamHandler = impl FnMut(BatteryChargeState);

#[pin_data]
pub struct BatteryServiceStream {
    #[pin]
    handle: BatteryServiceHandle<BatteryServiceStreamHandler>,

    waker: AtomicWaker,

    value: Cell<Option<BatteryChargeState>>,
}

impl futures::Stream for BatteryServiceStream {
    type Item = BatteryChargeState;

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

#[define_opaque(BatteryServiceStreamHandler)]
fn stream_closure(ptr: NonNull<BatteryServiceStream>) -> BatteryServiceStreamHandler {
    move |evt| unsafe {
        let waker_ptr = &raw mut (*ptr.as_ptr()).waker;
        let value_ptr = &raw mut (*ptr.as_ptr()).value;
        (*value_ptr).set(Some(evt));
        (*waker_ptr).wake();
    }
}

/// Similar to [listen], this returns a [futures::Stream] of [BatteryChargeState].
///
/// NOTE: You can create multiple battery event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [BatteryServiceStream] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [BatteryServiceStream] is dropped"]
pub fn stream() -> impl PinInit<BatteryServiceStream> {
    pin_init::pin_init!(&this in BatteryServiceStream {
        handle <- listen(stream_closure(this)),
        waker: AtomicWaker::new(),
        value: Cell::new(None),
    })
}

unsafe fn re_register_callback(list: &mut List<BatteryServiceEntry>) {
    if list.is_empty() {
        unsafe {
            bindings::battery_state_service_unsubscribe();

            return;
        }
    }

    unsafe {
        bindings::battery_state_service_subscribe(Some(battery_service_callback));
    }
}

#[pinned_drop]
impl<F> PinnedDrop for BatteryServiceHandle<F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            LIST.with_mut(|l| {
                l.remove(NonNull::from_mut(self.project().entry));

                re_register_callback(l);
            });
        }
    }
}

unsafe extern "C" fn battery_service_callback(event: BatteryChargeState) {
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
