use core::fmt::{self, Write};

use heapless::{CString, LenType};

use crate::bindings::AppLogLevel;
use crate::single_core_cell::SingleCoreCell;

pub struct CStringWriter<'a, const N: usize, T: LenType = usize> {
    pub buf: &'a mut CString<N, T>,
}

impl<const N: usize, T: LenType> Write for CStringWriter<'_, N, T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let used = self.buf.as_bytes().len();
        let space = N.saturating_sub(1).saturating_sub(used);
        let take = s.len().min(space);
        if take > 0 {
            let _ = self.buf.extend_from_bytes(&s.as_bytes()[..take]);
        }
        Ok(())
    }
}

struct LogBuffers {
    path: heapless::CString<32, u8>,
    buf: heapless::CString<128, u8>,
}

#[cfg(feature = "logging")]
static LOG_BUFFERS: SingleCoreCell<Option<LogBuffers>> = SingleCoreCell::new(None);

pub fn log_at(level: AppLogLevel, file: &str, line: u32, args: fmt::Arguments<'_>) {
    #[cfg(feature = "logging")]
    {
        LOG_BUFFERS.with_mut(|b| {
            let Some(LogBuffers { path, buf }) = b else {
                return;
            };

            *path = heapless::CString::new();
            *buf = heapless::CString::new();

            let mut writer = CStringWriter { buf };
            let _ = writer.write_fmt(args);

            let mut path_writer = CStringWriter { buf: path };
            let _ = write!(path_writer, "{}", file);

            unsafe {
                crate::bindings::app_log(
                    level as u8,
                    path.as_ptr().cast(),
                    line as i32,
                    buf.as_ptr().cast(),
                );
            }
        });
    }
}

pub fn init() {
    #[cfg(feature = "logging")]
    {
        LOG_BUFFERS.with_mut(|b| {
            *b = Some(LogBuffers {
                path: heapless::CString::new(),
                buf: heapless::CString::new(),
            })
        });
    }
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log_impl::log_at(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_ERROR,
            file!(),
            line!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log_impl::log_at(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_WARNING,
            file!(),
            line!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log_impl::log_at(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_INFO,
            file!(),
            line!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log_impl::log_at(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_DEBUG,
            file!(),
            line!(),
            format_args!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "logging")]
        $crate::log_impl::log_at(
            crate::bindings::AppLogLevel::APP_LOG_LEVEL_DEBUG_VERBOSE,
            file!(),
            line!(),
            format_args!($($arg)*),
        )
    };
}
