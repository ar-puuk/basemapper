// R looks for R_init_{dll_name} on load; extendr generates R_init_basemapper_extendr.
// This C file bridges the two and gives R's build system a source file to compile,
// which is required for it to produce basemapper.dll from the Rust static library.

void R_init_basemapper_extendr(void *dll);

void R_init_basemapper(void *dll) {
    R_init_basemapper_extendr(dll);
}
