use core::ptr::NonNull;

use cordyceps::List;
use pin_init::PinInit;

use crate::{
    bindings::{self, BatteryChargeState},
    multi_registration_listener::{
        Entry, Handle, MultiRegistrationListRoot, MultiRegistrationListener,
        MultiRegistrationService, MultiRegistrationStreamListener, Stream, StreamHandler,
    },
    single_core_cell::SingleCoreCell,
};

pub trait BatteryServiceHandler<'env> = FnMut(BatteryChargeState) + 'env;

pub(crate) type BatteryServiceHandlerVTable = dyn BatteryServiceHandler<'static>;

pub struct BatteryService {
    _private: (),
}

/// Access to the battery state service, use this to subscribe
impl BatteryService {
    /// Listen to battery events
    ///
    /// When the returned [Handle] is dropped, the callback will be
    /// deregistered and the closure dropped.
    ///
    /// NOTE: You can create multiple battery event listeners from multiple locations,
    /// the library handles this elegantly using an intrusive linked list of
    /// stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
    /// closure passed in, if [Handle] could move, it would invalidate
    /// this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn listen<F>(callback: F) -> impl PinInit<Handle<'static, BatteryServiceListener<F>>>
    where
        F: FnMut(BatteryChargeState),
    {
        Handle::init(
            BatteryServiceListener { callback },
            (),
            const { &BatteryService { _private: () } },
        )
    }

    /// Similar to [Self::listen], this returns a [futures::Stream] of [BatteryChargeState].
    ///
    /// NOTE: You can create multiple battery event listeners from multiple locations,
    /// the library handles this elegantly using an intrusive linked list of
    /// stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
    /// closure passed in, if [Stream] could move, it would invalidate
    /// this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn stream()
    -> impl PinInit<Stream<'static, BatteryServiceListener<StreamHandler<BatteryChargeState>>>>
    {
        Stream::init((), const { &BatteryService { _private: () } })
    }
}

static LIST: SingleCoreCell<List<Entry<NonNull<BatteryServiceHandlerVTable>>>> =
    SingleCoreCell::new(List::new());

impl MultiRegistrationService for BatteryService {
    type CallbackData = NonNull<BatteryServiceHandlerVTable>;

    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData> {
        &LIST
    }
}

pub struct BatteryServiceListener<F> {
    callback: F,
}

impl MultiRegistrationStreamListener for BatteryServiceListener<StreamHandler<BatteryChargeState>> {
    type Value = BatteryChargeState;

    unsafe fn from_stream_handler(handler: StreamHandler<BatteryChargeState>) -> Self {
        Self { callback: handler }
    }
}

impl<'env, F> MultiRegistrationListener for BatteryServiceListener<F>
where
    F: BatteryServiceHandler<'env>,
{
    type Service = BatteryService;
    type Extra = ();

    unsafe fn extract(
        self_: NonNull<Self>,
        _extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData {
        unsafe {
            let ptr = &raw mut (*self_.as_ptr()).callback as *mut dyn BatteryServiceHandler<'_>;
            NonNull::new_unchecked(core::mem::transmute(ptr))
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
                    bindings::battery_state_service_unsubscribe();

                    return;
                }

                bindings::battery_state_service_subscribe(Some(battery_service_callback));
            });
        }
    }
}
unsafe extern "C" fn battery_service_callback(event: BatteryChargeState) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.data.as_ptr())(event);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
