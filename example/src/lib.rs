#![no_std]
#![feature(impl_trait_in_assoc_type)]

use futures::StreamExt as _;
use heapless::CString;
use pin_init::stack_pin_init;

use bordstein::{
    bindings::{GColor8, GTextAlignment, TimeUnits},
    prelude::*,
    shapes,
};

bordstein::main!(async_main);

// #[embassy_executor::task]
// async fn async_main(services: bordstein::PebbleServices, spawner: embassy_executor::Spawner) {
//     window::with_window(async |mut h| {
//         pin_init::stack_pin_init!(let layer = h
//         .root_layer()
//         .new_child::<Layer>(GRect::new(0, 0, 10, 10))
//         .unwrap()
//         .with_update_proc(|l, mut ctx| {
//             let bounds = l.bounds();
//             ctx.set_fill_colour(GColor::RED);
//             ctx.fill_rect(bounds, 10, GCornerMask::GCornersAll);
//         }));

//         for i in 0..200 {
//             layer.set_frame(GRect::new(i, i, 10, 10));
//             Timer::after_millis(50).await;
//         }
//     })
//     .await;
// }

#[embassy_executor::task]
async fn async_main(services: bordstein::PebbleServices, spawner: embassy_executor::Spawner) {
    async_main_(services, spawner).await;
}

async fn async_main_(mut services: bordstein::PebbleServices, _spawner: embassy_executor::Spawner) {
    bordstein::info!("Async main called!");

    with_window(async |mut h| {
        let app_messages = services.app_messages.open(1024, 512);
        stack_pin_init!(let _app_message_listener = app_messages.listen(
            |_d| {},
            |_| {},
            |_| {},
            |_, _| {},
        ));
        stack_pin_init!(let _app_message_listener = app_messages.listen(
            |_d| {},
            |_| {},
            |_| {},
            |_, _| {},
        ));
        stack_pin_init!(let _app_message_listener = app_messages.listen_received(
            |_d| {},
        ));

        let _ = app_messages.send(|d| d.u8(10001, 123));

        h.set_background_colour(GColor8::RED);

        let window_bounds = h.root_layer().bounds();
        bordstein::info!("Window bounds: {:?}", window_bounds);

        stack_pin_init!(let timer_minutes = TickService::listen(TimeUnits::MINUTE_UNIT, |time, _| {
            bordstein::info!("minute timer tick: {:?}", time);
        }));

        let mut foo = 123;

        {
            stack_pin_init!(let timer_seconds = TickService::listen(TimeUnits::SECOND_UNIT, |time, _| {
                bordstein::info!("second timer tick: {:?}", time);
            }));

            let root_layer = h.root_layer();
            let status_bar = root_layer.new_child::<StatusBarLayer>(()).unwrap();

            let remaining_space =
                window_bounds.shrink_to_avoid(status_bar.layer().bounds(), shapes::Edge::Top, 0);

            stack_pin_init! {
                let child_layer = root_layer
                    .new_child::<Layer>(remaining_space)
                    .unwrap()
                    .with_update_proc(|_layer, _ctx| {
                        bordstein::debug!("Hello from layer callback: {}", foo);
                        foo += 1;
                    })
            };

            let mut num_taps: u32 = 0;

            let mut accelerometer_service = services.accelerometer.enable();
            stack_pin_init!(let tap_events = accelerometer_service.subscribe_to_tap_service(|axis, dir| {
                num_taps += 1;
                bordstein::info!("Tap! {}, {:?}, {}", num_taps, axis, dir);
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

            bordstein::info!("Child bounds: {:?}", child_layer.bounds());
        }

        stack_pin_init!(let timer_seconds_stream = TickService::stream(TimeUnits::SECOND_UNIT));
        while let Some(t) = timer_seconds_stream.next().await {
            bordstein::info!("second tick stream: {}", t.0.secs);
        }

        // layers now destroyed, app should show just the window with its red background

        // if you have nothing else to do, but want to wait until the system
        // closes the window, you can use core::future::pending.
        core::future::pending::<()>().await;
    })
    .await
    .unwrap();
}
