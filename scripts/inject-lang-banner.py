#!/usr/bin/env python3
"""
inject-lang-banner.py  <site-root>

Post-build step: walk every HTML file under <site-root>/r/ and
<site-root>/python/ and inject a plain-text language-switch banner.

No JavaScript — the banner is a <div> with a single <a> tag whose href
is a relative path computed from the file's depth in the tree.  The CSS
lives in a <style> block injected into <head>, so it works even on deeply
nested pages where an external asset path might resolve incorrectly.

Usage (GitHub Actions example):
    python scripts/inject-lang-banner.py site/

To update the base URL for a custom domain: the links are all relative,
so no changes are needed here.  Add a CNAME step to your workflow instead.
"""

import re
import sys
from pathlib import Path

# ── Inline style inserted once into <head> ────────────────────────────────────
BANNER_STYLE = """\
<style>
.bm-banner{
  text-align:center;padding:7px 16px;font-size:.82rem;
  border-bottom:1px solid rgba(0,0,0,.08);line-height:1.8;
}
.bm-banner-r  {background:#e8f0fe;color:#1a3a6b;}
.bm-banner-py {background:#e8f5e9;color:#1a3b1a;}
.bm-banner a  {font-weight:600;text-decoration:underline;}
.bm-banner-r  a{color:#1a3a6b;}
.bm-banner-py a{color:#1a3b1a;}
</style>"""

# ── Banner HTML templates (one placeholder: the relative href) ────────────────
BANNER_R = (
    '<div class="bm-banner bm-banner-r">'
    "R documentation &mdash; "
    '<a href="{href}">Switch to Python docs</a>'
    "</div>"
)

BANNER_PY = (
    '<div class="bm-banner bm-banner-py">'
    "Python documentation &mdash; "
    '<a href="{href}">Switch to R docs</a>'
    "</div>"
)


def relative_target(html_file: Path, site_root: Path, target_dir: str) -> str:
    """Return a relative href from html_file to site_root/target_dir/."""
    depth = len(html_file.relative_to(site_root).parent.parts)
    return "../" * depth + target_dir + "/"


def inject(html: str, style_block: str, banner_html: str) -> str:
    """Insert style into <head> and banner after the opening <body> tag."""
    # Insert style block before </head>
    html = html.replace("</head>", style_block + "\n</head>", 1)
    # Insert banner div immediately after the opening <body ...> tag
    html = re.sub(
        r"(<body[^>]*>)",
        r"\1\n" + banner_html,
        html,
        count=1,
    )
    return html


def process_tree(site_root: Path, lang_dir: str, banner_tpl: str, target_dir: str) -> int:
    tree = site_root / lang_dir
    if not tree.is_dir():
        print(f"  [skip] {tree} does not exist", file=sys.stderr)
        return 0
    count = 0
    for html_file in sorted(tree.rglob("*.html")):
        href = relative_target(html_file, site_root, target_dir)
        banner_html = banner_tpl.format(href=href)
        original = html_file.read_text(encoding="utf-8", errors="replace")
        patched = inject(original, BANNER_STYLE, banner_html)
        if patched != original:
            html_file.write_text(patched, encoding="utf-8")
            count += 1
    return count


def main() -> None:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <site-root>", file=sys.stderr)
        sys.exit(1)

    site_root = Path(sys.argv[1]).resolve()
    if not site_root.is_dir():
        print(f"Error: {site_root} is not a directory", file=sys.stderr)
        sys.exit(1)

    r_count  = process_tree(site_root, "r",      BANNER_R,  "python")
    py_count = process_tree(site_root, "python",  BANNER_PY, "r")

    print(f"Injected banners into {r_count} R pages and {py_count} Python pages.")


if __name__ == "__main__":
    main()
