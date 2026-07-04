use core::{marker::PhantomData, ptr::NonNull};

use cordyceps::List;
use pin_init::PinInit;

use crate::{
    bindings::{self, AppMessageResult},
    dictionary::{DictionaryRef, DictionaryWriter},
    multi_registration_listener::{
        Entry, Handle, MultiRegistrationListRoot, MultiRegistrationListener,
        MultiRegistrationService,
    },
    single_core_cell::SingleCoreCell,
};

/// This struct provides access to app messages. You must use
/// [AppMessages::open] to be able to register handlers and send messages.
///
/// You receive an instance of this from [crate::main].
pub struct AppMessages {
    _private: (),
}

impl AppMessages {
    #[doc(hidden)]
    pub unsafe fn steal() -> Self {
        Self { _private: () }
    }

    /// Open the app message service with the given buffer sizes.
    ///
    /// Returns an [OpenAppMessages] that can be used to send messages and
    /// register event listeners via [OpenAppMessages::listen].
    ///
    /// When the returned [OpenAppMessages] is dropped, the app message service
    /// is closed and SDK callbacks are deregistered.
    pub fn open<'handle>(
        &'handle mut self,
        size_inbound: u32,
        size_outbound: u32,
    ) -> OpenAppMessages<'handle> {
        unsafe {
            bindings::app_message_set_context(self as *mut _ as *mut _);

            bindings::app_message_register_inbox_received(Some(received_callback));
            bindings::app_message_register_inbox_dropped(Some(dropped_callback));
            bindings::app_message_register_outbox_sent(Some(sent_callback));
            bindings::app_message_register_outbox_failed(Some(failed_callback));

            bindings::app_message_open(size_inbound, size_outbound);
        }

        OpenAppMessages {
            _phantom: PhantomData,
        }
    }
}

static LIST: SingleCoreCell<List<Entry<AppMessagesPointers>>> = SingleCoreCell::new(List::new());

pub struct AppMessagesPointers {
    inbox_received: NonNull<AppMessageInboxReceivedHandlerVTable>,
    inbox_dropped: NonNull<AppMessageInboxDroppedHandlerVTable>,
    outbox_sent: NonNull<AppMessageOutboxSentHandlerVTable>,
    outbox_failed: NonNull<AppMessageOutboxFailedHandlerVTable>,
}

pub type EmptyInboxDroppedHandler<'a> = impl AppMessageInboxDroppedHandler<'a>;
pub type EmptyOutboxSentHandler<'a> = impl AppMessageOutboxSentHandler<'a>;
pub type EmptyOutboxFailedHandler<'a> = impl AppMessageOutboxFailedHandler<'a>;

impl<'open> MultiRegistrationService for OpenAppMessages<'open> {
    type CallbackData = AppMessagesPointers;

    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData> {
        &LIST
    }
}

pub struct AppMessageListener<'open, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed> {
    inbox_received: FInboxReceived,
    inbox_dropped: FInboxDropped,
    outbox_sent: FOutboxSent,
    outbox_failed: FOutboxFailed,

    _phantom: PhantomData<&'open ()>,
}

impl<'env, 'open, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>
    MultiRegistrationListener
    for AppMessageListener<'open, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>
where
    FInboxReceived: AppMessageInboxReceivedHandler<'env>,
    FInboxDropped: AppMessageInboxDroppedHandler<'env>,
    FOutboxSent: AppMessageOutboxSentHandler<'env>,
    FOutboxFailed: AppMessageOutboxFailedHandler<'env>,
{
    type Service = OpenAppMessages<'open>;

    type Extra = ();

    unsafe fn extract(
        self_: NonNull<Self>,
        _extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData {
        unsafe {
            let inbox_received = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).inbox_received
                    as *mut dyn AppMessageInboxReceivedHandler<'_>,
            ));
            let inbox_dropped = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).inbox_dropped
                    as *mut dyn AppMessageInboxDroppedHandler<'_>,
            ));
            let outbox_sent = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).outbox_sent as *mut dyn AppMessageOutboxSentHandler<'_>,
            ));
            let outbox_failed = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).outbox_failed
                    as *mut dyn AppMessageOutboxFailedHandler<'_>,
            ));

            AppMessagesPointers {
                inbox_received,
                inbox_dropped,
                outbox_sent,
                outbox_failed,
            }
        }
    }

    fn reregister<'service>(
        _list: &'service MultiRegistrationListRoot<
            <Self::Service as MultiRegistrationService>::CallbackData,
        >,
    ) {
    }
}

pub struct OpenAppMessages<'handle> {
    _phantom: PhantomData<&'handle mut ()>,
}

impl<'open> OpenAppMessages<'open> {
    pub fn send(
        &self,
        f: impl for<'dictionary> FnOnce(
            &mut DictionaryWriter<'dictionary>,
        ) -> Result<(), bindings::DictionaryResult>,
    ) -> Result<(), AppMessageSendResult> {
        let mut ptr = core::ptr::null_mut();

        unsafe {
            bindings::app_message_outbox_begin(&raw mut ptr).into_result()?;
            f(ptr.cast::<DictionaryWriter>().as_mut().unwrap())?;
            bindings::app_message_outbox_send().into_result()?;
        }

        Ok(())
    }

    /// Register callbacks to listen on app message events.
    ///
    /// These closures are capable of borrowing references to local variables.
    ///
    /// NOTE: You can create multiple app message event listeners from multiple
    /// locations, the library handles this elegantly using an intrusive linked
    /// list of stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the stack allocated closures passed in. If [Handle]
    /// could move, it would invalidate this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in
    /// your stack frame.
    pub fn listen<'env, 'this, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>(
        &'this self,
        inbox_received: FInboxReceived,
        inbox_dropped: FInboxDropped,
        outbox_sent: FOutboxSent,
        outbox_failed: FOutboxFailed,
    ) -> impl PinInit<
        Handle<
            'this,
            AppMessageListener<'open, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>,
        >,
    >
    where
        // 'open: 'env,
        // 'this: 'env,
        // 'open: 'env,
        // 'this: 'env,
        FInboxReceived: for<'message> FnMut(DictionaryRef<'message>) + 'env,
        FInboxDropped: FnMut(AppMessageResult) + 'env,
        FOutboxSent: for<'message> FnMut(DictionaryRef<'message>) + 'env,
        FOutboxFailed: for<'message> FnMut(DictionaryRef<'message>, AppMessageResult) + 'env,
    {
        Handle::init(
            AppMessageListener {
                inbox_received,
                inbox_dropped,
                outbox_sent,
                outbox_failed,
                _phantom: PhantomData,
            },
            (),
            &self,
        )
    }

    /// Register callbacks to listen on app message receive events.
    ///
    /// These closures are capable of borrowing references to local variables.
    ///
    /// NOTE: You can create multiple app message event listeners from multiple
    /// locations, the library handles this elegantly using an intrusive linked
    /// list of stack-allocated nodes.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the stack allocated closures passed in. If [Handle]
    /// could move, it would invalidate this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in
    /// your stack frame.
    pub fn listen_received<'this, 'env, FInboxReceived>(
        &'this self,
        inbox_received: FInboxReceived,
    ) -> impl PinInit<
        Handle<
            'this,
            AppMessageListener<
                'open,
                FInboxReceived,
                EmptyInboxDroppedHandler<'env>,
                EmptyOutboxSentHandler<'env>,
                EmptyOutboxFailedHandler<'env>,
            >,
        >,
    >
    where
        FInboxReceived: for<'message> FnMut(DictionaryRef<'message>) + 'env,
    {
        self.listen(
            inbox_received,
            empty_inbox_dropped_handler(),
            empty_outbox_sent_handler(),
            empty_outbox_failed_handler(),
        )
    }
}

#[define_opaque(EmptyInboxDroppedHandler)]
fn empty_inbox_dropped_handler<'a>() -> EmptyInboxDroppedHandler<'a> {
    |_| {}
}

#[define_opaque(EmptyOutboxSentHandler)]
fn empty_outbox_sent_handler<'a>() -> EmptyOutboxSentHandler<'a> {
    |_| {}
}

#[define_opaque(EmptyOutboxFailedHandler)]
fn empty_outbox_failed_handler<'a>() -> EmptyOutboxFailedHandler<'a> {
    |_, _| {}
}

impl Drop for OpenAppMessages<'_> {
    fn drop(&mut self) {
        unsafe {
            bindings::app_message_deregister_callbacks();
            bindings::app_message_set_context(core::ptr::null_mut());
        }
    }
}

pub trait AppMessageInboxReceivedHandler<'env> =
    for<'message> FnMut(DictionaryRef<'message>) + 'env;

pub trait AppMessageInboxDroppedHandler<'env> = FnMut(AppMessageResult) + 'env;

pub trait AppMessageOutboxSentHandler<'env> = for<'message> FnMut(DictionaryRef<'message>) + 'env;

pub trait AppMessageOutboxFailedHandler<'env> =
    for<'message> FnMut(DictionaryRef<'message>, AppMessageResult) + 'env;

pub(crate) type AppMessageInboxReceivedHandlerVTable = dyn AppMessageInboxReceivedHandler<'static>;

pub(crate) type AppMessageInboxDroppedHandlerVTable = dyn AppMessageInboxDroppedHandler<'static>;

pub(crate) type AppMessageOutboxSentHandlerVTable = dyn AppMessageOutboxSentHandler<'static>;

pub(crate) type AppMessageOutboxFailedHandlerVTable = dyn AppMessageOutboxFailedHandler<'static>;

#[derive(Clone, Copy, Debug)]
pub enum AppMessageSendResult {
    DictionaryResult(bindings::DictionaryResult),
    AppMessageResult(bindings::AppMessageResult),
}

impl From<bindings::DictionaryResult> for AppMessageSendResult {
    fn from(value: bindings::DictionaryResult) -> Self {
        Self::DictionaryResult(value)
    }
}

impl From<bindings::AppMessageResult> for AppMessageSendResult {
    fn from(value: bindings::AppMessageResult) -> Self {
        Self::AppMessageResult(value)
    }
}

impl bindings::AppMessageResult {
    fn into_result(self) -> Result<(), Self> {
        if self == Self::APP_MSG_OK {
            Ok(())
        } else {
            Err(self)
        }
    }
}

unsafe extern "C" fn received_callback(
    iterator: *mut bindings::DictionaryIterator,
    _context: *mut core::ffi::c_void,
) {
    let root_dict = unsafe { *NonNull::new(iterator).unwrap().as_ptr() };

    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                let mut dict = root_dict.clone();
                let dict_ref = crate::dictionary::DictionaryRef::new(NonNull::from_mut(&mut dict));
                (*entry.data.inbox_received.as_ptr())(dict_ref);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn dropped_callback(reason: AppMessageResult, _context: *mut core::ffi::c_void) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.data.inbox_dropped.as_ptr())(reason);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn sent_callback(
    iterator: *mut bindings::DictionaryIterator,
    _context: *mut core::ffi::c_void,
) {
    let root_dict = unsafe { *NonNull::new(iterator).unwrap().as_ptr() };

    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                let mut dict = root_dict.clone();
                let dict_ref = crate::dictionary::DictionaryRef::new(NonNull::from_mut(&mut dict));
                (*entry.data.outbox_sent.as_ptr())(dict_ref);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn failed_callback(
    iterator: *mut bindings::DictionaryIterator,
    reason: AppMessageResult,
    _context: *mut core::ffi::c_void,
) {
    let root_dict = unsafe { *NonNull::new(iterator).unwrap().as_ptr() };

    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                let mut dict = root_dict.clone();
                let dict_ref = crate::dictionary::DictionaryRef::new(NonNull::from_mut(&mut dict));
                (*entry.data.outbox_failed.as_ptr())(dict_ref, reason);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}
