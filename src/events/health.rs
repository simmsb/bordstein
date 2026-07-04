use core::ptr::NonNull;

use cordyceps::List;
use pin_init::PinInit;

use crate::{
    bindings::{self, HealthEventType},
    multi_registration_listener::{
        Entry, Handle, MultiRegistrationListRoot, MultiRegistrationListener,
        MultiRegistrationService, MultiRegistrationStreamListener, Stream, StreamHandler,
    },
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

pub trait HealthServiceHandler<'env> = FnMut(HealthEventType) + 'env;

pub(crate) type HealthServiceHandlerVTable = dyn HealthServiceHandler<'static>;

pub struct HealthService {
    _private: (),
}

/// Access to the health service, use this to subscribe
impl HealthService {
    /// Listen to health events
    ///
    /// When the returned [Handle] is dropped, the callback will be
    /// deregistered and the closure dropped.
    ///
    /// NOTE: You can create multiple health event listeners from multiple
    /// locations, the library handles this elegantly using an intrusive linked
    /// list of stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the closure passed in, if [Handle] could move, it would invalidate this
    /// reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn listen<F>(callback: F) -> impl PinInit<Handle<'static, HealthServiceListener<F>>>
    where
        F: FnMut(HealthEventType),
    {
        Handle::init(
            HealthServiceListener { callback },
            (),
            const { &HealthService { _private: () } },
        )
    }

    /// Similar to [Self::listen], this returns a [futures::Stream] of health events.
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
    pub fn stream()
    -> impl PinInit<Stream<'static, HealthServiceListener<StreamHandler<HealthEventType>>>> {
        Stream::init((), const { &HealthService { _private: () } })
    }
}

static LIST: SingleCoreCell<List<Entry<NonNull<HealthServiceHandlerVTable>>>> =
    SingleCoreCell::new(List::new());

impl MultiRegistrationService for HealthService {
    type CallbackData = NonNull<HealthServiceHandlerVTable>;

    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData> {
        &LIST
    }
}

pub struct HealthServiceListener<F> {
    callback: F,
}

impl MultiRegistrationStreamListener for HealthServiceListener<StreamHandler<HealthEventType>> {
    type Value = HealthEventType;

    unsafe fn from_stream_handler(handler: StreamHandler<HealthEventType>) -> Self {
        Self { callback: handler }
    }
}

impl<'env, F> MultiRegistrationListener for HealthServiceListener<F>
where
    F: HealthServiceHandler<'env>,
{
    type Service = HealthService;
    type Extra = ();

    unsafe fn extract(
        self_: NonNull<Self>,
        _extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData {
        unsafe {
            let ptr = &raw mut (*self_.as_ptr()).callback as *mut dyn HealthServiceHandler<'_>;
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
                    bindings::health_service_events_unsubscribe();

                    return;
                }

                bindings::health_service_events_subscribe(
                    Some(health_service_callback),
                    core::ptr::null_mut(),
                );
            });
        }
    }
}

unsafe extern "C" fn health_service_callback(
    event: HealthEventType,
    _context: *mut core::ffi::c_void,
) {
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
