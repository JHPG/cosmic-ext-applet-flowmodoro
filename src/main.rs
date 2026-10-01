use cosmic::iced::{time, window::Id, Alignment, Length, Rectangle, Subscription, Vector};
use cosmic::prelude::*;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget;
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ponytail: fixed Flowmodoro ratio, make configurable if 1/5 doesn't fit
const BREAK_RATIO: u32 = 5;

// ponytail: fixed 5-minute lateness nudge, make configurable if it gets annoying
const LATE_NUDGE: Duration = Duration::from_secs(5 * 60);
const ICON: &str = "io.github.jhpg.cosmic-ext-applet-flowmodoro-symbolic";
static EDIT_ID: LazyLock<cosmic::iced::core::widget::Id> =
    LazyLock::new(|| cosmic::iced::core::widget::Id::new("edit"));

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Idle,
    Focus(SystemTime),
    Break(SystemTime, SystemTime), // (start, end)
}

struct Applet {
    core: cosmic::Core,
    phase: Phase,
    popup: Option<Id>,
    // part being edited in the popup: (unit in seconds, typed text)
    editing: Option<(u64, String)>,
}

#[derive(Debug, Clone)]
enum Message {
    Start,
    TakeBreak,
    Stop,
    Tick,
    EditPart(u64),
    EditInput(String),
    EditConfirm,
    EditCancel,
    Surface(cosmic::surface::Action<Message>),
    PopupClosed(Id),
}

/// UI strings. ponytail: plain struct per language; move to Fluent (i18n-embed)
/// if outside translators start contributing.
struct Tr {
    ready: &'static str,
    idle_hint: &'static str,
    start: &'static str,
    focus: &'static str,
    earned: fn(&str) -> String,
    reset: &'static str,
    take_break: &'static str,
    brk: &'static str,
    rest: fn(&str) -> String,
    late: fn(&str) -> String,
    skip: &'static str,
    break_started: fn(&str, &str) -> String,
    break_over: &'static str,
    break_over_body: &'static str,
    still_away: &'static str,
    still_away_body: &'static str,
}

const EN: Tr = Tr {
    ready: "Ready",
    idle_hint: "Focus as long as you like; break = focus ÷ 5",
    start: "Start focus",
    focus: "Focus",
    earned: |b| format!("Break earned: {b}"),
    reset: "Reset",
    take_break: "Take a break",
    brk: "Break",
    rest: |e| format!("Rest · {e} so far"),
    late: |o| format!("Late by: {o}"),
    skip: "End break",
    break_started: |f, b| format!("Focused {f} → {b} break"),
    break_over: "Break over",
    break_over_body: "Time to get back to focus",
    still_away: "Still on break",
    still_away_body: "The break ended 5 minutes ago",
};

const PT: Tr = Tr {
    ready: "Pronto",
    idle_hint: "Foque o quanto quiser; pausa = foco ÷ 5",
    start: "Iniciar foco",
    focus: "Foco",
    earned: |b| format!("Pausa acumulada: {b}"),
    reset: "Reset",
    take_break: "Descansar",
    brk: "Pausa",
    rest: |e| format!("Descanse · {e} até agora"),
    late: |o| format!("Atraso: {o}"),
    skip: "Encerrar pausa",
    break_started: |f, b| format!("Foco {f} → pausa de {b}"),
    break_over: "Pausa acabou",
    break_over_body: "Hora de voltar ao foco",
    still_away: "Ainda na pausa",
    still_away_body: "A pausa acabou há 5 minutos",
};

const ES: Tr = Tr {
    ready: "Listo",
    idle_hint: "Concéntrate todo el tiempo que quieras; descanso = concentración ÷ 5",
    start: "Empezar a concentrarse",
    focus: "Concentración",
    earned: |b| format!("Descanso acumulado: {b}"),
    reset: "Reiniciar",
    take_break: "Descansar",
    brk: "Descanso",
    rest: |e| format!("Descanse · {e} hasta ahora"),
    late: |o| format!("Retraso: {o}"),
    skip: "Terminar el descanso",
    break_started: |f, b| format!("Concentración {f} → descanso de {b}"),
    break_over: "Descanso terminado",
    break_over_body: "Es hora de volver a concentrarse",
    still_away: "Todavía en descanso",
    still_away_body: "El descanso terminó hace 5 minutos",
};

/// Language from a POSIX locale ("pt_BR.UTF-8", "es_ES.UTF-8"); English otherwise.
fn pick(locale: &str) -> &'static Tr {
    if locale.starts_with("pt") {
        &PT
    } else if locale.starts_with("es") {
        &ES
    } else {
        &EN
    }
}

fn tr() -> &'static Tr {
    static T: LazyLock<&Tr> = LazyLock::new(|| {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        pick(&locale)
    });
    *T
}

fn break_for(focus: Duration) -> Duration {
    focus / BREAK_RATIO
}

/// Time split as (unit in seconds, text): [h,] mm, ss
fn parts(d: Duration) -> Vec<(u64, String)> {
    let s = d.as_secs();
    let mut v = vec![
        (60, format!("{:02}", s / 60 % 60)),
        (1, format!("{:02}", s % 60)),
    ];
    if s >= 3600 {
        v.insert(0, (3600, (s / 3600).to_string()));
    } else {
        v[0].1 = format!("{:02}", s / 60);
    }
    v
}

fn fmt(d: Duration) -> String {
    parts(d)
        .into_iter()
        .map(|(_, t)| t)
        .collect::<Vec<_>>()
        .join(":")
}

/// Replace one part (h, min or s) of `elapsed` with `v`; other parts unchanged.
fn set_part(elapsed: Duration, unit: u64, v: u64) -> Duration {
    let e = elapsed.as_secs();
    let cur = if unit == 3600 {
        e / 3600
    } else {
        e / unit % 60
    };
    Duration::from_secs(e - cur * unit + v * unit)
}

// Panel runs one applet process per monitor; they share state through this file.
// Missing file = Idle.

fn state_dir() -> PathBuf {
    let mut dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    // inside Flatpak only $XDG_RUNTIME_DIR/app/<id> is shared between instances
    if let Some(id) = std::env::var_os("FLATPAK_ID") {
        dir = dir.join("app").join(id);
    }
    dir
}

fn state_path() -> PathBuf {
    state_dir().join("flowmodoro")
}

fn bell_path(kind: &str) -> PathBuf {
    state_dir().join(format!("flowmodoro.{kind}"))
}

/// One-shot: only the instance that creates the marker notifies, so N monitors = 1 notification.
fn bell(kind: &str) -> bool {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(bell_path(kind))
        .is_ok()
}

fn encode(p: Phase) -> Option<String> {
    let ms = |t: SystemTime| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    match p {
        Phase::Idle => None,
        Phase::Focus(t) => Some(format!("focus {}", ms(t))),
        Phase::Break(s, e) => Some(format!("break {} {}", ms(s), ms(e))),
    }
}

fn decode(s: &str) -> Phase {
    let at = |v: &str| {
        v.parse()
            .ok()
            .map(|ms: u64| UNIX_EPOCH + Duration::from_millis(ms))
    };
    let mut w = s.trim().split(' ');
    match (
        w.next().unwrap_or_default(),
        w.next().and_then(at),
        w.next().and_then(at),
    ) {
        ("focus", Some(t), None) => Phase::Focus(t),
        ("break", Some(s), Some(e)) => Phase::Break(s, e),
        _ => Phase::Idle,
    }
}

fn load() -> Phase {
    fs::read_to_string(state_path()).map_or(Phase::Idle, |s| decode(&s))
}

fn save(p: Phase) {
    let path = state_path();
    match encode(p) {
        None => drop(fs::remove_file(path)),
        Some(s) => {
            // write + rename so the other instance never reads a half-written file
            let tmp = path.with_extension(std::process::id().to_string());
            if fs::write(&tmp, s).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }
}

/// Typed part value; capped so a huge number can't overflow the clock math.
fn parse_part(s: &str) -> Option<u64> {
    s.trim().parse().ok().filter(|&v| v <= 24 * 60)
}

fn since(t: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(t).unwrap_or_default()
}

/// Remaining time while the break lasts, "+mm:ss" of overrun once it is over.
fn break_clock(end: SystemTime, now: SystemTime) -> String {
    match end.duration_since(now) {
        Ok(rest) => fmt(rest),
        Err(_) => format!("+{}", fmt(since(end, now))),
    }
}

fn notify(summary: &str, body: &str) {
    let (summary, body) = (summary.to_owned(), body.to_owned());
    // notify-rust blocks on D-Bus; keep it off the UI thread
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .appname("Flowmodoro")
            .summary(&summary)
            .body(&body)
            .icon(ICON)
            .show();
    });
}

impl cosmic::Application for Applet {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "io.github.jhpg.cosmic-ext-applet-flowmodoro";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _: ()) -> (Self, Task<cosmic::Action<Message>>) {
        let app = Self {
            core,
            phase: load(),
            popup: None,
            editing: None,
        };
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        let now = SystemTime::now();
        // another monitor's instance may have changed it
        self.phase = load();
        let next = match (message, self.phase) {
            (Message::Surface(a), _) => return Task::done(cosmic::Action::Surface(a)),
            (Message::PopupClosed(id), _) => {
                self.editing = None;
                if self.popup == Some(id) {
                    self.popup = None;
                }
                return Task::none();
            }
            (Message::EditPart(unit), _) => {
                self.editing = Some((unit, String::new()));
                return widget::text_input::focus(EDIT_ID.clone());
            }
            (Message::EditInput(t), _) => {
                if let Some((_, s)) = &mut self.editing {
                    *s = t;
                }
                return Task::none();
            }
            (Message::EditCancel, _) => {
                self.editing = None;
                return Task::none();
            }
            (Message::EditConfirm, phase) => {
                let Some((unit, v)) = self
                    .editing
                    .as_ref()
                    .and_then(|(u, t)| Some((*u, parse_part(t)?)))
                else {
                    return Task::none();
                };
                self.editing = None;
                let elapsed = match phase {
                    Phase::Idle => Duration::ZERO,
                    Phase::Focus(start) => since(start, now),
                    Phase::Break(..) => return Task::none(),
                };
                Phase::Focus(now - set_part(elapsed, unit, v))
            }
            (Message::Start, _) => Phase::Focus(now),
            (Message::TakeBreak, Phase::Focus(start)) => {
                // new break: reset the one-shot markers left by the previous one
                for kind in ["over", "late"] {
                    drop(fs::remove_file(bell_path(kind)));
                }
                let focus = since(start, now);
                let b = break_for(focus);
                notify(tr().brk, &(tr().break_started)(&fmt(focus), &fmt(b)));
                Phase::Break(now, now + b)
            }
            (Message::Stop, _) => Phase::Idle,
            (Message::Tick, Phase::Break(_, end)) if now >= end => {
                let over = since(end, now);
                if bell("over") {
                    notify(tr().break_over, tr().break_over_body);
                }
                if over >= LATE_NUDGE && bell("late") {
                    notify(tr().still_away, tr().still_away_body);
                }
                return Task::none();
            }
            _ => return Task::none(),
        };
        save(next);
        self.phase = next;
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let popup = self.popup;
        let toggle = move |offset: Vector, bounds: Rectangle| match popup {
            Some(id) => Message::Surface(destroy_popup(id)),
            None => Message::Surface(app_popup::<Applet>(
                |_| Default::default(),
                move |state: &mut Applet| {
                    let id = Id::unique();
                    state.popup = Some(id);
                    let mut s = state.core.applet.get_popup_settings(
                        state.core.main_window_id().unwrap(),
                        id,
                        None,
                        None,
                        None,
                    );
                    s.positioner.anchor_rect = Rectangle {
                        x: (bounds.x - offset.x) as i32,
                        y: (bounds.y - offset.y) as i32,
                        width: bounds.width as i32,
                        height: bounds.height as i32,
                    };
                    s
                },
                Some(Box::new(|state: &Applet| {
                    state.popup_view().map(cosmic::Action::App)
                })),
            )),
        };
        let now = SystemTime::now();
        let time = match self.phase {
            Phase::Idle => {
                let icon = self
                    .core
                    .applet
                    .icon_button(ICON)
                    .on_press_with_rectangle(toggle);
                return self.core.applet.autosize_window(icon).into();
            }
            Phase::Focus(start) => fmt(since(start, now)),
            Phase::Break(_, end) => break_clock(end, now),
        };
        let applet = &self.core.applet;
        let (w, h) = applet.suggested_size(true);
        let (pad_major, pad_minor) = applet.suggested_padding(true);
        let content = widget::row::with_children(vec![
            widget::icon::from_name(ICON).icon().size(w).into(),
            applet.text(time).into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center);
        // ponytail: horizontal panel layout only; vertical panels get a wide button
        let button = widget::button::custom(
            widget::container(content).center_y(Length::Fixed(f32::from(h + 2 * pad_minor))),
        )
        .padding([0, pad_major])
        .class(cosmic::theme::Button::AppletIcon)
        .on_press_down_with_rectange(toggle);
        self.core.applet.autosize_window(button).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        self.popup_view()
    }

    fn subscription(&self) -> Subscription<Message> {
        // ponytail: always polls the state file each second (also while idle, to see a
        // start from another monitor); switch to inotify if the wakeups ever matter
        let tick = time::every(Duration::from_secs(1)).map(|_| Message::Tick);
        if self.editing.is_none() {
            return tick;
        }
        let esc = cosmic::iced::event::listen_with(|e, _, _| {
            use cosmic::iced::keyboard::{key::Named, Event::KeyPressed, Key};
            match e {
                cosmic::iced::Event::Keyboard(KeyPressed {
                    key: Key::Named(Named::Escape),
                    ..
                }) => Some(Message::EditCancel),
                _ => None,
            }
        });
        Subscription::batch([tick, esc])
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl Applet {
    fn popup_view(&self) -> Element<'_, Message> {
        let t = tr();
        let now = SystemTime::now();
        let (title, time, detail, buttons): (_, _, String, Vec<Element<_>>) = match self.phase {
            Phase::Idle => (
                t.ready,
                Duration::ZERO,
                t.idle_hint.into(),
                vec![widget::button::suggested(t.start)
                    .on_press(Message::Start)
                    .into()],
            ),
            Phase::Focus(start) => (
                t.focus,
                since(start, now),
                (t.earned)(&fmt(break_for(since(start, now)))),
                vec![
                    widget::button::standard(t.reset)
                        .on_press(Message::Stop)
                        .into(),
                    widget::button::suggested(t.take_break)
                        .on_press(Message::TakeBreak)
                        .into(),
                ],
            ),
            Phase::Break(_, end) => {
                let (detail, time) = if now < end {
                    ((t.rest)(&fmt(since(now, end))), since(now, end))
                } else {
                    ((t.late)(&fmt(since(end, now))), Duration::ZERO)
                };
                (
                    t.brk,
                    time,
                    detail,
                    vec![widget::button::suggested(t.skip)
                        .on_press(Message::Stop)
                        .into()],
                )
            }
        };
        // click a part (h/min/s) to edit it; break countdown isn't editable
        let editable = !matches!(self.phase, Phase::Break(..));
        let mut clock = widget::Row::new().align_y(Alignment::Center);
        for (i, (unit, text)) in parts(time).into_iter().enumerate() {
            if i > 0 {
                clock = clock.push(widget::text::title1(":"));
            }
            clock = clock.push(match &self.editing {
                Some((u, typed)) if *u == unit => widget::text_input(text, typed)
                    .id(EDIT_ID.clone())
                    .on_input(Message::EditInput)
                    .on_submit(|_| Message::EditConfirm)
                    // match title1 (35px / 52px line) so the clock doesn't jump while editing
                    .size(35.)
                    .line_height(cosmic::iced::widget::text::LineHeight::Absolute(
                        52.0.into(),
                    ))
                    .padding([0, 6])
                    .width(Length::Fixed(56.))
                    .into(),
                _ if editable => widget::button::custom(widget::text::title1(text))
                    .class(cosmic::theme::Button::Text)
                    .padding(0)
                    .on_press(Message::EditPart(unit))
                    .into(),
                _ => Element::from(widget::text::title1(text)),
            });
        }
        if let Some((_, typed)) = &self.editing {
            clock = clock.push(
                widget::button::icon(widget::icon::from_name("object-select-symbolic"))
                    .on_press_maybe(parse_part(typed).map(|_| Message::EditConfirm)),
            );
        }
        let content = widget::Column::new()
            .push(widget::text::heading(title))
            .push(clock)
            .push(widget::text::caption(detail))
            .push(widget::row::with_children(buttons).spacing(8))
            .spacing(8)
            .padding(16)
            .width(Length::Fill)
            .align_x(Alignment::Center);
        self.core.applet.popup_container(content).into()
    }
}

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<Applet>(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_and_format() {
        assert_eq!(
            break_for(Duration::from_secs(25 * 60)),
            Duration::from_secs(5 * 60)
        );
        assert_eq!(fmt(Duration::from_secs(65)), "01:05");
        assert_eq!(fmt(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn language() {
        assert_eq!(pick("pt_BR.UTF-8").focus, "Foco");
        assert_eq!(pick("pt_PT").focus, "Foco");
        assert_eq!(pick("es_ES.UTF-8").focus, "Concentración");
        assert_eq!(pick("es_MX").skip, "Terminar el descanso");
        assert_eq!(pick("en_US.UTF-8").focus, "Focus");
        assert_eq!(pick("").focus, "Focus");
    }

    #[test]
    fn state_roundtrip() {
        let t = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        for p in [
            Phase::Focus(t),
            Phase::Break(t, t + Duration::from_secs(600)),
        ] {
            assert_eq!(decode(&encode(p).unwrap()), p);
        }
        assert_eq!(encode(Phase::Idle), None);
        assert_eq!(decode("break 1700000000123"), Phase::Idle);
        assert_eq!(decode("focus 1 2"), Phase::Idle);
        assert_eq!(decode("garbage"), Phase::Idle);
        assert_eq!(parse_part(" 25 "), Some(25));
        assert_eq!(parse_part("-5"), None);
        assert_eq!(parse_part("99999999999999999"), None);
        // 05:30 → minutes=25 → 25:30; 1:02:05 → seconds=0 → 1:02:00
        let d = |s| Duration::from_secs(s);
        assert_eq!(set_part(d(330), 60, 25), d(1530));
        assert_eq!(set_part(d(3725), 1, 0), d(3720));
        assert_eq!(set_part(d(3725), 3600, 2), d(7325));
    }

    #[test]
    fn break_overrun_is_reported() {
        let t = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let over = |d: u64| break_clock(t, t + Duration::from_secs(d));
        assert_eq!(break_clock(t, t - Duration::from_secs(600)), "10:00");
        assert_eq!(over(1), "+00:01");
        assert_eq!(over(180), "+03:00");
        assert_eq!(over(3725), "+1:02:05");
    }

    #[test]
    fn break_bell_fires_once() {
        let dir = std::env::temp_dir().join(format!("flowmodoro-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
        assert!(bell("t"));
        assert!(!bell("t"));
        assert!(bell("u"));
        drop(std::fs::remove_dir_all(&dir));
    }
}
