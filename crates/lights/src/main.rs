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

//! The `lights` command: brightness from the command line, plus the TUI
//! (`lights app`) and the desktop UI (`lights gui`).

use std::process::ExitCode;
use std::sync::LazyLock;
use std::thread;

use clap::{Parser, Subcommand, ValueEnum};
use lights_core::brightness::{self, DEFAULT_STEP, DdcError};
use lights_core::monitors::{self, Display};
use lights_core::{VERSION, legal};

#[derive(Parser)]
#[command(
    name = "lights",
    about = "Brightness controller for monitors via ddcutil",
    version = VERSION,
    long_version = LONG_VERSION.as_str(),
    after_help = legal::notice("type `lights show w'", "type `lights show c'"),
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Raw ddcutil monitor detection
    Detect,
    /// List detected displays
    Info,
    /// Show current brightness
    Get {
        #[arg(default_value = "all")]
        target: String,
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
        target: String,
        #[arg(default_value_t = DEFAULT_STEP)]
        step: i32,
    },
    /// Decrease brightness
    Down {
        #[arg(default_value = "all")]
        target: String,
        #[arg(default_value_t = DEFAULT_STEP)]
        step: i32,
    },
    /// Launch the interactive TUI
    App,
    /// Launch the desktop (GTK 4) UI
    Gui,
    /// Show the warranty (w), licence conditions (c) or third-party notices (n)
    Show { what: Shown },
}

#[derive(Clone, Copy, ValueEnum)]
enum Shown {
    W,
    C,
    N,
}

static LONG_VERSION: LazyLock<String> =
    LazyLock::new(|| format!("{VERSION}\n{}", legal::version_text()));

/// The addressable displays matching the requested target.
fn resolve_target(displays: Vec<Display>, target: &str) -> Result<Vec<Display>, String> {
    let usable: Vec<Display> = displays.into_iter().filter(Display::addressable).collect();
    if target == "all" || target == "*" {
        return Ok(usable);
    }
    // Numeric index (checked before the model keyword, which may contain digits).
    if let Ok(index) = target.parse::<i64>() {
        let hit: Vec<Display> = usable
            .into_iter()
            .filter(|d| d.index.map(i64::from) == Some(index))
            .collect();
        return if hit.is_empty() {
            Err(format!("no display with index {index}"))
        } else {
            Ok(hit)
        };
    }
    // Model or manufacturer keyword.
    let low = target.to_lowercase();
    let matches: Vec<Display> = usable
        .into_iter()
        .filter(|d| d.model.to_lowercase().contains(&low) || d.mfg.to_lowercase().contains(&low))
        .collect();
    if matches.is_empty() {
        Err(format!("unknown target '{target}'"))
    } else {
        Ok(matches)
    }
}

fn bus(d: &Display) -> u32 {
    d.bus()
        .expect("resolve_target keeps addressable displays only")
}

/// Run `f(bus)` for every target, in parallel (one bus each).
fn run_all(
    targets: &[Display],
    f: impl Fn(u32) -> Result<i32, DdcError> + Sync,
) -> Vec<Result<i32, DdcError>> {
    let f = &f;
    thread::scope(|scope| {
        let handles: Vec<_> = targets
            .iter()
            .map(|d| scope.spawn(move || f(bus(d))))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(DdcError("worker panicked".into())))
            })
            .collect()
    })
}

fn detect() -> Result<Vec<Display>, String> {
    monitors::detect().map_err(|e| e.to_string())
}

fn cmd_detect() -> Result<bool, String> {
    println!("{}", monitors::detect_raw().map_err(|e| e.to_string())?);
    Ok(true)
}

fn cmd_info() -> Result<bool, String> {
    for d in detect()? {
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

fn cmd_get(target: &str, json: bool) -> Result<bool, String> {
    let targets = resolve_target(detect()?, target)?;
    let results = run_all(&targets, brightness::get_value);
    let mut ok = true;
    for (d, r) in targets.iter().zip(&results) {
        if let Err(e) = r {
            eprintln!("error reading display {}: {e}", d.label());
            ok = false;
        }
    }
    if json {
        let out: Vec<serde_json::Value> = targets
            .iter()
            .zip(&results)
            .filter_map(|(d, r)| {
                let value = r.as_ref().ok()?;
                Some(serde_json::json!({"display": d.index, "model": d.model, "brightness": value}))
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
        );
        return Ok(ok);
    }
    for (d, r) in targets.iter().zip(&results) {
        if let Ok(value) = r {
            println!("display {} ({}): {value}%", d.label(), d.model);
        }
    }
    Ok(ok)
}

fn apply(target: &str, f: impl Fn(u32) -> Result<i32, DdcError> + Sync) -> Result<bool, String> {
    let targets = resolve_target(detect()?, target)?;
    let mut ok = true;
    for (d, r) in targets.iter().zip(run_all(&targets, f)) {
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

fn cmd_set(args: &[String]) -> Result<bool, String> {
    let (target, percent) = match args {
        [percent] => ("all", percent),
        [target, percent] => (target.as_str(), percent),
        _ => unreachable!("clap takes one or two values"),
    };
    let percent: i32 = percent
        .parse()
        .map_err(|_| format!("invalid percent '{percent}' (expected 0-100)"))?;
    apply(target, |bus| brightness::set_value(bus, percent))
}

fn cmd_show(what: Shown) -> Result<bool, String> {
    let text = match what {
        Shown::W => legal::WARRANTY,
        Shown::C => legal::LICENSE,
        Shown::N => legal::NOTICES,
    };
    print!("{text}");
    Ok(true)
}

#[cfg(feature = "tui")]
fn cmd_app() -> Result<bool, String> {
    lights_tui::run().map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(not(feature = "tui"))]
fn cmd_app() -> Result<bool, String> {
    Err("this build of lights has no TUI (built without the `tui` feature)".into())
}

#[cfg(feature = "gui")]
fn cmd_gui() -> Result<bool, String> {
    Ok(lights_gui::run() == 0)
}

#[cfg(not(feature = "gui"))]
fn cmd_gui() -> Result<bool, String> {
    Err("this build of lights has no desktop UI (built without the `gui` feature)".into())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Detect => cmd_detect(),
        Command::Info => cmd_info(),
        Command::Get { target, json } => cmd_get(target, *json),
        Command::Set { args } => cmd_set(args),
        Command::Up { target, step } => apply(target, |bus| brightness::bump_value(bus, *step)),
        Command::Down { target, step } => apply(target, |bus| brightness::bump_value(bus, -step)),
        Command::App => cmd_app(),
        Command::Gui => cmd_gui(),
        Command::Show { what } => cmd_show(*what),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
