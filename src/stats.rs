//! The numbers behind the Stats tab. Pure: the caller passes the history, the
//! running focus, the clock and the timezone, so the day boundaries are testable.

use std::time::{Duration, SystemTime};

use crate::clock::since;
use crate::history::Entry;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

// ponytail: fixed 25/50 min spread thresholds, make configurable if they stop fitting
const SHORT_BELOW: Duration = Duration::from_secs(25 * 60);
const LONG_FROM: Duration = Duration::from_secs(50 * 60);

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Summary {
    pub(crate) today_focus: Duration,
    pub(crate) sessions: u32,
    pub(crate) avg: Duration,
    pub(crate) longest: Duration,
    pub(crate) earned: Duration,
    pub(crate) taken: Duration,
    pub(crate) late: u32,
    pub(crate) late_over: Duration,
    /// oldest first; (day of month, focus total). The last entry is today.
    pub(crate) week: [(u8, Duration); 7],
    /// (short, medium, long) focus sessions over the same 7 days
    pub(crate) spread: [u32; 3],
}

/// The civil day an instant falls on in `tz`; None if the instant can't be
/// represented.
fn day_of(t: SystemTime, tz: &TimeZone) -> Option<Date> {
    Some(Timestamp::try_from(t).ok()?.to_zoned(tz.clone()).date())
}

fn bucket(focus: Duration) -> usize {
    if focus < SHORT_BELOW {
        0
    } else if focus < LONG_FROM {
        1
    } else {
        2
    }
}

/// `live` is the start of a focus still running; it counts as a session of its
/// start day, with its length so far.
pub(crate) fn summarize(
    entries: &[Entry],
    live: Option<SystemTime>,
    now: SystemTime,
    tz: &TimeZone,
) -> Summary {
    let Some(today) = day_of(now, tz) else {
        return Summary::default();
    };
    let days: [Date; 7] = std::array::from_fn(|i| today.saturating_sub((6 - i as i64).days()));

    let mut focus: Vec<(Date, Duration)> = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Focus { start, focus } => Some((day_of(*start, tz)?, *focus)),
            Entry::Break { .. } => None,
        })
        .collect();
    if let Some(start) = live {
        if let Some(day) = day_of(start, tz) {
            focus.push((day, since(start, now)));
        }
    }

    let mut s = Summary::default();
    for (slot, day) in s.week.iter_mut().zip(days.iter()) {
        slot.0 = day.day() as u8;
    }
    for (day, length) in &focus {
        let Some(k) = days.iter().position(|d| d == day) else {
            continue;
        };
        s.week[k].1 += *length;
        s.spread[bucket(*length)] += 1;
        if *day == today {
            s.today_focus += *length;
            s.sessions += 1;
            s.longest = s.longest.max(*length);
        }
    }
    if s.sessions > 0 {
        s.avg = s.today_focus / s.sessions;
    }

    for e in entries {
        let Entry::Break {
            start,
            earned,
            taken,
        } = e
        else {
            continue;
        };
        if day_of(*start, tz) != Some(today) {
            continue;
        }
        s.earned += *earned;
        s.taken += *taken;
        if taken > earned {
            s.late += 1;
            s.late_over += *taken - *earned;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    /// 2024-06-15 00:00 UTC, the start of "today" in these tests
    const TODAY0: u64 = 1_718_409_600;
    const DAY: u64 = 86_400;

    fn at(s: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(s)
    }

    fn now() -> SystemTime {
        at(TODAY0 + 12 * 3600)
    }

    fn min(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    fn focus(start: u64, length: Duration) -> Entry {
        Entry::Focus {
            start: at(start),
            focus: length,
        }
    }

    #[test]
    fn today_adds_up_completed_focus_and_the_running_one() {
        let entries = [focus(TODAY0 + 3600, min(25)), focus(TODAY0 + 7200, min(50))];
        let s = summarize(&entries, Some(now() - min(10)), now(), &TimeZone::UTC);
        assert_eq!(s.today_focus, min(85));
        assert_eq!(s.sessions, 3);
        assert_eq!(s.avg, Duration::from_secs(28 * 60 + 20));
        assert_eq!(s.longest, min(50));
    }

    #[test]
    fn the_week_runs_from_six_days_ago_to_today() {
        let tz = &TimeZone::UTC;
        let seven_ago = [focus(TODAY0 - 7 * DAY + 3600, min(10))];
        let s = summarize(&seven_ago, None, now(), tz);
        assert_eq!(
            s.week.iter().map(|(_, d)| *d).sum::<Duration>(),
            Duration::ZERO
        );

        let six_ago = [focus(TODAY0 - 6 * DAY + 3600, min(10))];
        let s = summarize(&six_ago, None, now(), tz);
        assert_eq!(s.week[0], (9, min(10)), "2024-06-09 is first");
        assert_eq!(s.week[6].0, 15, "today is last");
        assert_eq!(s.week.iter().map(|(_, d)| *d).sum::<Duration>(), min(10));
    }

    #[test]
    fn spread_buckets_sessions_at_the_thresholds() {
        let lengths = [
            Duration::from_secs(24 * 60 + 59),
            Duration::from_secs(25 * 60),
            Duration::from_secs(49 * 60 + 59),
            Duration::from_secs(50 * 60),
        ];
        let entries: Vec<Entry> = lengths
            .iter()
            .enumerate()
            .map(|(i, l)| focus(TODAY0 + 3600 * (i as u64 + 1), *l))
            .collect();
        let s = summarize(&entries, None, now(), &TimeZone::UTC);
        assert_eq!(s.spread, [1, 2, 1]);
    }

    #[test]
    fn a_break_taken_past_what_it_earned_is_late() {
        let brk = |taken, earned| Entry::Break {
            start: at(TODAY0 + 3600),
            earned: min(earned),
            taken: min(taken),
        };
        let entries = [brk(12, 10), brk(8, 10)];
        let s = summarize(&entries, None, now(), &TimeZone::UTC);
        assert_eq!(s.late, 1);
        assert_eq!(s.late_over, min(2));
        assert_eq!(s.earned, min(20));
        assert_eq!(s.taken, min(20));
    }

    #[test]
    fn yesterdays_focus_is_not_today() {
        let entries = [focus(TODAY0 - 3600, min(25))];
        let s = summarize(&entries, None, now(), &TimeZone::UTC);
        assert_eq!(s.today_focus, Duration::ZERO);
        assert_eq!(s.sessions, 0);
        assert_eq!(s.week[5], (14, min(25)));
    }
}
