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

//! The preset picker: the presets from the config file, to set every
//! monitor to one. Presets are added by editing that file.

use std::path::Path;

use lights_core::config::Preset;
use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::hit::{Hit, Hits};
use crate::theme::{ACCENT, HOVER, PANEL};

/// What a key or click in the picker asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pick {
    Keep,
    Close,
    /// Set every monitor to this level.
    Apply(i32),
}

pub(crate) struct PresetPicker {
    presets: Vec<Preset>,
    selected: usize,
    file: String,
}

impl PresetPicker {
    pub fn new(presets: Vec<Preset>, file: &Path) -> Self {
        let file = file.display().to_string();

        let file = match std::env::var("HOME") {
            Ok(home) if !home.is_empty() && file.starts_with(&home) => {
                format!("~{}", &file[home.len()..])
            }
            _ => file,
        };

        Self {
            presets,
            selected: 0,
            file,
        }
    }

    pub fn pick(&self, index: usize) -> Pick {
        self.presets
            .get(index)
            .map_or(Pick::Keep, |p| Pick::Apply(p.level))
    }

    pub fn on_key(&mut self, code: KeyCode) -> Pick {
        let count = self.presets.len();

        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'p') => return Pick::Close,
            KeyCode::Enter | KeyCode::Char(' ') => return self.pick(self.selected),
            KeyCode::Char(digit @ '1'..='9') => {
                return self.pick(digit as usize - '1' as usize);
            }
            KeyCode::Char('j') | KeyCode::Down if count > 0 => {
                self.selected = (self.selected + 1) % count;
            }
            KeyCode::Char('k') | KeyCode::Up if count > 0 => {
                self.selected = (self.selected + count - 1) % count;
            }
            _ => {}
        }

        Pick::Keep
    }

    pub fn draw(&self, frame: &mut Frame, hits: &mut Hits, hover: Option<Hit>) {
        let footer = format!(" edit {} ", self.file);
        let names = self.presets.iter().map(|p| p.name.chars().count());
        let rows = names.clone().map(|name| name + 12);
        let widest = rows.max().unwrap_or(0).max(footer.chars().count() + 2);

        let full = frame.area();
        let width = (widest as u16 + 4).min(full.width);
        let lines = self.presets.len().max(2) as u16;
        let height = (lines + 2).min(full.height);

        let area = Rect {
            x: full.x + (full.width - width) / 2,
            y: full.y + (full.height - height) / 2,
            width,
            height,
        };

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(ACCENT)
            .title(" Presets ")
            .title_bottom(Line::raw(footer).right_aligned())
            .bg(PANEL);
        let inner = block.inner(area).inner(Margin::new(1, 0));
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        if self.presets.is_empty() {
            frame.render_widget(
                Paragraph::new("No presets yet: add some under [presets],\ne.g. night = 30"),
                inner,
            );
            return;
        }

        let name_width = names.max().unwrap_or(0);

        for (i, preset) in self.presets.iter().enumerate() {
            let row = Rect {
                y: inner.y + i as u16,
                height: 1,
                ..inner
            };

            if row.y >= inner.y + inner.height {
                break;
            }

            let number = if i < 9 {
                (i + 1).to_string()
            } else {
                " ".to_owned()
            };
            let text = format!(
                "{number}  {:<name_width$}  {:>3}%",
                preset.name, preset.level
            );

            let style = if i == self.selected {
                Style::new().fg(ACCENT).bold()
            } else if hover == Some(Hit::Preset(i)) {
                Style::new().bg(HOVER)
            } else {
                Style::new()
            };

            frame.render_widget(Paragraph::new(text).style(style), row);
            hits.push(row, Hit::Preset(i));
        }
    }
}
