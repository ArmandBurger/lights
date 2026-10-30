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

//! The user's settings in `~/.config/lights/config.toml`. The file can be
//! edited by hand; the desktop UI also edits it, keeping its comments and
//! layout.
//!
//! ```toml
//! [presets]
//! day = 80
//! night = 30
//! ```

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Table, value};

/// A named brightness that every monitor can be set to at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub level: i32,
}

pub struct Config {
    path: PathBuf,
    doc: DocumentMut,
}

impl Config {
    /// Used while there is no config file.
    const DEFAULT: &str = "[presets]\nday = 80\nevening = 50\nnight = 30\ndark = 10\n";

    /// `$XDG_CONFIG_HOME/lights/config.toml`, else under `~/.config`.
    pub fn path() -> Option<PathBuf> {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

        Some(dir.join("lights").join("config.toml"))
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::path().ok_or("cannot find the config directory: HOME is not set")?;
        Self::load_from(path)
    }

    fn load_from(path: PathBuf) -> Result<Self, String> {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => Self::DEFAULT.to_owned(),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };

        let doc = text
            .parse()
            .map_err(|e| format!("{}: {e}", path.display()))?;

        Ok(Self { path, doc })
    }

    pub fn file(&self) -> &Path {
        &self.path
    }

    /// In file order, leaving out entries that are not a whole 0-100.
    pub fn presets(&self) -> Vec<Preset> {
        let Some(table) = self.doc.get("presets").and_then(Item::as_table_like) else {
            return Vec::new();
        };

        table
            .iter()
            .filter_map(|(name, item)| {
                let level = i32::try_from(item.as_integer()?).ok()?;

                (0..=100).contains(&level).then(|| Preset {
                    name: name.to_owned(),
                    level,
                })
            })
            .collect()
    }

    /// Case-insensitive.
    pub fn preset(&self, name: &str) -> Option<Preset> {
        self.presets()
            .into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Add a preset, or change the level of one with this name.
    pub fn set_preset(&mut self, name: &str, level: i32) {
        if !self.doc.get("presets").is_some_and(Item::is_table_like) {
            self.doc.insert("presets", Item::Table(Table::new()));
        }

        let name = self
            .preset(name)
            .map_or_else(|| name.to_owned(), |p| p.name);
        self.doc["presets"][name.as_str()] = value(i64::from(level.clamp(0, 100)));
    }

    pub fn remove_preset(&mut self, name: &str) {
        if let Some(table) = self
            .doc
            .get_mut("presets")
            .and_then(Item::as_table_like_mut)
        {
            table.remove(name);
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let error = |e: std::io::Error| format!("cannot save {}: {e}", self.path.display());

        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(error)?;
        }

        fs::write(&self.path, self.doc.to_string()).map_err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(config: &Config) -> Vec<(String, i32)> {
        config
            .presets()
            .into_iter()
            .map(|p| (p.name, p.level))
            .collect()
    }

    #[test]
    fn edits_keep_the_file_as_written() {
        let dir = std::env::temp_dir().join(format!("lights-config-{}", std::process::id()));
        let path = dir.join("config.toml");

        let config = Config::load_from(path.clone()).unwrap();
        assert_eq!(config.preset("NIGHT").map(|p| p.level), Some(30));

        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            "# mine\n[presets]\nnight = 20 # cosy\nbroken = \"x\"\nloud = 150\n",
        )
        .unwrap();

        let mut config = Config::load_from(path.clone()).unwrap();
        assert_eq!(names(&config), [("night".to_owned(), 20)]);

        config.set_preset("Night", 25);
        config.set_preset("reading", 60);
        config.remove_preset("loud");
        config.save().unwrap();

        let text = fs::read_to_string(&path).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(text.starts_with("# mine\n[presets]\n"), "{text}");
        assert!(text.contains("broken = \"x\""), "{text}");
        assert!(!text.contains("loud"), "{text}");

        let config = Config {
            path,
            doc: text.parse().unwrap(),
        };
        assert_eq!(
            names(&config),
            [("night".to_owned(), 25), ("reading".to_owned(), 60)]
        );
    }
}
