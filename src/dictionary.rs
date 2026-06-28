use core::{ffi::CStr, marker::PhantomData, mem::MaybeUninit, ptr::NonNull};

use crate::bindings::{self, DictionaryResult, TupleType};

pub struct DictionaryRef<'dictionary> {
    inner: NonNull<bindings::DictionaryIterator>,

    _phantom: PhantomData<&'dictionary mut ()>,
}

impl<'dictionary> DictionaryRef<'dictionary> {
    pub(crate) fn new(inner: NonNull<bindings::DictionaryIterator>) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }

    pub fn iter(&self) -> DictionaryIterator<'dictionary> {
        DictionaryIterator::new(unsafe { *self.inner.as_ptr() })
    }
}

pub struct Dictionary<'buffer> {
    inner: bindings::DictionaryIterator,

    buffer_len: u16,

    _phantom: PhantomData<&'buffer ()>,
}

impl<'buffer> Dictionary<'buffer> {
    pub fn buffer(&self) -> &'buffer [u8] {
        let ptr = self.inner.dictionary as *const u8;

        unsafe { core::slice::from_raw_parts(ptr, self.buffer_len as usize) }
    }
}

#[repr(transparent)]
pub struct DictionaryWriter<'buffer> {
    inner: bindings::DictionaryIterator,

    _phantom: PhantomData<&'buffer mut ()>,
}

impl<'buffer> DictionaryWriter<'buffer> {
    pub fn new(buf: &'buffer mut [u8]) -> Result<Self, DictionaryResult> {
        let mut inner = MaybeUninit::<bindings::DictionaryIterator>::uninit();

        let len = buf.len().truncate::<u16>();

        unsafe {
            bindings::dict_write_begin(inner.as_mut_ptr(), buf.as_ptr() as *mut _, len)
                .into_result()?;
        }

        Ok(Self {
            inner: unsafe { inner.assume_init() },
            _phantom: PhantomData,
        })
    }

    // pub(crate) fn from_raw(inner: bindings::DictionaryIterator) -> Self {
    //     Self {
    //         inner,
    //         _phantom: PhantomData,
    //     }
    // }

    pub fn data(&mut self, key: u32, data: &[u8]) -> Result<(), DictionaryResult> {
        unsafe {
            bindings::dict_write_data(&raw mut self.inner, key, data.as_ptr(), data.len() as u16)
                .into_result()
        }
    }

    pub fn cstring(&mut self, key: u32, cstring: &CStr) -> Result<(), DictionaryResult> {
        unsafe {
            bindings::dict_write_cstring(&raw mut self.inner, key, cstring.as_ptr()).into_result()
        }
    }

    pub fn u8(&mut self, key: u32, val: u8) -> Result<(), DictionaryResult> {
        unsafe { bindings::dict_write_uint8(&raw mut self.inner, key, val).into_result() }
    }

    pub fn u16(&mut self, key: u32, val: u16) -> Result<(), DictionaryResult> {
        unsafe { bindings::dict_write_uint16(&raw mut self.inner, key, val).into_result() }
    }

    pub fn u32(&mut self, key: u32, val: u32) -> Result<(), DictionaryResult> {
        unsafe { bindings::dict_write_uint32(&raw mut self.inner, key, val).into_result() }
    }

    pub fn i8(&mut self, key: u32, val: i8) -> Result<(), DictionaryResult> {
        unsafe { bindings::dict_write_int8(&raw mut self.inner, key, val).into_result() }
    }

    pub fn i16(&mut self, key: u32, val: i16) -> Result<(), DictionaryResult> {
        unsafe { bindings::dict_write_int16(&raw mut self.inner, key, val).into_result() }
    }

    pub fn i32(&mut self, key: u32, val: i32) -> Result<(), DictionaryResult> {
        unsafe { bindings::dict_write_int32(&raw mut self.inner, key, val).into_result() }
    }

    pub fn finish(mut self) -> Option<Dictionary<'buffer>> {
        let buffer_len = unsafe { bindings::dict_write_end(&raw mut self.inner) as u16 };

        if buffer_len == 0 {
            return None;
        }

        Some(Dictionary {
            inner: self.inner,
            buffer_len,
            _phantom: PhantomData,
        })
    }
}

impl bindings::DictionaryResult {
    fn into_result(self) -> Result<(), Self> {
        if self == Self::DICT_OK {
            Ok(())
        } else {
            Err(self)
        }
    }
}

pub struct Tuple<'dictionary> {
    inner: NonNull<bindings::Tuple>,

    _phantom: PhantomData<&'dictionary ()>,
}

/// Helper type that makes the inner field unaligned.
#[repr(Rust, packed)]
struct Unaligned<T>(T);

impl<'dictionary> Tuple<'dictionary> {
    fn from_ptr(ptr: NonNull<bindings::Tuple>) -> Self {
        Self {
            inner: ptr,
            _phantom: PhantomData,
        }
    }

    pub fn key(&self) -> u32 {
        unsafe { (*self.inner.as_ptr()).key }
    }

    pub fn value(&self) -> TupleValue<'dictionary> {
        let type_ = unsafe { (*self.inner.as_ptr()).type_() };
        let length = unsafe { (*self.inner.as_ptr()).length };
        let val = unsafe { &raw const (*self.inner.as_ptr()).value };

        // NOTE: SDK docs mention the integer values are little endian. I'm
        // unsure if they mean that the architecture of pebble watches are
        // always little endian, or that the sdk is always just coercing a
        // little endian encoded byte array to the integer type.
        //
        // NOTE: we're doing raw casts here because bindgen generates a 4-byte
        // aligned value union for Tuple, which is incorrect unless the SDK is
        // doing some fancy stuff to ensure the value is aligned.
        match (type_, length) {
            (TupleType::TUPLE_UINT, 1) => {
                TupleValue::Uint8(unsafe { (*(val as *const Unaligned<u8>)).0 })
            }
            (TupleType::TUPLE_UINT, 2) => {
                TupleValue::Uint16(unsafe { (*(val as *const Unaligned<u16>)).0 })
            }
            (TupleType::TUPLE_UINT, 4) => {
                TupleValue::Uint32(unsafe { (*(val as *const Unaligned<u32>)).0 })
            }
            (TupleType::TUPLE_INT, 1) => {
                TupleValue::Int8(unsafe { (*(val as *const Unaligned<i8>)).0 })
            }
            (TupleType::TUPLE_INT, 2) => {
                TupleValue::Int16(unsafe { (*(val as *const Unaligned<i16>)).0 })
            }
            (TupleType::TUPLE_INT, 4) => {
                TupleValue::Int32(unsafe { (*(val as *const Unaligned<i32>)).0 })
            }
            (TupleType::TUPLE_CSTRING, _) => {
                TupleValue::String(unsafe { CStr::from_ptr(val as *const u8) })
            }
            (TupleType::TUPLE_BYTE_ARRAY, len) => TupleValue::ByteArray(unsafe {
                core::slice::from_raw_parts(val as *const u8, len as usize)
            }),
            _ => panic!("Invalid tuple with invalid size for integer"),
        }
    }
}

/// A decoded tuple value.
///
/// Note, integers from the phone are always i32.
pub enum TupleValue<'dictionary> {
    Uint32(u32),
    Uint16(u16),
    Uint8(u8),
    Int32(i32),
    Int16(i16),
    Int8(i8),
    String(&'dictionary CStr),
    ByteArray(&'dictionary [u8]),
}

/// A dictionary iterator that works like a normal rust iterator.
pub struct DictionaryIterator<'dictionary> {
    iterator: bindings::DictionaryIterator,
    started: bool,
    _phantom: PhantomData<&'dictionary ()>,
}

impl<'dictionary> DictionaryIterator<'dictionary> {
    fn new(iterator: bindings::DictionaryIterator) -> Self {
        Self {
            iterator,
            started: false,
            _phantom: PhantomData,
        }
    }
}

impl<'dictionary> Iterator for DictionaryIterator<'dictionary> {
    type Item = Tuple<'dictionary>;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = if self.started {
            unsafe { bindings::dict_read_next(&raw mut self.iterator) }
        } else {
            unsafe { bindings::dict_read_first(&raw mut self.iterator) }
        };

        let ptr = NonNull::new(ptr)?;

        Some(Tuple::from_ptr(ptr))
    }
}
