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

//! The Fluent UI System Icons compiled into the binary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Add,
    Subtract,
    BrightnessHigh,
    BrightnessLow,
    Linked,
    Separate,
}

impl Icon {
    /// Holds `scalable/actions/*-symbolic.svg` (see `assets/icons/lights.gresource.xml`).
    pub const RESOURCE_PATH: &str = "/io/github/armandburger/lights/icons";

    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "ic_fluent_add_24_filled-symbolic",
            Self::Subtract => "ic_fluent_subtract_24_filled-symbolic",
            Self::BrightnessHigh => "ic_fluent_brightness_high_24_filled-symbolic",
            Self::BrightnessLow => "ic_fluent_brightness_low_24_filled-symbolic",
            Self::Linked => "ic_fluent_link_24_filled-symbolic",
            Self::Separate => "ic_fluent_link_dismiss_24_filled-symbolic",
        }
    }
}
