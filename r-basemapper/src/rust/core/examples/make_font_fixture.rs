//! Writes a small subset of a font (ASCII letters, digits, punctuation) for
//! the font tests: cargo run -p basemapper-core --example make_font_fixture -- in.ttf out.ttf
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("read font");
    let fonts = basemapper_core::vector::fonts::subset_for_fixture(&data, " ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.,'-/()&");
    std::fs::write(&args[2], fonts).expect("write font");
}
