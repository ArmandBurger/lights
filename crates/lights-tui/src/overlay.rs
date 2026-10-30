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

//! A scrollable text shown over everything (keys, about, warranty, licence).

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Margin, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::theme::ACCENT;

pub(crate) struct Overlay {
    title: &'static str,
    text: String,
    scroll: u16,
}

impl Overlay {
    const PAGE: i32 = 20;
    const WHEEL: i32 = 3;

    pub fn new(title: &'static str, text: String) -> Self {
        Self {
            title,
            text,
            scroll: 0,
        }
    }

    /// Scroll by `lines` (negative: up). Clamped to the text when drawn.
    pub fn scroll_by(&mut self, lines: i32) {
        self.scroll = self.scroll.saturating_add_signed(lines as i16);
    }

    pub fn scroll_wheel(&mut self, down: bool) {
        self.scroll_by(if down { Self::WHEEL } else { -Self::WHEEL });
    }

    /// Handle a key; false when it closes the overlay.
    pub fn on_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'w' | 'c' | 'n' | '?') => return false,
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.scroll_by(Self::PAGE),
            KeyCode::PageUp => self.scroll_by(-Self::PAGE),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = u16::MAX,
            _ => {}
        }

        true
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let full = frame.area();
        let width = 84.min(full.width);
        let height = (full.height * 9 / 10).max(3).min(full.height);

        let area = Rect {
            x: full.x + (full.width - width) / 2,
            y: full.y + (full.height - height) / 2,
            width,
            height,
        };

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(ACCENT)
            .title(format!(" {} ", self.title))
            .title_bottom(Line::raw(" Esc to close ").right_aligned());
        let inner = block.inner(area).inner(Margin::new(1, 0));
        let lines = self.text.lines().count() as u16;
        self.scroll = self.scroll.min(lines.saturating_sub(inner.height));
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(self.text.as_str()).scroll((self.scroll, 0)),
            inner,
        );
    }
}
