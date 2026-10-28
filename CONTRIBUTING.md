[English](CONTRIBUTING.md) | [Русский](CONTRIBUTING.ru.md)

# Contributing to BroGlow

Thanks for helping improve BroGlow. For bug reports, include your OS, BroGlow
version, steps to reproduce, the expected and actual result, and relevant output
(for example `broglow ble status` or logs). For changes, describe what changed
and how you checked it.

To install or use the app, see the [README](README.md) and
[setup manual](site/guide/en/hardware.md).

## Development setup

Run commands from the repository root. Use stable Rust with rustfmt and Clippy;
Windows builds use the MSVC toolchain. On Debian/Ubuntu, install the system
libraries required by the tray, webview and serial-port dependencies:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libudev-dev
```

The Linux tray's GTK 3 bindings (0.18, unmaintained) pull in `glib` 0.18
([RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html):
BroGlow does not call the unsound `VariantStrIter`; the fix in `glib` 0.20 is not
available to GTK 3) and the build-time `proc-macro-error`
([RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)).
Mac and Windows builds do not include them.

Install [uv](https://docs.astral.sh/uv/getting-started/installation/) 0.12.17 or
newer. The project uses Python 3.14, pinned in `.python-version`;
uv downloads it if it is missing. Sync the locked development tools and enable the commit hook:

```bash
uv sync --frozen --group dev
uv run --frozen --group dev prek install
```

The `dev` group in `pyproject.toml` includes the `test` group and pins Black,
clang-format and prek. The `docs` and `cad` groups pin the website and CAD toolchains, and `uv.lock`
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
and commit again. CI fails on unformatted Python. Keep Black's version in
`pyproject.toml` and `prek.toml` in sync.

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
uv run --frozen --group test pytest
uv run --frozen flash --build-only
uv run --frozen python scripts/check-generated-artifacts.py
```

Python tests use pytest functions, explicit fixtures and parametrized cases in
`test_*.py` modules. The `test` group pins pytest and includes the docs dependencies
needed by the website tests.

The firmware host test (`firmware/esp32-c3-supermini/tests/test_firmware.py`)
compiles the firmware logic with the system `c++`, so it needs a C++17 compiler.
Rust tests include end-to-end checks
of the compiled binary. For agent-integration, popover or lamp-status changes,
also run `node --test tests/js/*.test.mjs` after `cargo build --locked`
(needs Node.js 24). See
[test coverage and platform limits](CLAUDE.md#ci) for details.

The [CI workflow](.github/workflows/ci.yaml) has the firmware host, packaging
and platform checks. For icon changes, also run `assets/icons/render.py --check`.
CI does not build the firmware: for firmware changes, run the build above
and the [`led-test` build](site/guide/en/hardware.md#standalone-led-test-firmware) locally.

For a local app update and live service checks, follow the
[source installation procedure](site/guide/en/hardware.md#install-or-update-the-mac-service).
A successful build alone does not verify Bluetooth permissions or physical LEDs.

### GitHub Actions dependency caches

The Python jobs in CI and Pages explicitly enable setup-uv's dependency cache,
including their reuse by tagged releases. The cache uses the runner's OS and
architecture, detected Python version, `pyproject.toml`, `uv.lock` and the
`uv-0.12.17` suffix. Rust source and Cargo version changes retain it; Python,
uv, dependency manifest/lockfile or platform changes invalidate it. Update the
suffix when changing the pinned uv version.

The CI `firmware` job is the only Linux writer, on pushes to the default branch
(`master`); its `dev` group includes `test` and `docs`. Other CI/Pages jobs only
restore, as does `firmware` on PRs or release tags. After an authorized merge
or dependency change, the first successful `firmware` run on `master` seeds
the cache for subsequent PR and tag checks. Wait for that run if warm caches
are needed; no additional Linux warm-up job is required. Different tags cannot reuse
caches saved under each other's refs, so release checks read the default-branch
cache instead.

A cache miss performs the normal frozen dependency installation; checks still
run. Additional native uv/CAD caches remain deferred because their transfer and
population costs have not justified them.

#### Native release caches

Native release jobs restore dependency caches populated by the optional
[Warm release dependencies workflow](.github/workflows/cache-warm.yaml):

| Cache | Producer and consumer | Boundaries |
| --- | --- | --- |
| Cargo release dependencies | Matching `macos-15` ARM64, `macos-15-intel` x86_64 or `windows-latest` MSVC x86_64 jobs. | Separate release-profile keys by runner/target; compiler, image version, build/SDK environment and dependency hashes are included. Compatible older dependency caches may be restored after dependency changes; Cargo rebuilds what changed. |
| PlatformIO downloads | The `source` job on `macos-15`, the one job that builds the firmware. | Only `~/.platformio/platforms` and `~/.platformio/packages`; exact key includes runner/architecture, Python version, `platformio.ini`, `pyproject.toml` and `uv.lock`. No broader fallback. |
| Packaged C3 firmware | The `source` job writes it; every platform job takes it as the `firmware` artifact. | `dist/firmware`; exact key is the hash of the firmware sources, the packaging scripts, the firmware licenses, `pyproject.toml` and `uv.lock`. A hit skips the PlatformIO build entirely. Release runs populate it. |

Rust source and app-version-only changes retain the Cargo dependency cache and
the packaged firmware; firmware source changes retain the PlatformIO downloads.
Final app binaries, virtual environments (`.venv`), `.pio` build outputs and
release archives are not cached. Releases always run the locked app build, build
the firmware only when its inputs changed, and preserve the source, version,
freshness, license and checksum checks. The Mac jobs build only Rust and package
the firmware artifact with `package-macos.py --firmware`.

Optionally populate these caches on the default branch:

```bash
gh workflow run cache-warm.yaml --ref master
```

Wait for all three native jobs before expecting their caches in future tags.
The workflow only runs on the default branch; on `macos-15` it installs PlatformIO
dependencies without flashing and saves them before compiling Rust. An exact Cargo cache hit
skips compilation in the warm-up job. It does not package or publish a release.

Warm-up consumes runner time and cache storage; it is not required before every
release. Repeat it after a cache-key change or eviction only when expected reuse
justifies that cost. Release jobs only restore. Local performance evidence belongs
in ignored `.recovery/`; hosted runner timings and cache hit rates need separate
GitHub Actions verification.

## Documentation and generated files

Keep the README focused on the product, installation and everyday use. Put shared
development instructions here; [CLAUDE.md](CLAUDE.md) holds the code map and
implementation notes.

Keep English and Russian documents in sync as described in the
[website documentation guide](site/README.md#keep-the-instructions-in-sync), and run
`uv run --frozen --only-group docs python site/check-translations.py`.

Keep generated STL, STEP/STP, 3MF and ZIP files ignored and untracked, regardless
of filename case. Write enclosure deliverables under `dist/enclosure/`, which is
ignored as a whole. If a tool writes elsewhere, for example under `models/`,
confirm the path is ignored with `git check-ignore -v <path>` before committing.
Commit editable sources and reviewed slicer evidence (see the
[maintainer runbook](RELEASING.md)). Use only the project's own
images and follow the [media policy](MEDIA.md).

## Releases and website publishing

Follow the [maintainer runbook](RELEASING.md) for release preparation,
enclosure generation, packaging and publication, and the
[website runbook](site/README.md#publish-to-github-pages) for Pages deployment.
