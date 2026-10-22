"""Behavioral checks for the website's MkDocs hooks (no release network calls)."""

from html import unescape
from html.parser import HTMLParser
from pathlib import Path
from types import SimpleNamespace
import os
import re

import markdown
import pytest

import pages

REPOSITORY_URL = "https://github.com/example/lamp"
DIGEST = "a" * 64


class Landing(HTMLParser):
    def __init__(self):
        super().__init__()
        self.elements = []
        self.text = []

    def handle_starttag(self, tag, attributes):
        self.elements.append((tag, dict(attributes)))

    def handle_data(self, data):
        self.text.append(data)


@pytest.mark.parametrize(
    "home,repo_url",
    [
        ("https://broglow.github.io/broglow/", "https://github.com/broglow/broglow"),
        ("https://example.github.io/lamp/", REPOSITORY_URL),
    ],
)
def test_landing_preview_without_javascript(home: str, repo_url: str) -> None:
    landing = Landing()
    landing.feed(pages.landing_page(home, repo_url))
    assert any(
        tag == "html" and attributes.get("lang") == "en"
        for tag, attributes in landing.elements
    )
    assert not re.search(r"[А-Яа-яЁё]", "".join(landing.text).replace("Русский", ""))
    assert ("link", {"rel": "canonical", "href": home}) in landing.elements
    metadata = {
        attributes.get("property", attributes.get("name")): attributes.get("content")
        for tag, attributes in landing.elements
        if tag == "meta"
    }
    assert metadata["og:type"] == "website"
    assert metadata["og:locale"] == "en_US"
    assert metadata["og:url"] == home
    assert metadata["twitter:card"] == "summary_large_image"
    assert metadata["og:title"] == metadata["twitter:title"]
    assert metadata["og:description"] == metadata["twitter:description"]
    assert metadata["og:image"] == metadata["twitter:image"]
    assert metadata["og:image:alt"] == metadata["twitter:image:alt"]
    for key in ("description", "og:title", "og:description", "og:image:alt"):
        assert metadata[key] and not re.search(r"[А-Яа-яЁё]", metadata[key])
    image = metadata["og:image"].removeprefix(home)
    assert image.startswith("assets/") and (pages.SITE / image).is_file()
    assert any(
        tag == "a"
        and "repository-badge" in attributes.get("class", "").split()
        and attributes.get("href") == repo_url
        for tag, attributes in landing.elements
    )
    assert any(
        tag == "script"
        and attributes.get("src", "").removeprefix("./").split("?")[0] == "github.js"
        and "defer" in attributes
        for tag, attributes in landing.elements
    )
    assert (pages.SITE / "github.js").is_file()


def test_published_landing_is_self_contained_and_reuses_native_ui(
    tmp_path: Path,
) -> None:
    home = "https://example.github.io/lamp/"
    guide = write(tmp_path, "docs/installation/index.html", "Existing guide")

    pages.write_landing(tmp_path, home, REPOSITORY_URL)

    assert guide.read_text() == "Existing guide"
    html = (tmp_path / "index.html").read_text()
    assert f'href="{home}"' in html
    assert f'content="{home}assets/photos/broglow-all-lights.jpg"' in html
    assert f'href="{REPOSITORY_URL}"' in html
    assert 'href="assets/favicon.svg"' in html
    for name in (
        "styles.css",
        "themes.css",
        "theme.js",
        "language.js",
        "app.js",
        "app-preview.js",
        "lamp.js",
        "ar.js",
        "ar.css",
        "github.js",
    ):
        assert (tmp_path / name).is_file()
        assert name in html
    assert "app-ui/popover/presentation.js" in html
    assert "app-ui/popover/document.js" in html
    assert "app-ui/icons/broglow-green.png" in html
    runtime = "\n".join(path.read_text() for path in tmp_path.glob("*.js"))
    for source in (html, runtime):
        for unwanted in (
            "../assets/",
            "../../",
            "demos/homepage",
            "https://broglow.github.io/broglow/",
        ):
            assert unwanted not in source
    assert "new URL('app-ui/popover.html', document.baseURI)" in runtime
    assert "app-ui/icons/broglow-${aggregateSignal}.png" in runtime
    assert "./assets/photos/${photos[photoSignal]}" in runtime
    assert "./assets/photos/${photos[modelSignal]}" in runtime
    assert 'import("./vendor/model-viewer-module-4.3.1.min.js")' in runtime
    assert f"phoneUrl('{home}')" in (tmp_path / "ar.js").read_text()
    assert '"three":"./vendor/three-0.183.0/three.module.min.js"' in html

    native = pages.SITE.parent / "assets"
    for source in (native / "popover").rglob("*"):
        if source.is_file():
            assert (
                tmp_path / "app-ui" / source.relative_to(native)
            ).read_bytes() == source.read_bytes()
    for name in (
        "popover.html",
        *(f"icons/broglow-{color}.png" for color in ("green", "yellow", "red", "none")),
    ):
        assert (tmp_path / "app-ui" / name).read_bytes() == (native / name).read_bytes()
    for path in (
        "assets/broglow.glb",
        "assets/photos/popover-dark.png",
        "assets/photos/broglow-all-lights.jpg",
        "assets/photos/signal-green.jpg",
        "assets/photos/signal-yellow.jpg",
        "assets/photos/signal-red.jpg",
        "vendor/model-viewer-module-4.3.1.min.js",
        "vendor/qrcode-generator-2.0.4.js",
        "vendor/three-0.183.0/three.module.min.js",
        "vendor/three-0.183.0/three.core.min.js",
    ):
        assert (tmp_path / path).read_bytes() == (pages.SITE / path).read_bytes()
    assert (tmp_path / "assets/favicon.svg").read_bytes() == (
        pages.SITE / "guide/assets/favicon.svg"
    ).read_bytes()


@pytest.fixture
def repository(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setattr(pages, "SITE", tmp_path / "site")
    write(tmp_path, "Cargo.toml", '[package]\nversion = "1.2.3"\n')
    return tmp_path


@pytest.fixture
def offline_releases(monkeypatch: pytest.MonkeyPatch) -> None:
    def unexpected_fetch(repo_url: str, version: str, archive: str) -> None:
        pytest.fail(f"Unexpected release lookup: {repo_url}/{version}/{archive}")

    monkeypatch.setattr(pages, "release_checksum", unexpected_fetch)


pytestmark = pytest.mark.usefixtures("offline_releases")


def write(root: Path, path: str, content: str = "") -> Path:
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)
    return target


def page(source: str = "en/index.md", url: str = "") -> SimpleNamespace:
    return SimpleNamespace(file=SimpleNamespace(src_uri=source), url=url)


def config(public: bool = False, published: bool = False) -> SimpleNamespace:
    return SimpleNamespace(
        repo_url=REPOSITORY_URL if public else None,
        site_url="https://example.github.io/lamp/docs/" if published else None,
    )


def render_download(marker: str, source: str = "en/index.md") -> str:
    return pages.on_page_markdown(
        f"<!-- {marker} -->", page=page(source), config=config()
    )


def package(root: Path, path: str, modified: int = 1, complete: bool = True) -> Path:
    archive = write(root, path)
    os.utime(archive, (modified, modified))
    if complete:
        write(root, f"{path}.sha256", f"{DIGEST}  {archive.name}\n")
    return archive


@pytest.mark.parametrize(
    ("marker", "path", "fallback"),
    (
        pytest.param(
            "mac-downloads",
            "dist/broglow-v1.0.0-aarch64-apple-darwin.tar.gz",
            "hardware.md#build-from-source",
            id="mac",
        ),
        pytest.param(
            "model-download",
            "dist/enclosure/broglow-models-v1.0.0.zip",
            "printing.md#generate-from-source",
            id="enclosure",
        ),
    ),
)
def test_local_downloads_require_a_complete_package(
    repository: Path, marker: str, path: str, fallback: str
) -> None:
    package(repository, path, complete=False)

    rendered = render_download(marker)

    assert fallback in rendered
    assert "{ download }" not in rendered


@pytest.mark.parametrize(
    ("marker", "pattern"),
    (
        pytest.param(
            "mac-downloads", "dist/broglow-v{}-aarch64-apple-darwin.tar.gz", id="mac"
        ),
        pytest.param(
            "model-download", "dist/enclosure/broglow-models-v{}.zip", id="enclosure"
        ),
    ),
)
@pytest.mark.parametrize(
    ("source", "prefix"),
    (
        pytest.param("en/index.md", "../../", id="en-index"),
        pytest.param("en/alternatives.md", "../../../", id="en-nested"),
        pytest.param("ru/index.md", "../../../", id="ru-index"),
        pytest.param("ru/printing.md", "../../../../", id="ru-nested"),
    ),
)
def test_local_downloads_select_newest_complete_package_at_each_page_depth(
    repository: Path, marker: str, pattern: str, source: str, prefix: str
) -> None:
    package(repository, pattern.format("1.0.0"), modified=1)
    archive = package(repository, pattern.format("2.0.0"), modified=2)
    package(repository, pattern.format("3.0.0"), modified=3, complete=False)

    rendered = render_download(marker, source)

    path = archive.relative_to(repository).as_posix()
    assert f"]({prefix}{path}){{ download }}" in rendered
    assert f"{DIGEST}  {archive.name}" in rendered
    assert ".sha256)" not in rendered


def test_mismatched_checksum_does_not_advertise_a_package(repository: Path) -> None:
    archive = package(repository, "dist/enclosure/broglow-models-v1.0.0.zip")
    archive.with_name(archive.name + ".sha256").write_text(
        f"{DIGEST}  another-package.zip\n"
    )

    with pytest.raises(ValueError, match="Not a sha256sum line"):
        render_download("model-download")


@pytest.mark.usefixtures("repository")
@pytest.mark.parametrize(
    "locale", (pytest.param("en", id="en"), pytest.param("ru", id="ru"))
)
def test_public_preview_routes_to_releases_without_fetching_checksums(
    locale: str,
) -> None:
    rendered = pages.on_page_markdown(
        "<!-- mac-downloads -->\n<!-- model-download -->",
        page=page(f"{locale}/index.md"),
        config=config(public=True),
    )

    assert f"]({REPOSITORY_URL}/releases)" in rendered
    assert f"]({REPOSITORY_URL}/archive/HEAD.zip)" in rendered
    for archive in (
        "broglow-v1.2.3-aarch64-apple-darwin.tar.gz",
        "broglow-v1.2.3-x86_64-apple-darwin.tar.gz",
        "broglow-models-v1.2.3.zip",
    ):
        assert archive in rendered
    assert "/dist/" not in rendered


@pytest.mark.usefixtures("repository")
def test_published_downloads_show_release_checksums_inline(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def release_checksum(repo_url: str, version: str, archive: str) -> str:
        return f"{DIGEST}  {archive}"

    monkeypatch.setattr(pages, "release_checksum", release_checksum)

    rendered = pages.on_page_markdown(
        "<!-- model-download -->",
        page=page(),
        config=config(public=True, published=True),
    )

    checksum = f"{DIGEST}  broglow-models-v1.2.3.zip"
    assert f"```text\n{checksum}\n```" in rendered
    assert ".sha256)" not in rendered


def test_pages_without_downloads_do_not_fetch_release_metadata() -> None:
    source = "Ordinary guide content"

    result = pages.on_page_markdown(
        source, page=page(), config=config(public=True, published=True)
    )

    assert result == source


@pytest.mark.parametrize(
    ("text", "expected"),
    (
        pytest.param(f"{DIGEST}  model.zip\n", f"{DIGEST}  model.zip", id="valid"),
        pytest.param(f"{DIGEST}  other.zip", None, id="wrong-archive"),
        pytest.param("<html>Not found</html>", None, id="error-page"),
    ),
)
def test_checksum_parser_requires_sha256sum_for_the_named_archive(
    text: str, expected: str | None
) -> None:
    assert pages.checksum_line(text, "model.zip") == expected


@pytest.mark.usefixtures("repository")
@pytest.mark.parametrize(
    "target",
    (
        pytest.param("missing.md", id="missing"),
        pytest.param("../../../outside.stp", id="outside-repository"),
    ),
)
@pytest.mark.parametrize(
    "public", (pytest.param(False, id="local"), pytest.param(True, id="public"))
)
def test_manual_links_reject_missing_and_outside_repository_targets(
    target: str, public: bool
) -> None:
    with pytest.raises(
        ValueError, match="Missing or outside-repository manual reference"
    ):
        pages.on_page_content(
            f'<a href="{target}">Broken</a>',
            page=page("en/printing.md"),
            config=config(public=public),
        )


def test_manual_repository_files_and_icons_work_in_local_and_public_guides(
    repository: Path,
) -> None:
    write(repository, "firmware/board/main.cpp")
    write(repository, "assets/icon.png", "image fixture")
    sample = (
        '<a href="../../../firmware/board/">Firmware</a>'
        '<a href="../../../firmware/board/main.cpp">Source</a>'
        '<img src="../../../assets/icon.png" alt="Status">'
    )
    current_page = page("en/hardware.md")

    local = pages.on_page_content(sample, page=current_page, config=config())
    public = pages.on_page_content(
        sample, page=current_page, config=config(public=True)
    )

    assert "<code>firmware/board</code>" in local
    assert 'href="../../../' not in local
    assert f"{REPOSITORY_URL}/tree/HEAD/firmware/board" in public
    assert f"{REPOSITORY_URL}/blob/HEAD/firmware/board/main.cpp" in public
    assert "data:image/png;base64," in local
    assert "data:image/png;base64," in public


@pytest.mark.parametrize(
    ("locale", "other", "translated"),
    (
        pytest.param("en", "ru", "../ru/hardware/", id="en-to-ru"),
        pytest.param("ru", "en", "../../hardware/", id="ru-to-en"),
    ),
)
def test_manual_guide_links_stay_on_site_and_keep_the_section(
    repository: Path, locale: str, other: str, translated: str
) -> None:
    write(repository, f"site/guide/{locale}/firmware.md")
    write(repository, f"site/guide/{other}/hardware.md")
    sample = (
        '<a href="firmware.md#commands">Firmware</a>'
        f'<a href="../{other}/hardware.md#wiring">Language</a>'
        '<h2 id="camera-2026"></h2><a href="#camera--2026">Camera</a>'
    )

    result = pages.on_page_content(
        sample, page=page(f"{locale}/hardware.md"), config=config(public=True)
    )

    assert 'href="../firmware/#commands"' in result
    assert f'href="{translated}#wiring"' in result
    assert 'href="#camera-2026"' in result


def test_remote_manual_and_printing_links_resolve_to_translated_site_pages() -> None:
    source = "".join(
        f'<a href="{REPOSITORY_URL}/blob/HEAD/{path}">Guide</a>'
        for path in (
            "site/guide/ru/hardware.md#usb-assisted-setup",
            "models/magnetic/GUIDE_ROUND3X2.en.md#fit",
            "models/magnetic/GUIDE_ROUND3X2.md",
        )
    )

    result = pages.on_page_content(
        source, page=page("en/hardware.md"), config=config(public=True)
    )

    for target in (
        "../ru/hardware/#usb-assisted-setup",
        "../printing/#fit",
        "../ru/printing/",
    ):
        assert f'href="{target}"' in result


@pytest.mark.parametrize(
    ("url", "prefix"),
    (
        pytest.param("", "../", id="index"),
        pytest.param("installation/", "../../", id="en-nested"),
        pytest.param("ru/hardware/", "../../../", id="ru-nested"),
    ),
)
def test_repository_photos_are_served_from_site_assets_at_each_depth(
    repository: Path, url: str, prefix: str
) -> None:
    write(repository, "site/assets/photos/lamp.jpg")
    source = f'<img src="{REPOSITORY_URL}/raw/HEAD/site/assets/photos/lamp.jpg">'

    result = pages.on_page_content(
        source, page=page("en/installation.md", url), config=config(public=True)
    )

    assert f'src="{prefix}assets/photos/lamp.jpg"' in result


def test_included_heading_anchors_reach_toc_without_rewriting_code_examples(
    tmp_path: Path,
) -> None:
    write(
        tmp_path,
        "section.md",
        '<a name="stable-section"></a>\n\n## Translated heading\n\n'
        '```html\n<a name="code-example"></a>\n\n## Literal example\n```\n',
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
            "pymdownx.snippets": {"base_path": [str(tmp_path)], "check_paths": True}
        },
    )

    rendered = renderer.convert('--8<-- "section.md"')

    assert renderer.toc_tokens[0]["id"] == "stable-section"
    assert rendered.count('id="stable-section"') == 1
    assert '<a name="code-example"></a>\n\n## Literal example' in unescape(rendered)


def test_repository_url_expands_in_prompts_and_prose() -> None:
    renderer = markdown.Markdown(
        extensions=["pymdownx.superfences", pages.SectionAnchors(REPOSITORY_URL)]
    )

    result = renderer.convert(
        "```text\nRead {{ repo_url }}/README.md\n```\n\n{{ repo_url }}"
    )

    assert "{{ repo_url }}" not in result
    assert result.count(REPOSITORY_URL) == 2


def test_not_found_page_uses_absolute_links_under_repository_prefix() -> None:
    home = "https://example.github.io/lamp/"

    rendered = pages.not_found_page(home)

    for path in ("", "docs/", "docs/ru/"):
        assert f'href="{home}{path}"' in rendered
