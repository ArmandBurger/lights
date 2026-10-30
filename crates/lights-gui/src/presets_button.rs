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
//! to it. The ⋮ menu adds a preset (a row to fill in and save) or deletes
//! some (tick them, then confirm). Closing the list cancels either. Presets
//! are kept in the config file, read again each time the list opens so hand
//! edits show up.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, pango};
use lights_core::config::{Config, Preset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Apply,
    Add,
    Delete,
}

pub(crate) struct PresetsButton {
    pub button: gtk::MenuButton,
    popover: gtk::Popover,
    mode: Cell<Mode>,
    list: gtk::ListBox,
    /// The presets shown, in list order, with their delete ticks.
    rows: RefCell<Vec<(Preset, gtk::CheckButton)>>,
    add_row: gtk::Box,
    name: gtk::Entry,
    level: gtk::SpinButton,
    delete_row: gtk::Box,
    delete: gtk::Button,
    file: gtk::Label,
    on_apply: Box<dyn Fn(i32)>,
    on_error: Box<dyn Fn(String)>,
}

impl PresetsButton {
    const TOOLTIP: &str = "Presets: set all monitors to a saved level  (p)";
    /// Fixed, so adding or deleting does not resize the list.
    const WIDTH: i32 = 320;
    /// The rows' own side padding (see style.css).
    const INSET: i32 = 8;

    pub fn new(on_apply: impl Fn(i32) + 'static, on_error: impl Fn(String) + 'static) -> Rc<Self> {
        let menu = gio::Menu::new();
        menu.append(Some("New preset"), Some("presets.add"));
        menu.append(Some("Delete presets…"), Some("presets.delete"));
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Add or delete presets")
            .menu_model(&menu)
            .css_classes(["flat"])
            .build();

        // The rows carry their own padding, so everything else is inset by
        // it to line up with their text.
        let header = gtk::Box::builder()
            .spacing(6)
            .margin_start(Self::INSET)
            .build();
        header.append(
            &gtk::Label::builder()
                .label("Presets")
                .hexpand(true)
                .xalign(0.0)
                .css_classes(["heading"])
                .build(),
        );
        header.append(&more);

        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build();

        let name = gtk::Entry::builder()
            .placeholder_text("Name")
            .width_chars(4)
            .hexpand(true)
            .build();
        let level = gtk::SpinButton::with_range(0.0, 100.0, 5.0);
        level.set_width_chars(3);
        let cancel_add = gtk::Button::with_label("Cancel");
        let save = gtk::Button::builder()
            .label("Save")
            .css_classes(["suggested-action"])
            .build();
        let form = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        form.append(&name);
        form.append(&level);
        let add_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_start(Self::INSET)
            .margin_end(Self::INSET)
            .visible(false)
            .build();
        add_row.append(&form);
        add_row.append(&Self::actions(&cancel_add, &save));

        let cancel_delete = gtk::Button::with_label("Cancel");
        let delete = gtk::Button::builder()
            .label("Delete")
            .css_classes(["destructive-action"])
            .build();
        let delete_row = Self::actions(&cancel_delete, &delete);
        delete_row.set_margin_end(Self::INSET);
        delete_row.set_visible(false);

        let file = gtk::Label::builder()
            .xalign(0.0)
            .max_width_chars(1)
            .ellipsize(pango::EllipsizeMode::Start)
            .margin_start(Self::INSET)
            .margin_end(Self::INSET)
            .css_classes(["info"])
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(4)
            .margin_bottom(12)
            .margin_start(4)
            .margin_end(4)
            .width_request(Self::WIDTH)
            .css_classes(["presets"])
            .build();
        content.append(&header);
        content.append(&list);
        content.append(&add_row);
        content.append(&delete_row);
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
            mode: Cell::new(Mode::Apply),
            list,
            rows: RefCell::default(),
            add_row,
            name,
            level,
            delete_row,
            delete,
            file,
            on_apply: Box::new(on_apply),
            on_error: Box::new(on_error),
        });

        let actions = gio::SimpleActionGroup::new();

        for (action, mode) in [("add", Mode::Add), ("delete", Mode::Delete)] {
            let action = gio::SimpleAction::new(action, None);
            let this = Rc::downgrade(&presets);

            action.connect_activate(move |_, _| {
                if let Some(this) = this.upgrade() {
                    this.set_mode(mode);
                }
            });

            actions.add_action(&action);
        }

        content.insert_action_group("presets", Some(&actions));

        let this = Rc::downgrade(&presets);

        presets.popover.connect_show(move |_| {
            if let Some(this) = this.upgrade() {
                this.refill();
            }
        });

        let this = Rc::downgrade(&presets);

        presets.popover.connect_closed(move |_| {
            if let Some(this) = this.upgrade() {
                this.mode.set(Mode::Apply);
            }
        });

        for cancel in [&cancel_add, &cancel_delete] {
            let this = Rc::downgrade(&presets);

            cancel.connect_clicked(move |_| {
                if let Some(this) = this.upgrade() {
                    this.set_mode(Mode::Apply);
                }
            });
        }

        let this = Rc::downgrade(&presets);

        presets.list.connect_row_activated(move |_, row| {
            if let Some(this) = this.upgrade() {
                this.activate(row.index());
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

        let this = Rc::downgrade(&presets);

        presets.delete.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                this.delete_ticked();
            }
        });

        presets
    }

    /// Cancel and the confirming button, at the right.
    fn actions(cancel: &gtk::Button, confirm: &gtk::Button) -> gtk::Box {
        let actions = gtk::Box::builder()
            .spacing(6)
            .halign(gtk::Align::End)
            .build();
        actions.append(cancel);
        actions.append(confirm);
        actions
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

    fn set_mode(self: &Rc<Self>, mode: Mode) {
        self.mode.set(mode);

        if mode == Mode::Add {
            self.name.set_text("");
            self.level.set_value(0.0);
        }

        self.refill();

        if mode == Mode::Add {
            self.name.grab_focus();
        }
    }

    /// Show the presets in the config file, as the mode wants them: the
    /// same rows in every mode, with a tick to delete while deleting.
    fn refill(self: &Rc<Self>) {
        let mode = self.mode.get();
        self.add_row.set_visible(mode == Mode::Add);
        self.delete_row.set_visible(mode == Mode::Delete);
        self.delete.set_sensitive(false);
        self.rows.borrow_mut().clear();
        self.list.remove_all();

        let Some(config) = self.load() else {
            return;
        };

        self.file.set_label(&config.file().display().to_string());
        let presets = config.presets();

        if presets.is_empty() {
            let empty = gtk::Label::builder()
                .label("No presets yet")
                .xalign(0.0)
                .css_classes(["info"])
                .build();
            self.list.append(
                &gtk::ListBoxRow::builder()
                    .child(&empty)
                    .activatable(false)
                    .build(),
            );
        }

        for preset in presets {
            let tick = gtk::CheckButton::builder()
                .visible(mode == Mode::Delete)
                .build();
            let this = Rc::downgrade(self);

            tick.connect_toggled(move |_| {
                if let Some(this) = this.upgrade() {
                    let any = this.rows.borrow().iter().any(|(_, t)| t.is_active());
                    this.delete.set_sensitive(any);
                }
            });

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&tick);
            row.append(
                &gtk::Label::builder()
                    .label(&preset.name)
                    .hexpand(true)
                    .xalign(0.0)
                    .ellipsize(pango::EllipsizeMode::End)
                    .build(),
            );
            row.append(
                &gtk::Label::builder()
                    .label(format!("{}%", preset.level))
                    .css_classes(["pending"])
                    .build(),
            );

            self.list.append(&row);
            self.rows.borrow_mut().push((preset, tick));
        }
    }

    /// A click on the `index`th row: apply it, or tick it while deleting.
    fn activate(&self, index: i32) {
        let rows = self.rows.borrow();

        let Some((preset, tick)) = usize::try_from(index).ok().and_then(|i| rows.get(i)) else {
            return;
        };

        match self.mode.get() {
            Mode::Delete => tick.set_active(!tick.is_active()),
            Mode::Apply => {
                let level = preset.level;
                drop(rows);
                self.popover.popdown();
                (self.on_apply)(level);
            }
            Mode::Add => {}
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
    }

    fn delete_ticked(self: &Rc<Self>) {
        let names: Vec<String> = self
            .rows
            .borrow()
            .iter()
            .filter(|(_, tick)| tick.is_active())
            .map(|(preset, _)| preset.name.clone())
            .collect();

        self.edit(|config| {
            for name in &names {
                config.remove_preset(name);
            }
        });
    }

    /// Change the config file, then go back to the plain list.
    fn edit(self: &Rc<Self>, change: impl FnOnce(&mut Config)) {
        let Some(mut config) = self.load() else {
            return;
        };

        change(&mut config);

        if let Err(e) = config.save() {
            (self.on_error)(e);
        }

        self.set_mode(Mode::Apply);
    }
}
