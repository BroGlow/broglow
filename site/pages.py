"""Build the landing and MkDocs guide as one GitHub Pages artifact."""

import argparse
import base64
from html import escape, unescape
import mimetypes
import posixpath
import re
import shutil
import tomllib
from functools import cache
from urllib.error import HTTPError
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit
from urllib.request import urlopen

from markdown.extensions import Extension
from markdown.preprocessors import Preprocessor
from mkdocs.commands.build import build
from mkdocs.config import load_config

SITE = Path(__file__).resolve().parent
REFERENCE_SOURCES = {
    "printing.md": (
        "models/magnetic/GUIDE_ROUND3X2.en.md",
        "models/magnetic/GUIDE_ROUND3X2.md",
    ),
}
SELF_CONTAINED_REFERENCES = {
    "hardware.md",
    "firmware.md",
    "releasing.md",
    "media.md",
}
GUIDE_ROUTES = {
    ("" if locale == "en" else "ru/") + path.stem
    for locale in ("en", "ru")
    for path in (SITE / "guide" / locale).glob("*.md")
}


def enclosure_package():
    packages = [
        path
        for path in (SITE.parent / "dist/enclosure").glob(
            "broglow-enclosure-magnetic-v*.zip"
        )
        if path.with_name(path.name + ".sha256").is_file()
    ]
    return max(packages, key=lambda path: path.stat().st_mtime_ns, default=None)


def checksum_line(text, archive):
    """One `sha256sum` line for the archive, or None for anything else."""
    text = text.strip()
    return (
        text if re.fullmatch(rf"[0-9a-f]{{64}}  {re.escape(archive)}", text) else None
    )


@cache
def release_checksum(repo_url, version, archive):
    """The published archive's SHA-256; None before the release exists."""
    url = f"{repo_url}/releases/download/v{version}/{archive}.sha256"
    try:
        with urlopen(url, timeout=10) as response:
            text = response.read().decode()
    except HTTPError as error:
        if error.code == 404:
            return None
        raise
    line = checksum_line(text, archive)
    if line is None:
        raise ValueError(f"Unexpected checksum file at {url}: {text[:80]!r}")
    return line


class SectionAnchorPreprocessor(Preprocessor):
    def run(self, lines):
        pattern = r'(?m)^<a name="([^"\n]+)"></a>\n(?:[ \t]*\n)*(#{1,6} [^\n]+)$'
        return re.sub(
            pattern, lambda match: f"{match[2]} {{ #{match[1]} }}", "\n".join(lines)
        ).split("\n")


class RepoUrlPreprocessor(Preprocessor):
    def __init__(self, md, repo_url):
        super().__init__(md)
        self.repo_url = repo_url

    def run(self, lines):
        return [line.replace("{{ repo_url }}", self.repo_url) for line in lines]


class SectionAnchors(Extension):
    def __init__(self, repo_url=None, **kwargs):
        super().__init__(**kwargs)
        self.repo_url = repo_url

    def extendMarkdown(self, md):
        # After snippets (32) and fenced code (25), before HTML blocks (20).
        md.preprocessors.register(SectionAnchorPreprocessor(md), "section_anchors", 24)
        if self.repo_url:
            # After snippets, before fenced code: reaches included guides and prompts.
            md.preprocessors.register(
                RepoUrlPreprocessor(md, self.repo_url), "repo_url", 31.5
            )


def cargo_package():
    return tomllib.loads((SITE.parent / "Cargo.toml").read_text())["package"]


def repository_url():
    """The public repository, set once in Cargo.toml."""
    return cargo_package().get("repository", "").rstrip("/")


def on_config(config, **kwargs):
    config.mdx_configs["pymdownx.snippets"]["base_path"] = [str(SITE.parent)]
    config.markdown_extensions.append(
        SectionAnchors((config.repo_url or repository_url()).rstrip("/"))
    )
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
    # The website's language menu preserves the section and saved preference.
    html = re.sub(
        r'<p><a href="[^"]*">English</a> \| <a href="[^"]*">Русский</a></p>\n?',
        "",
        html,
    )

    def source_path(url):
        parsed = urlsplit(unescape(url))
        if not parsed.path or parsed.scheme or parsed.netloc:
            return None
        path = (source.parent / unquote(parsed.path)).resolve()
        # MkDocs has already resolved links whose filenames match a guide page.
        route = posixpath.normpath(posixpath.join(current, unquote(parsed.path)))
        if not path.exists() and route in GUIDE_ROUTES:
            return None
        if not path.is_relative_to(SITE.parent) or not path.exists():
            raise ValueError(f"Missing or outside-repository manual reference: {url}")
        return path

    def link(match):
        url, attributes, label = match.groups()
        # GitHub keeps repeated hyphens in heading anchors; Python-Markdown folds them.
        if url.startswith("#") and f'id="{url[1:]}"' not in html:
            normalized = re.sub(r"-{2,}", "-", url)
            if f'id="{normalized[1:]}"' in html:
                return f'<a href="{normalized}"{attributes}>{label}</a>'
        # Guides shipped outside the site link the published guide remotely; stay on-site here.
        remote = f"{(config.repo_url or repository_url()).rstrip('/')}/blob/HEAD/"
        if unescape(url).startswith(remote):
            relative, _, fragment = unescape(url)[len(remote) :].partition("#")
            target = None
            if relative.startswith("site/guide/") and relative.endswith(".md"):
                target = relative[len("site/guide/") :].removesuffix(".md")
                target = target.removeprefix("en/")
            for route, translations in REFERENCE_SOURCES.items():
                if relative in translations:
                    target = ("ru/" if relative == translations[1] else "") + route
                    target = target.removesuffix(".md")
            if target is not None:
                target = posixpath.relpath(target, current) + "/"
                if fragment:
                    target += "#" + quote(fragment)
                return f'<a href="{escape(target, quote=True)}"{attributes}>{label}</a>'
        path = source_path(url)
        if path is None:
            return match[0]
        relative = path.relative_to(SITE.parent).as_posix()
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
    """Offer the Mac bundles and the enclosure archive from Releases."""
    if (
        "<!-- mac-downloads -->" not in markdown
        and "<!-- model-download -->" not in markdown
    ):
        return markdown
    ru = page.file.src_uri.startswith("ru/")
    # Built pages live at docs/[ru/][PAGE/]index.html; downloads sit in the repository.
    prefix = "../" * (2 + ru + (not page.file.src_uri.endswith("/index.md")))
    if config.repo_url:
        releases = config.repo_url + "/releases"
        source = config.repo_url + "/archive/HEAD.zip"
        version = cargo_package()["version"]

        def published(archive):
            # Only the published site looks checksums up; tests and previews stay offline.
            if not getattr(config, "site_url", None):
                return None
            return release_checksum(config.repo_url, version, archive)

        mac_archives = {
            f"broglow-v{version}-aarch64-apple-darwin.tar.gz": (
                "Apple Silicon",
                published(f"broglow-v{version}-aarch64-apple-darwin.tar.gz"),
            ),
            f"broglow-v{version}-x86_64-apple-darwin.tar.gz": (
                "Intel",
                published(f"broglow-v{version}-x86_64-apple-darwin.tar.gz"),
            ),
        }
        mac = (
            f"[Приложение для Mac + прошивка C3 · релизы]({releases})\n\n"
            "Выберите полный пакет для своего Mac и сверьте его SHA-256:\n\n"
            if ru
            else f"[Mac app + C3 firmware · releases]({releases})\n\n"
            "Choose the complete bundle for your Mac and verify its SHA-256:\n\n"
        )
        for archive, (chip, checksum) in mac_archives.items():
            mac += f"`{archive}` ({chip})\n\n"
            if checksum:
                mac += f"```text\n{checksum}\n```\n\n"
        if not all(checksum for _, checksum in mac_archives.values()):
            mac += (
                "SHA-256 каждого пакета опубликован рядом с ним на странице релиза.\n\n"
                if ru
                else "Each package's SHA-256 is published next to it on the release page.\n\n"
            )
        mac += (
            f"Чтобы собрать приложение самостоятельно, [скачайте исходники]({source}) и соберите их по [инструкции](hardware.md#build-from-source)."
            if ru
            else f"To build the app yourself, [download the source]({source}) and [build from source](hardware.md#build-from-source)."
        )
        guide = "printing.md"
        archive = f"broglow-enclosure-magnetic-v{version}.zip"
        checksum = published(archive)
        if ru:
            models = (
                f"[Файлы корпуса для печати · релизы]({releases})\n\n"
                f"Скачайте `{archive}` из релиза. В архиве нет вложенных папок: пять деталей STL "
                "в ориентации для печати, те же детали в STEP, два вида сборки и `BUILD.json`.\n\n"
            )
            models += (
                f"SHA-256:\n\n```text\n{checksum}\n```\n\n"
                if checksum
                else "SHA-256 архива опубликован рядом с ним на странице релиза.\n\n"
            )
            models += (
                f"Чтобы сгенерировать комплект самостоятельно, [скачайте исходники]({source}) и "
                f"[выполните генерацию]({guide}#generate-from-source). Автоматический "
                "ZIP исходников GitHub не содержит готовых STL и STEP."
            )
        else:
            models = (
                f"[Enclosure print files · releases]({releases})\n\n"
                f"Download `{archive}` from the release. The archive has no subfolders: the five "
                "STL parts in print orientation, the same parts as STEP, two assembly views and `BUILD.json`.\n\n"
            )
            models += (
                f"SHA-256:\n\n```text\n{checksum}\n```\n\n"
                if checksum
                else "The archive's SHA-256 is published next to it on the release page.\n\n"
            )
            models += (
                f"To generate the package yourself, [download the source]({source}) and "
                f"[generate it from source]({guide}#generate-from-source). GitHub's automatic "
                "source ZIP contains no ready-to-print STL or STEP."
            )

    else:
        packages = [
            path
            for path in (SITE.parent / "dist").glob("broglow-*-apple-darwin*.tar.gz")
            if path.with_name(path.name + ".sha256").is_file()
        ]
        if packages:
            package = max(packages, key=lambda path: path.stat().st_mtime_ns)
            label = "Локальный пакет Mac + C3" if ru else "Local Mac + C3 package"
            checksum = checksum_line(
                package.with_name(package.name + ".sha256").read_text(), package.name
            )
            if checksum is None:
                raise ValueError(f"Not a sha256sum line: {package.name}.sha256")
            mac = (
                f"[{label}]({prefix}dist/{package.name}){{ download }}\n\n"
                f"SHA-256:\n\n```text\n{checksum}\n```"
            )
            mac += (
                "\n\nПроверьте архитектуру и дату сборки в `BUILD.json` внутри пакета."
                if ru
                else "\n\nCheck the architecture and build date in the package's `BUILD.json`."
            )
        else:
            mac = (
                "В этой копии исходников нет готового пакета. [Соберите из исходников](hardware.md#build-from-source) "
                "или попросите владельца проекта предоставить полный пакет Mac + C3 и контрольную сумму."
                if ru
                else "This source copy has no prebuilt package. [Build from source](hardware.md#build-from-source) "
                "or obtain a complete Mac + C3 bundle and checksum from the project owner."
            )
        print_package = enclosure_package()
        if print_package:
            label = "Файлы корпуса для печати" if ru else "Enclosure print files"
            checksum = checksum_line(
                print_package.with_name(print_package.name + ".sha256").read_text(),
                print_package.name,
            )
            if checksum is None:
                raise ValueError(f"Not a sha256sum line: {print_package.name}.sha256")
            models = (
                f"[{label}]({prefix}dist/enclosure/{print_package.name}){{ download }}\n\n"
                f"SHA-256:\n\n```text\n{checksum}\n```\n\n"
            )
            models += (
                "В архиве нет вложенных папок: пять деталей STL в ориентации для печати, "
                "те же детали в STEP, два вида сборки и `BUILD.json`."
                if ru
                else "The archive has no subfolders: the five STL parts in print orientation, "
                "the same parts as STEP, two assembly views and `BUILD.json`."
            )
        else:
            models = (
                "В этой копии исходников нет архива корпуса. [Сгенерируйте комплект](printing.md#generate-from-source) "
                "или скачайте его из релиза проекта."
                if ru
                else "This source copy has no enclosure archive. [Generate the package](printing.md#generate-from-source) "
                "or download it from the project's release."
            )
    return markdown.replace("<!-- mac-downloads -->", mac).replace(
        "<!-- model-download -->", models
    )


def not_found_page(home):
    """GitHub Pages serves a root 404.html for every missing path, so links are absolute."""
    home = escape(home, quote=True)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Page not found · BroGlow</title>
<style>
body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; background: #111; color: #eee; font: 18px/1.5 system-ui, sans-serif; text-align: center; }}
a {{ color: #ff8a3d; }}
@media (prefers-color-scheme: light) {{ body {{ background: #f5f3ec; color: #1c1f1c; }} a {{ color: #c25b2a; }} }}
</style>
</head>
<body>
<main>
<h1>Page not found</h1>
<p><a href="{home}">BroGlow home</a> · <a href="{home}docs/">Guide</a></p>
<p lang="ru">Страница не найдена. <a href="{home}">Главная</a> · <a href="{home}docs/ru/">Руководство</a></p>
</main>
</body>
</html>
"""


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
    # Default GitHub Pages address; fixes sitemap/canonical URLs and the 404 page.
    home = "https://{}.github.io/{}/".format(*args.repository.lower().split("/"))
    config = load_config(
        str(SITE / "mkdocs.yaml"),
        strict=True,
        repo_url=f"https://github.com/{args.repository}",
        site_url=home + "docs/",
        site_dir=str(output / "docs"),
    )
    build(config)
    for name in ("index.html", "styles.css", "i18n.js", "theme.js", "model-preview.js"):
        shutil.copy2(SITE / name, output / name)
    shutil.copytree(SITE / "assets", output / "assets")
    shutil.copytree(SITE / "vendor", output / "vendor")
    (output / "404.html").write_text(not_found_page(home))
    (output / ".nojekyll").touch()
    print(f"Pages artifact: {output}")


if __name__ == "__main__":
    main()
