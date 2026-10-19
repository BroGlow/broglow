<a href="../../">Back to the project</a>

<a name="alternatives"></a>

# Alternative installations

The recommended route is [Homebrew](installation.md#homebrew). Use this page when you
cannot use Homebrew, want the offline Mac package, or run Windows or Linux. Every
route ends with `broglow install`, which registers the hooks in your coding agents
and, on macOS, installs the menu-bar app and starts it at login.

<a name="mac-package"></a>

## Mac package

The complete Mac package contains the app, the ESP32-C3 firmware and the operating
guide. Apple Silicon builds have been tested on a real Mac; Intel builds are unverified
for installation and Bluetooth.

<!-- mac-downloads -->

Verify, extract and install the package by following
[Install a prebuilt package](hardware.md#install-a-prebuilt-package); it ends with
`"$HOME/.cargo/bin/broglow" install`. Then [check PATH](#path) and
[pair the lamp](installation.md#pairing).

<a name="from-source"></a>

## Build from source (macOS and Linux)

You need [Rust stable](https://doc.rust-lang.org/book/ch01-01-installation.html); on
macOS also the Apple command-line tools (`xcode-select --install`), on Linux the
[system development libraries]({{ repo_url }}/blob/HEAD/CONTRIBUTING.md#development-setup)
for the GTK 3 tray. From a full source checkout:

```bash
cargo install --locked --path .
"$HOME/.cargo/bin/broglow" install
```

Details and checks: [Build from source](hardware.md#build-from-source). No Linux
binaries are published; on Linux the lamp works over USB only, and the tray's GTK 3
bindings carry known advisories listed in the contributing guide.

<a name="path"></a>

## Run broglow from any folder (PATH)

The package and source routes put the command in `~/.cargo/bin/broglow` (the prebuilt
package does not need Cargo); Homebrew's `$(brew --prefix)/bin` is already on PATH. In
a fresh terminal outside the package or source folder, check:

--8<-- "site/guide/en/hardware.md:path-check"

If the first line is not your `~/.cargo/bin/broglow` (shown as a full path, such as
`/Users/you/.cargo/bin/broglow`), add this line **once** to `~/.zshrc` (default macOS
shell), preserving existing settings:

--8<-- "site/guide/en/hardware.md:path-setup"

Open a new terminal and repeat the checks. If `~/.zshrc` already loads
`"$HOME/.cargo/env"` (the Rust installer adds it), change nothing; other shells use
their own startup file. `broglow install` does not change `PATH`.

<a name="windows"></a>

## Windows

Download `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` from
[Releases]({{ repo_url }}/releases): the tray app and CLI in `broglow.exe`, with USB
lamp support; no installer, Bluetooth, firmware or manuals. Extract it into a
permanent folder (for example `tar -xf <file>` in PowerShell) and run from that folder:

```powershell
.\broglow.exe install
```

`.\broglow.exe menubar` launches the tray directly; `.\broglow.exe` opens the
terminal dashboard. To build from source on Windows instead, use the Rust MSVC
toolchain, run `cargo install --locked --path .`, then `broglow install`.
