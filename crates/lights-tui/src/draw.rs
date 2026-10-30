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

use lights_core::controller::{MonitorView, grid_shape};
use lights_core::keys::HELP_LINE;
use lights_core::legal;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::{App, Hit};

const ACCENT: Color = Color::Rgb(0xff, 0xa6, 0x2b);
const PRIMARY: Color = Color::Rgb(0x01, 0x78, 0xd4);
const FRAME: Color = Color::Rgb(0x90, 0x90, 0x90);
const PANEL: Color = Color::Rgb(0x30, 0x30, 0x30);
const HOVER: Color = Color::Rgb(0x2a, 0x2a, 0x2a);

// Terminal cells are about twice as tall as wide, so glyphs are drawn wide.
// Each half uses the largest glyph that fits: (min width, min height, glyph).
const PLUS: [(u16, u16, &[&str]); 3] = [
    (
        12,
        5,
        &[
            "    ██    ",
            "    ██    ",
            "██████████",
            "    ██    ",
            "    ██    ",
        ],
    ),
    (10, 3, &["   ██   ", "████████", "   ██   "]),
    (0, 0, &["+"]),
];
const MINUS: [(u16, u16, &[&str]); 3] = [
    (12, 5, &["██████████"]),
    (10, 3, &["████████"]),
    (0, 0, &["−"]),
];

const SHOW_W: &str = "press w";
const SHOW_C: &str = "press c";

/// `area` without `x` columns on each side and `y` rows top and bottom.
fn shrink(area: Rect, x: u16, y: u16) -> Rect {
    area.inner(Margin::new(x, y))
}

impl App {
    pub(crate) fn draw(&mut self, frame: &mut Frame) {
        self.hits.clear();
        let (_, rows) = grid_shape(self.view.monitors.len());
        let notice = if rows > 2 {
            legal::notice_short(SHOW_W, SHOW_C)
        } else {
            legal::notice(SHOW_W, SHOW_C)
        };
        let notice_height = notice.lines().count() as u16;
        let [header, toolbar, screens, notice_area, keys] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(notice_height),
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
        frame.render_widget(
            Paragraph::new(notice).add_modifier(Modifier::DIM),
            shrink(notice_area, 2, 0),
        );
        frame.render_widget(Paragraph::new(format!("  {HELP_LINE}")).bg(PANEL), keys);
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

        if let Some((message, _)) = &self.message {
            draw_toast(frame, message, screens);
        }
        if self.overlay.is_some() {
            self.draw_overlay(frame);
        }
    }

    fn draw_lock(&mut self, frame: &mut Frame, area: Rect) {
        let width = 24.min(area.width);
        let button = Rect {
            x: area.x + (area.width - width) / 2,
            width,
            ..area
        };
        let locked = self.view.locked;
        let label = if locked {
            "Locked · synced"
        } else {
            "Unlocked"
        };
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
        self.hits.push((button, Hit::Lock));
    }

    fn draw_screens(&mut self, frame: &mut Frame, area: Rect) {
        let monitors = self.view.monitors.clone();
        if monitors.is_empty() {
            let status = if self.view.detected {
                "no monitors responding  (r to retry)"
            } else {
                "detecting monitors…"
            };
            let middle = Rect {
                y: area.y + area.height / 2,
                height: 1.min(area.height),
                ..area
            };
            frame.render_widget(Paragraph::new(status).alignment(Alignment::Center), middle);
            return;
        }
        let (columns, rows) = grid_shape(monitors.len());
        let dense = rows > 1;
        let row_areas = Layout::vertical(vec![Constraint::Fill(1); rows]).split(area);
        for (i, monitor) in monitors.iter().enumerate() {
            let cells = Layout::horizontal(vec![Constraint::Fill(1); columns])
                .split(row_areas[i / columns]);
            let cell = if dense {
                shrink(cells[i % columns], 1, 0)
            } else {
                shrink(cells[i % columns], 2, 1)
            };
            self.draw_monitor(frame, cell, monitor, dense);
        }
    }

    fn draw_monitor(&mut self, frame: &mut Frame, area: Rect, m: &MonitorView, dense: bool) {
        self.hits.push((area, Hit::Select(m.bus)));
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
        let line = shrink(divider, 0, gap);
        frame.render_widget(
            Paragraph::new(vec![Line::raw("│"); line.height as usize]).fg(FRAME),
            line,
        );
        self.draw_half(
            frame,
            minus,
            Hit::Step {
                bus: m.bus,
                delta: -1,
            },
            &MINUS,
        );
        self.draw_half(
            frame,
            plus,
            Hit::Step {
                bus: m.bus,
                delta: 1,
            },
            &PLUS,
        );

        let d = &m.display;
        let value = m
            .value
            .map_or_else(|| "n/a".to_owned(), |v| format!("{v}%"));
        let busy = if m.busy { " …" } else { "" };
        let model = if d.model.is_empty() {
            "unknown"
        } else {
            &d.model
        };
        let text = Text::from(vec![
            Line::from(vec![
                Span::raw(model).bold(),
                Span::raw("  "),
                Span::raw(format!("{value}{busy}")).bold().fg(ACCENT),
            ]),
            Line::raw(format!(
                "display {} · {} · {}",
                d.label(),
                d.connector,
                d.bus_name()
            ))
            .add_modifier(Modifier::DIM),
        ]);
        frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), caption);
    }

    fn draw_half(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        hit: Hit,
        glyphs: &[(u16, u16, &[&str])],
    ) {
        let hovered = self.hover == Some(hit);
        if hovered {
            frame.render_widget(Block::new().bg(HOVER), area);
        }
        let glyph = glyphs
            .iter()
            .find(|(w, h, _)| area.width >= *w && area.height >= *h)
            .map_or(&["+"][..], |(_, _, g)| *g);
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
        self.hits.push((area, hit));
    }

    fn draw_overlay(&mut self, frame: &mut Frame) {
        let Some(overlay) = &mut self.overlay else {
            return;
        };
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
            .title(format!(" {} ", overlay.title))
            .title_bottom(Line::raw(" Esc to close ").right_aligned());
        let inner = shrink(block.inner(area), 1, 0);
        let lines = overlay.text.lines().count() as u16;
        overlay.scroll = overlay.scroll.min(lines.saturating_sub(inner.height));
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(overlay.text.as_str()).scroll((overlay.scroll, 0)),
            inner,
        );
    }
}

/// A short message in the bottom-right corner of `area`.
fn draw_toast(frame: &mut Frame, message: &str, area: Rect) {
    let width = (message.chars().count() as u16 + 4).min(area.width);
    let height = 3.min(area.height);
    let at = Rect {
        x: area.x + area.width - width,
        y: area.y + area.height - height,
        width,
        height,
    };
    frame.render_widget(Clear, at);
    frame.render_widget(
        Paragraph::new(message).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(ACCENT),
        ),
        at,
    );
}
