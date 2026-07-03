use core::{
    marker::{PhantomData, PhantomPinned},
    pin::Pin,
    ptr::NonNull,
};

use cordyceps::{Linked, List, list::Links};
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::{
    bindings::{self, AppMessageResult},
    dictionary::{DictionaryRef, DictionaryWriter},
    single_core_cell::SingleCoreCell,
};

/// This struct provides access to app messages. You must use
/// [AppMessages::open] to be able to register handlers and send messages.
///
/// You receive an instance of this from [crate::main].
pub struct AppMessages {
    listeners: SingleCoreCell<List<AppMessageEntry>>,
}

impl AppMessages {
    #[doc(hidden)]
    pub unsafe fn steal() -> Self {
        Self {
            listeners: SingleCoreCell::new(List::new()),
        }
    }

    /// Open the app message service with the given buffer sizes.
    ///
    /// Returns an [AppMessagesHandle] that can be used to send messages and
    /// register event listeners via [AppMessagesHandle::listen].
    ///
    /// When the returned [AppMessagesHandle] is dropped, the app message service
    /// is closed and SDK callbacks are deregistered.
    pub fn open<'handle>(
        &'handle mut self,
        size_inbound: u32,
        size_outbound: u32,
    ) -> AppMessagesHandle<'handle> {
        unsafe {
            bindings::app_message_set_context(self as *mut _ as *mut _);

            bindings::app_message_register_inbox_received(Some(received_callback));
            bindings::app_message_register_inbox_dropped(Some(dropped_callback));
            bindings::app_message_register_outbox_sent(Some(sent_callback));
            bindings::app_message_register_outbox_failed(Some(failed_callback));

            bindings::app_message_open(size_inbound, size_outbound);
        }

        AppMessagesHandle {
            app_messages: NonNull::from(self),
            _phantom: PhantomData,
        }
    }
}

struct AppMessageEntry {
    links: Links<AppMessageEntry>,

    callback_inbox_received: *mut AppMessageInboxReceivedHandlerVTable,
    callback_inbox_dropped: *mut AppMessageInboxDroppedHandlerVTable,
    callback_outbox_sent: *mut AppMessageOutboxSentHandlerVTable,
    callback_outbox_failed: *mut AppMessageOutboxFailedHandlerVTable,
}

unsafe impl Linked<Links<AppMessageEntry>> for AppMessageEntry {
    type Handle = NonNull<AppMessageEntry>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Links<AppMessageEntry>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = core::ptr::addr_of_mut!((*target).links);

            NonNull::new_unchecked(links)
        }
    }
}

pub type EmptyInboxDroppedHandler = impl AppMessageInboxDroppedHandler<'static>;
pub type EmptyOutboxSentHandler = impl AppMessageOutboxSentHandler<'static>;
pub type EmptyOutboxFailedHandler = impl AppMessageOutboxFailedHandler<'static>;

pub struct AppMessagesHandle<'handle> {
    app_messages: NonNull<AppMessages>,
    _phantom: PhantomData<&'handle mut ()>,
}

impl<'handle> AppMessagesHandle<'handle> {
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
    /// the stack allocated closures passed in. If [AppMessageListenerHandle]
    /// could move, it would invalidate this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in
    /// your stack frame.
    #[must_use = "Callbacks are deregistered and dropped when [AppMessageListenerHandle] is dropped."]
    pub fn listen<'subscription, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>(
        &self,
        inbox_received: FInboxReceived,
        inbox_dropped: FInboxDropped,
        outbox_sent: FOutboxSent,
        outbox_failed: FOutboxFailed,
    ) -> impl PinInit<
        AppMessageListenerHandle<
            'subscription,
            FInboxReceived,
            FInboxDropped,
            FOutboxSent,
            FOutboxFailed,
        >,
    >
    where
        'subscription: 'handle,
        FInboxReceived: for<'message> FnMut(DictionaryRef<'message>) + 'subscription,
        FInboxDropped: FnMut(AppMessageResult) + 'subscription,
        FOutboxSent: for<'message> FnMut(DictionaryRef<'message>) + 'subscription,
        FOutboxFailed:
            for<'message> FnMut(DictionaryRef<'message>, AppMessageResult) + 'subscription,
    {
        let app_messages = self.app_messages;

        pin_init::pin_init!{&this in AppMessageListenerHandle {
            callback_inbox_received: inbox_received,
            callback_inbox_dropped: inbox_dropped,
            callback_outbox_sent: outbox_sent,
            callback_outbox_failed: outbox_failed,

            entry: AppMessageEntry {
                links: Links::default(),
                callback_inbox_received:
                unsafe { core::mem::transmute::<_, *mut AppMessageInboxReceivedHandlerVTable>(&raw mut (*this.as_ptr()).callback_inbox_received as *mut dyn AppMessageInboxReceivedHandler<'_>) },
                callback_inbox_dropped:
                unsafe { core::mem::transmute::<_, *mut AppMessageInboxDroppedHandlerVTable>(&raw mut (*this.as_ptr()).callback_inbox_dropped as *mut dyn AppMessageInboxDroppedHandler<'_>) },
                callback_outbox_sent:
                unsafe { core::mem::transmute::<_, *mut AppMessageOutboxSentHandlerVTable>(&raw mut (*this.as_ptr()).callback_outbox_sent as *mut dyn AppMessageOutboxSentHandler<'_>) },
                callback_outbox_failed:
                unsafe { core::mem::transmute::<_, *mut AppMessageOutboxFailedHandlerVTable>(&raw mut (*this.as_ptr()).callback_outbox_failed as *mut dyn AppMessageOutboxFailedHandler<'_>) },
            },

            app_messages,

            _pin_phantom: PhantomPinned,
            _phantom: PhantomData,
        }}
        .pin_chain(move |p| {
            let project = p.project();

            unsafe {
                project.app_messages.as_ref().listeners.with_mut(|l| {
                    l.push_front(NonNull::from_mut(project.entry));
                });
            }

            Ok(())
        })
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
    /// the stack allocated closures passed in. If [AppMessageListenerHandle]
    /// could move, it would invalidate this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in
    /// your stack frame.
    #[must_use = "Callbacks are deregistered and dropped when [AppMessageListenerHandle] is dropped."]
    pub fn listen_received<'subscription, FInboxReceived>(
        &self,
        inbox_received: FInboxReceived,
    ) -> impl PinInit<
        AppMessageListenerHandle<
            'subscription,
            FInboxReceived,
            EmptyInboxDroppedHandler,
            EmptyOutboxSentHandler,
            EmptyOutboxFailedHandler,
        >,
    >
    where
        'subscription: 'handle,
        FInboxReceived: for<'message> FnMut(DictionaryRef<'message>) + 'subscription,
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
fn empty_inbox_dropped_handler() -> EmptyInboxDroppedHandler {
    |_| {}
}

#[define_opaque(EmptyOutboxSentHandler)]
fn empty_outbox_sent_handler() -> EmptyOutboxSentHandler {
    |_| {}
}

#[define_opaque(EmptyOutboxFailedHandler)]
fn empty_outbox_failed_handler() -> EmptyOutboxFailedHandler {
    |_, _| {}
}

impl Drop for AppMessagesHandle<'_> {
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

/// This struct represents that an AppMessage handler is registered.
/// When dropped, the handler is deregistered.
#[must_use = "Callbacks are deregistered and dropped when [AppMessageListenerHandle] is dropped."]
#[pin_data(PinnedDrop)]
pub struct AppMessageListenerHandle<
    'handle,
    FInboxReceived,
    FInboxDropped,
    FOutboxSent,
    FOutboxFailed,
> {
    #[pin]
    callback_inbox_received: FInboxReceived,

    #[pin]
    callback_inbox_dropped: FInboxDropped,

    #[pin]
    callback_outbox_sent: FOutboxSent,

    #[pin]
    callback_outbox_failed: FOutboxFailed,

    entry: AppMessageEntry,

    app_messages: NonNull<AppMessages>,

    #[pin]
    _pin_phantom: PhantomPinned,

    _phantom: PhantomData<&'handle mut ()>,
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

#[pinned_drop]
impl<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed> PinnedDrop
    for AppMessageListenerHandle<'handle, FInboxReceived, FInboxDropped, FOutboxSent, FOutboxFailed>
{
    fn drop(self: Pin<&mut Self>) {
        let project = self.project();

        unsafe {
            project.app_messages.as_ref().listeners.with_mut(|l| {
                l.remove(NonNull::from_mut(project.entry));
            });
        }
    }
}

unsafe extern "C" fn received_callback(
    iterator: *mut bindings::DictionaryIterator,
    context: *mut core::ffi::c_void,
) {
    let root_dict = unsafe { *NonNull::new(iterator).unwrap().as_ptr() };
    let app_messages = context as *mut AppMessages;

    unsafe {
        (*app_messages).listeners.with_mut(|l| {
            for entry in l.iter_mut() {
                let mut dict = root_dict.clone();
                let dict_ref = crate::dictionary::DictionaryRef::new(NonNull::from_mut(&mut dict));
                (*entry.callback_inbox_received)(dict_ref);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn dropped_callback(reason: AppMessageResult, context: *mut core::ffi::c_void) {
    let app_messages = context as *mut AppMessages;

    unsafe {
        (*app_messages).listeners.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.callback_inbox_dropped)(reason);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn sent_callback(
    iterator: *mut bindings::DictionaryIterator,
    context: *mut core::ffi::c_void,
) {
    let root_dict = unsafe { *NonNull::new(iterator).unwrap().as_ptr() };
    let app_messages = context as *mut AppMessages;

    unsafe {
        (*app_messages).listeners.with_mut(|l| {
            for entry in l.iter_mut() {
                let mut dict = root_dict.clone();
                let dict_ref = crate::dictionary::DictionaryRef::new(NonNull::from_mut(&mut dict));
                (*entry.callback_outbox_sent)(dict_ref);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn failed_callback(
    iterator: *mut bindings::DictionaryIterator,
    reason: AppMessageResult,
    context: *mut core::ffi::c_void,
) {
    let root_dict = unsafe { *NonNull::new(iterator).unwrap().as_ptr() };
    let app_messages = context as *mut AppMessages;

    unsafe {
        (*app_messages).listeners.with_mut(|l| {
            for entry in l.iter_mut() {
                let mut dict = root_dict.clone();
                let dict_ref = crate::dictionary::DictionaryRef::new(NonNull::from_mut(&mut dict));
                (*entry.callback_outbox_failed)(dict_ref, reason);
            }
        });
    }

    unsafe { crate::executor::poll_executor() };
}
