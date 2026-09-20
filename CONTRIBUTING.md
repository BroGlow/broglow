[English](CONTRIBUTING.md) | [Русский](CONTRIBUTING.ru.md)

# Contributing to BroGlow

Thanks for helping improve BroGlow. For bug reports, include your OS, BroGlow
version, steps to reproduce and the expected result. For changes, describe what
changed and how you checked it.

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

Python 3.14 matches CI. Install [uv](https://docs.astral.sh/uv/getting-started/installation/),
sync the locked development tools and enable the commit hook:

```bash
uv sync --frozen --group dev
uv run --frozen --group dev prek install
```

The development group in `pyproject.toml` pins Black, clang-format and prek;
`uv.lock` fixes their transitive dependencies. Hooks use the native
[`prek.toml`](prek.toml) configuration.

## Formatting and hooks

```bash
cargo fmt
uv run --frozen --group dev black .
uv run --frozen --group dev prek run --all-files
```

Black uses its default style for all Python files. The prek hook formats staged
Python files; stage any formatting changes before committing again. CI runs
`uv run --frozen --group dev black --check --diff .` and fails if formatting is
needed. Keep Black's version in `pyproject.toml` and `prek.toml` in sync.

Firmware uses clang-format 23.1.1 with the repository's Google-style configuration.
YAML sequences use block style: one item per line, including workflow `needs`
and matrices.

## Build and test

```bash
cargo build --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
uv run --frozen --group dev black --check --diff .
uv run --frozen python -m unittest discover -s tests -p 'test_flash.py'
uv run --frozen flash --build-only
uv run --frozen python scripts/check-generated-artifacts.py
```

Rust tests include end-to-end checks of the compiled binary. For agent-integration
changes, also run `node --test tests/js/*.test.mjs` after building; CI runs that
suite on Linux. See [test coverage and platform limits](CLAUDE.md#ci) for details.
The [CI workflow](.github/workflows/ci.yaml) has the complete firmware, packaging
and platform checks; run the checks relevant to your change.

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

Keep generated STL, STEP/STP and ZIP files ignored and untracked, regardless of
filename case. Generate enclosure deliverables under `dist/enclosure/` and verify
their paths with `git check-ignore`. Commit editable sources and reviewed evidence.
Use our own images and follow the [media policy](site/guide/en/media.md).

## Releases and website publishing

Follow the [maintainer runbook](site/guide/en/releasing.md) for release preparation,
enclosure generation, packaging and publication, and the
[website runbook](site/README.md#publish-to-github-pages) for Pages deployment.
