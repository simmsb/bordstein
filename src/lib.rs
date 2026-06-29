#![no_std]
#![feature(integer_casts)]
#![feature(integer_widen_truncate)]
#![feature(trait_alias)]
#![feature(atomic_ptr_null)]
#![feature(type_alias_impl_trait)]
#![feature(int_roundings)]

pub mod app_message;
pub mod colour;
pub mod dictionary;
pub mod events;
pub mod executor;
pub mod font;
pub mod graphics_context;
pub mod layers;
pub mod log_impl;
pub mod resources;
pub mod shapes;
pub mod single_core_cell;
pub mod time;
pub mod time_driver;
pub mod utils;
pub mod window;
pub mod storage;

pub use layers::IsLayer;

pub mod bindings {
    #![allow(warnings)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub struct PebbleServices {
    pub accelerometer: events::accelerometer::AccelerometerService,
    pub app_messages: app_message::AppMessages,
}

impl PebbleServices {
    #[doc(hidden)]
    pub unsafe fn steal() -> Self {
        unsafe {
            Self {
                accelerometer: events::accelerometer::AccelerometerService::steal(),
                app_messages: app_message::AppMessages::steal(),
            }
        }
    }
}

#[macro_export]
/// Create the main function, and specify which async function should be called.
///
/// # Example
///
/// ```rs
/// main!(my_async_main);
///
/// #[embassy_executor::task]
/// async fn my_async_main(services: PebbleServices, spawner: embassy_executor::Spawner) {
///   // ...
/// }
/// ```
macro_rules! main {
    ($main_fn:ident) => {
        fn init(s: embassy_executor::Spawner) {
            s.spawn($main_fn(unsafe { $crate::PebbleServices::steal() }, s).unwrap());
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn main() {
            $crate::executor::init();
            $crate::executor::run(init);
        }
    };
}

// extern, no_mangle so we can set a breakpoint
#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn trigger_panic() -> ! {
    unsafe {
        bindings::exit_reason_set(bindings::AppExitReason::APP_EXIT_NOT_SPECIFIED);
        bindings::window_stack_pop_all(false);

        bindings::app_event_loop();
    };

    // unsafe {
    //     let crash: *mut u32 = core::ptr::null_mut();
    //     core::ptr::write_volatile(crash, 0xDEADBEEF);
    // }

    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let msg = info.message().as_str().unwrap_or("<no message>");
    crate::error!("Panic! {}", msg);
    crate::error!("{}:{}", info.location().map(|l| l.file()).unwrap_or(""), info.location().map(|l| l.line()).unwrap_or(0));
    trigger_panic();
}
