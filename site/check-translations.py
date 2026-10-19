"""Check EN/RU coverage and stable section links for authored project docs."""

from pathlib import Path
import re

import markdown

from pages import REFERENCE_SOURCES, SectionAnchors

ROOT = Path(__file__).resolve().parents[1]
SITE = ROOT / "site"


def headings(text):
    document = markdown.Markdown(
        extensions=[
            "meta",
            "fenced_code",
            "tables",
            "attr_list",
            "toc",
            SectionAnchors(),
        ]
    )
    document.convert(text)

    def flatten(tokens):
        return [(token["level"], token["id"]) for token in tokens] + [
            item for token in tokens for item in flatten(token["children"])
        ]

    return sorted(flatten(document.toc_tokens))


def check_pair(english, russian):
    assert (
        english.is_file() and russian.is_file()
    ), f"Missing translation: {english} / {russian}"
    en, ru = english.read_text(), russian.read_text()
    assert re.search(r"[А-Яа-яЁё]{3}", ru), f"No Russian content: {russian}"
    assert (
        '<div lang="en"' not in ru
    ), f"English fallback in Russian document: {russian}"
    assert headings(en) == headings(ru), f"Section IDs differ: {english} / {russian}"


pairs = [
    (ROOT / name, ROOT / Path(name).with_suffix(".ru.md"))
    for name in (
        "README.md",
        "CONTRIBUTING.md",
        "AGENTS.md",
        "CLAUDE.md",
        "site/README.md",
        "site/vendor/README.md",
    )
]
pairs.append(
    (
        ROOT / "models/magnetic/GUIDE_ROUND3X2.en.md",
        ROOT / "models/magnetic/GUIDE_ROUND3X2.md",
    )
)

en_pages = {path.name for path in (SITE / "guide/en").glob("*.md")}
ru_pages = {path.name for path in (SITE / "guide/ru").glob("*.md")}
assert en_pages == ru_pages, f"Missing website translations: {en_pages ^ ru_pages}"
pairs += [
    (SITE / "guide/en" / name, SITE / "guide/ru" / name) for name in sorted(en_pages)
]
for english, russian in pairs:
    check_pair(english, russian)

for name, sources in REFERENCE_SOURCES.items():
    for locale, source in zip(("en", "ru"), sources):
        wrapper = (SITE / "guide" / locale / name).read_text()
        assert (
            f'--8<-- "{source}"' in wrapper
        ), f"Wrong included language: {locale}/{name}"
        assert (ROOT / source).is_file(), source

for locale in ("en", "ru"):
    manual = (SITE / "guide" / locale / "hardware.md").read_text()
    for section in ("lamp-wiring", "path-check", "path-setup"):
        assert f"start:{section}" in manual and f"end:{section}" in manual
        page = "index.md" if section == "lamp-wiring" else "alternatives.md"
        assert (
            f'--8<-- "site/guide/{locale}/hardware.md:{section}"'
            in (SITE / "guide" / locale / page).read_text()
        )

print(
    f"PASS: {len(pairs)} EN/RU document pairs, matching section IDs and localized includes"
)
