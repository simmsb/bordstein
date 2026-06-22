// use core::fmt::{self, Write};

// use heapless::{CString, LenType};

// use crate::bindings::AppLogLevel;
// use crate::single_core_cell::SingleCoreCell;

// pub struct CStringWriter<'a, const N: usize, T: LenType = usize> {
//     pub buf: &'a mut CString<N, T>,
// }

// impl<const N: usize, T: LenType> Write for CStringWriter<'_, N, T> {
//     fn write_str(&mut self, s: &str) -> fmt::Result {
//         let used = self.buf.as_bytes().len();
//         let space = N.saturating_sub(1).saturating_sub(used);
//         let take = s.len().min(space);
//         if take > 0 {
//             let _ = self.buf.extend_from_bytes(&s.as_bytes()[..take]);
//         }
//         Ok(())
//     }
// }

// struct LogBuffers {
//     path: heapless::CString<32, u8>,
//     buf: heapless::CString<128, u8>,
// }

#[cfg(feature = "logging")]
use core::mem::MaybeUninit;

#[cfg(feature = "logging")]
use crate::single_core_cell::SingleCoreCell;

#[cfg(feature = "logging")]
static LOG_BUFFER: SingleCoreCell<MaybeUninit<heapless::CString<128>>> =
    SingleCoreCell::new(MaybeUninit::uninit());

#[doc(hidden)]
pub fn _with_log_buf(f: impl FnOnce(&mut heapless::CString<128>)) {
    unsafe {
        LOG_BUFFER.with_mut(|mbuf| {
            let buf = mbuf.write(heapless::CString::new());
            f(buf);
        });
    }
}

// pub fn log_at(level: AppLogLevel, file: &CStr, line: u32, ) {
//     #[cfg(feature = "logging")]
//     {
//         let mut path = heapless::CString::<32>::new();
//         let mut buf = heapless::CString::<64>::new();

//         let mut writer = CStringWriter { buf: &mut buf };
//         let _ = writer.write_fmt(args);

//         let mut path_writer = CStringWriter { buf: &mut path };
//         let _ = write!(path_writer, "{}", file);

//         unsafe {
//             crate::bindings::app_log(
//                 level as u8,
//                 path.as_ptr().cast(),
//                 line as i32,
//                 buf.as_ptr().cast(),
//             );
//         }
//     }
// }

#[macro_export]
macro_rules! log {
    ($level:expr, $file:expr, $line:expr, $($arg:tt)*) => {
        #[cfg(feature = "logging")]
        {
            crate::log_impl::_with_log_buf(|buf| {
                let _ = ::ufmt::uwrite!(buf, $($arg)*);
                #[allow(unused_unsafe)]
                unsafe {
                    $crate::bindings::app_log(
                        $level as u8,
                        $file,
                        $line,
                        buf.as_c_str().as_ptr(),
                    );
                }
            });
        }
    };
}

// pub fn init() {
//     #[cfg(feature = "logging")]
//     {
//         LOG_BUFFERS.with_mut(|b| {
//             *b = Some(LogBuffers {
//                 path: heapless::CString::new(),
//                 buf: heapless::CString::new(),
//             })
//         });
//     }
// }

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log!(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_ERROR,
            concat!(file!(), "\0").as_ptr() as *const core::ffi::c_char,
            line!() as core::ffi::c_int,
            $($arg)*,
        )
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log!(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_WARNING,
            concat!(file!(), "\0").as_ptr() as *const core::ffi::c_char,
            line!() as core::ffi::c_int,
            $($arg)*,
        )
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log!(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_INFO,
            concat!(file!(), "\0").as_ptr() as *const core::ffi::c_char,
            line!() as core::ffi::c_int,
            $($arg)*,
        )
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log!(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_DEBUG,
            concat!(file!(), "\0").as_ptr() as *const core::ffi::c_char,
            line!() as core::ffi::c_int,
            $($arg)*,
        )
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log!(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_DEBUG_VERBOSE,
            concat!(file!(), "\0").as_ptr() as *const core::ffi::c_char,
            line!() as core::ffi::c_int,
            $($arg)*,
        )
    };
}
