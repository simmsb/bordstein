#![no_std]
#![feature(integer_casts)]
#![feature(integer_widen_truncate)]
#![feature(impl_trait_in_assoc_type)]
#![feature(async_fn_traits)]

pub mod executor;
pub mod single_core_cell;
pub mod time_driver;
pub mod log_impl;
pub mod window;

pub mod bindings {
    #![allow(warnings)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    // log_impl::init();
    crate::info!("Main called!");
    executor::init();
    executor::run(init);
}

#[embassy_executor::task]
async fn async_main() {
    window::with_window(async |h| {
        core::future::pending::<()>().await;
    }).await.unwrap();
}

fn init(s: embassy_executor::Spawner) {
    crate::info!("Init called!");

    s.spawn(async_main().unwrap());
}

// extern, no_mangle so we can set a breakpoint
#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn trigger_panic() -> ! {
    unsafe {
        bindings::exit_reason_set(bindings::AppExitReason::APP_EXIT_NOT_SPECIFIED);
        bindings::window_stack_pop_all(false);

        // bindings::app_event_loop();
    };

    unsafe {
        let crash: *mut u32 = core::ptr::null_mut();
        core::ptr::write_volatile(crash, 0xDEADBEEF);
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    trigger_panic();
}
