use core::{marker::PhantomPinned, pin::Pin, ptr::NonNull};

use cordyceps::{Linked, List, list::Links};
use pin_init::{PinInit, pin_data, pinned_drop};

use crate::{
    bindings::{self, AnimationProgress, GRect, UnobstructedAreaHandlers},
    single_core_cell::SingleCoreCell,
};

struct UnobstructedAreaServiceEntry {
    links: Links<UnobstructedAreaServiceEntry>,

    will_change: *mut UnobstructedAreaWillChangeHandlerVTable,
    change: *mut UnobstructedAreaChangeHandlerVTable,
    did_change: *mut UnobstructedAreaDidChangeHandlerVTable,
}

unsafe impl Linked<Links<UnobstructedAreaServiceEntry>> for UnobstructedAreaServiceEntry {
    type Handle = NonNull<UnobstructedAreaServiceEntry>;

    fn into_ptr(r: Self::Handle) -> core::ptr::NonNull<Self> {
        r
    }

    unsafe fn from_ptr(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        ptr
    }

    unsafe fn links(
        ptr: core::ptr::NonNull<Self>,
    ) -> core::ptr::NonNull<Links<UnobstructedAreaServiceEntry>> {
        let target = ptr.as_ptr();

        unsafe {
            let links = core::ptr::addr_of_mut!((*target).links);

            NonNull::new_unchecked(links)
        }
    }
}

pub trait UnobstructedAreaWillChangeHandler<'env> = FnMut(GRect) + 'env;
pub trait UnobstructedAreaChangeHandler<'env> = FnMut(AnimationProgress) + 'env;
pub trait UnobstructedAreaDidChangeHandler<'env> = FnMut() + 'env;

pub(crate) type UnobstructedAreaWillChangeHandlerVTable =
    dyn UnobstructedAreaWillChangeHandler<'static>;
pub(crate) type UnobstructedAreaChangeHandlerVTable = dyn UnobstructedAreaChangeHandler<'static>;
pub(crate) type UnobstructedAreaDidChangeHandlerVTable =
    dyn UnobstructedAreaDidChangeHandler<'static>;

/// Represents an active subscription. When this is dropped the callback will be deregistered.
#[must_use = "Callback is deregistered and dropped when [UnobstructedAreaServiceHandle] is dropped"]
#[pin_data(PinnedDrop)]
pub struct UnobstructedAreaServiceHandle<FWillChange, FChange, FDidChange> {
    #[pin]
    will_change: FWillChange,

    #[pin]
    change: FChange,

    #[pin]
    did_change: FDidChange,

    entry: UnobstructedAreaServiceEntry,

    #[pin]
    _pin_phantom: PhantomPinned,
}

static LIST: SingleCoreCell<List<UnobstructedAreaServiceEntry>> = SingleCoreCell::new(List::new());

/// Listen to unobstructed_area events, the given callback will be called on unobstructed_area events
/// matching the passed time units.
///
/// When the returned [UnobstructedAreaServiceHandle] is dropped, the callback will be
/// deregistered and the closure dropped.
///
/// NOTE: You can create multiple unobstructed_area event listeners from multiple locations,
/// the library handles this elegantly using an intrusive linked list of
/// stack-allocated nodes. The unobstructed_area service is automatically re-registered as
/// listeners are added and removed.
///
/// This returns a [PinInit] as we need to pass the pebble SDK a pointer to the
/// closure passed in, if [UnobstructedAreaServiceHandle] could move, it would invalidate
/// this reference.
///
/// Use [pin_init::stack_pin_init] to allocate the result of this method in your
/// stack frame.
#[must_use = "Callback is deregistered and dropped when [UnobstructedAreaServiceHandle] is dropped"]
pub fn listen<FWillChange, FChange, FDidChange>(
    will_change: FWillChange,
    change: FChange,
    did_change: FDidChange,
) -> impl PinInit<UnobstructedAreaServiceHandle<FWillChange, FChange, FDidChange>>
where
    FWillChange: for<'tm> FnMut(GRect),
    FChange: for<'tm> FnMut(AnimationProgress),
    FDidChange: for<'tm> FnMut(),
{
    pin_init::pin_init!(&this in UnobstructedAreaServiceHandle {
        will_change,
        change,
        did_change,

        entry: UnobstructedAreaServiceEntry {
            links: Links::default(),
            will_change: unsafe { core::mem::transmute::<_, *mut UnobstructedAreaWillChangeHandlerVTable>(&raw mut (*this.as_ptr()).will_change as *mut dyn UnobstructedAreaWillChangeHandler<'_>) },
            change: unsafe { core::mem::transmute::<_, *mut UnobstructedAreaChangeHandlerVTable>(&raw mut (*this.as_ptr()).change as *mut dyn UnobstructedAreaChangeHandler<'_>) },
            did_change: unsafe { core::mem::transmute::<_, *mut UnobstructedAreaDidChangeHandlerVTable>(&raw mut (*this.as_ptr()).did_change as *mut dyn UnobstructedAreaDidChangeHandler<'_>) },
        },

        _pin_phantom: PhantomPinned,
    }).pin_chain(|p| {
        let project = p.project();

        unsafe {
            LIST.with_mut(|l| {
                l.push_front(NonNull::from_mut(project.entry));

                re_register_callback(l);
            });
        }

        Ok(())
    })
}

unsafe fn re_register_callback(list: &mut List<UnobstructedAreaServiceEntry>) {
    if list.is_empty() {
        unsafe {
            bindings::unobstructed_area_service_unsubscribe();

            return;
        }
    }

    unsafe {
        bindings::unobstructed_area_service_subscribe(
            UnobstructedAreaHandlers {
                will_change: Some(unobstructed_area_will_change_callback),
                change: Some(unobstructed_area_change_callback),
                did_change: Some(unobstructed_area_did_change_callback),
            },
            core::ptr::null_mut(),
        );
    }
}

#[pinned_drop]
impl<F, G, H> PinnedDrop for UnobstructedAreaServiceHandle<F, G, H> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            LIST.with_mut(|l| {
                l.remove(NonNull::from_mut(self.project().entry));

                re_register_callback(l);
            });
        }
    }
}

unsafe extern "C" fn unobstructed_area_will_change_callback(
    final_unobstructed_screen_area: GRect,
    _ctx: *mut core::ffi::c_void,
) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.will_change)(final_unobstructed_screen_area);
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}

unsafe extern "C" fn unobstructed_area_change_callback(
    progress: AnimationProgress,
    _ctx: *mut core::ffi::c_void,
) {
    unsafe {
        LIST.with_mut(|l| {
            for entry in l.iter_mut() {
                (*entry.change)(progress);
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
                (*entry.did_change)();
            }
        })
    };

    // one of the closures might have woken a waker, so poll once afterwards
    unsafe { crate::executor::poll_executor() };
}
