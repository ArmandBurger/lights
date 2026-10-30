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

//! Everything the lights CLI, TUI and desktop UI share: monitor detection
//! and brightness over `ddcutil`, the controller holding brightness state,
//! the vim-style key handling, the user's config and the licence texts.

pub mod backend;
pub mod brightness;
pub mod config;
pub mod controller;
pub mod ddcutil;
pub mod grid;
pub mod keys;
pub mod legal;
pub mod monitors;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
