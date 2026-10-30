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

//! The bar along the bottom: the key legend, replaced by messages for a few
//! seconds, and the keys typed so far on the right.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, pango};
use lights_core::keys::VimKeys;

pub(crate) struct StatusBar {
    pub root: gtk::Box,
    message: gtk::Label,
    pending: gtk::Label,
    /// Bumped per message so an older message's timeout leaves a newer one.
    message_token: Cell<u64>,
}

impl StatusBar {
    const MESSAGE_SECONDS: u32 = 4;
    const LEGEND_CLASS: &str = "dim-label";

    pub fn new() -> Rc<Self> {
        let root = gtk::Box::builder().css_classes(["statusbar"]).build();
        let message = gtk::Label::builder()
            .hexpand(true)
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .build();
        let pending = gtk::Label::builder().css_classes(["pending"]).build();
        root.append(&message);
        root.append(&pending);

        let bar = Rc::new(Self {
            root,
            message,
            pending,
            message_token: Cell::new(0),
        });

        bar.show_legend();
        bar
    }

    fn show_legend(&self) {
        self.message.set_label(VimKeys::HELP_LINE);
        self.message.add_css_class(Self::LEGEND_CLASS);
    }

    pub fn show_message(self: &Rc<Self>, message: &str) {
        let token = self.message_token.get() + 1;
        self.message_token.set(token);
        self.message.set_label(message);
        self.message.remove_css_class(Self::LEGEND_CLASS);
        let this = Rc::downgrade(self);

        glib::timeout_add_seconds_local_once(Self::MESSAGE_SECONDS, move || {
            if let Some(this) = this.upgrade()
                && this.message_token.get() == token
            {
                this.show_legend();
            }
        });
    }

    pub fn set_pending(&self, keys: &str) {
        self.pending.set_label(keys);
    }
}
