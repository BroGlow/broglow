# ClawLight agent entry points

ClawLight is our standalone project, built for our ESP32-C3 SuperMini lamps and
local agent workflows. Compatibility with the referenced ClawLight project,
its C6 hardware, protocols, commands, or releases is not a requirement.
Choose behavior from this project's needs; preserve source attribution and
license notices separately from product requirements. Protect our existing
settings, Bluetooth bonds, and installed service identity when making changes.
Additional board targets are welcome when they need only a small build/pin
configuration change; mark hardware validation separately. C6 currently needs
a firmware port, so leave it deferred per the [board assessment](docs/hardware-setup.md#esp32-c6-status).

For **installation, updates, connecting a lamp, or checking the Mac service**, read
[the agent runbook](docs/hardware-setup.md#agent-runbook) first. It routes to the
prebuilt package, source build, USB pairing, and service validation procedures.
Use that guide as the single source of operating commands and completion criteria.

For **source changes, builds, or tests**, also read [CLAUDE.md](CLAUDE.md). Work from
this repository's root; the parent directory is a separate workspace. Preserve
existing user changes and Git history.

For **release packaging, distribution, or website publication**, follow
[the release runbook](docs/releasing.md). Prepare version 1.0.0 with the enclosure
release candidate as a separate print download. Source archives include our CAD;
source-archive exclusions do not remove files from Git history.

For **images, screenshots, or other media**, use only our own assets and follow
[the media policy and photo checklist](docs/media.md). Label CAD renders accurately
and use current source instead of old packages.

For **backups or test evidence**, use the ignored `.recovery/` directory and follow
the [local recovery policy](docs/hardware-setup.md#local-recovery-files).

A prebuilt package has `BUILD.json` and `bin/clawlight`; a source checkout has
`Cargo.toml` and `src/`. Select the matching installation path. A request to install
or connect does not imply publishing a release or reflashing a working board.

For **hardware-guide edits or changes to setup, wiring, controls, or status behavior**,
follow [website documentation maintenance](site/README.md#keep-the-instructions-in-sync)
in the same change. The website includes the hardware reference and shared command
blocks directly; review both quick-guide languages for affected summaries, then
build and check the local and Pages documentation before finishing.

For **YAML edits**, use block-style sequences throughout the project: one item per
line, including `needs` and workflow matrices. Inline arrays are not our style.
