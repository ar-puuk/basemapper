<#
.SYNOPSIS
    Build the unified basemapper documentation site locally.

.DESCRIPTION
    Runs roxygen2::roxygenize (R man pages), pkgdown (R HTML site), and
    great-docs (Python HTML site), then assembles everything into site/ and
    assembles everything into site/.

    Prerequisites
    -------------
    - R with packages: roxygen2, pkgdown, bslib, ggplot2, sf, jsonlite,
      tmap, stars, knitr, rmarkdown  (auto-installed if missing)
    - Rust toolchain + RTools45 (required to compile the R/Python packages)
    - Python with pip (great-docs is installed automatically via .[dev])

    Add Rscript and the Python Scripts folder to your session PATH if needed:
        $env:PATH = "C:\Program Files\R\R-4.6.0\bin;" +
                    "C:\Users\<you>\AppData\Roaming\Python\Python313\Scripts;" +
                    $env:PATH

    After running, commit and push to deploy:
        git add site/
        git commit -m "docs: rebuild site"
        git push

.EXAMPLE
    .\scripts\build-docs.ps1
#>
param()

$ErrorActionPreference = 'Stop'

# Resolve repo root from this script's location (scripts/ sits one level below root).
$repoRoot = Split-Path $PSScriptRoot
Set-Location $repoRoot

# -- Paths --------------------------------------------------------------------
$siteDir      = Join-Path $repoRoot 'site'
$siteR        = Join-Path $siteDir  'r'
$sitePy       = Join-Path $siteDir  'python'
$rPkg         = Join-Path $repoRoot 'r-basemapper'
$pyPkg        = Join-Path $repoRoot 'py-basemapper'
$gdocWork     = Join-Path $pyPkg    'great-docs'
$gdocOut      = Join-Path $gdocWork '_site'
$docsIndex    = Join-Path (Join-Path $repoRoot 'docs') 'index.html'
$siteIndex    = Join-Path $siteDir  'index.html'
$logoSrc      = Join-Path (Join-Path $rPkg 'man') (Join-Path 'figures' 'logo.svg')
$logoDest     = Join-Path $pyPkg 'logo.svg'

# -- Hard prerequisites (Rscript and python must exist before anything else) --
foreach ($cmd in @('Rscript', 'python')) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "Required command not found: '$cmd'. Add it to PATH and retry."
    }
}

# -- Python: install package (compiles Rust extension, installs great-docs) ---
Write-Host '==> pip install -e .[dev] (compiles Rust extension, installs great-docs) ...' -ForegroundColor Cyan
Push-Location $pyPkg
try {
    python -m pip install -e ".[dev]"
} finally {
    Pop-Location
}

# Check great-docs is now available (may need the Scripts folder on PATH).
if (-not (Get-Command 'great-docs' -ErrorAction SilentlyContinue)) {
    Write-Error "great-docs not found on PATH after pip install. Add your Python Scripts folder to PATH (see .DESCRIPTION) and retry."
}

# -- R: ensure required packages are installed --------------------------------
Write-Host '==> Checking R packages ...' -ForegroundColor Cyan
Rscript -e "pkgs <- c('roxygen2','pkgdown','bslib','ggplot2','sf','jsonlite','tmap','stars','knitr','rmarkdown'); missing <- pkgs[!sapply(pkgs, requireNamespace, quietly=TRUE)]; if (length(missing)) { message('Installing: ', paste(missing, collapse=', ')); install.packages(missing, repos='https://cran.r-project.org') }"

# -- Clean previous build output ----------------------------------------------
Write-Host '==> Cleaning old build output ...' -ForegroundColor Cyan
foreach ($dir in @($siteR, $sitePy, $gdocWork)) {
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
}
# Ensure site/ exists even on first run.
if (-not (Test-Path $siteDir)) { New-Item -ItemType Directory -Path $siteDir | Out-Null }

# -- Copy logo for Python docs (great-docs looks for logo.svg in its CWD) -----
if (Test-Path $logoSrc) {
    Write-Host '==> Copying logo.svg -> py-basemapper/logo.svg ...' -ForegroundColor Cyan
    Copy-Item $logoSrc $logoDest -Force
} else {
    Write-Warning "logo.svg not found at $logoSrc -- Python docs will build without a logo."
}

# -- R: regenerate man pages --------------------------------------------------
Write-Host '==> roxygen2::roxygenize (regenerating man pages) ...' -ForegroundColor Cyan
Rscript -e "roxygen2::roxygenize('r-basemapper')"

# -- R: pkgdown ---------------------------------------------------------------
Write-Host '==> pkgdown::build_site -> site/r/ ...' -ForegroundColor Cyan
# Output destination is set in r-basemapper/_pkgdown.yml (destination: ../site/r).
Rscript -e "pkgdown::build_site(pkg='r-basemapper', preview=FALSE, new_process=FALSE)"

# -- Python: great-docs -------------------------------------------------------
Write-Host '==> great-docs build -> py-basemapper/great-docs/_site/ ...' -ForegroundColor Cyan
# PYTHONUTF8=1 forces UTF-8 I/O on Windows (avoids charmap errors from Unicode in generated files).
Push-Location $pyPkg
try {
    $env:PYTHONUTF8 = '1'
    & great-docs build
} finally {
    Remove-Item Env:PYTHONUTF8 -ErrorAction SilentlyContinue
    Pop-Location
}

if (-not (Test-Path $gdocOut)) {
    Write-Error "great-docs build produced no output at: $gdocOut"
}

# -- Assemble into site/ ------------------------------------------------------
Write-Host '==> Copying Python docs -> site/python/ ...' -ForegroundColor Cyan
# $sitePy was cleaned above so Copy-Item creates it directly (no nesting).
Copy-Item -Recurse $gdocOut $sitePy

Write-Host '==> Copying landing page -> site/index.html ...' -ForegroundColor Cyan
Copy-Item $docsIndex $siteIndex -Force

# -- Done ---------------------------------------------------------------------
Write-Host ''
Write-Host 'Build complete. Commit and push to deploy:' -ForegroundColor Green
Write-Host '    git add site/'
Write-Host "    git commit -m 'docs: rebuild site'"
Write-Host '    git push'
