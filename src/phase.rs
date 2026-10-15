//! The phase machine: what a message does to a phase, and which bells it asks
//! for. Knows nothing about COSMIC, so `just test` drives it directly.

use crate::clock::{fmt, set_part, since};
use crate::history::Entry;
use crate::state::Bell;
use crate::tr::tr;
use std::time::{Duration, SystemTime};

// ponytail: fixed Flowmodoro ratio, make configurable if 1/5 doesn't fit
pub(crate) const BREAK_RATIO: u32 = 5;

// ponytail: fixed 5-minute lateness nudge, make configurable if it gets annoying
pub(crate) const LATE_NUDGE: Duration = Duration::from_secs(5 * 60);

// ponytail: a tick gap this long means the machine slept or stalled; idle ticks are 5 s, focus 1 s. Shorter sleeps still count as focus, lower it if that overcounts.
pub(crate) const AWAY_GAP: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Phase {
    Idle,
    Focus(SystemTime),
    Break(SystemTime, SystemTime), // (start, end)
}

impl Phase {
    /// How long the phase has been running. A break counts down instead, so its
    /// elapsed time is zero.
    fn elapsed(self, now: SystemTime) -> Duration {
        match self {
            Phase::Focus(start) => since(start, now),
            Phase::Idle | Phase::Break(..) => Duration::ZERO,
        }
    }
}

/// Something the user can do, and so something a popup offers as a button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Action {
    Start,
    TakeBreak,
    Stop,
}

/// Everything the phase machine takes as input, including what comes from the
/// clock rather than from the user.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Msg {
    Act(Action),
    Tick,
    /// the clock jumped past `AWAY_GAP` after `last_seen`: the machine slept
    Away {
        last_seen: SystemTime,
    },
    /// the user set clock part `unit` (seconds per unit) to `value`
    SetPart {
        unit: u64,
        value: u64,
    },
}

/// What `step` asks the outside world to do. The machine decides *which* bells
/// are due; the shared state file decides which of them are new.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Effect {
    /// fire this bell's notification, unless this bell already fired this break
    Bell(Bell),
    /// clear the bells left by the previous break
    ResetBells,
    Notify {
        summary: &'static str,
        body: String,
    },
    /// a finished focus or break, appended to the history file
    Record(Entry),
}

/// The break a focus earns.
pub(crate) fn break_for(focus: Duration) -> Duration {
    focus / BREAK_RATIO
}

/// What a message does to a phase, and the effects it wants fired. `None` means
/// the phase doesn't move, so nothing is written.
pub(crate) fn step(
    msg: Msg,
    phase: Phase,
    now: SystemTime,
    fx: &mut dyn FnMut(Effect),
) -> Option<Phase> {
    match msg {
        Msg::Tick => match phase {
            Phase::Break(_, end) if now >= end => {
                fx(Effect::Bell(Bell::Over));
                if since(end, now) >= LATE_NUDGE {
                    fx(Effect::Bell(Bell::Late));
                }
                None
            }
            // a tick during idle or focus has nothing to say
            _ => None,
        },
        // the machine slept: the focus ends at the last tick it saw and the break
        // starts there, so the sleep is rest, never focus
        Msg::Away { last_seen } => match phase {
            Phase::Focus(start) => {
                let focus = since(start, last_seen);
                if focus.is_zero() {
                    return None;
                }
                fx(Effect::Record(Entry::Focus { start, focus }));
                let brk = break_for(focus);
                // the whole break passed while asleep: record it taken in full, so
                // Stop later never sees the sleep as break time
                if now >= last_seen + brk {
                    fx(Effect::Record(Entry::Break {
                        start: last_seen,
                        earned: brk,
                        taken: brk,
                    }));
                    return Some(Phase::Idle);
                }
                fx(Effect::ResetBells);
                fx(Effect::Notify {
                    summary: tr().brk,
                    body: (tr().break_started)(&fmt(focus), &fmt(brk)),
                });
                Some(Phase::Break(last_seen, last_seen + brk))
            }
            _ => None,
        },
        // a break clock is not editable
        Msg::SetPart { .. } if matches!(phase, Phase::Break(..)) => None,
        Msg::SetPart { unit, value } => Some(Phase::Focus(
            now - set_part(phase.elapsed(now), unit, value),
        )),
        Msg::Act(Action::Start) => Some(Phase::Focus(now)),
        Msg::Act(Action::Stop) => match phase {
            // a break that ends is recorded; a focus stopped with Reset is discarded
            Phase::Break(start, end) => {
                fx(Effect::Record(Entry::Break {
                    start,
                    earned: since(start, end),
                    taken: since(start, now),
                }));
                Some(Phase::Idle)
            }
            Phase::Idle | Phase::Focus(_) => Some(Phase::Idle),
        },
        Msg::Act(Action::TakeBreak) => match phase {
            Phase::Focus(start) => {
                let focus = since(start, now);
                // a zero-length focus would drag the averages down, so it isn't kept
                if !focus.is_zero() {
                    fx(Effect::Record(Entry::Focus { start, focus }));
                }
                let brk = break_for(focus);
                fx(Effect::ResetBells);
                fx(Effect::Notify {
                    summary: tr().brk,
                    body: (tr().break_started)(&fmt(focus), &fmt(brk)),
                });
                Some(Phase::Break(now, now + brk))
            }
            // nothing to earn a break from outside a focus session
            _ => None,
        },
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

    /// run one message through the machine, collecting the effects it wants
    fn run(msg: Msg, phase: Phase, now: SystemTime) -> (Option<Phase>, Vec<Effect>) {
        let mut fx = Vec::new();
        let next = step(msg, phase, now, &mut |e| fx.push(e));
        (next, fx)
    }

    #[test]
    fn a_focus_earns_a_fifth_of_its_length_as_a_break() {
        assert_eq!(break_for(d(25 * 60)), d(BREAK_RATIO as u64 * 60));
    }

    #[test]
    fn starting_and_stopping_move_the_phase() {
        assert_eq!(
            run(Msg::Act(Action::Start), Phase::Idle, at(1000)),
            (Some(Phase::Focus(at(1000))), vec![])
        );
        // starting again mid-session restarts it
        assert_eq!(
            run(Msg::Act(Action::Start), Phase::Focus(at(0)), at(1000)),
            (Some(Phase::Focus(at(1000))), vec![])
        );
        assert_eq!(
            run(Msg::Act(Action::Stop), Phase::Focus(at(0)), at(1000)),
            (Some(Phase::Idle), vec![])
        );
        // a tick outside a finished break says nothing and writes nothing
        for p in [Phase::Idle, Phase::Focus(at(0))] {
            assert_eq!(run(Msg::Tick, p, at(1000)), (None, vec![]));
        }
    }

    #[test]
    fn a_sleep_ends_the_focus_at_the_last_tick_and_starts_the_break_there() {
        // asleep well past the break: focus stops at 100 s, and the 20 s break is already over
        let (next, fx) = run(
            Msg::Away { last_seen: at(100) },
            Phase::Focus(at(0)),
            at(5000),
        );
        assert_eq!(next, Some(Phase::Idle));
        assert_eq!(
            fx,
            vec![
                Effect::Record(Entry::Focus {
                    start: at(0),
                    focus: d(100),
                }),
                Effect::Record(Entry::Break {
                    start: at(100),
                    earned: d(20),
                    taken: d(20),
                }),
            ]
        );
        // woke inside the break: it keeps running from the sleep moment
        let (next, _) = run(
            Msg::Away { last_seen: at(100) },
            Phase::Focus(at(0)),
            at(110),
        );
        assert_eq!(next, Some(Phase::Break(at(100), at(120))));
    }

    #[test]
    fn a_sleep_outside_a_focus_or_with_no_focus_changes_nothing() {
        for p in [Phase::Idle, Phase::Break(at(0), at(60))] {
            assert_eq!(
                run(Msg::Away { last_seen: at(100) }, p, at(5000)),
                (None, vec![])
            );
        }
        // the focus started after the last tick the machine saw: nothing to end
        assert_eq!(
            run(
                Msg::Away { last_seen: at(0) },
                Phase::Focus(at(100)),
                at(5000)
            ),
            (None, vec![])
        );
    }

    #[test]
    fn a_break_earns_its_share_and_rearms_the_bells() {
        let (next, fx) = run(Msg::Act(Action::TakeBreak), Phase::Focus(at(0)), at(1000));
        let brk = break_for(d(1000));
        assert_eq!(next, Some(Phase::Break(at(1000), at(1000 + brk.as_secs()))));
        assert_eq!(
            fx.len(),
            3,
            "the focus is recorded, the bells are cleared and the break is announced"
        );
        assert_eq!(
            fx[0],
            Effect::Record(Entry::Focus {
                start: at(0),
                focus: d(1000)
            })
        );
        assert_eq!(fx[1], Effect::ResetBells);
        let Effect::Notify { summary, body } = &fx[2] else {
            panic!("the break is announced, not silent")
        };
        assert_eq!(*summary, tr().brk);
        assert!(body.contains("16:40"), "focus length missing from: {body}");
        assert!(body.contains("03:20"), "break length missing from: {body}");

        // already resting: nothing to earn, nothing said
        assert_eq!(
            run(
                Msg::Act(Action::TakeBreak),
                Phase::Break(at(0), at(9)),
                at(1000)
            ),
            (None, vec![])
        );
    }

    #[test]
    fn the_bells_fire_once_then_again_when_late() {
        let started = Phase::Break(at(500), at(1000));
        // a second past the end: only the break-over bell is due
        assert_eq!(
            run(Msg::Tick, started, at(1001)),
            (None, vec![Effect::Bell(Bell::Over)])
        );
        // a second short of the nudge: still only the first
        assert_eq!(
            run(Msg::Tick, started, at(1000 + 300) - Duration::from_secs(1)),
            (None, vec![Effect::Bell(Bell::Over)])
        );
        // exactly at the nudge: both, and the nudge is quoted in minutes
        let (next, fx) = run(Msg::Tick, started, at(1000) + LATE_NUDGE);
        assert_eq!(next, None, "the overrun doesn't move the phase");
        assert_eq!(fx, vec![Effect::Bell(Bell::Over), Effect::Bell(Bell::Late)]);
        let (_, body) = Bell::Late.copy();
        assert!(body.contains(&(LATE_NUDGE.as_secs() / 60).to_string()));
    }

    #[test]
    fn editing_starts_or_corrects_a_focus_session() {
        // typing while idle starts a session with that much already counted
        assert_eq!(
            run(
                Msg::SetPart {
                    unit: 60,
                    value: 25
                },
                Phase::Idle,
                at(0)
            ),
            (Some(Phase::Focus(at(0) - d(1500))), vec![])
        );
        // editing down corrects the session that is running
        assert_eq!(
            run(
                Msg::SetPart { unit: 60, value: 5 },
                Phase::Focus(at(0)),
                at(20000)
            ),
            (Some(Phase::Focus(at(1680))), vec![])
        );
        // a break clock is not editable
        assert_eq!(
            run(
                Msg::SetPart { unit: 60, value: 5 },
                Phase::Break(at(0), at(9)),
                at(20000)
            ),
            (None, vec![])
        );
    }

    #[test]
    fn a_finished_focus_and_break_are_recorded_but_a_reset_is_not() {
        // taking a break keeps the focus that earned it
        let (_, fx) = run(Msg::Act(Action::TakeBreak), Phase::Focus(at(0)), at(1000));
        assert_eq!(
            fx[0],
            Effect::Record(Entry::Focus {
                start: at(0),
                focus: d(1000)
            })
        );
        // zero time on the focus: nothing kept, so averages aren't skewed
        let (_, fx) = run(
            Msg::Act(Action::TakeBreak),
            Phase::Focus(at(1000)),
            at(1000),
        );
        assert!(!fx.iter().any(|e| matches!(e, Effect::Record(_))));
        // ending a break keeps what was earned and what was taken
        assert_eq!(
            run(
                Msg::Act(Action::Stop),
                Phase::Break(at(500), at(1000)),
                at(1300)
            ),
            (
                Some(Phase::Idle),
                vec![Effect::Record(Entry::Break {
                    start: at(500),
                    earned: d(500),
                    taken: d(800)
                })]
            )
        );
    }
}
