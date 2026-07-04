pub fn cancel() {
    unsafe {
        crate::bindings::vibes_cancel();
    }
}

pub fn short_pulse() {
    unsafe {
        crate::bindings::vibes_short_pulse();
    }
}

pub fn long_pulse() {
    unsafe {
        crate::bindings::vibes_long_pulse();
    }
}

pub fn double_pulse() {
    unsafe {
        crate::bindings::vibes_double_pulse();
    }
}

pub fn enqueue_custom_pattern(segments: &'static [u32]) {
    let vibe_pattern = crate::bindings::VibePattern {
        durations: segments.as_ptr(),
        num_segments: segments.len() as u32,
    };
    unsafe {
        crate::bindings::vibes_enqueue_custom_pattern(vibe_pattern);
    }
}
