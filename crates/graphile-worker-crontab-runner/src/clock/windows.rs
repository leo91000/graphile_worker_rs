// The Windows timezone resolver is adapted from Chrono 0.4.45.
// Copyright (c) 2012-2014 The Rust Project Developers.
// Copyright (c) 2014-2026 Kang Seonghoon and contributors.
// SPDX-License-Identifier: MIT

use std::mem::MaybeUninit;
use std::ptr;

use time::{Date, Month, OffsetDateTime, UtcOffset};
use windows_sys::Win32::{
    Foundation::SYSTEMTIME,
    System::Time::{GetTimeZoneInformationForYear, TIME_ZONE_INFORMATION},
};

const FIRST_WINDOWS_YEAR: i32 = 1601;
const LAST_WINDOWS_YEAR: i32 = 30827;

pub(super) fn offset_at(datetime: OffsetDateTime) -> Option<UtcOffset> {
    let utc = datetime.checked_to_offset(UtcOffset::UTC)?;
    // Request rules for the instant's year, and extrapolate from the nearest
    // supported rule set for dates outside the Windows range.
    let reference_year = utc.year().clamp(FIRST_WINDOWS_YEAR, LAST_WINDOWS_YEAR) as u16;
    let mut rules = MaybeUninit::<TIME_ZONE_INFORMATION>::uninit();
    // SAFETY: A null timezone selects the current zone and rules points to
    // writable storage. Windows initializes it on success.
    let success = unsafe {
        GetTimeZoneInformationForYear(reference_year, ptr::null_mut(), rules.as_mut_ptr())
    };
    if success == 0 {
        return None;
    }
    // SAFETY: The successful call initialized rules.
    let rules = unsafe { rules.assume_init() };
    offset_from_rules(utc, &rules)
}

fn offset_from_rules(utc: OffsetDateTime, rules: &TIME_ZONE_INFORMATION) -> Option<UtcOffset> {
    let bias_offset = |adjustment| {
        let seconds = rules.Bias.checked_add(adjustment)?.checked_mul(-60)?;
        UtcOffset::from_whole_seconds(seconds).ok()
    };
    let standard = bias_offset(rules.StandardBias)?;
    let daylight = bias_offset(rules.DaylightBias)?;
    let standard_start = transition(rules.StandardDate, utc.year(), daylight)?;
    let daylight_start = transition(rules.DaylightDate, utc.year(), standard)?;

    let use_daylight = match (standard_start, daylight_start) {
        (Some(standard), Some(daylight)) if daylight < standard => {
            (daylight..standard).contains(&utc)
        }
        (Some(standard), Some(daylight)) => utc < standard || utc >= daylight,
        (Some(standard), None) => utc < standard,
        (None, Some(daylight)) => utc >= daylight,
        (None, None) => false,
    };
    Some(if use_daylight { daylight } else { standard })
}

fn transition(
    rule: SYSTEMTIME,
    year: i32,
    previous_offset: UtcOffset,
) -> Option<Option<OffsetDateTime>> {
    if rule.wMonth == 0 && rule.wYear == 0 {
        return Some(None);
    }

    let month = Month::try_from(u8::try_from(rule.wMonth).ok()?).ok()?;
    let day = u8::try_from(rule.wDay).ok()?;
    let date = if rule.wYear != 0 {
        Date::from_calendar_date(i32::from(rule.wYear), month, day).ok()?
    } else {
        if !(1..=5).contains(&day) || rule.wDayOfWeek > 6 {
            return None;
        }

        let first = Date::from_calendar_date(year, month, 1).ok()?;
        let first_weekday = i32::from(first.weekday().number_days_from_sunday());
        let days_to_weekday = (i32::from(rule.wDayOfWeek) - first_weekday).rem_euclid(7) as u8;
        let day = day * 7 - 6 + days_to_weekday;
        Date::from_calendar_date(year, month, day)
            .or_else(|_| Date::from_calendar_date(year, month, day - 7))
            .ok()?
    };

    let local = date
        .with_hms_milli(
            u8::try_from(rule.wHour).ok()?,
            u8::try_from(rule.wMinute).ok()?,
            u8::try_from(rule.wSecond).ok()?,
            rule.wMilliseconds,
        )
        .ok()?;
    Some(Some(local.assume_offset(previous_offset)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{datetime, offset};

    #[test]
    fn extrapolates_windows_daylight_rules_to_the_requested_calendar_year() {
        let rules = TIME_ZONE_INFORMATION {
            Bias: 300,
            DaylightBias: -60,
            StandardDate: SYSTEMTIME {
                wMonth: 11,
                wDay: 1,
                wHour: 2,
                ..Default::default()
            },
            DaylightDate: SYSTEMTIME {
                wMonth: 3,
                wDay: 2,
                wHour: 2,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            offset_from_rules(datetime!(1500-01-01 08:00 UTC), &rules),
            Some(offset!(-5))
        );
        assert_eq!(
            offset_from_rules(datetime!(1500-07-01 08:00 UTC), &rules),
            Some(offset!(-4))
        );
        assert_eq!(
            offset_from_rules(datetime!(+40000-01-01 08:00 UTC), &rules),
            Some(offset!(-5))
        );
    }
}
