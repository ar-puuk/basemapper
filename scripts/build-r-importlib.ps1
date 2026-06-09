# build-r-importlib.ps1
#
# Generates the GNU import library (.r-lib/libR.dll.a) required to build
# r-basemapper.dll against R.dll on Windows using the RTools45 GNU toolchain.
#
# Must be re-run whenever R is upgraded.
#
# Prerequisites:
#   - R 4.x installed at C:\Program Files\R\R-<version>\
#   - RTools45 installed at C:\rtools45\
#
# Output (all written to .r-lib/ in the workspace root):
#   R.def        — raw export list extracted from R.dll via objdump
#   R_gnu.def    — patched def with DATA keyword on all LibExtern variables
#   libR.dll.a   — GNU import library built from R_gnu.def via dlltool
#   libgcc_eh.a  — empty stub required because the static.posix toolchain
#                  does not ship it but Rust's GNU stdlib tries to link it
#   R.lib / R.exp — MSVC import library built from R.def via lib.exe
#                   (used only for non-extendr MSVC builds)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Paths ────────────────────────────────────────────────────────────────────
$WorkspaceRoot = Split-Path -Parent $PSScriptRoot
$OutDir        = Join-Path $WorkspaceRoot ".r-lib"

$RHome      = "C:\Program Files\R\R-4.6.0"
$RTools     = "C:\rtools45"
$RDll       = Join-Path $RHome "bin\x64\R.dll"

$Objdump    = "$RTools\mingw64\bin\objdump.exe"
$Dlltool    = "$RTools\mingw64\bin\dlltool.exe"
$Ar         = "$RTools\mingw64\bin\ar.exe"

foreach ($tool in $Objdump, $Dlltool, $Ar) {
    if (-not (Test-Path $tool)) { throw "Not found: $tool" }
}
if (-not (Test-Path $RDll)) { throw "Not found: $RDll" }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# ── Step 1: extract all export names from R.dll ───────────────────────────────
# objdump -p prints the export table in human-readable form; we parse the
# symbol names out of the "[  N] +base[  B] <addr>  <name>" lines.
Write-Host "Extracting exports from R.dll..."
$objdumpOut = & $Objdump -p $RDll
$exports = $objdumpOut |
    Select-String '^\s*\[\s*\d+\]\s+\+base\[' |
    ForEach-Object {
        if ($_.Line -match '\+base\[\s*\d+\]\s+[0-9a-f]+\s+(\S+)') {
            $Matches[1]
        }
    } |
    Where-Object { $_ }

Write-Host "$($exports.Count) exports found"

# ── Step 2: build R.def (plain, no DATA) ──────────────────────────────────────
Write-Host "Writing R.def..."
$defLines = @("LIBRARY R.dll", "EXPORTS") + ($exports | ForEach-Object { "    $_" })
$defLines | Set-Content (Join-Path $OutDir "R.def") -Encoding ascii

# ── Step 3: identify LibExtern (DATA) symbols ────────────────────────────────
# These are the C-level global variables exported from R.dll.  The list is
# derived from R's header files (Rinternals.h, Rinterface.h, etc.).
# Any symbol that extendr's LibExtern macro uses must appear here.
$dataSymbols = @(
    "R_NilValue", "R_GlobalEnv", "R_EmptyEnv", "R_BaseEnv",
    "R_BaseNamespace", "R_NamespaceRegistry",
    "R_Srcref", "R_TrueValue", "R_FalseValue", "R_LogicalNAValue",
    "R_MissingArg", "R_UnboundValue", "R_RestartToken",
    "R_NaString", "R_BlankString", "R_BlankScalarString",
    "R_CurrentExpression", "R_ParseError", "R_ParseErrorMsg",
    "R_ParseContext", "R_ParseContextLine", "R_ParseContextLast",
    "R_NaN", "R_PosInf", "R_NegInf", "R_NaReal", "R_NaInt",
    "R_GlobalContext",
    "R_interrupts_suspended", "R_interrupts_pending",
    "R_jit_enabled", "R_compile_pkgs",
    "R_PPStackSize", "R_PPStackTop",
    "R_Outputfile", "R_Consolefile",
    "R_CStackLimit", "R_CStackStart", "R_CStackDir",
    "R_SignalHandlers", "R_Interactive",
    "R_DirtyImage",
    "R_dot_Generic", "R_dot_Class", "R_dot_Method",
    "R_dot_Methods", "R_dot_defined", "R_dot_target",
    "R_SymbolTable",
    "R_HandlerStack", "R_RestartStack",
    "R_Connections",
    "R_StringHash",
    "R_IsRunningMain",
    "R_MacroExpansionDepth",
    "R_Expressions", "R_Expressions_keep",
    "R_WarnLength",
    "R_CollectWarnings"
)
# Filter to only those actually present in this R version's DLL.
$dataSet = [System.Collections.Generic.HashSet[string]]$dataSymbols
$presentData = $exports | Where-Object { $dataSet.Contains($_) }
Write-Host "$($presentData.Count) DATA symbols identified"

# ── Step 4: build R_gnu.def (DATA-qualified) ─────────────────────────────────
Write-Host "Writing R_gnu.def..."
$gnuLines = @("LIBRARY R.dll", "EXPORTS")
foreach ($sym in $exports) {
    if ($dataSet.Contains($sym)) {
        $gnuLines += "    $sym DATA"
    } else {
        $gnuLines += "    $sym"
    }
}
$gnuLines | Set-Content (Join-Path $OutDir "R_gnu.def") -Encoding ascii

# ── Step 5: build libR.dll.a via dlltool ─────────────────────────────────────
Write-Host "Building libR.dll.a..."
$gnuDef  = Join-Path $OutDir "R_gnu.def"
$gnuLib  = Join-Path $OutDir "libR.dll.a"
& $Dlltool --def $gnuDef --dllname R.dll --output-lib $gnuLib
if ($LASTEXITCODE -ne 0) { throw "dlltool failed" }
Write-Host "libR.dll.a written ($([int](Get-Item $gnuLib).Length) bytes)"

# ── Step 6: empty libgcc_eh.a stub ───────────────────────────────────────────
# RTools45 static.posix toolchain doesn't ship libgcc_eh.a but Rust's GNU
# stdlib requests it at link time.  An empty archive satisfies the linker.
Write-Host "Creating libgcc_eh.a stub..."
$stubSrc = [System.IO.Path]::GetTempFileName()
Remove-Item $stubSrc -Force  # ar needs a non-existent path for 'crs'
& $Ar crs (Join-Path $OutDir "libgcc_eh.a")
Write-Host "libgcc_eh.a stub written"

Write-Host ""
Write-Host "Done.  Files in .r-lib/:"
Get-ChildItem $OutDir | Format-Table Name, Length -AutoSize
