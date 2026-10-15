//! Clock parts: the arithmetic and the text behind `[h:]mm:ss`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long has passed since `t`; zero if `t` lies in the future.
pub(crate) fn since(t: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(t).unwrap_or_default()
}

/// Milliseconds since the Unix epoch: the unit the state and history files store.
pub(crate) fn millis(t: SystemTime) -> u128 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

/// Reads a millisecond count written by `millis` back into an instant.
pub(crate) fn at_millis(s: &str) -> Option<SystemTime> {
    Some(UNIX_EPOCH + Duration::from_millis(s.parse().ok()?))
}

/// Time split as (unit in seconds, text): [h,] mm, ss
pub(crate) fn parts(d: Duration) -> Vec<(u64, String)> {
    let s = d.as_secs();
    if s >= 3600 {
        vec![
            (3600, (s / 3600).to_string()),
            (60, format!("{:02}", s / 60 % 60)),
            (1, format!("{:02}", s % 60)),
        ]
    } else {
        vec![
            (60, format!("{:02}", s / 60)),
            (1, format!("{:02}", s % 60)),
        ]
    }
}

pub(crate) fn fmt(d: Duration) -> String {
    parts(d)
        .into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join(":")
}

/// Replace one part (h, min or s) of `elapsed` with `v`; other parts unchanged.
/// `parse_part` caps `v` per unit, so a part never carries into its neighbour.
pub(crate) fn set_part(elapsed: Duration, unit: u64, v: u64) -> Duration {
    let e = elapsed.as_secs();
    let cur = if unit == 3600 {
        e / 3600
    } else {
        e / unit % 60
    };
    Duration::from_secs(e - cur * unit + v * unit)
}

/// Typed value for one clock part, capped to what that part can hold: 0..=59 for
/// minutes and seconds, 0..=23 for hours. A value the clock can't hold is
/// rejected, which leaves the confirm button disabled.
pub(crate) fn parse_part(unit: u64, s: &str) -> Option<u64> {
    let limit = if unit == 3600 { 24 } else { 60 };
    s.trim().parse().ok().filter(|&v| v < limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_formats_hours_minutes_seconds() {
        assert_eq!(fmt(Duration::from_secs(65)), "01:05");
        assert_eq!(fmt(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn a_part_holds_only_what_its_unit_can_hold() {
        assert_eq!(parse_part(60, " 25 "), Some(25));
        assert_eq!(parse_part(60, "59"), Some(59));
        assert_eq!(parse_part(60, "60"), None);
        assert_eq!(parse_part(1, "60"), None);
        assert_eq!(parse_part(3600, "23"), Some(23));
        assert_eq!(parse_part(3600, "24"), None);
        assert_eq!(parse_part(60, "-5"), None);
        assert_eq!(parse_part(60, "99999999999999999"), None);

        // 05:30 → minutes=25 → 25:30, and no carry into the hours
        let d = |s| Duration::from_secs(s);
        assert_eq!(fmt(set_part(d(330), 60, 25)), "25:30");
        assert_eq!(fmt(set_part(d(20000), 60, 59)), "5:59:20");
        assert_eq!(fmt(set_part(d(3725), 1, 0)), "1:02:00");
        assert_eq!(fmt(set_part(d(3725), 3600, 2)), "2:02:05");
    }
}
