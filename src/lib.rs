//! cps — the **Cudane Python Subsystem**.
//!
//! One shared, pyo3-backed engine for **plugins**, **themes** and **TUIs** that every
//! Cudane component embeds — Cesar, Context, Outsider, MCX and Leon — so a single
//! Python skill (a theme file, a plugin hook, a full-screen TUI) works everywhere with
//! the exact same contract.
//!
//! # Concepts
//!
//! - [`PythonConfig`] — the shared configuration contract:
//!   `enabled`, `theme`, `tui`, `plugins`, `venv_path`, `tui_mode`, `fallback_on_error`.
//! - [`PythonEngine`] — boots a freethreaded interpreter exactly once, activates the
//!   configured venv, then loads the configured theme, TUI and plugins.
//! - [`theme::ThemeEngine`] — imports a Python module and calls `render_prompt`,
//!   `render_right_prompt`, `render_command_summary` and (optionally) `run()`.
//! - [`plugin::PluginManager`] — imports Python modules whose top-level callables are
//!   treated as event hooks, and maintains the `p.desc` plugin registry.
//! - [`tui::TuiEngine`] — imports a Python module exposing `run()` for full-screen
//!   applications.
//!
//! # Integration
//!
//! Each component calls [`configure`] once at startup with its brand, descriptor
//! directories and message reporter, then builds an engine from its own config:
//!
//! ```no_run
//! use cps::{Options, PythonConfig, PythonEngine};
//!
//! cps::configure(Options::new("context"));
//!
//! let cfg = PythonConfig::default();
//! let engine = PythonEngine::new(&cfg);
//! # let _ = engine;
//! ```
//!
//! Descriptor files (`t.desc` for themes and TUIs, `p.desc` for plugins) are searched
//! in the `desc_dirs` given to [`Options`]. Themes, plugins and TUIs can also be
//! installed at runtime through the static registries (`ThemeEngine::register`,
//! `PluginManager::register`, `TuiEngine::register`).
//!
//! # The engine is optional everywhere
//!
//! The interpreter is only initialised when `PythonConfig::enabled` is `true`. Every
//! load path is wrapped in `catch_unwind` and degrades to a native fallback, so a
//! missing Python or a broken plugin can never take down the host component.

pub mod config;
pub mod engine;
pub mod paths;
pub mod plugin;
pub mod theme;
pub mod tui;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

pub use config::PythonConfig;
pub use engine::PythonEngine;
pub use paths::expand_tilde;
pub use plugin::{PluginEntry, PluginManager};
pub use theme::{ThemeEngine, ThemeEntry, ThemeResult};
pub use tui::{TuiEngine, TuiEntry};

/// Emits the subsystem's status messages through the host component's own UI.
///
/// Cesar logs with `[Done]`/`[Warning]`/`[Error]` prefixes, Context with `ctx:`,
/// and MCX/Outsider route through their `UserInterface`. Implementing `Reporter`
/// lets each component keep its look while sharing 100% of the engine logic.
pub trait Reporter: Send + Sync {
    /// Informational message (theme/plugin/TUI successfully loaded).
    fn info(&self, msg: &str);
    /// Recoverable problem (missing file, unavailable engine).
    fn warning(&self, msg: &str);
    /// Hard failure (Python exception while loading).
    fn error(&self, msg: &str);
}

/// Default reporter — writes `[Done] ::`, `[Warning] ::` and `[Error] ::` lines to
/// stderr, matching the Cesar convention.
#[derive(Debug, Default)]
pub struct StderrReporter;

impl Reporter for StderrReporter {
    fn info(&self, msg: &str) {
        eprintln!("[Done] :: {msg}");
    }
    fn warning(&self, msg: &str) {
        eprintln!("[Warning] :: {msg}");
    }
    fn error(&self, msg: &str) {
        eprintln!("[Error] :: {msg}");
    }
}

/// Per-process options for the subsystem.
///
/// [`Options::new`] seeds a sensible default set of descriptor directories for a
/// brand (`~/.config/<brand>`, `/etc/<brand>`, `.`) and the [`StderrReporter`].
#[derive(Clone)]
pub struct Options {
    /// Component name used to build the default descriptor/config paths.
    pub brand: String,
    /// Directories searched for `t.desc` and `p.desc` descriptor files.
    pub desc_dirs: Vec<PathBuf>,
    /// Where status messages are delivered.
    pub reporter: Arc<dyn Reporter>,
}

impl Options {
    /// Create defaults for a brand: `~/.config/<brand>`, `/etc/<brand>` and `.`.
    pub fn new(brand: impl Into<String>) -> Self {
        let brand = brand.into();
        let mut desc_dirs = Vec::new();
        if let Ok(home) = std::env::var("HOME") {
            desc_dirs.push(PathBuf::from(home).join(".config").join(&brand));
        }
        desc_dirs.push(PathBuf::from("/etc").join(&brand));
        desc_dirs.push(PathBuf::from("."));
        Self {
            brand,
            desc_dirs,
            reporter: Arc::new(StderrReporter),
        }
    }

    /// Override the descriptor search directories.
    pub fn with_desc_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.desc_dirs = dirs;
        self
    }

    /// Override the message reporter.
    pub fn with_reporter(mut self, reporter: Arc<dyn Reporter>) -> Self {
        self.reporter = reporter;
        self
    }
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

/// Install the subsystem options. Call this once, early in `main`, before building
/// any [`PythonEngine`]. The first call wins; later calls are ignored.
pub fn configure(opts: Options) {
    let _ = OPTIONS.set(opts);
}

/// The active options — defaults to a `"cudane"` brand when [`configure`] was never
/// called, so the crate is usable standalone (e.g. by `cps`' own CLI).
pub(crate) fn options() -> &'static Options {
    OPTIONS.get_or_init(|| Options::new("cudane"))
}

/// Informational message through the configured reporter.
pub(crate) fn info(msg: &str) {
    options().reporter.info(msg);
}

/// Warning message through the configured reporter.
pub(crate) fn warning(msg: &str) {
    options().reporter.warning(msg);
}

/// Error message through the configured reporter.
pub(crate) fn error(msg: &str) {
    options().reporter.error(msg);
}
