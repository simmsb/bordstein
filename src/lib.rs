#![no_std]
#![feature(integer_casts)]
#![feature(integer_widen_truncate)]
#![feature(trait_alias)]
#![feature(atomic_ptr_null)]
#![feature(type_alias_impl_trait)]
#![feature(int_roundings)]
#![feature(fn_traits)]
#![feature(unboxed_closures)]

#[doc(hidden)]
pub mod log_impl;

#[doc(hidden)]
pub mod executor;

mod single_core_cell;
mod time_driver;

/// Manage pebble backlight
pub mod light;

/// Access to app messages.
pub mod app_message;

/// Utility methods on colours.
pub mod colour;

/// Bindings for dictionaries, for both reading and writing.
pub mod dictionary;

/// Subscribe to PebbleOS events.
pub mod events;

/// Access to system and user fonts.
pub mod font;

/// Access to the graphics context and other drawing methods.
pub mod graphics_context;

/// Create and manage layers.
pub mod layers;

/// Bindings for the resource api.
pub mod resources;

/// Utility methods for points, sizes, and rectangles.
pub mod shapes;

/// Date and time related utilitie.s
pub mod time;

pub mod utils;

/// Create windows.
pub mod window;

/// Access to the storage api.
pub mod storage;

pub mod multi_registration_listener;
/// Manage vibration motor
pub mod vibes;

/// Bitmaps
pub mod bitmap;

/// App glance
pub mod glance;

pub mod prelude {
    pub use crate::dictionary::*;
    pub use crate::events::{
        accelerometer::AccelerometerService, battery_state::BatteryService,
        connection::ConnectionService, health::HealthService, tick::TickService,
        unobstructed_area::UnobstructedAreaService,
    };
    pub use crate::font::Font;
    pub use crate::font::system as system_fonts;
    pub use crate::graphics_context::GContext;
    pub use crate::layers::{
        AsChildLayer, IsLayer, Layer, LayerMut, LayerRef, StatusBarLayer, TextLayer,
    };
    pub use crate::resources::Resource;
    pub use crate::time::{Datetime, Timestamp};
    pub use crate::window::*;
}

/// Access to the automatically generated bindings to the pebble SDK.
///
/// This is generated from the SDK in use at the time of building, so your
/// resource and message IDs will also be in here.
///
/// NOTE: If you use SDK functions which correspond to services which already
/// have Rust wrappers, you could break some assumptions made by the library.
pub mod bindings {
    #![allow(warnings)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

/// Module containing the automatically generated app message keys.
///
/// They also appear in [bindings], but we generate some rust wrappers here as
/// bindgen makes them `static mut`.
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

/// A collection of PebbleOS services that have global configuration, and
/// therefore need to be configured from a single location.
///
/// An instance of this struct will be given to you by the [main] macro.
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
/// The main function should be annotated with #\[embassy_executor::task\] with two parameters:
///
/// - [PebbleServices], which your code can use to use app messages and the accelerometer service.
/// - [embassy_executor::Spawner], which you can use to spawn more async tasks.
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
    crate::error!(
        "{}:{}",
        info.location().map(|l| l.file()).unwrap_or(""),
        info.location().map(|l| l.line()).unwrap_or(0)
    );
    trigger_panic();
}
