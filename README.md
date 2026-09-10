##

```
 ██████╗██████╗  ██████╗
██╔════╝██╔══██╗██╔════╝
██║     ██████╔╝███████╗
██║     ██╔═══╝ ╚════██║
╚██████╗██║     ██████║
 ╚═════╝╚═╝    ╚═════╝
```

##

`▐▀` `-` `▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▌`

Shared Python subsystem for **`[Cudane]`** ecosystem. `pyo3` engine for **`[plugins]`**, **`[themes]`**, and **`[TUIs]`**.

- **`[Version]`**: **`[0.0.70]`**

`▐▄` `-` `▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▌`

<details>
<summary>Contents</summary>

## Table of Contents

- [**`[Overview]`**](#overview)
- [**`[Architecture]`**](#architecture)
- [**`[Python]`**](#python)
- [**`[Descriptor files]`**](#descriptor-files)
- [**`[Configuration]`**](#configuration)
- [**`[Integration]`**](#integration)
- [**`[CLI reference]`**](#cli-reference)
- [**`[Building]`**](#building)
- [**`[Installation]`**](#installation)
- [**`[Testing]`**](#testing)
- [**`[Continuous integration]`**](#continuous-integration)
- [**`[Structure]`**](#structure)
- [**`[Dependencies]`**](#dependencies)
- [**`[Contributing]`**](#contributing)

</details>

<details>
<summary>Overview</summary>

## Overview

`cps` unifies Python host integration for the Cudane ecosystem. Instead of each
component keeping its own `src/python/{mod,plugin,theme,tui}.rs`, `cps` offers one
shared crate, one contract, and one runtime.

A host component integrates with `cps` like this:

```rust
use cps::{Options, PythonConfig, PythonEngine};

cps::configure(Options::new("context"));
let engine = PythonEngine::new(&PythonConfig::default());
```

From there, `cps` boots Python once, optionally activates a venv, loads the
configured theme and TUI, and attaches plugins to the host lifecycle.

</details>

<details>
<summary>Architecture</summary>

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                        cps crate                         │
│                                                          │
│  PythonConfig   config.rs     shared contract            │
│  PythonEngine   engine.rs     boot + venv + load         │
│  ThemeEngine    theme.rs      render_prompt / run        │
│  PluginManager  plugin.rs     hook discovery + registry  │
│  TuiEngine      tui.rs        run full-screen apps       │
│  Reporter       lib.rs        host-branded output        │
│  Options        lib.rs        desc dirs + reporter       │
└──────────────────────────────────────────────────────────┘
        │                          ▲
        │ configure(Options)       │ list/by_name/apply
        ▼                          │
┌─────────────────────────┐  ┌───────────────────────────────┐
│  Host component         │  │  Descriptor files             │
│  (csr/ctx/ous/mcx/lbt)  │  │  ~/.config/<brand>/{t,p}.desc │
└─────────────────────────┘  └───────────────────────────────┘
```

`cps` has two layers:

- **`[The engine]`** — Python runtime boot, venv activation, module loading.
- **`[The registries]`** — descriptor-backed theme/plugin/TUI indexes.

</details>

<details>
<summary>Python</summary>

## Python

A theme module may export these optional functions:

| Function | Signature | Purpose |
|---|---|---|
| `render_prompt` | `render_prompt(**context) -> dict \| str` | returns prompt state and styling |
| `render_right_prompt` | `render_right_prompt(**context) -> str` | right-aligned suffix |
| `render_command_summary` | `render_command_summary(**context) -> str` | one-line command summary |
| `run` | `run() -> bool` | full-screen TUI mode when `tui_mode` is enabled |

Render functions receive a context map with keys such as `cwd`, `user`, `host`,
`exit_code`, and `brand`.

A plugin module exposes top-level hook callables such as `on_startup`,
`on_shutdown`, and `on_command`.

A TUI module exposes a single `run()` entry point.

See [docs/PYTHON.md](docs/PYTHON.md) for full authoring guidance.

</details>

<details>
<summary>Descriptor files</summary>

## Descriptor files

`t.desc` registers themes and TUIs. `p.desc` registers plugins. Both are loaded
from descriptor directories configured by the host. The first existing valid file
wins.

```toml
# t.desc
[theme.cps]
name = "cps"
path = "~/CPS/themes/cps.py"
description = "Default cps theme with host-aware prompt"

[tui.installer]
name = "Installer"
path = "~/CPS/tuis/installer.py"
description = "Installer TUI for package workflows"
```

```toml
# p.desc
[plugin.example]
name = "Example"
path = "~/CPS/examples/example_plugin.py"
aliases = { hi = "echo 'hi from cps example plugin'" }
```

Registry commands such as `register`, `register_desc`, and `unregister`
update the in-memory index. Persistent installation is done by writing descriptor
files or using the host CLI.

</details>

<details>
<summary>Configuration</summary>

## Configuration

```toml
[python]
enabled = true
theme = "~/CPS/themes/cps.py"
tui = ""
plugins = ["~/CPS/examples/example_plugin.py"]
fallback_on_error = true
venv_path = "~/venvs/cudane"
tui_mode = false
```

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | bool | `false` | master switch — disables Python when false |
| `theme` | str | `""` | theme module path |
| `tui` | str | `""` | TUI module path |
| `plugins` | list | `[]` | plugin module paths |
| `fallback_on_error` | bool | `true` | fall back to native behavior on Python failure |
| `venv_path` | str | `""` | optional venv activation path |
| `tui_mode` | bool | `false` | run the theme's `run()` after startup |

`cps` config is loaded from `~/.config/cps/config.toml`, `/etc/cps/config.toml`,
or `./cps.toml`.

</details>

<details>
<summary>Integration</summary>

## Integration

| Component | Brand | Dependency | Integration |
|---|---|---|---|
| Cesar | `cesar` | `cps = { git = "https://github.com/Mapuse/CPS" }` | theme/plugin/TUI commands |
| Context | `context` | `cps = { git = "https://github.com/Mapuse/CPS" }` | shell prompt and startup hooks |
| Outsider | `ous` | `cps = { git = "https://github.com/Mapuse/CPS" }` | build-aware prompt and theme registry |
| MCX | `mcx` | `cps = { git = "https://github.com/Mapuse/CPS" }` | package-aware prompt and plugin aliases |
| Leon | `lbt` | `cps = { git = "https://github.com/Mapuse/CPS" }` | boot companion and preview tooling |

Every host:

1. depends on `cps` as an external crate;
2. calls `cps::configure(Options::new("<brand>"))` at startup;
3. replaces its own `crate::python::…` path with `cps::…`.

</details>

<details>
<summary>CLI reference</summary>

## CLI reference

```
cps theme    list | apply <name>    | register <name> <path>    | unregister <name>
cps plugin   list | run <alias> …   | register <name> <path>    | unregister <name>
cps tui      list | apply <name>    | register <name> <path>    | unregister <name>
cps engine   [--config <path>]                                   # boot engine and render sample prompt
```

This CLI is the reference integration surface for the subsystem.

</details>

<details>
<summary>Building</summary>

## Building

Requires Rust and Python headers for `pyo3`, plus the Cudane musl toolchain for
ecosystem builds.

### Target profiles

| Profile | Arch | Rust triple | Prefix |
|---|---|---|---|
| `amd64` | x86_64 | `x86_64-unknown-linux-musl` | `/system` |
| `arm64` | aarch64 | `aarch64-unknown-linux-musl` | `/system` |

`env.mk` auto-detects the host architecture and sets `RUST_TARGET` accordingly.
Pass `RUST_TARGET=<triple>` explicitly to override.

Define `TRIPLE` once for cross-install snippets below:

```sh
TRIPLE=x86_64-unknown-linux-musl   # amd64
TRIPLE=aarch64-unknown-linux-musl  # arm64
```

### Native build

```sh
cargo build --release --locked
# or
make build
# or
ninja
# or
meson setup builddir --prefix=/system && meson compile -C builddir
# or
cmake -B build -DCMAKE_INSTALL_PREFIX=/system && cmake --build build
```

### Cross-compile (amd64)

```sh
# Cargo
cargo build --release --locked --target x86_64-unknown-linux-musl

# Make
make build RUST_TARGET=x86_64-unknown-linux-musl

# Ninja
ninja                          # auto-detects via env.mk

# Meson
./scripts/crossgen.sh
meson setup builddir --cross-file cross.txt --prefix=/system
meson compile -C builddir

# CMake
cmake -B build -DCMAKE_TOOLCHAIN_FILE=toolchain.cmake -DCMAKE_INSTALL_PREFIX=/system
cmake --build build
```

### Cross-compile (arm64)

```sh
# Cargo
cargo build --release --locked --target aarch64-unknown-linux-musl

# Make
make build RUST_TARGET=aarch64-unknown-linux-musl

# Ninja
ninja                          # auto-detects via env.mk

# Meson
./scripts/crossgen.sh
meson setup builddir --cross-file cross.txt --prefix=/system
meson compile -C builddir

# CMake
cmake -B build -DCMAKE_TOOLCHAIN_FILE=toolchain.cmake -DCMAKE_INSTALL_PREFIX=/system
cmake --build build
```

### Lint and test

```sh
make clippy     # cargo clippy --all-targets -- -D warnings
make test       # cargo test --locked
```

All build paths use `--locked`. `cross.txt` is regenerated by `./scripts/crossgen.sh`.

</details>

<details>
<summary>Installation</summary>

## Installation

```sh
make install                                    # native install to $PREFIX
make install DESTDIR=$DESTDIR PREFIX=$PREFIX    # staged install under DESTDIR
```

### Cross-install (amd64)

```sh
TRIPLE=x86_64-unknown-linux-musl

# Cargo
cargo build --release --locked --target $TRIPLE
mkdir -p $DESTDIR/$PREFIX/bin
cp target/$TRIPLE/release/cps $DESTDIR/$PREFIX/bin/

# Make
make build  RUST_TARGET=$TRIPLE
make install RUST_TARGET=$TRIPLE DESTDIR=$DESTDIR PREFIX=$PREFIX

# Meson
meson setup builddir --cross-file cross.txt --prefix=$PREFIX
meson compile -C builddir
meson install -C builddir --DESTDIR=$DESTDIR

# CMake
cmake -B build -DCMAKE_TOOLCHAIN_FILE=toolchain.cmake -DCMAKE_INSTALL_PREFIX=$PREFIX
cmake --build build
cmake --install build --prefix $DESTDIR$PREFIX
```

### Cross-install (arm64)

```sh
TRIPLE=aarch64-unknown-linux-musl

# Cargo
cargo build --release --locked --target $TRIPLE
mkdir -p $DESTDIR/$PREFIX/bin
cp target/$TRIPLE/release/cps $DESTDIR/$PREFIX/bin/

# Make
make build  RUST_TARGET=$TRIPLE
make install RUST_TARGET=$TRIPLE DESTDIR=$DESTDIR PREFIX=$PREFIX

# Meson
meson setup builddir --cross-file cross.txt --prefix=$PREFIX
meson compile -C builddir
meson install -C builddir --DESTDIR=$DESTDIR

# CMake
cmake -B build -DCMAKE_TOOLCHAIN_FILE=toolchain.cmake -DCMAKE_INSTALL_PREFIX=$PREFIX
cmake --build build
cmake --install build --prefix $DESTDIR$PREFIX
```

Installs the `cps` binary to `${PREFIX}/bin/cps` and shared assets (themes,
descriptors, config) under `${PREFIX}/share/cps`.

### Environment variables

| Variable | Default | Meaning |
|---|---|---|
| `RUST_TARGET` | auto-detected | Rust target triple (`x86_64-unknown-linux-musl` or `aarch64-unknown-linux-musl`) |
| `PROFILE` | `release` | Build profile (`release` or `debug`) |
| `PREFIX` | `/system` | Install prefix — `make install` writes `${PREFIX}/bin/cps` and `${PREFIX}/share/cps/…` |
| `DESTDIR` | *(empty)* | Staging root — prepended to `PREFIX` for staged/cross installs |

</details>

<details>
<summary>Testing</summary>

## Testing

### Native baseline

```sh
cargo test --locked
```

Tests cover config parsing, path expansion, descriptor registry behavior, and the
disabled-engine path.

### Full amd64 gate

```sh
cargo fmt --check
make clippy     # cargo clippy --all-targets -- -D warnings
make test       # cargo test --locked
```

### Cross-test (arm64)

Pyo3 cannot cross-compile without a target `libpython`, so arm64 cross-testing
uses `--no-default-features` to exclude the `python` feature:

```sh
CC_aarch64_unknown_linux_musl=$PWD/toolchains/zig-aarch64-musl-cc
AR_aarch64_unknown_linux_musl=/usr/bin/ar
cargo test --locked --no-default-features --target aarch64-unknown-linux-musl
```

Test binaries execute under qemu-user/binfmt on an amd64 host. CI runs the arm64
leg natively on `ubuntu-24.04-arm` with default features (including `python`).

### Cross-test (amd64)

Pyo3 cannot cross-compile without a target `libpython`, so amd64 cross-testing
from an arm64 host also uses `--no-default-features` to exclude the `python` feature:

```sh
CC_x86_64_unknown_linux_musl=$PWD/toolchains/zig-x86_64-musl-cc
AR_x86_64_unknown_linux_musl=/usr/bin/ar
cargo test --locked --no-default-features --target x86_64-unknown-linux-musl
```

The x86_64 test binaries execute under qemu-user/binfmt on an arm64 host. CI
runs the amd64 leg natively on `ubuntu-latest` with default features (including `python`).

</details>

<details>
<summary>Continuous integration</summary>

## Continuous integration

### rust.yml (branch `shell`)

Matrix: `amd64` on `ubuntu-latest` + `arm64` on `ubuntu-24.04-arm`.

Steps: install `meson` / `ninja-build`, `cargo build --verbose`, `cargo test --verbose`.

The arm64 leg runs natively (default features, including `python`). The amd64
leg is the standard host gate.

### python-package.yml (branch `crate`)

Runs `flake8` across Python 3.9, 3.10, and 3.11.

</details>

<details>
<summary>Structure</summary>

## Structure

```text
src/lib.rs          # Reporter trait, Options, configure, module wiring
src/config.rs       # PythonConfig — shared contract
src/engine.rs       # PythonEngine — boot, venv activation, load
src/paths.rs        # path helpers, descriptor search, venv activation
src/theme.rs        # ThemeEngine, theme registry, render helpers
src/plugin.rs       # PluginManager, hook discovery, registry
src/tui.rs          # TuiEngine, TUI loading, runtime execute
src/bin/cps.rs      # reference CLI for theme/plugin/tui/engine
themes/             # cps.py, minimal.py
examples/           # example_plugin.py
tests/              # integration tests (no interpreter needed)
docs/               # authoring guide
```

</details>

<details>
<summary>Dependencies</summary>

## Dependencies

- `pyo3` for Python embedding
- `serde` / `toml` for config and descriptor parsing
- `clap` for the CLI
- `anyhow` / `thiserror` for error handling
- `parking_lot` / `once_cell` for runtime state

</details>

## Contributing

`cps` follows Cudane conventions:

- keep dependencies minimal and pinned
- preserve `--locked` builds
- keep host-specific behavior behind `Reporter` / `Options`
- handle Python failure safely with `fallback_on_error = true`

## License

**MIT License** ─ See [[**`LICENSE`**](https://github.com/Mapuse/.github/blob/profile/LICENSE)] for More Details.