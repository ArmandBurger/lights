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
//! (see `VimKeys::HELP`), handled before GTK's own focus navigation. Presets
//! live in the config file, which the presets button edits. The
//! About dialog carries the copyright, licence and third-party notices.

mod card;
mod icon;
mod link_button;
mod presets_button;
mod scalable_icon;
mod status_bar;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio};

use crate::icon::Icon;
use crate::window::Window;

pub const APP_ID: &str = "io.github.armandburger.lights";
/// Styles for the cards and the status bar.
const CSS: &str = include_str!("../assets/style.css");

fn startup() {
    let Some(display) = gdk::Display::default() else {
        return;
    };

    gtk::IconTheme::for_display(&display).add_resource_path(Icon::RESOURCE_PATH);
    gtk::Window::set_default_icon_name(APP_ID);
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
