use core::ffi::CStr;

use crate::{bindings, resources::Resource};

#[derive(Clone)]
pub struct Font(pub(crate) bindings::GFont);

pub mod system {
    pub use crate::bindings::{
        FONT_KEY_BITHAM_18_LIGHT_SUBSET, FONT_KEY_BITHAM_30_BLACK, FONT_KEY_BITHAM_34_LIGHT_SUBSET,
        FONT_KEY_BITHAM_34_MEDIUM_NUMBERS, FONT_KEY_BITHAM_42_BOLD, FONT_KEY_BITHAM_42_LIGHT,
        FONT_KEY_BITHAM_42_MEDIUM_NUMBERS, FONT_KEY_DROID_SERIF_28_BOLD, FONT_KEY_FONT_FALLBACK,
        FONT_KEY_FONT_FALLBACK_INTERNAL, FONT_KEY_GOTHIC_09, FONT_KEY_GOTHIC_14,
        FONT_KEY_GOTHIC_14_BOLD, FONT_KEY_GOTHIC_18, FONT_KEY_GOTHIC_18_BOLD, FONT_KEY_GOTHIC_24,
        FONT_KEY_GOTHIC_24_BOLD, FONT_KEY_GOTHIC_28, FONT_KEY_GOTHIC_28_BOLD,
        FONT_KEY_LECO_20_BOLD_NUMBERS, FONT_KEY_LECO_26_BOLD_NUMBERS_AM_PM,
        FONT_KEY_LECO_28_LIGHT_NUMBERS, FONT_KEY_LECO_32_BOLD_NUMBERS,
        FONT_KEY_LECO_36_BOLD_NUMBERS, FONT_KEY_LECO_38_BOLD_NUMBERS, FONT_KEY_LECO_42_NUMBERS,
        FONT_KEY_LECO_60_BOLD_NUMBERS_AM_PM, FONT_KEY_LECO_60_NUMBERS_AM_PM,
        FONT_KEY_ROBOTO_BOLD_SUBSET_49, FONT_KEY_ROBOTO_CONDENSED_21,
    };
}

impl Font {
    pub fn get_system_font(key: &CStr) -> Self {
        Font(unsafe { bindings::fonts_get_system_font(key.as_ptr()) })
    }

    pub fn load_custom_font(handle: Resource) -> Self {
        Font(unsafe { bindings::fonts_load_custom_font(handle.0.as_ptr()) })
    }
}
