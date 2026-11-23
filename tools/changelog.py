"""Write the root CHANGELOG.md from the git history.

Commits follow Conventional Commits (`type(scope): subject`). The changelog
has a section per merge into `main` (newest first) with the commits that
merge brought in (its PR as GitHub records it, via `gh`), a section per run
of commits made on `main` directly,
and an "Unreleased" section for what `dev` has that `main` does not. Within
a section, commits are grouped by type.

Usage: python tools/changelog.py [--main origin/main] [--dev dev]
"""

import argparse
import pathlib
import re
import subprocess
from collections import OrderedDict

ROOT = pathlib.Path(__file__).resolve().parent.parent
REPO = "https://github.com/ar-puuk/basemapper"
TYPES = OrderedDict([
    ("feat", "Features"),
    ("fix", "Fixes"),
    ("perf", "Performance"),
    ("refactor", "Refactoring"),
    ("docs", "Documentation"),
    ("test", "Tests"),
    ("ci", "CI"),
    ("build", "Build"),
    ("chore", "Maintenance"),
    ("other", "Other"),
])
CONVENTIONAL = re.compile(r"^(?P<type>[a-z]+)(?:\((?P<scope>[^)]*)\))?!?:\s*(?P<subject>.+)$")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8").stdout


def commits(rev_range: str, *extra: str) -> list[dict]:
    """Commits in `rev_range`, newest first: hash, date, subject, parents."""
    out = git("log", "--format=%H%x1f%ad%x1f%s%x1f%P", "--date=short", *extra, rev_range)
    rows = []
    for line in out.splitlines():
        h, d, s, p = line.split("\x1f")
        rows.append({"hash": h, "date": d, "subject": s, "parents": p.split()})
    return rows


def entry(c: dict) -> tuple[str, str]:
    """(type, markdown line) for a commit."""
    m = CONVENTIONAL.match(c["subject"])
    kind, scope, subject = ("other", "", c["subject"])
    if m and m["type"] in TYPES:
        kind, scope, subject = m["type"], m["scope"] or "", m["subject"]
    subject = re.sub(r"(?<![\w/])#(\d+)\b", rf"[#\1]({REPO}/pull/\1)", subject)
    prefix = f"**{scope}:** " if scope else ""
    return kind, f"- {prefix}{subject} ([{c['hash'][:7]}]({REPO}/commit/{c['hash']}))"


def section(title: str, items: list[dict], note: str = "") -> str:
    groups: dict[str, list[str]] = OrderedDict((k, []) for k in TYPES)
    for c in items:
        if len(c["parents"]) > 1:
            continue  # merges are the sections themselves
        kind, line = entry(c)
        groups[kind].append(line)
    body = [f"## {title}", ""]
    if note:
        body += [note, ""]
    for kind, lines in groups.items():
        if lines:
            body += [f"### {TYPES[kind]}", "", *lines, ""]
    return "\n".join(body)


def pull_request(sha: str) -> tuple[int, str] | None:
    """The PR a merge commit belongs to, as GitHub records it (needs `gh`)."""
    try:
        out = subprocess.run(
            ["gh", "api", f"repos/ar-puuk/basemapper/commits/{sha}/pulls", "-q", ".[0] | [.number, .title] | @tsv"],
            cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8",
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None
    if not out:
        return None
    num, title = out.split("	", 1)
    return int(num), title


def merge_title(c: dict) -> str:
    """A merge's heading: its PR from GitHub, else its message (a PR number in
    a message GitHub does not know is left unlinked)."""
    pr = pull_request(c["hash"])
    if pr:
        return f"{c['date']}: [#{pr[0]}]({REPO}/pull/{pr[0]}) {pr[1]}"
    m = re.match(r"Merge pull request #\d+ from \S+/(\S+)", c["subject"])
    return f"{c['date']}: merge of {m[1] if m else c['subject']}"


def version_tags() -> dict[str, str]:
    """Commit hash -> version tag (`v0.1.0`), for the tags on GitHub's main."""
    out = git("tag", "--list", "v*", "--format=%(refname:short) %(*objectname) %(objectname)")
    tags = {}
    for line in out.splitlines():
        name, *hashes = line.split()
        tags[hashes[0]] = name  # an annotated tag's commit, else the tag's own
    return tags


def versioned(title: str, items: list[dict], tags: dict[str, str]) -> str:
    """`title` led by the version tag on any of `items`."""
    found = [tags[c["hash"]] for c in items if c["hash"] in tags]
    if not found:
        return title
    v = found[0]
    when, _, what = title.partition(": ")  # "date: what" -> "version (date): what"
    return f"[{v}]({REPO}/releases/tag/{v}) ({when}): {what}"


def run_title(items: list[dict]) -> str:
    """A run of direct commits on main, by its dates."""
    first, last = items[-1]["date"], items[0]["date"]
    span = first if first == last else f"{first} to {last}"
    return f"{span}: commits on `main`"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--main", default="origin/main", help="the released branch (default origin/main)")
    ap.add_argument("--dev", default="dev", help="the development branch (default dev)")
    ap.add_argument("--release", metavar="VERSION",
                    help="the version the unreleased commits will be tagged with when merged (e.g. v0.1.0)")
    args = ap.parse_args()
    tags = version_tags()

    parts = [
        "# Changelog",
        "",
        "Every change to basemapper, from the commit history (written by",
        "`python tools/changelog.py`; do not edit by hand). Each section is a",
        "merge into `main`, newest first, led by its version tag. What changed",
        "for users of each package,",
        "in more words: [R](r-basemapper/NEWS.md) and",
        "[Python](py-basemapper/CHANGELOG.md).",
        "",
    ]

    unreleased = commits(f"{args.main}..{args.dev}", "--no-merges")
    if unreleased:
        if args.release:
            parts.append(section(args.release, unreleased,
                                 f"Tagged {args.release} when merged into `main`; until then on `{args.dev}`."))
        else:
            parts.append(section("Unreleased", unreleased, f"On `{args.dev}`, not yet merged into `main`."))

    # Walk main's first-parent line: a merge is a section of what it brought
    # in; runs of direct commits are grouped together.
    line = commits(args.main, "--first-parent")
    direct: list[dict] = []

    def flush() -> None:
        if direct:
            parts.append(section(versioned(run_title(direct), direct, tags), direct))
            direct.clear()

    for c in line:
        if len(c["parents"]) > 1:
            flush()
            brought = commits(f"{c['parents'][0]}..{c['parents'][1]}", "--no-merges")
            parts.append(section(versioned(merge_title(c), [c], tags), brought))
        else:
            direct.append(c)
    flush()

    (ROOT / "CHANGELOG.md").write_text("\n".join(parts).rstrip() + "\n", encoding="utf-8", newline="\n")
    print(f"wrote CHANGELOG.md ({len(line)} commits on main's first-parent line, {len(unreleased)} unreleased)")


if __name__ == "__main__":
    main()
