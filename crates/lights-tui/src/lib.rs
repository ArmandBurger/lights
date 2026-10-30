// lights - brightness controller for monitors via ddcutil
// Copyright (C) 2026  Armand Burger
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Interactive TUI for lights (ratatui + crossterm).
//!
//! Each monitor is drawn as a screen: click its left half (minus) or right
//! half (plus) to step brightness by 5, snapping to a multiple of 5 first
//! (74 -> 75 -> 80). Click a screen to select it. Keys are vim-style (see
//! `keys::HELP`); w / c / n show the warranty, the licence and third-party
//! notices; p picks a preset from the config file. Brightness state,
//! debouncing and the lock live in `Controller`.

mod draw;
mod glyph;
mod hit;
mod overlay;
mod presets;
mod theme;
mod toast;

use std::io::{self, stdout};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use lights_core::backend;
use lights_core::config::Config;
use lights_core::controller::{Controller, View};
use lights_core::keys::{Action, Key, VimKeys};
use lights_core::legal;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as Input, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use ratatui::crossterm::execute;

use crate::hit::{Hit, Hits};
use crate::overlay::Overlay;
use crate::presets::{Pick, PresetPicker};
use crate::toast::Toast;

enum Event {
    Changed,
    Report(String),
    Input(Input),
}

struct App {
    controller: Controller,
    vim: VimKeys,
    view: View,
    toast: Option<Toast>,
    overlay: Option<Overlay>,
    picker: Option<PresetPicker>,
    hits: Hits,
    hover: Option<Hit>,
    quit: bool,
}

/// Run the TUI until the user quits.
pub fn run() -> io::Result<()> {
    let (tx, rx) = mpsc::channel();
    let changed = tx.clone();
    let reported = tx.clone();
    let controller = Controller::new(
        backend::from_env(),
        move || {
            let _ = changed.send(Event::Changed);
        },
        move |message| {
            let _ = reported.send(Event::Report(message));
        },
    );

    thread::spawn(move || {
        while let Ok(input) = event::read() {
            if tx.send(Event::Input(input)).is_err() {
                break;
            }
        }
    });

    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture)?;
    let restore_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture);
        restore_hook(info);
    }));

    let mut app = App::new(controller.clone());
    controller.start();
    let result = app.event_loop(&mut terminal, &rx);

    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    // Write any still-debouncing targets; the terminal is already back.
    controller.shutdown();
    result
}

impl App {
    fn new(controller: Controller) -> Self {
        Self {
            view: controller.view(),
            controller,
            vim: VimKeys::new(),
            toast: None,
            overlay: None,
            picker: None,
            hits: Hits::default(),
            hover: None,
            quit: false,
        }
    }

    fn event_loop(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        rx: &mpsc::Receiver<Event>,
    ) -> io::Result<()> {
        loop {
            self.view = self.controller.view();
            terminal.draw(|frame| self.draw(frame))?;
            let timeout = self
                .toast
                .as_ref()
                .map_or(Duration::from_secs(60), Toast::remaining);
            match rx.recv_timeout(timeout) {
                Ok(event) => self.handle(event),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }

            // Coalesce bursts (e.g. mouse movement) into one redraw.
            while let Ok(event) = rx.try_recv() {
                self.handle(event);
            }

            if self.toast.as_ref().is_some_and(Toast::expired) {
                self.toast = None;
            }

            if self.quit {
                return Ok(());
            }
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Changed => {}
            Event::Report(message) => self.toast = Some(Toast::new(message)),
            Event::Input(Input::Key(key)) if key.kind != KeyEventKind::Release => self.on_key(key),
            Event::Input(Input::Mouse(mouse)) => self.on_mouse(mouse),
            Event::Input(_) => {}
        }
    }

    fn on_key(&mut self, event: KeyEvent) {
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            if matches!(event.code, KeyCode::Char('c' | 'q')) {
                self.quit = true;
            }

            return;
        }

        if event.modifiers.contains(KeyModifiers::ALT) {
            return;
        }

        if let Some(overlay) = &mut self.overlay {
            if !overlay.on_key(event.code) {
                self.overlay = None;
            }

            return;
        }

        if let Some(picker) = &mut self.picker {
            let pick = picker.on_key(event.code);
            self.on_pick(pick);
            return;
        }

        let Some(key) = to_key(event.code) else {
            return;
        };

        match self.vim.feed(key) {
            Some(action) => self.run(action),
            None => match key {
                Key::Char('w') => self.show("Warranty", legal::WARRANTY.to_owned()),
                Key::Char('c') => {
                    self.show("GNU General Public License v3", legal::LICENSE.to_owned());
                }
                Key::Char('n') => self.show("Third-party notices", legal::NOTICES.to_owned()),
                _ => {}
            },
        }
    }

    fn run(&mut self, action: Action) {
        let c = &self.controller;

        match action {
            Action::Pending => {}
            Action::Select(delta) => c.select(delta),
            Action::Step(steps) => c.step(steps, None),
            Action::Set(value) => c.set_level(value, None),
            Action::Lock => c.toggle_lock(),
            Action::Sync => c.sync(),
            Action::Presets => match Config::load() {
                Ok(config) => {
                    self.picker = Some(PresetPicker::new(config.presets(), config.file()))
                }
                Err(e) => self.toast = Some(Toast::new(e)),
            },
            Action::Reload => c.reload(),
            Action::Help => {
                let text: Vec<String> = VimKeys::HELP
                    .iter()
                    .map(|(keys, what)| format!("{keys:<18}{what}"))
                    .collect();
                self.show("Keys", text.join("\n"));
            }
            Action::Quit => self.quit = true,
        }
    }

    fn on_pick(&mut self, pick: Pick) {
        match pick {
            Pick::Keep => {}
            Pick::Close => self.picker = None,
            Pick::Apply(level) => {
                self.picker = None;
                self.controller.set_all(level);
            }
        }
    }

    fn show(&mut self, title: &'static str, text: String) {
        self.overlay = Some(Overlay::new(title, text));
    }

    fn on_mouse(&mut self, mouse: MouseEvent) {
        if let Some(overlay) = &mut self.overlay {
            match mouse.kind {
                MouseEventKind::ScrollDown => overlay.scroll_wheel(true),
                MouseEventKind::ScrollUp => overlay.scroll_wheel(false),
                _ => {}
            }

            return;
        }

        let hit = self.hits.at(mouse.column, mouse.row);

        if let Some(picker) = &self.picker {
            match mouse.kind {
                MouseEventKind::Moved | MouseEventKind::Drag(_) => self.hover = hit,
                MouseEventKind::Down(MouseButton::Left) => {
                    let pick = match hit {
                        Some(Hit::Preset(i)) => picker.pick(i),
                        _ => Pick::Close,
                    };

                    self.on_pick(pick);
                }
                _ => {}
            }

            return;
        }

        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => self.hover = hit,
            MouseEventKind::Down(MouseButton::Left) => match hit {
                Some(Hit::Lock) => self.controller.toggle_lock(),
                Some(Hit::Sync) => self.controller.sync(),
                Some(Hit::About) => self.show("About", legal::about()),
                Some(Hit::Select(bus)) => self.controller.select_bus(bus),
                Some(Hit::Step { bus, steps }) => self.controller.click_step(bus, steps),
                Some(Hit::Preset(_)) | None => {}
            },
            _ => {}
        }
    }
}

fn to_key(code: KeyCode) -> Option<Key> {
    match code {
        KeyCode::Char(c) => Some(Key::Char(c)),
        KeyCode::Left => Some(Key::Left),
        KeyCode::Right => Some(Key::Right),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Esc => Some(Key::Escape),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lights_core::backend::Fake;
    use lights_core::config::Preset;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEventState;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Instant;

    fn app(monitors: u32) -> App {
        let controller =
            Controller::new(Arc::new(Fake::new(monitors, Duration::ZERO)), || {}, |_| {});
        controller.reload();
        let deadline = Instant::now() + Duration::from_secs(5);

        while !controller.view().detected {
            assert!(Instant::now() < deadline, "detection timed out");
            thread::sleep(Duration::from_millis(5));
        }

        App::new(controller)
    }

    fn render(app: &mut App, width: u16, height: u16) -> String {
        app.view = app.controller.view();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn click(app: &mut App, hit: Hit) {
        let area = app.hits.area(hit).expect("hit drawn");

        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + area.width / 2,
            row: area.y + area.height / 2,
            modifiers: KeyModifiers::NONE,
        };

        app.handle(Event::Input(Input::Mouse(mouse)));
    }

    fn key(app: &mut App, c: char) {
        let event = KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        app.handle(Event::Input(Input::Key(event)));
    }

    #[test]
    fn draws_screens_and_steps_on_click() {
        let mut app = app(2);
        let screen = render(&mut app, 120, 34);
        println!("{screen}");
        assert!(screen.contains("Fake Monitor 1  20%"));
        assert!(screen.contains("Fake Monitor 2  35%"));
        assert!(screen.contains("display 2 · card0-DP-2 · i2c-101"));
        assert!(screen.contains("██████████"));

        click(&mut app, Hit::Step { bus: 101, steps: 1 });
        let screen = render(&mut app, 120, 34);
        assert!(screen.contains("Fake Monitor 2  40% …"), "{screen}");
        click(
            &mut app,
            Hit::Step {
                bus: 100,
                steps: -1,
            },
        );
        assert_eq!(app.controller.view().monitors[0].value, Some(15));
        assert!(app.controller.view().monitors[0].selected);
    }

    #[test]
    fn keys_and_overlays() {
        let mut app = app(2);
        render(&mut app, 100, 30);
        key(&mut app, '7');
        key(&mut app, '0');
        assert!(render(&mut app, 100, 30).ends_with("  70"));
        key(&mut app, '%');
        assert_eq!(app.controller.view().monitors[0].value, Some(70));
        key(&mut app, 'w');
        let screen = render(&mut app, 100, 30);
        assert!(screen.contains("Warranty") && screen.contains("15. Disclaimer of Warranty."));
        key(&mut app, 'j'); // scrolls the overlay, not the brightness
        assert_eq!(app.controller.view().monitors[0].value, Some(70));
        key(&mut app, 'q');
        assert!(app.overlay.is_none() && !app.quit);
        key(&mut app, 'q');
        assert!(app.quit);
    }

    #[test]
    fn presets_set_every_monitor() {
        let mut app = app(2);
        let presets = vec![
            Preset {
                name: "day".to_owned(),
                level: 80,
            },
            Preset {
                name: "night".to_owned(),
                level: 30,
            },
        ];
        app.picker = Some(PresetPicker::new(presets, Path::new("/tmp/config.toml")));
        let screen = render(&mut app, 100, 30);
        assert!(screen.contains("2  night   30%"), "{screen}");
        assert!(screen.contains("edit /tmp/config.toml"), "{screen}");

        key(&mut app, 'j');
        key(&mut app, 'j');
        assert_eq!(app.controller.view().monitors[0].value, Some(20));
        key(&mut app, ' '); // j wraps back to day
        assert!(app.picker.is_none());
        let values: Vec<_> = app
            .controller
            .view()
            .monitors
            .iter()
            .map(|m| m.value)
            .collect();
        assert_eq!(values, [Some(80), Some(80)]);
    }

    #[test]
    fn dense_grid_and_small_terminal() {
        let mut app = app(9);
        let screen = render(&mut app, 120, 40);
        println!("{screen}");
        assert!(screen.contains("Fake Monitor 9"));
        assert!(screen.contains("lights Copyright (C) 2026 Armand Burger"));
        click(&mut app, Hit::About);
        let screen = render(&mut app, 120, 40);
        assert!(screen.contains("About") && screen.contains("GNU General Public License"));

        // Tiny terminals must not panic.
        for (w, h) in [(20, 8), (5, 3), (1, 1)] {
            render(&mut app, w, h);
        }

        let mut app = self::app(16);
        render(&mut app, 80, 24);
    }
}
