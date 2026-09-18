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
    """Offer complete Mac bundles, with a source route before the first release."""
    ru = page.file.src_uri.startswith("ru/")
    if config.repo_url:
        releases = config.repo_url + "/releases"
        source = config.repo_url + "/archive/HEAD.zip"
        mac = (f"[Приложение для Mac + прошивка C3 · релизы]({releases})"
               if ru else f"[Mac app + C3 firmware · releases]({releases})")
        mac += ("\n\nВыберите полный пакет для своего Mac: `clawlight-vVERSION-aarch64-apple-darwin.tar.gz` "
                "для Apple Silicon или `clawlight-vVERSION-x86_64-apple-darwin.tar.gz` для Intel, "
                "и одноимённый файл `.sha256`. Архивы `clawlight-cli-…` не содержат прошивку и руководство. "
                f"Если полного пакета ещё нет, [скачайте исходники]({source}) и соберите по инструкции ниже."
                if ru else "\n\nChoose the complete bundle for your Mac: `clawlight-vVERSION-aarch64-apple-darwin.tar.gz` "
                "for Apple Silicon or `clawlight-vVERSION-x86_64-apple-darwin.tar.gz` for Intel, "
                "plus its matching `.sha256` file. `clawlight-cli-…` archives omit the firmware and manual. "
                f"If no complete package is available, [download the source]({source}) and build using the guide below.")
        guide = config.repo_url + "/blob/HEAD/models/clawlight/PRINT_GUIDE.md"
        models = (f"[Комплект для печати корпуса · релизы]({releases})\n\n"
                  "Выберите `clawlight-enclosure-vVERSION.zip` и одноимённый файл `.sha256`. "
                  "Это отдельный комплект с CAD, STL и руководством; он не входит в пакет приложения. "
                  f"[Инструкция по печати (RU)]({guide}). Если релиз ещё не опубликован, "
                  f"[скачайте исходники с CAD и STL]({source})."
                  if ru else f"[Enclosure print package · releases]({releases})\n\n"
                  "Choose `clawlight-enclosure-vVERSION.zip` and its matching `.sha256` file. "
                  "This separate package contains CAD, STL and a print guide; it is not part of the app bundle. "
                  f"[Print guide (RU)]({guide}). Before the release is published, "
                  f"[download the source with CAD and STL]({source}).")

    else:
        packages = [path for path in (SITE.parent / "dist").glob("clawlight-*-apple-darwin*.tar.gz")
                    if path.with_name(path.name + ".sha256").is_file()]
        if packages:
            package = max(packages, key=lambda path: path.stat().st_mtime_ns)
            prefix = "../../../../" if ru else "../../../"
            label = "Локальный пакет Mac + C3" if ru else "Local Mac + C3 package"
            checksum = "Контрольная сумма" if ru else "Checksum"
            mac = f"[{label}]({prefix}dist/{package.name}){{ download }}\n\n[{checksum}]({prefix}dist/{package.name}.sha256){{ download }}"
            mac += ("\n\nПроверьте архитектуру и дату сборки в `BUILD.json` внутри пакета."
                    if ru else "\n\nCheck the architecture and build date in the package's `BUILD.json`.")
        else:
            mac = ("Готового пакета в этом checkout нет. [Соберите из исходников](hardware.md#build-from-source) "
                   "или попросите владельца проекта предоставить полный пакет Mac + C3 и контрольную сумму."
                   if ru else "This checkout has no prebuilt package. [Build from source](hardware.md#build-from-source) "
                   "or obtain a complete Mac + C3 bundle and checksum from the project owner.")
        print_package = SITE.parent / "models/clawlight-print-pack.zip"
        if print_package.is_file():
            prefix = "../../../" if ru else "../../"
            label = "Комплект для печати корпуса и инструкция" if ru else "Enclosure print package and guide"
            models = f"[{label}]({prefix}models/{print_package.name}){{ download }}"
            models += ("\n\nРаспакуйте архив и откройте `START_HERE.md`, затем `models/clawlight/PRINT_GUIDE.md` (RU). "
                       "В комплекте — CAD, STL и настройки печати; приложение и прошивка скачиваются отдельно."
                       if ru else "\n\nExtract the archive and open `START_HERE.md`, then `models/clawlight/PRINT_GUIDE.md` (RU). "
                       "The package includes CAD, STL and print settings; download the app and firmware separately.")
        else:
            models = ("В этом checkout нет архива корпуса. Откройте `models/clawlight/PRINT_GUIDE.md` "
                      "в полных исходниках проекта или получите комплект корпуса из релиза проекта."
                      if ru else "This checkout has no enclosure archive. Open `models/clawlight/PRINT_GUIDE.md` "
                      "in the full project source or obtain the enclosure package from the project's release.")
    models = ("**Корпус · релиз-кандидат 1.0.0, готов к выпуску.** Финальная физическая проверка сборки продолжается.\n\n"
              if ru else "**Enclosure · 1.0.0 release candidate, ready for release.** Final physical assembly testing is in progress.\n\n") + models
    return markdown.replace("<!-- mac-downloads -->", mac).replace("<!-- model-download -->", models)


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
    for name in ("index.html", "styles.css", "i18n.js", "model-preview.js"):
        shutil.copy2(SITE / name, output / name)
    shutil.copytree(SITE / "assets", output / "assets")
    shutil.copytree(SITE / "vendor", output / "vendor")
    (output / ".nojekyll").touch()
    print(f"Pages artifact: {output}")


if __name__ == "__main__":
    main()
