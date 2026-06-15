# basemapper — Code Review Action Plan

> Review feedback assisted by the [critical-code-reviewer skill](https://github.com/posit-dev/skills/blob/main/posit-dev/critical-code-reviewer/SKILL.md).
>
> Audience: implementer (Sonnet). Source of truth for *intent* is the constitution
> (`.specify/memory/constitution.md`) and the spec (`specs/001-spatial-basemap-renderer/`).
> This document lists what to change and why, with `file:line` anchors. Work the
> Blocking items first; nothing downstream can be verified until a clean clone builds.
>
> **Decisions resolved with the project owner (apply these directly — no longer open):**
> - **#6 Python errors:** wire up the typed hierarchy at the FFI boundary.
> - **#7 `add_basemap` return:** keep `xarray.DataArray`; document the deviation.
> - **Spec edits:** authorized — amend `spec.md`, `plan.md`, and `contracts/*.md` to match.
> - **tmap (#tmap suggestion):** keep the `terra`/aux-layer implementation; update the spec.

---

## Summary

The architecture is sound and the hard parts (headless full-scene vector rendering,
the tmap aux-layer integration) are done competently. What fails review is the
connective tissue: dead scaffolding presented as live code, a vector path that drops
half its contract, a build only the original author can run, and three
"Constitution Check ✅ PASS" claims the implementation contradicts.

**Verdict: Request Changes.** Most fixes are deletions and doc corrections; items 1–3
are real correctness/reproducibility problems and should block.

**Priority order:**
1. #1 build reproducibility — nothing else can be verified until a clean clone builds.
2. #2 vector-path parity — correctness + the only `SC-006` gap.
3. #3 anti-meridian guard — stop the panic.
4. #4–#6, #9 cleanup (dead code/errors/exceptions, spec drift) — fast, high signal.
5. #7, #8, #10, #11 contract/constitution reconciliation — decide and document.
6. #12 CI snapshot gate.

---

## Blocking

### 1. The workspace cannot be built from a fresh clone

**Where:** root `Cargo.toml` (`[patch]` section); `core/Cargo.toml:8`.

```toml
[patch."https://github.com/ar-puuk/maplibre-rs"]
maplibre = { path = "../maplibre-rs/maplibre" }
```

The patch points **outside the repository** to a sibling checkout. Constitution
Principle I: *"Every crate MUST be independently buildable via `cargo build`."* A fresh
clone fails until `ar-puuk/maplibre-rs` is separately cloned to exactly `../maplibre-rs`.
This prerequisite is undocumented.

Compounding it: `core/Cargo.toml:8` pins `rev = "3bb172337ad36cc39e87c1b90a42e5fcbd8dc87"`
— **39 hex chars, not 40** (malformed/truncated SHA) — while the latest commit message
says *"bump maplibre-rs rev to 1a930af."* The declared rev, the local `[patch]`, and the
commit log disagree about which maplibre is actually built.

**Fix:**
- Pick one source of truth for the maplibre dependency: either a git submodule vendored
  into the repo, or a correct, full 40-char git `rev` with the `[patch]` removed.
- Fix the malformed SHA.
- Document any prerequisite in `README.md`.
- Confirm a clean clone + `cargo build --workspace` succeeds with no out-of-tree paths.

### 2. Vector-tile render path silently violates four request parameters and `SC-006`

**Where:** `core/src/renderer.rs:340-452` (`render_vector_tiles`); call site `core/src/lib.rs:82`.

- `renderer.rs:410-414` builds its **own** `reqwest::Client` with **no `.timeout()`** →
  a hung vector tile server blocks forever. Violates `SC-006`
  ("error within 10 seconds … rather than hanging indefinitely").
- `renderer.rs:417` fetches tiles in a **sequential `for` loop** (ignores `tile_concurrency`).
- `auth_token` is never applied: the raster path appends `?access_token=` in
  `tile_fetcher.rs:67`, but the vector path calls `expand_url` directly
  (`renderer.rs:418`) — **authenticated Mapbox MVT sources 401**.
- `fail_on_tile_error` is ignored: `renderer.rs:423` always returns a hard `RenderError`
  on fetch failure, inconsistent with `fetch_all_tiles` (`tile_fetcher.rs:143`).

**Fix:** Thread `tile_timeout_ms`, `tile_concurrency`, `auth_token`, and
`fail_on_tile_error` into `render_vector_tiles`. Reuse the `reqwest::Client` already
built in `render` (`lib.rs:28`) rather than constructing a second one. Append the Mapbox
access token for vector sources. Fetch concurrently (mirror `fetch_all_tiles`' semaphore
pattern) and honor fail-soft. Add at least one test for the authenticated + timeout path.

### 3. Anti-meridian bbox → integer underflow

**Where:** `core/src/bbox.rs:92` (`tile_count`); also `core/src/tile_fetcher.rs:40` (`build_tile_coords`).

`(x1 - x0 + 1) * (y1 - y0 + 1)` underflows when a bbox crosses 180° (`x0 > x1`): **debug
panic, release wraps to a huge `u32`**. The `compute_zoom` reduction loop (`lib.rs:54`)
then crashes or returns a misleading `MaxTilesExceeded`. Spec lists anti-meridian as a
required edge case (`spec.md:293`).

**Fix:** Detect `x0 > x1` (or `y0 > y1`) and either reject with a clear
`BasemapError::InvalidBbox` explaining anti-meridian crossing is unsupported, or handle
the wrap. Add a unit test in `core/tests/bbox_unit.rs`.

---

## Required Changes

### 4. ~110 lines of dead wgpu scaffolding

**Where:** `core/src/renderer.rs:6-117` — `WgpuContext`, `initialize_wgpu_headless`,
`create_render_texture`, `read_texture_to_vec`.

Never called anywhere (confirmed by grep across all crates). The raster path is pure-CPU
via the `image` crate (`composite_raster_tiles`); the vector path uses maplibre's own
headless renderer. `read_texture_to_vec:103` also ignores the `map_async` `Result`
callback — a latent bug if anyone revives it.

**Fix:** Delete the block. If the raster path was *intended* to use wgpu, document why it
doesn't and open a separate task — but do not leave unwired scaffolding in `renderer.rs`.

### 5. Dead/contradictory error and `TileSource` variants

**Where:** `core/src/error.rs:23-28`; `core/src/tile_fetcher.rs:16`.

- `VectorRenderNotImplemented`'s message says vector rendering *"is not yet implemented …
  planned for Track B Part 2."* It **is** implemented and the variant is never
  constructed. Delete it.
- `TileSource::EsriVectorTile` is matched/expanded (`lib.rs:83`, `tile_fetcher.rs:74,103`)
  but **never constructed** — `extract_tile_source_from_style` only returns `XyzRaster`
  or `MapboxVectorTile` (`lib.rs:224-231`). Either remove the variant or branch on it.

### 6. Python typed-exception hierarchy is never raised — docstrings claim otherwise

**Where:** `py-basemapper/src/basemapper/exceptions.py`; FFI mapping
`py-basemapper/src/lib.rs:9`; `_render.py:63`; `matplotlib_integration.py:46`.

The Rust FFI maps **every** error to `PyRuntimeError` (built-in `RuntimeError`), not
`basemapper.BasemapError`. `normalize_bbox` raises `ValueError`/`TypeError`
(`bbox_utils.py:124,143,159`). So nothing raises `BasemapError`/`StyleError`/
`NetworkError`, yet docstrings say *"Raises: BasemapError."* `test_render.py:35` uses
`pytest.raises(Exception, …)` to dodge this.

**Fix (DECIDED — wire up the typed hierarchy):**
- Expose an error discriminant from Rust. In `core/src/error.rs`, add a method (e.g.
  `pub fn kind(&self) -> &'static str`) returning a stable category per variant
  (`"validation"`, `"style"`, `"network"`, `"render"`). In `py-basemapper/src/lib.rs`,
  stop collapsing everything to `PyRuntimeError`: construct the matching Python exception
  by importing `basemapper.exceptions` and raising `ValidationError` / `StyleError` /
  `NetworkError` / `BasemapError` based on `kind()`. (Map `InvalidBbox`,
  `InvalidDimensions`, `InvalidLayerFilter`, `MaxTilesExceeded` → `ValidationError`;
  `StyleFetchFailed`, `StyleParseError` → `StyleError`; `TileFetchFailed` → `NetworkError`;
  everything else → `BasemapError`.)
- Make Python-side validation consistent: have `normalize_bbox`/`_validate_bbox_values`
  (`bbox_utils.py`) raise `ValidationError` instead of `ValueError`/`TypeError`
  (keep them subclassing the standard types is fine — `ValidationError(BasemapError)`).
- Tighten tests to assert the real types, e.g. `test_render.py:35` →
  `pytest.raises(StyleError, match="version")`.
- Verify every "Raises:" docstring now names a type the code actually raises.

### 7. `add_basemap` returns `xarray.DataArray`, contradicting Constitution IV / FR-011 / the plan

**Where:** `py-basemapper/src/basemapper/matplotlib_integration.py:93`; claim in `plan.md:102`.

Constitution Principle IV: *"Python bindings MUST surface the byte array as a NumPy
`ndarray` and attach spatial metadata as an ndarray attribute."* The plan's Constitution
Check asserts numpy is used — false. xarray is reasonable (numpy can't carry `.attrs`
cleanly), but this is a **deviation** that must be recorded.

**Fix (DECIDED — keep xarray, document the deviation):**
- Leave `add_basemap` returning `xr.DataArray`; keep `xarray` as a hard dependency.
- Add a row to the plan's Complexity Tracking table (`plan.md:234`, currently empty):
  *Violation* = "Constitution IV: Python returns `xarray.DataArray`, not bare `ndarray`";
  *Why Needed* = "numpy `ndarray` cannot carry named spatial metadata without subclassing;
  xarray gives `.attrs` + labeled dims with no custom array type"; *Simpler Alternative
  Rejected* = "ndarray subclass — fragile across numpy versions and surprising to users."
- Fix the inaccurate Constitution Check claim at `plan.md:102` to state xarray is used and
  reference the Complexity Tracking row.
- (Optional) Note in the `add_basemap` docstring that `.values` yields the underlying
  numpy array for callers who want it.

### 8. `detect_crs_from_axes` swallows the "no CRS" case the spec says must error

**Where:** `py-basemapper/src/basemapper/bbox_utils.py:39-43`.

Falls back to EPSG:4326 with a `warnings.warn` when no projection is found. Spec US1
Acceptance Scenario 3 (`spec.md:43`) requires a **clear error**. A bare matplotlib `Axes`
has no `.projection`, so the common path silently renders a plausible-but-wrong basemap.

**Fix (DECIDED):** Raise `ValidationError` (the typed exception from #6) with a clear
message that a geographic CRS is required, instead of warning + defaulting to 4326. Do
**not** add an opt-in flag — the spec requires a hard error. Update the
`detect_crs_from_axes` docstring (it currently says "or WGS-84 as a fallback") and add a
test asserting `pytest.raises(ValidationError)` for an axes with no detectable CRS.

### 9. `VectorProvider` requires `source_layer` — undocumented divergence from spec/contracts

**Where:** `py-basemapper/src/basemapper/providers.py:69`; `r-basemapper/R/providers.R:85`;
contradicts `spec.md` FR-015, US3 scenario 2, US4 scenario 2, and `contracts/*.md`.

Both bindings add a **required** `source_layer` arg (good: R and Python agree). The spec
examples show `VectorProvider(url, paint=paint)` with no such arg — those examples now
throw `TypeError`. `source_layer` is genuinely needed for MVT, so the **spec is wrong**.

**Fix (DECIDED — spec edits authorized):** Update `spec.md` FR-015 and the US3 scenario 2
/ US4 scenario 2 examples, plus `contracts/python-api.md` and `contracts/r-api.md`, to
include the required `source_layer` argument (`VectorProvider(url, source_layer, paint=...)`
/ `vector_provider(url, source_layer, paint=...)`). No binding code change needed — R and
Python already agree; just confirm parity and that examples in the updated spec run.

### 10. Layer-filtered vector renders aren't transparent — FR-023 / SC-008 half-met

**Where:** `core/src/lib.rs:48` computes `transparent`; vector branch `lib.rs:85` never
passes it to `render_vector_tiles`. Only `composite_raster_tiles` (`renderer.rs:130`)
honors the transparent clear color.

`FR-023` / `SC-008` (transparent background for `layers=Some`) hold for raster and
**silently fail for vector** — the sources where layer isolation matters most.

**Fix:** Pass the `transparent` flag into `render_vector_tiles`; when set, configure the
headless render's clear color to `[0,0,0,0]` and suppress the style's opaque background
layer (or the `BackgroundPlugin` fill). Verify the alpha channel of non-rendered pixels.

### 11. Undocumented Constitution-I deviation: `r-basemapper` is not a workspace member

**Where:** root `Cargo.toml:2` (`members = ["core", "py-basemapper"]`);
`r-basemapper/Cargo.toml:4` (separate workspace); false claim at `plan.md:71`.

Constitution Principle I mandates the workspace contain `r-basemapper/`. The separate
workspace exists to support Option-A vendoring — a defensible call, but a real deviation.

**Fix (DECIDED — spec edits authorized):** Add a Complexity Tracking row (`plan.md:234`):
*Violation* = "Constitution I: `r-basemapper` is a separate workspace, not a root member";
*Why Needed* = "Option-A vendoring lets the R package build from an `install_github()`
tarball with no access to the repo-root workspace"; *Simpler Alternative Rejected* =
"single root workspace including `r-basemapper` — breaks standalone/CRAN tarball builds."
Correct the "three members" claim at `plan.md:71` to describe the two-member root plus the
vendored R workspace. Cross-link to #12 (the drift guardrail).

### 12. Vendored-core divergence has no guardrail

**Where:** `r-basemapper/src/rust/core/` (byte-identical copy of `core/`, verified);
`r-basemapper/src/Makevars:22` (`sync-core`).

`sync-core` `cp`s from `../../core`, which only exists in-repo, so installed/CRAN builds
rely on the committed snapshot and sync only happens on the dev machine. Nothing prevents
silent drift — R users could get a different renderer than Python users.

**Fix:** Add a CI step that fails when `diff -r core/src r-basemapper/src/rust/core/src`
is non-empty (or a committed hash gate). This keeps the documented Option-A approach but
makes drift a build failure instead of a shipped bug.

---

## Suggestions (Strong)

- **JSON parsed up to 4× per render**: `style.rs:49`, `style.rs:88`, `lib.rs:69`,
  `renderer.rs:350`. Parse once into `serde_json::Value` and thread it through.
  `lib.rs:45` also clones the entire style string in the no-filter branch needlessly.
- **`compute_zoom` ignores height/aspect** (`bbox.rs:80`); `composite_raster_tiles`
  applies independent `scale_x`/`scale_y` (`renderer.rs:144-145`), so a bbox whose aspect
  ≠ `width:height` is anisotropically stretched with no warning. Assert/warn on mismatch.
- **tmap (DECIDED — keep `terra`, update spec):** Keep `tmap_integration.R` as-is; do
  **not** rewrite to `stars` + `tm_rgb`. Rewrite spec FR-027 (and SC-009 wording) to
  describe the actual `terra` + deferred aux-layer (`tmapGridAuxPrepare`/`tmapGridAuxPlot`)
  approach. Pin `tmap (>= 4.0, < 5)` in `DESCRIPTION` and move `terra` from Suggests to
  Imports if `tm_basemap` hard-requires it. Add a code comment at `tmap_integration.R:137`
  flagging the documented coupling to tmap private generics as an upgrade-risk surface.
  (`stars` is no longer needed — drop any leftover references in spec/docs.)
- **R FFI clamps zoom instead of erroring**: `r-basemapper/src/rust/src/lib.rs:44`
  (`z.clamp(0, 22)`) silently accepts out-of-range zoom that `core` would reject
  (`bbox.rs:50`). Let core validate; drop the clamp.
- **`.Call("wrap__render_basemap_raw", …)`** (`render_basemap_raw.R:111`) hard-codes the
  extendr-mangled symbol. Brittle; call the generated wrapper instead.
- **`list_layers` (R) returns `sort(ids)` but prints unsorted `df`** (`layer_utils.R:36,38`)
  — returned order ≠ displayed order. Pick one.
- **Thin tests for 30 FRs**: `test_render.py` has 3 tests, one real assertion, and imports
  `responses` unused. No coverage for `SC-002` (pixel-exact dims), the layer filter,
  transparent compositing, or the vector path (the riskiest code). Add targeted tests.

---

## Acceptance checklist for this plan

- [ ] Fresh clone + `cargo build --workspace` succeeds with no out-of-tree paths (#1)
- [ ] Vector path honors timeout, concurrency, auth, fail-soft; authed MVT test passes (#2)
- [ ] Anti-meridian bbox returns a clear error (no panic), with a unit test (#3)
- [ ] Dead wgpu scaffolding removed (#4)
- [ ] `VectorRenderNotImplemented` + unused `EsriVectorTile` resolved (#5)
- [ ] Typed exceptions raised via Rust `kind()` discriminant; docstrings + tests assert real types (#6)
- [ ] xarray kept; Complexity Tracking row added; `plan.md:102` corrected (#7)
- [ ] Missing-CRS path raises `ValidationError` per spec US1.3 (#8)
- [ ] Spec FR-015 + US3/US4 + contracts updated for `source_layer` (#9)
- [ ] Transparent background honored on the vector path; alpha verified (#10)
- [ ] Constitution-I deviation recorded in Complexity Tracking; `plan.md:71` corrected (#11)
- [ ] CI gate fails on `core/` vs vendored-core drift (#12)
- [ ] tmap: spec FR-027 rewritten to `terra`/aux-layer; `tmap`/`terra` pinned in DESCRIPTION
