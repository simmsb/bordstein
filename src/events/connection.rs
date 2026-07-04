use core::ptr::NonNull;

use cordyceps::List;
use pin_init::PinInit;

use crate::{
    bindings,
    multi_registration_listener::{
        Entry, Handle, MultiRegistrationListRoot, MultiRegistrationListener,
        MultiRegistrationService, MultiRegistrationStreamListener, Stream, StreamHandler,
    },
    single_core_cell::SingleCoreCell,
};

pub fn peek_pebble_app_connection() -> bool {
    unsafe { bindings::connection_service_peek_pebble_app_connection() }
}

pub trait ConnectionServiceHandler<'env> = FnMut(bool) + 'env;

pub(crate) type ConnectionServiceHandlerVTable = dyn ConnectionServiceHandler<'static>;

pub struct ConnectionService {
    _private: (),
}

/// Access to the connection state service, use this to subscribe
impl ConnectionService {
    /// Listen to connection events
    ///
    /// When the returned [Handle] is dropped, the callback will be
    /// deregistered and the closure dropped.
    ///
    /// NOTE: You can create multiple connection event listeners from multiple
    /// locations, the library handles this elegantly using an intrusive linked
    /// list of stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the closure passed in, if [Handle] could move, it would invalidate this
    /// reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn listen<F>(callback: F) -> impl PinInit<Handle<'static, ConnectionServiceListener<F>>>
    where
        F: FnMut(bool),
    {
        Handle::init(
            ConnectionServiceListener { callback },
            (),
            const { &ConnectionService { _private: () } },
        )
    }

    /// Similar to [Self::listen], this returns a [futures::Stream] of bool.
    ///
    /// NOTE: You can create multiple connection event listeners from multiple
    /// locations, the library handles this elegantly using an intrusive linked
    /// list of stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the closure passed in, if [Stream] could move, it would invalidate this
    /// reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn stream() -> impl PinInit<Stream<'static, ConnectionServiceListener<StreamHandler<bool>>>>
    {
        Stream::init((), const { &ConnectionService { _private: () } })
    }
}

static LIST: SingleCoreCell<List<Entry<NonNull<ConnectionServiceHandlerVTable>>>> =
    SingleCoreCell::new(List::new());

impl MultiRegistrationService for ConnectionService {
    type CallbackData = NonNull<ConnectionServiceHandlerVTable>;

    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData> {
        &LIST
    }
}

pub struct ConnectionServiceListener<F> {
    callback: F,
}

impl MultiRegistrationStreamListener for ConnectionServiceListener<StreamHandler<bool>> {
    type Value = bool;

    unsafe fn from_stream_handler(handler: StreamHandler<bool>) -> Self {
        Self { callback: handler }
    }
}

impl<'env, F> MultiRegistrationListener for ConnectionServiceListener<F>
where
    F: ConnectionServiceHandler<'env>,
{
    type Service = ConnectionService;
    type Extra = ();

    unsafe fn extract(
        self_: NonNull<Self>,
        _extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData {
        unsafe {
            let ptr = &raw mut (*self_.as_ptr()).callback as *mut dyn ConnectionServiceHandler<'_>;
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
                    bindings::connection_service_unsubscribe();

                    return;
                }

                bindings::connection_service_subscribe(bindings::ConnectionHandlers {
                    // ignore for now, might make this into App/Pebblekit
                    // connected/disconnected events if needed.
                    pebblekit_connection_handler: None,
                    pebble_app_connection_handler: Some(connection_service_callback),
                });
            });
        }
    }
}

unsafe extern "C" fn connection_service_callback(connected: bool) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.data.as_ptr())(connected);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
