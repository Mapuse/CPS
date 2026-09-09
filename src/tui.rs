use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, Once, OnceLock};

#[cfg(feature = "python")]
use pyo3::prelude::*;

use crate::config::PythonConfig;
#[cfg(feature = "python")]
use crate::error;
use crate::expand_tilde;
use crate::info;
use crate::paths::{desc_candidates, load_desc};
#[cfg(feature = "python")]
use crate::warning;

/// Parsed `[tui.<id>]` sections from a `t.desc` descriptor file.
#[derive(serde::Deserialize)]
struct TuiDescConfig {
    #[serde(rename = "tui")]
    tuis: HashMap<String, TuiDescEntry>,
}

#[derive(serde::Deserialize)]
struct TuiDescEntry {
    name: String,
    path: String,
    description: Option<String>,
}

/// A loaded Python TUI module. The module must expose a `run()` entry point; the host
/// component decides *when* to call it (e.g. after startup, or as a command). Built
/// without the `python` feature this is an inert stub.
#[cfg(feature = "python")]
pub struct TuiEngine {
    module: PyObject,
}

#[cfg(not(feature = "python"))]
pub struct TuiEngine {}

#[cfg(feature = "python")]
impl TuiEngine {
    /// Import the TUI module named by `cfg.tui` (a `.py` path).
    ///
    /// Returns `None` when unconfigured, missing or unimportable.
    pub fn load(cfg: &PythonConfig) -> Option<Self> {
        if cfg.tui.is_empty() {
            return None;
        }
        let path = expand_tilde(&cfg.tui);
        let std_path = std::path::PathBuf::from(&path);
        if !std_path.exists() {
            warning(&format!("tui file not found: {path}"));
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
                info(&format!("loaded tui: {path}"));
                Some(engine)
            }
            Err(e) => {
                error(&format!("failed to load tui {path}: {e}"));
                None
            }
        }
    }

    /// Whether the TUI module defines a `run()`.
    pub fn has_run(&self) -> bool {
        Python::with_gil(|py| self.module.bind(py).hasattr("run").unwrap_or(false))
    }

    /// Invoke the TUI's `run()`. Returns `false` on error.
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
impl TuiEngine {
    /// Always `None` — the crate was built without Python support.
    pub fn load(_cfg: &PythonConfig) -> Option<Self> {
        None
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

// ──────────────────────────────────────────────────────────────────────────
// Static registry — TUIs registered via `t.desc` or the component's CLI.
// ──────────────────────────────────────────────────────────────────────────

/// A registered TUI from a `t.desc` descriptor.
#[derive(Debug, Clone)]
pub struct TuiEntry {
    pub name: String,
    pub path: String,
    pub description: String,
}

fn tui_registry() -> &'static Mutex<HashMap<String, TuiEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, TuiEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ensure_tdesc_loaded() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for path in desc_candidates("t.desc") {
            if let Some(config) = load_desc::<TuiDescConfig>(&path) {
                for (id, entry) in config.tuis {
                    let expanded = expand_tilde(&entry.path);
                    let dest = Path::new(&expanded);
                    let description = entry.description.unwrap_or_default();
                    TuiEngine::register_desc(&id, &entry.name, dest, &description);
                }
                info(&format!("loaded tuis from t.desc: {}", path.display()));
                return;
            }
        }
    });
}

impl TuiEngine {
    /// All registered TUIs.
    pub fn list() -> Vec<TuiEntry> {
        ensure_tdesc_loaded();
        tui_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Look up a TUI by id **or** display name.
    pub fn by_name(name: &str) -> Option<TuiEntry> {
        ensure_tdesc_loaded();
        let registry = tui_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry
            .get(name)
            .cloned()
            .or_else(|| registry.values().find(|e| e.name == name).cloned())
    }

    /// Run a registered TUI's file with `python3 <path>` and return its stdout.
    pub fn apply(entry: &TuiEntry) -> Result<String, String> {
        ensure_tdesc_loaded();
        let path = expand_tilde(&entry.path);
        let std_path = std::path::PathBuf::from(&path);
        if !std_path.exists() {
            return Err(format!("tui file not found: {path}"));
        }
        let output = std::process::Command::new("python3")
            .arg(&path)
            .output()
            .map_err(|e| format!("failed to run tui: {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// Register a TUI in-memory (blank description).
    pub fn register(name: &str, dest: &Path) {
        TuiEngine::register_desc(name, name, dest, "");
    }

    /// Register a TUI with a display name and description.
    pub fn register_desc(name: &str, display_name: &str, dest: &Path, description: &str) {
        let mut registry = tui_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.insert(
            name.to_string(),
            TuiEntry {
                name: display_name.to_string(),
                path: dest.to_string_lossy().to_string(),
                description: description.to_string(),
            },
        );
    }

    /// Remove a TUI by id or display name.
    pub fn unregister(name: &str) {
        let mut registry = tui_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.remove(name);
        registry.retain(|_, e| e.name != name);
    }
}
