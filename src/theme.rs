use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, Once, OnceLock};

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyDict;

use crate::config::PythonConfig;
#[cfg(feature = "python")]
use crate::error;
use crate::expand_tilde;
use crate::info;
use crate::paths::{desc_candidates, load_desc};
#[cfg(feature = "python")]
use crate::warning;

/// Parsed `[theme.<id>]` sections from a `t.desc` descriptor file.
#[derive(serde::Deserialize)]
struct ThemeDescConfig {
    #[serde(rename = "theme")]
    themes: HashMap<String, ThemeDescEntry>,
}

#[derive(serde::Deserialize)]
struct ThemeDescEntry {
    name: String,
    path: String,
    description: Option<String>,
}

/// A loaded Python theme module.
///
/// The module is imported once; [`ThemeEngine::render_prompt`] and friends invoke its
/// Python functions on every call, so a theme can be live-edited in Python without a
/// Rust rebuild. Built without the `python` feature this is an inert stub that always
/// falls back to the native prompt.
#[cfg(feature = "python")]
pub struct ThemeEngine {
    module: PyObject,
}

#[cfg(not(feature = "python"))]
pub struct ThemeEngine {}

/// The structured result of a `render_prompt` call.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeResult {
    /// Lines printed above the prompt (header / multi-line content).
    pub lines_above: Vec<String>,
    /// The final input prefix, e.g. `"~/code ❯ "`.
    pub input_prefix: String,
    /// Right-aligned suffix (git info, time, …).
    pub right_prompt: String,
    /// Named colors, e.g. `{"accent": "#22d3ee", "error": "#ef4444"}`.
    pub colors: HashMap<String, String>,
    /// Any extra keys returned by the theme, untouched by the engine.
    pub extra: HashMap<String, String>,
}

#[cfg(feature = "python")]
impl ThemeEngine {
    /// Import the theme module named by `cfg.theme` (a `.py` path).
    ///
    /// The module's directory is prepended to `sys.path` and the file stem is
    /// imported. Returns `None` when unconfigured, missing or unimportable.
    pub fn load(cfg: &PythonConfig) -> Option<Self> {
        if cfg.theme.is_empty() {
            return None;
        }
        let path = expand_tilde(&cfg.theme);
        let std_path = std::path::PathBuf::from(&path);
        if !std_path.exists() {
            warning(&format!("theme file not found: {path}"));
            return None;
        }
        let parent = std_path.parent()?.to_str()?;
        let file_stem = std_path.file_stem()?.to_str()?;
        let parent_str = parent.to_string();
        let file_stem = file_stem.to_string();
        let result: PyResult<Self> = Python::with_gil(|py| {
            let sys = py.import("sys")?;
            sys.getattr("path")?
                .call_method1("insert", (0, &parent_str))?;
            let module = py.import(&file_stem)?.into();
            Ok(Self { module })
        });
        match result {
            Ok(engine) => {
                info(&format!("loaded theme: {path}"));
                Some(engine)
            }
            Err(e) => {
                error(&format!("failed to load theme {path}: {e}"));
                None
            }
        }
    }

    /// Call the theme's `render_prompt(**context)` and parse the result.
    ///
    /// Falls back to a plain `"<cwd> ❯ "` prompt on any Python error.
    pub fn render_prompt(&self, context: &HashMap<String, String>) -> ThemeResult {
        let default = ThemeResult::default_prompt(context);
        let result: PyResult<ThemeResult> = Python::with_gil(|py| {
            let kwargs = PyDict::new(py);
            for (k, v) in context {
                kwargs.set_item(k.as_str(), v.as_str())?;
            }
            let val = self
                .module
                .call_method(py, "render_prompt", (), Some(&kwargs))?;
            parse_theme_result(py, &val)
        });
        result.unwrap_or(default)
    }

    /// Call the theme's `render_right_prompt(**context)`.
    pub fn render_right_prompt(&self, context: &HashMap<String, String>) -> String {
        let result: PyResult<String> = Python::with_gil(|py| {
            let kwargs = PyDict::new(py);
            for (k, v) in context {
                kwargs.set_item(k.as_str(), v.as_str())?;
            }
            let val = self
                .module
                .call_method(py, "render_right_prompt", (), Some(&kwargs))?;
            val.extract::<String>(py)
        });
        result.unwrap_or_default()
    }

    /// Call the theme's `render_command_summary(**context)`.
    pub fn render_command_summary(&self, context: &HashMap<String, String>) -> String {
        let result: PyResult<String> = Python::with_gil(|py| {
            let kwargs = PyDict::new(py);
            for (k, v) in context {
                kwargs.set_item(k.as_str(), v.as_str())?;
            }
            let val = self
                .module
                .call_method(py, "render_command_summary", (), Some(&kwargs))?;
            val.extract::<String>(py)
        });
        result.unwrap_or_default()
    }

    /// Whether the theme module defines a `run()` (full-screen mode).
    pub fn has_run(&self) -> bool {
        Python::with_gil(|py| self.module.bind(py).hasattr("run").unwrap_or(false))
    }

    /// Invoke the theme's `run()` (full-screen mode). Returns `false` on error.
    pub fn run(&self) -> bool {
        Python::with_gil(|py| match self.module.call_method0(py, "run") {
            Ok(_) => true,
            Err(e) => {
                warning(&format!("python TUI exited: {e}"));
                false
            }
        })
    }
}

#[cfg(not(feature = "python"))]
impl ThemeEngine {
    /// Always `None` — the crate was built without Python support.
    pub fn load(_cfg: &PythonConfig) -> Option<Self> {
        None
    }

    /// Native fallback prompt.
    pub fn render_prompt(&self, context: &HashMap<String, String>) -> ThemeResult {
        ThemeResult::default_prompt(context)
    }

    /// Empty right prompt.
    pub fn render_right_prompt(&self, _context: &HashMap<String, String>) -> String {
        String::new()
    }

    /// Empty command summary.
    pub fn render_command_summary(&self, _context: &HashMap<String, String>) -> String {
        String::new()
    }

    /// Always `false`.
    pub fn has_run(&self) -> bool {
        false
    }

    /// Always `false`.
    pub fn run(&self) -> bool {
        false
    }
}

/// Parse a Python return value into a [`ThemeResult`].
///
/// Accepts either a dict with the documented keys or a plain string (used as the
/// single line above the prompt).
#[cfg(feature = "python")]
fn parse_theme_result(py: Python, val: &PyObject) -> PyResult<ThemeResult> {
    let mut res = ThemeResult::default();
    let any = val.bind(py);
    if let Ok(dict) = any.downcast::<PyDict>() {
        if let Ok(Some(v)) = dict.get_item("lines_above")
            && let Ok(list) = v.downcast::<pyo3::types::PyList>()
        {
            res.lines_above = list.iter().filter_map(|x| x.extract().ok()).collect();
        }
        if let Ok(Some(v)) = dict.get_item("input_prefix") {
            res.input_prefix = v.extract().unwrap_or_default();
        }
        if let Ok(Some(v)) = dict.get_item("right_prompt") {
            res.right_prompt = v.extract().unwrap_or_default();
        }
        if let Ok(Some(c)) = dict.get_item("colors")
            && let Ok(cd) = c.downcast::<PyDict>()
        {
            for item in cd.iter() {
                if let (Ok(key), Ok(val)) = (item.0.extract::<String>(), item.1.extract::<String>())
                {
                    res.colors.insert(key, val);
                }
            }
        }
        for item in dict.iter() {
            if let (Ok(key), Ok(val)) = (item.0.extract::<String>(), item.1.extract::<String>())
                && key != "lines_above"
                && key != "input_prefix"
                && key != "right_prompt"
                && key != "colors"
            {
                res.extra.insert(key, val);
            }
        }
    } else if let Ok(s) = any.extract::<String>() {
        res.lines_above = vec![s];
    }
    Ok(res)
}

impl ThemeResult {
    /// The native fallback prompt used when Python is unavailable or errors.
    fn default_prompt(context: &HashMap<String, String>) -> Self {
        let cwd = context.get("cwd").map(|s| s.as_str()).unwrap_or("~");
        Self {
            lines_above: vec![],
            input_prefix: format!("{cwd} ❯ "),
            right_prompt: String::new(),
            colors: HashMap::new(),
            extra: HashMap::new(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Static registry — themes installed via `t.desc` or the component's CLI.
// ──────────────────────────────────────────────────────────────────────────

/// A registered theme from a `t.desc` descriptor.
#[derive(Debug, Clone)]
pub struct ThemeEntry {
    pub name: String,
    pub path: String,
    pub description: String,
}

fn theme_registry() -> &'static Mutex<HashMap<String, ThemeEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ThemeEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ensure_tdesc_loaded() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for path in desc_candidates("t.desc") {
            if let Some(config) = load_desc::<ThemeDescConfig>(&path) {
                for (id, entry) in config.themes {
                    let expanded = expand_tilde(&entry.path);
                    let dest = Path::new(&expanded);
                    let description = entry.description.unwrap_or_default();
                    ThemeEngine::register_desc(&id, &entry.name, dest, &description);
                }
                info(&format!("loaded themes from t.desc: {}", path.display()));
                return;
            }
        }
    });
}

impl ThemeEngine {
    /// All registered themes (id → name → description → path).
    pub fn list() -> Vec<ThemeEntry> {
        ensure_tdesc_loaded();
        theme_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Look up a theme by id **or** display name.
    pub fn by_name(name: &str) -> Option<ThemeEntry> {
        ensure_tdesc_loaded();
        let registry = theme_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry
            .get(name)
            .cloned()
            .or_else(|| registry.values().find(|e| e.name == name).cloned())
    }

    /// Run a registered theme's file with `python3 <path>` and return its stdout.
    pub fn apply(entry: &ThemeEntry) -> Result<String, String> {
        ensure_tdesc_loaded();
        let path = expand_tilde(&entry.path);
        let std_path = std::path::PathBuf::from(&path);
        if !std_path.exists() {
            return Err(format!("theme file not found: {path}"));
        }
        let output = std::process::Command::new("python3")
            .arg(&path)
            .output()
            .map_err(|e| format!("failed to run theme: {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// Register a theme in-memory (`register` keeps a blank description).
    pub fn register(name: &str, dest: &Path) {
        ThemeEngine::register_desc(name, name, dest, "");
    }

    /// Register a theme with a display name and description.
    pub fn register_desc(name: &str, display_name: &str, dest: &Path, description: &str) {
        let mut registry = theme_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.insert(
            name.to_string(),
            ThemeEntry {
                name: display_name.to_string(),
                path: dest.to_string_lossy().to_string(),
                description: description.to_string(),
            },
        );
    }

    /// Remove a theme by id or display name.
    pub fn unregister(name: &str) {
        let mut registry = theme_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.remove(name);
        registry.retain(|_, e| e.name != name);
    }
}
