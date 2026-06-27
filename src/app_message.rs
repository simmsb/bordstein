use core::{
    marker::{PhantomData, PhantomPinned},
    pin::Pin,
    ptr::NonNull,
};

use pin_init::{PinInit, pinned_drop};

use crate::{
    bindings::{self, AppMessageResult},
    dictionary::{DictionaryRef, DictionaryWriter},
};

pub struct AppMessages;

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

#[pin_init::pin_data(PinnedDrop)]
pub struct AppMessagesHandle<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed> {
    #[pin]
    callback_inbox_received: FInboxReceived,

    #[pin]
    callback_inbox_dropped: FInboxDropped,

    #[pin]
    callback_outbox_sent: FOutboxSent,

    #[pin]
    callback_outbox_failed: FOutboxFailed,

    vtables: AppMessagesVTables,

    #[pin]
    _pin_phantom: PhantomPinned,

    _phantom: PhantomData<&'handle ()>,
}

impl<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>
    AppMessagesHandle<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>
{
    pub fn send(
        self: &mut Pin<&mut Self>,
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
}

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

struct AppMessagesVTables {
    callback_inbox_received_vtable: *mut AppMessageInboxReceivedHandlerVTable,
    callback_inbox_dropped_vtable: *mut AppMessageInboxDroppedHandlerVTable,
    callback_outbox_sent_vtable: *mut AppMessageOutboxSentHandlerVTable,
    callback_outbox_failed_vtable: *mut AppMessageOutboxFailedHandlerVTable,
}

impl AppMessages {
    /// Set callbacks to listen on app message events.
    ///
    /// These closures are capable of borrowing references to local variables.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to
    /// the stack allocated closures passed in. If [AppMessagesHandle] could
    /// move, it would invalidate this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in
    /// your stack frame.
    #[must_use = "Callbacks are deregistered and dropped when [AppMessagesHandle] is dropped."]
    pub fn listen<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>(
        &mut self,
        size_inbound: u32,
        size_outbound: u32,
        inbox_received: FInboxReceived,
        inbox_dropped: FInboxDropped,
        outbox_sent: FOutboxSent,
        outbox_failed: FOutboxFailed,
    ) -> impl PinInit<
        AppMessagesHandle<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>,
    >
    where
        FInboxReceived: for<'message> FnMut(DictionaryRef<'message>) + 'handle,
        FInboxDropped: FnMut(AppMessageResult) + 'handle,
        FOutboxSent: for<'message> FnMut(DictionaryRef<'message>) + 'handle,
        FOutboxFailed: for<'message> FnMut(DictionaryRef<'message>, AppMessageResult) + 'handle,
    {
        pin_init::pin_init!(&this in AppMessagesHandle {
            callback_inbox_received: inbox_received,
            callback_inbox_dropped: inbox_dropped,
            callback_outbox_sent: outbox_sent,
            callback_outbox_failed: outbox_failed,

            vtables: AppMessagesVTables {
                callback_inbox_received_vtable:
                unsafe { core::mem::transmute::<_, *mut AppMessageInboxReceivedHandlerVTable>(&raw mut (*this.as_ptr()).callback_inbox_received as *mut dyn AppMessageInboxReceivedHandler<'_>) },
                callback_inbox_dropped_vtable:
                unsafe { core::mem::transmute::<_, *mut AppMessageInboxDroppedHandlerVTable>(&raw mut (*this.as_ptr()).callback_inbox_dropped as *mut dyn AppMessageInboxDroppedHandler<'_>) },
                callback_outbox_sent_vtable:
                unsafe { core::mem::transmute::<_, *mut AppMessageOutboxSentHandlerVTable>(&raw mut (*this.as_ptr()).callback_outbox_sent as *mut dyn AppMessageOutboxSentHandler<'_>) },
                callback_outbox_failed_vtable:
                unsafe { core::mem::transmute::<_, *mut AppMessageOutboxFailedHandlerVTable>(&raw mut (*this.as_ptr()).callback_outbox_failed as *mut dyn AppMessageOutboxFailedHandler<'_>) },
            },

            _pin_phantom: PhantomPinned,
            _phantom: PhantomData,
        })
        .pin_chain(move |p| {
            let project = p.project();

            unsafe {
                bindings::app_message_set_context(&raw mut *project.vtables as *mut _);

                bindings::app_message_register_inbox_received(Some(received_callback));
                bindings::app_message_register_inbox_dropped(Some(dropped_callback));
                bindings::app_message_register_outbox_sent(Some(sent_callback));
                bindings::app_message_register_outbox_failed(Some(failed_callback));

                bindings::app_message_open(size_inbound, size_outbound);
            }

            Ok(())
        })
    }
}

#[pinned_drop]
impl<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed> PinnedDrop
    for AppMessagesHandle<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>
{
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            bindings::app_message_deregister_callbacks();
            bindings::app_message_set_context(core::ptr::null_mut());
        }
    }
}

unsafe extern "C" fn received_callback(
    iterator: *mut bindings::DictionaryIterator,
    context: *mut core::ffi::c_void,
) {
    let dict = crate::dictionary::DictionaryRef::new(NonNull::new(iterator).unwrap());

    let vtables = context as *mut AppMessagesVTables;

    unsafe {
        (*(*vtables).callback_inbox_received_vtable)(dict);
    }
}

unsafe extern "C" fn dropped_callback(reason: AppMessageResult, context: *mut core::ffi::c_void) {
    let vtables = context as *mut AppMessagesVTables;

    unsafe {
        (*(*vtables).callback_inbox_dropped_vtable)(reason);
    }
}

unsafe extern "C" fn sent_callback(
    iterator: *mut bindings::DictionaryIterator,
    context: *mut core::ffi::c_void,
) {
    let dict = crate::dictionary::DictionaryRef::new(NonNull::new(iterator).unwrap());

    let vtables = context as *mut AppMessagesVTables;

    unsafe {
        (*(*vtables).callback_outbox_sent_vtable)(dict);
    }
}

unsafe extern "C" fn failed_callback(
    iterator: *mut bindings::DictionaryIterator,
    reason: AppMessageResult,
    context: *mut core::ffi::c_void,
) {
    let dict = crate::dictionary::DictionaryRef::new(NonNull::new(iterator).unwrap());

    let vtables = context as *mut AppMessagesVTables;

    unsafe {
        (*(*vtables).callback_outbox_failed_vtable)(dict, reason);
    }
}
