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

//! Vim-style keys shared by the TUI and the desktop UI.
//!
//! Both UIs translate their key events into a `Key` (a printable character
//! such as `'k'`, `'K'`, `'5'`, `'%'`, or an arrow / Escape) and feed it to
//! `VimKeys`, which returns what to do. A count prefix repeats a motion, as
//! in vim: `3k` is three steps up.

/// k / j, and a mouse click.
pub const STEP: i32 = 5;
/// K / J.
pub const STEP_BIG: i32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Left,
    Right,
    Up,
    Down,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Key consumed, waiting for more (a count or `g`).
    Pending,
    /// Move the selection by this many monitors.
    Select(i32),
    /// Step brightness by `steps` (signed) steps of `size`.
    Step {
        steps: i32,
        size: i32,
    },
    /// Set brightness to this percent.
    Set(i32),
    Lock,
    Reload,
    Help,
    Quit,
}

pub const HELP: [(&str, &str); 11] = [
    ("h / l  (← / →)", "select previous / next monitor"),
    ("k / j  (↑ / ↓)", "brighter / dimmer by 5%"),
    ("K / J", "brighter / dimmer by 10%"),
    ("<count>", "repeat, e.g. 3k"),
    ("<n>%", "set brightness to n%, e.g. 70%"),
    ("gg / G", "100% / 0%"),
    ("s", "link: adjust all monitors together"),
    ("r", "re-detect monitors"),
    ("?", "show keys"),
    ("q", "quit"),
    ("Esc", "cancel a pending count"),
];

pub const HELP_LINE: &str = "h/l select · k/j ±5 · K/J ±10 · 3k repeat · 70% set · \
                             gg/G max/min · s link · r refresh · ? keys · q quit";

fn motion(key: Key) -> Option<Action> {
    let step = |steps, size| Some(Action::Step { steps, size });
    match key {
        Key::Char('h') | Key::Left => Some(Action::Select(-1)),
        Key::Char('l') | Key::Right => Some(Action::Select(1)),
        Key::Char('k') | Key::Up => step(1, STEP),
        Key::Char('j') | Key::Down => step(-1, STEP),
        Key::Char('K') => step(1, STEP_BIG),
        Key::Char('J') => step(-1, STEP_BIG),
        _ => None,
    }
}

fn command(key: Key) -> Option<Action> {
    match key {
        Key::Char('s') => Some(Action::Lock),
        Key::Char('r') => Some(Action::Reload),
        Key::Char('?') => Some(Action::Help),
        Key::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct VimKeys {
    count: String,
    g: bool,
}

impl VimKeys {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keys typed so far for an unfinished command (vim's showcmd).
    pub fn pending(&self) -> String {
        format!("{}{}", self.count, if self.g { "g" } else { "" })
    }

    pub fn reset(&mut self) {
        self.count.clear();
        self.g = false;
    }

    /// Handle one key; `None` means the key is not ours.
    pub fn feed(&mut self, key: Key) -> Option<Action> {
        let count: Option<i32> = self.count.parse().ok();
        if let Key::Char(digit @ '0'..='9') = key
            && !self.g
        {
            if digit == '0' && self.count.is_empty() {
                self.reset();
                return None;
            }
            if self.count.len() < 3 {
                self.count.push(digit);
            }
            return Some(Action::Pending);
        }
        if key == Key::Char('g') {
            if self.g {
                self.reset();
                return Some(Action::Set(count.map_or(100, |n| n.min(100))));
            }
            self.g = true;
            return Some(Action::Pending);
        }
        let had_pending = !self.pending().is_empty();
        self.reset();
        match key {
            Key::Escape => had_pending.then_some(Action::Pending),
            Key::Char('G') => Some(Action::Set(count.map_or(0, |n| n.min(100)))),
            Key::Char('%') => count.map(|n| Action::Set(n.min(100))),
            _ => match motion(key) {
                Some(Action::Select(n)) => Some(Action::Select(n * count.unwrap_or(1))),
                Some(Action::Step { steps, size }) => Some(Action::Step {
                    steps: steps * count.unwrap_or(1),
                    size,
                }),
                _ => command(key),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(keys: &str) -> Vec<Option<Action>> {
        let mut vim = VimKeys::new();
        keys.chars().map(|c| vim.feed(Key::Char(c))).collect()
    }

    #[test]
    fn counts_repeat_motions() {
        assert_eq!(
            feed("3k").last(),
            Some(&Some(Action::Step {
                steps: 3,
                size: STEP
            }))
        );
        assert_eq!(
            feed("2J").last(),
            Some(&Some(Action::Step {
                steps: -2,
                size: STEP_BIG
            }))
        );
        assert_eq!(feed("2h").last(), Some(&Some(Action::Select(-2))));
    }

    #[test]
    fn sets_levels() {
        assert_eq!(feed("70%").last(), Some(&Some(Action::Set(70))));
        assert_eq!(feed("gg").last(), Some(&Some(Action::Set(100))));
        assert_eq!(feed("40gg").last(), Some(&Some(Action::Set(40))));
        assert_eq!(feed("G").last(), Some(&Some(Action::Set(0))));
        assert_eq!(feed("999%").last(), Some(&Some(Action::Set(100))));
        assert_eq!(feed("%"), vec![None]);
    }

    #[test]
    fn pending_and_escape() {
        let mut vim = VimKeys::new();
        assert_eq!(vim.feed(Key::Char('1')), Some(Action::Pending));
        assert_eq!(vim.feed(Key::Char('2')), Some(Action::Pending));
        assert_eq!(vim.pending(), "12");
        assert_eq!(vim.feed(Key::Escape), Some(Action::Pending));
        assert_eq!(vim.pending(), "");
        assert_eq!(vim.feed(Key::Escape), None);
        assert_eq!(vim.feed(Key::Char('0')), None);
        assert_eq!(vim.feed(Key::Char('x')), None);
        assert_eq!(vim.feed(Key::Char('q')), Some(Action::Quit));
    }
}
