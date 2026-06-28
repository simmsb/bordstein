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

use pebble_async::{
    IsLayer as _, bindings::{self, GTextAlignment, TimeUnits}, events, layers::{Layer, StatusBarLayer, TextLayer}, shapes, window
};

pebble_async::main!(async_main);

#[embassy_executor::task]
async fn async_main(services: pebble_async::PebbleServices, spawner: embassy_executor::Spawner) {
    async_main_(services, spawner).await;
}

async fn async_main_(mut services: pebble_async::PebbleServices, spawner: embassy_executor::Spawner) {
    pebble_async::info!("Async main called!");
    window::with_window(async |mut h| {
        let mut app_messages = services.app_messages.open(1024, 512);
        stack_pin_init!(let _app_message_listener = app_messages.listen(
            |_d| {},
            |_| {},
            |_| {},
            |_, _| {},
        ));

        h.set_background_colour(bindings::GColor8::RED);

        let window_bounds = h.root_layer().bounds();
        pebble_async::info!("Window bounds: {:?}", window_bounds);

        stack_pin_init!(let timer_minutes = events::tick::listen(TimeUnits::MINUTE_UNIT, |time, _| {
            pebble_async::info!("minute timer tick: {:?}", time);
        }));

        let mut foo = 123;

        {
            stack_pin_init!(let timer_seconds = events::tick::listen(TimeUnits::SECOND_UNIT, |time, _| {
                pebble_async::info!("second timer tick: {:?}", time);
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
                        pebble_async::debug!("Hello from layer callback: {}", foo);
                        foo += 1;
                    })
            };

            let mut num_taps: u32 = 0;

            let mut accelerometer_service = services.accelerometer.enable();
            stack_pin_init!(let tap_events = accelerometer_service.subscribe_to_tap_service(|axis, dir| {
                num_taps += 1;
                pebble_async::info!("Tap! {}, {:?}, {}", num_taps, axis, dir);
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

            pebble_async::info!("Child bounds: {:?}", child_layer.bounds());
        }

        stack_pin_init!(let timer_seconds_stream = events::tick::stream(TimeUnits::SECOND_UNIT));
        while let Some(t) = timer_seconds_stream.next().await {
            pebble_async::info!("second tick stream: {}", t.secs);
        }

        // layers now destroyed, app should show just the window with its red background

        core::future::pending::<()>().await;
    })
    .await
    .unwrap();
}
