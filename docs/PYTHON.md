# cps — Python authoring guide

Everything you need to write themes, plugins and TUIs for the Cudane ecosystem.
One file, one contract, works in Cesar, Context, Outsider, MCX and Leon.

## Themes

A theme is a single Python module. All functions are optional; cps only calls the
ones your module defines.

### `render_prompt(**context)`

Returns a **dict** (preferred) or a **plain string** (treated as the line above the
prompt).

Dict keys:

| Key | Type | Meaning |
|---|---|---|
| `lines_above` | `list[str]` | lines printed above the prompt |
| `input_prefix` | `str` | the prompt text itself |
| `right_prompt` | `str` | right-aligned suffix |
| `colors` | `dict[str, str]` | named colors (hex or ANSI), component-defined keys |
| *(anything else)* | `str` | collected into the `extra` map, passed through untouched |

```python
def render_prompt(**context):
    cwd = context.get("cwd", "~")
    code = int(context.get("exit_code", "0") or "0")
    mark = "\u2714" if code == 0 else "\u2718"
    return {
        "lines_above": [f"[{context.get('brand', 'cps')}]"],
        "input_prefix": f"{cwd} {mark} \u276f ",
        "right_prompt": "",
        "colors": {"accent": "#22d3ee", "error": "#ef4444"},
    }
```

### `render_right_prompt(**context) -> str`

Right-aligned suffix, e.g. git status or a clock.

### `render_command_summary(**context) -> str`

One line rendered after a command runs. `context` includes the exit code.

### `run() -> bool`

Full-screen mode. Invoked when `tui_mode = true` (or by `theme.apply`). Return
`True` on success.

### Context keys

Provided by the host component; at minimum `cwd`, `user`, `host`, `exit_code`,
`brand`. Components add their own (e.g. MCX passes package/target state, Leon passes
the BGRT rectangle and GOP framebuffer info from the boot report).

## Plugins

A plugin is a Python module whose top-level callables become event hooks. The module
is imported, every public top-level callable is discovered, and the host fires events
by name.

```python
def on_startup(**context):
    pass

def on_shutdown(**context):
    pass

def on_command(**context):
    cmd = context.get("command", "")
    ...
```

Events are fired with `**kwargs` built from a `HashMap<String, String>`. Modules with
no callables are skipped with a note.

Registering a plugin (e.g. `csr plugin install x.py`) records it in `p.desc`, where
you can also bind command aliases:

```toml
[plugin.example]
name = "Example"
path = "~/CPS/examples/example_plugin.py"
aliases = { build = "make -C . {}" }   # {} is replaced by the run args
```

`PluginManager::run` then executes the aliased command from the plugin's directory,
so plugins can wrap arbitrary build/deploy logic from any component CLI.

## TUIs

A TUI is a Python module exposing `run()`. cps imports it (its directory is prepended
to `sys.path`) and the host calls `run()` when it wants the full-screen app. Use
crossterm/rich/urwid from a venv — activate it with `venv_path` and everything you
`pip install` becomes importable.

```python
def run() -> bool:
    # full-screen app here
    return True
```

## Venvs

```toml
[python]
enabled = true
venv_path = "~/venvs/cudane"
```

cps prepends `<venv>/lib/pythonX.Y/site-packages` (or `Lib/site-packages` on Windows)
to `sys.path`, so themes/plugins/TUIs can import anything installed in the venv. This
is how components ship batteries-included Python without polluting the system Python.
