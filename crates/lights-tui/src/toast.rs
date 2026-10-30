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

//! A short message in the corner that disappears after a moment.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::theme::ACCENT;

pub(crate) struct Toast {
    text: String,
    shown: Instant,
}

impl Toast {
    const TIME: Duration = Duration::from_secs(1);

    pub fn new(text: String) -> Self {
        Self {
            text,
            shown: Instant::now(),
        }
    }

    /// Time left before it should be hidden.
    pub fn remaining(&self) -> Duration {
        Self::TIME.saturating_sub(self.shown.elapsed())
    }

    pub fn expired(&self) -> bool {
        self.remaining().is_zero()
    }

    /// Draw in the bottom-right corner of `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let width = (self.text.chars().count() as u16 + 4).min(area.width);
        let height = 3.min(area.height);

        let at = Rect {
            x: area.x + area.width - width,
            y: area.y + area.height - height,
            width,
            height,
        };

        frame.render_widget(Clear, at);
        frame.render_widget(
            Paragraph::new(self.text.as_str()).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(ACCENT),
            ),
            at,
        );
    }
}
