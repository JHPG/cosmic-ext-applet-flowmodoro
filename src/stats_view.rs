//! The Stats tab: the summary of the history file, drawn as text. The numbers
//! come from `stats.rs`; this module only lays them out.

use std::time::SystemTime;

use crate::clock::fmt;
use crate::history::Entry;
use crate::phase::Phase;
use crate::stats::{summarize, Summary};
use crate::tr::tr;
use cosmic::iced::Alignment;
use cosmic::widget;
use cosmic::Element;
use jiff::tz::TimeZone;

/// A focus still running counts toward today as well as the history does.
fn summary(history: &[Entry], phase: Phase, now: SystemTime, tz: &TimeZone) -> Summary {
    let live = match phase {
        Phase::Focus(start) => Some(start),
        Phase::Idle | Phase::Break(..) => None,
    };
    summarize(history, live, now, tz)
}

pub(crate) fn view<'a, M: 'a>(history: &[Entry], phase: Phase, now: SystemTime) -> Element<'a, M> {
    let s = summary(history, phase, now, &TimeZone::system());
    let t = tr();
    let caption = if s.sessions == 0 {
        t.no_focus.to_owned()
    } else {
        (t.sessions)(s.sessions, &fmt(s.avg), &fmt(s.longest))
    };
    let mut week = widget::Row::new().spacing(8).align_y(Alignment::End);
    for (day, focus) in s.week {
        week = week.push(
            widget::Column::new()
                .push(widget::text::caption(day.to_string()))
                .push(widget::text::body(fmt(focus)))
                .align_x(Alignment::Center),
        );
    }
    widget::Column::new()
        .push(widget::text::caption(t.focus_today))
        .push(widget::text::title1(fmt(s.today_focus)))
        .push(widget::text::caption(caption))
        .push(widget::text::caption((t.breaks_today)(
            &fmt(s.earned),
            &fmt(s.taken),
        )))
        .push(widget::text::caption((t.late_today)(
            s.late,
            &fmt(s.late_over),
        )))
        .push(widget::text::caption((t.spread)(
            s.spread[0],
            s.spread[1],
            s.spread[2],
        )))
        .push(widget::text::caption(t.last_7))
        .push(week)
        .spacing(8)
        .align_x(Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn at(s: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(s)
    }

    #[test]
    fn a_running_focus_counts_toward_today_only_while_it_runs() {
        // noon UTC, so the day is the same in every zone the test can meet
        let noon = 1_718_452_800;
        let focus = Duration::from_secs(600);
        let live = summary(&[], Phase::Focus(at(noon)), at(noon + 600), &TimeZone::UTC);
        assert_eq!(live.today_focus, focus);
        assert_eq!(live.sessions, 1);
        let idle = summary(&[], Phase::Idle, at(noon + 600), &TimeZone::UTC);
        assert_eq!(idle.today_focus, Duration::ZERO);
        assert_eq!(idle.sessions, 0);
    }
}
