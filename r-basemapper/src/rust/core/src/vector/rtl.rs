//! Right-to-left text, as MapLibre GL JS draws it with the
//! @mapbox/mapbox-gl-rtl-text plugin (0.3.0, ICU compiled to WebAssembly):
//!
//! - `apply_arabic_shaping`: ICU `u_shapeArabic` with
//!   `U_SHAPE_LETTERS_SHAPE | U_SHAPE_TEXT_DIRECTION_LOGICAL`. Arabic letters
//!   become their isolated / initial / medial / final presentation forms
//!   (U+FB50–U+FDFF, U+FE70–U+FEFC) from their neighbours' joining types;
//!   lam + alef becomes one ligature (the text shrinks); tashkeel are
//!   transparent and take their own isolated or medial forms. Applied to
//!   every label after `text-transform`.
//! - `process_bidi`: ICU `ubidi` with a default-LTR paragraph level. Lines are
//!   broken on the logical text first; each line is then reordered for
//!   display (UAX #9 with bracket pairs, rule L1 for trailing whitespace),
//!   right-to-left runs reversed with mirrored brackets, and bidi controls
//!   removed (`UBIDI_DO_MIRRORING | UBIDI_REMOVE_BIDI_CONTROLS`).
//!
//! The shaping tables and the joining walk are ported from ICU's ushape.cpp
//! (© Unicode, Inc. and others, Unicode License); the mirroring pairs are
//! BidiMirroring.txt (Unicode 15).

use unicode_bidi::BidiInfo;

const IRRELEVANT: u16 = 4;
const LAMTYPE: u16 = 16;
const ALEFTYPE: u16 = 32;
const LINKR: u16 = 1;
const LINKL: u16 = 2;
const APRESENT: u16 = 8;
const LAMALEF_SPACE_SUB: u16 = 0xFFFF;
const LAM_CHAR: u16 = 0x0644;
const SPACE_CHAR: u16 = 0x0020;

const IRRELEVANT_POS: [u16; 8] = [0x0, 0x2, 0x4, 0x6, 0x8, 0xA, 0xC, 0xE];

#[rustfmt::skip]
const ARA_LINK: [u16; 178] = [
    1 + 32 + 256 * 0x11, 1 + 32 + 256 * 0x13, 1 + 256 * 0x15, 1 + 32 + 256 * 0x17, // 0622-0625
    1 + 2 + 256 * 0x19, 1 + 32 + 256 * 0x1D, 1 + 2 + 256 * 0x1F, 1 + 256 * 0x23,    // 0626-0629
    1 + 2 + 256 * 0x25, 1 + 2 + 256 * 0x29, 1 + 2 + 256 * 0x2D, 1 + 2 + 256 * 0x31, // 062A-062D
    1 + 2 + 256 * 0x35, 1 + 256 * 0x39, 1 + 256 * 0x3B, 1 + 256 * 0x3D,             // 062E-0631
    1 + 256 * 0x3F, 1 + 2 + 256 * 0x41, 1 + 2 + 256 * 0x45, 1 + 2 + 256 * 0x49,     // 0632-0635
    1 + 2 + 256 * 0x4D, 1 + 2 + 256 * 0x51, 1 + 2 + 256 * 0x55, 1 + 2 + 256 * 0x59, // 0636-0639
    1 + 2 + 256 * 0x5D,                                                             // 063A
    0, 0, 0, 0, 0,                                                                  // 063B-063F
    1 + 2,                                                                          // 0640
    1 + 2 + 256 * 0x61, 1 + 2 + 256 * 0x65, 1 + 2 + 256 * 0x69,                     // 0641-0643
    1 + 2 + 16 + 256 * 0x6D,                                                        // 0644
    1 + 2 + 256 * 0x71, 1 + 2 + 256 * 0x75, 1 + 2 + 256 * 0x79,                     // 0645-0647
    1 + 256 * 0x7D, 1 + 256 * 0x7F, 1 + 2 + 256 * 0x81,                             // 0648-064A
    4 + 256, 4 + 128 + 256, 4 + 128 + 256, 4 + 128 + 256,                           // 064B-064E
    4 + 128 + 256, 4 + 128 + 256, 4 + 64 + 256 * 3, 4 + 256,                        // 064F-0652
    4 + 256 * 7, 4 + 256 * 8, 4 + 256 * 8, 4 + 256,                                 // 0653-0656
    0, 0, 0, 0, 0,                                                                  // 0657-065B
    1 + 256 * 0x85, 1 + 256 * 0x87, 1 + 256 * 0x89, 1 + 256 * 0x8B,                 // 065C-065F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,                                 // 0660-066F
    4 + 256 * 6,                                                                    // 0670
    1 + 8,                                                                          // 0671
    1 + 32, 1 + 32, 0, 1 + 32,                                                      // 0672-0675
    1, 1,                                                                           // 0676-0677
    1 + 2,                                                                          // 0678
    1 + 2 + 8 + 256 * 0x16, 1 + 2 + 8 + 256 * 0x0E, 1 + 2 + 8 + 256 * 0x02,         // 0679-067B
    1 + 2, 1 + 2,                                                                   // 067C-067D
    1 + 2 + 8 + 256 * 0x06, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2,                      // 067E-0683
    1 + 2, 1 + 2, 1 + 2 + 8 + 256 * 0x2A, 1 + 2,                                    // 0684-0687
    1 + 8 + 256 * 0x38,                                                             // 0688
    1, 1, 1,                                                                        // 0689-068B
    1 + 8 + 256 * 0x34, 1 + 8 + 256 * 0x32, 1 + 8 + 256 * 0x36,                     // 068C-068E
    1, 1,                                                                           // 068F-0690
    1 + 8 + 256 * 0x3C,                                                             // 0691
    1, 1, 1, 1, 1, 1, 1 + 8 + 256 * 0x3A, 1,                                        // 0692-0699
    1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2,           // 069A-06A3
    1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2 + 8 + 256 * 0x3E, 1 + 2, 1 + 2, 1 + 2, 1 + 2, // 06A4-06AD
    1 + 2, 1 + 2 + 8 + 256 * 0x42, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, 1 + 2, // 06AE-06B7
    1 + 2, 1 + 2,                                                                   // 06B8-06B9
    1 + 8 + 256 * 0x4E, 1 + 2 + 8 + 256 * 0x50,                                     // 06BA-06BB
    1 + 2, 1 + 2,                                                                   // 06BC-06BD
    1 + 2 + 8 + 256 * 0x5A, 1 + 2,                                                  // 06BE-06BF
    1 + 8 + 256 * 0x54, 1 + 2 + 8 + 256 * 0x56,                                     // 06C0-06C1
    1, 1, 1,                                                                        // 06C2-06C4
    1 + 8 + 256 * 0x90, 1 + 8 + 256 * 0x89, 1 + 8 + 256 * 0x87, 1 + 8 + 256 * 0x8B, // 06C5-06C8
    1 + 8 + 256 * 0x92, 1, 1 + 8 + 256 * 0x8E,                                      // 06C9-06CB
    1 + 2 + 8 + 256 * 0xAC, 1,                                                      // 06CC-06CD
    1 + 2, 1 + 2,                                                                   // 06CE-06CF
    1 + 2 + 8 + 256 * 0x94, 1 + 2,                                                  // 06D0-06D1
    1 + 8 + 256 * 0x5E, 1 + 8 + 256 * 0x60,                                         // 06D2-06D3
];

#[rustfmt::skip]
const PRES_A_LINK: [u8; 275] = [
    0, 1, 0, 0, 0, 0, 0, 1, 2, 3, 0, 0, 0, 0, 0, 0, // FB5
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FB6
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 0, 0, // FB7
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, // FB8
    2, 3, 0, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FB9
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FBA
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FBB
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FBC
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FBD
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FBE
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, // FBF
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FC0
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FC1
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FC2
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FC3
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // FC4
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, // FC5
    4, 4, 4,                                        // FC6
];

#[rustfmt::skip]
const PRES_B_LINK: [u8; 141] = [
    3, 3, 3, 0, 3, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, // FE7
    0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 2, 3, 0, 1, 0, // FE8
    1, 2, 3, 0, 1, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, // FE9
    3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 0, 1, 0, 1, 0, // FEA
    1, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, // FEB
    3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, // FEC
    3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, // FED
    3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 0, // FEE
    1, 0, 1, 2, 3, 0, 1, 0, 1, 0, 1, 0, 1,          // FEF (to FEFC)
];

#[rustfmt::skip]
const CONVERT_FB_TO_06: [u16; 176] = [
    0x671, 0x671, 0x67B, 0x67B, 0x67B, 0x67B, 0x67E, 0x67E, 0x67E, 0x67E, 0, 0, 0, 0, 0x67A, 0x67A, // FB5
    0x67A, 0x67A, 0, 0, 0, 0, 0x679, 0x679, 0x679, 0x679, 0, 0, 0, 0, 0, 0,                         // FB6
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x686, 0x686, 0x686, 0x686, 0, 0,                                 // FB7
    0, 0, 0x68D, 0x68D, 0x68C, 0x68C, 0x68E, 0x68E, 0x688, 0x688, 0x698, 0x698, 0x691, 0x691, 0x6A9, 0x6A9, // FB8
    0x6A9, 0x6A9, 0x6AF, 0x6AF, 0x6AF, 0x6AF, 0, 0, 0, 0, 0, 0, 0, 0, 0x6BA, 0x6BA,                 // FB9
    0x6BB, 0x6BB, 0x6BB, 0x6BB, 0x6C0, 0x6C0, 0x6C1, 0x6C1, 0x6C1, 0x6C1, 0x6BE, 0x6BE, 0x6BE, 0x6BE, 0x6D2, 0x6D2, // FBA
    0x6D3, 0x6D3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,                                         // FBB
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,                                                 // FBC
    0, 0, 0, 0, 0, 0, 0, 0x6C7, 0x6C7, 0x6C6, 0x6C6, 0x6C8, 0x6C8, 0, 0x6CB, 0x6CB,                 // FBD
    0x6C5, 0x6C5, 0x6C9, 0x6C9, 0x6D0, 0x6D0, 0x6D0, 0x6D0, 0, 0, 0, 0, 0, 0, 0, 0,                 // FBE
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x6CC, 0x6CC, 0x6CC, 0x6CC,                                 // FBF
];

#[rustfmt::skip]
const CONVERT_FE_TO_06: [u16; 141] = [
    0x64B, 0x64B, 0x64C, 0x64C, 0x64D, 0x64D, 0x64E, 0x64E, 0x64F, 0x64F, 0x650, 0x650, 0x651, 0x651, 0x652, 0x652, // FE7
    0x621, 0x622, 0x622, 0x623, 0x623, 0x624, 0x624, 0x625, 0x625, 0x626, 0x626, 0x626, 0x626, 0x627, 0x627, 0x628, // FE8
    0x628, 0x628, 0x628, 0x629, 0x629, 0x62A, 0x62A, 0x62A, 0x62A, 0x62B, 0x62B, 0x62B, 0x62B, 0x62C, 0x62C, 0x62C, // FE9
    0x62C, 0x62D, 0x62D, 0x62D, 0x62D, 0x62E, 0x62E, 0x62E, 0x62E, 0x62F, 0x62F, 0x630, 0x630, 0x631, 0x631, 0x632, // FEA
    0x632, 0x633, 0x633, 0x633, 0x633, 0x634, 0x634, 0x634, 0x634, 0x635, 0x635, 0x635, 0x635, 0x636, 0x636, 0x636, // FEB
    0x636, 0x637, 0x637, 0x637, 0x637, 0x638, 0x638, 0x638, 0x638, 0x639, 0x639, 0x639, 0x639, 0x63A, 0x63A, 0x63A, // FEC
    0x63A, 0x641, 0x641, 0x641, 0x641, 0x642, 0x642, 0x642, 0x642, 0x643, 0x643, 0x643, 0x643, 0x644, 0x644, 0x644, // FED
    0x644, 0x645, 0x645, 0x645, 0x645, 0x646, 0x646, 0x646, 0x646, 0x647, 0x647, 0x647, 0x647, 0x648, 0x648, 0x649, // FEE
    0x649, 0x64A, 0x64A, 0x64A, 0x64A, 0x65C, 0x65C, 0x65D, 0x65D, 0x65E, 0x65E, 0x65F, 0x65F,                      // FEF
];

/// shapeTable[next link][last link][current link].
const SHAPE_TABLE: [[[u16; 4]; 4]; 4] = [
    [[0, 0, 0, 0], [0, 0, 0, 0], [0, 1, 0, 3], [0, 1, 0, 1]],
    [[0, 0, 2, 2], [0, 0, 1, 2], [0, 1, 1, 2], [0, 1, 1, 3]],
    [[0, 0, 0, 0], [0, 0, 0, 0], [0, 1, 0, 3], [0, 1, 0, 3]],
    [[0, 0, 1, 2], [0, 0, 1, 2], [0, 1, 1, 2], [0, 1, 1, 3]],
];

/// Bidi_Mirroring_Glyph pairs, sorted by the first code point.
#[rustfmt::skip]
const MIRRORS: [(u32, u32); 428] = [
    (0x0028, 0x0029), (0x0029, 0x0028), (0x003C, 0x003E), (0x003E, 0x003C), (0x005B, 0x005D), (0x005D, 0x005B),
    (0x007B, 0x007D), (0x007D, 0x007B), (0x00AB, 0x00BB), (0x00BB, 0x00AB), (0x0F3A, 0x0F3B), (0x0F3B, 0x0F3A),
    (0x0F3C, 0x0F3D), (0x0F3D, 0x0F3C), (0x169B, 0x169C), (0x169C, 0x169B), (0x2039, 0x203A), (0x203A, 0x2039),
    (0x2045, 0x2046), (0x2046, 0x2045), (0x207D, 0x207E), (0x207E, 0x207D), (0x208D, 0x208E), (0x208E, 0x208D),
    (0x2208, 0x220B), (0x2209, 0x220C), (0x220A, 0x220D), (0x220B, 0x2208), (0x220C, 0x2209), (0x220D, 0x220A),
    (0x2215, 0x29F5), (0x221F, 0x2BFE), (0x2220, 0x29A3), (0x2221, 0x299B), (0x2222, 0x29A0), (0x2224, 0x2AEE),
    (0x223C, 0x223D), (0x223D, 0x223C), (0x2243, 0x22CD), (0x2245, 0x224C), (0x224C, 0x2245), (0x2252, 0x2253),
    (0x2253, 0x2252), (0x2254, 0x2255), (0x2255, 0x2254), (0x2264, 0x2265), (0x2265, 0x2264), (0x2266, 0x2267),
    (0x2267, 0x2266), (0x2268, 0x2269), (0x2269, 0x2268), (0x226A, 0x226B), (0x226B, 0x226A), (0x226E, 0x226F),
    (0x226F, 0x226E), (0x2270, 0x2271), (0x2271, 0x2270), (0x2272, 0x2273), (0x2273, 0x2272), (0x2274, 0x2275),
    (0x2275, 0x2274), (0x2276, 0x2277), (0x2277, 0x2276), (0x2278, 0x2279), (0x2279, 0x2278), (0x227A, 0x227B),
    (0x227B, 0x227A), (0x227C, 0x227D), (0x227D, 0x227C), (0x227E, 0x227F), (0x227F, 0x227E), (0x2280, 0x2281),
    (0x2281, 0x2280), (0x2282, 0x2283), (0x2283, 0x2282), (0x2284, 0x2285), (0x2285, 0x2284), (0x2286, 0x2287),
    (0x2287, 0x2286), (0x2288, 0x2289), (0x2289, 0x2288), (0x228A, 0x228B), (0x228B, 0x228A), (0x228F, 0x2290),
    (0x2290, 0x228F), (0x2291, 0x2292), (0x2292, 0x2291), (0x2298, 0x29B8), (0x22A2, 0x22A3), (0x22A3, 0x22A2),
    (0x22A6, 0x2ADE), (0x22A8, 0x2AE4), (0x22A9, 0x2AE3), (0x22AB, 0x2AE5), (0x22B0, 0x22B1), (0x22B1, 0x22B0),
    (0x22B2, 0x22B3), (0x22B3, 0x22B2), (0x22B4, 0x22B5), (0x22B5, 0x22B4), (0x22B6, 0x22B7), (0x22B7, 0x22B6),
    (0x22B8, 0x27DC), (0x22C9, 0x22CA), (0x22CA, 0x22C9), (0x22CB, 0x22CC), (0x22CC, 0x22CB), (0x22CD, 0x2243),
    (0x22D0, 0x22D1), (0x22D1, 0x22D0), (0x22D6, 0x22D7), (0x22D7, 0x22D6), (0x22D8, 0x22D9), (0x22D9, 0x22D8),
    (0x22DA, 0x22DB), (0x22DB, 0x22DA), (0x22DC, 0x22DD), (0x22DD, 0x22DC), (0x22DE, 0x22DF), (0x22DF, 0x22DE),
    (0x22E0, 0x22E1), (0x22E1, 0x22E0), (0x22E2, 0x22E3), (0x22E3, 0x22E2), (0x22E4, 0x22E5), (0x22E5, 0x22E4),
    (0x22E6, 0x22E7), (0x22E7, 0x22E6), (0x22E8, 0x22E9), (0x22E9, 0x22E8), (0x22EA, 0x22EB), (0x22EB, 0x22EA),
    (0x22EC, 0x22ED), (0x22ED, 0x22EC), (0x22F0, 0x22F1), (0x22F1, 0x22F0), (0x22F2, 0x22FA), (0x22F3, 0x22FB),
    (0x22F4, 0x22FC), (0x22F6, 0x22FD), (0x22F7, 0x22FE), (0x22FA, 0x22F2), (0x22FB, 0x22F3), (0x22FC, 0x22F4),
    (0x22FD, 0x22F6), (0x22FE, 0x22F7), (0x2308, 0x2309), (0x2309, 0x2308), (0x230A, 0x230B), (0x230B, 0x230A),
    (0x2329, 0x232A), (0x232A, 0x2329), (0x2768, 0x2769), (0x2769, 0x2768), (0x276A, 0x276B), (0x276B, 0x276A),
    (0x276C, 0x276D), (0x276D, 0x276C), (0x276E, 0x276F), (0x276F, 0x276E), (0x2770, 0x2771), (0x2771, 0x2770),
    (0x2772, 0x2773), (0x2773, 0x2772), (0x2774, 0x2775), (0x2775, 0x2774), (0x27C3, 0x27C4), (0x27C4, 0x27C3),
    (0x27C5, 0x27C6), (0x27C6, 0x27C5), (0x27C8, 0x27C9), (0x27C9, 0x27C8), (0x27CB, 0x27CD), (0x27CD, 0x27CB),
    (0x27D5, 0x27D6), (0x27D6, 0x27D5), (0x27DC, 0x22B8), (0x27DD, 0x27DE), (0x27DE, 0x27DD), (0x27E2, 0x27E3),
    (0x27E3, 0x27E2), (0x27E4, 0x27E5), (0x27E5, 0x27E4), (0x27E6, 0x27E7), (0x27E7, 0x27E6), (0x27E8, 0x27E9),
    (0x27E9, 0x27E8), (0x27EA, 0x27EB), (0x27EB, 0x27EA), (0x27EC, 0x27ED), (0x27ED, 0x27EC), (0x27EE, 0x27EF),
    (0x27EF, 0x27EE), (0x2983, 0x2984), (0x2984, 0x2983), (0x2985, 0x2986), (0x2986, 0x2985), (0x2987, 0x2988),
    (0x2988, 0x2987), (0x2989, 0x298A), (0x298A, 0x2989), (0x298B, 0x298C), (0x298C, 0x298B), (0x298D, 0x2990),
    (0x298E, 0x298F), (0x298F, 0x298E), (0x2990, 0x298D), (0x2991, 0x2992), (0x2992, 0x2991), (0x2993, 0x2994),
    (0x2994, 0x2993), (0x2995, 0x2996), (0x2996, 0x2995), (0x2997, 0x2998), (0x2998, 0x2997), (0x299B, 0x2221),
    (0x29A0, 0x2222), (0x29A3, 0x2220), (0x29A4, 0x29A5), (0x29A5, 0x29A4), (0x29A8, 0x29A9), (0x29A9, 0x29A8),
    (0x29AA, 0x29AB), (0x29AB, 0x29AA), (0x29AC, 0x29AD), (0x29AD, 0x29AC), (0x29AE, 0x29AF), (0x29AF, 0x29AE),
    (0x29B8, 0x2298), (0x29C0, 0x29C1), (0x29C1, 0x29C0), (0x29C4, 0x29C5), (0x29C5, 0x29C4), (0x29CF, 0x29D0),
    (0x29D0, 0x29CF), (0x29D1, 0x29D2), (0x29D2, 0x29D1), (0x29D4, 0x29D5), (0x29D5, 0x29D4), (0x29D8, 0x29D9),
    (0x29D9, 0x29D8), (0x29DA, 0x29DB), (0x29DB, 0x29DA), (0x29E8, 0x29E9), (0x29E9, 0x29E8), (0x29F5, 0x2215),
    (0x29F8, 0x29F9), (0x29F9, 0x29F8), (0x29FC, 0x29FD), (0x29FD, 0x29FC), (0x2A2B, 0x2A2C), (0x2A2C, 0x2A2B),
    (0x2A2D, 0x2A2E), (0x2A2E, 0x2A2D), (0x2A34, 0x2A35), (0x2A35, 0x2A34), (0x2A3C, 0x2A3D), (0x2A3D, 0x2A3C),
    (0x2A64, 0x2A65), (0x2A65, 0x2A64), (0x2A79, 0x2A7A), (0x2A7A, 0x2A79), (0x2A7B, 0x2A7C), (0x2A7C, 0x2A7B),
    (0x2A7D, 0x2A7E), (0x2A7E, 0x2A7D), (0x2A7F, 0x2A80), (0x2A80, 0x2A7F), (0x2A81, 0x2A82), (0x2A82, 0x2A81),
    (0x2A83, 0x2A84), (0x2A84, 0x2A83), (0x2A85, 0x2A86), (0x2A86, 0x2A85), (0x2A87, 0x2A88), (0x2A88, 0x2A87),
    (0x2A89, 0x2A8A), (0x2A8A, 0x2A89), (0x2A8B, 0x2A8C), (0x2A8C, 0x2A8B), (0x2A8D, 0x2A8E), (0x2A8E, 0x2A8D),
    (0x2A8F, 0x2A90), (0x2A90, 0x2A8F), (0x2A91, 0x2A92), (0x2A92, 0x2A91), (0x2A93, 0x2A94), (0x2A94, 0x2A93),
    (0x2A95, 0x2A96), (0x2A96, 0x2A95), (0x2A97, 0x2A98), (0x2A98, 0x2A97), (0x2A99, 0x2A9A), (0x2A9A, 0x2A99),
    (0x2A9B, 0x2A9C), (0x2A9C, 0x2A9B), (0x2A9D, 0x2A9E), (0x2A9E, 0x2A9D), (0x2A9F, 0x2AA0), (0x2AA0, 0x2A9F),
    (0x2AA1, 0x2AA2), (0x2AA2, 0x2AA1), (0x2AA6, 0x2AA7), (0x2AA7, 0x2AA6), (0x2AA8, 0x2AA9), (0x2AA9, 0x2AA8),
    (0x2AAA, 0x2AAB), (0x2AAB, 0x2AAA), (0x2AAC, 0x2AAD), (0x2AAD, 0x2AAC), (0x2AAF, 0x2AB0), (0x2AB0, 0x2AAF),
    (0x2AB1, 0x2AB2), (0x2AB2, 0x2AB1), (0x2AB3, 0x2AB4), (0x2AB4, 0x2AB3), (0x2AB5, 0x2AB6), (0x2AB6, 0x2AB5),
    (0x2AB7, 0x2AB8), (0x2AB8, 0x2AB7), (0x2AB9, 0x2ABA), (0x2ABA, 0x2AB9), (0x2ABB, 0x2ABC), (0x2ABC, 0x2ABB),
    (0x2ABD, 0x2ABE), (0x2ABE, 0x2ABD), (0x2ABF, 0x2AC0), (0x2AC0, 0x2ABF), (0x2AC1, 0x2AC2), (0x2AC2, 0x2AC1),
    (0x2AC3, 0x2AC4), (0x2AC4, 0x2AC3), (0x2AC5, 0x2AC6), (0x2AC6, 0x2AC5), (0x2AC7, 0x2AC8), (0x2AC8, 0x2AC7),
    (0x2AC9, 0x2ACA), (0x2ACA, 0x2AC9), (0x2ACB, 0x2ACC), (0x2ACC, 0x2ACB), (0x2ACD, 0x2ACE), (0x2ACE, 0x2ACD),
    (0x2ACF, 0x2AD0), (0x2AD0, 0x2ACF), (0x2AD1, 0x2AD2), (0x2AD2, 0x2AD1), (0x2AD3, 0x2AD4), (0x2AD4, 0x2AD3),
    (0x2AD5, 0x2AD6), (0x2AD6, 0x2AD5), (0x2ADE, 0x22A6), (0x2AE3, 0x22A9), (0x2AE4, 0x22A8), (0x2AE5, 0x22AB),
    (0x2AEC, 0x2AED), (0x2AED, 0x2AEC), (0x2AEE, 0x2224), (0x2AF7, 0x2AF8), (0x2AF8, 0x2AF7), (0x2AF9, 0x2AFA),
    (0x2AFA, 0x2AF9), (0x2BFE, 0x221F), (0x2E02, 0x2E03), (0x2E03, 0x2E02), (0x2E04, 0x2E05), (0x2E05, 0x2E04),
    (0x2E09, 0x2E0A), (0x2E0A, 0x2E09), (0x2E0C, 0x2E0D), (0x2E0D, 0x2E0C), (0x2E1C, 0x2E1D), (0x2E1D, 0x2E1C),
    (0x2E20, 0x2E21), (0x2E21, 0x2E20), (0x2E22, 0x2E23), (0x2E23, 0x2E22), (0x2E24, 0x2E25), (0x2E25, 0x2E24),
    (0x2E26, 0x2E27), (0x2E27, 0x2E26), (0x2E28, 0x2E29), (0x2E29, 0x2E28), (0x2E55, 0x2E56), (0x2E56, 0x2E55),
    (0x2E57, 0x2E58), (0x2E58, 0x2E57), (0x2E59, 0x2E5A), (0x2E5A, 0x2E59), (0x2E5B, 0x2E5C), (0x2E5C, 0x2E5B),
    (0x3008, 0x3009), (0x3009, 0x3008), (0x300A, 0x300B), (0x300B, 0x300A), (0x300C, 0x300D), (0x300D, 0x300C),
    (0x300E, 0x300F), (0x300F, 0x300E), (0x3010, 0x3011), (0x3011, 0x3010), (0x3014, 0x3015), (0x3015, 0x3014),
    (0x3016, 0x3017), (0x3017, 0x3016), (0x3018, 0x3019), (0x3019, 0x3018), (0x301A, 0x301B), (0x301B, 0x301A),
    (0xFE59, 0xFE5A), (0xFE5A, 0xFE59), (0xFE5B, 0xFE5C), (0xFE5C, 0xFE5B), (0xFE5D, 0xFE5E), (0xFE5E, 0xFE5D),
    (0xFE64, 0xFE65), (0xFE65, 0xFE64), (0xFF08, 0xFF09), (0xFF09, 0xFF08), (0xFF1C, 0xFF1E), (0xFF1E, 0xFF1C),
    (0xFF3B, 0xFF3D), (0xFF3D, 0xFF3B), (0xFF5B, 0xFF5D), (0xFF5D, 0xFF5B), (0xFF5F, 0xFF60), (0xFF60, 0xFF5F),
    (0xFF62, 0xFF63), (0xFF63, 0xFF62),
];

fn get_link(ch: u16) -> u16 {
    match ch {
        0x0622..=0x06D3 => ARA_LINK[(ch - 0x0622) as usize],
        0x200D => 3,
        0x206D..=0x206F => 4,
        0xFB50..=0xFC62 => PRES_A_LINK[(ch - 0xFB50) as usize] as u16,
        0xFE70..=0xFEFC => PRES_B_LINK[(ch - 0xFE70) as usize] as u16,
        _ => 0,
    }
}

fn change_lam_alef(ch: u16) -> u16 {
    match ch {
        0x0622 => 0x065C,
        0x0623 => 0x065D,
        0x0625 => 0x065E,
        0x0627 => 0x065F,
        _ => 0,
    }
}

fn is_tashkeel(ch: u16) -> bool {
    (0x064B..=0x0652).contains(&ch)
}

fn is_alef(ch: u16) -> bool {
    matches!(ch, 0x0622 | 0x0623 | 0x0625 | 0x0627)
}

/// countSpaces: spaces at each end (none at the right when all are spaces).
fn count_spaces(s: &[u16]) -> (usize, usize) {
    let l = s.iter().take_while(|&&c| c == SPACE_CHAR).count();
    if l == s.len() {
        return (l, 0);
    }
    (l, s.iter().rev().take_while(|&&c| c == SPACE_CHAR).count())
}

/// invertBuffer: reverses the text between the leading and trailing spaces.
fn invert(s: &mut [u16]) {
    let (l, r) = count_spaces(s);
    let end = s.len() - r;
    if l < end {
        s[l..end].reverse();
    }
}

/// calculateSize with U_SHAPE_LAMALEF_RESIZE, logical order: the length
/// ICU reserves (one less per lam followed by alef, or presentation-form
/// tashkeel).
fn calculate_size(s: &[u16]) -> usize {
    let mut n = s.len();
    for i in 0..s.len() {
        if (s[i] == LAM_CHAR && i + 1 < s.len() && is_alef(s[i + 1])) || (0xFE70..=0xFE7F).contains(&s[i]) {
            n -= 1;
        }
    }
    n
}

/// shapeUnicode (tashkeel flag 1) over a buffer in reversed order, then the
/// lam-alef placeholders removed (handleGeneratedSpaces, resize).
fn shape_unicode(dest: &mut Vec<u16>) {
    for c in dest.iter_mut() {
        if (0xFB50..=0xFBFF).contains(c) {
            let m = CONVERT_FB_TO_06[(*c - 0xFB50) as usize];
            if m != 0 {
                *c = m;
            }
        } else if (0xFE70..=0xFEFC).contains(c) {
            *c = CONVERT_FE_TO_06[(*c - 0xFE70) as usize];
        }
    }
    let n = dest.len() as isize;
    let (iend, step) = (-1isize, -1isize);
    let mut i = n - 1;
    let at = |d: &Vec<u16>, k: isize| d[k as usize];
    let mut curr_link = get_link(at(dest, i));
    let mut last_pos = i;
    let (mut nx, mut nw): (isize, isize);
    nx = -2;
    let (mut prev_link, mut last_link, mut next_link) = (0u16, 0u16, 0u16);
    let mut lamalef_found = false;
    while i != iend {
        if (curr_link & 0xFF00) > 0 || (get_link(at(dest, i)) & IRRELEVANT) != 0 {
            nw = i + step;
            while nx < 0 {
                if nw == iend {
                    next_link = 0;
                    nx = 3000;
                } else {
                    next_link = get_link(at(dest, nw));
                    if (next_link & IRRELEVANT) == 0 {
                        nx = nw;
                    } else {
                        nw += step;
                    }
                }
            }
            if (curr_link & ALEFTYPE) > 0 && (last_link & LAMTYPE) > 0 {
                lamalef_found = true;
                let w = change_lam_alef(at(dest, i));
                if w != 0 {
                    dest[i as usize] = LAMALEF_SPACE_SUB;
                    dest[last_pos as usize] = w;
                    i = last_pos;
                }
                last_link = prev_link;
                curr_link = get_link(w);
            }
            let mut shape = SHAPE_TABLE[(next_link & (LINKR + LINKL)) as usize][(last_link & (LINKR + LINKL)) as usize]
                [(curr_link & (LINKR + LINKL)) as usize];
            let c = at(dest, i);
            if (curr_link & (LINKR + LINKL)) == 1 {
                shape &= 1;
            } else if is_tashkeel(c) {
                if (last_link & LINKL) != 0 && (next_link & LINKR) != 0 && c != 0x064C && c != 0x064D {
                    shape = 1;
                    if (next_link & ALEFTYPE) == ALEFTYPE && (last_link & LAMTYPE) == LAMTYPE {
                        shape = 0;
                    }
                } else {
                    shape = 0;
                }
            }
            if (c ^ 0x0600) < 0x100 {
                if is_tashkeel(c) {
                    dest[i as usize] = 0xFE70 + IRRELEVANT_POS[(c - 0x064B) as usize] + shape;
                } else if (curr_link & APRESENT) > 0 {
                    dest[i as usize] = 0xFB50 + (curr_link >> 8) + shape;
                } else if (curr_link >> 8) > 0 && (curr_link & IRRELEVANT) == 0 {
                    dest[i as usize] = 0xFE70 + (curr_link >> 8) + shape;
                }
            }
        }
        if (curr_link & IRRELEVANT) == 0 {
            prev_link = last_link;
            last_link = curr_link;
            last_pos = i;
        }
        i += step;
        if i == nx {
            curr_link = next_link;
            nx = -2;
        } else if i != iend {
            curr_link = get_link(at(dest, i));
        }
    }
    if lamalef_found {
        dest.retain(|&c| c != LAMALEF_SPACE_SUB);
        // u_strlen: the text ends at a NUL.
        if let Some(z) = dest.iter().position(|&c| c == 0) {
            dest.truncate(z);
        }
    }
}

/// The plugin's applyArabicShaping (ushape_arabic).
pub fn apply_arabic_shaping(input: &str) -> String {
    let src: Vec<u16> = input.encode_utf16().collect();
    // Only the 06xx letters change; presentation forms are re-shaped.
    if !src.iter().any(|&c| (0x0600..=0x06FF).contains(&c) || (0xFB50..=0xFEFC).contains(&c)) {
        return input.to_owned();
    }
    let reserved = calculate_size(&src);
    let mut buf = src;
    invert(&mut buf);
    shape_unicode(&mut buf);
    invert(&mut buf);
    // The plugin reserves `reserved + 1` units and terminates at
    // `reserved`; a longer result is an overflow error (input unchanged).
    if buf.len() > reserved + 1 {
        return input.to_owned();
    }
    buf.truncate(reserved);
    String::from_utf16_lossy(&buf)
}

fn mirror(c: char) -> char {
    match MIRRORS.binary_search_by_key(&(c as u32), |p| p.0) {
        Ok(k) => char::from_u32(MIRRORS[k].1).unwrap_or(c),
        Err(_) => c,
    }
}

/// A character of a right-to-left script (Hebrew, Arabic, Syriac, Thaana,
/// NKo, Samaritan, Mandaic and their presentation forms).
pub fn is_rtl_char(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// ICU IS_BIDI_CONTROL_CHAR: ZWNJ, ZWJ, LRM, RLM, LRE..RLO, LRI..PDI.
fn is_bidi_control(c: char) -> bool {
    matches!(c as u32, 0x200C..=0x200F | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// The plugin's processBidirectionalText: `chars` in logical order with
/// line breaks (char indices, the last one the text length) chosen on it;
/// returns each line in visual order. Paragraph ends (after a paragraph
/// separator) are merged into the breaks as the plugin does.
pub fn process_bidi(chars: &[char], breaks: &[usize]) -> Vec<Vec<char>> {
    if chars.is_empty() {
        return vec![Vec::new()];
    }
    let text: String = chars.iter().collect();
    // Byte offset of each char index (and of the end).
    let mut byte_at: Vec<usize> = Vec::with_capacity(chars.len() + 1);
    let mut b = 0;
    for c in chars {
        byte_at.push(b);
        b += c.len_utf8();
    }
    byte_at.push(b);
    let char_at = |byte: usize| byte_at.partition_point(|&x| x < byte);
    let info = BidiInfo::new(&text, None);
    // mergeParagraphLineBreakPoints.
    let mut merged: Vec<usize> = Vec::new();
    for p in &info.paragraphs {
        let end = char_at(p.range.end);
        for &lb in breaks {
            if lb < end && merged.last().is_none_or(|&m| lb > m) {
                merged.push(lb);
            }
        }
        merged.push(end);
    }
    for &lb in breaks {
        if merged.last().is_none_or(|&m| lb > m) {
            merged.push(lb);
        }
    }
    let mut lines = Vec::with_capacity(merged.len());
    let mut start = 0;
    for &end in &merged {
        let end = end.min(chars.len());
        let mut line: Vec<char> = Vec::with_capacity(end.saturating_sub(start));
        if start < end {
            let (b0, b1) = (byte_at[start], byte_at[end]);
            let para = info.paragraphs.iter().find(|p| p.range.start <= b0 && b0 < p.range.end).unwrap_or(&info.paragraphs[0]);
            let b1 = b1.min(para.range.end);
            let (levels, runs) = info.visual_runs(para, b0..b1);
            for run in runs {
                let s = &text[run.clone()];
                if levels[run.start].is_rtl() {
                    line.extend(s.chars().rev().map(mirror));
                } else {
                    line.extend(s.chars());
                }
            }
            line.retain(|&c| !is_bidi_control(c));
        }
        lines.push(line);
        start = end.max(start);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u32> {
        s.chars().map(|c| c as u32).collect()
    }

    #[test]
    fn arabic_letters_take_contextual_forms() {
        // بيت: beh initial, yeh medial, teh final.
        assert_eq!(hex(&apply_arabic_shaping("\u{0628}\u{064A}\u{062A}")), vec![0xFE91, 0xFEF4, 0xFE96]);
        // A lone letter is isolated; text without Arabic is unchanged.
        assert_eq!(hex(&apply_arabic_shaping("\u{0628}")), vec![0xFE8F]);
        assert_eq!(apply_arabic_shaping("Salt Lake"), "Salt Lake");
        // دار: dal and reh never join to the left.
        assert_eq!(hex(&apply_arabic_shaping("\u{062F}\u{0627}\u{0631}")), vec![0xFEA9, 0xFE8D, 0xFEAD]);
    }

    #[test]
    fn lam_alef_becomes_one_ligature() {
        // سلام: seen initial, lam-alef final ligature, meem isolated.
        assert_eq!(hex(&apply_arabic_shaping("\u{0633}\u{0644}\u{0627}\u{0645}")), vec![0xFEB3, 0xFEFC, 0xFEE1]);
        // Spaces around the word stay where they are.
        assert_eq!(hex(&apply_arabic_shaping(" \u{0644}\u{0627} ")), vec![0x20, 0xFEFB, 0x20]);
    }

    #[test]
    fn bidi_reorders_lines_and_mirrors_brackets() {
        let c = |s: &str| s.chars().collect::<Vec<char>>();
        let s = |v: &Vec<char>| v.iter().collect::<String>();
        // Pure RTL: reversed.
        let t = c("\u{05D0}\u{05D1}\u{05D2}");
        assert_eq!(s(&process_bidi(&t, &[3])[0]), "\u{05D2}\u{05D1}\u{05D0}");
        // LTR paragraph with an RTL run.
        let t = c("ab \u{05D0}\u{05D1} cd");
        assert_eq!(s(&process_bidi(&t, &[t.len()])[0]), "ab \u{05D1}\u{05D0} cd");
        // RTL paragraph: brackets mirror, numbers stay left to right.
        let t = c("\u{05D0} (12)");
        assert_eq!(s(&process_bidi(&t, &[t.len()])[0]), "(12) \u{05D0}");
        // Bidi controls are removed.
        let t = c("a\u{200F}b");
        assert_eq!(s(&process_bidi(&t, &[3])[0]), "ab");
        // Each line is reordered on its own.
        let t = c("\u{05D0}\u{05D1} \u{05D2}\u{05D3}");
        let lines = process_bidi(&t, &[3, 5]);
        assert_eq!(lines.iter().map(s).collect::<Vec<_>>(), vec![" \u{05D1}\u{05D0}", "\u{05D3}\u{05D2}"]);
    }
}
