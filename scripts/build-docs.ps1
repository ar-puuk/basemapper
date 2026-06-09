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
    - Python environment with great-docs CLI on PATH
      (great-docs is installed automatically from basemapper[dev] below)

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
$siteDir    = Join-Path $repoRoot 'site'
$siteR      = Join-Path $siteDir  'r'
$sitePy     = Join-Path $siteDir  'python'
$pyPkg      = Join-Path $repoRoot 'py-basemapper'
# great-docs writes Quarto source to great-docs/ then renders HTML into great-docs/_site/
$gdocWork   = Join-Path $pyPkg    'great-docs'
$gdocOut    = Join-Path $gdocWork '_site'
$docsIndex  = Join-Path (Join-Path $repoRoot 'docs') 'index.html'
$siteIndex  = Join-Path $siteDir  'index.html'
$bannerScript = Join-Path (Join-Path $repoRoot 'scripts') 'inject-lang-banner.py'


# ── Clean previous build output ───────────────────────────────────────────────
Write-Host '==> Cleaning old build output ...' -ForegroundColor Cyan
foreach ($dir in @($siteR, $sitePy, $gdocWork)) {
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
}

# ── R: ensure required packages are installed ────────────────────────────────
Write-Host '==> Checking R packages ...' -ForegroundColor Cyan
Rscript -e "pkgs <- c('roxygen2','pkgdown','ggplot2','sf','jsonlite','tmap','stars','knitr','rmarkdown'); missing <- pkgs[!sapply(pkgs, requireNamespace, quietly=TRUE)]; if (length(missing)) { message('Installing: ', paste(missing, collapse=', ')); install.packages(missing, repos='https://cran.r-project.org') }"

# ── Python: install package (compiles Rust extension via maturin) ─────────────
# Required so great-docs can do dynamic introspection of the compiled extension.
Write-Host '==> pip install -e .[dev] (compiling Rust extension) ...' -ForegroundColor Cyan
Push-Location $pyPkg
try {
    python -m pip install -e ".[dev]"
} finally {
    Pop-Location
}

# ── R: regenerate man pages ───────────────────────────────────────────────────
Write-Host '==> roxygen2::roxygenize (regenerating man pages) ...' -ForegroundColor Cyan
Rscript -e "roxygen2::roxygenize('r-basemapper')"

# ── R: pkgdown ────────────────────────────────────────────────────────────────
Write-Host '==> pkgdown::build_site -> site/r/ ...' -ForegroundColor Cyan
# Destination is configured via r-basemapper/_pkgdown.yml (destination: ../site/r).
Rscript -e "pkgdown::build_site(pkg='r-basemapper', preview=FALSE, new_process=FALSE)"

# ── Python: great-docs ────────────────────────────────────────────────────────
Write-Host '==> great-docs build -> py-basemapper/great-docs/ ...' -ForegroundColor Cyan
# PYTHONUTF8 forces UTF-8 file I/O on Windows (avoids charmap codec errors from
# Unicode characters in generated Quarto/Markdown files).
Push-Location $pyPkg
try {
    $env:PYTHONUTF8 = '1'
    & great-docs build
} finally {
    $env:PYTHONUTF8 = ''
    Pop-Location
}

if (-not (Test-Path $gdocOut)) {
    Write-Error "great-docs build did not produce output at $gdocOut"
}

# ── Assemble into site/ ───────────────────────────────────────────────────────
Write-Host '==> Copying Python docs -> site/python/ ...' -ForegroundColor Cyan
Copy-Item -Recurse $gdocOut $sitePy

Write-Host '==> Copying landing page -> site/index.html ...' -ForegroundColor Cyan
Copy-Item $docsIndex $siteIndex -Force

# ── Banner injection ──────────────────────────────────────────────────────────
Write-Host '==> Injecting language-switch banners ...' -ForegroundColor Cyan
python $bannerScript $siteDir

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ''
Write-Host 'Build complete. Commit and push to deploy:' -ForegroundColor Green
Write-Host '    git add site/'
Write-Host "    git commit -m 'docs: rebuild site'"
Write-Host '    git push'
