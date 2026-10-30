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

//! The header bar button that links the monitors' brightness.

use gtk::prelude::*;

use crate::icon::Icon;

pub(crate) struct LinkButton {
    pub button: gtk::ToggleButton,
    icon: gtk::Image,
    label: gtk::Label,
}

impl LinkButton {
    const LINK_TOOLTIP: &str = "Link: adjust all monitors together  (s)";
    const UNLINK_TOOLTIP: &str =
        "Linked: all monitors change together. Click to adjust each on its own  (s)";

    pub fn new() -> Self {
        let icon = gtk::Image::new();
        let label = gtk::Label::new(None);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.append(&icon);
        content.append(&label);

        let link = Self {
            button: gtk::ToggleButton::builder().child(&content).build(),
            icon,
            label,
        };

        link.set_linked(false);
        link
    }

    pub fn set_linked(&self, linked: bool) {
        let (icon, label, tooltip) = if linked {
            (Icon::Linked, "Linked", Self::UNLINK_TOOLTIP)
        } else {
            (Icon::Separate, "Separate", Self::LINK_TOOLTIP)
        };

        self.button.set_active(linked);
        self.icon.set_icon_name(Some(icon.name()));
        self.label.set_label(label);
        self.button.set_tooltip_text(Some(tooltip));
    }
}
