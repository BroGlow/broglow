"""Check links across the site; optionally verify files served by the preview."""
import argparse
import os
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit, urlunsplit
from urllib.request import urlopen


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.links = []

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if "id" in attrs:
            assert attrs["id"] not in self.ids, f"Duplicate id: {attrs['id']}"
            self.ids.add(attrs["id"])
        for key in ("href", "src", "poster", "ios-src", "data-model-src", "data-viewer-src"):
            if key in attrs:
                self.links.append((attrs[key], "download" in attrs))


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--base-url", help="Preview page URL, e.g. http://127.0.0.1:8080/site/")
parser.add_argument("--site-dir", type=Path, help="Check a standalone Pages artifact instead of the local preview")
args = parser.parse_args()
site = args.site_dir.resolve() if args.site_dir else Path(__file__).resolve().parent
boundary = site if args.site_dir else site.parent
assert (site / "docs/index.html").is_file(), "Build the guide first: .venv/bin/mkdocs build --strict -f mkdocs.yaml"
pages = {}
for path in [site / "index.html", *sorted((site / "docs").rglob("*.html"))]:
    page = Links()
    page.feed(path.read_text())
    pages[path] = page
files = set(pages)
external = set()
for path, page in pages.items():
    page_url = path.relative_to(site).as_posix()
    for href, download in page.links:
        url = urlsplit(href)
        if url.scheme == "data":
            continue
        if url.scheme in ("http", "https"):
            external.add(href)
            continue
        assert not url.scheme and not url.netloc, f"Unsupported link: {href}"
        assert not url.path.startswith("/"), f"Root-relative link breaks project Pages: {href}"
        assert not url.path.endswith(".md"), f"Raw Markdown navigation: {href}"
        target = (path.parent / unquote(url.path)).resolve() if url.path else path
        if target.is_dir():
            target /= "index.html"
        assert target.is_relative_to(boundary), f"Link leaves the published tree: {href}"
        assert target.is_file(), f"Missing file from {page_url}: {href}"
        if url.fragment:
            assert target in pages and unquote(url.fragment) in pages[target].ids, f"Missing anchor from {page_url}: {href}"
        if target.name.endswith((".zip", ".tar.gz", ".sha256")):
            assert download, f"File must download instead of navigating away: {href}"
        files.add(target)

if args.base_url:
    for target in sorted(files):
        href = os.path.relpath(target, site)
        url = urlsplit(urljoin(args.base_url, href))
        request_url = urlunsplit(url._replace(fragment=""))
        with urlopen(request_url, timeout=15) as response:
            assert response.status == 200, request_url
            assert response.read() == target.read_bytes(), f"Stale or incorrect response: {request_url}"
        print(f"HTTP 200: {href or 'index.html'}")

print(f"PASS: {len(pages)} pages, {sum(len(page.links) for page in pages.values())} references, valid files and anchors, no Markdown navigation")
for url in sorted(external):
    print(f"External link (check separately): {url}")
