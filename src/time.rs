use crate::bindings;

/// A UTC timestamp stored as the number of seconds since the epoch.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, ufmt::derive::uDebug)]
pub struct Timestamp(pub bindings::time_t);

impl ufmt::uDisplay for Timestamp {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        ufmt::uwrite!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, ufmt::derive::uDebug)]
pub struct Datetime {
    // 0..=59, 0..=60 on a leap second
    pub secs: u8,

    // 0..=59
    pub mins: u8,

    // 0..=23
    pub hours: u8,

    // 1..=31
    pub day_of_month: u8,

    // 0..=6
    pub day_of_week: u8,

    // 0..=365
    pub day_of_year: u16,

    // 0..=11
    pub month: u8,

    // Years since 1900
    pub year: u16,
}

impl Datetime {
    pub(crate) fn from_tm(tm: &bindings::tm) -> Self {
        Self {
            secs: tm.tm_sec as u8,
            mins: tm.tm_min as u8,
            hours: tm.tm_hour as u8,
            day_of_month: tm.tm_mday as u8,
            day_of_week: tm.tm_wday as u8,
            day_of_year: tm.tm_yday as u16,
            month: tm.tm_mon as u8,
            year: tm.tm_year as u16,
        }
    }
}

/// Return a UTC timestamp representing the current time.
pub fn now() -> Timestamp {
    Timestamp(unsafe { bindings::time(core::ptr::null_mut()) })
}

/// Convert a timestamp to a locally zoned [Datetime]
pub fn gmtime(ts: Timestamp) -> Datetime {
    unsafe {
        // This pointer is valid until we next call gmtime or something
        let tm = bindings::gmtime(&raw const ts.0);

        Datetime::from_tm(tm.as_ref_unchecked())
    }
}
