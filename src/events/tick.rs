use core::ptr::NonNull;

use cordyceps::List;
use pin_init::PinInit;

use crate::{
    bindings::{self, TimeUnits},
    multi_registration_listener::{
        Entry, Handle, MultiRegistrationListRoot, MultiRegistrationListener,
        MultiRegistrationService, MultiRegistrationStreamListener, Stream, StreamHandler,
    },
    single_core_cell::SingleCoreCell,
    time::Datetime,
};

pub trait TickServiceHandler<'env> = for<'tm> FnMut(&'tm bindings::tm, bindings::TimeUnits) + 'env;

pub(crate) type TickServiceHandlerVTable = dyn TickServiceHandler<'static>;

pub struct TickService {
    _private: (),
}

/// Access to the tick service, use this to subscribe
impl TickService {
    /// Listen to tick events, the given callback will be called on tick events
    /// matching the passed time units.
    ///
    /// When the returned [Handle] is dropped, the callback will be
    /// deregistered and the closure dropped.
    ///
    /// NOTE: You can create multiple tick event listeners from multiple locations,
    /// the library handles this elegantly using an intrusive linked list of
    /// stack-allocated nodes. The tick service is automatically re-registered as
    /// listeners are added and removed.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
    /// closure passed in, if [Handle] could move, it would invalidate
    /// this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn listen<F>(
        units: TimeUnits,
        callback: F,
    ) -> impl PinInit<Handle<'static, TickServiceListener<F>>>
    where
        F: for<'tm> FnMut(&'tm bindings::tm, bindings::TimeUnits),
    {
        Handle::init(
            TickServiceListener { callback },
            units,
            const { &TickService { _private: () } },
        )
    }

    /// Similar to [Self::listen], this returns a [futures::Stream] of ([Datetime], [TimeUnits]).
    ///
    /// NOTE: You can create multiple health event listeners from multiple
    /// locations, the library handles this elegantly using an intrusive linked
    /// list of stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the closure passed in, if [Stream] could move, it would invalidate this
    /// reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn stream(
        units: TimeUnits,
    ) -> impl PinInit<Stream<'static, TickServiceListener<TickServiceStreamHandler>>> {
        Stream::init(units, const { &TickService { _private: () } })
    }
}

static LIST: SingleCoreCell<List<Entry<(TimeUnits, NonNull<TickServiceHandlerVTable>)>>> =
    SingleCoreCell::new(List::new());

impl MultiRegistrationService for TickService {
    type CallbackData = (TimeUnits, NonNull<TickServiceHandlerVTable>);

    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData> {
        &LIST
    }
}

pub struct TickServiceListener<F> {
    callback: F,
}

// pub type TickServiceStreamHandler = impl TickServiceHandler<'static>;
//
// Using a TAIT here causes a compiler crash, so we use a fn trait instead.

pub struct TickServiceStreamHandler {
    inner: StreamHandler<(Datetime, TimeUnits)>,
}

impl FnOnce<(&bindings::tm, bindings::TimeUnits)> for TickServiceStreamHandler {
    type Output = ();

    extern "rust-call" fn call_once(
        mut self,
        args: (&bindings::tm, bindings::TimeUnits),
    ) -> Self::Output {
        self.call_mut(args)
    }
}

impl FnMut<(&bindings::tm, bindings::TimeUnits)> for TickServiceStreamHandler {
    extern "rust-call" fn call_mut(
        &mut self,
        args: (&bindings::tm, bindings::TimeUnits),
    ) -> Self::Output {
        (self.inner)((Datetime::from_tm(args.0), args.1))
    }
}

impl MultiRegistrationStreamListener for TickServiceListener<TickServiceStreamHandler> {
    type Value = (Datetime, TimeUnits);

    // #[define_opaque(TickServiceStreamHandler)]
    unsafe fn from_stream_handler(handler: StreamHandler<(Datetime, TimeUnits)>) -> Self {
        Self {
            callback: TickServiceStreamHandler { inner: handler },
        }
    }
}

impl<'env, F> MultiRegistrationListener for TickServiceListener<F>
where
    F: TickServiceHandler<'env>,
{
    type Service = TickService;
    type Extra = TimeUnits;

    unsafe fn extract(
        self_: NonNull<Self>,
        extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData {
        unsafe {
            let ptr = &raw mut (*self_.as_ptr()).callback as *mut dyn TickServiceHandler<'_>;
            (extra, NonNull::new_unchecked(core::mem::transmute(ptr)))
        }
    }

    fn reregister<'service>(
        list: &'service MultiRegistrationListRoot<
            <Self::Service as MultiRegistrationService>::CallbackData,
        >,
    ) {
        unsafe {
            list.with_mut(|list| {
                if list.is_empty() {
                    bindings::tick_timer_service_unsubscribe();

                    return;
                }

                let mut new_units = TimeUnits(0);

                for entry in list.iter() {
                    new_units |= entry.data.0;
                }

                bindings::tick_timer_service_subscribe(new_units, Some(tick_service_callback));
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
                if (units_changed & entry.data.0) != TimeUnits(0) {
                    let fn_ptr = entry.data.1;
                    (*fn_ptr.as_ptr())(tm, units_changed);
                }
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
