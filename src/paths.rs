use std::path::{Path, PathBuf};

use crate::options;

/// Expand a leading `~` in a path against `$HOME`.
///
/// Used for every path that comes from configuration, so users can write
/// `theme = "~/themes/mine.py"` and it resolves regardless of the working directory.
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with('~')
        && let Ok(home) = std::env::var("HOME")
    {
        return path.replacen('~', &home, 1);
    }
    path.to_string()
}

/// Candidate descriptor files of a given name across every configured `desc_dir`,
/// plus the current working directory.
///
/// - `"t.desc"` — themes and TUIs
/// - `"p.desc"` — plugins
///
/// The first file that exists *and parses* wins.
pub fn desc_candidates(name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for dir in &options().desc_dirs {
        candidates.push(dir.join(name));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(name));
    }
    candidates
}

/// Activate a virtual environment by prepending its `site-packages` to `sys.path`.
///
/// Handles the standard POSIX layout (`<venv>/lib/pythonX.Y/site-packages`) and the
/// Windows layout (`<venv>/Lib/site-packages`).
pub fn activate_venv(path_str: &str) {
    use pyo3::prelude::*;

    let venv = PathBuf::from(expand_tilde(path_str));
    if !venv.exists() {
        crate::warning(&format!("venv not found: {}", venv.display()));
        return;
    }
    let _ = Python::with_gil(|py| -> PyResult<()> {
        let sys = py.import("sys")?;
        let sys_path = sys.getattr("path")?;
        let plat = std::env::consts::OS;
        let candidates: Vec<PathBuf> = if plat == "windows" {
            vec![venv.join("Lib").join("site-packages")]
        } else {
            let mut v = Vec::new();
            if let Ok(entries) = std::fs::read_dir(venv.join("lib")) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("python") {
                        v.push(entry.path().join("site-packages"));
                    }
                }
            }
            v.push(venv.join("Lib").join("site-packages"));
            v
        };
        for p in &candidates {
            if p.exists() {
                sys_path.call_method1("insert", (0, p.to_str().unwrap_or_default()))?;
                crate::info(&format!(
                    "activated venv: {} (site-packages: {})",
                    venv.display(),
                    p.display()
                ));
                return Ok(());
            }
        }
        crate::warning(&format!(
            "venv site-packages not found in: {}",
            venv.display()
        ));
        Ok(())
    });
}

/// Parse a descriptor file (`t.desc` / `p.desc`) at `path` into `T`.
///
/// Returns `None` when the file is missing or unparseable, so the search loop in
/// `desc_candidates` can move on to the next candidate.
pub(crate) fn load_desc<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}
