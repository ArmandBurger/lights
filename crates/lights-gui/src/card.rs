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

//! One monitor: a screen with minus/plus halves and a caption.

use std::rc::Rc;

use gtk::pango;
use gtk::prelude::*;
use lights_core::controller::{Controller, MonitorView};

use crate::icon::Icon;
use crate::scalable_icon::ScalableIcon;

pub struct MonitorCard {
    pub root: gtk::Box,
    screen: gtk::Box,
    level_icon: gtk::Image,
    level: gtk::Label,
}

impl MonitorCard {
    /// `on_step(bus, steps)` runs when a half is clicked, `on_select(bus)`
    /// when anywhere on the card is.
    pub fn new(
        view: &MonitorView,
        on_step: Rc<dyn Fn(u32, i32)>,
        on_select: Rc<dyn Fn(u32)>,
    ) -> Self {
        let bus = view.bus;
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .hexpand(true)
            .vexpand(true)
            .build();

        let screen = gtk::Box::builder()
            .css_classes(["screen"])
            .vexpand(true)
            .build();
        let step = Controller::STEP;
        let minus = Self::half(Icon::Subtract, "minus", &format!("Dimmer by {step}%  (j)"));
        let plus = Self::half(Icon::Add, "plus", &format!("Brighter by {step}%  (k)"));
        let step = on_step.clone();
        minus.connect_clicked(move |_| step(bus, -1));
        plus.connect_clicked(move |_| on_step(bus, 1));
        screen.append(&minus);
        screen.append(
            &gtk::Separator::builder()
                .orientation(gtk::Orientation::Vertical)
                .margin_top(16)
                .margin_bottom(16)
                .build(),
        );
        screen.append(&plus);
        root.append(&screen);

        let d = &view.display;
        let caption = gtk::Box::builder()
            .spacing(8)
            .halign(gtk::Align::Center)
            .build();
        let level_icon = gtk::Image::builder().pixel_size(20).build();
        let level = gtk::Label::builder().css_classes(["level"]).build();
        caption.append(
            &gtk::Label::builder()
                .label(d.model_name())
                .css_classes(["model"])
                .ellipsize(pango::EllipsizeMode::End)
                .build(),
        );
        caption.append(&level_icon);
        caption.append(&level);
        root.append(&caption);
        root.append(
            &gtk::Label::builder()
                .label(d.summary())
                .css_classes(["info"])
                .ellipsize(pango::EllipsizeMode::End)
                .build(),
        );

        let click = gtk::GestureClick::new();
        click.connect_pressed(move |_, _, _, _| on_select(bus));
        root.add_controller(click);

        let card = Self {
            root,
            screen,
            level_icon,
            level,
        };

        card.update(view);
        card
    }

    pub fn update(&self, view: &MonitorView) {
        self.level.set_label(&view.level_text());

        let icon = if view.value.is_some_and(|v| v < 50) {
            Icon::BrightnessLow
        } else {
            Icon::BrightnessHigh
        };

        self.level_icon.set_icon_name(Some(icon.name()));

        if view.selected {
            self.screen.add_css_class("selected");
        } else {
            self.screen.remove_css_class("selected");
        }
    }

    fn half(icon: Icon, side: &str, tooltip: &str) -> gtk::Button {
        gtk::Button::builder()
            .child(&ScalableIcon::new(icon.name(), 0.5, 96.0))
            .css_classes(["step", side])
            .hexpand(true)
            .focus_on_click(false)
            .tooltip_text(tooltip)
            .build()
    }
}
