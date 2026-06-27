#![no_std]
#![feature(integer_casts)]
#![feature(integer_widen_truncate)]
#![feature(impl_trait_in_assoc_type)]
#![feature(trait_alias)]

use heapless::CString;
use pin_init::stack_pin_init;

use self::{
    bindings::GTextAlignment,
    layer::{Layer, StatusBarLayer, TextLayer},
};

pub mod app_message;
pub mod colour;
pub mod dictionary;
pub mod executor;
pub mod font;
pub mod graphics_context;
pub mod layer;
pub mod log_impl;
pub mod shapes;
pub mod single_core_cell;
pub mod time_driver;
pub mod window;

pub use layer::IsLayer as _;

pub mod bindings {
    #![allow(warnings)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    crate::info!("Main called!");

    // unsafe {
    //     let w = bindings::window_create();
    //     bindings::window_set_background_color(w, bindings::GColor8 { argb: 0b11110011 });
    //     bindings::window_stack_push(w, true);
    // }

    executor::init();
    executor::run(init);

    crate::info!("Main leaving!");
}

#[embassy_executor::task]
async fn async_main() {
    crate::info!("Async main called!");
    window::with_window(async |mut h| {
        let app_messages = &mut app_message::AppMessages;
        stack_pin_init!(let app_messages = app_messages.listen(
            1024,
            512,
            |d| {},
            |_| {},
            |_| {},
            |_, _| {},
        ));

        h.set_background_colour(bindings::GColor8::RED);

        let window_bounds = h.root_layer().bounds();
        crate::info!("Window bounds: {:?}", window_bounds);

        let mut foo = 123;

        {
            let status_bar = h.root_layer().new_child::<StatusBarLayer>(()).unwrap();

            let remaining_space =
                window_bounds.shrink_to_avoid(status_bar.layer().bounds(), shapes::Edge::Top, 0);

            stack_pin_init! {
                let child_layer = h
                    .root_layer()
                    .new_child::<Layer>(remaining_space)
                    .unwrap()
                    .with_update_proc(|_layer, _ctx| {
                        crate::debug!("Hello from layer callback: {}", foo);
                        foo += 1;
                    })
            };

            let mut text_layer = child_layer
                .new_child::<TextLayer>(child_layer.bounds())
                .unwrap();
            text_layer.set_text_alignment(GTextAlignment::GTextAlignmentCenter);

            let mut text_content: CString<64>;
            for i in 0..10 {
                text_content = CString::<64>::new();
                let _ = ufmt::uwrite!(&mut text_content, "{}", i);
                let _guard = text_layer.set_text(&text_content);

                embassy_time::Timer::after_secs(1).await;

                app_messages.send(|d| {
                    d.u16(10001, 1234)?;

                    Ok(())
                }).unwrap();
            }

            crate::info!("Child bounds: {:?}", child_layer.bounds());
        }

        // layers now destroyed, app should show just the window with its red background

        core::future::pending::<()>().await;
    })
    .await
    .unwrap();
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
fn panic(info: &core::panic::PanicInfo) -> ! {
    let msg = info.message().as_str().unwrap_or("<no message>");
    crate::error!("Panic! {}", msg);
    trigger_panic();
}
