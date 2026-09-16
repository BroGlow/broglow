"""Build the landing and MkDocs guide as one GitHub Pages artifact."""
import argparse
import re
import shutil
from pathlib import Path

from mkdocs.commands.build import build
from mkdocs.config import load_config

SITE = Path(__file__).resolve().parent


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
    config = load_config(str(SITE / "mkdocs.yml"), strict=True,
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
