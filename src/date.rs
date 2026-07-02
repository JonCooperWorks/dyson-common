//! Proleptic-Gregorian civil-date arithmetic, via Howard Hinnant's algorithms.
//!
//! Pure integer math, no dependencies. Both repos carried a copy of one
//! direction or the other — dyson the epoch→(Y,M,D) split for rendering, swarm
//! the (Y,M,D)→days count for its cost-window and `--from YYYY-MM-DD` parsing.
//! Both live here so the calendar arithmetic is audited in one place.

/// Convert a Unix timestamp (seconds since epoch) to a `(year, month, day)`
/// tuple in UTC.
#[must_use]
pub const fn unix_to_ymd(secs: u64) -> (i64, u64, u64) {
    let z = (secs / 86400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    (y, m, d)
}

/// Days from the Unix epoch (1970-01-01) to the given proleptic Gregorian civil
/// date. Negative results are valid for pre-epoch dates.
#[must_use]
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 }.div_euclid(400);
    let yoe = y - era * 400;
    let month = i64::from(month);
    let doy =
        (153 * (month + if month > 2 { -3 } else { 9 }) + 2).div_euclid(5) + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_to_ymd_known_values() {
        assert_eq!(unix_to_ymd(0), (1970, 1, 1));
        // 2025-01-15 00:00:00 UTC = 1736899200
        assert_eq!(unix_to_ymd(1_736_899_200), (2025, 1, 15));
    }

    #[test]
    fn days_from_civil_known_values() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 1, 1), 10_957);
        assert_eq!(days_from_civil(2026, 6, 11), 20_615);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[test]
    fn round_trips_through_both_directions() {
        // days_from_civil(unix_to_ymd(t)) == t / 86400 for a few epochs.
        for secs in [0u64, 1_736_899_200, 1_600_000_000, 946_684_800] {
            let (y, m, d) = unix_to_ymd(secs);
            assert_eq!(
                days_from_civil(y, m as u32, d as u32),
                (secs / 86400) as i64
            );
        }
    }
}
