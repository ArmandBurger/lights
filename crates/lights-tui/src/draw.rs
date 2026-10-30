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

//! Drawing. Each monitor is a "screen": a rounded box split by a vertical
//! divider, a large minus on the left half and a large plus on the right
//! half, with the display's name and info underneath. Clickable areas are
//! recorded in `App::hits` as they are drawn.

use lights_core::controller::MonitorView;
use lights_core::grid::Grid;
use lights_core::keys::VimKeys;
use lights_core::legal;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::App;
use crate::glyph::Glyph;
use crate::hit::Hit;
use crate::theme::{ACCENT, FRAME, HOVER, PANEL, PRIMARY};

impl App {
    pub(crate) fn draw(&mut self, frame: &mut Frame) {
        self.hits.clear();
        let [header, toolbar, screens, notice, keys] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        frame.render_widget(
            Paragraph::new(" ◉ lights")
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
            header,
        );
        self.draw_lock(frame, toolbar);
        self.draw_screens(frame, screens);
        self.draw_copyright(frame, notice.inner(Margin::new(2, 0)));
        frame.render_widget(
            Paragraph::new(format!("  {}", VimKeys::HELP_LINE)).bg(PANEL),
            keys,
        );
        // The pending count (vim's showcmd) sits on the right, over the help
        // text if the terminal is narrow.
        let pending = self.vim.pending();

        if !pending.is_empty() {
            let width = (pending.len() as u16 + 4).min(keys.width);

            let at = Rect {
                x: keys.x + keys.width - width,
                width,
                ..keys
            };

            frame.render_widget(
                Paragraph::new(format!("  {pending}  ")).bg(PANEL).bold(),
                at,
            );
        }

        if let Some(toast) = &self.toast {
            toast.draw(frame, screens);
        }

        if let Some(picker) = &self.picker {
            picker.draw(frame, &mut self.hits, self.hover);
        }

        if let Some(overlay) = &mut self.overlay {
            overlay.draw(frame);
        }
    }

    fn draw_copyright(&mut self, frame: &mut Frame, area: Rect) {
        let link = Rect {
            width: (legal::COPYRIGHT.chars().count() as u16).min(area.width),
            ..area
        };

        let style = if self.hover == Some(Hit::About) {
            Style::new().underlined()
        } else {
            Style::new().dim()
        };

        frame.render_widget(Paragraph::new(legal::COPYRIGHT).style(style), link);
        self.hits.push(link, Hit::About);
    }

    /// The link button, with a sync button beside it while linked monitors
    /// differ.
    fn draw_lock(&mut self, frame: &mut Frame, area: Rect) {
        let width = 24.min(area.width);

        let button = Rect {
            x: area.x + (area.width - width) / 2,
            width,
            ..area
        };

        if self.view.can_sync() {
            let sync = Rect {
                x: button.x + button.width + 2,
                width: 12,
                ..area
            }
            .intersection(area);

            let style = if self.hover == Some(Hit::Sync) {
                Style::new().bg(FRAME).fg(Color::Black)
            } else {
                Style::new().bg(PANEL)
            };

            frame.render_widget(
                Paragraph::new(vec![Line::raw(""), Line::raw("= Sync").bold()])
                    .alignment(Alignment::Center)
                    .style(style),
                sync,
            );
            self.hits.push(sync, Hit::Sync);
        }

        let locked = self.view.locked;
        let label = if locked { "⇄ Linked" } else { "Separate" };
        let hovered = self.hover == Some(Hit::Lock);

        let style = match (locked, hovered) {
            (true, _) => Style::new().bg(PRIMARY).fg(Color::White),
            (false, true) => Style::new().bg(FRAME).fg(Color::Black),
            (false, false) => Style::new().bg(PANEL),
        };

        frame.render_widget(
            Paragraph::new(vec![Line::raw(""), Line::raw(label).bold()])
                .alignment(Alignment::Center)
                .style(style),
            button,
        );
        self.hits.push(button, Hit::Lock);
    }

    fn draw_screens(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(status) = self.view.status() {
            let middle = Rect {
                y: area.y + area.height / 2,
                height: 1.min(area.height),
                ..area
            };

            frame.render_widget(Paragraph::new(status).alignment(Alignment::Center), middle);
            return;
        }

        let monitors = self.view.monitors.clone();
        let grid = Grid::fit(monitors.len());
        let dense = grid.rows > 1;

        let margin = if dense {
            Margin::new(1, 0)
        } else {
            Margin::new(2, 1)
        };

        let row_areas = Layout::vertical(vec![Constraint::Fill(1); grid.rows]).split(area);

        for (i, monitor) in monitors.iter().enumerate() {
            let (column, row) = grid.position(i);
            let cells =
                Layout::horizontal(vec![Constraint::Fill(1); grid.columns]).split(row_areas[row]);
            self.draw_monitor(frame, cells[column].inner(margin), monitor, dense);
        }
    }

    fn draw_monitor(&mut self, frame: &mut Frame, area: Rect, m: &MonitorView, dense: bool) {
        self.hits.push(area, Hit::Select(m.bus));
        let [panel, caption] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

        let border = if m.selected { ACCENT } else { FRAME };
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(border);
        let inner = block.inner(panel);
        frame.render_widget(block, panel);

        let half = inner.width.saturating_sub(1) / 2;

        let minus = Rect {
            width: half,
            ..inner
        };

        let divider = Rect {
            x: inner.x + half,
            width: 1.min(inner.width),
            ..inner
        };

        let plus = Rect {
            x: divider.x + divider.width,
            width: inner.width.saturating_sub(half + 1),
            ..inner
        };

        let gap = if dense { 1 } else { 2 };
        let line = divider.inner(Margin::new(0, gap));
        frame.render_widget(
            Paragraph::new(vec![Line::raw("│"); line.height as usize]).fg(FRAME),
            line,
        );
        self.draw_half(
            frame,
            minus,
            Hit::Step {
                bus: m.bus,
                steps: -1,
            },
            &Glyph::MINUS,
        );
        self.draw_half(
            frame,
            plus,
            Hit::Step {
                bus: m.bus,
                steps: 1,
            },
            &Glyph::PLUS,
        );

        let text = Text::from(vec![
            Line::from(vec![
                Span::raw(m.display.model_name()).bold(),
                Span::raw("  "),
                Span::raw(m.level_text()).bold().fg(ACCENT),
            ]),
            Line::raw(m.display.summary()).add_modifier(Modifier::DIM),
        ]);
        frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), caption);
    }

    fn draw_half(&mut self, frame: &mut Frame, area: Rect, hit: Hit, glyphs: &[Glyph]) {
        let hovered = self.hover == Some(hit);

        if hovered {
            frame.render_widget(Block::new().bg(HOVER), area);
        }

        let glyph = Glyph::fit(glyphs, area).rows;
        let height = (glyph.len() as u16).min(area.height);

        let at = Rect {
            y: area.y + (area.height - height) / 2,
            height,
            ..area
        };

        let lines: Vec<Line> = glyph.iter().map(|l| Line::raw(*l)).collect();

        let style = if hovered {
            Style::new().fg(ACCENT)
        } else {
            Style::new()
        };

        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .style(style),
            at,
        );
        self.hits.push(area, hit);
    }
}
