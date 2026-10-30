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

//! The command line: what it accepts and what each command does.

use std::sync::LazyLock;

use clap::{Parser, Subcommand, ValueEnum};
use lights_core::config::Config;
use lights_core::ddcutil::DdcError;
use lights_core::monitors;
use lights_core::{VERSION, brightness, legal};

use crate::target::Target;

const DEFAULT_STEP: i32 = 10;

static LONG_VERSION: LazyLock<String> =
    LazyLock::new(|| format!("{VERSION}\n{}", legal::version_text()));

#[derive(Parser)]
#[command(
    name = "lights",
    about = "Brightness controller for monitors via ddcutil",
    version = VERSION,
    long_version = LONG_VERSION.as_str(),
    after_help = legal::notice("type `lights show w'", "type `lights show c'"),
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Raw ddcutil monitor detection
    Detect,
    /// List detected displays
    Info,
    /// Show current brightness
    Get {
        #[arg(default_value = "all")]
        target: Target,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Set brightness percent (0-100): `set [target] <percent>`
    Set {
        #[arg(num_args = 1..=2, value_names = ["TARGET", "PERCENT"], required = true)]
        args: Vec<String>,
    },
    /// Increase brightness
    Up {
        #[arg(default_value = "all")]
        target: Target,
        #[arg(default_value_t = DEFAULT_STEP)]
        step: i32,
    },
    /// Decrease brightness
    Down {
        #[arg(default_value = "all")]
        target: Target,
        #[arg(default_value_t = DEFAULT_STEP)]
        step: i32,
    },
    /// List the presets, or set every display to one: `preset [name]`
    Preset {
        name: Option<String>,
        /// Print the list as JSON
        #[arg(long)]
        json: bool,
    },
    /// Launch the interactive TUI (the default)
    Tui,
    /// Launch the desktop (GTK 4) UI
    Gui,
    /// Show the warranty (w), licence conditions (c) or third-party notices (n)
    Show { what: Shown },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Shown {
    W,
    C,
    N,
}

impl Command {
    /// Ok(false): the command ran but failed on some display.
    pub fn run(&self) -> Result<bool, String> {
        match self {
            Self::Detect => Self::detect(),
            Self::Info => Self::info(),
            Self::Get { target, json } => Self::get(target, *json),
            Self::Set { args } => Self::set(args),
            Self::Up { target, step } => {
                Self::apply(target, |bus| brightness::bump_value(bus, *step))
            }
            Self::Down { target, step } => {
                Self::apply(target, |bus| brightness::bump_value(bus, -step))
            }
            Self::Preset { name, json } => Self::preset(name.as_deref(), *json),
            Self::Tui => Self::tui(),
            Self::Gui => Self::gui(),
            Self::Show { what } => Self::show(*what),
        }
    }

    fn detect() -> Result<bool, String> {
        println!("{}", monitors::detect_raw().map_err(|e| e.to_string())?);
        Ok(true)
    }

    fn info() -> Result<bool, String> {
        for d in monitors::detect().map_err(|e| e.to_string())? {
            let status = if d.addressable() { "ok" } else { "DDC down" };
            let name = [d.model.as_str(), d.i2c_bus.as_str()]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or("unknown");
            println!(
                "  [{}] {name} ({}) i2c:{} [{status}]",
                d.label(),
                d.connector,
                d.i2c_bus
            );
        }

        Ok(true)
    }

    fn get(target: &Target, json: bool) -> Result<bool, String> {
        let results = target.run(brightness::get_value)?;
        let mut ok = true;

        for (d, r) in &results {
            if let Err(e) = r {
                eprintln!("error reading display {}: {e}", d.label());
                ok = false;
            }
        }

        let read = results
            .iter()
            .filter_map(|(d, r)| r.as_ref().ok().map(|value| (d, value)));
        if json {
            let out: Vec<serde_json::Value> = read
                .map(|(d, value)| {
                    serde_json::json!({"display": d.index, "model": d.model, "brightness": value})
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
            );
        } else {
            for (d, value) in read {
                println!("display {} ({}): {value}%", d.label(), d.model);
            }
        }

        Ok(ok)
    }

    fn set(args: &[String]) -> Result<bool, String> {
        let (target, percent) = match args {
            [percent] => (Target::All, percent),
            [target, percent] => {
                let Ok(target) = target.parse::<Target>();
                (target, percent)
            }
            _ => unreachable!("clap takes one or two values"),
        };

        let percent: i32 = percent
            .parse()
            .map_err(|_| format!("invalid percent '{percent}' (expected 0-100)"))?;
        Self::apply(&target, |bus| brightness::set_value(bus, percent))
    }

    fn preset(name: Option<&str>, json: bool) -> Result<bool, String> {
        let config = Config::load()?;

        let Some(name) = name else {
            let presets = config.presets();

            if json {
                let out: Vec<serde_json::Value> = presets
                    .iter()
                    .map(|p| serde_json::json!({"name": p.name, "brightness": p.level}))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
                );
            } else {
                for p in &presets {
                    println!("{:<12} {}%", p.name, p.level);
                }

                println!("(edit {})", config.file().display());
            }

            return Ok(true);
        };

        let preset = config.preset(name).ok_or_else(|| {
            format!(
                "unknown preset '{name}' (see `lights preset`, or add it to {})",
                config.file().display()
            )
        })?;

        Self::apply(&Target::All, |bus| brightness::set_value(bus, preset.level))
    }

    /// Run `f` on every target and print the brightness it leaves.
    fn apply(
        target: &Target,
        f: impl Fn(u32) -> Result<i32, DdcError> + Sync,
    ) -> Result<bool, String> {
        let mut ok = true;

        for (d, r) in target.run(f)? {
            match r {
                Ok(value) => println!("display {} ({}): {value}%", d.label(), d.model),
                Err(e) => {
                    eprintln!("error on display {} ({}): {e}", d.label(), d.model);
                    ok = false;
                }
            }
        }

        Ok(ok)
    }

    fn show(what: Shown) -> Result<bool, String> {
        let text = match what {
            Shown::W => legal::WARRANTY,
            Shown::C => legal::LICENSE,
            Shown::N => legal::NOTICES,
        };

        print!("{text}");
        Ok(true)
    }

    #[cfg(feature = "tui")]
    fn tui() -> Result<bool, String> {
        lights_tui::run().map_err(|e| e.to_string())?;
        Ok(true)
    }

    #[cfg(not(feature = "tui"))]
    fn tui() -> Result<bool, String> {
        Err("this build of lights has no TUI (built without the `tui` feature)".into())
    }

    #[cfg(feature = "gui")]
    fn gui() -> Result<bool, String> {
        Ok(lights_gui::run() == 0)
    }

    #[cfg(not(feature = "gui"))]
    fn gui() -> Result<bool, String> {
        Err("this build of lights has no desktop UI (built without the `gui` feature)".into())
    }
}
