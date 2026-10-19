use crate::map::{shade, Map};
use crate::rng::Rng;

pub const DEFAULT_BACKGROUND: u8 = 128;
pub const DEFAULT_BACKGROUND_ITERATIONS: u32 = 3;
pub const DEFAULT_ITERATIONS: u32 = 125;
pub const DEFAULT_SCALE: f32 = 60.0;

/// Procedural stand-in for the sprite packs on the JSplacement 2 screen.
/// These are generated shapes, not the original sprite artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub enum Pack {
    Classic,
    Crap,
    BigData,
    Aggro,
    Custom,
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub w: u32,
    pub h: u32,
    pub gray: Vec<u8>,
    pub mask: Vec<u8>,
}

impl Sprite {
    pub fn blank(w: u32, h: u32) -> Self {
        let n = (w as usize).saturating_mul(h as usize);
        Self {
            w,
            h,
            gray: vec![0; n],
            mask: vec![0; n],
        }
    }

    pub fn from_rgba(image: &image::RgbaImage) -> Self {
        let w = image.width();
        let h = image.height();
        if w == 0 || h == 0 {
            return Self::blank(1, 1);
        }
        let mut sprite = Self::blank(w, h);
        for y in 0..h {
            for x in 0..w {
                let pixel = image.get_pixel(x, y).0;
                let lum =
                    (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                let index = (y * w + x) as usize;
                sprite.gray[index] = lum;
                sprite.mask[index] = pixel[3];
            }
        }
        sprite
    }

    pub fn fit_inside(&self, max_w: u32, max_h: u32) -> Self {
        if self.w == 0 || self.h == 0 || max_w == 0 || max_h == 0 {
            return Self::blank(1, 1);
        }
        let scale = (max_w as f32 / self.w as f32)
            .min(max_h as f32 / self.h as f32)
            .max(0.01);
        let nw = (self.w as f32 * scale).round().clamp(1.0, max_w as f32) as u32;
        let nh = (self.h as f32 * scale).round().clamp(1.0, max_h as f32) as u32;
        self.scaled_to(nw, nh)
    }

    fn scaled_to(&self, nw: u32, nh: u32) -> Self {
        if nw == self.w && nh == self.h {
            return self.clone();
        }
        let mut out = Self::blank(nw, nh);
        for y in 0..nh {
            for x in 0..nw {
                let sx = ((x as u64 * self.w as u64) / nw as u64) as u32;
                let sy = ((y as u64 * self.h as u64) / nh as u64) as u32;
                let from = (sy * self.w + sx) as usize;
                let to = (y * nw + x) as usize;
                out.gray[to] = self.gray[from];
                out.mask[to] = self.mask[from];
            }
        }
        out
    }

    fn rot90(self) -> Self {
        let mut out = Self::blank(self.h, self.w);
        for y in 0..self.h {
            for x in 0..self.w {
                let from = (y * self.w + x) as usize;
                let nx = self.h - 1 - y;
                let ny = x;
                let to = (ny * out.w + nx) as usize;
                out.gray[to] = self.gray[from];
                out.mask[to] = self.mask[from];
            }
        }
        out
    }

    fn flip(self, flip_x: bool, flip_y: bool) -> Self {
        if !flip_x && !flip_y {
            return self;
        }
        let mut out = Self::blank(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                let nx = if flip_x { self.w - 1 - x } else { x };
                let ny = if flip_y { self.h - 1 - y } else { y };
                let from = (y * self.w + x) as usize;
                let to = (ny * out.w + nx) as usize;
                out.gray[to] = self.gray[from];
                out.mask[to] = self.mask[from];
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Params {
    pub pack: Pack,
    pub background: u8,
    pub background_iterations: u32,
    pub iterations: u32,
    pub scale: f32,
    pub seamless: bool,
    pub rotate: bool,
    pub flip: bool,
    pub blend: bool,
    #[cfg_attr(target_arch = "wasm32", serde(skip))]
    pub custom: Vec<Sprite>,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            pack: Pack::Crap,
            background: DEFAULT_BACKGROUND,
            background_iterations: DEFAULT_BACKGROUND_ITERATIONS,
            iterations: DEFAULT_ITERATIONS,
            scale: DEFAULT_SCALE,
            seamless: true,
            rotate: true,
            flip: true,
            blend: true,
            custom: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plate,
    Frame,
    Vent,
    Grill,
    Bolts,
    Hatch,
    Circuit,
    Wells,
    Chip,
    Pipes,
    Stripes,
    Pads,
    Greeble,
}

pub fn generate(params: &Params, width: u32, height: u32, seed: u64) -> Map {
    let mut rng = Rng::new(seed);
    let mut map = Map::filled(width, height, params.background);
    if width == 0 || height == 0 {
        return map;
    }
    for _ in 0..params.background_iterations {
        let long = (width.min(height) as f32 * (0.34 + rng.f32() * 0.42)) as u32;
        let (sw, sh) = varied_size(long.max(12), &mut rng, width, height);
        let base = shade(params.background, rng.i32(-36, 36));
        let mut sprite = Canvas::new(sw, sh);
        paint_plate(&mut sprite, base, rng.bool(), 36);
        place(&mut map, &mut rng, sprite.finish(), params);
    }
    for _ in 0..params.iterations {
        let (sw, sh) = stamp_size(width, height, params.scale, params.pack, &mut rng);
        let sprite = if params.pack == Pack::Custom && !params.custom.is_empty() {
            params.custom[rng.index(params.custom.len())].fit_inside(sw, sh)
        } else if params.pack == Pack::Custom {
            break;
        } else {
            let kind = pick_kind(params.pack, &mut rng);
            build(kind, sw, sh, params.pack, &mut rng)
        };
        let sprite = orient(sprite, params, &mut rng);
        place(&mut map, &mut rng, sprite, params);
    }
    map
}

fn place(map: &mut Map, rng: &mut Rng, sprite: Sprite, params: &Params) {
    let sw = sprite.w as i32;
    let sh = sprite.h as i32;
    let (x, y) = if params.seamless {
        (
            rng.i32(0, map.width().saturating_sub(1) as i32),
            rng.i32(0, map.height().saturating_sub(1) as i32),
        )
    } else {
        (
            rng.i32(-sw / 8, map.width() as i32 - sw + sw / 8),
            rng.i32(-sh / 8, map.height() as i32 - sh + sh / 8),
        )
    };
    let sprite = if params.blend {
        feather_edges(sprite)
    } else {
        sprite
    };
    map.blit(
        &sprite.gray,
        &sprite.mask,
        sprite.w,
        sprite.h,
        x,
        y,
        1.0,
        params.seamless,
    );
}

fn feather_edges(mut sprite: Sprite) -> Sprite {
    let w = sprite.w as i32;
    let h = sprite.h as i32;
    if w == 0 || h == 0 {
        return sprite;
    }
    let original = sprite.mask.clone();
    for y in 0..h {
        for x in 0..w {
            let index = (y * w + x) as usize;
            if original[index] == 0 {
                continue;
            }
            let mut exposed = false;
            for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if nx < 0 || ny < 0 || nx >= w || ny >= h || original[(ny * w + nx) as usize] == 0 {
                    exposed = true;
                    break;
                }
            }
            if exposed {
                sprite.mask[index] = sprite.mask[index].saturating_mul(2) / 3;
            }
        }
    }
    sprite
}

fn orient(mut sprite: Sprite, params: &Params, rng: &mut Rng) -> Sprite {
    if params.rotate {
        for _ in 0..rng.u32(0, 3) {
            sprite = sprite.rot90();
        }
    }
    if params.flip {
        sprite = sprite.flip(rng.bool(), rng.bool());
    }
    sprite
}

fn stamp_size(width: u32, height: u32, scale: f32, pack: Pack, rng: &mut Rng) -> (u32, u32) {
    let short = width.min(height) as f32;
    let pack_scale = match pack {
        Pack::Classic => 0.72,
        Pack::Crap => 0.5,
        Pack::BigData => 0.26,
        Pack::Aggro => 0.84,
        Pack::Custom => 0.55,
    };
    let max_px = (short * (scale / 100.0).clamp(0.02, 1.5) * pack_scale).max(18.0);
    let min_px = (max_px * 0.18).clamp(8.0, max_px);
    let long = min_px + rng.f32().powf(1.4) * (max_px - min_px);
    varied_size(long.round() as u32, rng, width, height)
}

fn varied_size(long: u32, rng: &mut Rng, width: u32, height: u32) -> (u32, u32) {
    let aspect = 0.42 + rng.f32() * 1.5;
    let mut sw = long as f32;
    let mut sh = (long as f32 / aspect).max(8.0);
    if rng.bool() {
        std::mem::swap(&mut sw, &mut sh);
    }
    (
        sw.round().clamp(8.0, width.max(8) as f32) as u32,
        sh.round().clamp(8.0, height.max(8) as f32) as u32,
    )
}

fn pick_kind(pack: Pack, rng: &mut Rng) -> Kind {
    let table: &[(Kind, u32)] = match pack {
        Pack::Classic => &[
            (Kind::Plate, 4),
            (Kind::Frame, 3),
            (Kind::Vent, 4),
            (Kind::Bolts, 3),
            (Kind::Hatch, 2),
            (Kind::Grill, 2),
            (Kind::Pads, 2),
        ],
        Pack::BigData => &[
            (Kind::Chip, 4),
            (Kind::Circuit, 4),
            (Kind::Wells, 3),
            (Kind::Pads, 3),
            (Kind::Grill, 2),
            (Kind::Greeble, 3),
            (Kind::Frame, 1),
        ],
        Pack::Aggro => &[
            (Kind::Plate, 3),
            (Kind::Frame, 3),
            (Kind::Stripes, 3),
            (Kind::Hatch, 2),
            (Kind::Vent, 2),
            (Kind::Greeble, 3),
            (Kind::Bolts, 2),
        ],
        Pack::Crap | Pack::Custom => &[
            (Kind::Plate, 2),
            (Kind::Vent, 3),
            (Kind::Grill, 2),
            (Kind::Bolts, 2),
            (Kind::Hatch, 2),
            (Kind::Circuit, 3),
            (Kind::Wells, 3),
            (Kind::Chip, 2),
            (Kind::Pipes, 2),
            (Kind::Greeble, 5),
            (Kind::Stripes, 2),
            (Kind::Pads, 2),
        ],
    };
    let weights: Vec<u32> = table.iter().map(|(_, weight)| *weight).collect();
    table[rng.weighted(&weights)].0
}

fn build(kind: Kind, w: u32, h: u32, pack: Pack, rng: &mut Rng) -> Sprite {
    let (base, raised) = base_height(pack, rng);
    let punch = match pack {
        Pack::Aggro => 88,
        Pack::Classic => 46,
        Pack::BigData => 60,
        _ => 62,
    };
    if kind == Kind::Frame {
        return frame_sprite(w, h, base, punch);
    }
    let mut canvas = Canvas::new(w, h);
    paint_plate(&mut canvas, base, raised, punch);
    if rng.chance(0.28) && w > 18 && h > 18 {
        canvas.clear_corners((w.min(h) as i32 / 7).max(2));
    }
    match kind {
        Kind::Plate | Kind::Frame => {}
        Kind::Vent => {
            let horizontal = rng.bool();
            vents(&mut canvas, rng, base, punch, horizontal);
        }
        Kind::Grill => grill(&mut canvas, rng, base, punch),
        Kind::Bolts => {
            if let Some(bounds) = inner(&canvas) {
                bolts(&mut canvas, rng, bounds, base, punch);
            }
        }
        Kind::Hatch => hatch(&mut canvas, rng, base, punch),
        Kind::Circuit => circuit(&mut canvas, rng, base, punch),
        Kind::Wells => wells(&mut canvas, rng, base, punch),
        Kind::Chip => chip(&mut canvas, base, punch),
        Kind::Pipes => {
            let horizontal = rng.bool();
            pipes(&mut canvas, rng, base, punch, horizontal);
        }
        Kind::Stripes => stripes(&mut canvas, rng, base, punch),
        Kind::Pads => pads(&mut canvas, rng, base, punch),
        Kind::Greeble => {
            let count = rng.u32(2, 4);
            for _ in 0..count {
                let feature = rng.u32(0, 6);
                match feature {
                    0 => {
                        let horizontal = rng.bool();
                        vents(&mut canvas, rng, base, punch, horizontal);
                    }
                    1 => wells(&mut canvas, rng, base, punch),
                    2 => {
                        if let Some(bounds) = inner(&canvas) {
                            bolts(&mut canvas, rng, bounds, base, punch);
                        }
                    }
                    3 => circuit(&mut canvas, rng, base, punch),
                    4 => pads(&mut canvas, rng, base, punch),
                    5 => stripes(&mut canvas, rng, base, punch),
                    _ => grill(&mut canvas, rng, base, punch),
                }
            }
        }
    }
    canvas.finish()
}

fn base_height(pack: Pack, rng: &mut Rng) -> (u8, bool) {
    let raised = rng.bool();
    let base = match pack {
        Pack::Aggro => {
            if raised {
                rng.u32(205, 255)
            } else {
                rng.u32(0, 40)
            }
        }
        Pack::Classic => {
            if raised {
                rng.u32(148, 205)
            } else {
                rng.u32(58, 112)
            }
        }
        Pack::BigData => {
            if raised {
                rng.u32(118, 176)
            } else {
                rng.u32(22, 78)
            }
        }
        _ => {
            if raised {
                rng.u32(142, 220)
            } else {
                rng.u32(32, 102)
            }
        }
    } as u8;
    (base, raised)
}

fn frame_sprite(w: u32, h: u32, base: u8, punch: i32) -> Sprite {
    let mut canvas = Canvas::new(w, h);
    let width = w as i32;
    let height = h as i32;
    let thickness = (w.min(h) as i32 / 7).max(1);
    canvas.stroke(0, 0, width, height, thickness, shade(base, punch / 2));
    let inset = thickness + (thickness / 2).max(1);
    if width > inset * 2 + 2 && height > inset * 2 + 2 {
        canvas.stroke(
            inset,
            inset,
            width - inset * 2,
            height - inset * 2,
            (thickness / 2).max(1),
            shade(base, -punch),
        );
    }
    canvas.finish()
}

fn paint_plate(canvas: &mut Canvas, base: u8, raised: bool, punch: i32) {
    let (edge, lip, face) = if raised {
        (shade(base, -punch), shade(base, punch / 3), base)
    } else {
        (
            shade(base, punch / 3),
            shade(base, -punch / 5),
            shade(base, -punch / 2),
        )
    };
    let w = canvas.width() as i32;
    let h = canvas.height() as i32;
    canvas.fill(0, 0, w, h, edge);
    if w > 4 && h > 4 {
        canvas.fill(1, 1, w - 2, h - 2, lip);
    }
    if w > 8 && h > 8 {
        canvas.fill(3, 3, w - 6, h - 6, face);
    }
}

fn inner(canvas: &Canvas) -> Option<(i32, i32, i32, i32)> {
    let margin = (canvas.width().min(canvas.height()) / 8).max(2) as i32;
    let w = canvas.width() as i32 - margin * 2;
    let h = canvas.height() as i32 - margin * 2;
    if w < 4 || h < 4 {
        None
    } else {
        Some((margin, margin, w, h))
    }
}

fn vents(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32, horizontal: bool) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let slots = rng.u32(3, 8) as i32;
    let span = if horizontal { h } else { w };
    let slot = (span / (slots * 2)).max(1);
    let color = shade(base, -punch);
    let mut cursor = 0;
    for _ in 0..slots {
        if cursor + slot > span {
            break;
        }
        if horizontal {
            canvas.fill(x, y + cursor, w, slot, color);
        } else {
            canvas.fill(x + cursor, y, slot, h, color);
        }
        cursor += slot * 2;
    }
}

fn grill(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let step = rng.i32(3, 6);
    let color = shade(base, -punch);
    let mut cursor = 0;
    while cursor < h {
        canvas.fill(x, y + cursor, w, 1, color);
        cursor += step;
    }
    cursor = 0;
    while cursor < w {
        canvas.fill(x + cursor, y, 1, h, color);
        cursor += step + 1;
    }
}

fn bolts(canvas: &mut Canvas, rng: &mut Rng, bounds: (i32, i32, i32, i32), base: u8, punch: i32) {
    let (x, y, w, h) = bounds;
    let radius = (w.min(h) / 12).clamp(1, 6);
    let inset = radius + 1;
    if w <= inset * 2 || h <= inset * 2 {
        return;
    }
    let spots = [
        (x + inset, y + inset),
        (x + w - inset, y + inset),
        (x + inset, y + h - inset),
        (x + w - inset, y + h - inset),
    ];
    let cap = shade(base, punch / 2);
    let hole = shade(base, -punch / 2);
    for (cx, cy) in spots {
        canvas.disc(cx, cy, radius, cap);
        canvas.put(cx, cy, hole);
    }
    if rng.chance(0.4) && w > 20 {
        let mid_y = y + h / 2;
        canvas.disc(x + inset, mid_y, radius, cap);
        canvas.disc(x + w - inset, mid_y, radius, cap);
    }
}

fn hatch(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let thickness = (w.min(h) / 10).max(1);
    canvas.stroke(x, y, w, h, thickness, shade(base, -punch));
    let bar = (h / 7).max(1);
    canvas.fill(x, y + h / 2 - bar / 2, w, bar, shade(base, punch / 2));
    bolts(canvas, rng, (x, y, w, h), base, punch);
}

fn circuit(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let mut cx = x + rng.i32(0, (w / 3).max(0));
    let mut cy = y + rng.i32(0, (h / 3).max(0));
    let trace = shade(base, punch);
    let pad = shade(base, punch + 16);
    for _ in 0..rng.u32(4, 8) {
        if rng.bool() {
            let len = rng.i32(3, (w / 2).max(3));
            canvas.fill(cx, cy, len, 2, trace);
            cx = (cx + len).clamp(x, x + w - 3);
        } else {
            let len = rng.i32(3, (h / 2).max(3));
            canvas.fill(cx, cy, 2, len, trace);
            cy = (cy + len).clamp(y, y + h - 3);
        }
        canvas.fill(cx - 1, cy - 1, 4, 4, pad);
    }
}

fn wells(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let n = rng.u32(2, 4) as i32;
    let cell_w = w / n;
    let cell_h = h / n;
    if cell_w < 3 || cell_h < 3 {
        return;
    }
    let pad = (cell_w.min(cell_h) / 5).max(1);
    for gy in 0..n {
        for gx in 0..n {
            let cx = x + gx * cell_w + pad;
            let cy = y + gy * cell_h + pad;
            let cw = cell_w - pad * 2;
            let ch = cell_h - pad * 2;
            if cw > 1 && ch > 1 {
                canvas.fill(cx, cy, cw, ch, shade(base, -punch));
                if cw > 4 && ch > 4 {
                    canvas.fill(cx + 1, cy + 1, cw - 2, ch - 2, shade(base, punch / 3));
                }
            }
        }
    }
}

fn chip(canvas: &mut Canvas, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let pin = shade(base, punch);
    let mut py = y;
    while py + 2 < y + h {
        canvas.fill(0, py, x.max(2), 2, pin);
        canvas.fill(x + w, py, (canvas.width() as i32 - (x + w)).max(2), 2, pin);
        py += 4;
    }
    let die_w = (w / 2).max(2);
    let die_h = (h / 2).max(2);
    canvas.stroke(
        x + (w - die_w) / 2,
        y + (h - die_h) / 2,
        die_w,
        die_h,
        1,
        shade(base, -punch / 2),
    );
    canvas.fill(
        x + (w - die_w) / 2 + 2,
        y + (h - die_h) / 2 + 2,
        (die_w - 4).max(1),
        (die_h - 4).max(1),
        shade(base, -punch / 3),
    );
}

fn pipes(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32, horizontal: bool) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let count = rng.u32(2, 4) as i32;
    let span = if horizontal { h } else { w };
    let pipe = (span / (count * 2)).max(2);
    let mut cursor = pipe / 2;
    while cursor + pipe < span && count > 0 {
        let channel = shade(base, -punch);
        let lip = shade(base, punch / 2);
        if horizontal {
            canvas.fill(x, y + cursor, w, pipe, channel);
            canvas.fill(x, y + cursor, w, 1, lip);
            canvas.fill(x, y + cursor + pipe - 1, w, 1, lip);
        } else {
            canvas.fill(x + cursor, y, pipe, h, channel);
            canvas.fill(x + cursor, y, 1, h, lip);
            canvas.fill(x + cursor + pipe - 1, y, 1, h, lip);
        }
        cursor += pipe * 2;
    }
}

fn stripes(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let period = rng.i32(3, 6).max(2);
    let color = shade(base, -punch);
    for yy in y..(y + h) {
        for xx in x..(x + w) {
            if (xx + yy).rem_euclid(period) == 0 {
                canvas.put(xx, yy, color);
            }
        }
    }
}

fn pads(canvas: &mut Canvas, rng: &mut Rng, base: u8, punch: i32) {
    let Some((x, y, w, h)) = inner(canvas) else {
        return;
    };
    let max_r = (w.min(h) / 5).max(2);
    for _ in 0..rng.u32(3, 6) {
        let radius = rng.i32(2, max_r);
        if w <= radius * 2 || h <= radius * 2 {
            continue;
        }
        let cx = rng.i32(x + radius, x + w - radius);
        let cy = rng.i32(y + radius, y + h - radius);
        canvas.disc(cx, cy, radius, shade(base, punch / 2));
        canvas.ring(cx, cy, radius, 1, shade(base, -punch / 3));
    }
}

struct Canvas {
    sprite: Sprite,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            sprite: Sprite::blank(w.max(1), h.max(1)),
        }
    }

    fn width(&self) -> u32 {
        self.sprite.w
    }

    fn height(&self) -> u32 {
        self.sprite.h
    }

    fn finish(self) -> Sprite {
        self.sprite
    }

    fn put(&mut self, x: i32, y: i32, value: u8) {
        if x < 0 || y < 0 || x >= self.sprite.w as i32 || y >= self.sprite.h as i32 {
            return;
        }
        let index = (y as u32 * self.sprite.w + x as u32) as usize;
        self.sprite.gray[index] = value;
        self.sprite.mask[index] = 255;
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, value: u8) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.sprite.w as i32);
        let y1 = (y + h).min(self.sprite.h as i32);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.put(xx, yy, value);
            }
        }
    }

    fn stroke(&mut self, x: i32, y: i32, w: i32, h: i32, thickness: i32, value: u8) {
        if w <= 0 || h <= 0 || thickness <= 0 {
            return;
        }
        let t = thickness.min(w).min(h);
        self.fill(x, y, w, t, value);
        if h > t {
            self.fill(x, y + h - t, w, t, value);
        }
        if h > t * 2 {
            self.fill(x, y + t, t, h - t * 2, value);
            if w > t {
                self.fill(x + w - t, y + t, t, h - t * 2, value);
            }
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, radius: i32, value: u8) {
        if radius <= 0 {
            self.put(cx, cy, value);
            return;
        }
        let r2 = radius * radius;
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r2 {
                    self.put(x, y, value);
                }
            }
        }
    }

    fn ring(&mut self, cx: i32, cy: i32, radius: i32, thickness: i32, value: u8) {
        let thickness = thickness.max(1);
        if radius <= 0 {
            self.put(cx, cy, value);
            return;
        }
        let outer = radius * radius;
        let inner_r = (radius - thickness).max(0);
        let inner = inner_r * inner_r;
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let d = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                if d <= outer && d >= inner {
                    self.put(x, y, value);
                }
            }
        }
    }

    fn clear_corners(&mut self, rad: i32) {
        if rad <= 1 {
            return;
        }
        let w = self.sprite.w as i32;
        let h = self.sprite.h as i32;
        for y in 0..h {
            for x in 0..w {
                let cut = (x < rad && y < rad && x + y < rad)
                    || (x >= w - rad && y < rad && (w - 1 - x) + y < rad)
                    || (x < rad && y >= h - rad && x + (h - 1 - y) < rad)
                    || (x >= w - rad && y >= h - rad && (w - 1 - x) + (h - 1 - y) < rad);
                if cut {
                    let index = (y as u32 * self.sprite.w + x as u32) as usize;
                    self.sprite.mask[index] = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_deterministic() {
        let a = generate(&Params::default(), 96, 96, 8);
        let b = generate(&Params::default(), 96, 96, 8);
        assert_eq!(a.pixels(), b.pixels());
        assert!(a.pixels().iter().any(|pixel| *pixel != DEFAULT_BACKGROUND));
    }

    #[test]
    fn rotation_turns_width_into_height() {
        let sprite = Sprite {
            w: 2,
            h: 1,
            gray: vec![1, 2],
            mask: vec![255, 255],
        };
        let turned = sprite.rot90();
        assert_eq!((turned.w, turned.h), (1, 2));
        assert_eq!(turned.gray, vec![1, 2]);
    }
}
