"""Build the landing and MkDocs guide as one GitHub Pages artifact."""

import argparse
import base64
from html import escape, unescape
import json
import mimetypes
import posixpath
import re
import shutil
import tomllib
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit

from markdown.extensions import Extension
from markdown.preprocessors import Preprocessor
from mkdocs.commands.build import build
from mkdocs.config import load_config

SITE = Path(__file__).resolve().parent
REFERENCE_SOURCES = {
    "printing.md": (
        "models/broglow/PRINT_GUIDE.en.md",
        "models/broglow/PRINT_GUIDE.md",
    ),
}
SELF_CONTAINED_REFERENCES = {
    "hardware.md",
    "firmware.md",
    "releasing.md",
    "media.md",
    "enclosure.md",
}
GUIDE_ROUTES = {
    ("" if locale == "en" else "ru/") + path.stem
    for locale in ("en", "ru")
    for path in (SITE / "guide" / locale).glob("*.md")
}


def enclosure_package():
    packages = [
        path
        for path in (SITE.parent / "dist/enclosure").glob("broglow-enclosure-v*.zip")
        if path.with_name(path.name + ".sha256").is_file()
    ]
    return max(packages, key=lambda path: path.stat().st_mtime_ns, default=None)


class SectionAnchorPreprocessor(Preprocessor):
    def run(self, lines):
        pattern = r'(?m)^<a name="([^"\n]+)"></a>\n(?:[ \t]*\n)*(#{1,6} [^\n]+)$'
        return re.sub(
            pattern, lambda match: f"{match[2]} {{ #{match[1]} }}", "\n".join(lines)
        ).split("\n")


class SectionAnchors(Extension):
    def extendMarkdown(self, md):
        # After snippets (32) and fenced code (25), before HTML blocks (20).
        md.preprocessors.register(SectionAnchorPreprocessor(md), "section_anchors", 24)


def on_config(config, **kwargs):
    config.mdx_configs["pymdownx.snippets"]["base_path"] = [str(SITE.parent)]
    config.markdown_extensions.append(SectionAnchors())
    return config


def on_page_content(html, *, page, config, **kwargs):
    """Keep reference-page repository links and icons valid on the website."""
    name = page.file.src_uri.rsplit("/", 1)[-1]
    if name not in REFERENCE_SOURCES and name not in SELF_CONTAINED_REFERENCES:
        return html
    ru = page.file.src_uri.startswith("ru/")
    source = (
        SITE.parent / REFERENCE_SOURCES[name][ru]
        if name in REFERENCE_SOURCES
        else SITE / "guide" / page.file.src_uri
    )
    current = ("ru/" if ru else "") + name.removesuffix(".md")
    model = SITE.parent / "models/broglow"
    generated = {
        model / f"{name}.step"
        for name in ("broglow", "appearance", "exploded", "usb_coupon")
    }
    generated.update(
        model / part["file"]
        for part in json.loads((model / "parts.json").read_text()).values()
    )
    # The website's language menu preserves the section and saved preference.
    html = re.sub(
        r'<p><a href="[^"]*">English</a> \| <a href="[^"]*">Русский</a></p>\n?',
        "",
        html,
    )

    def source_path(url, *, allow_generated=False):
        parsed = urlsplit(unescape(url))
        if not parsed.path or parsed.scheme or parsed.netloc:
            return None
        path = (source.parent / unquote(parsed.path)).resolve()
        # MkDocs has already resolved links whose filenames match a guide page.
        route = posixpath.normpath(posixpath.join(current, unquote(parsed.path)))
        if not path.exists() and route in GUIDE_ROUTES:
            return None
        if not path.is_relative_to(SITE.parent) or not (
            path.exists() or allow_generated and path in generated
        ):
            raise ValueError(f"Missing or outside-repository manual reference: {url}")
        return path

    def link(match):
        url, attributes, label = match.groups()
        # GitHub keeps repeated hyphens in heading anchors; Python-Markdown folds them.
        if url.startswith("#") and f'id="{url[1:]}"' not in html:
            normalized = re.sub(r"-{2,}", "-", url)
            if f'id="{normalized[1:]}"' in html:
                return f'<a href="{normalized}"{attributes}>{label}</a>'
        path = source_path(url, allow_generated=True)
        if path is None:
            return match[0]
        relative = path.relative_to(SITE.parent).as_posix()
        if path in generated:
            if config.repo_url:
                version = tomllib.loads((SITE.parent / "Cargo.toml").read_text())[
                    "package"
                ]["version"]
                target = f"{config.repo_url}/releases/download/v{version}/broglow-enclosure-v{version}.zip"
            elif package := enclosure_package():
                target = posixpath.relpath(package, SITE / "docs" / current)
            else:
                target = (
                    posixpath.relpath(("ru/" if ru else "") + "releasing", current)
                    + "/"
                )
            download = " download" if target.endswith(".zip") else ""
            return f'<a href="{escape(target, quote=True)}"{attributes}{download}>{label}</a> (<code>{escape(relative)}</code>)'
        fragment = urlsplit(unescape(url)).fragment
        for route, translations in REFERENCE_SOURCES.items():
            if relative in translations:
                target = (
                    "ru/" if relative == translations[1] else ""
                ) + route.removesuffix(".md")
                target = posixpath.relpath(target, current) + "/"
                if fragment:
                    target += "#" + quote(fragment)
                return f'<a href="{escape(target, quote=True)}"{attributes}>{label}</a>'
        if path.is_relative_to(SITE / "guide") and path.suffix == ".md":
            target = path.relative_to(SITE / "guide").with_suffix("").as_posix()
            target = target.removeprefix("en/")
            target = posixpath.relpath(target, current) + "/"
            if fragment:
                target += "#" + quote(fragment)
            return f'<a href="{escape(target, quote=True)}"{attributes}>{label}</a>'
        if not config.repo_url:
            return f"{label} (<code>{escape(relative)}</code>)"
        kind = "tree" if path.is_dir() else "blob"
        target = f"{config.repo_url}/{kind}/HEAD/{quote(relative)}"
        if fragment:
            target += "#" + quote(fragment)
        return f'<a href="{escape(target, quote=True)}"{attributes}>{label}</a>'

    def icon(match):
        before, url, after = match.groups()
        path = source_path(url)
        if path is None:
            return match[0]
        mime = mimetypes.guess_type(path.name)[0]
        if mime != "image/png":
            raise ValueError(f"Unsupported manual icon: {path}")
        data = base64.b64encode(path.read_bytes()).decode("ascii")
        return f"{before}data:{mime};base64,{data}{after}"

    html = re.sub(r'<a href="([^"]+)"([^>]*)>(.*?)</a>', link, html, flags=re.DOTALL)
    return re.sub(r'(<img\b[^>]*\bsrc=")([^"]+)("[^>]*>)', icon, html)


def on_page_markdown(markdown, *, page, config, **kwargs):
    """Offer complete Mac bundles, with a source route before the first release."""
    ru = page.file.src_uri.startswith("ru/")
    if config.repo_url:
        releases = config.repo_url + "/releases"
        source = config.repo_url + "/archive/HEAD.zip"
        mac = (
            f"[Приложение для Mac + прошивка C3 · релизы]({releases})"
            if ru
            else f"[Mac app + C3 firmware · releases]({releases})"
        )
        mac += (
            "\n\nВыберите полный пакет для своего Mac: `broglow-vVERSION-aarch64-apple-darwin.tar.gz` "
            "для Apple Silicon или `broglow-vVERSION-x86_64-apple-darwin.tar.gz` для Intel, "
            "и одноимённый файл `.sha256`. Архивы Windows (`broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz`) не содержат прошивку и руководство. "
            f"Если полного пакета ещё нет, [скачайте исходники]({source}) и соберите по инструкции ниже."
            if ru
            else "\n\nChoose the complete bundle for your Mac: `broglow-vVERSION-aarch64-apple-darwin.tar.gz` "
            "for Apple Silicon or `broglow-vVERSION-x86_64-apple-darwin.tar.gz` for Intel, "
            "plus its matching `.sha256` file. Windows archives (`broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz`) omit the firmware and manual. "
            f"If no complete package is available, [download the source]({source}) and build using the guide below."
        )
        guide = "printing.md"
        models = (
            f"[Комплект для печати корпуса · релизы]({releases})\n\n"
            "Выберите `broglow-enclosure-vVERSION.zip` и одноимённый файл `.sha256`. "
            "Это отдельный комплект с исходниками CAD, STL, STEP и руководством; он не входит в пакет приложения. "
            f"[Инструкция по печати]({guide}). Если релиз ещё не опубликован, "
            f"[скачайте исходники]({source}) и сгенерируйте комплект по инструкции. "
            "Автоматический ZIP исходников GitHub не содержит готовых STL, STEP или архивов печати."
            if ru
            else f"[Enclosure print package · releases]({releases})\n\n"
            "Choose `broglow-enclosure-vVERSION.zip` and its matching `.sha256` file. "
            "This separate package contains CAD source, STL, STEP and a print guide; it is not part of the app bundle. "
            f"[Print guide]({guide}). Before the release is published, "
            f"[download the source]({source}) and generate the package using the guide. "
            "GitHub's automatic source ZIP contains no ready-to-print STL, STEP or print archives."
        )

    else:
        packages = [
            path
            for path in (SITE.parent / "dist").glob("broglow-*-apple-darwin*.tar.gz")
            if path.with_name(path.name + ".sha256").is_file()
        ]
        if packages:
            package = max(packages, key=lambda path: path.stat().st_mtime_ns)
            prefix = "../../../../" if ru else "../../../"
            label = "Локальный пакет Mac + C3" if ru else "Local Mac + C3 package"
            checksum = "Контрольная сумма" if ru else "Checksum"
            mac = f"[{label}]({prefix}dist/{package.name}){{ download }}\n\n[{checksum}]({prefix}dist/{package.name}.sha256){{ download }}"
            mac += (
                "\n\nПроверьте архитектуру и дату сборки в `BUILD.json` внутри пакета."
                if ru
                else "\n\nCheck the architecture and build date in the package's `BUILD.json`."
            )
        else:
            mac = (
                "Готового пакета в этом checkout нет. [Соберите из исходников](hardware.md#build-from-source) "
                "или попросите владельца проекта предоставить полный пакет Mac + C3 и контрольную сумму."
                if ru
                else "This checkout has no prebuilt package. [Build from source](hardware.md#build-from-source) "
                "or obtain a complete Mac + C3 bundle and checksum from the project owner."
            )
        print_package = enclosure_package()
        if print_package:
            prefix = "../../../" if ru else "../../"
            label = (
                "Комплект для печати корпуса и инструкция"
                if ru
                else "Enclosure print package and guide"
            )
            checksum = "Контрольная сумма" if ru else "Checksum"
            models = (
                f"[{label}]({prefix}dist/enclosure/{print_package.name}){{ download }}\n\n"
                f"[{checksum}]({prefix}dist/enclosure/{print_package.name}.sha256){{ download }}"
            )
            models += (
                "\n\nРаспакуйте архив и откройте `START_HERE.ru.md`, затем `models/broglow/PRINT_GUIDE.md`. "
                "В комплекте — CAD, STL и настройки печати; приложение и прошивка скачиваются отдельно."
                if ru
                else "\n\nExtract the archive and open `START_HERE.md`, then `models/broglow/PRINT_GUIDE.en.md`. "
                "The package includes CAD, STL and print settings; download the app and firmware separately."
            )
        else:
            models = (
                "В этом checkout нет архива корпуса. Откройте `models/broglow/PRINT_GUIDE.md` "
                "в полных исходниках проекта и [сгенерируйте комплект](releasing.md) "
                "или получите комплект корпуса из релиза проекта."
                if ru
                else "This checkout has no enclosure archive. Open `models/broglow/PRINT_GUIDE.en.md` "
                "in the full project source and [generate the package](releasing.md) "
                "or obtain the enclosure package from the project's release."
            )
        models += (
            "\n\n[Инструкция по печати и сборке](printing.md)."
            if ru
            else "\n\n[Printing and assembly instructions](printing.md)."
        )
    return markdown.replace("<!-- mac-downloads -->", mac).replace(
        "<!-- model-download -->", models
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository", required=True, help="Your GitHub OWNER/REPOSITORY"
    )
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repository):
        parser.error("--repository must be a GitHub OWNER/REPOSITORY")
    output = SITE / "_pages"
    if output.exists():
        shutil.rmtree(output)
    config = load_config(
        str(SITE / "mkdocs.yaml"),
        strict=True,
        repo_url=f"https://github.com/{args.repository}",
        site_dir=str(output / "docs"),
    )
    build(config)
    for name in ("index.html", "styles.css", "i18n.js", "model-preview.js"):
        shutil.copy2(SITE / name, output / name)
    shutil.copytree(SITE / "assets", output / "assets")
    shutil.copytree(SITE / "vendor", output / "vendor")
    (output / ".nojekyll").touch()
    print(f"Pages artifact: {output}")


if __name__ == "__main__":
    main()
