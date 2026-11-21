//! MapLibre style-spec values: colors, property functions, expressions, and
//! filters (both legacy and expression syntax).
//!
//! Semantics follow `@maplibre/maplibre-gl-style-spec` so renders can match
//! MapLibre GL JS pixel-for-pixel. Colors are stored premultiplied, as in the
//! style-spec `Color` class, and are interpolated in premultiplied space.

use super::mvt::{Feature, GeomType, Value};
use serde_json::Value as Json;

// ── Color ────────────────────────────────────────────────────────────────────

/// Premultiplied RGBA in 0..=1, like the style-spec `Color`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    pub fn parse(s: &str) -> Option<Color> {
        let c = csscolorparser::parse(s.trim()).ok()?;
        // Premultiply in double precision, as MapLibre's JS Color does,
        // before the float32 conversion a uniform upload performs.
        let a = c.a;
        Some(Color { r: (c.r * a) as f32, g: (c.g * a) as f32, b: (c.b * a) as f32, a: a as f32 })
    }

    pub fn with_opacity(self, o: f32) -> Color {
        Color { r: self.r * o, g: self.g * o, b: self.b * o, a: self.a * o }
    }

    fn lerp(a: Color, b: Color, t: f32) -> Color {
        Color {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }
}

// ── Values ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Color(Color),
    Array(Vec<Val>),
}

impl Val {
    pub fn from_json(j: &Json) -> Val {
        match j {
            Json::Null => Val::Null,
            Json::Bool(b) => Val::Bool(*b),
            Json::Number(n) => Val::Num(n.as_f64().unwrap_or(0.0)),
            Json::String(s) => Val::Str(s.clone()),
            Json::Array(a) => Val::Array(a.iter().map(Val::from_json).collect()),
            Json::Object(_) => Val::Null,
        }
    }

    fn from_prop(v: &Value) -> Val {
        match v {
            Value::String(s) => Val::Str(s.clone()),
            Value::Number(n) => Val::Num(*n),
            Value::Bool(b) => Val::Bool(*b),
        }
    }

    pub fn as_num(&self) -> Option<f64> {
        match self {
            Val::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Coerce to a color: Color values pass through, strings are parsed.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Val::Color(c) => Some(*c),
            Val::Str(s) => Color::parse(s),
            _ => None,
        }
    }

    pub fn as_num_array(&self) -> Option<Vec<f64>> {
        match self {
            Val::Array(a) => a.iter().map(|v| v.as_num()).collect(),
            _ => None,
        }
    }

    fn truthy(&self) -> bool {
        matches!(self, Val::Bool(true))
    }

    fn to_string_val(&self) -> String {
        match self {
            Val::Null => String::new(),
            Val::Bool(b) => b.to_string(),
            Val::Num(n) => format_num(*n),
            Val::Str(s) => s.clone(),
            Val::Color(c) => {
                let a = c.a.max(1e-9);
                format!(
                    "rgba({},{},{},{})",
                    (c.r / a * 255.0).round(),
                    (c.g / a * 255.0).round(),
                    (c.b / a * 255.0).round(),
                    c.a
                )
            }
            Val::Array(_) => String::new(),
        }
    }
}

fn format_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

// ── Evaluation context ───────────────────────────────────────────────────────

pub struct Ctx<'a> {
    pub zoom: f64,
    pub feature: Option<&'a Feature>,
}

impl Ctx<'_> {
    fn prop(&self, key: &str) -> Val {
        self.feature
            .and_then(|f| f.properties.get(key))
            .map(Val::from_prop)
            .unwrap_or(Val::Null)
    }

    fn geom_type(&self) -> Val {
        match self.feature.map(|f| f.geom_type) {
            Some(GeomType::Point) => Val::Str("Point".into()),
            Some(GeomType::LineString) => Val::Str("LineString".into()),
            Some(GeomType::Polygon) => Val::Str("Polygon".into()),
            None => Val::Null,
        }
    }

    fn id(&self) -> Val {
        self.feature.and_then(|f| f.id).map(|i| Val::Num(i as f64)).unwrap_or(Val::Null)
    }
}

// ── Property values (constant | legacy function | expression) ───────────────

/// Kind of value a property expects; affects legacy-function defaults and
/// how string constants are coerced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Number,
    Color,
    Array,
    Enum,
}

/// Evaluate a paint/layout property value.
pub fn eval_property(j: &Json, kind: Kind, ctx: &Ctx) -> Val {
    let v = match j {
        Json::Object(_) => eval_legacy_function(j, kind, ctx),
        Json::Array(a) if a.first().map_or(false, |f| f.is_string()) && kind != Kind::Array => {
            eval_expr(j, ctx)
        }
        Json::Array(a) if kind == Kind::Array && a.first().map_or(false, |f| f.is_string()) => {
            // Array-valued properties: a string head means an expression
            // (e.g. ["literal", [2, 1]] or ["step", ...]).
            eval_expr(j, ctx)
        }
        _ => Val::from_json(j),
    };
    coerce(v, kind)
}

fn coerce(v: Val, kind: Kind) -> Val {
    match (kind, &v) {
        (Kind::Color, Val::Str(s)) => Color::parse(s).map(Val::Color).unwrap_or(Val::Null),
        _ => v,
    }
}

/// True if evaluating `j` requires feature data (so it can't be hoisted to
/// once per layer).
pub fn is_data_driven(j: &Json) -> bool {
    match j {
        Json::Object(o) => o.contains_key("property"),
        Json::Array(a) => {
            if let Some(Json::String(op)) = a.first() {
                if matches!(
                    op.as_str(),
                    "get" | "has" | "!has" | "geometry-type" | "id" | "properties" | "feature-state"
                ) {
                    return true;
                }
            }
            a.iter().any(is_data_driven)
        }
        _ => false,
    }
}

fn interpolate_val(a: &Val, b: &Val, t: f64) -> Val {
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => Val::Num(x + (y - x) * t),
        (Val::Color(x), Val::Color(y)) => Val::Color(Color::lerp(*x, *y, t as f32)),
        (Val::Str(_), _) | (_, Val::Str(_)) => {
            match (a.as_color(), b.as_color()) {
                (Some(x), Some(y)) => Val::Color(Color::lerp(x, y, t as f32)),
                _ => a.clone(),
            }
        }
        (Val::Array(x), Val::Array(y)) if x.len() == y.len() => {
            Val::Array(x.iter().zip(y).map(|(p, q)| interpolate_val(p, q, t)).collect())
        }
        _ => a.clone(),
    }
}

/// Exponential interpolation factor, as in the style spec.
fn exp_factor(input: f64, base: f64, lower: f64, upper: f64) -> f64 {
    let diff = upper - lower;
    if diff == 0.0 {
        return 0.0;
    }
    let progress = input - lower;
    if base == 1.0 {
        progress / diff
    } else {
        (base.powf(progress) - 1.0) / (base.powf(diff) - 1.0)
    }
}

/// Unit cubic-bezier(x1, y1, x2, y2) solved for y at x = t.
fn cubic_bezier(t: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let (cx, cy) = (3.0 * x1, 3.0 * y1);
    let (bx, by) = (3.0 * (x2 - x1) - cx, 3.0 * (y2 - y1) - cy);
    let (ax, ay) = (1.0 - cx - bx, 1.0 - cy - by);
    let sample_x = |s: f64| ((ax * s + bx) * s + cx) * s;
    let sample_y = |s: f64| ((ay * s + by) * s + cy) * s;
    let deriv_x = |s: f64| (3.0 * ax * s + 2.0 * bx) * s + cx;
    let mut s = t;
    for _ in 0..8 {
        let x = sample_x(s) - t;
        if x.abs() < 1e-6 {
            return sample_y(s);
        }
        let d = deriv_x(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= x / d;
    }
    // Bisection fallback.
    let (mut lo, mut hi) = (0.0, 1.0);
    s = t;
    for _ in 0..32 {
        let x = sample_x(s);
        if (x - t).abs() < 1e-6 {
            break;
        }
        if t > x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    sample_y(s)
}

fn eval_legacy_function(j: &Json, kind: Kind, ctx: &Ctx) -> Val {
    let stops = match j.get("stops").and_then(|s| s.as_array()) {
        Some(s) if !s.is_empty() => s,
        _ => return j.get("default").map(Val::from_json).unwrap_or(Val::Null),
    };
    let interpolatable = matches!(kind, Kind::Number | Kind::Color | Kind::Array);
    let ftype = j
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or(if interpolatable { "exponential" } else { "interval" });
    let base = j.get("base").and_then(|b| b.as_f64()).unwrap_or(1.0);

    // Composite (zoom-and-property) functions: stop inputs are {zoom, value}.
    let is_composite = stops[0].get(0).map_or(false, |s| s.is_object());
    let input = if is_composite {
        // Evaluate the property dimension per zoom stop, then interpolate on zoom.
        let prop = j.get("property").and_then(|p| p.as_str()).unwrap_or("");
        let pval = ctx.prop(prop);
        let mut zooms: Vec<f64> = stops
            .iter()
            .filter_map(|s| s.get(0)?.get("zoom")?.as_f64())
            .collect();
        zooms.dedup();
        let per_zoom: Vec<(f64, Val)> = zooms
            .iter()
            .map(|&z| {
                let inner: Vec<Json> = stops
                    .iter()
                    .filter(|s| s.get(0).and_then(|i| i.get("zoom")).and_then(|v| v.as_f64()) == Some(z))
                    .map(|s| Json::Array(vec![s[0]["value"].clone(), s[1].clone()]))
                    .collect();
                let f = serde_json::json!({ "type": ftype, "base": base, "stops": inner });
                (z, eval_stops(&f, &pval, ftype, base, kind))
            })
            .collect();
        return interpolate_stops(&per_zoom, ctx.zoom, "exponential", base);
    } else if let Some(prop) = j.get("property").and_then(|p| p.as_str()) {
        let v = ctx.prop(prop);
        if ftype == "identity" {
            return coerce(v, kind);
        }
        v
    } else {
        Val::Num(ctx.zoom)
    };
    let out = eval_stops(j, &input, ftype, base, kind);
    if out == Val::Null {
        j.get("default").map(Val::from_json).unwrap_or(Val::Null)
    } else {
        out
    }
}

fn eval_stops(j: &Json, input: &Val, ftype: &str, base: f64, kind: Kind) -> Val {
    let stops: Vec<(Val, Val)> = j["stops"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| Some((Val::from_json(s.get(0)?), coerce(Val::from_json(s.get(1)?), kind))))
                .collect()
        })
        .unwrap_or_default();
    match ftype {
        "categorical" => stops
            .iter()
            .find(|(k, _)| k == input)
            .map(|(_, v)| v.clone())
            .unwrap_or(Val::Null),
        _ => {
            let Some(x) = input.as_num() else { return Val::Null };
            let numeric: Vec<(f64, Val)> =
                stops.into_iter().filter_map(|(k, v)| Some((k.as_num()?, v))).collect();
            interpolate_stops(&numeric, x, ftype, base)
        }
    }
}

fn interpolate_stops(stops: &[(f64, Val)], x: f64, ftype: &str, base: f64) -> Val {
    if stops.is_empty() {
        return Val::Null;
    }
    if x <= stops[0].0 {
        return stops[0].1.clone();
    }
    let last = stops.len() - 1;
    if x >= stops[last].0 {
        return stops[last].1.clone();
    }
    let i = stops.iter().rposition(|(k, _)| *k <= x).unwrap_or(0);
    if ftype == "interval" {
        return stops[i].1.clone();
    }
    let (lo, hi) = (&stops[i], &stops[i + 1]);
    let t = exp_factor(x, base, lo.0, hi.0);
    interpolate_val(&lo.1, &hi.1, t)
}

// ── Expressions ──────────────────────────────────────────────────────────────

fn arg(a: &[Json], i: usize, ctx: &Ctx) -> Val {
    a.get(i).map(|e| eval_expr(e, ctx)).unwrap_or(Val::Null)
}

fn num(a: &[Json], i: usize, ctx: &Ctx) -> f64 {
    arg(a, i, ctx).as_num().unwrap_or(0.0)
}

/// Evaluate an expression. Non-array JSON evaluates to itself.
pub fn eval_expr(j: &Json, ctx: &Ctx) -> Val {
    let a = match j {
        Json::Array(a) if a.first().map_or(false, |f| f.is_string()) => a,
        _ => return Val::from_json(j),
    };
    let op = a[0].as_str().unwrap();
    match op {
        "literal" => a.get(1).map(Val::from_json).unwrap_or(Val::Null),
        "zoom" => Val::Num(ctx.zoom),
        "get" => {
            if a.len() > 2 {
                return Val::Null; // object-scoped get: not supported
            }
            match arg(a, 1, ctx) {
                Val::Str(k) => ctx.prop(&k),
                _ => Val::Null,
            }
        }
        "has" => match arg(a, 1, ctx) {
            Val::Str(k) => Val::Bool(ctx.feature.map_or(false, |f| f.properties.contains_key(&k))),
            _ => Val::Bool(false),
        },
        "!has" => match arg(a, 1, ctx) {
            Val::Str(k) => Val::Bool(!ctx.feature.map_or(false, |f| f.properties.contains_key(&k))),
            _ => Val::Bool(true),
        },
        "geometry-type" => ctx.geom_type(),
        "id" => ctx.id(),
        "==" | "!=" => {
            let eq = arg(a, 1, ctx) == arg(a, 2, ctx);
            Val::Bool(if op == "==" { eq } else { !eq })
        }
        "<" | "<=" | ">" | ">=" => {
            let (x, y) = (arg(a, 1, ctx), arg(a, 2, ctx));
            let ord = match (&x, &y) {
                (Val::Num(p), Val::Num(q)) => p.partial_cmp(q),
                (Val::Str(p), Val::Str(q)) => Some(p.cmp(q)),
                _ => None,
            };
            Val::Bool(match (op, ord) {
                ("<", Some(o)) => o.is_lt(),
                ("<=", Some(o)) => o.is_le(),
                (">", Some(o)) => o.is_gt(),
                (">=", Some(o)) => o.is_ge(),
                _ => false,
            })
        }
        "all" => Val::Bool(a[1..].iter().all(|e| eval_expr(e, ctx).truthy())),
        "any" => Val::Bool(a[1..].iter().any(|e| eval_expr(e, ctx).truthy())),
        "!" => Val::Bool(!arg(a, 1, ctx).truthy()),
        "none" => Val::Bool(!a[1..].iter().any(|e| eval_expr(e, ctx).truthy())),
        "in" => {
            let needle = arg(a, 1, ctx);
            match arg(a, 2, ctx) {
                Val::Array(hay) => Val::Bool(hay.contains(&needle)),
                Val::Str(hay) => Val::Bool(needle.as_str().map_or(false, |n| hay.contains(n))),
                _ => Val::Bool(false),
            }
        }
        "match" => {
            let input = arg(a, 1, ctx);
            let mut i = 2;
            while i + 1 < a.len() {
                let labels = &a[i];
                let hit = match labels {
                    Json::Array(ls) => ls.iter().any(|l| Val::from_json(l) == input),
                    l => Val::from_json(l) == input,
                };
                if hit {
                    return eval_expr(&a[i + 1], ctx);
                }
                i += 2;
            }
            a.last().map(|e| eval_expr(e, ctx)).unwrap_or(Val::Null)
        }
        "case" => {
            let mut i = 1;
            while i + 1 < a.len() {
                if eval_expr(&a[i], ctx).truthy() {
                    return eval_expr(&a[i + 1], ctx);
                }
                i += 2;
            }
            a.last().map(|e| eval_expr(e, ctx)).unwrap_or(Val::Null)
        }
        "coalesce" => a[1..]
            .iter()
            .map(|e| eval_expr(e, ctx))
            .find(|v| *v != Val::Null)
            .unwrap_or(Val::Null),
        "step" => {
            let x = num(a, 1, ctx);
            let mut out = &a[2];
            let mut i = 3;
            while i + 1 < a.len() {
                if a[i].as_f64().map_or(false, |s| x >= s) {
                    out = &a[i + 1];
                } else {
                    break;
                }
                i += 2;
            }
            eval_expr(out, ctx)
        }
        "interpolate" | "interpolate-hcl" | "interpolate-lab" => {
            let interp = a.get(1).and_then(|v| v.as_array());
            let x = num(a, 2, ctx);
            let stops: Vec<(f64, &Json)> = a[3..]
                .chunks_exact(2)
                .filter_map(|c| Some((c[0].as_f64()?, &c[1])))
                .collect();
            if stops.is_empty() {
                return Val::Null;
            }
            if x <= stops[0].0 {
                return eval_expr(stops[0].1, ctx);
            }
            let last = stops.len() - 1;
            if x >= stops[last].0 {
                return eval_expr(stops[last].1, ctx);
            }
            let i = stops.iter().rposition(|(k, _)| *k <= x).unwrap_or(0);
            let (lo, hi) = (stops[i], stops[i + 1]);
            let kind = interp.and_then(|v| v.first()).and_then(|v| v.as_str()).unwrap_or("linear");
            let t = match kind {
                "exponential" => {
                    let base = interp.and_then(|v| v.get(1)).and_then(|v| v.as_f64()).unwrap_or(1.0);
                    exp_factor(x, base, lo.0, hi.0)
                }
                "cubic-bezier" => {
                    let c: Vec<f64> =
                        interp.map(|v| v[1..].iter().filter_map(|n| n.as_f64()).collect()).unwrap_or_default();
                    let lin = exp_factor(x, 1.0, lo.0, hi.0);
                    if c.len() == 4 {
                        cubic_bezier(lin, c[0], c[1], c[2], c[3])
                    } else {
                        lin
                    }
                }
                _ => exp_factor(x, 1.0, lo.0, hi.0),
            };
            interpolate_val(&eval_expr(lo.1, ctx), &eval_expr(hi.1, ctx), t)
        }
        "+" => Val::Num(a[1..].iter().map(|e| eval_expr(e, ctx).as_num().unwrap_or(0.0)).sum()),
        "*" => Val::Num(a[1..].iter().map(|e| eval_expr(e, ctx).as_num().unwrap_or(0.0)).product()),
        "-" => {
            if a.len() == 2 {
                Val::Num(-num(a, 1, ctx))
            } else {
                Val::Num(num(a, 1, ctx) - num(a, 2, ctx))
            }
        }
        "/" => Val::Num(num(a, 1, ctx) / num(a, 2, ctx)),
        "%" => Val::Num(num(a, 1, ctx) % num(a, 2, ctx)),
        "^" => Val::Num(num(a, 1, ctx).powf(num(a, 2, ctx))),
        "min" => Val::Num(a[1..].iter().filter_map(|e| eval_expr(e, ctx).as_num()).fold(f64::INFINITY, f64::min)),
        "max" => Val::Num(a[1..].iter().filter_map(|e| eval_expr(e, ctx).as_num()).fold(f64::NEG_INFINITY, f64::max)),
        "abs" => Val::Num(num(a, 1, ctx).abs()),
        "round" => Val::Num(num(a, 1, ctx).round()),
        "floor" => Val::Num(num(a, 1, ctx).floor()),
        "ceil" => Val::Num(num(a, 1, ctx).ceil()),
        "sqrt" => Val::Num(num(a, 1, ctx).sqrt()),
        "ln" => Val::Num(num(a, 1, ctx).ln()),
        "log10" => Val::Num(num(a, 1, ctx).log10()),
        "log2" => Val::Num(num(a, 1, ctx).log2()),
        "pi" => Val::Num(std::f64::consts::PI),
        "e" => Val::Num(std::f64::consts::E),
        "to-number" => {
            for e in &a[1..] {
                match eval_expr(e, ctx) {
                    Val::Num(n) => return Val::Num(n),
                    Val::Str(s) => {
                        if let Ok(n) = s.trim().parse() {
                            return Val::Num(n);
                        }
                    }
                    Val::Bool(b) => return Val::Num(if b { 1.0 } else { 0.0 }),
                    Val::Null => return Val::Num(0.0),
                    _ => {}
                }
            }
            Val::Num(0.0)
        }
        "to-string" => Val::Str(arg(a, 1, ctx).to_string_val()),
        "to-boolean" => Val::Bool(match arg(a, 1, ctx) {
            Val::Bool(b) => b,
            Val::Num(n) => n != 0.0 && !n.is_nan(),
            Val::Str(s) => !s.is_empty(),
            Val::Null => false,
            _ => true,
        }),
        "to-color" => a[1..]
            .iter()
            .find_map(|e| eval_expr(e, ctx).as_color())
            .map(Val::Color)
            .unwrap_or(Val::Color(Color::BLACK)),
        "rgb" | "rgba" => {
            let alpha = if op == "rgba" { num(a, 4, ctx) as f32 } else { 1.0 };
            Val::Color(Color {
                r: num(a, 1, ctx) as f32 / 255.0 * alpha,
                g: num(a, 2, ctx) as f32 / 255.0 * alpha,
                b: num(a, 3, ctx) as f32 / 255.0 * alpha,
                a: alpha,
            })
        }
        "concat" => Val::Str(a[1..].iter().map(|e| eval_expr(e, ctx).to_string_val()).collect()),
        "downcase" => Val::Str(arg(a, 1, ctx).to_string_val().to_lowercase()),
        "upcase" => Val::Str(arg(a, 1, ctx).to_string_val().to_uppercase()),
        "length" => match arg(a, 1, ctx) {
            Val::Str(s) => Val::Num(s.chars().count() as f64),
            Val::Array(v) => Val::Num(v.len() as f64),
            _ => Val::Null,
        },
        "number" | "string" | "boolean" | "array" | "object" => {
            // Type assertions: pass through the first argument.
            arg(a, if op == "array" && a.len() > 2 { a.len() - 1 } else { 1 }, ctx)
        }
        "let" | "var" => Val::Null, // not yet supported
        _ => {
            log::debug!("unsupported expression operator: {op}");
            Val::Null
        }
    }
}

// ── Filters ──────────────────────────────────────────────────────────────────

/// MapLibre's `isExpressionFilter`: decides legacy vs expression syntax.
pub fn is_expression_filter(f: &Json) -> bool {
    match f {
        Json::Bool(_) => true,
        Json::Array(a) if !a.is_empty() => match a[0].as_str() {
            Some("has") => a.len() >= 2 && a[1] != "$id" && a[1] != "$type",
            Some("in") => a.len() >= 3 && (!a[1].is_string() || a[2].is_array()),
            Some("!in") | Some("!has") | Some("none") => false,
            Some("==") | Some("!=") | Some(">") | Some(">=") | Some("<") | Some("<=") => {
                a.len() != 3 || a[1].is_array() || a[2].is_array()
            }
            Some("any") | Some("all") => {
                a[1..].iter().all(|x| is_expression_filter(x) || x.is_boolean())
            }
            _ => true,
        },
        _ => false,
    }
}

/// Evaluate a layer filter (legacy or expression) against a feature.
pub fn eval_filter(f: &Json, ctx: &Ctx) -> bool {
    if is_expression_filter(f) {
        eval_expr(f, ctx).truthy()
    } else {
        eval_legacy_filter(f, ctx)
    }
}

fn legacy_key(key: &Json, ctx: &Ctx) -> Val {
    match key.as_str() {
        Some("$type") => ctx.geom_type(),
        Some("$id") => ctx.id(),
        Some(k) => ctx.prop(k),
        None => Val::Null,
    }
}

fn eval_legacy_filter(f: &Json, ctx: &Ctx) -> bool {
    let Some(a) = f.as_array() else { return true };
    let Some(op) = a.first().and_then(|o| o.as_str()) else { return true };
    match op {
        "all" => a[1..].iter().all(|x| eval_legacy_filter(x, ctx)),
        "any" => a[1..].iter().any(|x| eval_legacy_filter(x, ctx)),
        "none" => !a[1..].iter().any(|x| eval_legacy_filter(x, ctx)),
        "has" => a.get(1).map_or(false, |k| legacy_key(k, ctx) != Val::Null),
        "!has" => a.get(1).map_or(true, |k| legacy_key(k, ctx) == Val::Null),
        "in" | "!in" => {
            let v = a.get(1).map(|k| legacy_key(k, ctx)).unwrap_or(Val::Null);
            let found = a[2..].iter().any(|x| Val::from_json(x) == v);
            if op == "in" { found } else { !found }
        }
        "==" | "!=" | "<" | "<=" | ">" | ">=" => {
            let v = a.get(1).map(|k| legacy_key(k, ctx)).unwrap_or(Val::Null);
            let w = a.get(2).map(Val::from_json).unwrap_or(Val::Null);
            match op {
                "==" => v == w,
                "!=" => v != w,
                _ => {
                    let ord = match (&v, &w) {
                        (Val::Num(p), Val::Num(q)) => p.partial_cmp(q),
                        (Val::Str(p), Val::Str(q)) => Some(p.cmp(q)),
                        _ => None,
                    };
                    match (op, ord) {
                        ("<", Some(o)) => o.is_lt(),
                        ("<=", Some(o)) => o.is_le(),
                        (">", Some(o)) => o.is_gt(),
                        (">=", Some(o)) => o.is_ge(),
                        _ => false,
                    }
                }
            }
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn feat(props: &[(&str, Value)], t: GeomType) -> Feature {
        Feature {
            id: Some(7),
            geom_type: t,
            properties: props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<HashMap<_, _>>(),
            geometry: super::super::mvt::Geometry::Points(vec![]),
        }
    }

    #[test]
    fn color_parse_premultiplies() {
        let c = Color::parse("rgba(206,217,184,0.5)").unwrap();
        assert!((c.a - 0.5).abs() < 1e-6);
        assert!((c.r - 206.0 / 255.0 * 0.5).abs() < 1e-4);
        assert_eq!(Color::parse("#fff").unwrap(), Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 });
    }

    #[test]
    fn legacy_filter_symbol_equality() {
        let f = feat(&[("_symbol", Value::Number(0.0))], GeomType::Polygon);
        let ctx = Ctx { zoom: 13.0, feature: Some(&f) };
        let filt = json!(["==", "_symbol", 0]);
        assert!(!is_expression_filter(&filt));
        assert!(eval_filter(&filt, &ctx));
        assert!(!eval_filter(&json!(["==", "_symbol", 1]), &ctx));
        assert!(eval_filter(&json!(["==", "$type", "Polygon"]), &ctx));
        assert!(eval_filter(&json!(["all", ["==", "_symbol", 0], ["!in", "x", 1, 2]]), &ctx));
    }

    #[test]
    fn expression_filter_and_match() {
        let f = feat(&[("class", Value::String("motorway".into()))], GeomType::LineString);
        let ctx = Ctx { zoom: 13.0, feature: Some(&f) };
        let filt = json!(["match", ["get", "class"], ["motorway", "trunk"], true, false]);
        assert!(is_expression_filter(&filt));
        assert!(eval_filter(&filt, &ctx));
        assert!(eval_filter(&json!(["==", ["geometry-type"], "LineString"]), &ctx));
    }

    #[test]
    fn interpolate_and_legacy_stops() {
        let ctx = Ctx { zoom: 12.5, feature: None };
        let e = json!(["interpolate", ["linear"], ["zoom"], 12, 1, 13, 3]);
        assert_eq!(eval_property(&e, Kind::Number, &ctx), Val::Num(2.0));
        let lf = json!({"stops": [[12, 1], [13, 3]]});
        assert_eq!(eval_property(&lf, Kind::Number, &ctx), Val::Num(2.0));
        let base2 = json!({"base": 2, "stops": [[12, 0], [13, 3]]});
        let v = eval_property(&base2, Kind::Number, &ctx).as_num().unwrap();
        assert!((v - 3.0 * (2f64.sqrt() - 1.0)).abs() < 1e-9);
        let step = json!(["step", ["zoom"], 0, 12, 5, 14, 9]);
        assert_eq!(eval_property(&step, Kind::Number, &ctx), Val::Num(5.0));
    }

    #[test]
    fn color_interpolation_is_premultiplied() {
        let ctx = Ctx { zoom: 1.0, feature: None };
        let e = json!(["interpolate", ["linear"], ["zoom"], 0, "rgba(255,0,0,0)", 2, "rgba(255,0,0,1)"]);
        let c = eval_property(&e, Kind::Color, &ctx).as_color().unwrap();
        assert!((c.a - 0.5).abs() < 1e-6 && (c.r - 0.5).abs() < 1e-6);
    }
}
