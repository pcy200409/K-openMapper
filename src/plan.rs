//! A plan of the map as an SVG or PNG picture (File > Export a plan of the map, Export a line
//! map): its tiles, water, roads, tracks and footpaths seen from above, the bus lines' ways in
//! colour with a key, the stops with their names, a scale bar and north. To share with players
//! or print. A line map leaves the ground out: the lines and their stops alone.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use glam::{DVec2, DVec3};

use crate::doc::{tile_origin, tile_size, Document};
use crate::i18n::tr;
use crate::library::{Category, Library};

/// The colours of the lines, in turn.
const LINE_COLOURS: [&str; 10] = ["#d7263d", "#1b78c8", "#2e933c", "#f08a24", "#7b2cbf", "#0f7173", "#c2185b", "#5c6bc0", "#8d6e63", "#b8a200"];

/// What a picture shows.
#[derive(Debug, Clone)]
pub struct Options {
    /// The map's ground, water and roads under the lines (else the lines alone).
    pub background: bool,
    /// Only this line (else all of them).
    pub line: Option<String>,
}

impl Default for Options {
    fn default() -> Options {
        Options { background: true, line: None }
    }
}

/// What is drawn, in picture units (x right, y down), in the order drawn.
enum Prim {
    Rect { x: f64, y: f64, w: f64, h: f64, rx: f64, fill: &'static str },
    /// A line with round ends and joins; a dash of (on, off).
    Line { pts: Vec<(f64, f64)>, colour: String, width: f64, opacity: f64, dash: Option<(f64, f64)> },
    Poly { pts: Vec<(f64, f64)>, fill: &'static str },
    Circle { x: f64, y: f64, r: f64, fill: &'static str, stroke: (&'static str, f64) },
    /// A text on its baseline, left or centred; a halo round it of (colour, width).
    Text { x: f64, y: f64, size: f64, text: String, fill: String, bold: bool, centred: bool, halo: Option<(&'static str, f64)> },
}

/// A picture of the map: its size and what is drawn.
pub struct Picture {
    w: f64,
    h: f64,
    prims: Vec<Prim>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The map's plan; `roots`: where the stops' models are found (the lines' ways go by them).
pub fn svg(doc: &Document, lib: &Library, roots: &[PathBuf]) -> String {
    to_svg(&picture(doc, lib, roots, &Options::default()))
}

/// A stop of a line: where it stands and its name (the timetable's, the trip's or the
/// object's label).
fn line_stop(doc: &Document, files: &mut crate::timetable::network::Files, id: i64, own: Option<&str>) -> Option<(DVec2, String)> {
    let (k, o) = doc.tiles.iter().find_map(|(k, t)| t.tile.objects.iter().find(|o| o.id == id).map(|o| (*k, o)))?;
    let abs = files.get(&o.file).map(|f| f.abs_height).unwrap_or(false);
    let p = doc.object_world(k, o, abs).truncate();
    let name = doc.timetable.stop_name(id).map(str::to_string).filter(|n| !n.is_empty()).or_else(|| own.map(str::to_string).filter(|n| !n.is_empty())).or_else(|| o.extra.first().cloned()).unwrap_or_default();
    Some((p, name.replace('@', " ")))
}

/// What the picture shows, laid out.
pub fn picture(doc: &Document, lib: &Library, roots: &[PathBuf], opts: &Options) -> Picture {
    let ts = tile_size();
    if doc.tiles.is_empty() {
        return Picture { w: 1.0, h: 1.0, prims: Vec::new() };
    }
    // the lines' ways (each trip's way along its track, else along the roads) and stops
    let tt = &doc.timetable;
    let mut ways: Vec<(String, &'static str, Vec<Vec<DVec2>>)> = Vec::new();
    let mut line_stops: Vec<(i64, DVec2, String)> = Vec::new();
    if !tt.trips.is_empty() {
        let mut files = crate::timetable::network::Files::new(roots.to_vec());
        let net = crate::timetable::network::Network::build(doc, &mut files);
        for (li, line) in tt.lines.iter().enumerate() {
            if opts.line.as_ref().is_some_and(|l| !l.eq_ignore_ascii_case(&line.name)) {
                continue;
            }
            let colour = LINE_COLOURS[li % LINE_COLOURS.len()];
            let mut drawn = Vec::new();
            for name in crate::timetable::route::line_trips(tt, line) {
                let Some(trip) = tt.trip(&name) else { continue };
                let mut at: Vec<DVec3> = Vec::new();
                for (id, own) in crate::timetable::route::trip_stations(trip) {
                    let Some(p) = crate::timetable::stop_world(doc, &mut files, id) else { continue };
                    at.push(p);
                    if !line_stops.iter().any(|s| s.0 == id) {
                        if let Some((q, n)) = line_stop(doc, &mut files, id, own.as_deref()) {
                            line_stops.push((id, q, n));
                        }
                    }
                }
                let way: Vec<DVec2> = crate::timetable::route::trip_way(doc, &net, trip, &at).iter().map(|p| p.truncate()).collect();
                if way.len() >= 2 {
                    drawn.push(way);
                }
            }
            if !drawn.is_empty() {
                ways.push((line.name.clone(), colour, drawn));
            }
        }
    }
    // the picture's extent: the map's tiles, or (the lines alone) the lines
    let tiles_box = doc.tiles.keys().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), k| (a.min(tile_origin(*k)), b.max(tile_origin(*k) + DVec2::splat(ts))));
    let lines_box = ways.iter().flat_map(|w| w.2.iter().flatten()).chain(line_stops.iter().map(|s| &s.1)).fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
    let plain = !opts.background && lines_box.0.x < lines_box.1.x;
    let (lo, hi) = if plain { lines_box } else { tiles_box };
    // (room round the map for the title, the key and the scale: they grow with the picture)
    let margin = if plain { ((hi - lo).max_element() * 0.06).max(60.0) } else { ((hi - lo).max_element() * 0.03).max(40.0) };
    let (w, h) = (hi.x - lo.x + 2.0 * margin, hi.y - lo.y + 2.0 * margin);
    // (north up: the picture's y runs down)
    let flip = |p: DVec2| (p.x - lo.x + margin, hi.y - p.y + margin);
    let font = (w.max(h) / 160.0).clamp(5.0, 40.0);
    // (the title, the key, the scale and north: read at the size of the whole picture)
    let big = (w.max(h) / 90.0).clamp(font, 120.0);
    let mut out: Vec<Prim> = Vec::new();
    out.push(Prim::Rect { x: 0.0, y: 0.0, w, h, rx: 0.0, fill: "#f7f6f2" });
    if !plain {
        // the tiles
        for k in doc.tiles.keys() {
            let (x, y) = flip(tile_origin(*k) + DVec2::new(0.0, ts));
            out.push(Prim::Rect { x, y, w: ts, h: ts, rx: 0.0, fill: "#e7ecdc" });
        }
        for k in doc.tiles.keys() {
            let (x, y) = flip(tile_origin(*k) + DVec2::new(0.0, ts));
            out.push(Prim::Line { pts: vec![(x, y), (x + ts, y), (x + ts, y + ts), (x, y + ts), (x, y)], colour: "#d6dccb".into(), width: 0.5, opacity: 1.0, dash: None });
        }
        // water: where a tile's ground lies below its water, in 5 m cells
        let cell = 5.0;
        let n = (ts / cell).round() as usize;
        for (k, t) in &doc.tiles {
            let Some(wl) = t.water else { continue };
            let level = (wl[0] + wl[1] + wl[2] + wl[3]) / 4.0;
            let org = tile_origin(*k);
            for j in 0..n {
                let mut run: Option<usize> = None;
                for i in 0..=n {
                    let wet = i < n && t.height_at((i as f64 + 0.5) * cell, (j as f64 + 0.5) * cell) < level;
                    match (wet, run) {
                        (true, None) => run = Some(i),
                        (false, Some(a)) => {
                            let (x, y) = flip(org + DVec2::new(a as f64 * cell, (j + 1) as f64 * cell));
                            out.push(Prim::Rect { x, y, w: (i - a) as f64 * cell, h: cell + 0.2, rx: 0.0, fill: "#a9cfe8" });
                            run = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        // the splines by their kind
        let mut roads: Vec<(Vec<DVec2>, f64)> = Vec::new();
        let mut tracks: Vec<Vec<DVec2>> = Vec::new();
        let mut paths: Vec<Vec<DVec2>> = Vec::new();
        for (k, t) in &doc.tiles {
            for sp in t.tile.splines.iter().filter(|s| !s.deleted) {
                let c = omsi_geometry::SplineCurve::from_map(sp, tile_origin(*k));
                let m = (sp.length / 4.0).ceil().max(1.0) as usize;
                let pts: Vec<DVec2> = (0..=m).map(|i| c.point_at(sp.length * i as f64 / m as f64).truncate()).collect();
                match lib.find(&sp.file).map(|a| (a.category, a.half_width)) {
                    Some((Category::Roads, hw)) => roads.push((pts, (hw as f64).max(2.0))),
                    Some((Category::Tracks, _)) => tracks.push(pts),
                    Some((Category::Paths, _)) => paths.push(pts),
                    _ => {}
                }
            }
        }
        let line = |pts: &[DVec2], colour: &str, width: f64, dash: Option<(f64, f64)>| Prim::Line { pts: pts.iter().map(|p| flip(*p)).collect(), colour: colour.into(), width, opacity: 1.0, dash };
        out.extend(paths.iter().map(|p| line(p, "#c8b98f", 1.2, None)));
        // (each road its own width: an outline under, the road over it)
        out.extend(roads.iter().map(|(p, hw)| line(p, "#a7a59d", hw * 2.0 + 1.6, None)));
        out.extend(roads.iter().map(|(p, hw)| line(p, "#ffffff", hw * 2.0, None)));
        out.extend(tracks.iter().map(|p| line(p, "#5a5a5a", 1.6, Some((6.0, 3.0)))));
    }
    // the lines' ways
    let (width, opacity) = if plain { ((font * 0.55).max(3.0), 0.9) } else { ((font * 0.45).max(2.5), 0.75) };
    for (_, colour, drawn) in &ways {
        for way in drawn {
            out.push(Prim::Line { pts: way.iter().map(|p| flip(*p)).collect(), colour: colour.to_string(), width, opacity, dash: None });
        }
    }
    // the stops, each name once over its stops (the two sides of a road): the map's own, or
    // with the lines alone those of the lines
    let stops: Vec<(String, DVec2)> = if plain {
        line_stops.iter().map(|s| (s.2.clone(), s.1)).collect()
    } else {
        doc.tiles.iter().flat_map(|(k, t)| t.tile.objects.iter().filter(|o| o.flag == 7 && o.extra.len() >= 4).map(move |o| (o.extra[0].replace('@', " "), tile_origin(*k) + DVec2::new(o.pos[0], o.pos[1])))).collect()
    };
    // (the stops of either side, "Rathaus A" and "Rathaus B", are named once: "Rathaus")
    let base = |n: &str| -> String {
        let n = n.trim();
        match n.rsplit_once(' ') {
            Some((head, tail)) if tail.chars().count() == 1 && tail.chars().all(|c| c.is_ascii_uppercase()) && !head.trim().is_empty() => head.trim().to_string(),
            _ => n.to_string(),
        }
    };
    let mut groups: Vec<(String, Vec<DVec2>)> = Vec::new();
    for (name, p) in &stops {
        let name = base(name);
        match groups.iter_mut().find(|g| g.0 == name && g.1.iter().any(|q| (*q - *p).length() < 80.0)) {
            Some(g) => g.1.push(*p),
            None => groups.push((name, vec![*p])),
        }
    }
    let r = (font * 0.35).max(2.0);
    for (_, p) in &stops {
        let (x, y) = flip(*p);
        out.push(Prim::Circle { x, y, r, fill: "#ffffff", stroke: ("#222222", 0.8) });
    }
    // (a name that would lie on one already put is left out)
    let mut taken: Vec<[f64; 4]> = Vec::new();
    for (name, ps) in &groups {
        let c = ps.iter().copied().sum::<DVec2>() / ps.len() as f64;
        let (x, y) = flip(c);
        let y = y - r - font * 0.4;
        let half = name.chars().map(|ch| if ch.is_ascii() { 0.55 } else { 1.0 }).sum::<f64>() * font * 0.5;
        let b = [x - half, y - font, x + half, y + font * 0.25];
        if taken.iter().any(|t| b[0] < t[2] && b[2] > t[0] && b[1] < t[3] && b[3] > t[1]) {
            continue;
        }
        taken.push(b);
        out.push(Prim::Text { x, y, size: font, text: name.clone(), fill: "#222222".into(), bold: false, centred: true, halo: Some(("#f7f6f2", font * 0.25)) });
    }
    // the title, the lines' key, the scale and north
    let name = if doc.global.friendly_name.is_empty() { doc.global.name.clone() } else { doc.global.friendly_name.clone() };
    let left = margin * 0.5;
    out.push(Prim::Text { x: left, y: margin * 0.5 + big * 1.4, size: big * 1.6, text: name, fill: "#222222".into(), bold: true, centred: false, halo: Some(("#f7f6f2", big * 0.3)) });
    for (i, (n, c, _)) in ways.iter().enumerate() {
        let y = margin * 0.5 + big * (3.2 + 1.5 * i as f64);
        let bw = (big * 2.6).max(big * (0.8 + 0.62 * n.chars().count() as f64));
        out.push(Prim::Rect { x: left, y: y - big, w: bw, h: big * 1.25, rx: big * 0.25, fill: c });
        out.push(Prim::Text { x: left + bw * 0.5, y, size: big, text: n.clone(), fill: "#ffffff".into(), bold: true, centred: true, halo: None });
    }
    // (a round length about a fifth of the picture)
    let bar = [10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0].into_iter().rfind(|b| *b <= (w - 2.0 * margin) / 5.0).unwrap_or(10.0);
    let (bx, by) = (w - margin * 0.5 - bar, h - margin * 0.5);
    let label = if bar >= 1000.0 { format!("{} km", bar / 1000.0) } else { format!("{bar} m") };
    let tick = big * 0.4;
    for pts in [vec![(bx, by), (bx + bar, by)], vec![(bx, by - tick), (bx, by)], vec![(bx + bar, by - tick), (bx + bar, by)]] {
        out.push(Prim::Line { pts, colour: "#222222".into(), width: big * 0.15, opacity: 1.0, dash: None });
    }
    out.push(Prim::Text { x: bx + bar / 2.0, y: by - big * 0.6, size: big, text: label, fill: "#222222".into(), bold: false, centred: true, halo: None });
    let (nx, ny) = (w - margin * 0.5 - big, margin * 0.5 + big * 2.5);
    out.push(Prim::Poly { pts: vec![(nx, ny - big * 1.6), (nx + big * 0.6, ny), (nx, ny - big * 0.4), (nx - big * 0.6, ny)], fill: "#222222" });
    out.push(Prim::Text { x: nx, y: ny - big * 1.9, size: big, text: "N".into(), fill: "#222222".into(), bold: true, centred: true, halo: None });
    out.push(Prim::Text { x: left, y: h - margin * 0.5, size: big * 0.7, text: tr("Made with openMapper", "Сделано в openMapper").into(), fill: "#8a8a8a".into(), bold: false, centred: false, halo: None });
    Picture { w, h, prims: out }
}

/// The picture as SVG (some 2400 pixels along its longer side when opened as a picture).
pub fn to_svg(p: &Picture) -> String {
    let (w, h) = (p.w, p.h);
    let px = 2400.0 / w.max(h);
    let mut s = String::new();
    let _ = writeln!(s, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0} {h:.0}" width="{:.0}" height="{:.0}" font-family="Malgun Gothic, Apple SD Gothic Neo, Noto Sans KR, Helvetica, Arial, sans-serif">"#, w * px, h * px);
    for prim in &p.prims {
        match prim {
            Prim::Rect { x, y, w, h, rx, fill } => {
                let _ = writeln!(s, r#"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="{rx:.1}" fill="{fill}"/>"#);
            }
            Prim::Line { pts, colour, width, opacity, dash } => {
                let mut d = String::new();
                for (i, (x, y)) in pts.iter().enumerate() {
                    let _ = write!(d, "{}{x:.1},{y:.1}", if i == 0 { "M" } else { " L" });
                }
                let dash = dash.map(|(a, b)| format!(r#" stroke-dasharray="{a} {b}""#)).unwrap_or_default();
                let _ = writeln!(s, r#"<path d="{d}" fill="none" stroke="{colour}" stroke-width="{width:.1}" stroke-opacity="{opacity}" stroke-linecap="round" stroke-linejoin="round"{dash}/>"#);
            }
            Prim::Poly { pts, fill } => {
                let d: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
                let _ = writeln!(s, r#"<polygon points="{}" fill="{fill}"/>"#, d.join(" "));
            }
            Prim::Circle { x, y, r, fill, stroke } => {
                let _ = writeln!(s, r#"<circle cx="{x:.1}" cy="{y:.1}" r="{r:.1}" fill="{fill}" stroke="{}" stroke-width="{}"/>"#, stroke.0, stroke.1);
            }
            Prim::Text { x, y, size, text, fill, bold, centred, halo } => {
                let halo = halo.map(|(c, w)| format!(r#" stroke="{c}" stroke-width="{w:.1}" paint-order="stroke""#)).unwrap_or_default();
                let _ = writeln!(s, r#"<text x="{x:.1}" y="{y:.1}" font-size="{size:.1}" fill="{fill}"{}{}{halo}>{}</text>"#, if *bold { r#" font-weight="bold""# } else { "" }, if *centred { r#" text-anchor="middle""# } else { "" }, esc(text));
            }
        }
    }
    s.push_str("</svg>\n");
    s
}

/// `#rrggbb` with an opacity.
fn colour(hex: &str, opacity: f64) -> tiny_skia::Color {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    tiny_skia::Color::from_rgba8((v >> 16) as u8, (v >> 8) as u8, v as u8, (opacity.clamp(0.0, 1.0) * 255.0) as u8)
}

fn paint(c: tiny_skia::Color) -> tiny_skia::Paint<'static> {
    let mut p = tiny_skia::Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

/// The font the PNG's texts are drawn in: the system's Korean one (it has Latin too), else
/// the interface's.
fn png_font() -> Option<&'static ab_glyph::FontVec> {
    static F: std::sync::OnceLock<Option<ab_glyph::FontVec>> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let bytes = crate::icons::korean_font().or_else(|| {
            let defs = egui::FontDefinitions::default();
            ["Ubuntu-Light", "NotoSans-Regular"].iter().find_map(|n| defs.font_data.get(*n)).map(|d| d.font.to_vec())
        })?;
        ab_glyph::FontVec::try_from_vec(bytes).ok()
    })
    .as_ref()
}

/// A text as an outline `px` high: its path with the baseline at y = 0, and its width.
fn text_path(f: &impl ab_glyph::Font, s: &str, px: f32) -> Option<(tiny_skia::Path, f32)> {
    use ab_glyph::OutlineCurve;
    let k = px / f.units_per_em()?;
    let mut pb = tiny_skia::PathBuilder::new();
    let mut pen = 0.0f32;
    for c in s.chars() {
        let id = f.glyph_id(c);
        if let Some(o) = f.outline(id) {
            let mut last: Option<(f32, f32)> = None;
            let at = |p: ab_glyph::Point| (pen + p.x * k, -p.y * k);
            for curve in &o.curves {
                let (start, end) = match *curve {
                    OutlineCurve::Line(a, b) => (a, b),
                    OutlineCurve::Quad(a, _, c) => (a, c),
                    OutlineCurve::Cubic(a, _, _, d) => (a, d),
                };
                let s0 = at(start);
                if last != Some(s0) {
                    if last.is_some() {
                        pb.close();
                    }
                    pb.move_to(s0.0, s0.1);
                }
                match *curve {
                    OutlineCurve::Line(_, b) => {
                        let b = at(b);
                        pb.line_to(b.0, b.1);
                    }
                    OutlineCurve::Quad(_, b, c) => {
                        let (b, c) = (at(b), at(c));
                        pb.quad_to(b.0, b.1, c.0, c.1);
                    }
                    OutlineCurve::Cubic(_, b, c, d) => {
                        let (b, c, d) = (at(b), at(c), at(d));
                        pb.cubic_to(b.0, b.1, c.0, c.1, d.0, d.1);
                    }
                }
                last = Some(at(end));
            }
            if last.is_some() {
                pb.close();
            }
        }
        pen += f.h_advance_unscaled(id) * k;
    }
    Some((pb.finish()?, pen))
}

/// The picture as a PNG image, `longest` pixels along its longer side.
pub fn to_png(p: &Picture, longest: u32) -> Option<image::RgbaImage> {
    use tiny_skia::{FillRule, LineCap, LineJoin, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};
    let k = longest as f64 / p.w.max(p.h);
    let (pw, ph) = ((p.w * k).round().max(1.0) as u32, (p.h * k).round().max(1.0) as u32);
    let mut pix = Pixmap::new(pw, ph)?;
    let tf = Transform::from_scale(k as f32, k as f32);
    let poly = |pts: &[(f64, f64)], close: bool| {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(*x as f32, *y as f32);
            } else {
                pb.line_to(*x as f32, *y as f32);
            }
        }
        if close {
            pb.close();
        }
        pb.finish()
    };
    for prim in &p.prims {
        match prim {
            Prim::Rect { x, y, w, h, rx, fill } => {
                let path = if *rx > 0.0 {
                    let (x, y, w, h, r) = (*x as f32, *y as f32, *w as f32, *h as f32, *rx as f32);
                    let mut pb = PathBuilder::new();
                    pb.move_to(x + r, y);
                    pb.line_to(x + w - r, y);
                    pb.quad_to(x + w, y, x + w, y + r);
                    pb.line_to(x + w, y + h - r);
                    pb.quad_to(x + w, y + h, x + w - r, y + h);
                    pb.line_to(x + r, y + h);
                    pb.quad_to(x, y + h, x, y + h - r);
                    pb.line_to(x, y + r);
                    pb.quad_to(x, y, x + r, y);
                    pb.close();
                    pb.finish()
                } else {
                    tiny_skia::Rect::from_xywh(*x as f32, *y as f32, *w as f32, *h as f32).map(PathBuilder::from_rect)
                };
                if let Some(path) = path {
                    pix.fill_path(&path, &paint(colour(fill, 1.0)), FillRule::Winding, tf, None);
                }
            }
            Prim::Line { pts, colour: c, width, opacity, dash } => {
                let Some(path) = poly(pts, false) else { continue };
                let stroke = Stroke { width: *width as f32, line_cap: LineCap::Round, line_join: LineJoin::Round, dash: dash.and_then(|(a, b)| StrokeDash::new(vec![a as f32, b as f32], 0.0)), ..Default::default() };
                pix.stroke_path(&path, &paint(colour(c, *opacity)), &stroke, tf, None);
            }
            Prim::Poly { pts, fill } => {
                if let Some(path) = poly(pts, true) {
                    pix.fill_path(&path, &paint(colour(fill, 1.0)), FillRule::Winding, tf, None);
                }
            }
            Prim::Circle { x, y, r, fill, stroke } => {
                if let Some(path) = PathBuilder::from_circle(*x as f32, *y as f32, *r as f32) {
                    pix.fill_path(&path, &paint(colour(fill, 1.0)), FillRule::Winding, tf, None);
                    pix.stroke_path(&path, &paint(colour(stroke.0, 1.0)), &Stroke { width: stroke.1 as f32, ..Default::default() }, tf, None);
                }
            }
            Prim::Text { x, y, size, text, fill, bold, centred, halo } => {
                let Some(font) = png_font() else { continue };
                let px = (*size * k) as f32;
                let Some((path, width)) = text_path(font, text, px) else { continue };
                let left = (*x * k) as f32 - if *centred { width * 0.5 } else { 0.0 };
                let at = Transform::from_translate(left, (*y * k) as f32);
                if let Some((c, w)) = halo {
                    let s = Stroke { width: (*w * k) as f32, line_join: LineJoin::Round, ..Default::default() };
                    pix.stroke_path(&path, &paint(colour(c, 1.0)), &s, at, None);
                }
                let ink = paint(colour(fill, 1.0));
                pix.fill_path(&path, &ink, FillRule::Winding, at, None);
                if *bold {
                    pix.stroke_path(&path, &ink, &Stroke { width: px * 0.05, ..Default::default() }, at, None);
                }
            }
        }
    }
    // (the background is opaque: the premultiplied pixels are the plain ones)
    image::RgbaImage::from_raw(pw, ph, pix.take())
}

/// `openmapper --plan path/to/global.cfg out.svg` (no window; `out.png`: a picture).
pub fn cli(cfg: &Path, out: &Path) -> i32 {
    let prefs = crate::install::Prefs::load();
    let Some(omsi) = crate::install::find_omsi_root(&prefs) else {
        eprintln!("no OMSI 2 installation found");
        return 2;
    };
    let roots = crate::install::Roots::new(omsi);
    let mut lib = Library::default();
    lib.start_scan(&roots);
    let doc = match Document::load(&roots, cfg, |_| {}) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: {e}", cfg.display());
            return 2;
        }
    };
    while lib.scanning {
        lib.poll();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // OPENMAPPER_PLAN_LINES=1: the lines alone; =<line>: that line alone
    let opts = match std::env::var("OPENMAPPER_PLAN_LINES") {
        Ok(l) => Options { background: false, line: Some(l).filter(|l| !l.is_empty() && l != "1" && l != "all") },
        Err(_) => Options::default(),
    };
    match write(&picture(&doc, &lib, &omsi_cfg::content_roots(), &opts), out) {
        Ok(()) => {
            println!("{}", out.display());
            0
        }
        Err(e) => {
            eprintln!("{}: {e}", out.display());
            1
        }
    }
}

/// Write the picture as SVG, or as PNG when the file is named so.
pub fn write(p: &Picture, out: &Path) -> Result<(), String> {
    let png = out.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if png {
        let img = to_png(p, 4096).ok_or_else(|| "the picture could not be drawn".to_string())?;
        img.save(out).map_err(|e| e.to_string())
    } else {
        std::fs::write(out, to_svg(p)).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::tests::flat_map;

    #[test]
    fn a_plan_names_each_pair_of_stops_once() {
        let mut doc = flat_map(2);
        doc.global.friendly_name = "Town & Country".into();
        let stop = |id: i64, x: f64| omsi_map::MapObject { file: "stop.sco".into(), id, pos: [x, 100.0, 0.0], flag: 7, extra: vec!["Rathaus <Mitte>".into(), "0".into(), "0".into(), "10".into()], ..Default::default() };
        let t = doc.tiles.get_mut(&(0, 0)).unwrap();
        t.tile.objects.extend([stop(1, 100.0), stop(2, 112.0)]);
        doc.tiles.get_mut(&(1, 0)).unwrap().water = Some([5.0; 4]);
        let s = svg(&doc, &Library::default(), &[]);
        assert!(s.starts_with("<svg") && s.trim_end().ends_with("</svg>"));
        assert_eq!(s.matches("<circle").count(), 2);
        assert_eq!(s.matches("Rathaus &lt;Mitte&gt;").count(), 1);
        assert!(s.contains("Town &amp; Country"));
        // (the flat ground lies under the water all over its tile)
        assert!(s.contains("#a9cfe8"));
        assert!(s.matches("<rect").count() > 60);
    }

    #[test]
    fn a_plan_is_drawn_as_a_png_too() {
        let doc = flat_map(2);
        let img = to_png(&picture(&doc, &Library::default(), &[], &Options::default()), 256).unwrap();
        assert_eq!(img.width().max(img.height()), 256);
        // (the ground's colour, not left transparent)
        assert_eq!(img.get_pixel(img.width() / 2, img.height() / 2)[3], 255);
    }
}
