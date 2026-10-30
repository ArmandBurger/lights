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

//! GTK 4 desktop UI for lights.
//!
//! Same model as the TUI: each monitor is a screen with a minus half and a
//! plus half (click to step by 5, snapping to multiples of 5), its name and
//! info underneath. Every action is also on the keyboard with vim-style keys
//! (see `keys::HELP`), handled before GTK's own focus navigation. The About
//! dialog carries the copyright, licence and third-party notices.

mod card;
mod icon;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;

use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use lights_core::backend;
use lights_core::controller::{Controller, grid_shape};
use lights_core::keys::{Action, HELP, HELP_LINE, Key, STEP, VimKeys};
use lights_core::{VERSION, legal};

use crate::card::MonitorCard;

pub const APP_ID: &str = "io.github.armandburger.lights";
/// Holds `scalable/actions/*-symbolic.svg` (see `assets/icons/lights.gresource.xml`).
const ICONS_RESOURCE: &str = "/io/github/armandburger/lights/icons";
const ICON_ADD: &str = "ic_fluent_add_24_filled-symbolic";
const ICON_SUBTRACT: &str = "ic_fluent_subtract_24_filled-symbolic";
const ICON_HIGH: &str = "ic_fluent_brightness_high_24_filled-symbolic";
const ICON_LOW: &str = "ic_fluent_brightness_low_24_filled-symbolic";
const ICON_LINKED: &str = "ic_fluent_link_24_filled-symbolic";
const ICON_SEPARATE: &str = "ic_fluent_link_dismiss_24_filled-symbolic";
const LINK_TOOLTIP: &str = "Link: adjust all monitors together  (s)";
const UNLINK_TOOLTIP: &str = "Linked: all monitors change together. Click to adjust each on its own  (s)";
const MESSAGE_SECONDS: u32 = 4;

// Window sizing: each monitor card is about CARD_WIDTH x CARD_HEIGHT at the
// requested size; the window never asks for more than SCREEN_FRACTION of
// the screen. Cards and icons shrink and grow with the window after that.
const CARD_WIDTH: i32 = 280;
const CARD_HEIGHT: i32 = 200;
const GAP: i32 = 16;
/// Header bar + status bar.
const CHROME_HEIGHT: i32 = 84;
const SCREEN_FRACTION: f64 = 0.9;

/// Styles for the cards and the status bar.
const CSS: &str = include_str!("../assets/style.css");

/// Sent from controller threads to the UI thread.
enum UiEvent {
    Changed,
    Report(String),
    /// Pending writes are done; exit.
    Quit,
}

/// Window size that fits a grid of cards, capped to the screen.
fn window_size(columns: usize, rows: usize, display: Option<&gdk::Display>) -> (i32, i32) {
    let (columns, rows) = (columns as i32, rows as i32);
    let mut width = columns * CARD_WIDTH + (columns + 1) * GAP;
    let mut height = rows * CARD_HEIGHT + (rows + 1) * GAP + CHROME_HEIGHT;
    let display = display.cloned().or_else(gdk::Display::default);
    let monitor = display
        .and_then(|d| d.monitors().item(0))
        .and_then(|m| m.downcast::<gdk::Monitor>().ok());
    if let Some(monitor) = monitor {
        let area = monitor.geometry();
        width = width.min((f64::from(area.width()) * SCREEN_FRACTION) as i32);
        height = height.min((f64::from(area.height()) * SCREEN_FRACTION) as i32);
    }
    (width, height)
}

struct Window {
    window: gtk::ApplicationWindow,
    controller: Controller,
    events: async_channel::Sender<UiEvent>,
    vim: RefCell<VimKeys>,
    layout: Cell<Option<u64>>,
    shape: Cell<(usize, usize)>,
    cards: RefCell<Vec<MonitorCard>>,
    lock_button: gtk::ToggleButton,
    lock_icon: gtk::Image,
    lock_label: gtk::Label,
    keys_button: gtk::MenuButton,
    stack: gtk::Stack,
    status: gtk::Label,
    grid: gtk::Grid,
    message: gtk::Label,
    pending: gtk::Label,
    message_token: Cell<u64>,
    quitting: Cell<bool>,
}

impl Window {
    fn new(app: &gtk::Application) -> Rc<Self> {
        let (width, height) = window_size(2, 1, None);
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

        let header = gtk::HeaderBar::new();
        let lock_icon = gtk::Image::from_icon_name(ICON_SEPARATE);
        let lock_label = gtk::Label::new(Some("Separate"));
        let lock_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        lock_content.append(&lock_icon);
        lock_content.append(&lock_label);
        let lock_button = gtk::ToggleButton::builder()
            .child(&lock_content)
            .tooltip_text(LINK_TOOLTIP)
            .build();
        header.pack_start(&lock_button);
        let about = gtk::Button::builder()
            .icon_name("help-about-symbolic")
            .tooltip_text("About and licences")
            .build();
        let keys_button = gtk::MenuButton::builder()
            .icon_name("input-keyboard-symbolic")
            .tooltip_text("Keyboard shortcuts  (?)")
            .popover(&help_popover())
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
        let status = gtk::Label::builder()
            .label("detecting monitors…")
            .css_classes(["title-2"])
            .build();
        stack.add_named(&status, Some("status"));
        let grid = gtk::Grid::builder()
            .row_homogeneous(true)
            .column_homogeneous(true)
            .row_spacing(GAP)
            .column_spacing(GAP)
            .margin_top(GAP)
            .margin_bottom(GAP)
            .margin_start(GAP)
            .margin_end(GAP)
            .build();
        stack.add_named(&grid, Some("monitors"));

        let bar = gtk::Box::builder().css_classes(["statusbar"]).build();
        let message = gtk::Label::builder()
            .label(HELP_LINE)
            .css_classes(["dim-label"])
            .hexpand(true)
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .build();
        let pending = gtk::Label::builder().css_classes(["pending"]).build();
        bar.append(&message);
        bar.append(&pending);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&stack);
        body.append(&bar);
        window.set_child(Some(&body));

        let ui = Rc::new(Self {
            window,
            controller,
            events,
            vim: RefCell::new(VimKeys::new()),
            layout: Cell::new(None),
            shape: Cell::new((0, 0)),
            cards: RefCell::new(Vec::new()),
            lock_button,
            lock_icon,
            lock_label,
            keys_button,
            stack,
            status,
            grid,
            message,
            pending,
            message_token: Cell::new(0),
            quitting: Cell::new(false),
        });

        let this = ui.clone();
        ui.lock_button
            .connect_clicked(move |_| this.controller.toggle_lock());
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
                        UiEvent::Report(message) => this.show_message(&message),
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

        ui.controller.start();
        ui
    }

    // -- drawing ---------------------------------------------------------

    fn sync(self: &Rc<Self>) {
        let view = self.controller.view();
        self.lock_button.set_active(view.locked);
        let (icon, label, tooltip) = if view.locked {
            (ICON_LINKED, "Linked", UNLINK_TOOLTIP)
        } else {
            (ICON_SEPARATE, "Separate", LINK_TOOLTIP)
        };
        self.lock_icon.set_icon_name(Some(icon));
        self.lock_label.set_label(label);
        self.lock_button.set_tooltip_text(Some(tooltip));

        if view.monitors.is_empty() {
            self.status.set_label(if view.detected {
                "no monitors responding  (r to retry)"
            } else {
                "detecting monitors…"
            });
            self.stack.set_visible_child_name("status");
        } else {
            self.stack.set_visible_child_name("monitors");
        }

        if self.layout.get() != Some(view.layout) {
            self.layout.set(Some(view.layout));
            let mut cards = self.cards.borrow_mut();
            for card in cards.drain(..) {
                self.grid.remove(&card.root);
            }
            let this = self.clone();
            let on_step: Rc<dyn Fn(u32, i32)> = Rc::new(move |bus, direction| {
                this.controller.select_bus(bus);
                this.controller.step(direction, STEP, Some(bus));
            });
            let this = self.clone();
            let on_select: Rc<dyn Fn(u32)> = Rc::new(move |bus| this.controller.select_bus(bus));
            cards.extend(
                view.monitors
                    .iter()
                    .map(|m| MonitorCard::new(m, on_step.clone(), on_select.clone())),
            );
            let (columns, rows) = grid_shape(cards.len());
            for (i, card) in cards.iter().enumerate() {
                self.grid
                    .attach(&card.root, (i % columns) as i32, (i / columns) as i32, 1, 1);
            }
            if !cards.is_empty() && (columns, rows) != self.shape.get() {
                self.shape.set((columns, rows));
                let (width, height) =
                    window_size(columns, rows, Some(&WidgetExt::display(&self.window)));
                self.window.set_default_size(width, height);
            }
        }
        for (card, m) in self.cards.borrow().iter().zip(&view.monitors) {
            card.update(m);
        }
    }

    fn show_message(self: &Rc<Self>, message: &str) {
        let token = self.message_token.get() + 1;
        self.message_token.set(token);
        self.message.set_label(message);
        self.message.remove_css_class("dim-label");
        let this = Rc::downgrade(self);
        glib::timeout_add_seconds_local_once(MESSAGE_SECONDS, move || {
            // Only clear if no newer message arrived in the meantime.
            if let Some(this) = this.upgrade()
                && this.message_token.get() == token
            {
                this.message.set_label(HELP_LINE);
                this.message.add_css_class("dim-label");
            }
        });
    }

    fn show_about(&self) {
        let dialog = gtk::AboutDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .program_name("lights")
            .version(VERSION)
            .comments("Brightness controller for monitors via ddcutil")
            .copyright("Copyright © 2026 Armand Burger")
            .authors(["Armand Burger https://github.com/ArmandBurger"])
            .website("https://github.com/ArmandBurger")
            .website_label("GitHub @ArmandBurger · Twitter/X @ghastly_dev")
            .logo_icon_name(ICON_HIGH)
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

    // -- input -----------------------------------------------------------

    fn on_key(&self, keyval: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
            return glib::Propagation::Proceed;
        }
        let key = match keyval {
            gdk::Key::Left => Key::Left,
            gdk::Key::Right => Key::Right,
            gdk::Key::Up => Key::Up,
            gdk::Key::Down => Key::Down,
            gdk::Key::Escape => Key::Escape,
            _ => match keyval.to_unicode() {
                Some(c) if !c.is_control() => Key::Char(c),
                _ => return glib::Propagation::Proceed,
            },
        };
        let action = self.vim.borrow_mut().feed(key);
        self.pending.set_label(&self.vim.borrow().pending());
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
            Action::Step { steps, size } => c.step(steps, size, None),
            Action::Set(value) => c.set_level(value, None),
            Action::Lock => c.toggle_lock(),
            Action::Reload => c.reload(),
            Action::Help => self.keys_button.popup(),
            Action::Quit => self.quit(),
        }
    }

    // -- quitting --------------------------------------------------------

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

fn help_popover() -> gtk::Popover {
    let grid = gtk::Grid::builder()
        .column_spacing(24)
        .row_spacing(6)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    for (row, (keys, what)) in HELP.iter().enumerate() {
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

fn startup() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    gtk::IconTheme::for_display(&display).add_resource_path(ICONS_RESOURCE);
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Run the desktop UI until it is closed; returns the exit status.
pub fn run() -> i32 {
    gio::resources_register_include!("lights.gresource").expect("icons are compiled in");
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| startup());
    // A second launch re-activates the running instance.
    let window: RefCell<Option<Rc<Window>>> = RefCell::new(None);
    app.connect_activate(move |app| {
        let ui = window
            .borrow_mut()
            .get_or_insert_with(|| Window::new(app))
            .clone();
        ui.window.present();
    });
    // Our own arguments were parsed already; GTK gets none.
    i32::from(app.run_with_args::<&str>(&[]).get())
}
