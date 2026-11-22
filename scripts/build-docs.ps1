<#
.SYNOPSIS
    Build the unified basemapper documentation site locally.

.DESCRIPTION
    Runs roxygen2::roxygenize (R man pages), pkgdown (R HTML site),
    great-docs (Python HTML site) and rustdoc (Rust HTML site), then
    assembles everything into site/ and gives the three sites one
    navigation. The R articles and the Python user guide are executed, and
    the Rust guide's examples are run: building needs network access.

    Prerequisites
    -------------
    - R with packages: roxygen2, pkgdown, bslib, ggplot2, sf, jsonlite,
      tmap, stars, knitr, rmarkdown  (auto-installed if missing)
    - Rust toolchain + RTools45 (required to compile the R/Python packages
      and to build the Rust docs)
    - Python with pip (great-docs is installed automatically via .[dev])
    - The Quarto CLI (great-docs renders with it)
    - On macOS/Linux, PowerShell 7: pwsh ./scripts/build-docs.ps1

    See "Updating the documentation" in README.md for the whole process.

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
    if ($LASTEXITCODE -ne 0) { Write-Error 'pip install failed' }
    # great-docs executes the user guide with py-basemapper/.venv's Python
    # when that folder exists (else this Python): give it the same packages.
    $venvPython = Join-Path $pyPkg (Join-Path '.venv' (Join-Path 'Scripts' 'python.exe'))
    if (Test-Path $venvPython) {
        Write-Host '==> also into py-basemapper/.venv, which great-docs runs the user guide with ...' -ForegroundColor Cyan
        if (Get-Command uv -ErrorAction SilentlyContinue) {
            uv pip install --python $venvPython -e ".[dev]"
        } else {
            & $venvPython -m pip install -e ".[dev]"
        }
        if ($LASTEXITCODE -ne 0) { Write-Error 'installing into py-basemapper/.venv failed' }
    }
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

# -- Protomaps styles for the PMTiles examples --------------------------------
# The R articles and Python guide load them as styles/protomaps-*.json; the
# copies are git-ignored, core/guide/styles/ is the source.
$stylesSrc = Join-Path (Join-Path (Join-Path $repoRoot 'core') 'guide') 'styles'
$stylesDests = @(
    (Join-Path (Join-Path (Join-Path (Join-Path $repoRoot 'r-basemapper') 'vignettes') 'articles') 'styles'),
    (Join-Path (Join-Path $pyPkg 'user_guide') 'styles')
)
foreach ($dest in $stylesDests) {
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    Copy-Item (Join-Path $stylesSrc '*.json') $dest -Force
}

# -- R: regenerate man pages --------------------------------------------------
Write-Host '==> roxygen2::roxygenize (regenerating man pages) ...' -ForegroundColor Cyan
Rscript -e "roxygen2::roxygenize('r-basemapper')"
if ($LASTEXITCODE -ne 0) { Write-Error 'roxygen2 failed' }

# -- R: pkgdown ---------------------------------------------------------------
Write-Host '==> pkgdown::build_site -> site/r/ ...' -ForegroundColor Cyan
# Output destination is set in r-basemapper/_pkgdown.yml (destination: ../site/r).
Rscript -e "pkgdown::build_site(pkg='r-basemapper', preview=FALSE, new_process=FALSE)"
if ($LASTEXITCODE -ne 0) { Write-Error 'pkgdown failed' }

# -- Python: great-docs -------------------------------------------------------
Write-Host '==> great-docs build -> py-basemapper/great-docs/_site/ ...' -ForegroundColor Cyan
# PYTHONUTF8=1 forces UTF-8 I/O on Windows (avoids charmap errors from Unicode in generated files).
Push-Location $pyPkg
try {
    $env:PYTHONUTF8 = '1'
    & great-docs build
    if ($LASTEXITCODE -ne 0) { Write-Error 'great-docs failed' }
} finally {
    Remove-Item Env:PYTHONUTF8 -ErrorAction SilentlyContinue
    Pop-Location
}

if (-not (Test-Path $gdocOut)) {
    Write-Error "great-docs build produced no output at: $gdocOut"
}

# -- Rust: guide examples + rustdoc -> site/rust/ ----------------------------
Write-Host '==> Rust docs (runs the guide examples) -> site/rust/ ...' -ForegroundColor Cyan
$env:PYTHONUTF8 = '1'
try {
    python (Join-Path $PSScriptRoot 'docs_rust.py') $siteDir
    if ($LASTEXITCODE -ne 0) { Write-Error 'Rust docs failed' }
} finally {
    Remove-Item Env:PYTHONUTF8 -ErrorAction SilentlyContinue
}

# -- Assemble into site/ ------------------------------------------------------
Write-Host '==> Copying Python docs -> site/python/ ...' -ForegroundColor Cyan
# $sitePy was cleaned above so Copy-Item creates it directly (no nesting).
Copy-Item -Recurse $gdocOut $sitePy

Write-Host '==> Copying landing page -> site/index.html ...' -ForegroundColor Cyan
Copy-Item $docsIndex $siteIndex -Force

# -- One navigation for the three sites ---------------------------------------
Write-Host '==> Adding the R | Python | Rust switcher to every page ...' -ForegroundColor Cyan
python (Join-Path $PSScriptRoot 'docs_postprocess.py') $siteDir
if ($LASTEXITCODE -ne 0) { Write-Error 'docs post-processing failed' }

# -- Done ---------------------------------------------------------------------
Write-Host ''
Write-Host 'Build complete. Commit and push to deploy:' -ForegroundColor Green
Write-Host '    git add site/'
Write-Host "    git commit -m 'docs: rebuild site'"
Write-Host '    git push'
