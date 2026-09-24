#!/usr/bin/env python3
"""Write third-party notices for BroGlow release packages.

rust: the crates Cargo compiles for the broglow binary on one target, with the
license and notice files shipped in each crate, plus the Rust standard library
notices from the toolchain that builds the binary.
firmware: the components linked into the ESP32-C3 image, read from its linker map.

Texts are copied from local sources (crate sources, the toolchain, PlatformIO
packages) or from reviewed upstream copies under licenses/. Nothing is invented.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
LICENSES = ROOT / "licenses"
NOTICES = "THIRD-PARTY-NOTICES.md"
RUST_STD = "RUST-STANDARD-LIBRARY-COPYRIGHT.html"
SHIPPED_TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
)

LICENSE_FILE = re.compile(
    r"(?i)^(licen[cs]e|copying|notice|copyright|unlicense)([-_.].*)?$"
)
SOURCE_SUFFIXES = {
    ".c",
    ".cc",
    ".cpp",
    ".h",
    ".in",
    ".js",
    ".json",
    ".py",
    ".rs",
    ".s",
    ".sh",
    ".toml",
    ".ts",
    ".yaml",
    ".yml",
}

# Template fields that must never appear as a copyright line. Verbatim license
# documents keep their "how to apply" instructions, which contain such fields.
PLACEHOLDERS = (
    "<year>",
    "<copyright holders>",
    "<owner>",
    "<name of author>",
    "[yyyy]",
    "[name of copyright owner]",
    "{yyyy}",
    "{name of copyright owner}",
)
INSTRUCTIONS = re.compile(
    r"APPENDIX: How to apply the Apache License to your work\.[\s\S]*?"
    r"limitations under the License\."
    r"|How to Apply These Terms to Your New (?:Programs|Libraries)[\s\S]*\Z"
)
FENCED = re.compile(r"(?ms)^(`{3,})text\n(.*?)^\1$")

# Standard texts for crates that ship no license file (SPDX license-list-data
# v3.29.0, text/<id>.txt). The MIT template copyright line and the Apache
# "how to apply" appendix are omitted: the crates state no copyright holder.
SPDX_SOURCE = "https://github.com/spdx/license-list-data/blob/v3.29.0/text/{}.txt"

# Crates whose package omits the license file that their repository provides.
# Copies are reviewed and pinned to the commit in the crate's .cargo_vcs_info.json;
# "complete" means the copy is itself the license text, not only an explanation.
OBJC2 = ("objc2-LICENSE.md", "LICENSE.md", False)
WEBVIEW2 = ("webview2-rs-LICENSE", "LICENSE", True)
UPSTREAM = {
    ("block2", "0.6.2"): ("b4167b582b2f75f9a1be75495c41b765344fd03c", *OBJC2),
    ("dispatch2", "0.3.1"): ("8852b424193ca41602281b3d7540d7c8ed51e49a", *OBJC2),
    ("objc2", "0.6.4"): ("8852b424193ca41602281b3d7540d7c8ed51e49a", *OBJC2),
    ("objc2-encode", "4.1.0"): ("8d214f5477365ffcbcbb7de058c86ed9a518efb7", *OBJC2),
    ("objc2-exception-helper", "0.1.1"): (
        "8d214f5477365ffcbcbb7de058c86ed9a518efb7",
        *OBJC2,
    ),
    **{
        (name, "0.3.2"): ("7b1abfd750a2cacaea71d6a56ecfb83cb7de560b", *OBJC2)
        for name in (
            "objc2-app-kit",
            "objc2-core-bluetooth",
            "objc2-core-foundation",
            "objc2-core-graphics",
            "objc2-foundation",
            "objc2-io-surface",
            "objc2-web-kit",
        )
    },
    ("webview2-com", "0.39.1"): (
        "edc2caf886175ccaebe86078c9cfe1ae2a187328",
        *WEBVIEW2,
    ),
    ("webview2-com-sys", "0.39.1"): (
        "edc2caf886175ccaebe86078c9cfe1ae2a187328",
        *WEBVIEW2,
    ),
    ("webview2-com-macros", "0.8.1"): (
        "dffa41a8a46d3f5565eefbff2de57d38d399f158",
        *WEBVIEW2,
    ),
}


class NoticeError(RuntimeError):
    pass


def command(*args):
    result = subprocess.run(
        args, cwd=ROOT, capture_output=True, text=True, encoding="utf-8"
    )
    if result.returncode:
        raise NoticeError(f"{' '.join(args)} failed:\n{result.stderr.strip()}")
    return result.stdout


def read_text(path):
    data = path.read_bytes()
    try:
        text = data.decode("utf-8-sig")
    except UnicodeDecodeError:
        text = data.decode("latin-1")
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return text.rstrip("\n") + "\n"


def placeholders(text):
    """Return template fields in one license text, outside its instructions."""
    text = INSTRUCTIONS.sub("", text)
    return sorted(field for field in PLACEHOLDERS if field in text)


def document_placeholders(markdown):
    """Return template fields in a notices file, checking each text on its own."""
    found = set()
    prose = FENCED.sub(
        lambda match: found.update(placeholders(match[2])) or "", markdown
    )
    found.update(field for field in PLACEHOLDERS if field in prose)
    return sorted(found)


def fence(text):
    runs = [len(run) for run in re.findall(r"`+", text)]
    return "`" * max(3, max(runs, default=0) + 1)


def block(text):
    marker = fence(text)
    return f"{marker}text\n{text}{marker}\n"


def cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ")


# Rust crates ------------------------------------------------------------------


def cargo_metadata(target):
    return json.loads(
        command(
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            target,
            "--manifest-path",
            str(ROOT / "Cargo.toml"),
        )
    )


def is_proc_macro(package):
    return any("proc-macro" in target["kind"] for target in package["targets"])


def library_name(package):
    for target in package["targets"]:
        if {"lib", "rlib", "proc-macro", "dylib", "cdylib", "staticlib"} & set(
            target["kind"]
        ):
            return target["name"]
    return package["name"]


def expand_features(package, requested):
    """Return (dependencies enabled by features, features requested on them)."""
    table = package["features"]
    optional = {
        d["rename"] or d["name"] for d in package["dependencies"] if d["optional"]
    }
    enabled, dependencies, requests, weak = set(), set(), {}, []
    pending = list(requested)
    while pending:
        feature = pending.pop()
        if feature in enabled:
            continue
        if feature not in table:
            if feature in optional:  # implicit feature of an optional dependency
                dependencies.add(feature)
            continue
        enabled.add(feature)
        for value in table[feature]:
            if value.startswith("dep:"):
                dependencies.add(value[4:])
            elif "/" in value:
                name, dependency_feature = value.split("/", 1)
                if name.endswith("?"):
                    weak.append((name[:-1], dependency_feature))
                else:
                    dependencies.add(name)
                    requests.setdefault(name, set()).add(dependency_feature)
            else:
                pending.append(value)
    for name, dependency_feature in weak:
        if name in dependencies:
            requests.setdefault(name, set()).add(dependency_feature)
    return dependencies, requests


def resolve(metadata):
    """Map each non-dev dependency of the root package to "binary" or "build".

    Cargo's metadata resolve graph also lists optional dependencies that are only
    named by weak features (dependency?/feature); Cargo does not build those. So
    features are re-derived from the root the way Cargo's feature resolver 2 does:
    dev-dependencies are skipped, and build scripts, procedural macros and their
    dependencies ("build") get feature sets separate from the linked code
    ("binary"). --filter-platform evaluates every edge for the target, which
    matches the native release builds.
    """
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    features = {(root, "binary"): {"default"}}
    pending = [(root, "binary")]
    while pending:
        key = pending.pop()
        package_id, context = key
        package = packages[package_id]
        dependencies, requests = expand_features(package, features[key])
        for edge in nodes[package_id]["deps"]:
            child = packages[edge["pkg"]]
            for kind in edge["dep_kinds"]:
                if kind["kind"] == "dev":
                    continue
                declared = [
                    d
                    for d in package["dependencies"]
                    if d["name"] == child["name"]
                    and d["kind"] == kind["kind"]
                    and d["target"] == kind["target"]
                    and (d["rename"] or library_name(child)).replace("-", "_")
                    == edge["name"]
                ]
                if not declared:
                    raise NoticeError(
                        f"Cannot match {package['name']} -> {child['name']} ({kind}) "
                        "to a Cargo.toml declaration."
                    )
                for dependency in declared:
                    name = dependency["rename"] or dependency["name"]
                    if dependency["optional"] and name not in dependencies:
                        continue
                    child_context = (
                        "build"
                        if context == "build"
                        or kind["kind"] == "build"
                        or is_proc_macro(child)
                        else "binary"
                    )
                    wanted = set(dependency["features"]) | requests.get(name, set())
                    if dependency["uses_default_features"]:
                        wanted.add("default")
                    child_key = (child["id"], child_context)
                    if child_key not in features or not wanted <= features[child_key]:
                        features.setdefault(child_key, set()).update(wanted)
                        pending.append(child_key)
    roles = {}
    for package_id, context in features:
        if package_id != root and roles.get(package_id) != "binary":
            roles[package_id] = context
    return packages[root], [(packages[i], role) for i, role in roles.items()]


def crates_for(target):
    """Return (root package, [(package, role)]) sorted by name and version."""
    root, crates = resolve(cargo_metadata(target))
    if any(not Path(package["manifest_path"]).is_file() for package, _ in crates):
        command("cargo", "fetch", "--locked", "--target", target)
    crates.sort(key=lambda item: (item[0]["name"], item[0]["version"]))
    for package, _ in crates:
        if not Path(package["manifest_path"]).is_file():
            raise NoticeError(
                f"Missing source for {package['name']} {package['version']}."
            )
    return root, crates


def shipped_files(package):
    directory = Path(package["manifest_path"]).parent
    found = []
    for folder, subfolders, filenames in os.walk(directory):
        subfolders[:] = sorted(d for d in subfolders if d not in (".git", "target"))
        for filename in sorted(filenames):
            path = Path(folder) / filename
            if (
                LICENSE_FILE.match(filename)
                and path.suffix.lower() not in SOURCE_SUFFIXES
            ):
                found.append(path)
    if package.get("license_file"):
        path = directory / package["license_file"]
        if path.is_file() and path.resolve() not in {p.resolve() for p in found}:
            found.append(path)
    result = []
    for path in found:
        try:
            name = path.resolve().relative_to(directory.resolve()).as_posix()
        except ValueError:
            name = path.name
        result.append((name, read_text(path)))
    return result


def license_ids(expression):
    ids = re.split(r"\s+(?:OR|AND|WITH)\s+|[()/]", expression or "")
    return sorted({part.strip() for part in ids if part.strip()})


def standard_text(spdx_id):
    path = LICENSES / "spdx" / f"{spdx_id}.txt"
    if not path.is_file():
        raise NoticeError(f"No reviewed standard text for {spdx_id} in {path.parent}.")
    text = read_text(path)
    text = re.sub(r"(?m)^Copyright \(c\) <year> <copyright holders>\n\n?", "", text)
    text = text.split("\nAPPENDIX: How to apply the Apache License")[0].rstrip() + "\n"
    if placeholders(text):
        raise NoticeError(f"Standard {spdx_id} text still has template fields.")
    return text


def upstream_copy(package):
    entry = UPSTREAM.get((package["name"], package["version"]))
    if not entry:
        return None
    commit, vendored, path, complete = entry
    info = Path(package["manifest_path"]).parent / ".cargo_vcs_info.json"
    recorded = json.loads(info.read_text(encoding="utf-8"))["git"]["sha1"]
    if recorded != commit:
        raise NoticeError(f"{package['name']} {package['version']}: commit changed.")
    url = f"{package['repository'].rstrip('/')}/blob/{commit}/{path}"
    return path, read_text(LICENSES / "rust" / vendored), url, complete


def rust_notices(target):
    """Return (Markdown notices, crates, crates that ship no license file)."""
    root, crates = crates_for(target)
    # LF-normalized, so Windows checkouts with CRLF conversion produce the same file.
    lock = hashlib.sha256(read_text(ROOT / "Cargo.lock").encode()).hexdigest()
    rustc = dict(
        line.split(": ", 1)
        for line in command("rustc", "-vV").splitlines()
        if ": " in line
    )
    texts, rows, missing = {}, [], []

    def reference(text, user, source=None):
        entry = texts.setdefault(
            text, {"id": f"T{len(texts) + 1}", "users": [], "source": source}
        )
        entry["users"].append(user)
        return f"[{entry['id']}](#{entry['id'].lower()})"

    for package, role in crates:
        label = f"{package['name']} {package['version']}"
        files = shipped_files(package)
        links = [
            f"{reference(text, f'{label} ({name})')} {name}" for name, text in files
        ]
        if not files:
            authors = package.get("authors") or []
            included = []
            copy = upstream_copy(package)
            complete = False
            if copy:
                path, text, url, complete = copy
                link = reference(
                    text,
                    f"{label} (upstream {path})",
                    "Not shipped in the crate packages below. Copied from their "
                    "repository at the commit recorded in each crate's "
                    "`.cargo_vcs_info.json`; the URLs are listed under "
                    "[Crates that ship no license file](#crates-that-ship-no-license-file).",
                )
                links.append(f"{link} upstream {path}")
                included.append(f"{link} `{path}` from {url}")
            if not complete:
                ids = license_ids(package["license"])
                if not ids:
                    raise NoticeError(
                        f"{label} has neither license files nor an SPDX id."
                    )
                for spdx_id in ids:
                    link = reference(
                        standard_text(spdx_id),
                        f"{label} (standard {spdx_id})",
                        f"Standard {spdx_id} text from {SPDX_SOURCE.format(spdx_id)}, "
                        "without the template's copyright line or the Apache "
                        '"how to apply" appendix.',
                    )
                    links.append(f"{link} standard {spdx_id}")
                    included.append(f"{link} standard `{spdx_id}` text")
            missing.append(
                (
                    package,
                    f"- **{label}** (`{package['license'] or 'no license field'}`) "
                    "ships no license file. "
                    + (
                        f"Cargo.toml authors: {', '.join(authors)}. "
                        if authors
                        else "Cargo.toml lists no authors. "
                    )
                    + "Included: "
                    + "; ".join(included)
                    + ".",
                )
            )
        rows.append(
            "| "
            + " | ".join(
                cell(value)
                for value in (
                    package["name"],
                    package["version"],
                    package["license"] or f"see {package.get('license_file')}",
                    package["repository"] or "not specified",
                    role,
                    ", ".join(links),
                )
            )
            + " |"
        )

    binary = sum(role == "binary" for _, role in crates)
    lines = [
        f"# Third-party notices: BroGlow {root['version']} for {target}",
        "",
        f"`broglow` is licensed under the MIT license ([LICENSE](LICENSE)). It is built "
        "with Rust and includes the Rust standard library and the crates listed below.",
        "",
        "- **Rust standard library:** compiled into the binary. Its copyright notices, "
        f"copied from the rustc {rustc['release']} toolchain "
        f"(`{rustc['commit-hash'][:9]}`), are in [{RUST_STD}]({RUST_STD}).",
        f"- **Crates:** {len(crates)} in total. {binary} are linked into the binary "
        f"(`binary`); {len(crates) - binary} only run while it is built, as build "
        "scripts, procedural macros or their dependencies (`build`).",
        "- **Texts:** each crate's license, notice and copyright files are reproduced "
        "as shipped in its source package, with line endings normalized. Identical "
        "texts appear once and list every crate that ships them.",
        f"- **Source:** `Cargo.lock` SHA-256 `{lock}`; crates resolved with "
        f"`cargo metadata --filter-platform {target}` and Cargo's feature rules, "
        "excluding dev-dependencies.",
        "",
        "## Crates",
        "",
        "| Crate | Version | License | Repository | Use | Texts |",
        "| --- | --- | --- | --- | --- | --- |",
        *rows,
        "",
    ]
    if missing:
        lines += [
            "## Crates that ship no license file",
            "",
            "No copyright line is added for these crates."
            + (
                " Standard texts omit the SPDX template's copyright line."
                if any("(standard " in u for e in texts.values() for u in e["users"])
                else ""
            ),
            "",
            *(note for _, note in missing),
            "",
        ]
    lines += ["## License and notice texts", ""]
    for text, entry in texts.items():
        lines += [f"### {entry['id']}", ""]
        if entry["source"]:
            lines += [entry["source"], ""]
        lines += ["Used by: " + "; ".join(entry["users"]) + ".", "", block(text)]
    document = "\n".join(lines).rstrip("\n") + "\n"
    found = document_placeholders(document)
    if found:
        raise NoticeError(f"Notices contain template fields: {', '.join(found)}")
    return document, crates, missing


def rust_std_notices():
    sysroot = Path(command("rustc", "--print", "sysroot").strip())
    path = sysroot / "share/doc/rust/COPYRIGHT-library.html"
    if not path.is_file():
        raise NoticeError(
            f"Missing {path}; install a Rust toolchain that ships the standard library notices."
        )
    return path


def write_rust_notices(directory, target):
    """Write the crate notices and the Rust standard library notices."""
    document, crates, missing = rust_notices(target)
    directory = Path(directory)
    (directory / NOTICES).write_text(document, encoding="utf-8", newline="\n")
    shutil.copyfile(rust_std_notices(), directory / RUST_STD)
    return crates, missing


# ESP32-C3 firmware ---------------------------------------------------------------

IDF = LICENSES / "esp-idf-v4.4.7"
IDF_SOURCE = "https://github.com/espressif/esp-idf/blob/v4.4.7/"
SDK_INCLUDE = "tools/sdk/esp32c3/include/"

# Each text: title, where it comes from, and how to read it.
FIRMWARE_TEXTS = {
    "esp-idf": (
        "Apache License 2.0 (ESP-IDF v4.4.7 license file)",
        ("vendored", "LICENSE", IDF_SOURCE + "LICENSE"),
    ),
    "copyright": (
        "ESP-IDF v4.4.7 copyrights and licenses",
        ("vendored", "COPYRIGHT.rst", IDF_SOURCE + "docs/en/COPYRIGHT.rst"),
    ),
    "freertos": (
        "FreeRTOS kernel notice",
        ("framework", SDK_INCLUDE + "freertos/include/freertos/FreeRTOS.h"),
    ),
    "lwip": (
        "lwIP notice",
        ("framework", SDK_INCLUDE + "lwip/lwip/src/include/lwip/init.h"),
    ),
    "mbedtls": (
        "Mbed TLS notice (Apache License 2.0 text: see above)",
        ("framework", SDK_INCLUDE + "mbedtls/mbedtls/include/mbedtls/version.h"),
    ),
    "wpa": (
        "wpa_supplicant license",
        (
            "vendored",
            "wpa_supplicant-README",
            IDF_SOURCE + "components/wpa_supplicant/README",
        ),
    ),
    "fastpbkdf2": (
        "fast-pbkdf2 dedication",
        (
            "vendored",
            "fastpbkdf2.c-L1-L19.txt",
            IDF_SOURCE + "components/wpa_supplicant/src/crypto/fastpbkdf2.c#L1-L19",
        ),
    ),
    "tlsf": (
        "TLSF allocator notice",
        (
            "vendored",
            "heap_tlsf.c-L1-L36.txt",
            IDF_SOURCE + "components/heap/heap_tlsf.c#L1-L36",
        ),
    ),
    "ubsan": (
        "UBSAN runtime notice",
        (
            "vendored",
            "ubsan.c-L1-L25.txt",
            IDF_SOURCE + "components/esp_system/ubsan.c#L1-L25",
        ),
    ),
    "newlib": (
        "newlib C library notices",
        ("toolchain", "share/licenses/newlib/COPYING.NEWLIB"),
    ),
    "gcc-exception": (
        "GCC Runtime Library Exception",
        ("toolchain", "share/licenses/gcc/COPYING.RUNTIME"),
    ),
    "gpl-3": (
        "GNU General Public License v3 (with the exception above)",
        ("toolchain", "share/licenses/gcc/COPYING3"),
    ),
}
SHIPPED_FIRMWARE_TEXTS = {
    "arduino": "ARDUINO-LICENSE.md",
    "ble": "BLE-LICENSE.txt",
}
# ESP-IDF v4.4.7 pins its precompiled RF libraries as submodules. Each repository's
# LICENSE is byte-identical to the vendored ESP-IDF LICENSE; esp32-wifi-lib's README
# states they are "provided under the same license as the parent esp-idf project".
ESPRESSIF_BINARY_REPOS = {
    "esp32-wifi-lib": "f2aae4d44ec7908013066e69d29b9948846c335c",
    "esp-phy-lib": "dcfdccf6cc2fc02d0886624b7998c890d1a19b28",
    "esp32c3-bt-lib": "e5c0f7256ecf5b5f8eb28c1793051a6b88f95124",
}


def espressif_binary(repo, note=""):
    """Archive entry for a precompiled library from an ESP-IDF submodule."""
    commit = ESPRESSIF_BINARY_REPOS[repo]
    return (
        f"Espressif precompiled library from [espressif/{repo}]"
        f"(https://github.com/espressif/{repo}/tree/{commit}), pinned by ESP-IDF "
        f"v4.4.7{note}",
        "Apache-2.0",
        ("esp-idf", "copyright"),
    )


FIRMWARE_ARCHIVES = {
    "libFrameworkArduino.a": (
        "Arduino-ESP32 2.0.17 core, compiled from source",
        "LGPL-2.1-or-later",
        ("arduino",),
    ),
    "libBLE.a": (
        "ESP32 BLE Arduino library from Arduino-ESP32 2.0.17, compiled from source",
        "Apache-2.0",
        ("ble",),
    ),
    "libfreertos.a": (
        "FreeRTOS kernel V10.4.3 with the ESP-IDF port",
        "MIT; ESP-IDF changes Apache-2.0",
        ("freertos", "esp-idf"),
    ),
    "liblwip.a": (
        "lwIP TCP/IP stack with the ESP-IDF port",
        "BSD-3-Clause; ESP-IDF port Apache-2.0",
        ("lwip", "esp-idf"),
    ),
    "libwpa_supplicant.a": (
        "wpa_supplicant with the ESP-IDF port, including fast-pbkdf2",
        "BSD-3-Clause; fast-pbkdf2 CC0-1.0; ESP-IDF port Apache-2.0",
        ("wpa", "fastpbkdf2", "esp-idf"),
    ),
    "libheap.a": (
        "ESP-IDF heap, including the TLSF allocator",
        "Apache-2.0; TLSF BSD-3-Clause",
        ("esp-idf", "tlsf"),
    ),
    "libesp_diagnostics.a": (
        "Espressif ESP Insights diagnostics, built into the framework package",
        "Apache-2.0",
        ("esp-idf",),
    ),
    "libesp_system.a": (
        "ESP-IDF system, including the UBSAN runtime",
        "Apache-2.0; ubsan.c BSD-2-Clause",
        ("esp-idf", "ubsan"),
    ),
    "libbt.a": (
        "ESP-IDF Bluetooth host: Bluedroid, BLE Mesh adapted from Zephyr, Espressif code",
        "Apache-2.0",
        ("esp-idf", "copyright"),
    ),
    "libnewlib.a": (
        "ESP-IDF newlib support",
        "Apache-2.0",
        ("esp-idf", "newlib"),
    ),
    "libnet80211.a": espressif_binary(
        "esp32-wifi-lib",
        "; ESP-IDF lists FreeBSD net80211 code (Sam Leffler, Errno Consulting, "
        "BSD license) without its license text",
    ),
    "libc.a": (
        "newlib C library from toolchain-riscv32-esp",
        "Various BSD-style and similar notices",
        ("newlib",),
    ),
    "libgcc.a": (
        "GCC 8.4.0 runtime from toolchain-riscv32-esp",
        "GPL-3.0-or-later WITH GCC-exception-3.1",
        ("gcc-exception", "gpl-3"),
    ),
    "libstdc++.a": (
        "GCC 8.4.0 C++ runtime from toolchain-riscv32-esp",
        "GPL-3.0-or-later WITH GCC-exception-3.1",
        ("gcc-exception", "gpl-3"),
    ),
    **{
        f"lib{name}.a": (
            "Mbed TLS 2.28.7 with ESP-IDF hardware support",
            "Apache-2.0 OR GPL-2.0-or-later; ESP-IDF port Apache-2.0",
            ("mbedtls", "esp-idf"),
        )
        for name in ("mbedcrypto", "mbedtls_2", "mbedx509")
    },
    **{
        f"lib{name}.a": espressif_binary("esp32-wifi-lib")
        for name in ("coexist", "core", "mesh", "pp")
    },
    **{f"lib{name}.a": espressif_binary("esp-phy-lib") for name in ("btbb", "phy")},
    "libbtdm_app.a": espressif_binary("esp32c3-bt-lib"),
    **{
        f"lib{name}.a": (f"ESP-IDF {name}", "Apache-2.0", ("esp-idf",))
        for name in (
            "app_update",
            "bootloader_support",
            "cxx",
            "driver",
            "efuse",
            "esp_common",
            "esp_eth",
            "esp_event",
            "esp_hw_support",
            "esp_netif",
            "esp_phy",
            "esp_pm",
            "esp_ringbuf",
            "esp_rom",
            "esp_timer",
            "esp_wifi",
            "espcoredump",
            "hal",
            "log",
            "nvs_flash",
            "pthread",
            "riscv",
            "soc",
            "spi_flash",
            "tcpip_adapter",
            "vfs",
        )
    },
}


def linked_archives(map_text):
    """Return {archive file name: number of linked members} from a GNU ld map."""
    loaded = map_text.split("\nDiscarded input sections", 1)[0]
    members = {}
    for path, member in re.findall(r"(?m)^(\S+\.a)\(([^)]+)\)$", loaded):
        members.setdefault(Path(path).name, set()).add(member)
    return {name: len(found) for name, found in sorted(members.items())}


def first_notice(path):
    """Return the first comment block in a header that carries a copyright notice."""
    text = read_text(path)
    for match in re.finditer(r"/\*[\s\S]*?\*/", text):
        if "Copyright" in match.group(0):
            return match.group(0) + "\n"
    raise NoticeError(f"No copyright comment in {path}.")


def firmware_notices(framework, toolchain, map_path):
    """Return Markdown notices for the firmware image described by map_path."""
    archives = linked_archives(read_text(map_path))
    unknown = sorted(set(archives) - set(FIRMWARE_ARCHIVES))
    if unknown:
        raise NoticeError(
            "Review the notices for newly linked archives: " + ", ".join(unknown)
        )
    framework_version = json.loads(
        (framework / "package.json").read_text(encoding="utf-8")
    )["version"]
    toolchain_version = json.loads(
        (toolchain / "package.json").read_text(encoding="utf-8")
    )["version"]
    sdk = read_text(framework / "tools/sdk/versions.txt").splitlines()[0]
    needed = {text_id for name in archives for text_id in FIRMWARE_ARCHIVES[name][2]}
    used = [text_id for text_id in FIRMWARE_TEXTS if text_id in needed]

    def link(text_id):
        if text_id in SHIPPED_FIRMWARE_TEXTS:
            name = SHIPPED_FIRMWARE_TEXTS[text_id]
            return f"[{name}]({name})"
        return f"[{text_id}](#{text_id})"

    lines = [
        "# Third-party notices: ESP32-C3 firmware",
        "",
        "These notices cover the images in this directory: `bootloader.bin`, "
        "`partitions.bin`, `boot_app0.bin` and `firmware.bin`. They were built with "
        f"PlatformIO from Arduino-ESP32 (framework package {framework_version}), its "
        f"precompiled ESP-IDF libraries ({sdk}) and toolchain-riscv32-esp "
        f"{toolchain_version}. The bootloader and `boot_app0.bin` come from the "
        "same framework package.",
        "",
        "The table lists every library archive that contributed code to "
        "`firmware.bin`, read from the linker map. Licenses are as stated by the "
        "upstream projects. Texts marked as files are in this directory.",
        "",
        "| Archive | Linked objects | Component | License | Texts |",
        "| --- | --- | --- | --- | --- |",
    ]
    for name, count in archives.items():
        component, license, text_ids = FIRMWARE_ARCHIVES[name]
        lines.append(
            f"| `{name}` | {count} | {cell(component)} | {cell(license)} | "
            + ", ".join(link(i) for i in text_ids)
            + " |"
        )
    lines += [
        "",
        "## Not available locally",
        "",
        "- The local framework package has no license text for Espressif's precompiled "
        "Wi-Fi, PHY, coexistence and Bluetooth controller libraries. They come from "
        "the ESP-IDF v4.4.7 submodules linked in the table, whose LICENSE files are "
        "the Apache License 2.0 text below.",
        "- ESP-IDF v4.4.7 names FreeBSD net80211 code in `libnet80211.a` but does not "
        "include that license text; none is reproduced here.",
        "- Texts from ESP-IDF v4.4.7 that the precompiled framework package omits were "
        "copied from the upstream tag; each section below names its source.",
        "",
        "## Texts",
        "",
    ]
    for text_id in used:
        if text_id in SHIPPED_FIRMWARE_TEXTS:
            continue
        title, source = FIRMWARE_TEXTS[text_id]
        if source[0] == "vendored":
            text = read_text(IDF / source[1])
            origin = f"Copied from {source[2]}."
        elif source[0] == "framework":
            text = first_notice(framework / source[1])
            origin = (
                f"First copyright comment of `{source[1]}` in the framework package."
            )
        else:
            text = read_text(toolchain / source[1])
            origin = f"`{source[1]}` from toolchain-riscv32-esp {toolchain_version}."
        lines += [
            f'<a id="{text_id}"></a>',
            "",
            f"### {title}",
            "",
            origin,
            "",
            block(text),
        ]
    document = "\n".join(lines).rstrip("\n") + "\n"
    found = document_placeholders(document)
    if found:
        raise NoticeError(
            f"Firmware notices contain template fields: {', '.join(found)}"
        )
    return document, archives


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    rust = commands.add_parser("rust", help="write Rust notices for one target")
    rust.add_argument("--target", required=True, choices=SHIPPED_TARGETS)
    rust.add_argument("--output-dir", type=Path, required=True)
    firmware = commands.add_parser("firmware", help="write firmware notices")
    firmware.add_argument(
        "--platformio-core", type=Path, default=Path.home() / ".platformio"
    )
    firmware.add_argument(
        "--map",
        type=Path,
        default=ROOT
        / "firmware/esp32-c3-supermini/.pio/build/esp32-c3-supermini/firmware.map",
    )
    firmware.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "rust":
            args.output_dir.mkdir(parents=True, exist_ok=True)
            crates, missing = write_rust_notices(args.output_dir, args.target)
            print(f"{args.output_dir / NOTICES}: {len(crates)} crates")
            for package, _ in missing:
                print(
                    f"  ships no license file: {package['name']} {package['version']}"
                )
        else:
            packages = args.platformio_core / "packages"
            document, archives = firmware_notices(
                packages / "framework-arduinoespressif32",
                packages / "toolchain-riscv32-esp",
                args.map,
            )
            args.output.write_text(document, encoding="utf-8", newline="\n")
            print(f"{args.output}: {len(archives)} linked archives")
    except NoticeError as error:
        raise SystemExit(f"error: {error}")


if __name__ == "__main__":
    main()
