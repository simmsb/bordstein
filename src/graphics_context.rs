use core::{marker::PhantomData, ptr::NonNull};

use crate::bindings;

pub struct GContext<'a> {
    inner: NonNull<bindings::GContext>,

    _phantom: PhantomData<&'a ()>,
}

impl<'a> GContext<'a> {
    pub(crate) fn new(raw: NonNull<bindings::GContext>) -> Self {
        Self {
            inner: raw,
            _phantom: PhantomData,
        }
    }
}
