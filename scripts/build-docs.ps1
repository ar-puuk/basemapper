<#
.SYNOPSIS
    Build the unified basemapper documentation site locally.

.DESCRIPTION
    Runs roxygen2::roxygenize (R man pages), pkgdown (R HTML site), and
    great-docs (Python HTML site), then assembles everything into site/ and
    injects the language-switch banners.

    Prerequisites
    -------------
    - R with packages: roxygen2, pkgdown, ggplot2, sf, jsonlite, tmap, stars,
      knitr, rmarkdown  (install once with: install.packages(c(...)))
    - Rust toolchain + RTools45 (required by roxygenize to compile the R package)
    - Python environment with basemapper[dev] and great-docs installed:
          pip install ".[dev]"   (run from py-basemapper/)
    - great-docs CLI on PATH (installed as part of [dev] extras above)

    After running, commit the updated site/ tree and push to trigger deployment:
        git add site/
        git commit -m "docs: rebuild site"
        git push

.EXAMPLE
    .\scripts\build-docs.ps1
#>
param()

$ErrorActionPreference = 'Stop'

# Resolve repo root from this script's location (scripts/ is one level under root).
$repoRoot = Split-Path $PSScriptRoot
Set-Location $repoRoot

# ── Prerequisites check ───────────────────────────────────────────────────────
foreach ($cmd in @('Rscript', 'python', 'great-docs')) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "Required command not found: '$cmd'. Ensure it is on your PATH and the correct environment is active."
    }
}

# ── Paths ─────────────────────────────────────────────────────────────────────
$siteDir  = Join-Path $repoRoot 'site'
$siteR    = Join-Path $siteDir  'r'
$sitePy   = Join-Path $siteDir  'python'
$gdocOut  = Join-Path $repoRoot 'py-basemapper' 'great-docs'

# pkgdown dest_dir is relative to the package directory (r-basemapper/),
# so '../site/r' resolves to <repo-root>/site/r.  Pass an absolute path
# using forward slashes so the R string literal is unambiguous on Windows.
$siteRFwd = $siteR.Replace('\', '/')

# ── Clean previous build output ───────────────────────────────────────────────
Write-Host '==> Cleaning old build output ...' -ForegroundColor Cyan
foreach ($dir in @($siteR, $sitePy, $gdocOut)) {
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
}

# ── R: regenerate man pages ───────────────────────────────────────────────────
Write-Host '==> roxygen2::roxygenize (regenerating man pages) ...' -ForegroundColor Cyan
Rscript -e "roxygen2::roxygenize('r-basemapper')"

# ── R: pkgdown ────────────────────────────────────────────────────────────────
Write-Host "==> pkgdown::build_site -> site/r/ ..." -ForegroundColor Cyan
Rscript -e @"
pkgdown::build_site(
  pkg         = 'r-basemapper',
  dest_dir    = '$siteRFwd',
  preview     = FALSE,
  new_process = FALSE
)
"@

# ── Python: great-docs ────────────────────────────────────────────────────────
Write-Host '==> great-docs build -> py-basemapper/great-docs/ ...' -ForegroundColor Cyan
Push-Location (Join-Path $repoRoot 'py-basemapper')
try {
    & great-docs build
} finally {
    Pop-Location
}

if (-not (Test-Path $gdocOut)) {
    Write-Error "great-docs build did not produce output at $gdocOut"
}

# ── Assemble into site/ ───────────────────────────────────────────────────────
Write-Host '==> Copying Python docs -> site/python/ ...' -ForegroundColor Cyan
Copy-Item -Recurse $gdocOut $sitePy

Write-Host '==> Copying landing page -> site/index.html ...' -ForegroundColor Cyan
Copy-Item (Join-Path $repoRoot 'docs' 'index.html') (Join-Path $siteDir 'index.html') -Force

# ── Banner injection ──────────────────────────────────────────────────────────
Write-Host '==> Injecting language-switch banners ...' -ForegroundColor Cyan
python (Join-Path $repoRoot 'scripts' 'inject-lang-banner.py') $siteDir

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ''
Write-Host 'Build complete. Commit and push to deploy:' -ForegroundColor Green
Write-Host '    git add site/'
Write-Host "    git commit -m 'docs: rebuild site'"
Write-Host '    git push'
