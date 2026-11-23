"""The version of basemapper's packages, kept the same everywhere.

R, Python and the Rust core share one version, which the release tag
(`v0.1.0`) names. It is written in six files; this script reads, checks
and sets them together.

Usage:
  python tools/version.py                 print the version
  python tools/version.py check [--tag T] fail unless every file has the
                                          same version (and it matches tag
                                          T, e.g. v0.1.0, when given)
  python tools/version.py set 0.2.0       write the version to every file,
                                          and the install commands in the
                                          READMEs to the release v0.2.0
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"^\d+\.\d+\.\d+(?:[-.][0-9A-Za-z.]+)?$")

# (file, pattern with the version as group 2): the package's own version,
# not a dependency's.
FILES = [
    ("core/Cargo.toml", r'(\[package\][^\[]*?\nversion\s*=\s*")([^"]+)(")'),
    ("r-basemapper/src/rust/core/Cargo.toml", r'(\[package\][^\[]*?\nversion\s*=\s*")([^"]+)(")'),
    ("r-basemapper/Cargo.toml", r'(\[package\][^\[]*?\nversion\s*=\s*")([^"]+)(")'),
    ("py-basemapper/Cargo.toml", r'(\[package\][^\[]*?\nversion\s*=\s*")([^"]+)(")'),
    ("py-basemapper/pyproject.toml", r'(\[project\][^\[]*?\nversion\s*=\s*")([^"]+)(")'),
    ("r-basemapper/DESCRIPTION", r"(\nVersion:\s*)(\S+)()"),
]


# Pages whose install commands name the release: pak's `@v0.1.0`,
# remotes' `ref = "v0.1.0"`, pip's `@v0.1.0#subdirectory`, Cargo's
# `tag = "v0.1.0"` / `--tag v0.1.0`, and the prose that names it (the
# landing page's code is highlighted HTML, hence the optional span).
READMES = [
    "README.md",
    "r-basemapper/README.md",
    "py-basemapper/README.md",
    "docs/index.html",
    "r-basemapper/vignettes/articles/getting-started.Rmd",
    "py-basemapper/user_guide/01-get-started.qmd",
]
PIN = re.compile(
    r'(@v|(?:ref|tag) = (?:<span class="s">)?"v|--tag v|installs the v|latest release, v'
    r'|releases">v)(\d+\.\d+\.\d+)'
)


def versions() -> dict[str, str]:
    found = {}
    for path, pattern in FILES:
        text = (ROOT / path).read_text(encoding="utf-8")
        m = re.search(pattern, text, re.S)
        if not m:
            sys.exit(f"{path}: no version found")
        found[path] = m.group(2)
    return found


def check(tag: str | None) -> None:
    found = versions()
    distinct = set(found.values())
    if len(distinct) != 1:
        for path, v in found.items():
            print(f"  {v:<12} {path}")
        sys.exit("versions differ; set them together with: python tools/version.py set X.Y.Z")
    version = distinct.pop()
    if tag is not None:
        name = tag.removeprefix("refs/tags/")
        if name != f"v{version}":
            sys.exit(f"tag {name} does not match the packages' version {version} (expected v{version})")
    print(f"basemapper {version}: the same in all {len(found)} files" + (f", and tag {tag}" if tag else ""))


def set_version(version: str) -> None:
    if not SEMVER.match(version):
        sys.exit(f"{version!r} is not a version like 0.2.0")
    for path, pattern in FILES:
        file = ROOT / path
        text = file.read_text(encoding="utf-8")
        new, n = re.subn(pattern, lambda m: m.group(1) + version + m.group(3), text, count=1, flags=re.S)
        if n != 1:
            sys.exit(f"{path}: no version found")
        file.write_bytes(new.encode("utf-8"))
        print(f"  {version:<12} {path}")
    for path in READMES:
        file = ROOT / path
        text = file.read_text(encoding="utf-8")
        new, n = PIN.subn(lambda m: m.group(1) + version, text)
        file.write_bytes(new.encode("utf-8"))
        print(f"  v{version:<11} {path} ({n} install pins)")


def main(argv: list[str]) -> None:
    if not argv:
        print(versions()[FILES[0][0]])
    elif argv[0] == "check":
        tag = argv[argv.index("--tag") + 1] if "--tag" in argv else None
        check(tag)
    elif argv[0] == "set" and len(argv) == 2:
        set_version(argv[1])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
