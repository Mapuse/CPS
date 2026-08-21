use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, Once, OnceLock};

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::PyDict;

use crate::config::PythonConfig;
use crate::expand_tilde;
use crate::info;
use crate::paths::{desc_candidates, load_desc};
#[cfg(feature = "python")]
use crate::warning;

/// Parsed `[plugin.<id>]` sections from a `p.desc` descriptor file.
#[derive(serde::Deserialize)]
struct PluginDescConfig {
    #[serde(rename = "plugin")]
    plugins: HashMap<String, PluginDescEntry>,
}

#[derive(serde::Deserialize)]
struct PluginDescEntry {
    name: String,
    path: String,
    #[serde(default)]
    aliases: HashMap<String, String>,
}

/// Live plugin manager: Python modules imported from `cfg.plugins`, whose top-level
/// callables are treated as event hooks and fired by [`PluginManager::fire`].
///
/// In addition to the live manager, a static registry holds plugins registered via
/// `p.desc` or the component CLI — see [`PluginManager::list`], [`PluginManager::run`].
pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
}

#[cfg(feature = "python")]
struct LoadedPlugin {
    name: String,
    module: PyObject,
    hooks: Vec<String>,
}

#[cfg(not(feature = "python"))]
struct LoadedPlugin;

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    /// Empty plugin manager.
    pub fn new() -> Self {
        Self { plugins: vec![] }
    }

    /// Import every module in `cfg.plugins`, collecting their top-level callables
    /// as event hooks. Broken or hook-less modules are skipped with a warning.
    #[cfg(feature = "python")]
    pub fn load_all(&mut self, cfg: &PythonConfig) {
        if cfg.plugins.is_empty() {
            return;
        }
        let plugins_result: PyResult<Vec<(String, PyObject, Vec<String>)>> = Python::with_gil(
            |py| {
                let mut loaded = Vec::new();
                let sys_path = py.import("sys")?.getattr("path")?;
                for plugin_path in &cfg.plugins {
                    let path = expand_tilde(plugin_path);
                    let std_path = std::path::PathBuf::from(&path);
                    if !std_path.exists() {
                        warning(&format!("plugin not found: {path}"));
                        continue;
                    }
                    let parent = match std_path.parent().and_then(|p| p.to_str()) {
                        Some(p) => p.to_string(),
                        None => {
                            warning(&format!("cannot determine parent of {path}"));
                            continue;
                        }
                    };
                    let file_stem = match std_path.file_stem().and_then(|s| s.to_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            warning(&format!("cannot determine name of {path}"));
                            continue;
                        }
                    };
                    let _ = sys_path.call_method1("insert", (0, &parent));
                    match load_one_plugin(py, &file_stem) {
                        Some((module, hooks)) => {
                            info(&format!(
                                "loaded plugin: {file_stem} (hooks: {})",
                                hooks.join(", ")
                            ));
                            loaded.push((file_stem, module, hooks));
                        }
                        None => {
                            info(&format!("plugin {file_stem} has no hooks, skipping"));
                        }
                    }
                }
                Ok(loaded)
            },
        );
        for (name, module, hooks) in plugins_result.unwrap_or_default() {
            self.plugins.push(LoadedPlugin { name, module, hooks });
        }
    }

    /// Fire an event to every loaded plugin that declares a matching hook.
    ///
    /// `data` becomes the `**kwargs` of the Python callable.
    #[cfg(feature = "python")]
    pub fn fire(&self, event: &str, data: &HashMap<String, String>) {
        let _ = Python::with_gil(|py| -> PyResult<()> {
            for plugin in &self.plugins {
                if plugin.hooks.contains(&event.to_string()) {
                    let kwargs = PyDict::new(py);
                    for (k, v) in data {
                        kwargs.set_item(k.as_str(), v.as_str())?;
                    }
                    let _ = plugin.module.call_method(py, event, (), Some(&kwargs));
                }
            }
            Ok(())
        });
    }

    /// Number of live plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Names of the live plugins.
    #[cfg(feature = "python")]
    pub fn names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name.clone()).collect()
    }

    /// Names of the live plugins (always empty without Python support).
    #[cfg(not(feature = "python"))]
    pub fn names(&self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(not(feature = "python"))]
impl PluginManager {
    /// No-op — the crate was built without Python support.
    pub fn load_all(&mut self, _cfg: &PythonConfig) {}

    /// No-op.
    pub fn fire(&self, _event: &str, _data: &HashMap<String, String>) {}
}

/// Import a module and collect its top-level callables (skipping `_`-names) as hooks.
#[cfg(feature = "python")]
fn load_one_plugin(py: Python, file_stem: &str) -> Option<(PyObject, Vec<String>)> {
    let module = py.import(file_stem).ok()?;
    let dir = module.dir().ok()?;
    let mut hooks = Vec::new();
    let builtins = py.import("builtins").ok()?;
    let callable = builtins.getattr("callable").ok()?;
    for item in dir.iter() {
        if let Ok(name) = item.extract::<String>() {
            if name.starts_with('_') {
                continue;
            }
            if let Ok(attr) = module.getattr(name.as_str())
                && callable
                    .call1((attr,))
                    .and_then(|r| r.extract::<bool>())
                    .unwrap_or(false)
            {
                hooks.push(name);
            }
        }
    }
    if hooks.is_empty() {
        return None;
    }
    Some((module.into(), hooks))
}

// ──────────────────────────────────────────────────────────────────────────
// Static registry — plugins installed via `p.desc` or the component's CLI.
// ──────────────────────────────────────────────────────────────────────────

/// A registered plugin from a `p.desc` descriptor.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub name: String,
    pub path: String,
    pub aliases: HashMap<String, String>,
}

fn plugin_registry() -> &'static Mutex<HashMap<String, PluginEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, PluginEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ensure_pdesc_loaded() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for path in desc_candidates("p.desc") {
            if let Some(config) = load_desc::<PluginDescConfig>(&path) {
                for (id, entry) in config.plugins {
                    let expanded = expand_tilde(&entry.path);
                    let dest = Path::new(&expanded);
                    let aliases = entry.aliases.clone();
                    PluginManager::register_desc(&id, &entry.name, dest, &aliases);
                }
                info(&format!("loaded plugins from p.desc: {}", path.display()));
                return;
            }
        }
    });
}

impl PluginManager {
    /// All registered plugins.
    pub fn list() -> Vec<PluginEntry> {
        ensure_pdesc_loaded();
        plugin_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Look up a plugin by one of its command aliases.
    ///
    /// Returns the entry plus the command string bound to that alias, so the caller
    /// can run it with [`PluginManager::run`].
    pub fn by_alias(alias: &str) -> Option<(PluginEntry, String)> {
        ensure_pdesc_loaded();
        let registry = plugin_registry().lock().unwrap_or_else(|e| e.into_inner());
        for entry in registry.values() {
            if let Some(cmd) = entry.aliases.get(alias) {
                return Some((entry.clone(), cmd.clone()));
            }
        }
        None
    }

    /// Run a registered plugin: the bound command (alias `func`) or a shell invocation
    /// of the plugin file, executed with `sh -c` from the plugin's directory.
    pub fn run(entry: &PluginEntry, func: &str, args: &[String]) -> Result<String, String> {
        ensure_pdesc_loaded();
        let path = expand_tilde(&entry.path);
        let std_path = std::path::PathBuf::from(&path);
        if !std_path.exists() {
            return Err(format!("plugin file not found: {path}"));
        }
        let mut command = func.to_string();
        let joined = args.join(" ");
        if command.contains("{}") {
            command = command.replace("{}", &joined);
        } else if !joined.is_empty() {
            command = format!("{command} {joined}");
        }
        let parent = std_path.parent().unwrap_or_else(|| Path::new("."));
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(parent)
            .output()
            .map_err(|e| format!("failed to run plugin: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if output.status.success() {
            Ok(stdout)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// Register a plugin in-memory.
    pub fn register(name: &str, dest: &Path, aliases: &HashMap<String, String>) {
        PluginManager::register_desc(name, name, dest, aliases);
    }

    /// Register a plugin with a display name and alias map.
    pub fn register_desc(
        name: &str,
        display_name: &str,
        dest: &Path,
        aliases: &HashMap<String, String>,
    ) {
        let mut registry = plugin_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.insert(
            name.to_string(),
            PluginEntry {
                name: display_name.to_string(),
                path: dest.to_string_lossy().to_string(),
                aliases: aliases.clone(),
            },
        );
    }

    /// Remove a plugin by id or display name.
    pub fn unregister(name: &str) {
        let mut registry = plugin_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry.remove(name);
        registry.retain(|_, e| e.name != name);
    }

    /// Look up a plugin by id or display name.
    pub fn by_name(name: &str) -> Option<PluginEntry> {
        ensure_pdesc_loaded();
        let registry = plugin_registry().lock().unwrap_or_else(|e| e.into_inner());
        registry
            .get(name)
            .cloned()
            .or_else(|| registry.values().find(|e| e.name == name).cloned())
    }
}
