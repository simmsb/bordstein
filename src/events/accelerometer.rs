use core::{
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
    sync::atomic::{AtomicPtr, Ordering},
};

use pin_init::{PinInit, pin_data, pinned_drop};

use crate::bindings::{self, AccelAxisType, AccelData, AccelRawData};

/// Access to the accelerometer. You must use [AccelerometerService::enable] to start it.
pub struct AccelerometerService {
    pub(crate) _private: (),
}

impl AccelerometerService {
    #[doc(hidden)]
    pub unsafe fn steal() -> Self {
        Self {
            _private: (),
        }
    }

    pub fn enable<'handle>(&'handle mut self) -> AccelerometerServiceHandle<'handle> {
        unsafe {
            bindings::accel_data_service_subscribe(1, None);
        }

        AccelerometerServiceHandle {
            _phantom: PhantomData,
        }
    }
}

pub struct AccelerometerServiceHandle<'handle> {
    _phantom: PhantomData<&'handle mut ()>,
}

impl<'handle> AccelerometerServiceHandle<'handle> {
    // TODO: AccelData is quite large and packed, it might be worth repacking it
    // into a rust struct
    pub fn peek(&mut self) -> bindings::AccelData {
        unsafe {
            let mut acceldata = MaybeUninit::uninit();
            let result = bindings::accel_service_peek(acceldata.as_mut_ptr());
            assert!(result >= 0);
            acceldata.assume_init()
        }
    }

    /// Set a callback to listen on accel service events.
    ///
    /// These closures are capable of borrowing references to local variables.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the stack allocated closures passed in. If [AccelerometerServiceHandle] could
    /// move, it would invalidate this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in
    /// your stack frame.
    ///
    /// N.B. I expose this as a callback due to there being a variable number of
    /// [AccelData] values, which must be consumed within the scope of the
    /// callback. If you want to react to these events in async code, you might
    /// consider having the callback simply push these onto a queue.
    #[must_use = "Service is unsubscribed and closure dropped when [DataServiceSubscription] is dropped."]
    pub fn subscribe_to_data_service<'subscription, F>(
        &'subscription mut self,
        samples_per_update: u32,
        callback: F,
    ) -> impl PinInit<DataServiceSubscription<'subscription, F>>
    where
        F: for<'samples> FnMut(&'samples [AccelData]) + 'subscription,
    {
        pin_init::pin_init!{&this in DataServiceSubscription {
            callback,
            callback_vtable:
            unsafe { core::mem::transmute::<_, *mut DataServiceSubscriptionHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn DataServiceSubscriptionHandler<'_>) },
            _phantom: PhantomData,
            _pin_phantom: PhantomPinned,
        }}.pin_chain(move |p| {
            let project = p.project();

            DATA_SERVICE_SUBSCRIPTION_VTABLE.store(&raw mut *project.callback_vtable, Ordering::SeqCst);

            unsafe {
                bindings::accel_data_service_subscribe(samples_per_update, Some(accel_data_service_handler));
            }

            Ok(())
        })
    }

    #[must_use = "Service is unsubscribed and closure dropped when [RawDataServiceSubscription] is dropped."]
    pub fn subscribe_to_raw_data_service<'subscription, F>(
        &'subscription mut self,
        samples_per_update: u32,
        callback: F,
    ) -> impl PinInit<RawDataServiceSubscription<'subscription, F>>
    where
        F: for<'samples> FnMut(&'samples [AccelRawData], u64) + 'subscription,
    {
        pin_init::pin_init!{&this in RawDataServiceSubscription {
            callback,
            callback_vtable:
            unsafe { core::mem::transmute::<_, *mut RawDataServiceSubscriptionHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn RawDataServiceSubscriptionHandler<'_>) },
            _phantom: PhantomData,
            _pin_phantom: PhantomPinned,
        }}.pin_chain(move |p| {
            let project = p.project();

            RAW_DATA_SERVICE_SUBSCRIPTION_VTABLE.store(&raw mut *project.callback_vtable, Ordering::SeqCst);

            unsafe {
                bindings::accel_raw_data_service_subscribe(samples_per_update, Some(accel_raw_data_service_handler));
            }

            Ok(())
        })
    }

    #[must_use = "Service is unsubscribed and closure dropped when [TapServiceSubscription] is dropped."]
    pub fn subscribe_to_tap_service<'subscription, F>(
        &'subscription mut self,
        callback: F,
    ) -> impl PinInit<TapServiceSubscription<'subscription, F>>
    where
        F: FnMut(AccelAxisType, i32) + 'subscription,
    {
        pin_init::pin_init!{&this in TapServiceSubscription {
            callback,
            callback_vtable:
            unsafe { core::mem::transmute::<_, *mut TapServiceSubscriptionHandlerVTable>(&raw mut (*this.as_ptr()).callback as *mut dyn TapServiceSubscriptionHandler<'_>) },
            _phantom: PhantomData,
            _pin_phantom: PhantomPinned,
        }}.pin_chain(move |p| {
            let project = p.project();

            TAP_SERVICE_SUBSCRIPTION_VTABLE.store(&raw mut *project.callback_vtable, Ordering::SeqCst);

            unsafe {
                bindings::accel_tap_service_subscribe(Some(accel_tap_service_handler));
            }

            Ok(())
        })
    }
}

impl<'handle> Drop for AccelerometerServiceHandle<'handle> {
    fn drop(&mut self) {
        unsafe {
            bindings::accel_data_service_unsubscribe();
        }
    }
}

pub trait DataServiceSubscriptionHandler<'env> = for<'samples> FnMut(&'samples [AccelData]) + 'env;
pub trait RawDataServiceSubscriptionHandler<'env> =
    for<'samples> FnMut(&'samples [AccelRawData], u64) + 'env;
pub trait TapServiceSubscriptionHandler<'env> = FnMut(AccelAxisType, i32) + 'env;

pub(crate) type DataServiceSubscriptionHandlerVTable = dyn DataServiceSubscriptionHandler<'static>;
pub(crate) type RawDataServiceSubscriptionHandlerVTable =
    dyn RawDataServiceSubscriptionHandler<'static>;
pub(crate) type TapServiceSubscriptionHandlerVTable = dyn TapServiceSubscriptionHandler<'static>;

static DATA_SERVICE_SUBSCRIPTION_VTABLE: AtomicPtr<*mut DataServiceSubscriptionHandlerVTable> =
    AtomicPtr::null();
static RAW_DATA_SERVICE_SUBSCRIPTION_VTABLE: AtomicPtr<
    *mut RawDataServiceSubscriptionHandlerVTable,
> = AtomicPtr::null();
static TAP_SERVICE_SUBSCRIPTION_VTABLE: AtomicPtr<*mut TapServiceSubscriptionHandlerVTable> =
    AtomicPtr::null();

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Service is unsubscribed and closure dropped when [DataServiceSubscription] is dropped."]
#[pin_data(PinnedDrop)]
pub struct DataServiceSubscription<'subscription, F> {
    #[pin]
    callback: F,

    callback_vtable: *mut DataServiceSubscriptionHandlerVTable,

    #[pin]
    _pin_phantom: PhantomPinned,

    _phantom: PhantomData<&'subscription mut ()>,
}

#[pinned_drop]
impl<'handle, F> PinnedDrop for DataServiceSubscription<'handle, F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            DATA_SERVICE_SUBSCRIPTION_VTABLE.store(core::ptr::null_mut(), Ordering::SeqCst);
            bindings::accel_data_service_subscribe(0, None);
        }
    }
}

unsafe extern "C" fn accel_data_service_handler(data: *mut AccelData, num_samples: u32) {
    let vtable = DATA_SERVICE_SUBSCRIPTION_VTABLE.load(Ordering::SeqCst);
    let Some(vtable) = NonNull::new(vtable) else {
        return;
    };

    unsafe {
        let accel_data = core::slice::from_raw_parts(data, num_samples as usize);
        (**vtable.as_ptr())(accel_data);
    }
}

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Service is unsubscribed and closure dropped when [RawDataServiceSubscription] is dropped."]
#[pin_data(PinnedDrop)]
pub struct RawDataServiceSubscription<'subscription, F> {
    #[pin]
    callback: F,

    callback_vtable: *mut RawDataServiceSubscriptionHandlerVTable,

    #[pin]
    _pin_phantom: PhantomPinned,

    _phantom: PhantomData<&'subscription mut ()>,
}

#[pinned_drop]
impl<'handle, F> PinnedDrop for RawDataServiceSubscription<'handle, F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            RAW_DATA_SERVICE_SUBSCRIPTION_VTABLE.store(core::ptr::null_mut(), Ordering::SeqCst);
            bindings::accel_raw_data_service_subscribe(0, None);
        }
    }
}

unsafe extern "C" fn accel_raw_data_service_handler(
    data: *mut AccelRawData,
    num_samples: u32,
    timestamp: u64,
) {
    let vtable = RAW_DATA_SERVICE_SUBSCRIPTION_VTABLE.load(Ordering::SeqCst);
    let Some(vtable) = NonNull::new(vtable) else {
        return;
    };

    unsafe {
        let accel_data = core::slice::from_raw_parts(data, num_samples as usize);
        (**vtable.as_ptr())(accel_data, timestamp);
    }
}

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Service is unsubscribed and closure dropped when [TapServiceSubscription] is dropped."]
#[pin_data(PinnedDrop)]
pub struct TapServiceSubscription<'subscription, F> {
    #[pin]
    callback: F,

    callback_vtable: *mut TapServiceSubscriptionHandlerVTable,

    #[pin]
    _pin_phantom: PhantomPinned,

    _phantom: PhantomData<&'subscription mut ()>,
}

#[pinned_drop]
impl<'handle, F> PinnedDrop for TapServiceSubscription<'handle, F> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            TAP_SERVICE_SUBSCRIPTION_VTABLE.store(core::ptr::null_mut(), Ordering::SeqCst);
            bindings::accel_tap_service_unsubscribe();
        }
    }
}

unsafe extern "C" fn accel_tap_service_handler(axis: AccelAxisType, direction: i32) {
    let vtable = TAP_SERVICE_SUBSCRIPTION_VTABLE.load(Ordering::SeqCst);
    let Some(vtable) = NonNull::new(vtable) else {
        return;
    };

    unsafe {
        (**vtable.as_ptr())(axis, direction);
    }
}
