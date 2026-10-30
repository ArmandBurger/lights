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

//! Colours of the terminal UI.

use ratatui::style::Color;

pub(crate) const ACCENT: Color = Color::Rgb(0xff, 0xa6, 0x2b);
pub(crate) const PRIMARY: Color = Color::Rgb(0x01, 0x78, 0xd4);
pub(crate) const FRAME: Color = Color::Rgb(0x90, 0x90, 0x90);
pub(crate) const PANEL: Color = Color::Rgb(0x30, 0x30, 0x30);
pub(crate) const HOVER: Color = Color::Rgb(0x2a, 0x2a, 0x2a);
