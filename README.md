<h1 align="center">Bordstein</h1>

<h2 align="center">Rust API bindings for the Pebble SDK which tries its best to be rusty</h2>

<p align="center">
  <a href="https://simmsb.github.io/bordstein/bordstein/index.html"><img alt="Docs" src="https://img.shields.io/badge/Documentation-green"></a>
</p>

> [!WARNING]
> This library is heavily in development, might not be fully safe, and will
> probably change often. Use at your own risk.

Bordstein is a Rust library which you can use to write PebbleOS applications in Rust.

I've designed it to try and abstract away all the C-isms of the C API, some examples:

1. Bordstein is an async API, it uses [embassy](https://embassy.dev/) to provide
   an async executor and a time implementation.
   
   Why async? It allows the library to return the power over control flow back
   to the user. Traditional PebbleOS applications must shuffle all of their
   control flow into callbacks due to `app_event_loop()` needing to take over.
   
   With async rust, the application still lives inside `app_event_loop()`, but
   the user code is transformed into a state machine by the Rust compiler which
   is progressed by the SDK callbacks.
   
2. Layers are created hierarchically, and rust lifetimes are used to ensure that
   a parent layer cannot be destroyed before all its children also are.
   
3. All event handlers can be `FnMut` closures which borrow their local
   environment. This means you can create a layer and some local variables, and
   update them from a callback, without having to store your layer in a global
   variable resulting in the need for manual lifetime management.

## Example

A super unuseful example, yet one that I believe gives a bit of a hint as to
what applications written in this library are like:

``` rust
#![no_std]
#![feature(impl_trait_in_assoc_type)]

use embassy_time::Timer;

use bordstein::{
    bindings::{GCornerMask, GRect},
    colour::GColor,
    layers::Layer,
};

bordstein::main!(async_main);

#[embassy_executor::task]
async fn async_main(services: bordstein::PebbleServices, spawner: embassy_executor::Spawner) {
    // Create a window and push it. The async closure passed in will run
    // until the async closure exits, or the window is popped (whichever comes first).
    window::with_window(async |mut h| {
    
        // If we want to attach a callback, we'll need to pin the layer.
        // This is because the closure's data will be stored inside the async task 
        // by the compiler, which will allow us to reference local variables and have
        // the closure be a FnMut without needing to allocate!
        // This also means that rust can check that you're not using a deallocated layer
        // from a callback.
        pin_init::stack_pin_init!(let layer = h
            .root_layer()
            .new_child::<Layer>(GRect::new(0, 0, 10, 10))
            .unwrap()
            .with_update_proc(|l, mut ctx| {
                // For the demo, simply fill a rounded rect.
                let bounds = l.bounds();
                ctx.set_fill_colour(GColor::RED);
                ctx.fill_rect(bounds, 10, GCornerMask::GCornersAll);
        }));

        // For the demo, set the position of the layer to i,i
        for i in 0..200 {
            layer.set_frame(GRect::new(i, i, 10, 10));
            
            // change the position every 50ms
            Timer::after_millis(50).await;
        }
        
        // After moving the layer around a bit, exit this function, destroying the layer and popping the window.
    })
    .await;
    
    // After the root window is popped, `app_event_loop()` will shortly exit, and the application will exit.
}
```


For a comprehensive example project, look at:
[pebble-weather-graph](https://github.com/simmsb/pebble-weather-graph)

## Hear ye, the mysteries of Rust on Pebble watches.

Here's some notes that might be useful:

1. Pebble applications running on hardware can't use memory barrier
   instructions. For that reason you need to use a custom target.json which sets
   `singlethread: true`, otherwise the memory barriers generated for atomic
   operations will crash.
   
   To aid in debugging efforts, the SDK's qemu emulator is perfectly fine with
   memory barrier instructions, it's only on hardware where they're not usable.
   
2. Your program must use the `pic`/`pie` relocation model, if you use `static`
   your code will half run, but references to globals become garbaged and your
   code will do things like crashing in `app_log` due to the file name string
   pointer being garbage.
