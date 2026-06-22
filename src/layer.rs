use core::{
    marker::{PhantomData, PhantomPinned},
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::NonNull,
};

use pin_init::{PinInit, pin_init, pinned_drop};

use crate::{
    bindings::{self, GRect, layer_get_data},
    graphics_context::GContext,
};

unsafe extern "C" fn layer_callback(layer: *mut bindings::Layer, context: *mut bindings::GContext) {
    let layer = LayerRef::from_ptr(NonNull::new(layer).unwrap());

    let context = GContext::new(NonNull::new(context).unwrap());

    let cb =
        unsafe { bindings::layer_get_data(layer.inner.inner.as_ptr()) as *mut LayerCallbackVTable };

    unsafe {
        (**cb)(layer, context);
    }
}

/// As [Layer], but isn't owned and therefore doesn't destroy the layer on drop.
pub struct LayerRef<'a> {
    inner: core::mem::ManuallyDrop<Layer<'a>>,
}
impl<'a> LayerRef<'a> {
    pub(crate) fn from_ptr(ptr: NonNull<bindings::Layer>) -> Self {
        Self {
            inner: core::mem::ManuallyDrop::new(Layer::from_ptr(ptr)),
        }
    }
}

impl<'a> Deref for LayerRef<'a> {
    type Target = Layer<'a>;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl<'a> DerefMut for LayerRef<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.inner
    }
}

/// A layer. The lifetime is used to track children.
pub struct Layer<'a> {
    inner: NonNull<bindings::Layer>,

    _phantom: PhantomData<&'a ()>,
}

/// A layer with an attached update function. This needs to be pinned in order
/// to have a stable reference to the callback data.
#[pin_init::pin_data]
pub struct LayerWithCallback<'a, F> {
    inner: Layer<'a>,

    callback: F,

    _phantom: PhantomData<&'a ()>,

    #[pin]
    _pin_phantom: PhantomPinned,
}

impl<'a, F> Deref for LayerWithCallback<'a, F> {
    type Target = Layer<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, F> DerefMut for LayerWithCallback<'a, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// fn ensure_thin<T>(_: &T) {
//     const {
//         assert!(size_of::<T>() == size_of::<usize>());
//     }
// }

type LayerCallbackVTable = *mut (dyn for<'cb> FnMut(LayerRef<'cb>, GContext<'cb>) + 'static);

impl<'a> Layer<'a> {
    pub(crate) fn new(frame: GRect) -> Option<Self> {
        let ptr =
            unsafe { bindings::layer_create_with_data(frame, size_of::<LayerCallbackVTable>()) };
        NonNull::new(ptr).map(Self::from_ptr)
    }

    pub(crate) fn from_ptr(ptr: NonNull<bindings::Layer>) -> Self {
        Layer {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    pub fn new_child<'child: 'a>(&self, frame: GRect) -> Option<Layer<'child>> {
        let child: Layer<'_> = Layer::new(frame)?;
        unsafe {
            bindings::layer_add_child(self.inner.as_ptr(), child.inner.as_ptr());
        }
        Some(child)
    }

    pub fn mark_dirty(&mut self) {
        unsafe {
            bindings::layer_mark_dirty(self.inner.as_ptr());
        }
    }

    pub fn frame(&self) -> GRect {
        unsafe { bindings::layer_get_frame(self.inner.as_ptr()) }
    }

    pub fn set_frame(&mut self, frame: GRect) {
        unsafe {
            bindings::layer_set_frame(self.inner.as_ptr(), frame);
        }
    }

    pub fn bounds(&self) -> GRect {
        unsafe { bindings::layer_get_bounds(self.inner.as_ptr()) }
    }

    pub fn set_bounds(&mut self, bounds: GRect) {
        unsafe {
            bindings::layer_set_bounds(self.inner.as_ptr(), bounds);
        }
    }

    pub fn with_callback<F>(self, callback: F) -> impl PinInit<LayerWithCallback<'a, F>>
    where
        F: for<'cb> FnMut(LayerRef<'cb>, GContext<'cb>) + 'a,
    {
        pin_init!(LayerWithCallback {
            inner: self,
            callback,
            _phantom: PhantomData,
            _pin_phantom: PhantomPinned,
        })
        .pin_chain(|p| {
            unsafe {
                let project = p.project();

                let callback_vtable =
                    project.callback as *mut (dyn for<'cb> FnMut(LayerRef<'cb>, GContext<'cb>) + 'a);

                // N.B. this erases the lifetimes of the closure captures
                let callback_vtable_static =
                    core::mem::transmute::<_, LayerCallbackVTable>(callback_vtable);

                // Pointer to the fat dyn pointer
                let cb = layer_get_data(project.inner.inner.as_ptr()) as *mut LayerCallbackVTable;
                cb.write(callback_vtable_static);

                bindings::layer_set_update_proc(project.inner.inner.as_ptr(), Some(layer_callback));
            }

            Ok(())
        })
    }
}

impl<'a> Drop for Layer<'a> {
    fn drop(&mut self) {
        crate::debug!("Dropping layer {:?}", self.inner.as_ptr());

        unsafe {
            bindings::layer_destroy(self.inner.as_ptr());
        }
    }
}
