"""Compare the public R and Python APIs: functions and their arguments.

The R and Python packages are meant to share one API (the same functions,
the same argument names). This lists where they differ. Until the 0.3.0
contract (docs/api.md) is written, it only reports (implementation plan,
phase 0); from phase 5 it fails on any difference not listed as intended.

Usage:  python tools/api_parity.py [--python PATH] [--strict]
  --python  the interpreter with basemapper installed (default: this one)
  --strict  exit 1 when the APIs differ
"""

import argparse
import json
import pathlib
import subprocess
import sys

# R name -> Python name, for functions both packages are meant to have.
PAIRS = {
    "basemap": "basemap",
    "style": "style",
    "provider": "provider",
    "tiles": "tiles",
    "wms": "wms",
    "wmts": "wmts",
    "around": "around",
    "outside": "outside",
    "geom_basemap": "geom_basemap",
    "list_layers": "list_layers",
    "get_attribution": "get_attribution",
    "list_providers": "list_providers",
    "tile_cache": "tile_cache",
    "fetch_options": "FetchOptions",
}
# Arguments that are each language's own spelling of the same thing.
LANGUAGE_ONLY = {"...", "m", "self", "x", "ax", "deprecated", "kwargs", "removed", "basemap_args"}

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
for n in basemapper.__all__:
    o = getattr(basemapper, n, None)
    if o is None:
        continue
    target = o.__init__ if inspect.isclass(o) and not issubclass(o, Exception) else o
    if inspect.isclass(o) and issubclass(o, Exception):
        continue
    try:
        out[n] = list(inspect.signature(target).parameters)
    except (TypeError, ValueError):
        pass
print(json.dumps(out))
"""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()

    # Run R inside r-basemapper/, so its renv library (the development build) is used.
    r_dir = pathlib.Path(__file__).resolve().parent.parent / "r-basemapper"
    r_out = subprocess.run(["Rscript", "-e", R_SCRIPT], capture_output=True, text=True, check=True, cwd=r_dir)
    r = json.loads(r_out.stdout.strip().splitlines()[-1])
    python = str(pathlib.Path(args.python).resolve())  # Windows does not resolve relative paths to executables
    py = json.loads(subprocess.run([python, "-c", PY_SCRIPT], capture_output=True, text=True, check=True).stdout)

    differences = 0
    print(f"{'function (R / Python)':38} differences")
    for rn, pn in PAIRS.items():
        ra, pa = r.get(rn), py.get(pn)
        label = f"{rn} / {pn}"
        if ra is None or pa is None:
            print(f"{label:38} missing in {'R' if ra is None else 'Python'}")
            differences += 1
            continue
        ra_s, pa_s = set(ra) - LANGUAGE_ONLY, set(pa) - LANGUAGE_ONLY
        only_r, only_py = sorted(ra_s - pa_s), sorted(pa_s - ra_s)
        if only_r or only_py:
            differences += 1
            parts = []
            if only_r:
                parts.append("R only: " + ", ".join(only_r))
            if only_py:
                parts.append("Python only: " + ", ".join(only_py))
            print(f"{label:38} " + "; ".join(parts))
        else:
            print(f"{label:38} same arguments")
    unpaired_r = sorted(set(r) - set(PAIRS) - {"providers"})
    unpaired_py = sorted(set(py) - set(PAIRS.values()) - {"__version__", "providers"})
    print(f"\nR only (no Python pair listed): {', '.join(unpaired_r) or 'none'}")
    print(f"Python only (no R pair listed): {', '.join(unpaired_py) or 'none'}")
    print(f"\n{differences} of {len(PAIRS)} paired functions differ")
    return 1 if args.strict and differences else 0


if __name__ == "__main__":
    sys.exit(main())
