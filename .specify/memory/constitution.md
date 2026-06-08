<!--
SYNC IMPACT REPORT
==================
Version change: (none) → 1.0.0 (initial constitution — populated from blank template)
Modified principles: N/A — all four principles are new
Added sections:
  - I. Cargo Workspace Monorepo (Principle 1)
  - II. Language Boundaries (Principle 2)
  - III. Memory Safety & Concurrency (Principle 3)
  - IV. Headless Output Standard (Principle 4)
  - Dependency Policy
  - Development Workflow
  - Governance
Removed sections: N/A — initial creation
Templates requiring updates:
  - ✅ .specify/templates/plan-template.md — "Constitution Check" placeholder is
       intentionally generic; gates are derived per-feature from this document at
       plan time. No structural update required.
  - ✅ .specify/templates/spec-template.md — No constitution-specific mandatory
       sections added. Template remains valid.
  - ✅ .specify/templates/tasks-template.md — No new principle-driven task types
       require template changes. Template remains valid.
Follow-up TODOs:
  - RATIFICATION_DATE set to 2026-06-08 (today) as the first formal adoption date.
       Update if an earlier governance decision should be recorded.
-->

# basemapper Constitution

## Core Principles

### I. Cargo Workspace Monorepo

The project MUST be structured as a Cargo Workspace containing at minimum:

- `core/` — the pure Rust rendering engine (no FFI, no language-specific code)
- `py-basemapper/` — the Python binding crate (PyO3/Maturin only)
- `r-basemapper/` — the R binding crate (extendr only)

Every crate MUST be independently buildable via `cargo build`. The workspace root
`Cargo.toml` is the single source of truth for all dependency versions. No binding
crate may depend on another binding crate (e.g., the R crate MUST NOT import the
Python crate and vice versa).

**Rationale**: A workspace monorepo keeps the core language-agnostic and bindings
thin. It also enables a single CI pipeline to build and test all targets atomically,
catching integration regressions across all language surfaces at once.

### II. Language Boundaries

- The `core/` crate MUST be 100% pure Rust. It MUST NOT contain `unsafe` blocks
  that call into C/C++, link against GDAL/GEOS/PROJ, or import non-Rust FFI code.
- Python bindings MUST use PyO3 and be compiled/distributed exclusively via
  Maturin. No ctypes, cffi, or manual shared-library linking is permitted.
- R bindings MUST use the extendr framework. No Rcpp or manual `.Call` interfaces
  are permitted.
- No C or C++ source files, no `build.rs` scripts that invoke a C/C++ compiler,
  and no system-level GDAL, GEOS, or PROJ library dependencies are allowed
  anywhere in the workspace.
- A crate with a `-sys` suffix or a `links = "..."` field in its manifest is
  presumed to violate this principle and MUST be explicitly approved via a
  constitution amendment before adoption.

**Rationale**: These constraints eliminate a whole class of portability and
packaging failures. Pure-Rust + PyO3/Maturin + extendr provides a well-defined,
cross-platform build matrix that requires no system GIS libraries from end-users.

### III. Memory Safety & Concurrency

- All async I/O in `core/` MUST use `tokio` as the async runtime and `reqwest`
  as the HTTP client. Blocking I/O on network operations via `std::thread::spawn`
  is not permitted.
- The Python binding layer MUST release the GIL (via `py.allow_threads(|| ...)`)
  when dispatching into async Rust execution to avoid stalling the Python thread.
- The R binding layer MUST NOT block R's main event loop. Long-running async tasks
  MUST be dispatched on a `tokio::Runtime` owned by the binding crate and polled
  independently of R's main thread.
- `unsafe` blocks are permitted only at FFI boundary crossings in the two binding
  crates. Each `unsafe` block MUST carry a `// SAFETY:` comment explaining the
  invariant that makes it sound.

**Rationale**: tokio + reqwest is a well-audited, composable async stack.
Releasing language-level locks during execution prevents deadlocks and keeps
interactive R/Python sessions responsive while tile fetches are in-flight.

### IV. Headless Output Standard

- `core/` MUST NOT produce any interactive UI component, browser window, display
  device output, or file-system-rendered image. Its public API MUST return either:
  - A raw RGBA byte array (`Vec<u8>`) paired with a `SpatialBounds` struct
    (min/max lat-lon or an EPSG-anchored bounding box), or
  - A structured error type.
- Width, height, and pixel density (DPI/scale) MUST be explicit, caller-supplied
  parameters. The engine MUST NOT infer or default to any display resolution.
- Python bindings MUST surface the byte array as a NumPy `ndarray` and attach
  spatial metadata as an ndarray attribute. R bindings MUST return a native `matrix`
  with spatial metadata as an R attribute. Spatial metadata MUST NOT be embedded
  in the pixel data stream.

**Rationale**: Headless rendering makes the engine embeddable in server, batch, and
notebook environments with no display server dependency. Returning raw arrays gives
downstream callers full control over visualization, encoding, and serialization.

## Dependency Policy

- All third-party Rust crate versions MUST be pinned via the committed `Cargo.lock`.
- New Rust dependencies require a brief justification comment in `Cargo.toml`
  (e.g., `# MVT tile decoding — lighter than the image crate for this use case`).
- Pure-Rust crates are strongly preferred. Any `-sys` crate or crate with a
  `links` field is subject to Principle II review before adoption.
- Python-side runtime dependencies MUST be declared only in `pyproject.toml`.
  R-side dependencies MUST be declared only in the binding crate's `DESCRIPTION`.

## Development Workflow

- Every change to `core/` MUST pass `cargo test --workspace` with no unannotated
  skips. Any `#[ignore]` attribute MUST include a reason comment.
- The CI pipeline MUST build and test all three crates: `core` (via `cargo test`),
  the Python wheel (via `maturin develop && pytest`), and the R package
  (via `R CMD check`).
- Pull requests that add a new public API surface to `core/` MUST include a
  corresponding binding stub in both `py-basemapper/` and `r-basemapper/`, even
  if the stub body is `unimplemented!()`. This enforces cross-language parity
  awareness at review time.
- Performance-critical changes to hot paths in `core/` MUST include a benchmark
  (in `benches/`) and reference throughput numbers (tiles/sec, bytes/sec) in the
  PR description.

## Governance

This constitution supersedes all other development practices for the basemapper
project. Amendments require:

1. A pull request updating this file with an appropriate semantic version bump.
2. A Sync Impact Report (HTML comment at the top of this file) listing all affected
   artifacts and their update status.
3. Explicit approval from the project owner before the PR is merged.

**Versioning policy**:

- MAJOR — removal or redefinition of a principle, or a backward-incompatible
  governance change.
- MINOR — new principle, new mandatory section, or material expansion of an
  existing principle.
- PATCH — clarifications, typo fixes, or wording refinements with no semantic
  change.

All feature implementation plans MUST pass the Constitution Check gate defined in
`plan-template.md` before entering the implementation phase. Any intentional
deviation from a principle MUST be documented in the Complexity Tracking table of
the relevant `plan.md` with an explicit justification and a description of the
simpler alternative that was rejected.

**Version**: 1.0.0 | **Ratified**: 2026-06-08 | **Last Amended**: 2026-06-08
