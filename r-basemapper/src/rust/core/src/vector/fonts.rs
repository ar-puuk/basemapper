//! Fonts for editable SVG text: MapLibre font names resolved to font files
//! (user folders, a local cache, then Google Fonts for open fonts), the
//! characters each font draws, and `@font-face` rules embedding them
//! (subset to those characters, or whole).
//!
//! MapLibre styles name fonts ("Open Sans Bold") but serve only SDF glyph
//! bitmaps, so true outlines need the font files themselves. Fonts that
//! cannot be found stay referenced by name: the text remains editable and
//! uses whatever the viewer has installed.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

/// A font file, read with skrifa (Google Fonts' fontations), for what SVG
/// text needs: which characters it has, a few metrics, and its names,
/// weight and style. Glyph IDs are 16-bit, as the subsetter takes them.
struct Face<'a>(skrifa::FontRef<'a>);

impl<'a> Face<'a> {
    /// The first font of a .ttf / .otf / .ttc file, if it is one: skrifa
    /// reads tables lazily, so the core ones are required here (a web page
    /// saved under a font's name is not a font).
    fn parse(data: &'a [u8]) -> Option<Self> {
        let font = skrifa::FontRef::from_index(data, 0).ok()?;
        let has = |t: &[u8; 4]| font.table_data(skrifa::Tag::new(t)).is_some();
        (has(b"head") && has(b"cmap") && has(b"maxp")).then_some(Face(font))
    }

    fn glyph_index(&self, c: char) -> Option<u16> {
        use skrifa::MetadataProvider;
        self.0.charmap().map(c).and_then(|g| u16::try_from(g.to_u32()).ok())
    }

    fn metrics(&self) -> skrifa::metrics::Metrics {
        use skrifa::MetadataProvider;
        self.0.metrics(skrifa::instance::Size::unscaled(), skrifa::instance::LocationRef::default())
    }

    /// The top of a glyph's outline in font units; None for an empty glyph
    /// (a space).
    fn glyph_top(&self, gid: u16) -> Option<f64> {
        use skrifa::MetadataProvider;
        let metrics = self.0.glyph_metrics(skrifa::instance::Size::unscaled(), skrifa::instance::LocationRef::default());
        let b = metrics.bounds(skrifa::GlyphId::new(gid as u32))?;
        let empty = b.x_min == 0.0 && b.y_min == 0.0 && b.x_max == 0.0 && b.y_max == 0.0;
        (!empty).then_some(b.y_max as f64)
    }

    /// The family, full and typographic family names (name IDs 1, 4, 16).
    fn names(&self) -> Vec<String> {
        use skrifa::{string::StringId, MetadataProvider};
        [StringId::FAMILY_NAME, StringId::FULL_NAME, StringId::TYPOGRAPHIC_FAMILY_NAME]
            .into_iter()
            .flat_map(|id| self.0.localized_strings(id).map(|s| s.to_string()).collect::<Vec<_>>())
            .collect()
    }

    /// The OS/2 weight class (400 regular, 700 bold).
    fn weight(&self) -> u16 {
        use skrifa::MetadataProvider;
        self.0.attributes().weight.value().round() as u16
    }

    fn is_italic(&self) -> bool {
        use skrifa::MetadataProvider;
        self.0.attributes().style == skrifa::attribute::Style::Italic
    }
}

/// How SVG output writes label text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SvgText {
    /// `<text>` elements (editable, selectable), fonts embedded per
    /// [`FontEmbed`]; glyph positions are MapLibre's layout.
    #[default]
    Editable,
    /// Every label drawn from the style's SDF glyph bitmaps (embedded images
    /// thresholded by SVG filters as MapLibre's shader does): MapLibre's
    /// glyphs and edges at any zoom, not editable. Editable mode falls back
    /// to this for labels whose font file is not available.
    Bitmap,
    /// Glyph outlines traced from the style's SDF glyphs: not editable,
    /// plain paths (no filters).
    Paths,
}

/// How fonts are embedded in editable SVG text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontEmbed {
    /// Only the glyphs the map uses (small files).
    #[default]
    Subset,
    /// Whole font files (new text typed later keeps the font).
    Full,
    /// Reference fonts by name only.
    None,
}

/// SVG output options.
#[derive(Clone, Debug)]
pub struct SvgOptions {
    pub text: SvgText,
    pub embed_fonts: FontEmbed,
    /// Folders searched for font files (.ttf / .otf) before downloading;
    /// default: `BASEMAPPER_FONT_DIRS` (path-separated).
    pub font_dirs: Vec<PathBuf>,
    /// Download open fonts from Google Fonts (cached on disk).
    pub download_fonts: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        let font_dirs = std::env::var_os("BASEMAPPER_FONT_DIRS")
            .map(|v| std::env::split_paths(&v).collect())
            .unwrap_or_default();
        SvgOptions { text: SvgText::Editable, embed_fonts: FontEmbed::Subset, font_dirs, download_fonts: true }
    }
}

/// A MapLibre font name split into family, CSS weight and italic, e.g.
/// "Montserrat SemiBold Italic" -> ("Montserrat", 600, true).
pub fn parse_font_name(name: &str) -> (String, u16, bool) {
    let mut words: Vec<&str> = name.split_whitespace().collect();
    let mut italic = false;
    let mut weight = 400;
    // Trailing style words, possibly combined ("Bold Italic", "Extra Bold").
    loop {
        let Some(last) = words.last().map(|w| w.to_ascii_lowercase()) else { break };
        let prev = if words.len() >= 2 { words[words.len() - 2].to_ascii_lowercase() } else { String::new() };
        let two = format!("{prev}{last}");
        let w2 = match two.as_str() {
            "extralight" | "ultralight" => Some(200),
            "semibold" | "demibold" => Some(600),
            "extrabold" | "ultrabold" => Some(800),
            _ => None,
        };
        if let Some(w) = w2 {
            weight = w;
            words.truncate(words.len() - 2);
            continue;
        }
        let w1 = match last.as_str() {
            "thin" | "hairline" => Some(100),
            "extralight" | "ultralight" => Some(200),
            "light" => Some(300),
            "regular" | "book" | "normal" | "roman" => Some(400),
            "medium" => Some(500),
            "semibold" | "demibold" => Some(600),
            "bold" => Some(700),
            "extrabold" | "ultrabold" | "heavy" => Some(800),
            "black" => Some(900),
            _ => None,
        };
        if let Some(w) = w1 {
            weight = w;
            words.pop();
            continue;
        }
        if last == "italic" || last == "oblique" {
            italic = true;
            words.pop();
            continue;
        }
        break;
    }
    let family = if words.is_empty() { name.to_owned() } else { words.join(" ") };
    (family, weight, italic)
}

/// The Google Fonts family for a style's font family (a few common names
/// differ), or None for fonts that are not there.
fn google_family(family: &str) -> Option<String> {
    let f = family.trim_start_matches("Klokantech ").trim();
    let mapped = match f {
        "Noto Sans CJK JP" | "Noto Sans CJK" => "Noto Sans JP",
        "Noto Sans CJK KR" => "Noto Sans KR",
        "Noto Sans CJK SC" => "Noto Sans SC",
        "Noto Sans CJK TC" => "Noto Sans TC",
        other => other,
    };
    let proprietary = ["Arial", "Arial Unicode MS", "Helvetica", "Helvetica Neue", "Avenir", "Avenir Next", "Tahoma", "Verdana", "Times New Roman", "Segoe UI", "Calibri", "Frutiger"];
    if proprietary.iter().any(|p| p.eq_ignore_ascii_case(mapped)) {
        return None;
    }
    Some(mapped.to_owned())
}

/// One font of the document: what CSS calls it and, when found, its file.
pub struct FontFace {
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    pub data: Option<Arc<Vec<u8>>>,
}

/// The fonts of an SVG document: faces, the faces of each font stack, and
/// the characters each face draws.
#[derive(Default)]
pub struct SvgFonts {
    pub faces: Vec<FontFace>,
    stacks: HashMap<String, Vec<usize>>,
    used: Mutex<Vec<BTreeSet<char>>>,
    /// Text drawn per stack, routed to faces by [`SvgFonts::finish`].
    texts: Mutex<Vec<(String, String)>>,
    /// Faces whose file has not been looked up yet (fallbacks are fetched
    /// only when a character needs them).
    pending: Mutex<Vec<bool>>,
    /// embed_fonts = None: text references fonts by name, never falls back.
    by_name: bool,
}

impl SvgFonts {
    /// `font-family`, `font-weight`, `font-style` attributes for a font stack
    /// ("Open Sans Bold,Arial Unicode MS Bold").
    pub fn text_attrs(&self, stack: &str) -> String {
        let idx = self.stacks.get(stack).cloned().unwrap_or_default();
        let mut fams: Vec<String> = Vec::new();
        for &i in &idx {
            let q = format!("'{}'", self.faces[i].family.replace('\'', ""));
            if !fams.contains(&q) {
                fams.push(q);
            }
        }
        fams.push("sans-serif".into());
        let (w, it) = idx.first().map(|&i| (self.faces[i].weight, self.faces[i].italic)).unwrap_or((400, false));
        format!(
            r#"font-family="{}" font-weight="{w}"{}"#,
            super::svg::xml_escape(&fams.join(", ")),
            if it { r#" font-style="italic""# } else { "" }
        )
    }

    /// Whether `text` can be written as `<text>` with an exact font: the
    /// stack's first font was found and has every (non-space) character
    /// (always, when fonts are referenced by name only).
    pub fn covers(&self, stack: &str, text: &str) -> bool {
        if self.by_name {
            return true;
        }
        let Some(&first) = self.stacks.get(stack).and_then(|idx| idx.first()) else { return false };
        let Some(data) = &self.faces[first].data else { return false };
        let Some(face) = Face::parse(data) else { return false };
        text.chars().all(|c| c.is_whitespace() || face.glyph_index(c).is_some())
    }

    /// Vertical shift (24 px glyph units, y down) from MapLibre's glyph line
    /// position to the font baseline. The SDF glyph `top` is measured from a
    /// generator-specific line (fontnik: the ascender), so it is measured per
    /// font: the median over glyphs of (ink top from the real outline - SDF
    /// top). `tops` gives (codepoint, SDF top) for the glyphs available.
    pub fn baseline_shift(&self, stack: &str, tops: &[(u32, f64)]) -> f64 {
        let face = self.stacks.get(stack).and_then(|idx| idx.iter().find_map(|&i| self.faces[i].data.clone()));
        let Some(data) = face else { return 22.0 };
        let Some(f) = Face::parse(&data) else { return 22.0 };
        let metrics = f.metrics();
        let scale = 24.0 / metrics.units_per_em as f64;
        let mut d: Vec<f64> = tops
            .iter()
            .filter_map(|&(cp, top)| {
                let g = f.glyph_index(char::from_u32(cp)?)?;
                Some(f.glyph_top(g)? * scale - top)
            })
            .collect();
        if d.is_empty() {
            return metrics.ascent as f64 * scale;
        }
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        d[d.len() / 2]
    }

    /// Record text drawn with `stack` (characters are assigned to fonts by
    /// [`SvgFonts::finish`]).
    pub fn note_text(&self, stack: &str, text: &str) {
        self.texts.lock().unwrap().push((stack.to_owned(), text.to_owned()));
    }

    /// Assign every recorded character to the first font of its stack that
    /// has it, looking up fallback fonts (e.g. CJK) only when a character
    /// needs them. Call before [`SvgFonts::font_face_css`].
    pub async fn finish(&mut self, client: &reqwest::Client, opts: &SvgOptions) {
        let texts = std::mem::take(&mut *self.texts.lock().unwrap());
        let mut used = vec![BTreeSet::new(); self.faces.len()];
        for (stack, text) in texts {
            let Some(idx) = self.stacks.get(&stack).cloned() else { continue };
            for ch in text.chars() {
                let mut chosen = None;
                for &i in &idx {
                    if std::mem::take(&mut self.pending.lock().unwrap()[i]) && opts.embed_fonts != FontEmbed::None {
                        let f = &self.faces[i];
                        let (family, weight, italic) = (f.family.clone(), f.weight, f.italic);
                        self.faces[i].data = find_font(client, &family, weight, italic, opts).await;
                    }
                    let Some(data) = &self.faces[i].data else { continue };
                    let Some(face) = Face::parse(data) else { continue };
                    // Spaces go to the first embedded font (layout uses explicit positions).
                    if ch.is_whitespace() || face.glyph_index(ch).is_some() {
                        chosen = Some(i);
                        break;
                    }
                }
                if let Some(i) = chosen {
                    used[i].insert(ch);
                }
            }
        }
        *self.used.lock().unwrap() = used;
    }

    /// `@font-face` rules for the faces that draw something.
    pub fn font_face_css(&self, embed: FontEmbed) -> String {
        if embed == FontEmbed::None {
            return String::new();
        }
        let used = self.used.lock().unwrap();
        let mut css = String::new();
        for (i, face) in self.faces.iter().enumerate() {
            let Some(data) = &face.data else { continue };
            if used[i].is_empty() {
                continue;
            }
            let bytes = match embed {
                FontEmbed::Full => Some(data.as_ref().clone()),
                _ => subset_font(data, &used[i]),
            };
            let Some(bytes) = bytes else { continue };
            css.push_str(&format!(
                "@font-face{{font-family:'{}';font-weight:{};font-style:{};src:url(data:font/ttf;base64,{}) format('truetype');}}",
                css_family(&face.family),
                face.weight,
                if face.italic { "italic" } else { "normal" },
                super::svg::base64(&bytes)
            ));
        }
        css
    }
}

/// `data` reduced to the glyphs of `chars` (plus .notdef), with a Unicode
/// cmap so browsers accept it. None if the font cannot be subset.
fn subset_font(data: &[u8], chars: &BTreeSet<char>) -> Option<Vec<u8>> {
    use allsorts::binary::read::ReadScope;
    use allsorts::font_data::FontData;
    use allsorts::subset::{subset, CmapTarget, SubsetProfile};
    let face = Face::parse(data)?;
    let mut gids: Vec<u16> = vec![0];
    for &c in chars {
        if let Some(g) = face.glyph_index(c) {
            if !gids.contains(&g) {
                gids.push(g);
            }
        }
    }
    let font = ReadScope::new(data).read::<FontData<'_>>().ok()?;
    let provider = font.table_provider(0).ok()?;
    match subset(&provider, &gids, &SubsetProfile::Minimal, CmapTarget::Unicode) {
        Ok(b) => Some(b),
        Err(e) => {
            crate::notes::report(
                crate::notes::Level::Decided,
                "font_not_subset",
                format!("font subsetting failed ({e:?}); embedding the whole font"),
            );
            Some(data.to_vec())
        }
    }
}

/// A subset of `data` with the characters of `chars` (test fixtures).
pub fn subset_for_fixture(data: &[u8], chars: &str) -> Vec<u8> {
    subset_font(data, &chars.chars().collect()).expect("subset")
}

/// Fonts found per (family, weight, italic) in this process (None: not found).
static FOUND: Lazy<Mutex<HashMap<(String, u16, bool), Option<Arc<Vec<u8>>>>>> = Lazy::new(Default::default);

/// Resolve the fonts of the given font stacks.
pub async fn resolve(client: &reqwest::Client, stacks: &[String], opts: &SvgOptions) -> SvgFonts {
    let mut fonts = SvgFonts { by_name: opts.embed_fonts == FontEmbed::None, ..Default::default() };
    let mut key_index: HashMap<(String, u16, bool), usize> = HashMap::new();
    let mut pending: Vec<bool> = Vec::new();
    for stack in stacks {
        let mut idx = Vec::new();
        for name in stack.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (family, weight, italic) = parse_font_name(name);
            let key = (family.clone(), weight, italic);
            let i = match key_index.get(&key) {
                Some(&i) => i,
                None => {
                    fonts.faces.push(FontFace { family: family.clone(), weight, italic, data: None });
                    pending.push(true);
                    key_index.insert(key, fonts.faces.len() - 1);
                    fonts.faces.len() - 1
                }
            };
            idx.push(i);
        }
        // The first font of each stack now: its metrics place the baseline.
        if let Some(&first) = idx.first() {
            if std::mem::take(&mut pending[first]) && opts.embed_fonts != FontEmbed::None {
                let f = &fonts.faces[first];
                let (family, weight, italic) = (f.family.clone(), f.weight, f.italic);
                fonts.faces[first].data = find_font(client, &family, weight, italic, opts).await;
            }
        }
        fonts.stacks.insert(stack.clone(), idx);
    }
    *fonts.used.lock().unwrap() = vec![BTreeSet::new(); fonts.faces.len()];
    *fonts.pending.lock().unwrap() = pending;
    fonts
}

async fn find_font(client: &reqwest::Client, family: &str, weight: u16, italic: bool, opts: &SvgOptions) -> Option<Arc<Vec<u8>>> {
    let key = (family.to_owned(), weight, italic);
    if let Some(hit) = FOUND.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let found = from_dirs(&opts.font_dirs, family, weight, italic)
        .or_else(|| cache_path(family, weight, italic).and_then(|p| std::fs::read(p).ok()))
        .map(Arc::new);
    let found = match found {
        Some(f) => Some(f),
        None if opts.download_fonts => download(client, family, weight, italic).await.map(Arc::new),
        None => None,
    };
    if found.is_none() {
        log::info!("font {family} {weight}{}: no file found; SVG text references it by name", if italic { " italic" } else { "" });
    }
    FOUND.lock().unwrap().insert(key, found.clone());
    found
}

/// A font file in `dirs` whose name table matches (typographic or legacy
/// family, OS/2 weight, italic flag), or whose full name is the font name.
fn from_dirs(dirs: &[PathBuf], family: &str, weight: u16, italic: bool) -> Option<Vec<u8>> {
    let mut files = Vec::new();
    for d in dirs {
        collect_font_files(d, &mut files, 3);
    }
    let mut best: Option<(u32, Vec<u8>)> = None;
    for path in files {
        let Ok(data) = std::fs::read(&path) else { continue };
        let Some(face) = Face::parse(&data) else { continue };
        let names = face.names();
        if !names.iter().any(|n| n.eq_ignore_ascii_case(family) || n.to_ascii_lowercase().starts_with(&family.to_ascii_lowercase())) {
            continue;
        }
        let w = face.weight();
        let score = (w as i32 - weight as i32).unsigned_abs() + if face.is_italic() == italic { 0 } else { 1000 };
        if best.as_ref().map_or(true, |b| score < b.0) {
            best = Some((score, data));
        }
    }
    best.filter(|b| b.0 < 1000).map(|b| b.1)
}

fn collect_font_files(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 {
                collect_font_files(&p, out, depth - 1);
            }
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf")) {
            out.push(p);
        }
    }
}

/// A font family name safe inside the SVG's `<style>` element: quotes,
/// backslashes and markup characters (which could close the element) are
/// dropped. Family names in styles come from untrusted style JSON.
fn css_family(family: &str) -> String {
    family.chars().filter(|c| !matches!(c, '\'' | '"' | '\\' | '<' | '>' | '&' | ';' | '{' | '}')).collect()
}

/// Downloaded fonts live in `BASEMAPPER_FONT_CACHE`, else in the tile
/// cache's directory (so `BASEMAPPER_CACHE_DIR=off` and the R package's
/// `tools::R_user_dir()` apply to fonts too), under `fonts`.
fn cache_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("BASEMAPPER_FONT_CACHE") {
        return Some(PathBuf::from(d));
    }
    crate::cache::dir().map(|d| d.join("fonts"))
}

fn cache_path(family: &str, weight: u16, italic: bool) -> Option<PathBuf> {
    let safe: String = family.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    Some(cache_dir()?.join(format!("{safe}-{weight}{}.ttf", if italic { "i" } else { "" })))
}

/// The TrueType file for a family/weight/style from Google Fonts (cached).
async fn download(client: &reqwest::Client, family: &str, weight: u16, italic: bool) -> Option<Vec<u8>> {
    let g = google_family(family)?;
    let fam = g.replace(' ', "+");
    let urls = [
        format!("https://fonts.googleapis.com/css2?family={fam}:ital,wght@{},{weight}", if italic { 1 } else { 0 }),
        format!("https://fonts.googleapis.com/css2?family={fam}"),
    ];
    for css_url in urls {
        let Ok(resp) = client.get(&css_url).header("User-Agent", "basemapper").send().await else { continue };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(css) = crate::net::read_limited(resp, crate::net::limit::ASSET).await else { continue };
        let css = String::from_utf8_lossy(&css);
        let Some(start) = css.find("url(") else { continue };
        let rest = &css[start + 4..];
        let Some(end) = rest.find(')') else { continue };
        let font_url = rest[..end].trim_matches(|c| c == '\'' || c == '"');
        let Ok(r) = client.get(font_url).send().await else { continue };
        if !r.status().is_success() {
            continue;
        }
        let Ok(bytes) = crate::net::read_limited(r, crate::net::limit::ASSET).await else { continue };
        if Face::parse(&bytes).is_none() {
            continue;
        }
        if let Some(p) = cache_path(family, weight, italic) {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&p, &bytes);
        }
        return Some(bytes);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_names_split_into_family_weight_style() {
        assert_eq!(parse_font_name("Open Sans Bold"), ("Open Sans".into(), 700, false));
        assert_eq!(parse_font_name("Montserrat SemiBold Italic"), ("Montserrat".into(), 600, true));
        assert_eq!(parse_font_name("Open Sans Semibold"), ("Open Sans".into(), 600, false));
        assert_eq!(parse_font_name("Noto Sans CJK JP Regular"), ("Noto Sans CJK JP".into(), 400, false));
        assert_eq!(parse_font_name("Roboto Extra Bold"), ("Roboto".into(), 800, false));
        assert_eq!(parse_font_name("Arial Unicode MS Regular"), ("Arial Unicode MS".into(), 400, false));
        assert_eq!(google_family("Noto Sans CJK JP").as_deref(), Some("Noto Sans JP"));
        assert_eq!(google_family("Arial"), None);
    }

    /// The values ttf-parser 0.25 read from the fixture font, which this
    /// crate used before skrifa: the switch must not change them.
    #[test]
    fn font_reading_matches_ttf_parser() {
        let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fonts/OpenSans-Bold-subset.ttf")).unwrap();
        let f = Face::parse(&data).expect("a font");
        let m = f.metrics();
        assert_eq!((m.units_per_em, m.ascent), (2048, 2189.0));
        assert_eq!((f.weight(), f.is_italic()), (700, false));
        for (c, gid, top) in [('S', 38, 1483.0), ('a', 46, 1141.0), ('l', 57, 1556.0), ('t', 65, 1356.0), ('L', 31, 1462.0),
                              ('k', 56, 1556.0), ('e', 50, 1139.0), ('Q', 36, 1485.0), ('1', 11, 1462.0)] {
            assert_eq!(f.glyph_index(c), Some(gid), "{c:?}");
            assert_eq!(f.glyph_top(gid), Some(top), "{c:?}");
        }
        // A space has a glyph but no outline; é is not in the subset.
        assert_eq!(f.glyph_index(' '), Some(1));
        assert_eq!(f.glyph_top(1), None);
        assert_eq!(f.glyph_index('é'), None);
        let names = f.names();
        assert!(names.contains(&"Open Sans".to_owned()) && names.contains(&"Open Sans Bold".to_owned()), "{names:?}");
        // Not a font: rejected, as ttf-parser rejected it.
        assert!(Face::parse(b"<!doctype html><html>sign in</html>").is_none());
    }

    #[test]
    fn fonts_from_a_folder_are_subset_and_embedded() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fonts");
        let opts = SvgOptions { text: SvgText::Editable, embed_fonts: FontEmbed::Subset, font_dirs: vec![dir], download_fonts: false };
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let client = reqwest::Client::new();
        let stack = "Open Sans Bold,Arial Unicode MS Bold".to_owned();
        let mut fonts = rt.block_on(resolve(&client, &[stack.clone()], &opts));
        assert!(fonts.faces[0].data.is_some(), "Open Sans Bold found in the fixture folder");
        assert!(fonts.faces[1].data.is_none(), "Arial Unicode MS is referenced by name only");
        let attrs = fonts.text_attrs(&stack);
        assert!(attrs.contains("'Open Sans', 'Arial Unicode MS', sans-serif") && attrs.contains(r#"font-weight="700""#), "{attrs}");
        fonts.note_text(&stack, "Salt Lake");
        rt.block_on(fonts.finish(&client, &opts));
        let css = fonts.font_face_css(FontEmbed::Subset);
        assert!(css.starts_with("@font-face{font-family:'Open Sans';font-weight:700;font-style:normal;"), "{}", &css[..80]);
        let b64 = css.split("base64,").nth(1).unwrap().split(')').next().unwrap();
        let bytes = decode_b64(b64);
        let face = Face::parse(&bytes).expect("embedded subset parses");
        for c in "SaltLake".chars() {
            assert!(face.glyph_index(c).is_some(), "{c} kept");
        }
        assert!(face.glyph_index('Q').is_none(), "unused glyphs dropped");
        assert!(fonts.font_face_css(FontEmbed::None).is_empty());
    }

    fn decode_b64(s: &str) -> Vec<u8> {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let val = |c: u8| T.iter().position(|&t| t == c).unwrap() as u32;
        let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
        let mut out = Vec::new();
        for chunk in bytes.chunks(4) {
            let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &c)| acc | (val(c) << (18 - 6 * i)));
            out.push((n >> 16) as u8);
            if chunk.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if chunk.len() > 3 {
                out.push(n as u8);
            }
        }
        out
    }
}
