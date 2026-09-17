# ClawLight agent entry points

For **installation, updates, connecting a lamp, or checking the Mac service**, read
[the agent runbook](docs/hardware-setup.md#agent-runbook) first. It routes to the
prebuilt package, source build, USB pairing, and service validation procedures.
Use that guide as the single source of operating commands and completion criteria.

For **source changes, builds, or tests**, also read [CLAUDE.md](CLAUDE.md). Work from
this repository's root; the parent directory is a separate workspace. Preserve
existing user changes and the upstream Git history.

A prebuilt package has `BUILD.json` and `bin/clawlight`; a source checkout has
`Cargo.toml` and `src/`. Select the matching installation path. A request to install
or connect does not imply publishing a release or reflashing a working board.

For **hardware-guide edits or changes to setup, wiring, controls, or status behavior**,
follow [website documentation maintenance](site/README.md#keep-the-instructions-in-sync)
in the same change. The website includes the hardware reference and shared command
blocks directly; review both quick-guide languages for affected summaries, then
build and check the local and Pages documentation before finishing.
