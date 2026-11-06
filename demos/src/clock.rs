//! The local time, for the teletext's clock. Rust's own library knows only
//! the time since 1970 in UTC, so the local time comes from the system as
//! std links it already: the C library's `localtime_r` on macOS and Linux,
//! kernel32's `GetLocalTime` on Windows. No crate needed, and where the
//! system says nothing it's UTC.

use std::time::{SystemTime, UNIX_EPOCH};

/// A moment as a clock on the wall shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Time {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
    /// 0 for Sunday to 6 for Saturday.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

impl Time {
    /// The day's name: `Saturday`.
    pub fn day_name(&self) -> &'static str {
        DAYS[usize::from(self.weekday % 7)]
    }

    /// The month's name: `October`.
    pub fn month_name(&self) -> &'static str {
        MONTHS[usize::from(self.month.clamp(1, 12) - 1)]
    }

    /// The day's name in three letters, as a teletext header has it: `Sat`.
    pub fn day_short(&self) -> &'static str {
        &self.day_name()[..3]
    }

    /// The month's name in three letters: `Oct`.
    pub fn month_short(&self) -> &'static str {
        &self.month_name()[..3]
    }

    /// `secs` after 1970 began, in UTC (Howard Hinnant's days-to-civil).
    pub fn utc(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let of_day = secs.rem_euclid(86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        // 1970-01-01 was a Thursday.
        let weekday = (days + 4).rem_euclid(7);
        Self { year: year as i32, month: month as u8, day: day as u8, weekday: weekday as u8, hour: (of_day / 3600) as u8, minute: (of_day / 60 % 60) as u8, second: (of_day % 60) as u8 }
    }
}

/// Seconds since 1970 began.
fn epoch() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The time now where the machine is.
pub fn now() -> Time {
    local().unwrap_or_else(|| Time::utc(epoch()))
}

#[cfg(unix)]
fn local() -> Option<Time> {
    use std::ffi::{c_char, c_int, c_long};

    /// The C library's `struct tm` as macOS and glibc lay it out, with room
    /// to spare after it.
    #[repr(C)]
    struct Tm {
        sec: c_int,
        min: c_int,
        hour: c_int,
        mday: c_int,
        mon: c_int,
        year: c_int,
        wday: c_int,
        yday: c_int,
        isdst: c_int,
        gmtoff: c_long,
        zone: *const c_char,
        spare: [c_long; 4],
    }

    extern "C" {
        // `time_t` is a `long` on the 64-bit systems Flow runs on.
        fn localtime_r(time: *const c_long, out: *mut Tm) -> *mut Tm;
    }

    let t = epoch() as c_long;
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null(), spare: [0; 4] };
    // SAFETY: a valid time and a `struct tm` to fill, both outliving the call.
    let filled = unsafe { localtime_r(&t, &mut tm) };
    if filled.is_null() {
        return None;
    }
    let byte = |v: c_int| u8::try_from(v).ok();
    Some(Time { year: tm.year + 1900, month: byte(tm.mon + 1)?, day: byte(tm.mday)?, weekday: byte(tm.wday)?, hour: byte(tm.hour)?, minute: byte(tm.min)?, second: byte(tm.sec.min(59))? })
}

#[cfg(windows)]
fn local() -> Option<Time> {
    /// kernel32's `SYSTEMTIME`.
    #[repr(C)]
    #[derive(Default)]
    struct WinTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(out: *mut WinTime);
    }

    let mut st = WinTime::default();
    // SAFETY: a `SYSTEMTIME` to fill, outliving the call.
    unsafe { GetLocalTime(&mut st) };
    let byte = |v: u16| u8::try_from(v).ok();
    Some(Time { year: i32::from(st.year), month: byte(st.month)?, day: byte(st.day)?, weekday: byte(st.day_of_week)?, hour: byte(st.hour)?, minute: byte(st.minute)?, second: byte(st.second)? })
}

#[cfg(not(any(unix, windows)))]
fn local() -> Option<Time> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates_count_from_1970_with_their_weekdays() {
        let start = Time::utc(0);
        assert_eq!((start.year, start.month, start.day, start.day_name()), (1970, 1, 1, "Thursday"));
        // 2026-10-31 12:34:56 UTC, a Saturday.
        let t = Time::utc(1_793_450_096);
        assert_eq!((t.year, t.month_name(), t.day, t.day_name()), (2026, "October", 31, "Saturday"));
        assert_eq!((t.month_short(), t.day_short()), ("Oct", "Sat"));
        assert_eq!((t.hour, t.minute, t.second), (12, 34, 56));
        let leap = Time::utc(951_782_400);
        assert_eq!((leap.year, leap.month, leap.day), (2000, 2, 29));
    }

    #[test]
    fn the_local_time_is_a_time_of_day() {
        let t = now();
        assert!(t.hour < 24 && t.minute < 60 && t.second < 60 && (1..=12).contains(&t.month) && (1..=31).contains(&t.day));
        // Within a day of UTC's, wherever the machine is.
        let utc = Time::utc(epoch());
        assert!((t.year - utc.year).abs() <= 1);
    }
}
