# Flowmodoro

The timer a COSMIC panel applet shows: focus for as long as you like, then a break
you earn in proportion. One shared state for the whole session.

## Language

**Phase**:
What the timer is doing right now: idle, focusing, or on a break. Every phase
renders a clock, a caption and a set of actions.
_Avoid_: Mode, state, status

**Focus**:
A phase with no end of its own, entered by starting the timer and left by hand.
It runs until the user stops it or takes the break it has earned.
_Avoid_: Working, session, pomodoro block

**Break**:
A phase that runs for a fixed span earned by the focus before it. It does not
end by itself: it is over, and stays over, until the user ends it.
_Avoid_: Rest, pause (in PT/ES the word for it means both)

**Break overrun**:
How far past its end a break has run, once it is over. Shown as `+mm:ss` in the
panel and captioned in the popup, so being late is visible rather than silent.
_Avoid_: Late time, overtime

**Break ratio**:
The share of a focus that a break is worth: one fifth today.
_Avoid_: Ratio constant, BREAK_RATIO (name it in code, not here)

**Late nudge**:
The second notification a break earns if the user is still on it some minutes
after it ended. Five minutes today.
_Avoid_: Reminder, second bell

**Clock part**:
One editable field of the clock — hours, minutes or seconds — holding only what
its unit can hold, so a part never carries into its neighbour.
_Avoid_: Field, segment, digit

**Reading**:
A phase as it appears at one instant: the clock, the caption, the actions and
whether the clock is editable. The panel and the popup show the same reading.
_Avoid_: View, snapshot, render

**Action**:
Something the user can do to the phase: start, take a break, stop. Actions are
offered as buttons and named in the user's language.
_Avoid_: Command, event, message

**Applet instance**:
One running copy of the applet, one per monitor, each with its own window.
Instances are peers: none owns the timer.
_Avoid_: Process, window (the window is the applet's, not the phase's)

**Shared state**:
The one phase all applet instances read and write, so every panel shows the same
timer. It survives a panel restart and clears on logout.
_Avoid_: Global state, sync

**Bell**:
A one-shot notification. Any number of applet instances see the same bell as due,
but exactly one of them is the one that rings it.
_Avoid_: Notification (a notification is what a bell produces), alert

**Part edit**:
Setting a clock part by hand, for a timer that was started late or wrongly. An
edit while idle starts a focus with that much already counted.
_Avoid_: Correction, backdating
