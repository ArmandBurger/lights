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
    /// Step brightness by this many (signed) steps.
    Step(i32),
    /// Set brightness to this percent.
    Set(i32),
    Lock,
    Reload,
    Help,
    Quit,
}

impl Action {
    /// A motion `count` times.
    fn motion(key: Key, count: i32) -> Option<Self> {
        match key {
            Key::Char('h') | Key::Left => Some(Self::Select(-count)),
            Key::Char('l') | Key::Right => Some(Self::Select(count)),
            Key::Char('k' | 'K') | Key::Up => Some(Self::Step(count)),
            Key::Char('j' | 'J') | Key::Down => Some(Self::Step(-count)),
            _ => None,
        }
    }

    fn command(key: Key) -> Option<Self> {
        match key {
            Key::Char('s') => Some(Self::Lock),
            Key::Char('r') => Some(Self::Reload),
            Key::Char('?') => Some(Self::Help),
            Key::Char('q') => Some(Self::Quit),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct VimKeys {
    count: String,
    g: bool,
}

impl VimKeys {
    pub const HELP: [(&str, &str); 10] = [
        ("← → or h l", "select the previous / next monitor"),
        ("↑ ↓ or k j", "brighter / dimmer by 5%"),
        ("3 then ↑", "a number first repeats: 15% brighter"),
        ("70 then %", "set brightness to 70%"),
        ("gg / G", "full brightness / off (100% / 0%)"),
        ("s", "link: adjust all monitors together"),
        ("r", "look for monitors again"),
        ("?", "show this list"),
        ("q", "quit"),
        ("Esc", "cancel a number you started typing"),
    ];

    /// The most used keys, for a one-line legend.
    pub const HELP_LINE: &str = "←→ select · ↑↓ brightness · s link · ? more keys · q quit";

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
            _ => Action::motion(key, count.unwrap_or(1)).or_else(|| Action::command(key)),
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
        assert_eq!(feed("3k").last(), Some(&Some(Action::Step(3))));
        assert_eq!(feed("2J").last(), Some(&Some(Action::Step(-2))));
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
