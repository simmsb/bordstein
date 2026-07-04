use core::ptr::NonNull;

use cordyceps::List;
use pin_init::PinInit;

use crate::{
    bindings::{self, UnobstructedAreaHandlers},
    multi_registration_listener::{
        Entry, Handle, MultiRegistrationListRoot, MultiRegistrationListener,
        MultiRegistrationService,
    },
    single_core_cell::SingleCoreCell,
};

pub trait UnobstructedAreaWillChangeHandler<'env> = FnMut(bindings::GRect) + 'env;
pub trait UnobstructedAreaChangeHandler<'env> = FnMut(bindings::AnimationProgress) + 'env;
pub trait UnobstructedAreaDidChangeHandler<'env> = FnMut() + 'env;

pub(crate) type UnobstructedAreaWillChangeHandlerVTable =
    dyn UnobstructedAreaWillChangeHandler<'static>;
pub(crate) type UnobstructedAreaChangeHandlerVTable = dyn UnobstructedAreaChangeHandler<'static>;
pub(crate) type UnobstructedAreaDidChangeHandlerVTable =
    dyn UnobstructedAreaDidChangeHandler<'static>;

pub struct UnobstructedAreaService {
    _private: (),
}

impl UnobstructedAreaService {
    /// Listen to unobstructed_area events, the given callback will be called on unobstructed_area events
    /// matching the passed time units.
    ///
    /// When the returned [Handle] is dropped, the callback will be
    /// deregistered and the closure dropped.
    ///
    /// NOTE: You can create multiple unobstructed_area event listeners from multiple locations,
    /// the library handles this elegantly using an intrusive linked list of
    /// stack-allocated nodes. The unobstructed_area service is automatically re-registered as
    /// listeners are added and removed.
    ///
    /// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
    /// closure passed in, if [Handle] could move, it would invalidate
    /// this reference.
    ///
    /// Use [pin_init::stack_pin_init] to allocate the result of this method in your
    /// stack frame.
    pub fn listen<FWillChange, FChange, FDidChange>(
        will_change: FWillChange,
        change: FChange,
        did_change: FDidChange,
    ) -> impl PinInit<Handle<'static, UnobstructedAreaServiceListener<FWillChange, FChange, FDidChange>>>
    where
        FWillChange: for<'tm> FnMut(bindings::GRect),
        FChange: for<'tm> FnMut(bindings::AnimationProgress),
        FDidChange: for<'tm> FnMut(),
    {
        Handle::init(
            UnobstructedAreaServiceListener {
                will_change,
                change,
                did_change,
            },
            (),
            const { &UnobstructedAreaService { _private: () } },
        )
    }
}

pub struct UnobstructedAreaPointers {
    will_change: NonNull<UnobstructedAreaWillChangeHandlerVTable>,
    change: NonNull<UnobstructedAreaChangeHandlerVTable>,
    did_change: NonNull<UnobstructedAreaDidChangeHandlerVTable>,
}

static LIST: SingleCoreCell<List<Entry<UnobstructedAreaPointers>>> =
    SingleCoreCell::new(List::new());

impl MultiRegistrationService for UnobstructedAreaService {
    type CallbackData = UnobstructedAreaPointers;

    fn list(&self) -> &MultiRegistrationListRoot<Self::CallbackData> {
        &LIST
    }
}

pub struct UnobstructedAreaServiceListener<FWillChange, FChange, FDidChange> {
    will_change: FWillChange,
    change: FChange,
    did_change: FDidChange,
}

impl<'env, FWillChange, FChange, FDidChange> MultiRegistrationListener
    for UnobstructedAreaServiceListener<FWillChange, FChange, FDidChange>
where
    FWillChange: UnobstructedAreaWillChangeHandler<'env>,
    FChange: UnobstructedAreaChangeHandler<'env>,
    FDidChange: UnobstructedAreaDidChangeHandler<'env>,
{
    type Service = UnobstructedAreaService;
    type Extra = ();

    unsafe fn extract(
        self_: NonNull<Self>,
        _extra: Self::Extra,
    ) -> <Self::Service as MultiRegistrationService>::CallbackData {
        unsafe {
            let will_change = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).will_change
                    as *mut dyn UnobstructedAreaWillChangeHandler<'_>,
            ));
            let change = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).change as *mut dyn UnobstructedAreaChangeHandler<'_>,
            ));
            let did_change = NonNull::new_unchecked(core::mem::transmute(
                &raw mut (*self_.as_ptr()).did_change
                    as *mut dyn UnobstructedAreaDidChangeHandler<'_>,
            ));

            UnobstructedAreaPointers {
                will_change,
                change,
                did_change,
            }
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
                    bindings::unobstructed_area_service_unsubscribe();

                    return;
                }

                bindings::unobstructed_area_service_subscribe(
                    UnobstructedAreaHandlers {
                        will_change: Some(unobstructed_area_will_change_callback),
                        change: Some(unobstructed_area_change_callback),
                        did_change: Some(unobstructed_area_did_change_callback),
                    },
                    core::ptr::null_mut(),
                );
            });
        }
    }
}

unsafe extern "C" fn unobstructed_area_will_change_callback(
    final_unobstructed_screen_area: bindings::GRect,
    _ctx: *mut core::ffi::c_void,
) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.data.will_change.as_ptr())(final_unobstructed_screen_area);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn unobstructed_area_change_callback(
    progress: bindings::AnimationProgress,
    _ctx: *mut core::ffi::c_void,
) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.data.change.as_ptr())(progress);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn unobstructed_area_did_change_callback(_ctx: *mut core::ffi::c_void) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.data.did_change.as_ptr())();
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
