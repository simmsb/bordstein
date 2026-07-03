use core::{cell::Cell, marker::PhantomPinned, pin::Pin, ptr::NonNull, task::Poll};

use cordyceps::{Linked, List, list::Links};
use embassy_sync::waitqueue::AtomicWaker;
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::{
    bindings::{self, TimeUnits},
    single_core_cell::SingleCoreCell,
    time::Datetime,
};

struct TickServiceEntry {
    links: Links<TickServiceEntry>,

    units: TimeUnits,
    callback: *mut TickServiceHandlerVTable,
}

unsafe impl Linked<Links<TickServiceEntry>> for TickServiceEntry {
    type Handle = NonNull<TickServiceEntry>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Links<TickServiceEntry>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = core::ptr::addr_of_mut!((*target).links);

            NonNull::new_unchecked(links)
        }
    }
}

pub trait TickServiceHandler<'env> = for<'tm> FnMut(&'tm bindings::tm, bindings::TimeUnits) + 'env;

pub(crate) type TickServiceHandlerVTable = dyn TickServiceHandler<'static>;

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Callback is deregistered and dropped when [TickServiceHandle] is dropped"]
#[pin_data(PinnedDrop)]
pub struct TickServiceHandle<F> {
    #[pin]
    callback: F,

    entry: TickServiceEntry,

    #[pin]
    _pin_phantom: PhantomPinned,
}

static LIST: SingleCoreCell<List<TickServiceEntry>> = SingleCoreCell::new(List::new());

/// Listen to tick events, the given callback will be called on tick events
/// matching the passed time units.
///
/// When the returned [TickServiceHandle] is dropped, the callback will be
/// deregistered and the closure dropped.
///
/// NOTE: You can create multiple tick event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes. The tick service is automatically re-registered as
/// listeners are added and removed.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [TickServiceHandle] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [TickServiceHandle] is dropped"]
pub fn listen<F>(units: TimeUnits, callback: F) -> impl PinInit<TickServiceHandle<F>>
where
    F: for<'tm> FnMut(&'tm bindings::tm, bindings::TimeUnits),
{
    pin_init::pin_init!(&this in TickServiceHandle {
        callback,

        entry: TickServiceEntry {
            links: Links::default(),
            units,
            callback: unsafe { core::mem::transmute::<_, *mut TickServiceHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn TickServiceHandler<'_>) },
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

type TickServiceStreamHandler = impl FnMut(&bindings::tm, TimeUnits);

#[pin_data]
pub struct TickServiceStream {
    #[pin]
    handle: TickServiceHandle<TickServiceStreamHandler>,

    waker: AtomicWaker,

    value: Cell<Option<(Datetime, TimeUnits)>>,
}

impl futures::Stream for TickServiceStream {
    type Item = (Datetime, TimeUnits);

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

#[define_opaque(TickServiceStreamHandler)]
fn stream_closure(ptr: NonNull<TickServiceStream>) -> TickServiceStreamHandler {
    move |tm: &bindings::tm, units| unsafe {
        let waker_ptr = &raw mut (*ptr.as_ptr()).waker;
        let value_ptr = &raw mut (*ptr.as_ptr()).value;
        (*value_ptr).set(Some((Datetime::from_tm(tm), units)));
        (*waker_ptr).wake();
    }
}

/// Similar to [listen], this returns a [futures::Stream] of [Datetime].
///
/// NOTE: You can create multiple tick event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes. The tick service is automatically re-registered as
/// listeners are added and removed.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [TickServiceStream] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [TickServiceStream] is dropped"]
pub fn stream(units: TimeUnits) -> impl PinInit<TickServiceStream> {
    pin_init::pin_init!(&this in TickServiceStream {
        handle <- listen(units, stream_closure(this)),
        waker: AtomicWaker::new(),
        value: Cell::new(None),
    })
}

unsafe fn re_register_callback(list: &mut List<TickServiceEntry>) {
    if list.is_empty() {
        unsafe {
            bindings::tick_timer_service_unsubscribe();

            return;
        }
    }

    let mut new_units = TimeUnits(0);

    for entry in list.iter() {
        new_units |= entry.units;
    }

    unsafe {
        bindings::tick_timer_service_subscribe(new_units, Some(tick_service_callback));
    }
}

#[pinned_drop]
impl<F> PinnedDrop for TickServiceHandle<F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            LIST.with_mut(|l| {
                l.remove(NonNull::from_mut(self.project().entry));

                re_register_callback(l);
            });
        }
    }
}

unsafe extern "C" fn tick_service_callback(
    tick_time: *mut bindings::tm,
    units_changed: bindings::TimeUnits,
) {
    let tm = unsafe { NonNull::new(tick_time).unwrap().as_ref() };

    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                if (units_changed & entry.units) != TimeUnits(0) {
                    (*entry.callback)(tm, units_changed);
                }
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
