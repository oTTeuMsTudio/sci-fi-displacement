use crate::map::Map;
use crate::rng::Rng;

pub const DEFAULT_ITERATIONS: u32 = 1000;
pub const DEFAULT_BACKGROUND: u8 = 128;
pub const DEFAULT_MIN_BRIGHTNESS: u8 = 0;
pub const DEFAULT_MAX_BRIGHTNESS: u8 = 255;
pub const DEFAULT_SCALE: f32 = 100.0;
pub const DEFAULT_GHOST_ALPHA: f32 = 0.8;
pub const DEFAULT_GHOST_HOLE: f32 = 50.0;
pub const DEFAULT_SPACING: f32 = 100.0;
pub const DEFAULT_MAX_AMOUNT: u32 = 5;
pub const DEFAULT_LINE_WIDTH: u32 = 10;

/// Layered plates, soft rects, grilles, bars, and seams.
/// Defaults follow the Classic screen: 1000 iterations on a mid-gray field.
#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Params {
    pub iterations: u32,
    pub background: u8,
    pub min_brightness: u8,
    pub max_brightness: u8,
    pub scale: f32,
    /// Opacity of the soft-rectangle layer. 0.8 matches the 80% alpha slider.
    pub ghost_alpha: f32,
    /// Percent chance a soft rectangle is drawn as a frame instead of a fill.
    pub ghost_hole: f32,
    /// Gap between grille cells or bars, as a percent of their thickness.
    pub spacing: f32,
    pub max_amount: u32,
    pub line_width: u32,
    pub squares: bool,
    pub ghosts: bool,
    pub grids: bool,
    pub vbars: bool,
    pub hbars: bool,
    pub crosses: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            iterations: DEFAULT_ITERATIONS,
            background: DEFAULT_BACKGROUND,
            min_brightness: DEFAULT_MIN_BRIGHTNESS,
            max_brightness: DEFAULT_MAX_BRIGHTNESS,
            scale: DEFAULT_SCALE,
            ghost_alpha: DEFAULT_GHOST_ALPHA,
            ghost_hole: DEFAULT_GHOST_HOLE,
            spacing: DEFAULT_SPACING,
            max_amount: DEFAULT_MAX_AMOUNT,
            line_width: DEFAULT_LINE_WIDTH,
            squares: true,
            ghosts: true,
            grids: true,
            vbars: true,
            hbars: true,
            crosses: true,
        }
    }
}

#[derive(Clone, Copy)]
enum Layer {
    Square,
    Ghost,
    Grid,
    VBar,
    HBar,
    Cross,
}

pub fn generate(params: &Params, width: u32, height: u32, seed: u64) -> Map {
    let mut rng = Rng::new(seed);
    let mut map = Map::filled(width, height, params.background);
    let layers = enabled_layers(params);
    if layers.is_empty() || width == 0 || height == 0 {
        return map;
    }
    let (min_b, max_b) = ordered_u8(params.min_brightness, params.max_brightness);
    let short = width.min(height);
    let total = params.iterations.max(1) as f32;
    for index in 0..params.iterations {
        // Large plates land first. Later passes add smaller grilles, bars, and chips on top.
        let progress = index as f32 / total;
        let detail_scale = params.scale * (1.0 - 0.7 * progress);
        let layer = layers[rng.index(layers.len())];
        let brightness = rng.u32(min_b as u32, max_b as u32) as u8;
        match layer {
            Layer::Square => stamp_square(&mut map, &mut rng, short, detail_scale, brightness),
            Layer::Ghost => {
                stamp_ghost(&mut map, &mut rng, short, params, detail_scale, brightness)
            }
            Layer::Grid => stamp_grid(&mut map, &mut rng, short, params, detail_scale, brightness),
            Layer::VBar => stamp_bars(
                &mut map,
                &mut rng,
                short,
                params,
                detail_scale,
                brightness,
                true,
            ),
            Layer::HBar => stamp_bars(
                &mut map,
                &mut rng,
                short,
                params,
                detail_scale,
                brightness,
                false,
            ),
            Layer::Cross => stamp_cross(&mut map, &mut rng, width, height, params, brightness),
        }
    }
    map
}

fn enabled_layers(params: &Params) -> Vec<Layer> {
    let mut layers = Vec::with_capacity(6);
    if params.squares {
        layers.push(Layer::Square);
    }
    if params.ghosts {
        layers.push(Layer::Ghost);
    }
    if params.grids {
        layers.push(Layer::Grid);
    }
    if params.vbars {
        layers.push(Layer::VBar);
    }
    if params.hbars {
        layers.push(Layer::HBar);
    }
    if params.crosses {
        layers.push(Layer::Cross);
    }
    layers
}

fn stamp_square(map: &mut Map, rng: &mut Rng, short: u32, scale: f32, brightness: u8) {
    let side = extent(rng, short, scale, 1.65);
    let (x, y) = origin(rng, map.width(), map.height(), side, side);
    map.fill_rect(x, y, side, side, brightness);
}

fn stamp_ghost(
    map: &mut Map,
    rng: &mut Rng,
    short: u32,
    params: &Params,
    scale: f32,
    brightness: u8,
) {
    let side = extent(rng, short, scale, 1.4);
    let (x, y) = origin(rng, map.width(), map.height(), side, side);
    let alpha = params.ghost_alpha.clamp(0.0, 1.0);
    let hole_chance = (params.ghost_hole / 100.0).clamp(0.0, 1.0);
    if rng.chance(hole_chance) {
        let thickness = (side / 7).max(1);
        map.blend_stroke_rect(x, y, side, side, thickness, brightness, alpha);
    } else {
        map.blend_rect(x, y, side, side, brightness, alpha);
    }
}

fn stamp_grid(
    map: &mut Map,
    rng: &mut Rng,
    short: u32,
    params: &Params,
    scale: f32,
    brightness: u8,
) {
    let amount = rng.u32(2, params.max_amount.max(2));
    let group = extent(rng, short, scale, 1.2);
    let spacing = (params.spacing / 100.0).max(0.0);
    let pitch_units = amount as f32 + (amount - 1) as f32 * spacing;
    let cell = (group as f32 / pitch_units).round().max(1.0) as i32;
    let gap = (cell as f32 * spacing).round().max(0.0) as i32;
    let span = amount as i32 * cell + (amount as i32 - 1) * gap;
    let (x0, y0) = origin(rng, map.width(), map.height(), span, span);
    for gy in 0..amount as i32 {
        for gx in 0..amount as i32 {
            let x = x0 + gx * (cell + gap);
            let y = y0 + gy * (cell + gap);
            map.fill_rect(x, y, cell, cell, brightness);
        }
    }
}

fn stamp_bars(
    map: &mut Map,
    rng: &mut Rng,
    short: u32,
    params: &Params,
    scale: f32,
    brightness: u8,
    vertical: bool,
) {
    let amount = rng.u32(2, params.max_amount.max(2)) as i32;
    let length = extent(rng, short, scale, 1.15);
    let across = extent(rng, short, scale * 0.55, 1.1).max(amount * 2);
    let spacing = (params.spacing / 100.0).max(0.0);
    let pitch = (across / amount).max(2);
    let thickness = (pitch as f32 / (1.0 + spacing)).round().max(1.0) as i32;
    let gap = (pitch - thickness).max(0);
    let span = amount * thickness + (amount - 1) * gap;
    let (w, h) = if vertical {
        (span, length)
    } else {
        (length, span)
    };
    let (x0, y0) = origin(rng, map.width(), map.height(), w, h);
    for index in 0..amount {
        let shift = index * (thickness + gap);
        if vertical {
            map.fill_rect(x0 + shift, y0, thickness, length, brightness);
        } else {
            map.fill_rect(x0, y0 + shift, length, thickness, brightness);
        }
    }
}

fn stamp_cross(
    map: &mut Map,
    rng: &mut Rng,
    width: u32,
    height: u32,
    params: &Params,
    brightness: u8,
) {
    let arm = extent(rng, width.min(height), params.scale, 0.55).max(4);
    let thickness = params.line_width.max(1) as i32;
    let cx = rng.i32(0, width.saturating_sub(1) as i32);
    let cy = rng.i32(0, height.saturating_sub(1) as i32);
    map.fill_rect(cx - arm / 2, cy - thickness / 2, arm, thickness, brightness);
    map.fill_rect(cx - thickness / 2, cy - arm / 2, thickness, arm, brightness);
}

/// `bias` above 1 favors smaller shapes. Below 1 favors larger ones.
fn extent(rng: &mut Rng, canvas: u32, scale_percent: f32, bias: f32) -> i32 {
    let max_px = (canvas as f32 * (scale_percent / 100.0)).clamp(2.0, canvas.max(2) as f32);
    let min_px = (max_px * 0.035).max(2.0);
    let t = rng.f32().powf(bias.max(0.05));
    (min_px + t * (max_px - min_px)).round() as i32
}

fn origin(rng: &mut Rng, width: u32, height: u32, w: i32, h: i32) -> (i32, i32) {
    let bleed_x = (w / 6).max(0);
    let bleed_y = (h / 6).max(0);
    let x = rng.i32(-bleed_x, width as i32 - w + bleed_x);
    let y = rng.i32(-bleed_y, height as i32 - h + bleed_y);
    (x, y)
}

fn ordered_u8(a: u8, b: u8) -> (u8, u8) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_rebuilds_the_same_map() {
        let params = Params {
            iterations: 80,
            ..Params::default()
        };
        let a = generate(&params, 96, 96, 11);
        let b = generate(&params, 96, 96, 11);
        let c = generate(&params, 96, 96, 12);
        assert_eq!(a.pixels(), b.pixels());
        assert_ne!(a.pixels(), c.pixels());
    }

    #[test]
    fn default_run_is_not_flat() {
        let map = generate(&Params::default(), 128, 128, 3);
        let distinct = distinct_count(map.pixels());
        assert!(
            distinct > 8,
            "expected a varied plate field, got {distinct} levels"
        );
    }

    fn distinct_count(pixels: &[u8]) -> usize {
        let mut seen = [false; 256];
        for pixel in pixels {
            seen[*pixel as usize] = true;
        }
        seen.iter().filter(|on| **on).count()
    }
}
