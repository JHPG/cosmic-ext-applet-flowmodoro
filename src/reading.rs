//! One reading of a phase at one instant: everything a surface renders, so the
//! panel and the popup can't each decide what a phase looks like.

use crate::clock::{fmt, since};
use crate::phase::{break_for, Action, Phase, BREAK_RATIO};
use crate::tr::tr;
use std::time::{Duration, SystemTime};

/// What a phase looks like at one instant. The panel reads `panel()`, the popup
/// reads the rest, and neither of them re-derives it.
pub(crate) struct Reading {
    pub(crate) title: &'static str,
    /// what the popup clock shows; its parts are editable when `editable`
    pub(crate) clock: Duration,
    /// how far past its end a break has run, zero otherwise
    pub(crate) over: Duration,
    pub(crate) detail: String,
    pub(crate) editable: bool,
    pub(crate) buttons: &'static [Action],
    /// the emphasized button; the others are plain
    pub(crate) primary: Action,
}

impl Reading {
    /// what the panel shows: the clock, counting up as "+mm:ss" once it's over
    pub(crate) fn panel(&self) -> String {
        if self.over.is_zero() {
            fmt(self.clock)
        } else {
            format!("+{}", fmt(self.over))
        }
    }
}

impl Action {
    /// Stop reads as "Reset" mid-session and "End break" during a break
    pub(crate) fn label(self, phase: Phase) -> &'static str {
        match (self, phase) {
            (Action::Start, _) => tr().start,
            (Action::TakeBreak, _) => tr().take_break,
            (Action::Stop, Phase::Break(..)) => tr().skip,
            (Action::Stop, _) => tr().reset,
        }
    }
}

impl Phase {
    /// Decided from one `now`, so a single render can't disagree with itself.
    pub(crate) fn read(self, now: SystemTime) -> Reading {
        let t = tr();
        match self {
            Phase::Idle => Reading {
                title: t.ready,
                clock: Duration::ZERO,
                over: Duration::ZERO,
                detail: (t.idle_hint)(BREAK_RATIO),
                editable: true,
                buttons: &[Action::Start],
                primary: Action::Start,
            },
            Phase::Focus(start) => {
                let elapsed = since(start, now);
                Reading {
                    title: t.focus,
                    clock: elapsed,
                    over: Duration::ZERO,
                    detail: (t.earned)(&fmt(break_for(elapsed))),
                    editable: true,
                    buttons: &[Action::Stop, Action::TakeBreak],
                    primary: Action::TakeBreak,
                }
            }
            Phase::Break(_, end) => {
                let rest = end.duration_since(now).unwrap_or_default();
                let over = since(end, now);
                Reading {
                    title: t.brk,
                    clock: rest,
                    over,
                    detail: if over.is_zero() {
                        (t.rest)(&fmt(rest))
                    } else {
                        (t.late)(&fmt(over))
                    },
                    editable: false,
                    buttons: &[Action::Stop],
                    primary: Action::Stop,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn at(s: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(s)
    }

    fn d(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn one_reading_drives_both_surfaces() {
        let focus = Phase::Focus(at(0));
        let read = focus.read(at(65));
        assert_eq!(read.title, tr().focus);
        assert_eq!(read.clock, d(65));
        assert_eq!(read.panel(), "01:05");
        assert!(read.editable);
        assert_eq!(read.buttons, [Action::Stop, Action::TakeBreak]);
        assert_eq!(read.primary, Action::TakeBreak);
        assert!(read.detail.contains("00:13"), "a fifth of it is earned");
        assert_eq!(Action::Stop.label(focus), tr().reset);

        let brk = |now| Phase::Break(at(0), at(1000)).read(now);
        assert!(!brk(at(400)).editable, "a break clock is not editable");
        assert_eq!(brk(at(400)).clock, d(600));
        assert_eq!(brk(at(400)).panel(), "10:00");
        assert_eq!(brk(at(400)).detail, (tr().rest)("10:00"));
        assert_eq!(brk(at(1000)).clock, d(0));
        assert_eq!(brk(at(1000)).panel(), "00:00");
        assert_eq!(brk(at(1180)).panel(), "+03:00");
        assert_eq!(brk(at(1180)).detail, (tr().late)("03:00"));
        assert_eq!(brk(at(1180)).buttons, [Action::Stop]);
        assert_eq!(Action::Stop.label(Phase::Break(at(0), at(1))), tr().skip);

        let idle = Phase::Idle.read(at(0));
        assert_eq!(idle.buttons, [Action::Start]);
        assert_eq!(idle.primary, Action::Start);
        assert!(idle.editable);
    }
}
