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
guide. Apple Silicon builds have been tested on a real Mac; Intel builds have not yet
been checked for installation and Bluetooth.

<!-- mac-downloads -->

Verify, extract and install the package by following
[Install a prebuilt package](hardware.md#install-a-prebuilt-package); it ends with
`"$HOME/.cargo/bin/broglow" install`. Then [check PATH](installation.md#path) and
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
