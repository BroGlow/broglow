"""Check landing links locally; optionally verify files served by the preview."""
import argparse
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
        for key in ("href", "src"):
            if key in attrs:
                self.links.append((attrs[key], "download" in attrs))


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--base-url", help="Preview page URL, e.g. http://127.0.0.1:8080/site/")
args = parser.parse_args()
site = Path(__file__).resolve().parent
page = Links()
page.feed((site / "index.html").read_text())
files = {"": site / "index.html"}
external = set()
for href, download in page.links:
    url = urlsplit(href)
    if url.scheme == "data":
        continue
    if url.scheme in ("http", "https"):
        external.add(href)
        continue
    assert not url.scheme and not url.netloc, f"Unsupported link: {href}"
    assert not url.path.endswith(".md"), f"Raw Markdown navigation: {href}"
    if not url.path:
        assert not url.fragment or unquote(url.fragment) in page.ids, f"Missing anchor: {href}"
        continue
    target = (site / unquote(url.path)).resolve()
    assert target.is_relative_to(site.parent), f"Link leaves the project: {href}"
    assert target.is_file(), f"Missing file: {href}"
    if target.name.endswith((".zip", ".tar.gz", ".sha256")):
        assert download, f"File must download instead of navigating away: {href}"
    files[url.path] = target

if args.base_url:
    for href, target in files.items():
        url = urlsplit(urljoin(args.base_url, href))
        request_url = urlunsplit(url._replace(fragment=""))
        with urlopen(request_url, timeout=15) as response:
            assert response.status == 200, request_url
            assert response.read() == target.read_bytes(), f"Stale or incorrect response: {request_url}"
        print(f"HTTP 200: {href or 'index.html'}")

print(f"PASS: {len(page.links)} references, {len(files)} local files, valid anchors, no Markdown navigation")
for url in sorted(external):
    print(f"External link (check separately): {url}")
