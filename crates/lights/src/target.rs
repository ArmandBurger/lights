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

//! Which displays a CLI command applies to.

use std::convert::Infallible;
use std::str::FromStr;
use std::thread;

use lights_core::ddcutil::DdcError;
use lights_core::monitors::{self, Display};

/// A display and what running a command on it returned.
pub type Outcome = (Display, Result<i32, DdcError>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `all` or `*`.
    All,
    /// The `Display N` number from `ddcutil detect`.
    Index(u32),
    /// A case-insensitive substring of the model or manufacturer.
    Keyword(String),
}

impl FromStr for Target {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "all" | "*" => Self::All,
            // An index wins over a model keyword that contains digits.
            _ => s
                .parse()
                .map_or_else(|_| Self::Keyword(s.to_owned()), Self::Index),
        })
    }
}

impl Target {
    /// The addressable displays among `displays` that match.
    fn select(&self, displays: Vec<Display>) -> Result<Vec<Display>, String> {
        let keyword = match self {
            Self::Keyword(word) => word.to_lowercase(),
            _ => String::new(),
        };

        let matches: Vec<Display> = displays
            .into_iter()
            .filter(Display::addressable)
            .filter(|d| match self {
                Self::All => true,
                Self::Index(index) => d.index == Some(*index),
                Self::Keyword(_) => {
                    d.model.to_lowercase().contains(&keyword)
                        || d.mfg.to_lowercase().contains(&keyword)
                }
            })
            .collect();
        match self {
            Self::Index(index) if matches.is_empty() => {
                Err(format!("no display with index {index}"))
            }
            Self::Keyword(word) if matches.is_empty() => Err(format!("unknown target '{word}'")),
            _ => Ok(matches),
        }
    }

    /// Detect displays and run `f(bus)` on each match, in parallel.
    pub fn run(
        &self,
        f: impl Fn(u32) -> Result<i32, DdcError> + Sync,
    ) -> Result<Vec<Outcome>, String> {
        let targets = self.select(monitors::detect().map_err(|e| e.to_string())?)?;
        let f = &f;

        Ok(thread::scope(|scope| {
            let handles: Vec<_> = targets
                .into_iter()
                .map(|d| {
                    let bus = d.bus().expect("only addressable displays are selected");
                    (d, scope.spawn(move || f(bus)))
                })
                .collect();
            handles
                .into_iter()
                .map(|(d, h)| {
                    let result = h
                        .join()
                        .unwrap_or_else(|_| Err(DdcError("worker panicked".into())));
                    (d, result)
                })
                .collect()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(index: u32, mfg: &str, model: &str) -> Display {
        Display {
            index: Some(index),
            i2c_bus: format!("/dev/i2c-{}", index + 7),
            model: model.to_owned(),
            mfg: mfg.to_owned(),
            ..Display::default()
        }
    }

    fn select(target: &str) -> Result<Vec<u32>, String> {
        let displays = vec![
            display(1, "DEL - Dell Inc.", "SE2219H"),
            display(2, "MSI - Microstep", "MAG 276CXF"),
        ];
        let target: Target = target.parse().unwrap();
        Ok(target
            .select(displays)?
            .iter()
            .filter_map(|d| d.index)
            .collect())
    }

    #[test]
    fn selects_by_index_keyword_or_all() {
        assert_eq!(select("all"), Ok(vec![1, 2]));
        assert_eq!(select("2"), Ok(vec![2]));
        assert_eq!(select("mag"), Ok(vec![2]));
        assert_eq!(select("Dell"), Ok(vec![1]));
        assert_eq!(select("3"), Err("no display with index 3".to_owned()));
        assert_eq!(select("LG"), Err("unknown target 'LG'".to_owned()));
    }
}
