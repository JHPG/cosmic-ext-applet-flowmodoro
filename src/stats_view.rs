//! The Stats tab: a hero card, stat tiles, and bars for the last week's focus.

use std::time::SystemTime;

use crate::clock::fmt;
use crate::history::Entry;
use crate::phase::Phase;
use crate::stats::{summarize, Summary};
use crate::tr::tr;
use cosmic::iced::{Alignment, Length};
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

pub(crate) fn view<'a, M: Clone + 'a>(
    history: &[Entry],
    phase: Phase,
    now: SystemTime,
) -> Element<'a, M> {
    let s = summary(history, phase, now, &TimeZone::system());
    let t = tr();

    let mut hero = widget::Column::new()
        .push(widget::text::caption(t.focus_today))
        .push(widget::text::title1(fmt(s.today_focus)))
        .spacing(4)
        .width(Length::Fill)
        .align_x(Alignment::Center);
    if s.sessions == 0 {
        hero = hero.push(widget::text::caption(t.no_focus));
    }
    let hero = widget::container(hero)
        .class(cosmic::theme::Container::Card)
        .padding(16)
        .width(Length::Fill);

    let mut breaks_extra = vec![(t.earned)(&fmt(s.earned))];
    if s.late > 0 {
        breaks_extra.push((t.late_today)(s.late, &fmt(s.late_over)));
    }
    let grid = widget::Column::new()
        .spacing(8)
        .width(Length::Fill)
        .push(widget::text::caption(t.today))
        .push(
            widget::Row::new()
                .spacing(8)
                .width(Length::Fill)
                .push(tile(t.sessions_label, s.sessions.to_string(), vec![]))
                .push(tile(t.average_label, fmt(s.avg), vec![])),
        )
        .push(
            widget::Row::new()
                .spacing(8)
                .width(Length::Fill)
                .push(tile(t.longest_label, fmt(s.longest), vec![]))
                .push(tile(t.breaks_label, fmt(s.taken), breaks_extra)),
        );

    let max = s
        .week
        .iter()
        .map(|(_, focus)| *focus)
        .max()
        .unwrap_or_default();
    let last = s.week.len() - 1;
    let mut week = widget::Column::new()
        .spacing(6)
        .width(Length::Fill)
        .push(widget::text::caption(t.last_7));
    let has_focus = s.week.iter().any(|(_, focus)| !focus.is_zero());
    for (i, (weekday, focus)) in s.week.iter().enumerate() {
        // hide empty days; with no focus all week, keep today's row
        if focus.is_zero() && (has_focus || i != last) {
            continue;
        }
        week = week.push(bar(
            t.weekdays[usize::from(*weekday)],
            40.0,
            i == last,
            share(focus.as_secs_f32(), max.as_secs_f32()),
            fmt(*focus),
            56.0,
        ));
    }

    widget::Column::new()
        .spacing(12)
        .width(Length::Fill)
        .push(hero)
        .push(grid)
        .push(week)
        .into()
}

/// One stat card: its label above a large value, then any notes.
fn tile<'a, M: 'a>(label: &'static str, value: String, extras: Vec<String>) -> Element<'a, M> {
    let mut body = widget::Column::new()
        .push(widget::text::caption(label))
        .push(widget::text::title3(value));
    for extra in extras {
        body = body.push(widget::text::caption(extra));
    }
    widget::container(body)
        .class(cosmic::theme::Container::Card)
        .padding(12)
        .width(Length::Fill)
        .into()
}

/// One bar row: a label, a bar filled to `ratio`, and the value at the end.
fn bar<'a, M: Clone + 'a>(
    label: &'static str,
    label_width: f32,
    emphasis: bool,
    ratio: f32,
    value: String,
    value_width: f32,
) -> Element<'a, M> {
    let label = if emphasis {
        widget::text::body(label)
    } else {
        widget::text::caption(label)
    };
    widget::Row::new()
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .push(label.width(Length::Fixed(label_width)))
        .push(widget::progress_bar::determinate_linear(ratio).width(Length::Fill))
        .push(widget::text::caption(value).width(Length::Fixed(value_width)))
        .into()
}

/// `part` as a share of `whole`; 0 when there is nothing to share.
fn share(part: f32, whole: f32) -> f32 {
    if whole > 0.0 {
        part / whole
    } else {
        0.0
    }
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
