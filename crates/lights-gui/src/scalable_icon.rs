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

//! A symbolic theme icon drawn to fit the space it is given.
//!
//! `gtk::Image` draws at a fixed pixel size that is also its minimum size,
//! which would stop the window from shrinking; this widget scales the icon
//! to a fraction of its smaller side instead, in the theme's text colour.

use gtk::glib;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, glib, graphene};

    /// Rasterised once at this size, then scaled.
    const LOAD_SIZE: i32 = 128;

    #[derive(Default)]
    pub struct ScalableIcon {
        pub icon_name: RefCell<String>,
        pub fraction: Cell<f32>,
        pub max_size: Cell<f32>,
        paintable: RefCell<Option<gtk::IconPaintable>>,
        scale: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ScalableIcon {
        const NAME: &'static str = "LightsScalableIcon";
        type Type = super::ScalableIcon;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ScalableIcon {}

    impl WidgetImpl for ScalableIcon {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (16, 48, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (width, height) = (widget.width() as f32, widget.height() as f32);
            let size = (width.min(height) * self.fraction.get()).min(self.max_size.get());

            if size < 1.0 {
                return;
            }

            let scale = widget.scale_factor();

            if self.paintable.borrow().is_none() || scale != self.scale.get() {
                let theme = gtk::IconTheme::for_display(&widget.display());
                let paintable = theme.lookup_icon(
                    &self.icon_name.borrow(),
                    &[],
                    LOAD_SIZE,
                    scale,
                    gtk::TextDirection::None,
                    gtk::IconLookupFlags::empty(),
                );
                self.paintable.replace(Some(paintable));
                self.scale.set(scale);
            }

            let paintable = self.paintable.borrow();

            let Some(paintable) = paintable.as_ref() else {
                return;
            };

            snapshot.save();
            snapshot.translate(&graphene::Point::new(
                (width - size) / 2.0,
                (height - size) / 2.0,
            ));
            paintable.snapshot_symbolic(
                snapshot.upcast_ref::<gdk::Snapshot>(),
                f64::from(size),
                f64::from(size),
                &[widget.color()],
            );
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    pub struct ScalableIcon(ObjectSubclass<imp::ScalableIcon>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ScalableIcon {
    /// `icon_name` drawn at `fraction` of the widget's smaller side, at most
    /// `max_size` pixels.
    pub fn new(icon_name: &str, fraction: f32, max_size: f32) -> Self {
        let icon: Self = glib::Object::builder()
            .property("hexpand", true)
            .property("vexpand", true)
            .build();
        let imp = icon.imp();
        imp.icon_name.replace(icon_name.to_owned());
        imp.fraction.set(fraction);
        imp.max_size.set(max_size);
        icon
    }
}
