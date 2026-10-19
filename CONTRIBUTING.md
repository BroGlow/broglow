[English](CONTRIBUTING.md) | [Русский](CONTRIBUTING.ru.md)

# Contributing to BroGlow

Thanks for helping improve BroGlow. For bug reports, include your OS, BroGlow
version, steps to reproduce, the expected and actual result, and relevant output
(for example `broglow ble status` or logs). For changes, describe what changed
and how you checked it.

These instructions apply to a source checkout. To install or use the app, start
with the [README](README.md) and [setup manual](site/guide/en/hardware.md).

## Development setup

Run commands from the repository root. Use stable Rust with rustfmt and Clippy;
Windows builds use the MSVC toolchain. On Debian/Ubuntu, install the system
libraries required by the tray, webview and serial-port dependencies:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libudev-dev
```

The Linux tray uses GTK 3 through `tao` and `tray-icon`. The GTK 3 Rust bindings
(0.18) are unmaintained and bring `glib` 0.18, which is affected by
[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
(unsound `VariantStrIter`, which BroGlow does not call; fixed only in `glib` 0.20,
which GTK 3 bindings cannot use), and the unmaintained build-time crate
`proc-macro-error` ([RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)).
Mac and Windows builds do not include these crates.

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) 0.12.17 or
newer (CI uses 0.12.17). The project uses Python 3.14, pinned in `.python-version`;
uv downloads it if it is missing. Sync the locked development tools and enable the commit hook:

```bash
uv sync --frozen --group dev
uv run --frozen --group dev prek install
```

The development group in `pyproject.toml` pins Black, clang-format and prek;
the `docs` and `cad` groups pin the website and CAD toolchains, and `uv.lock`
fixes their transitive dependencies. Hooks use the native
[`prek.toml`](prek.toml) configuration.

## Formatting and hooks

```bash
cargo fmt
uv run --frozen --group dev black .
uv run --frozen --group dev clang-format -i firmware/esp32-c3-supermini/src/*.cpp firmware/esp32-c3-supermini/tests/*.cpp
uv run --frozen --group dev prek run --all-files
```

Black uses its default style for all Python files. The prek hook runs Black on
staged Python files. If Black changes a file, the commit stops: stage the fixes
and commit again. CI runs
`uv run --frozen --group dev black --check --diff .` and fails if formatting is
needed. Keep Black's version in `pyproject.toml` and `prek.toml` in sync.

Firmware uses clang-format 23.1.1 with the Google-based
[`firmware/esp32-c3-supermini/.clang-format`](firmware/esp32-c3-supermini/.clang-format).
The prek hook does not run clang-format, so format firmware sources yourself; CI
fails on unformatted firmware.

Convention (not checked automatically): write YAML sequences in block style, one
item per line, including workflow `needs` and matrices.

## Build and test

```bash
cargo build --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
uv run --frozen --group dev black --check --diff .
uv run --frozen --group dev clang-format --dry-run --Werror firmware/esp32-c3-supermini/src/*.cpp firmware/esp32-c3-supermini/tests/*.cpp
uv run --frozen python firmware/esp32-c3-supermini/tests/check.py
uv run --frozen python -m unittest discover -s tests -p 'test_flash.py'
uv run --frozen flash --build-only
uv run --frozen python scripts/check-generated-artifacts.py
```

The firmware host test (`tests/check.py`) compiles the firmware logic with the
system `c++`, so it needs a C++17 compiler. Rust tests include end-to-end checks
of the compiled binary. For agent-integration, popover or lamp-status changes,
also run `node --test tests/js/*.test.mjs` after `cargo build --locked`; it needs
Node.js 24, as in CI, which runs that suite on Linux. See
[test coverage and platform limits](CLAUDE.md#ci) for details.

The [CI workflow](.github/workflows/ci.yaml) has the complete firmware, packaging
and platform checks. For packaging, icon or firmware build changes, also run the
matching steps of its `firmware` job: `tests/package_macos.py`,
`assets/icons/render.py --check` and the `led-test` PlatformIO build.

For a local app update and live service checks, follow the
[source installation procedure](site/guide/en/hardware.md#install-or-update-the-mac-service).
A successful build alone does not verify Bluetooth permissions or physical LEDs.

## Documentation and generated files

Keep the README focused on the product, installation and everyday use. Put shared
development instructions here; [CLAUDE.md](CLAUDE.md) holds the code map and
implementation notes.

Update English and Russian documents together, preserving section IDs, commands
and technical values. Follow the [website documentation guide](site/README.md#keep-the-instructions-in-sync)
for translation checks, local and Pages builds, and link checks.

Keep generated STL, STEP/STP, 3MF and ZIP files ignored and untracked, regardless
of filename case. Write enclosure deliverables under `dist/enclosure/`, which is
ignored as a whole. If a tool writes elsewhere, for example under `models/`,
confirm the path is ignored with `git check-ignore -v <path>` before committing.
Commit editable sources and reviewed slicer evidence (see the
[maintainer runbook](site/guide/en/releasing.md)). Use only the project's own
images and follow the [media policy](site/guide/en/media.md).

## Releases and website publishing

Follow the [maintainer runbook](site/guide/en/releasing.md) for release preparation,
enclosure generation, packaging and publication, and the
[website runbook](site/README.md#publish-to-github-pages) for Pages deployment.
