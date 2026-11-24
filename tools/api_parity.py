"""Compare the R and Python APIs with the contract (docs/api-contract.json).

The contract lists every public function of both packages and its
arguments (docs/api.md describes them). Each package's test suite checks
itself against it; this script reports both side by side, from installed
packages.

Usage:  python tools/api_parity.py [--python PATH] [--strict]
  --python  the interpreter with basemapper installed (default: this one)
  --strict  exit 1 when either package differs from the contract
"""

import argparse
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "docs" / "api-contract.json"

R_SCRIPT = r"""
suppressMessages(library(basemapper))
out <- lapply(setNames(nm = getNamespaceExports("basemapper")), function(f) {
  o <- get(f, envir = asNamespace("basemapper"))
  if (is.function(o)) names(formals(o)) else NULL
})
cat(jsonlite::toJSON(Filter(Negate(is.null), out), auto_unbox = FALSE))
"""

PY_SCRIPT = r"""
import inspect, json, basemapper
out = {}
def args(fn):
    return list(inspect.signature(fn).parameters)
for n in basemapper.__all__:
    o = getattr(basemapper, n, None)
    if o is None:
        continue
    if inspect.isclass(o):
        if issubclass(o, BaseException):
            continue
        out[n] = args(o.__init__)
        for m, v in vars(o).items():
            if callable(v) and not m.startswith("_"):
                out[n + "." + m] = args(v)
    elif callable(o):
        out[n] = args(o)
print(json.dumps(out))
"""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--strict", action="store_true")
    a = ap.parse_args()

    contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
    # Run R inside r-basemapper/, so its renv library (the development build) is used.
    r_out = subprocess.run(["Rscript", "-e", R_SCRIPT], capture_output=True, text=True, check=True,
                           cwd=ROOT / "r-basemapper")
    r = json.loads(r_out.stdout.strip().splitlines()[-1])
    python = str(pathlib.Path(a.python).resolve())  # Windows does not resolve relative paths to executables
    py = json.loads(subprocess.run([python, "-c", PY_SCRIPT], capture_output=True, text=True, check=True).stdout)

    ignore = {lang: set(v) for lang, v in contract["language_only_arguments"].items()}
    problems = 0
    print(f"{'R':18} {'Python':20} {'arguments (the contract)':40} result")
    for f in contract["functions"]:
        want = f["args"]
        cells = []
        for lang, found in (("r", r), ("python", py)):
            name = f.get(lang)
            if not name:
                cells.append("-")
                continue
            if name not in found:
                cells.append("MISSING")
                problems += 1
                continue
            got = sorted(set(found[name]) - ignore[lang])
            expect = sorted(want + f.get(lang + "_also", []))
            if got != expect:
                cells.append(f"differs: {got}")
                problems += 1
            else:
                cells.append("ok")
        result = "ok" if all(c in ("ok", "-") for c in cells) else "; ".join(cells)
        print(f"{f.get('r') or '-':18} {f.get('python') or '-':20} {', '.join(want)[:40]:40} {result}")
        if f.get("why"):
            print(f"{'':39}   ({f['why']})")
    print(f"\n{problems} difference(s) from the contract")
    return 1 if a.strict and problems else 0


if __name__ == "__main__":
    sys.exit(main())
