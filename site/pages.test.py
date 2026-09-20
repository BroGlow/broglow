"""Small check for local/public download routing in the MkDocs hook."""

from types import SimpleNamespace
from pathlib import Path
from tempfile import TemporaryDirectory
from html import unescape
import re
import markdown
from mkdocs.config import load_config

import pages
from pages import SITE, on_config, on_page_content, on_page_markdown

for locale in ("en", "ru"):
    for name in ("index.md", "installation.md"):
        source = (Path(__file__).parent / "guide" / locale / name).read_text()
        page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/{name}"))
        public = on_page_markdown(
            source,
            page=page,
            config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
        )
        assert "/dist/" not in public and "{ download }" not in public
        if name == "installation.md":
            assert "https://github.com/example/lamp/releases" in public
            assert "https://github.com/example/lamp/archive/HEAD.zip" in public
            assert "broglow-vVERSION-aarch64-apple-darwin.tar.gz" in public
            assert "broglow-vVERSION-x86_64-apple-darwin.tar.gz" in public
            assert (
                ".sha256" in public
                and "broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz" in public
            )
        else:
            assert "https://github.com/example/lamp/releases" in public
            assert "broglow-enclosure-vVERSION.zip" in public and ".sha256" in public
            assert "(printing.md)" in public
            assert "https://github.com/example/lamp/archive/HEAD.zip" in public
            assert "1.0.0" in public and "broglow-print-pack.zip" not in public
            assert "STL, STEP" in public
            assert (
                "source with CAD and STL" not in public
                and "исходники с CAD и STL" not in public
            )
        with TemporaryDirectory() as temporary:
            pages.SITE = Path(temporary) / "site"
            local = on_page_markdown(
                source, page=page, config=SimpleNamespace(repo_url=None)
            )
            if name == "installation.md":
                assert (
                    "hardware.md#build-from-source" in local and "/dist/" not in local
                )
                dist = Path(temporary) / "dist"
                dist.mkdir()
                archive = dist / "broglow-v1.0.0-aarch64-apple-darwin.tar.gz"
                archive.touch()
                assert "/dist/" not in on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                archive.with_name(archive.name + ".sha256").touch()
                local = on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                assert (
                    f"dist/{archive.name})" in local
                    and f"dist/{archive.name}.sha256)" in local
                )
            else:
                assert (
                    "broglow-print-pack.zip" not in local
                    and "{ download }" not in local
                )
                guide = "PRINT_GUIDE.md" if locale == "ru" else "PRINT_GUIDE.en.md"
                start = "START_HERE.ru.md" if locale == "ru" else "START_HERE.md"
                assert f"models/broglow/{guide}" in local and "(printing.md)" in local
                dist = Path(temporary) / "dist/enclosure"
                dist.mkdir(parents=True)
                archive = dist / "broglow-enclosure-v1.0.0.zip"
                archive.touch()
                assert "{ download }" not in on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                archive.with_name(archive.name + ".sha256").touch()
                local = on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                prefix = "../../../" if locale == "ru" else "../../"
                assert (
                    f"]({prefix}dist/enclosure/{archive.name}){{ download }}" in local
                )
                assert (
                    f"]({prefix}dist/enclosure/{archive.name}.sha256){{ download }}"
                    in local
                )
                assert start in local and f"models/broglow/{guide}" in local
        pages.SITE = SITE

print(
    "PASS: separate Mac/enclosure downloads and source fallback in both languages, including clean checkouts"
)

# Known generated files resolve without CAD outputs; other missing links still fail.
with TemporaryDirectory() as temporary:
    root = Path(temporary).resolve()
    pages.SITE = root / "site"
    model = root / "models/broglow"
    model.mkdir(parents=True)
    (model / "parts.json").write_bytes(
        (SITE.parent / "models/broglow/parts.json").read_bytes()
    )
    (model / "broglow.py").touch()
    (root / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
    for locale in ("en", "ru"):
        page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/printing.md"))
        for generated in (
            "stl/01_mascot_orange.stl",
            "stl/coupons/01_USB_base_coupon.stl",
            "broglow.step",
            "appearance.step",
            "exploded.step",
            "usb_coupon.step",
        ):
            assert not (model / generated).exists()
            sample = f'<a href="{generated}">Part</a>'
            local = on_page_content(
                sample, page=page, config=SimpleNamespace(repo_url=None)
            )
            assert 'href="../releasing/"' in local and generated in local
            public = on_page_content(
                sample,
                page=page,
                config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
            )
            assert (
                'href="https://github.com/example/lamp/releases/download/v1.2.3/broglow-enclosure-v1.2.3.zip"'
                in public
            )
            assert generated in public and "/blob/" not in public
        for invalid in (
            "missing.md",
            "missing.png",
            "missing.step",
            "stl/typo.stl",
            "../../../outside.stp",
        ):
            for repo in (None, "https://github.com/example/lamp"):
                try:
                    on_page_content(
                        f'<a href="{invalid}">Bad</a>',
                        page=page,
                        config=SimpleNamespace(repo_url=repo),
                    )
                except ValueError:
                    pass
                else:
                    raise AssertionError(
                        f"Accepted missing or outside-repository link: {invalid}"
                    )
        public = on_page_content(
            '<a href="broglow.py">Source</a>',
            page=page,
            config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
        )
        assert (
            'href="https://github.com/example/lamp/blob/HEAD/models/broglow/broglow.py"'
            in public
        )
    dist = root / "dist/enclosure"
    dist.mkdir(parents=True)
    archive = dist / "broglow-enclosure-v1.2.3.zip"
    archive.touch()
    archive.with_name(archive.name + ".sha256").touch()
    for locale in ("en", "ru"):
        page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/printing.md"))
        local = on_page_content(
            '<a href="broglow.step">Assembly</a>',
            page=page,
            config=SimpleNamespace(repo_url=None),
        )
        prefix = "../../../../" if locale == "ru" else "../../../"
        assert f'href="{prefix}dist/enclosure/{archive.name}" download' in local
    pages.SITE = SITE
print(
    "PASS: missing generated CAD routes to enclosure assets or generation instructions; other missing references fail"
)

config = on_config(load_config(str(SITE / "mkdocs.yaml")))
# GitHub-readable named anchors become native heading IDs after snippet expansion.
with TemporaryDirectory() as temporary:
    source = Path(temporary) / "section.md"
    source.write_text(
        '<a name="stable-section"></a>\n\n## Переведённый раздел\n\n'
        '```html\n<a name="code-example"></a>\n\n## Literal example\n```\n'
    )
    renderer = markdown.Markdown(
        extensions=[
            "pymdownx.snippets",
            "fenced_code",
            "attr_list",
            "toc",
            pages.SectionAnchors(),
        ],
        extension_configs={
            "pymdownx.snippets": {"base_path": [temporary], "check_paths": True}
        },
    )
    rendered = renderer.convert('--8<-- "section.md"')
    assert rendered.count('id="stable-section"') == 1 and "<a name=" not in rendered
    assert renderer.toc_tokens[0]["id"] == "stable-section"
    assert renderer.toc_tokens[0]["name"] == "Переведённый раздел"
    assert '<a name="code-example"></a>' in unescape(rendered)
    assert "## Literal example" in rendered and "{ #code-example }" not in rendered
master = (SITE / "guide/en/hardware.md").read_text()
for section, expected in (
    ("lamp-wiring", "GPIO3"),
    ("path-check", "command -v broglow"),
    ("path-setup", "export PATH="),
):
    for locale in ("en", "ru"):
        directive = f'--8<-- "site/guide/{locale}/hardware.md:{section}"'
        rendered = markdown.markdown(
            directive,
            extensions=config.markdown_extensions,
            extension_configs=config.mdx_configs,
        )
        assert expected in unescape(re.sub(r"<[^>]+>", "", rendered))
        page_name = "index" if section == "lamp-wiring" else "installation"
        assert directive in (SITE / "guide" / locale / f"{page_name}.md").read_text()

# Changing a manual changes its included diagram without editing the quick guide.
with TemporaryDirectory() as temporary:
    guide = Path(temporary) / "hardware.md"
    for value in ("220", "470"):
        guide.write_text(master.replace("220 Ω ── green", f"{value} Ω ── green"))
        rendered = markdown.markdown(
            '--8<-- "hardware.md:lamp-wiring"',
            extensions=["pymdownx.snippets"],
            extension_configs={
                "pymdownx.snippets": {"base_path": [temporary], "check_paths": True}
            },
        )
        assert f"{value} Ω ── green" in rendered

page = SimpleNamespace(file=SimpleNamespace(src_uri="en/hardware.md"))
sample = '<a href="../../../firmware/esp32-c3-supermini/">Firmware</a><img src="../../../assets/icons/broglow-red.png" alt="Red status lamp"><h2 id="camera-2026"></h2><a href="#camera--2026">Check</a>'
local = on_page_content(sample, page=page, config=SimpleNamespace(repo_url=None))
public = on_page_content(
    sample,
    page=page,
    config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
)
assert 'href="../' not in local and "firmware/esp32-c3-supermini" in local
assert "https://github.com/example/lamp/tree/HEAD/firmware/esp32-c3-supermini" in public
assert "data:image/png;base64," in local and 'href="#camera-2026"' in public
for locale in ("en", "ru"):
    page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/hardware.md"))
    sample = (
        '<a href="firmware.md#commands">Firmware</a>'
        f'<a href="../{"en" if locale == "ru" else "ru"}/hardware.md#wiring-and-resistor-choice">Language</a>'
    )
    for repo in (None, "https://github.com/example/lamp"):
        result = on_page_content(
            sample, page=page, config=SimpleNamespace(repo_url=repo)
        )
        assert 'href="../firmware/#commands"' in result
        expected = "../../hardware/" if locale == "ru" else "../ru/hardware/"
        assert f'href="{expected}#wiring-and-resistor-choice"' in result
        resolved = '<a href="../media/">Images</a>'
        assert (
            on_page_content(resolved, page=page, config=SimpleNamespace(repo_url=repo))
            == resolved
        )
        bar = '<p><a href="./">English</a> | <a href="media.ru.md">Русский</a></p>\n'
        assert (
            on_page_content(bar, page=page, config=SimpleNamespace(repo_url=repo)) == ""
        )
try:
    on_page_content(
        '<a href="../../outside.md">Bad</a>',
        page=page,
        config=SimpleNamespace(repo_url=None),
    )
except ValueError:
    pass
else:
    raise AssertionError("Reference escaped the repository")
print(
    "PASS: shared source snippets, live source edits, reference links/icons and anchors"
)
