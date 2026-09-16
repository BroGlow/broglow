"""Small check for local/public download routing in the MkDocs hook."""
from types import SimpleNamespace
from pathlib import Path

from pages import on_page_markdown


for locale in ("en", "ru"):
    for name in ("index.md", "installation.md"):
        source = (Path(__file__).parent / "guide" / locale / name).read_text()
        page = SimpleNamespace(file=SimpleNamespace(src_uri=f"{locale}/{name}"))
        assert on_page_markdown(source, page=page, config=SimpleNamespace(repo_url=None)) == source
        public = on_page_markdown(source, page=page, config=SimpleNamespace(repo_url="https://github.com/example/lamp"))
        assert "https://github.com/example/lamp/releases" in public
        assert "https://github.com/example/lamp/archive/HEAD.zip" in public
        assert "/dist/" not in public and "{ download }" not in public
        assert ("исходников" in public) if locale == "ru" else ("source" in public)

print("PASS: local downloads and public Releases/source fallback in both languages")
