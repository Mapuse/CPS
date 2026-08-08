//! Integration tests for cps — the Cudane Python Subsystem.
//!
//! These run without a live Python interpreter: every assertion targets pure-Rust
//! behaviour (path expansion, config parsing, the static registries) or the disabled
//! engine path, which short-circuits before the interpreter is touched.

use std::collections::HashMap;
use std::path::Path;

use cps::plugin::PluginManager;
use cps::theme::ThemeEngine;
use cps::tui::TuiEngine;
use cps::{Options, PythonConfig, PythonEngine};

/// A reporter that just records what it received, so tests can assert on messages.
#[derive(Default)]
struct RecordingReporter {
    lines: std::sync::Mutex<Vec<String>>,
}

impl cps::Reporter for RecordingReporter {
    fn info(&self, msg: &str) {
        self.lines.lock().unwrap().push(format!("info: {msg}"));
    }
    fn warning(&self, msg: &str) {
        self.lines.lock().unwrap().push(format!("warn: {msg}"));
    }
    fn error(&self, msg: &str) {
        self.lines.lock().unwrap().push(format!("err: {msg}"));
    }
}

fn fresh_options(brand: &str) -> Options {
    Options::new(brand).with_reporter(std::sync::Arc::new(RecordingReporter::default()))
}

#[test]
fn engine_is_disabled_when_config_is_disabled() {
    cps::configure(fresh_options("cps-test-disabled"));

    let cfg = PythonConfig::default(); // enabled = false
    let engine = PythonEngine::new(&cfg);

    assert!(!engine.tui_mode);
    assert!(engine.theme.is_none());
    assert!(engine.tui.is_none());
    assert_eq!(engine.plugins.count(), 0);
}

#[test]
fn python_config_parses_from_toml() {
    let toml = r#"
        [python]
        enabled = true
        theme = "~/themes/x.py"
        plugins = ["~/plug/a.py", "/etc/plug/b.py"]
        venv_path = "~/venv"
        tui_mode = true
    "#;

    #[derive(serde::Deserialize)]
    struct Cfg {
        #[serde(default)]
        python: PythonConfig,
    }

    let cfg: Cfg = toml::from_str(toml).unwrap();
    assert!(cfg.python.enabled);
    assert_eq!(cfg.python.theme, "~/themes/x.py");
    assert_eq!(cfg.python.plugins.len(), 2);
    assert!(cfg.python.tui_mode);
    assert!(cfg.python.fallback_on_error); // default true when absent
}

#[test]
fn python_config_defaults_are_sane() {
    let cfg = PythonConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.theme.is_empty());
    assert!(cfg.tui.is_empty());
    assert!(cfg.plugins.is_empty());
    assert!(cfg.fallback_on_error);
    assert!(cfg.venv_path.is_empty());
    assert!(!cfg.tui_mode);
}

#[test]
fn expand_tilde_resolves_home() {
    // SAFETY: tests run in parallel but every mutation here uses the same value,
    // and HOME is only read (never written) by the code under test.
    unsafe { std::env::set_var("HOME", "/tmp/fakehome") };
    assert_eq!(cps::expand_tilde("~/x/y.py"), "/tmp/fakehome/x/y.py");
    assert_eq!(cps::expand_tilde("/abs/path"), "/abs/path");
    assert_eq!(cps::expand_tilde("relative"), "relative");
}

#[test]
fn theme_registry_register_lookup_unregister() {
    cps::configure(fresh_options("cps-test-theme"));

    let id = format!("theme-{}", std::process::id());
    ThemeEngine::register(&id, Path::new("/tmp/fake.py"));
    assert!(ThemeEngine::by_name(&id).is_some());
    assert!(ThemeEngine::list().iter().any(|t| t.name == id));
    ThemeEngine::unregister(&id);
    assert!(ThemeEngine::by_name(&id).is_none());
}

#[test]
fn plugin_registry_register_lookup_unregister() {
    cps::configure(fresh_options("cps-test-plugin"));

    let id = format!("plug-{}", std::process::id());
    let mut aliases = HashMap::new();
    aliases.insert("greet".to_string(), "echo hello {}".to_string());
    PluginManager::register(&id, Path::new("/tmp/plug.py"), &aliases);

    let found = PluginManager::by_alias("greet");
    assert!(found.is_some());
    let (entry, cmd) = found.unwrap();
    assert_eq!(entry.name, id);
    assert_eq!(cmd, "echo hello {}");

    PluginManager::unregister(&id);
    assert!(PluginManager::by_alias("greet").is_none());
}

#[test]
fn tui_registry_register_lookup_unregister() {
    cps::configure(fresh_options("cps-test-tui"));

    let id = format!("tui-{}", std::process::id());
    TuiEngine::register(&id, Path::new("/tmp/tui.py"));
    assert!(TuiEngine::by_name(&id).is_some());
    TuiEngine::unregister(&id);
    assert!(TuiEngine::by_name(&id).is_none());
}

#[test]
fn options_default_desc_dirs_cover_config_etc_and_cwd() {
    // SAFETY: tests run in parallel but every mutation here uses the same value.
    unsafe { std::env::set_var("HOME", "/tmp/fakehome") };
    let opts = Options::new("brand-x");
    let dirs = opts.desc_dirs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>();
    assert!(dirs.contains(&"/tmp/fakehome/.config/brand-x".to_string()));
    assert!(dirs.contains(&"/etc/brand-x".to_string()));
    assert!(dirs.contains(&".".to_string()));
}
