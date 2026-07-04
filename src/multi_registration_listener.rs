//! This module is a utility for providing multiple registrations to a single notification.
//! This is done using an intrusive linked list, requiring zero allocations.

use core::{cell::Cell, marker::PhantomPinned, pin::Pin, ptr::NonNull, task::Poll};

use cordyceps::{Linked, List, list::Links};
use embassy_sync::waitqueue::AtomicWaker;
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::single_core_cell::SingleCoreCell;

pub(crate) type MultiRegistrationListRoot<CallbackData> = SingleCoreCell<List<Entry<CallbackData>>>;

pub trait MultiRegistrationService {
    type CallbackData: 'static;

    /// Retrieve the intrusive list root for this service.
    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData>;
}

/// An entry in the list of callbacks. `T` will be some struct containing data
/// used by the SDK callback (typically pointers to user callbacks and other data)
#[pin_data]
pub struct Entry<T> {
    #[pin]
    pub(crate) links: Links<Self>,

    pub(crate) data: T,

    #[pin]
    pub(crate) _pin_phantom: PhantomPinned,
}

unsafe impl<T> Linked<Links<Entry<T>>> for Entry<T> {
    type Handle = NonNull<Self>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Links<Self>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = &raw mut (*target).links;

            NonNull::new_unchecked(links)
        }
    }
}

pub trait MultiRegistrationListener {
    type Service: MultiRegistrationService;
    type Extra: 'static;

    /// Extract callback data from a raw pointer. `self_` is uninitialized when
    /// this function is called and therefore CallbackData should consist of
    /// only pointers calculated by offsetting `self_`.
    unsafe fn extract(
        self_: NonNull<Self>,
        extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData;

    /// called after a node is added or removed, should handle registering or
    /// deregistering.
    fn reregister<'service>(
        list: &'service MultiRegistrationListRoot<
            <Self::Service as MultiRegistrationService>::CallbackData,
        >,
    );
}

/// A handle representing that a callback is attached to an event.
///
/// When this handle is dropped, the callback will be deregistered.
#[must_use = "Callbacks are deregistered when this handle drops"]
#[pin_data(PinnedDrop)]
pub struct Handle<'service, T: MultiRegistrationListener> {
    #[pin]
    pub(crate) callbacks: T,

    #[pin]
    pub(crate) entry: Entry<<T::Service as MultiRegistrationService>::CallbackData>,

    #[pin]
    pub(crate) _pin_phantom: PhantomPinned,

    pub(crate) service: &'service T::Service,
}

impl<'service, T: MultiRegistrationListener> Handle<'service, T> {
    pub(crate) fn init(
        callbacks: T,
        extra: T::Extra,
        service: &'service T::Service,
    ) -> impl PinInit<Handle<'service, T>> {
        pin_init::pin_init!(&this in Self {
            callbacks,

            entry: Entry {
                links: Links::default(),
                data: unsafe {
                    T::extract(NonNull::new_unchecked(&raw mut (*this.as_ptr()).callbacks), extra)
                },
                _pin_phantom: PhantomPinned
            },
            _pin_phantom: PhantomPinned,
            service,
        })
        .pin_chain(|p| {
            p.register();

            Ok(())
        })
    }

    pub(crate) fn register(self: Pin<&mut Self>) {
        let p = self.project();
        unsafe {
            p.service.list().with_mut(|l| {
                l.push_front(NonNull::from_mut(p.entry.get_unchecked_mut()));
            });

            T::reregister(p.service.list());
        }
    }
}

#[pinned_drop]
impl<'service, T: MultiRegistrationListener> PinnedDrop for Handle<'service, T> {
    fn drop(self: Pin<&mut Self>) {
        let p = self.project();
        unsafe {
            p.service.list().with_mut(|l| {
                l.remove(NonNull::from_mut(p.entry.get_unchecked_mut()));
            });

            T::reregister(p.service.list());
        }
    }
}

/// A stream handle representing that a callback is attached to an event.
///
/// This type implements [futures::Stream], you can use the methods of
/// [futures::StreamExt] on it.
///
/// When this handle is dropped, the callback will be deregistered.
#[must_use = "Callbacks are deregistered when this handle drops"]
#[pin_data]
pub struct Stream<'service, T: MultiRegistrationStreamListener> {
    #[pin]
    pub(crate) handle: Handle<'service, T>,

    pub(crate) waker: AtomicWaker,

    pub(crate) value: Cell<Option<T::Value>>,
}

pub trait MultiRegistrationStreamListener: MultiRegistrationListener {
    type Value;
    unsafe fn from_stream_handler(handler: StreamHandler<Self::Value>) -> Self;
}

impl<'service, T: MultiRegistrationStreamListener> Stream<'service, T> {
    pub(crate) fn init(extra: T::Extra, service: &'service T::Service) -> impl PinInit<Self> {
        pin_init::pin_init!(&this in Self {
            handle <- Handle::init(unsafe { T::from_stream_handler(stream_closure(
                NonNull::new_unchecked(&raw mut (*this.as_ptr()).waker),
                NonNull::new_unchecked(&raw mut (*this.as_ptr()).value)
            ))}, extra, service),
            waker: AtomicWaker::new(),
            value: Cell::new(None),
        })
    }
}

impl<'service, T: MultiRegistrationStreamListener> futures::Stream for Stream<'service, T> {
    type Item = T::Value;

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

pub type StreamHandler<V> = impl FnMut(V);

#[define_opaque(StreamHandler)]
pub(crate) fn stream_closure<V>(
    waker: NonNull<AtomicWaker>,
    value: NonNull<Cell<Option<V>>>,
) -> StreamHandler<V> {
    move |evt| unsafe {
        (*value.as_ptr()).set(Some(evt));
        (*waker.as_ptr()).wake();
    }
}
