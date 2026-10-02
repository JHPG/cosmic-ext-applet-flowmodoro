//! The applet adapter: everything that needs a COSMIC surface lives here. The
//! phase machine, the reading, the shared state file and the strings each have
//! their own module; see `docs/adr/0001` for why the instances share a file.

mod clock;
mod phase;
mod reading;
mod state;
mod tr;

use cosmic::iced::{time, window::Id, Alignment, Length, Rectangle, Subscription, Vector};
use cosmic::prelude::*;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use clock::{parse_part, parts};
use phase::{step, Action, Effect, Msg, Phase};
use state::{load, save, Bell};

const ICON: &str = "io.github.jhpg.cosmic-ext-applet-flowmodoro-symbolic";
static EDIT_ID: LazyLock<cosmic::iced::core::widget::Id> =
    LazyLock::new(|| cosmic::iced::core::widget::Id::new("edit"));

struct Applet {
    core: cosmic::Core,
    phase: Phase,
    popup: Option<Id>,
    // part being edited in the popup: (unit in seconds, typed text)
    editing: Option<(u64, String)>,
}

#[derive(Debug, Clone)]
enum Message {
    Do(Action),
    Tick,
    EditPart(u64),
    EditInput(String),
    EditConfirm,
    EditCancel,
    Surface(cosmic::surface::Action<Message>),
    PopupClosed(Id),
}

/// The one place a phase decision reaches the outside world: the shared state
/// file decides which bells are new, this session fires them.
fn fire(e: Effect) {
    match e {
        Effect::Notify { summary, body } => notify(summary, &body),
        Effect::ResetBells => Bell::clear_all(),
        Effect::Bell(b) => {
            if b.fires() {
                let (summary, body) = b.copy();
                notify(summary, &body);
            }
        }
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
        // Surface and edit messages never move the phase, so they never touch
        // the shared state file either.
        let msg = match message {
            Message::Surface(a) => return Task::done(cosmic::Action::Surface(a)),
            Message::PopupClosed(id) => {
                self.editing = None;
                if self.popup == Some(id) {
                    self.popup = None;
                }
                return Task::none();
            }
            Message::EditPart(unit) => {
                self.editing = Some((unit, String::new()));
                return widget::text_input::focus(EDIT_ID.clone());
            }
            Message::EditInput(t) => {
                if let Some((_, s)) = &mut self.editing {
                    *s = t;
                }
                return Task::none();
            }
            Message::EditCancel => {
                self.editing = None;
                return Task::none();
            }
            Message::Tick => Msg::Tick,
            Message::Do(action) => Msg::Act(action),
            // a value the clock can't hold keeps the editor open to be fixed
            Message::EditConfirm => {
                let Some((unit, value)) = self
                    .editing
                    .as_ref()
                    .and_then(|(u, t)| parse_part(*u, t).map(|v| (*u, v)))
                else {
                    return Task::none();
                };
                self.editing = None;
                Msg::SetPart { unit, value }
            }
        };
        // another applet instance may have changed the file
        let phase = load();
        self.phase = phase;
        let next = step(msg, phase, SystemTime::now(), &mut fire);
        let Some(next) = next else {
            return Task::none();
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
        if self.phase == Phase::Idle {
            let icon = self
                .core
                .applet
                .icon_button(ICON)
                .on_press_with_rectangle(toggle);
            return self.core.applet.autosize_window(icon).into();
        }
        let applet = &self.core.applet;
        let (w, h) = applet.suggested_size(true);
        let (pad_major, pad_minor) = applet.suggested_padding(true);
        let content = widget::row::with_children(vec![
            widget::icon::from_name(ICON).icon().size(w).into(),
            applet
                .text(self.phase.read(SystemTime::now()).panel())
                .into(),
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
        // start from another applet instance); switch to inotify if the wakeups matter
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
        let read = self.phase.read(SystemTime::now());
        // click a part (h/min/s) to edit it; a break clock isn't editable
        let mut clock = widget::Row::new().align_y(Alignment::Center);
        for (i, (unit, text)) in parts(read.clock).into_iter().enumerate() {
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
                _ if read.editable => widget::button::custom(widget::text::title1(text))
                    .class(cosmic::theme::Button::Text)
                    .padding(0)
                    .on_press(Message::EditPart(unit))
                    .into(),
                _ => Element::from(widget::text::title1(text)),
            });
        }
        if let Some((unit, typed)) = &self.editing {
            clock = clock.push(
                widget::button::icon(widget::icon::from_name("object-select-symbolic"))
                    .on_press_maybe(parse_part(*unit, typed).map(|_| Message::EditConfirm)),
            );
        }
        let buttons = read
            .buttons
            .iter()
            .map(|&a| {
                let on_press = Message::Do(a);
                let label = a.label(self.phase);
                if a == read.primary {
                    widget::button::suggested(label).on_press(on_press).into()
                } else {
                    widget::button::standard(label).on_press(on_press).into()
                }
            })
            .collect::<Vec<Element<'_, Message>>>();
        let content = widget::Column::new()
            .push(widget::text::heading(read.title))
            .push(clock)
            .push(widget::text::caption(read.detail))
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
