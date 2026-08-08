use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use crate::config::PythonConfig;
use crate::paths::activate_venv;
use crate::plugin::PluginManager;
use crate::theme::ThemeEngine;
use crate::tui::TuiEngine;
use crate::warning;

/// Initialises the freethreaded interpreter at most once per process.
static INIT: Once = Once::new();
/// Set when interpreter initialisation panicked, so later callers degrade fast.
static INIT_FAILED: AtomicBool = AtomicBool::new(false);

/// The booted Python subsystem: a theme, a TUI and a plugin manager.
///
/// Built from a [`PythonConfig`] by [`PythonEngine::new`]. When `enabled` is `false`,
/// or Python is unavailable, every field degrades to its native fallback — the host
/// component must handle `theme: None` / `tui: None` / an empty plugin manager.
pub struct PythonEngine {
    /// Loaded theme module, if configured and importable.
    pub theme: Option<ThemeEngine>,
    /// Loaded TUI module, if configured and importable.
    pub tui: Option<TuiEngine>,
    /// Plugin manager, populated from `cfg.plugins`.
    pub plugins: PluginManager,
    /// Mirrors `cfg.tui_mode`.
    pub tui_mode: bool,
}

impl PythonEngine {
    /// Boot the engine from a config. Safe to call multiple times; the interpreter
    /// itself is only initialised once.
    ///
    /// # Panics
    /// No Python work here can panic the host: the interpreter init and every module
    /// load is wrapped in `catch_unwind`.
    pub fn new(cfg: &PythonConfig) -> Self {
        if !cfg.enabled {
            return Self::disabled();
        }
        INIT.call_once(|| {
            let ok = std::panic::catch_unwind(|| {
                pyo3::prepare_freethreaded_python();
            })
            .is_ok();
            if !ok {
                INIT_FAILED.store(true, Ordering::SeqCst);
            }
        });
        if INIT_FAILED.load(Ordering::SeqCst) {
            warning("python engine unavailable, falling back to native");
            return Self::disabled();
        }
        if !cfg.venv_path.is_empty() {
            activate_venv(&cfg.venv_path);
        }
        let theme = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ThemeEngine::load(cfg)
        }))
        .unwrap_or_else(|e| {
            warning(&format!("python theme failed to load: {e:?}"));
            None
        });
        let tui = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| TuiEngine::load(cfg)))
            .unwrap_or_else(|e| {
                warning(&format!("python tui failed to load: {e:?}"));
                None
            });
        let mut plugins = PluginManager::new();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            plugins.load_all(cfg);
        }));
        Self {
            theme,
            tui,
            plugins,
            tui_mode: cfg.tui_mode,
        }
    }

    fn disabled() -> Self {
        Self {
            theme: None,
            tui: None,
            plugins: PluginManager::new(),
            tui_mode: false,
        }
    }
}
