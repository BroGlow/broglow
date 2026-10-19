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
    for name in ("index.md", "alternatives.md", "printing.md"):
        source = (Path(__file__).parent / "guide" / locale / name).read_text()
        page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/{name}"))
        public = on_page_markdown(
            source,
            page=page,
            config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
        )
        assert "/dist/" not in public and "{ download }" not in public
        if name == "alternatives.md":
            version = pages.cargo_package()["version"]
            assert "https://github.com/example/lamp/releases" in public
            assert "https://github.com/example/lamp/archive/HEAD.zip" in public
            assert f"broglow-v{version}-aarch64-apple-darwin.tar.gz" in public
            assert f"broglow-v{version}-x86_64-apple-darwin.tar.gz" in public
            # No release lookup without a site_url: the page says when checksums appear.
            assert (
                "once the release is published" in public
                or "после публикации" in public
            )
            assert ".sha256)" not in public, "No checksum download link"
        else:
            version = pages.cargo_package()["version"]
            archive = f"broglow-enclosure-magnetic-v{version}.zip"
            assert "https://github.com/example/lamp/releases" in public
            # Tests never set site_url, so no release lookup happens; the page says
            # where the checksum will appear instead of offering a .sha256 download.
            assert (
                "once the release is published" in public
                or "после публикации релиза" in public
            )
            assert ".sha256" not in public
            assert "https://github.com/example/lamp/archive/HEAD.zip" in public
            assert "printing.md#generate-from-source" in public
            assert "glued" not in public.lower() and "START_HERE" not in public
        with TemporaryDirectory() as temporary:
            pages.SITE = Path(temporary) / "site"
            local = on_page_markdown(
                source, page=page, config=SimpleNamespace(repo_url=None)
            )
            if name == "alternatives.md":
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
                digest = "c" * 64
                archive.with_name(archive.name + ".sha256").write_text(
                    f"{digest}  {archive.name}\n"
                )
                local = on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                prefix = "../" * (2 + (locale == "ru") + 1)
                assert f"]({prefix}dist/{archive.name}){{ download }}" in local
                # The checksum is shown inline, never offered as a second download.
                assert f"```text\n{digest}  {archive.name}\n```" in local
                assert ".sha256)" not in local
            else:
                assert "{ download }" not in local
                assert "printing.md#generate-from-source" in local
                dist = Path(temporary) / "dist/enclosure"
                dist.mkdir(parents=True)
                archive = dist / "broglow-enclosure-magnetic-v1.0.0.zip"
                archive.touch()
                assert "{ download }" not in on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                digest = "0" * 64
                archive.with_name(archive.name + ".sha256").write_text(
                    f"{digest}  {archive.name}\n"
                )
                local = on_page_markdown(
                    source, page=page, config=SimpleNamespace(repo_url=None)
                )
                prefix = "../" * (2 + (locale == "ru") + (name != "index.md"))
                assert (
                    f"]({prefix}dist/enclosure/{archive.name}){{ download }}" in local
                ), local
                # The checksum is shown inline, never offered as a second download.
                assert f"```text\n{digest}  {archive.name}\n```" in local
                assert ".sha256)" not in local
        pages.SITE = SITE

print(
    "PASS: separate Mac/enclosure downloads, inline checksums and source fallback in both languages"
)

# Checksum lines are accepted only in sha256sum format for the named archive.
assert pages.checksum_line(f"{'a' * 64}  x.zip\n", "x.zip") == f"{'a' * 64}  x.zip"
assert pages.checksum_line(f"{'a' * 64}  other.zip\n", "x.zip") is None
assert pages.checksum_line("<html>not found</html>", "x.zip") is None
untouched = "<!-- nothing to expand -->\n"
assert (
    on_page_markdown(
        untouched,
        page=SimpleNamespace(file=SimpleNamespace(src_uri="en/agents.md")),
        config=SimpleNamespace(repo_url="https://x", site_url="https://x/docs/"),
    )
    == untouched
), "Pages without download markers never look up releases"
print("PASS: checksum lines are validated; pages without markers skip lookups")

# Missing references still fail; references inside the repository resolve.
with TemporaryDirectory() as temporary:
    root = Path(temporary).resolve()
    pages.SITE = root / "site"
    model = root / "models/broglow"
    model.mkdir(parents=True)
    (model / "broglow.py").touch()
    (root / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
    for locale in ("en", "ru"):
        page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/printing.md"))
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
            '<a href="../broglow/broglow.py">Source</a>',
            page=page,
            config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
        )
        assert (
            'href="https://github.com/example/lamp/blob/HEAD/models/broglow/broglow.py"'
            in public
        )
    pages.SITE = SITE
print("PASS: missing or outside-repository references fail; repository files resolve")

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
        page_name = "index" if section == "lamp-wiring" else "alternatives"
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

# Prompts link the planned repository; the base URL is set once in Cargo.toml.
document = markdown.Markdown(
    extensions=[
        "pymdownx.superfences",
        pages.SectionAnchors("https://github.com/example/lamp"),
    ]
)
html = document.convert(
    "```text\nRead {{ repo_url }}/blob/HEAD/README.md\n```\n\n{{ repo_url }}"
)
assert (
    "{{ repo_url }}" not in html and html.count("https://github.com/example/lamp") == 2
)
assert pages.repository_url() == "https://github.com/broglow/broglow"
print("PASS: {{ repo_url }} resolves in text and code blocks")

# Remote links to published guides (needed inside ZIPs) stay on-site in the website.
page = SimpleNamespace(file=SimpleNamespace(src_uri="ru/printing.md"))
html = on_page_content(
    '<a href="https://github.com/example/lamp/blob/HEAD/site/guide/ru/hardware.md#usb-assisted-setup">x</a>',
    page=page,
    config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
)
assert 'href="../hardware/#usb-assisted-setup"' in html, html
# The shipped hardware manual links the print guide by its repository path.
page = SimpleNamespace(file=SimpleNamespace(src_uri="en/hardware.md"))
html = on_page_content(
    '<a href="https://github.com/example/lamp/blob/HEAD/models/magnetic/GUIDE_ROUND3X2.en.md#fit">x</a>'
    '<a href="https://github.com/example/lamp/blob/HEAD/models/magnetic/GUIDE_ROUND3X2.md">y</a>',
    page=page,
    config=SimpleNamespace(repo_url="https://github.com/example/lamp"),
)
assert 'href="../printing/#fit"' in html and 'href="../ru/printing/"' in html, html
print("PASS: remote guide links map to site routes")

# The root 404 page links home and both guides with absolute URLs.
page = pages.not_found_page("https://example.github.io/lamp/")
for link in ("lamp/", "lamp/docs/", "lamp/docs/ru/"):
    assert f'href="https://example.github.io/{link}"' in page, link
print("PASS: root 404 page links home and both guides")
