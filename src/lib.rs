#![no_std]
#![feature(integer_casts)]
#![feature(integer_widen_truncate)]
#![feature(impl_trait_in_assoc_type)]
#![feature(trait_alias)]
#![feature(atomic_ptr_null)]
#![feature(type_alias_impl_trait)]

use futures::StreamExt as _;
use heapless::CString;
use pin_init::stack_pin_init;

use self::{
    bindings::{GTextAlignment, TimeUnits},
    layers::{Layer, StatusBarLayer, TextLayer},
};

pub mod app_message;
pub mod colour;
pub mod dictionary;
pub mod events;
pub mod executor;
pub mod font;
pub mod graphics_context;
pub mod layers;
pub mod log_impl;
pub mod shapes;
pub mod single_core_cell;
pub mod time_driver;
pub mod utils;
pub mod window;
pub mod time;

pub use layers::IsLayer as _;

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
    async_main_().await;
}

async fn async_main_() {
    crate::info!("Async main called!");
    window::with_window(async |mut h| {
        let app_messages = &mut app_message::AppMessages { _private: () };
        stack_pin_init!(let app_messages = app_messages.listen(
            1024,
            512,
            |_d| {},
            |_| {},
            |_| {},
            |_, _| {},
        ));


        h.set_background_colour(bindings::GColor8::RED);

        let window_bounds = h.root_layer().bounds();
        crate::info!("Window bounds: {:?}", window_bounds);

        stack_pin_init!(let timer_minutes = events::tick::listen(TimeUnits::MINUTE_UNIT, |time, _| {
            crate::info!("minute timer tick: {:?}", time);
        }));

        let mut foo = 123;

        {
            stack_pin_init!(let timer_seconds = events::tick::listen(TimeUnits::SECOND_UNIT, |time, _| {
                crate::info!("second timer tick: {:?}", time);
            }));

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


            let mut num_taps: u32 = 0;

            let mut accelerometer_service =
                events::accelerometer::AccelerometerService { _private: () }.enable();

            stack_pin_init!(let tap_events = accelerometer_service.subscribe_to_tap_service(|axis, dir| {
                num_taps += 1;
                crate::info!("Tap! {}, {:?}, {}", num_taps, axis, dir);
            }));

            let mut text_layer: TextLayer<'_> = child_layer
                .new_child::<TextLayer>(child_layer.bounds())
                .unwrap();
            text_layer.set_text_alignment(GTextAlignment::GTextAlignmentCenter);

            let mut text_content: CString<64>;
            for i in 0..10 {
                text_content = CString::<64>::new();
                let _ = ufmt::uwrite!(&mut text_content, "{}", i);
                let _guard = text_layer.set_text(&text_content);

                embassy_time::Timer::after_secs(1).await;

                app_messages
                    .send(|d| {
                        d.u16(10001, 1234)?;

                        Ok(())
                    })
                    .unwrap();
            }

            crate::info!("Child bounds: {:?}", child_layer.bounds());
        }

        stack_pin_init!(let timer_seconds_stream = events::tick::stream(TimeUnits::SECOND_UNIT));
        while let Some(t) = timer_seconds_stream.next().await {
            crate::info!("second tick stream: {}", t.secs);
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
