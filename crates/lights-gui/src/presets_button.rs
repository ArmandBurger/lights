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

//! The header bar button with the presets: click one to set every monitor
//! to it, or add, change and delete them. They are kept in the config file,
//! read again each time the list opens so hand edits show up.

use std::rc::Rc;

use gtk::pango;
use gtk::prelude::*;
use lights_core::config::Config;

pub(crate) struct PresetsButton {
    pub button: gtk::MenuButton,
    popover: gtk::Popover,
    list: gtk::Box,
    file: gtk::Label,
    name: gtk::Entry,
    level: gtk::SpinButton,
    on_apply: Box<dyn Fn(i32)>,
    on_error: Box<dyn Fn(String)>,
}

impl PresetsButton {
    const TOOLTIP: &str = "Presets: set all monitors to a saved level  (p)";

    pub fn new(on_apply: impl Fn(i32) + 'static, on_error: impl Fn(String) + 'static) -> Rc<Self> {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let name = gtk::Entry::builder()
            .placeholder_text("Name")
            .width_chars(10)
            .build();
        let level = gtk::SpinButton::with_range(0.0, 100.0, 5.0);
        level.set_value(50.0);
        let save = gtk::Button::builder()
            .label("Save")
            .tooltip_text("Add a preset, or change the one with this name")
            .css_classes(["suggested-action"])
            .build();

        let add = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        add.append(&name);
        add.append(&level);
        add.append(&save);

        let file = gtk::Label::builder()
            .xalign(0.0)
            .max_width_chars(30)
            .ellipsize(pango::EllipsizeMode::Start)
            .css_classes(["info"])
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        content.append(&list);
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        content.append(&add);
        content.append(&file);

        let popover = gtk::Popover::builder().child(&content).build();
        let button = gtk::MenuButton::builder()
            .label("Presets")
            .tooltip_text(Self::TOOLTIP)
            .popover(&popover)
            .build();

        let presets = Rc::new(Self {
            button,
            popover,
            list,
            file,
            name,
            level,
            on_apply: Box::new(on_apply),
            on_error: Box::new(on_error),
        });

        let this = Rc::downgrade(&presets);

        presets.popover.connect_show(move |_| {
            if let Some(this) = this.upgrade() {
                this.refill();
            }
        });

        let this = Rc::downgrade(&presets);

        save.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                this.save();
            }
        });

        let this = Rc::downgrade(&presets);

        presets.name.connect_activate(move |_| {
            if let Some(this) = this.upgrade() {
                this.save();
            }
        });

        presets
    }

    pub fn popup(&self) {
        self.button.popup();
    }

    pub fn is_open(&self) -> bool {
        self.popover.is_visible()
    }

    fn load(&self) -> Option<Config> {
        Config::load().map_err(|e| (self.on_error)(e)).ok()
    }

    /// Show the presets in the config file.
    fn refill(self: &Rc<Self>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }

        let Some(config) = self.load() else {
            return;
        };

        self.file.set_label(&config.file().display().to_string());
        let presets = config.presets();

        if presets.is_empty() {
            self.list.append(
                &gtk::Label::builder()
                    .label("No presets yet")
                    .css_classes(["info"])
                    .build(),
            );
        }

        for preset in presets {
            let label = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            label.append(
                &gtk::Label::builder()
                    .label(&preset.name)
                    .hexpand(true)
                    .xalign(0.0)
                    .build(),
            );
            label.append(
                &gtk::Label::builder()
                    .label(format!("{}%", preset.level))
                    .css_classes(["pending"])
                    .build(),
            );

            let apply = gtk::Button::builder()
                .child(&label)
                .hexpand(true)
                .css_classes(["flat"])
                .build();
            let remove = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(format!("Delete {}", preset.name))
                .css_classes(["flat"])
                .build();

            let this = Rc::downgrade(self);
            let level = preset.level;

            apply.connect_clicked(move |_| {
                if let Some(this) = this.upgrade() {
                    this.popover.popdown();
                    (this.on_apply)(level);
                }
            });

            let this = Rc::downgrade(self);
            let name = preset.name.clone();

            remove.connect_clicked(move |_| {
                if let Some(this) = this.upgrade() {
                    this.edit(|config| config.remove_preset(&name));
                }
            });

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            row.append(&apply);
            row.append(&remove);
            self.list.append(&row);
        }
    }

    fn save(self: &Rc<Self>) {
        let name = self.name.text();
        let name = name.trim();

        if name.is_empty() {
            self.name.grab_focus();
            return;
        }

        let level = self.level.value_as_int();
        self.edit(|config| config.set_preset(name, level));
        self.name.set_text("");
    }

    /// Change the config file and show the result.
    fn edit(self: &Rc<Self>, change: impl FnOnce(&mut Config)) {
        let Some(mut config) = self.load() else {
            return;
        };

        change(&mut config);

        if let Err(e) = config.save() {
            (self.on_error)(e);
        }

        self.refill();
    }
}
