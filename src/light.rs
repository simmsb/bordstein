use crate::bindings;

pub fn enable_interaction() {
    unsafe {
        bindings::light_enable_interaction();
    }
}

pub fn enable(enable: bool) {
    unsafe {
        bindings::light_enable(enable);
    }
}

pub fn set_colour(colour: bindings::GColor) {
    unsafe {
        bindings::light_set_color(colour);
    }
}

pub fn set_colour_rgb888(colour: u32) {
    unsafe {
        bindings::light_set_color_rgb888(colour);
    }
}

pub fn is_on() -> bool {
    unsafe { bindings::light_is_on() }
}
