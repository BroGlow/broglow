# BroGlow agent entry points

[English](AGENTS.md) | [Русский](AGENTS.ru.md)

BroGlow combines a CLI, desktop app, ESP32-C3 SuperMini firmware and printable
enclosure for local coding-agent workflows. Describe its features and supported
hardware directly. Credit the original ClawLight in README and retain license notices. BroGlow is a
separate application with fresh identities and state; do not add legacy aliases,
migrations or old-key fallbacks. Protect unrelated user settings and hooks.

For **installation, updates, connecting a lamp, or checking the Mac service**, read
[the agent runbook](site/guide/en/hardware.md#agent-runbook) first. It routes to the
prebuilt package, source build, USB pairing, and service validation procedures.
Use that guide as the single source of operating commands and completion criteria.

For **source changes, builds, or tests**, also read [CLAUDE.md](CLAUDE.md). Work from
this repository's root. Preserve existing user changes and Git history.

**Generated artifacts — applies to every change:** Keep STL, STEP/STP and ZIP
files untracked and ignored everywhere, regardless of filename case. Preserve
their `.gitignore` rules; never force-add these files or put them in Git LFS.
Keep editable sources and reviewed evidence in Git; distribute generated files
as release assets with checksums. After generation, verify output paths with
`git check-ignore`; before committing, run
`uv run --frozen python scripts/check-generated-artifacts.py`.

Keep the entire `models/broglow/preview/` directory ignored and untracked, including
renders, dimension diagrams and render recipes. Keep selected app/site assets and
reviewed validation data in Git.

For **enclosure generation, release packaging, distribution, or website publication**,
follow [the release runbook](site/guide/en/releasing.md) and its manual/agent generation
procedure. Generate enclosure deliverables under ignored `dist/enclosure/`.
GitHub source archives contain sources and instructions; ready-to-print files
come from the separate enclosure release download.

For **images, screenshots, or other media**, use only our own assets and follow
[the media policy and photo checklist](site/guide/en/media.md). Label CAD renders accurately
and use current source instead of old packages.

For **backups or test evidence**, use the ignored `.recovery/` directory and follow
the [local recovery policy](site/guide/en/hardware.md#local-recovery-files).

A prebuilt package has `BUILD.json` and `bin/broglow`; a source checkout has
`Cargo.toml` and `src/`. Select the matching installation path. A request to install
or connect does not imply publishing a release or reflashing a working board.

For **hardware-guide edits or changes to setup, wiring, controls, or status behavior**,
follow [website documentation maintenance](site/README.md#keep-the-instructions-in-sync)
in the same change. Keep English and Russian versions of every project guide and
developer document in sync. The website includes the matching language source;
preserve section IDs, commands and technical values across translations. Run
`uv run --no-project --python 3.14 --with-requirements site/requirements-docs.txt python site/check-translations.py`,
then build and check local and Pages documentation before finishing. Dependency
license texts remain verbatim.

For **YAML edits**, use block-style sequences throughout the project: one item per
line, including `needs` and workflow matrices. Inline arrays are not our style.
