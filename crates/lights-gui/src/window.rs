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

//! The main window: the link and presets buttons and other actions in the
//! header bar, the
//! monitor cards in a grid and the status bar.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lights_core::backend;
use lights_core::controller::Controller;
use lights_core::grid::Grid;
use lights_core::keys::{Action, Key, VimKeys};
use lights_core::{VERSION, legal};

use crate::card::MonitorCard;
use crate::icon::Icon;
use crate::link_button::LinkButton;
use crate::presets_button::PresetsButton;
use crate::status_bar::StatusBar;

/// Sent from controller threads to the UI thread.
enum UiEvent {
    Changed,
    Report(String),
    /// Pending writes are done; exit.
    Quit,
}

pub(crate) struct Window {
    pub window: gtk::ApplicationWindow,
    controller: Controller,
    events: async_channel::Sender<UiEvent>,
    vim: RefCell<VimKeys>,
    layout: Cell<Option<u64>>,
    shape: Cell<Option<Grid>>,
    cards: RefCell<Vec<MonitorCard>>,
    link: LinkButton,
    sync_button: gtk::Button,
    presets: Rc<PresetsButton>,
    keys_button: gtk::MenuButton,
    stack: gtk::Stack,
    status: gtk::Label,
    grid: gtk::Grid,
    status_bar: Rc<StatusBar>,
    quitting: Cell<bool>,
}

impl Window {
    // Each monitor card is about CARD_WIDTH x CARD_HEIGHT at the requested
    // size; the window never asks for more than SCREEN_FRACTION of the
    // screen. Cards and icons shrink and grow with the window after that.
    const CARD_WIDTH: i32 = 280;
    const CARD_HEIGHT: i32 = 200;
    const GAP: i32 = 16;
    /// Header bar + status bar.
    const CHROME_HEIGHT: i32 = 84;
    const SCREEN_FRACTION: f64 = 0.9;

    pub fn new(app: &gtk::Application) -> Rc<Self> {
        let (width, height) = Self::size_for(
            Grid {
                columns: 2,
                rows: 1,
            },
            None,
        );
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("lights")
            .default_width(width)
            .default_height(height)
            .build();

        let (events, rx) = async_channel::unbounded();
        let changed = events.clone();
        let reported = events.clone();
        let controller = Controller::new(
            backend::from_env(),
            move || {
                let _ = changed.send_blocking(UiEvent::Changed);
            },
            move |message| {
                let _ = reported.send_blocking(UiEvent::Report(message));
            },
        );

        let status_bar = StatusBar::new();
        let header = gtk::HeaderBar::new();
        let link = LinkButton::new();
        header.pack_start(&link.button);
        let sync_button = gtk::Button::builder()
            .label("Sync")
            .tooltip_text("Set the linked monitors to their average brightness  (=)")
            .visible(false)
            .build();
        header.pack_start(&sync_button);
        let applied = controller.clone();
        let failed = status_bar.clone();
        let presets = PresetsButton::new(
            move |level| applied.set_all(level),
            move |message| failed.show_message(&message),
        );
        header.pack_start(&presets.button);
        let about = gtk::Button::builder()
            .icon_name("help-about-symbolic")
            .tooltip_text("About and licences")
            .build();
        let keys_button = gtk::MenuButton::builder()
            .icon_name("input-keyboard-symbolic")
            .tooltip_text("Keyboard shortcuts  (?)")
            .popover(&Self::help_popover())
            .build();
        let refresh = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Re-detect monitors  (r)")
            .build();
        header.pack_end(&about);
        header.pack_end(&keys_button);
        header.pack_end(&refresh);
        window.set_titlebar(Some(&header));

        let stack = gtk::Stack::builder().vexpand(true).build();
        let status = gtk::Label::builder().css_classes(["title-2"]).build();
        stack.add_named(&status, Some("status"));
        let grid = gtk::Grid::builder()
            .row_homogeneous(true)
            .column_homogeneous(true)
            .row_spacing(Self::GAP)
            .column_spacing(Self::GAP)
            .margin_top(Self::GAP)
            .margin_bottom(Self::GAP)
            .margin_start(Self::GAP)
            .margin_end(Self::GAP)
            .build();
        stack.add_named(&grid, Some("monitors"));

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&stack);
        body.append(&status_bar.root);
        window.set_child(Some(&body));

        let ui = Rc::new(Self {
            window,
            controller,
            events,
            vim: RefCell::new(VimKeys::new()),
            layout: Cell::new(None),
            shape: Cell::new(None),
            cards: RefCell::new(Vec::new()),
            link,
            sync_button,
            presets,
            keys_button,
            stack,
            status,
            grid,
            status_bar,
            quitting: Cell::new(false),
        });

        let this = ui.clone();
        ui.link
            .button
            .connect_clicked(move |_| this.controller.toggle_lock());
        let this = ui.clone();
        ui.sync_button
            .connect_clicked(move |_| this.controller.sync());
        let this = ui.clone();
        about.connect_clicked(move |_| this.show_about());
        let this = ui.clone();
        refresh.connect_clicked(move |_| this.controller.reload());

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = ui.clone();
        keys.connect_key_pressed(move |_, keyval, _, state| this.on_key(keyval, state));
        ui.window.add_controller(keys);
        let this = ui.clone();

        ui.window.connect_close_request(move |_| {
            this.quit();
            glib::Propagation::Stop // closed by quit() once pending writes are done
        });

        let this = ui.clone();

        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                // Coalesce a burst of changes into one redraw.
                let mut changed = false;
                let mut next = Some(event);

                while let Some(event) = next {
                    match event {
                        UiEvent::Changed => changed = true,
                        UiEvent::Report(message) => this.status_bar.show_message(&message),
                        UiEvent::Quit => {
                            if let Some(app) = this.window.application() {
                                app.quit();
                            }

                            return;
                        }
                    }

                    next = rx.try_recv().ok();
                }

                if changed {
                    this.sync();
                }
            }
        });

        ui.sync();
        ui.controller.start();
        ui
    }

    /// Window size that fits `grid` of cards, capped to the screen.
    fn size_for(grid: Grid, display: Option<&gdk::Display>) -> (i32, i32) {
        let (columns, rows) = (grid.columns as i32, grid.rows as i32);
        let mut width = columns * Self::CARD_WIDTH + (columns + 1) * Self::GAP;
        let mut height = rows * Self::CARD_HEIGHT + (rows + 1) * Self::GAP + Self::CHROME_HEIGHT;
        let display = display.cloned().or_else(gdk::Display::default);
        let monitor = display
            .and_then(|d| d.monitors().item(0))
            .and_then(|m| m.downcast::<gdk::Monitor>().ok());
        if let Some(monitor) = monitor {
            let area = monitor.geometry();
            width = width.min((f64::from(area.width()) * Self::SCREEN_FRACTION) as i32);
            height = height.min((f64::from(area.height()) * Self::SCREEN_FRACTION) as i32);
        }

        (width, height)
    }

    fn help_popover() -> gtk::Popover {
        let grid = gtk::Grid::builder()
            .column_spacing(24)
            .row_spacing(6)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        for (row, (keys, what)) in VimKeys::HELP.iter().enumerate() {
            let row = row as i32;
            grid.attach(
                &gtk::Label::builder()
                    .label(*keys)
                    .xalign(0.0)
                    .css_classes(["pending"])
                    .build(),
                0,
                row,
                1,
                1,
            );
            grid.attach(
                &gtk::Label::builder().label(*what).xalign(0.0).build(),
                1,
                row,
                1,
                1,
            );
        }

        gtk::Popover::builder().child(&grid).build()
    }

    fn sync(self: &Rc<Self>) {
        let view = self.controller.view();
        self.link.set_linked(view.locked);
        self.sync_button.set_visible(view.can_sync());

        match view.status() {
            Some(status) => {
                self.status.set_label(status);
                self.stack.set_visible_child_name("status");
            }
            None => self.stack.set_visible_child_name("monitors"),
        }

        if self.layout.get() != Some(view.layout) {
            self.layout.set(Some(view.layout));
            let mut cards = self.cards.borrow_mut();

            for card in cards.drain(..) {
                self.grid.remove(&card.root);
            }

            let this = self.clone();
            let on_step: Rc<dyn Fn(u32, i32)> =
                Rc::new(move |bus, steps| this.controller.click_step(bus, steps));
            let this = self.clone();
            let on_select: Rc<dyn Fn(u32)> = Rc::new(move |bus| this.controller.select_bus(bus));
            cards.extend(
                view.monitors
                    .iter()
                    .map(|m| MonitorCard::new(m, on_step.clone(), on_select.clone())),
            );
            let grid = Grid::fit(cards.len());

            for (i, card) in cards.iter().enumerate() {
                let (column, row) = grid.position(i);
                self.grid
                    .attach(&card.root, column as i32, row as i32, 1, 1);
            }

            if !cards.is_empty() && self.shape.get() != Some(grid) {
                self.shape.set(Some(grid));
                let (width, height) = Self::size_for(grid, Some(&WidgetExt::display(&self.window)));
                self.window.set_default_size(width, height);
            }
        }

        for (card, m) in self.cards.borrow().iter().zip(&view.monitors) {
            card.update(m);
        }
    }

    fn show_about(&self) {
        let dialog = gtk::AboutDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .program_name("lights")
            .version(VERSION)
            .comments("Brightness controller for monitors via ddcutil")
            .copyright("Copyright © 2026 Armand Burger")
            .authors([format!("Armand Burger {}", legal::WEBSITE)])
            .website(legal::WEBSITE)
            .website_label(legal::CONTACT)
            .logo_icon_name(Icon::BrightnessHigh.name())
            .license_type(gtk::License::Custom)
            .license(legal::about())
            .wrap_license(false)
            .build();
        dialog.add_credit_section(
            "Icons",
            &[
                "Fluent UI System Icons © Microsoft Corporation (MIT License) \
               https://github.com/microsoft/fluentui-system-icons",
            ],
        );
        dialog.present();
    }

    fn on_key(&self, keyval: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        if self.presets.is_open()
            || state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK)
        {
            return glib::Propagation::Proceed;
        }

        let Some(key) = to_key(keyval) else {
            return glib::Propagation::Proceed;
        };

        let action = self.vim.borrow_mut().feed(key);
        self.status_bar.set_pending(&self.vim.borrow().pending());

        match action {
            Some(action) => {
                self.run(action);
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    }

    fn run(&self, action: Action) {
        let c = &self.controller;

        match action {
            Action::Pending => {}
            Action::Select(delta) => c.select(delta),
            Action::Step(steps) => c.step(steps, None),
            Action::Set(value) => c.set_level(value, None),
            Action::Lock => c.toggle_lock(),
            Action::Sync => c.sync(),
            Action::Presets => self.presets.popup(),
            Action::Reload => c.reload(),
            Action::Help => self.keys_button.popup(),
            Action::Quit => self.quit(),
        }
    }

    /// Hide at once, write still-debouncing targets, then exit.
    fn quit(&self) {
        if self.quitting.replace(true) {
            return;
        }

        self.window.set_visible(false);
        let controller = self.controller.clone();
        let events = self.events.clone();

        thread::spawn(move || {
            controller.shutdown();
            let _ = events.send_blocking(UiEvent::Quit);
        });
    }
}

fn to_key(keyval: gdk::Key) -> Option<Key> {
    match keyval {
        gdk::Key::Left => Some(Key::Left),
        gdk::Key::Right => Some(Key::Right),
        gdk::Key::Up => Some(Key::Up),
        gdk::Key::Down => Some(Key::Down),
        gdk::Key::Escape => Some(Key::Escape),
        _ => keyval
            .to_unicode()
            .filter(|c| !c.is_control())
            .map(Key::Char),
    }
}
