"""Finish the assembled documentation site (run by scripts/build-docs.ps1).

* Adds the shared R | Python | Rust switcher (docs/assets/site-nav.html) to
  every page of site/r (pkgdown), site/python (great-docs) and site/rust
  (rustdoc), so the three sites share one navigation.
* Writes site/rust/index.html, which opens the crate's documentation.
* Gives the landing page (site/index.html) the same language logos.
* Publishes the Protomaps styles of the PMTiles examples (core/guide/styles)
  at site/styles/, which the guides link to.

Usage: python scripts/docs_postprocess.py [site_dir]
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MARKER = "<!-- basemapper: one language switcher"


def inject_nav(site: pathlib.Path) -> int:
    snippet = (ROOT / "docs" / "assets" / "site-nav.html").read_text(encoding="utf-8")
    assert snippet.startswith(MARKER), "site-nav.html must start with its marker comment"
    count = 0
    for part in ("r", "python", "rust"):
        for page in (site / part).rglob("*.html"):
            html = page.read_text(encoding="utf-8")
            # Replace a switcher added by an earlier run, so it can be updated
            # without rebuilding the sites.
            html = re.sub(re.escape(MARKER) + r".*?</script>\n?", "", html, count=1, flags=re.S)
            if "</head>" not in html:
                continue
            page.write_text(html.replace("</head>", snippet + "</head>", 1), encoding="utf-8", newline="")
            count += 1
    return count


def landing_icons(site: pathlib.Path) -> None:
    snippet = (ROOT / "docs" / "assets" / "site-nav.html").read_text(encoding="utf-8")
    icons = re.search(r"var ICONS = (\{.*?\});", snippet).group(1)
    page = site / "index.html"
    html = page.read_text(encoding="utf-8")
    html = html.replace("<!-- bm-icons: language logos, added by scripts/docs_postprocess.py -->",
                        f"<script>window.BM_ICONS = {icons};</script>", 1)
    page.write_text(html, encoding="utf-8", newline="")


def rust_index(site: pathlib.Path) -> None:
    (site / "rust").mkdir(parents=True, exist_ok=True)
    (site / "rust" / "index.html").write_text(
        '<!DOCTYPE html><meta charset="utf-8"><title>basemapper-core</title>'
        '<meta http-equiv="refresh" content="0; url=basemapper_core/index.html">'
        '<a href="basemapper_core/index.html">basemapper-core documentation</a>\n',
        encoding="utf-8",
    )


def styles(site: pathlib.Path) -> None:
    import shutil
    src = ROOT / "core" / "guide" / "styles"
    dest = site / "styles"
    dest.mkdir(parents=True, exist_ok=True)
    for f in src.iterdir():
        if f.is_file():
            shutil.copy2(f, dest / f.name)


if __name__ == "__main__":
    site = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "site"
    rust_index(site)
    styles(site)
    landing_icons(site)
    print(f"site navigation added to {inject_nav(site)} pages")
