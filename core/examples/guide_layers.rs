//! Parts of a basemap: list a style's layers with what they show, preview a
//! filter, and render with it.
use basemapper_core::providers::{basemap_provider, ProviderOptions};
use basemapper_core::{list_layers, Basemap, Context, LayerFilter, Labels, Style};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let style = basemap_provider("OpenFreeMap.Liberty", &ProviderOptions::default())?;

    let all = list_layers(&style, None, &LayerFilter::default())?;
    println!("{} layers; the first five:", all.len());
    for l in all.iter().take(5) {
        println!("  {:<22} {:<10} {:<14} {}", l.id, l.kind, l.source_layer.as_deref().unwrap_or("-"), l.feature);
    }

    // Water, parks and roads, without labels.
    let features: Vec<String> = ["water", "parks", "roads"].map(String::from).to_vec();
    let filter = LayerFilter { labels: Labels::None, features: Some(features.clone()), ..Default::default() };
    println!("the filter keeps {} layers", list_layers(&style, None, &filter)?.len());

    // The filters belong to the style: "-labels" drops its labels.
    let parts = Style::new(style).features(["water", "parks", "roads", "-labels"].map(String::from).to_vec());
    let m = Basemap::new(parts).center(-111.8887, 40.7556).zoom(12.4).size(720, 405);
    std::fs::write("layers.png", m.render(&Context::Standalone)?.to_png()?)?;
    Ok(())
}
