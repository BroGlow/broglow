"""Behavioral checks for the website's MkDocs hooks (no release network calls)."""

from html import unescape
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import os
import unittest
from unittest.mock import patch

import markdown

import pages


class PagesTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary = self.enterContext(TemporaryDirectory())
        self.root = Path(temporary).resolve()
        self.enterContext(patch.object(pages, "SITE", self.root / "site"))
        self.write("Cargo.toml", '[package]\nversion = "1.2.3"\n')
        self.repository = "https://github.com/example/lamp"

    def write(self, path: str, content: str = "") -> Path:
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)
        return target

    def page(self, source: str = "en/index.md", url: str = "") -> SimpleNamespace:
        return SimpleNamespace(file=SimpleNamespace(src_uri=source), url=url)

    def config(self, public: bool = False, published: bool = False) -> SimpleNamespace:
        return SimpleNamespace(
            repo_url=self.repository if public else None,
            site_url="https://example.github.io/lamp/docs/" if published else None,
        )

    def render_download(self, marker: str, source: str = "en/index.md") -> str:
        return pages.on_page_markdown(
            f"<!-- {marker} -->", page=self.page(source), config=self.config()
        )

    def package(self, path: str, modified: int = 1, complete: bool = True) -> Path:
        archive = self.write(path)
        os.utime(archive, (modified, modified))
        if complete:
            self.write(f"{path}.sha256", f"{'a' * 64}  {archive.name}\n")
        return archive

    def test_local_downloads_require_a_complete_package(self) -> None:
        for marker, path, fallback in (
            (
                "mac-downloads",
                "dist/broglow-v1.0.0-aarch64-apple-darwin.tar.gz",
                "hardware.md#build-from-source",
            ),
            (
                "model-download",
                "dist/enclosure/broglow-models-v1.0.0.zip",
                "printing.md#generate-from-source",
            ),
        ):
            with self.subTest(marker=marker):
                self.package(path, complete=False)

                rendered = self.render_download(marker)

                self.assertIn(fallback, rendered)
                self.assertNotIn("{ download }", rendered)

    def test_local_downloads_select_newest_complete_package_at_each_page_depth(
        self,
    ) -> None:
        for marker, pattern in (
            ("mac-downloads", "dist/broglow-v{}-aarch64-apple-darwin.tar.gz"),
            ("model-download", "dist/enclosure/broglow-models-v{}.zip"),
        ):
            self.package(pattern.format("1.0.0"), modified=1)
            archive = self.package(pattern.format("2.0.0"), modified=2)
            self.package(pattern.format("3.0.0"), modified=3, complete=False)
            for source, prefix in (
                ("en/index.md", "../../"),
                ("en/alternatives.md", "../../../"),
                ("ru/index.md", "../../../"),
                ("ru/printing.md", "../../../../"),
            ):
                with self.subTest(marker=marker, source=source):
                    rendered = self.render_download(marker, source)

                    path = archive.relative_to(self.root).as_posix()
                    self.assertIn(f"]({prefix}{path}){{ download }}", rendered)
                    self.assertIn(f"{'a' * 64}  {archive.name}", rendered)
                    self.assertNotIn(".sha256)", rendered)

    def test_mismatched_checksum_does_not_advertise_a_package(self) -> None:
        archive = self.package("dist/enclosure/broglow-models-v1.0.0.zip")
        archive.with_name(archive.name + ".sha256").write_text(
            f"{'a' * 64}  another-package.zip\n"
        )

        with self.assertRaisesRegex(ValueError, "Not a sha256sum line"):
            self.render_download("model-download")

    def test_public_preview_routes_to_releases_without_fetching_checksums(self) -> None:
        with patch.object(pages, "release_checksum") as fetch:
            for locale in ("en", "ru"):
                rendered = pages.on_page_markdown(
                    "<!-- mac-downloads -->\n<!-- model-download -->",
                    page=self.page(f"{locale}/index.md"),
                    config=self.config(public=True),
                )

                self.assertIn(f"]({self.repository}/releases)", rendered)
                self.assertIn(f"]({self.repository}/archive/HEAD.zip)", rendered)
                for archive in (
                    "broglow-v1.2.3-aarch64-apple-darwin.tar.gz",
                    "broglow-v1.2.3-x86_64-apple-darwin.tar.gz",
                    "broglow-models-v1.2.3.zip",
                ):
                    self.assertIn(archive, rendered)
                self.assertNotIn("/dist/", rendered)
            fetch.assert_not_called()

    def test_published_downloads_show_release_checksums_inline(self) -> None:
        checksum = f"{'b' * 64}  broglow-models-v1.2.3.zip"
        with patch.object(pages, "release_checksum", return_value=checksum):
            rendered = pages.on_page_markdown(
                "<!-- model-download -->",
                page=self.page(),
                config=self.config(public=True, published=True),
            )

        self.assertIn(f"```text\n{checksum}\n```", rendered)
        self.assertNotIn(".sha256)", rendered)

    def test_pages_without_downloads_do_not_fetch_release_metadata(self) -> None:
        source = "Ordinary guide content"
        with patch.object(pages, "release_checksum") as fetch:
            result = pages.on_page_markdown(
                source,
                page=self.page(),
                config=self.config(public=True, published=True),
            )

        self.assertEqual(result, source)
        fetch.assert_not_called()

    def test_checksum_parser_rejects_wrong_archive_and_error_pages(self) -> None:
        line = f"{'a' * 64}  model.zip"
        self.assertEqual(pages.checksum_line(line + "\n", "model.zip"), line)
        for text in (line.replace("model.zip", "other.zip"), "<html>Not found</html>"):
            with self.subTest(text=text):
                self.assertIsNone(pages.checksum_line(text, "model.zip"))

    def test_manual_links_reject_missing_and_outside_repository_targets(self) -> None:
        for target in ("missing.md", "../../../outside.stp"):
            for public in (False, True):
                with self.subTest(target=target, public=public):
                    with self.assertRaisesRegex(
                        ValueError, "Missing or outside-repository manual reference"
                    ):
                        pages.on_page_content(
                            f'<a href="{target}">Broken</a>',
                            page=self.page("en/printing.md"),
                            config=self.config(public=public),
                        )

    def test_manual_repository_files_and_icons_work_in_local_and_public_guides(
        self,
    ) -> None:
        self.write("firmware/board/main.cpp")
        self.write("assets/icon.png", "image fixture")
        sample = (
            '<a href="../../../firmware/board/">Firmware</a>'
            '<a href="../../../firmware/board/main.cpp">Source</a>'
            '<img src="../../../assets/icon.png" alt="Status">'
        )
        page = self.page("en/hardware.md")

        local = pages.on_page_content(sample, page=page, config=self.config())
        public = pages.on_page_content(
            sample, page=page, config=self.config(public=True)
        )

        self.assertIn("<code>firmware/board</code>", local)
        self.assertNotIn('href="../../../', local)
        self.assertIn(f"{self.repository}/tree/HEAD/firmware/board", public)
        self.assertIn(f"{self.repository}/blob/HEAD/firmware/board/main.cpp", public)
        self.assertIn("data:image/png;base64,", local)
        self.assertIn("data:image/png;base64,", public)

    def test_manual_guide_links_stay_on_site_and_keep_the_section(self) -> None:
        for locale, other, translated in (
            ("en", "ru", "../ru/hardware/"),
            ("ru", "en", "../../hardware/"),
        ):
            self.write(f"site/guide/{locale}/firmware.md")
            self.write(f"site/guide/{other}/hardware.md")
            sample = (
                '<a href="firmware.md#commands">Firmware</a>'
                f'<a href="../{other}/hardware.md#wiring">Language</a>'
                '<h2 id="camera-2026"></h2><a href="#camera--2026">Camera</a>'
            )
            with self.subTest(locale=locale):
                result = pages.on_page_content(
                    sample,
                    page=self.page(f"{locale}/hardware.md"),
                    config=self.config(public=True),
                )

                self.assertIn('href="../firmware/#commands"', result)
                self.assertIn(f'href="{translated}#wiring"', result)
                self.assertIn('href="#camera-2026"', result)

    def test_remote_manual_and_printing_links_resolve_to_translated_site_pages(
        self,
    ) -> None:
        source = "".join(
            f'<a href="{self.repository}/blob/HEAD/{path}">Guide</a>'
            for path in (
                "site/guide/ru/hardware.md#usb-assisted-setup",
                "models/magnetic/GUIDE_ROUND3X2.en.md#fit",
                "models/magnetic/GUIDE_ROUND3X2.md",
            )
        )

        result = pages.on_page_content(
            source, page=self.page("en/hardware.md"), config=self.config(public=True)
        )

        for target in (
            "../ru/hardware/#usb-assisted-setup",
            "../printing/#fit",
            "../ru/printing/",
        ):
            self.assertIn(f'href="{target}"', result)

    def test_repository_photos_are_served_from_site_assets_at_each_depth(self) -> None:
        self.write("site/assets/photos/lamp.jpg")
        source = f'<img src="{self.repository}/raw/HEAD/site/assets/photos/lamp.jpg">'
        for url, prefix in (
            ("", "../"),
            ("installation/", "../../"),
            ("ru/hardware/", "../../../"),
        ):
            with self.subTest(url=url):
                result = pages.on_page_content(
                    source,
                    page=self.page("en/installation.md", url),
                    config=self.config(public=True),
                )

                self.assertIn(f'src="{prefix}assets/photos/lamp.jpg"', result)

    def test_included_heading_anchors_reach_toc_without_rewriting_code_examples(
        self,
    ) -> None:
        self.write(
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
                "pymdownx.snippets": {
                    "base_path": [str(self.root)],
                    "check_paths": True,
                }
            },
        )

        rendered = renderer.convert('--8<-- "section.md"')

        self.assertEqual(renderer.toc_tokens[0]["id"], "stable-section")
        self.assertEqual(rendered.count('id="stable-section"'), 1)
        self.assertIn(
            '<a name="code-example"></a>\n\n## Literal example', unescape(rendered)
        )

    def test_repository_url_expands_in_prompts_and_prose(self) -> None:
        renderer = markdown.Markdown(
            extensions=["pymdownx.superfences", pages.SectionAnchors(self.repository)]
        )

        result = renderer.convert(
            "```text\nRead {{ repo_url }}/README.md\n```\n\n{{ repo_url }}"
        )

        self.assertNotIn("{{ repo_url }}", result)
        self.assertEqual(result.count(self.repository), 2)

    def test_not_found_page_uses_absolute_links_under_repository_prefix(self) -> None:
        home = "https://example.github.io/lamp/"

        rendered = pages.not_found_page(home)

        for path in ("", "docs/", "docs/ru/"):
            self.assertIn(f'href="{home}{path}"', rendered)


if __name__ == "__main__":
    unittest.main()
