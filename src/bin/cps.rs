//! cps — Cudane Python Subsystem command-line front end.
//!
//! A reference CLI that boots the shared engine against a config file and manages the
//! theme / plugin / TUI registries — the same surface every Cudane component embeds
//! (`csr`, `ctx`, `ous`, `mcx`). It is also the template for Leon's boot companion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use cps::plugin::PluginManager;
use cps::theme::ThemeEngine;
use cps::tui::TuiEngine;
use cps::{Options, PythonConfig, PythonEngine};

#[derive(Parser)]
#[command(
    name = "cps",
    version,
    about = "Cudane Python Subsystem — shared plugins, themes and TUIs"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Theme commands (list / apply / register / unregister)
    Theme(ThemeArgs),
    /// Plugin commands (list / run / register / unregister)
    Plugin(PluginArgs),
    /// TUI commands (list / apply / register / unregister)
    Tui(TuiArgs),
    /// Boot the engine configured in a cps.toml and render a sample prompt
    Engine(EngineArgs),
}

#[derive(Parser)]
struct ThemeArgs {
    #[command(subcommand)]
    action: ThemeAction,
}

#[derive(Subcommand)]
enum ThemeAction {
    List,
    Apply { name: String },
    Register { name: String, path: String },
    Unregister { name: String },
}

#[derive(Parser)]
struct PluginArgs {
    #[command(subcommand)]
    action: PluginAction,
}

#[derive(Subcommand)]
enum PluginAction {
    List,
    Run { alias: String, #[arg(trailing_var_arg = true, allow_hyphen_values = true)] args: Vec<String> },
    Register { name: String, path: String },
    Unregister { name: String },
}

#[derive(Parser)]
struct TuiArgs {
    #[command(subcommand)]
    action: TuiAction,
}

#[derive(Subcommand)]
enum TuiAction {
    List,
    Apply { name: String },
    Register { name: String, path: String },
    Unregister { name: String },
}

#[derive(Parser)]
struct EngineArgs {
    /// Path to a config file (defaults to the standard cps.toml candidates)
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    cps::configure(Options::new("cps"));

    let cli = Cli::parse();
    match cli.cmd {
        Command::Theme(args) => match args.action {
            ThemeAction::List => {
                let themes = ThemeEngine::list();
                if themes.is_empty() {
                    println!("No themes registered.");
                    println!("  Add [theme.<id>] sections to a t.desc in one of the desc dirs.");
                }
                for t in &themes {
                    println!("{}", t.name);
                    println!("  Path:    {}", t.path);
                    if !t.description.is_empty() {
                        println!("  About:   {}", t.description);
                    }
                }
            }
            ThemeAction::Apply { name } => {
                let Some(theme) = ThemeEngine::by_name(&name) else {
                    eprintln!("No theme '{}'", name);
                    std::process::exit(1);
                };
                match ThemeEngine::apply(&theme) {
                    Ok(out) => print!("{out}"),
                    Err(e) => {
                        eprintln!("Theme '{}' failed: {e}", theme.name);
                        std::process::exit(1);
                    }
                }
            }
            ThemeAction::Register { name, path } => {
                ThemeEngine::register(&name, Path::new(&path));
                println!("Registered theme '{}' → {path}", name);
            }
            ThemeAction::Unregister { name } => {
                ThemeEngine::unregister(&name);
                println!("Unregistered theme '{}'", name);
            }
        },
        Command::Plugin(args) => match args.action {
            PluginAction::List => {
                let plugins = PluginManager::list();
                if plugins.is_empty() {
                    println!("No plugins registered.");
                    println!("  Add [plugin.<id>] sections to a p.desc in one of the desc dirs.");
                }
                for p in &plugins {
                    println!("{}", p.name);
                    println!("  Path:    {}", p.path);
                    if !p.aliases.is_empty() {
                        println!("  Aliases:");
                        for (alias, cmd) in &p.aliases {
                            println!("    {alias} → {cmd}");
                        }
                    }
                }
            }
            PluginAction::Run { alias, args } => {
                let Some((entry, func)) = PluginManager::by_alias(&alias) else {
                    eprintln!("No plugin alias '{}'", alias);
                    std::process::exit(1);
                };
                match PluginManager::run(&entry, &func, &args) {
                    Ok(out) => print!("{out}"),
                    Err(e) => {
                        eprintln!("Plugin '{}' failed: {e}", entry.name);
                        std::process::exit(1);
                    }
                }
            }
            PluginAction::Register { name, path } => {
                PluginManager::register(&name, Path::new(&path), &HashMap::new());
                println!("Registered plugin '{}' → {path}", name);
            }
            PluginAction::Unregister { name } => {
                PluginManager::unregister(&name);
                println!("Unregistered plugin '{}'", name);
            }
        },
        Command::Tui(args) => match args.action {
            TuiAction::List => {
                let tuis = TuiEngine::list();
                if tuis.is_empty() {
                    println!("No TUIs registered.");
                    println!("  Add [tui.<id>] sections to a t.desc in one of the desc dirs.");
                }
                for t in &tuis {
                    println!("{}", t.name);
                    println!("  Path:    {}", t.path);
                    if !t.description.is_empty() {
                        println!("  About:   {}", t.description);
                    }
                }
            }
            TuiAction::Apply { name } => {
                let Some(tui) = TuiEngine::by_name(&name) else {
                    eprintln!("No TUI '{}'", name);
                    std::process::exit(1);
                };
                match TuiEngine::apply(&tui) {
                    Ok(out) => print!("{out}"),
                    Err(e) => {
                        eprintln!("TUI '{}' failed: {e}", tui.name);
                        std::process::exit(1);
                    }
                }
            }
            TuiAction::Register { name, path } => {
                TuiEngine::register(&name, Path::new(&path));
                println!("Registered TUI '{}' → {path}", name);
            }
            TuiAction::Unregister { name } => {
                TuiEngine::unregister(&name);
                println!("Unregistered TUI '{}'", name);
            }
        },
        Command::Engine(args) => {
            let cfg = load_config(args.config);
            let engine = PythonEngine::new(&cfg);
            match &engine.theme {
                Some(theme) => {
                    let mut ctx = HashMap::new();
                    ctx.insert("cwd".into(), "/home/cudane".into());
                    ctx.insert("user".into(), "cudane".into());
                    ctx.insert("host".into(), "cudane".into());
                    ctx.insert("exit_code".into(), "0".into());
                    let res = theme.render_prompt(&ctx);
                    for line in &res.lines_above {
                        println!("{line}");
                    }
                    print!("{}", res.input_prefix);
                    println!("{}", res.right_prompt);
                }
                None => println!("engine disabled: set [python].enabled = true in a cps.toml"),
            }
        }
    }
}

/// Standard config file candidates, in priority order.
fn config_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        v.push(PathBuf::from(home).join(".config/cps/config.toml"));
    }
    v.push(PathBuf::from("/etc/cps/config.toml"));
    v.push(PathBuf::from("./cps.toml"));
    v
}

/// Load a `[python]` section from the first existing, parseable config file.
fn load_config(explicit: Option<PathBuf>) -> PythonConfig {
    #[derive(serde::Deserialize)]
    struct ConfigFile {
        #[serde(default)]
        python: PythonConfig,
    }
    let mut candidates = Vec::new();
    if let Some(path) = explicit {
        candidates.push(path);
    }
    candidates.extend(config_candidates());
    for path in candidates {
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(cfg) = toml::from_str::<ConfigFile>(&content)
        {
            return cfg.python;
        }
    }
    PythonConfig::default()
}
