#[cfg(feature = "python")]
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::PythonConfig;
use crate::plugin::PluginManager;
use crate::theme::ThemeEngine;
use crate::tui::TuiEngine;
#[cfg(feature = "python")]
use crate::paths::activate_venv;
#[cfg(feature = "python")]
use crate::warning;

/// Initialises the freethreaded interpreter at most once per process.
#[cfg(feature = "python")]
static INIT: Once = Once::new();
/// Set when interpreter initialisation panicked, so later callers degrade fast.
#[cfg(feature = "python")]
static INIT_FAILED: AtomicBool = AtomicBool::new(false);
/// Set once the interpreter has actually been booted in this process.
#[cfg(feature = "python")]
static BOOTED: AtomicBool = AtomicBool::new(false);
/// Set by [`PythonEngine::shutdown`] so later engines degrade instead of rebooting.
static SHUT_DOWN: AtomicBool = AtomicBool::new(false);

/// The booted Python subsystem: a theme, a TUI and a plugin manager.
///
/// Built from a [`PythonConfig`] by [`PythonEngine::new`]. The interpreter boots
/// lazily: only when `enabled` is `true` **and** at least one of theme, TUI or
/// plugins is configured. When `enabled` is `false`, nothing is configured, Python
/// is unavailable, or the engine was shut down, every field degrades to its native
/// fallback — the host component must handle `theme: None` / `tui: None` / an empty
/// plugin manager.
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
    /// Whether [`PythonEngine::new`] would boot anything at all for this config.
    ///
    /// Cheap and side-effect free: no interpreter, venv or module work happens here.
    /// Hosts can use it to skip plugin plumbing entirely on a native-only setup.
    pub fn needs_boot(cfg: &PythonConfig) -> bool {
        cfg.enabled
            && !SHUT_DOWN.load(Ordering::SeqCst)
            && (!cfg.theme.is_empty() || !cfg.tui.is_empty() || !cfg.plugins.is_empty() || cfg.tui_mode)
    }

    /// Boot the engine from a config. Safe to call multiple times; the interpreter
    /// is only initialised once, and only when there is something to load.
    ///
    /// # Panics
    /// No Python work here can panic the host: the interpreter init and every module
    /// load is wrapped in `catch_unwind`.
    pub fn new(cfg: &PythonConfig) -> Self {
        if !Self::needs_boot(cfg) {
            return Self::disabled();
        }
        #[cfg(feature = "python")]
        {
            INIT.call_once(|| {
                let ok = std::panic::catch_unwind(|| {
                    pyo3::prepare_freethreaded_python();
                })
                .is_ok();
                if ok {
                    BOOTED.store(true, Ordering::SeqCst);
                } else {
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
        #[cfg(not(feature = "python"))]
        {
            let _ = cfg;
            Self::disabled()
        }
    }

    /// Release every Python object held by this engine and finalise the interpreter
    /// if it was booted in this process.
    ///
    /// Idempotent: extra calls are no-ops, and any [`PythonEngine`] built afterwards
    /// degrades to the disabled fallback instead of rebooting the interpreter. Hosts
    /// should call this just before process exit ("auto uninit").
    pub fn shutdown(&mut self) {
        self.theme = None;
        self.tui = None;
        self.plugins = PluginManager::new();
        #[cfg(feature = "python")]
        if BOOTED.swap(false, Ordering::SeqCst) {
            unsafe {
                let _ = pyo3::ffi::Py_FinalizeEx();
            }
        }
        SHUT_DOWN.store(true, Ordering::SeqCst);
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
