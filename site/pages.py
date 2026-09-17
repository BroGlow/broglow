"""Build the landing and MkDocs guide as one GitHub Pages artifact."""
import argparse
import base64
from html import escape, unescape
import mimetypes
import re
import shutil
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit

from mkdocs.commands.build import build
from mkdocs.config import load_config

SITE = Path(__file__).resolve().parent


def on_config(config, **kwargs):
    config.mdx_configs["pymdownx.snippets"]["base_path"] = [str(SITE.parent)]
    return config


def on_page_content(html, *, page, config, **kwargs):
    """Keep the included manual's repository links and icons valid on the website."""
    if not page.file.src_uri.endswith("/hardware.md"):
        return html

    def source_path(url):
        parsed = urlsplit(unescape(url))
        if not parsed.path or parsed.scheme or parsed.netloc:
            return None
        path = (SITE.parent / "docs" / unquote(parsed.path)).resolve()
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
        path = source_path(url)
        if path is None:
            return match[0]
        relative = path.relative_to(SITE.parent).as_posix()
        if not config.repo_url:
            return f'{label} (<code>{escape(relative)}</code>)'
        kind = "tree" if path.is_dir() else "blob"
        fragment = urlsplit(unescape(url)).fragment
        target = f'{config.repo_url}/{kind}/HEAD/{quote(relative)}'
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
        return f'{before}data:{mime};base64,{data}{after}'

    html = re.sub(r'<a href="([^"]+)"([^>]*)>(.*?)</a>', link, html, flags=re.DOTALL)
    return re.sub(r'(<img\b[^>]*\bsrc=")([^"]+)("[^>]*>)', icon, html)


def on_page_markdown(markdown, *, page, config, **kwargs):
    """Local previews keep local downloads; Pages links to the fork's Releases."""
    if not config.repo_url:
        return markdown
    ru = page.file.src_uri.startswith("ru/")
    releases = config.repo_url + "/releases"

    def download(match):
        label, path = match.groups()
        if path.endswith(".sha256"):
            return ""  # The Releases link includes both archive and checksum.
        if "/models/" in path:
            label = "Модели корпуса · GitHub Releases" if ru else "Enclosure models · GitHub Releases"
            source = config.repo_url + "/archive/HEAD.zip"
            note = (f"Ищите `clawlight-print-pack.zip` в файлах релиза. Модели также есть в папке `models/` [архива исходников]({source})."
                    if ru else f"Look for `clawlight-print-pack.zip` in the release assets. Models are also in the `models/` folder of the [source archive]({source}).")
            return f"[{label}]({releases})\n\n{note}"
        else:
            label = "Пакеты для Mac и контрольные суммы · GitHub Releases" if ru else "Mac packages and checksums · GitHub Releases"
        return f"[{label}]({releases})"

    markdown = re.sub(r"\[([^\]]+)\]\(((?:\.\./)+(?:dist|models)/[^)]+)\)\{ download \}", download, markdown)
    if page.file.src_uri.endswith("installation.md"):
        source = config.repo_url + "/archive/HEAD.zip"
        note = (f"Если BLE-пакет ещё не опубликован, [скачайте исходники]({source}) и попросите агента собрать проект по инструкции ниже."
                if ru else f"If no BLE package is published yet, [download the source]({source}) and ask your agent to build it using the guide below.")
        markdown = markdown.replace("## ", note + "\n\n## ", 1)
    return markdown


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True, help="Your GitHub OWNER/REPOSITORY")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repository):
        parser.error("--repository must be a GitHub OWNER/REPOSITORY")
    output = SITE / "_pages"
    if output.exists():
        shutil.rmtree(output)
    config = load_config(str(SITE / "mkdocs.yaml"), strict=True,
                         repo_url=f"https://github.com/{args.repository}",
                         site_dir=str(output / "docs"))
    build(config)
    for name in ("index.html", "styles.css", "i18n.js"):
        shutil.copy2(SITE / name, output / name)
    shutil.copytree(SITE / "assets", output / "assets")
    (output / ".nojekyll").touch()
    print(f"Pages artifact: {output}")


if __name__ == "__main__":
    main()
