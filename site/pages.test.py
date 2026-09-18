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
        public = on_page_markdown(source, page=page, config=SimpleNamespace(repo_url="https://github.com/example/lamp"))
        assert "/dist/" not in public and "{ download }" not in public
        if name == "installation.md":
            assert "https://github.com/example/lamp/releases" in public
            assert "https://github.com/example/lamp/archive/HEAD.zip" in public
            assert "clawlight-vVERSION-aarch64-apple-darwin.tar.gz" in public
            assert "clawlight-vVERSION-x86_64-apple-darwin.tar.gz" in public
            assert ".sha256" in public and "clawlight-cli-" in public
        else:
            assert "https://github.com/example/lamp/releases" in public
            assert "clawlight-enclosure-vVERSION.zip" in public and ".sha256" in public
            assert "https://github.com/example/lamp/blob/HEAD/models/clawlight/PRINT_GUIDE.md" in public
            assert "https://github.com/example/lamp/archive/HEAD.zip" in public
            assert "1.0.0" in public and "clawlight-print-pack.zip" not in public
        with TemporaryDirectory() as temporary:
            pages.SITE = Path(temporary) / "site"
            local = on_page_markdown(source, page=page, config=SimpleNamespace(repo_url=None))
            if name == "installation.md":
                assert "hardware.md#build-from-source" in local and "/dist/" not in local
                dist = Path(temporary) / "dist"
                dist.mkdir()
                archive = dist / "clawlight-v1.0.0-aarch64-apple-darwin.tar.gz"
                archive.touch()
                assert "/dist/" not in on_page_markdown(source, page=page, config=SimpleNamespace(repo_url=None))
                archive.with_name(archive.name + ".sha256").touch()
                local = on_page_markdown(source, page=page, config=SimpleNamespace(repo_url=None))
                assert f"dist/{archive.name})" in local and f"dist/{archive.name}.sha256)" in local
            else:
                assert "clawlight-print-pack.zip" not in local and "{ download }" not in local
                assert "models/clawlight/PRINT_GUIDE.md" in local
                models = Path(temporary) / "models"
                models.mkdir()
                (models / "clawlight-print-pack.zip").touch()
                local = on_page_markdown(source, page=page, config=SimpleNamespace(repo_url=None))
                prefix = "../../../" if locale == "ru" else "../../"
                assert f"]({prefix}models/clawlight-print-pack.zip){{ download }}" in local
                assert "START_HERE.md" in local and "models/clawlight/PRINT_GUIDE.md" in local
        pages.SITE = SITE

print("PASS: separate Mac/enclosure downloads and source fallback in both languages, including clean checkouts")

config = on_config(load_config(str(SITE / "mkdocs.yaml")))
master = (SITE.parent / "docs/hardware-setup.md").read_text()
for section, expected in (("lamp-wiring", "GPIO3"), ("path-check", "command -v clawlight"),
                          ("path-setup", 'export PATH=')):
    directive = f'--8<-- "docs/hardware-setup.md:{section}"'
    rendered = markdown.markdown(directive, extensions=config.markdown_extensions,
                                 extension_configs=config.mdx_configs)
    assert expected in unescape(re.sub(r"<[^>]+>", "", rendered))
    for locale in ("en", "ru"):
        page_name = "index" if section == "lamp-wiring" else "installation"
        assert directive in (SITE / "guide" / locale / f"{page_name}.md").read_text()

# Changing the source changes the included diagram, without editing either translation.
with TemporaryDirectory() as temporary:
    guide = Path(temporary) / "hardware.md"
    for value in ("220", "470"):
        guide.write_text(master.replace("220 Ω ── green", f"{value} Ω ── green"))
        rendered = markdown.markdown('--8<-- "hardware.md:lamp-wiring"',
                                     extensions=["pymdownx.snippets"],
                                     extension_configs={"pymdownx.snippets": {"base_path": [temporary], "check_paths": True}})
        assert f"{value} Ω ── green" in rendered

page = SimpleNamespace(file=SimpleNamespace(src_uri="en/hardware.md"))
sample = '<a href="../firmware/esp32-c3-supermini/">Firmware</a><img src="../assets/icons/clawd-red.png" alt="Red status lamp"><h2 id="camera-2026"></h2><a href="#camera--2026">Check</a>'
local = on_page_content(sample, page=page, config=SimpleNamespace(repo_url=None))
public = on_page_content(sample, page=page, config=SimpleNamespace(repo_url="https://github.com/example/lamp"))
assert 'href="../' not in local and "firmware/esp32-c3-supermini" in local
assert 'https://github.com/example/lamp/tree/HEAD/firmware/esp32-c3-supermini' in public
assert 'data:image/png;base64,' in local and 'href="#camera-2026"' in public
try:
    on_page_content('<a href="../../outside.md">Bad</a>', page=page, config=SimpleNamespace(repo_url=None))
except ValueError:
    pass
else:
    raise AssertionError("Reference escaped the repository")
print("PASS: shared source snippets, live source edits, reference links/icons and anchors")
