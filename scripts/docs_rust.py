"""Build the Rust documentation into site/rust (run by scripts/build-docs.ps1).

Runs the guide examples (core/examples/guide_*.rs) in site/rust/img, so the
images they write are the ones the guide shows, saves what each prints to
core/guide/output/<name>.txt (included in the crate docs), then builds the
crate docs with rustdoc and copies them to site/rust.

Usage: python scripts/docs_rust.py [site_dir]
"""

import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "core"


def main(site: pathlib.Path) -> None:
    rust = site / "rust"
    if rust.exists():
        shutil.rmtree(rust)
    img = rust / "img"
    img.mkdir(parents=True)

    manifest = str(CORE / "Cargo.toml")
    subprocess.run(["cargo", "build", "--release", "--examples", "--manifest-path", manifest], check=True)
    out_dir = CORE / "guide" / "output"
    out_dir.mkdir(parents=True, exist_ok=True)
    for example in sorted((CORE / "examples").glob("guide_*.rs")):
        name = example.stem.removeprefix("guide_")
        print(f"==> example {example.stem}")
        run = subprocess.run(
            ["cargo", "run", "-q", "--release", "--manifest-path", manifest, "--example", example.stem],
            cwd=img, check=True, capture_output=True, text=True, encoding="utf-8",
        )
        (out_dir / f"{name}.txt").write_text(run.stdout, encoding="utf-8", newline="\n")
        print(run.stdout, end="")

    print("==> cargo doc")
    doc = ROOT / "target" / "doc"
    if doc.exists():
        shutil.rmtree(doc)  # only this crate's docs, nothing stale
    subprocess.run(["cargo", "doc", "--no-deps", "-p", "basemapper-core", "--manifest-path", manifest], check=True)
    for entry in doc.iterdir():
        dest = rust / entry.name
        if entry.is_dir():
            shutil.copytree(entry, dest)
        else:
            shutil.copy2(entry, dest)


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "site")
