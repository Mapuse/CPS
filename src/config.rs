use serde::{Deserialize, Serialize};

/// Shared configuration contract for the Python subsystem.
///
/// This exact shape is used by every Cudane component, so a `[python]` section in one
/// component's config means the same thing in all of them. It is byte-for-byte
/// identical across Cesar, Context, Outsider and MCX.
///
/// ```toml
/// [python]
/// enabled = true
/// theme   = "~/.config/context/themes/context.py"
/// tui     = ""
/// plugins = []
/// fallback_on_error = true
/// venv_path = ""
/// tui_mode = false
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PythonConfig {
    /// Master switch. When `false` the interpreter is never touched and the engine
    /// degrades to an empty native fallback.
    pub enabled: bool,
    /// Path to the Python theme module (`render_prompt` / `render_right_prompt` /
    /// `render_command_summary` / optional `run`). Empty disables theming.
    pub theme: String,
    /// Path to the Python TUI module (must expose `run()`). Empty disables the TUI.
    pub tui: String,
    /// Paths to Python plugin modules whose top-level callables are event hooks.
    pub plugins: Vec<String>,
    /// If `true` (default), any Python failure falls back to the native behaviour
    /// instead of surfacing as an error.
    pub fallback_on_error: bool,
    /// Optional virtual environment. When set, its `site-packages` is prepended to
    /// `sys.path` so themes/plugins can import bundled dependencies.
    pub venv_path: String,
    /// When `true`, the engine runs in TUI mode and the theme's `run()` is invoked
    /// after startup.
    pub tui_mode: bool,
}

impl Default for PythonConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            theme: String::new(),
            tui: String::new(),
            plugins: Vec::new(),
            // `fallback_on_error` defaults to true everywhere in the ecosystem:
            // a broken theme/plugin must never take down the host component.
            fallback_on_error: true,
            venv_path: String::new(),
            tui_mode: false,
        }
    }
}
