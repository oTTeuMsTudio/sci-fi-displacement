use crate::map::Map;
use crate::rng::Rng;

pub const DEFAULT_SCALE: u32 = 20;
pub const DEFAULT_MIN_BRIGHTNESS: u8 = 0;
pub const DEFAULT_CHANCE: f32 = 2.5;
pub const DEFAULT_MAX_LENGTH: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub enum Shape {
    Square,
    Box,
    Circle,
    Ring,
    Diamond,
    Gem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub enum Pattern {
    Row,
    Col,
    Rake,
    Tee,
    Tri,
    Diag,
    Grid,
}

#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Params {
    pub scale: u32,
    pub min_brightness: u8,
    /// Percent chance that a grid cell receives a mark.
    pub chance: f32,
    pub max_length: u32,
    pub shapes: Vec<Shape>,
    pub patterns: Vec<Pattern>,
    pub break_code: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            min_brightness: DEFAULT_MIN_BRIGHTNESS,
            chance: DEFAULT_CHANCE,
            max_length: DEFAULT_MAX_LENGTH,
            shapes: vec![
                Shape::Square,
                Shape::Box,
                Shape::Circle,
                Shape::Ring,
                Shape::Diamond,
                Shape::Gem,
            ],
            patterns: vec![
                Pattern::Row,
                Pattern::Col,
                Pattern::Rake,
                Pattern::Tee,
                Pattern::Tri,
                Pattern::Diag,
                Pattern::Grid,
            ],
            break_code: false,
        }
    }
}

pub fn generate(params: &Params, width: u32, height: u32, seed: u64) -> Map {
    let mut rng = Rng::new(seed);
    let mut map = Map::filled(width, height, 0);
    if width == 0 || height == 0 {
        return map;
    }
    // Scale sizes the marks. The lattice is finer so the default 2.5% chance still reads as a field.
    let step = (params.scale / 2).max(2);
    let chance = (params.chance / 100.0).clamp(0.0, 1.0);
    let shapes = if params.shapes.is_empty() && params.patterns.is_empty() {
        vec![Shape::Square]
    } else {
        params.shapes.clone()
    };
    let use_shape_bucket = !shapes.is_empty();
    let use_pattern_bucket = !params.patterns.is_empty();

    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            if rng.chance(chance) {
                let brightness = rng.u32(params.min_brightness as u32, 255) as u8;
                let (px, py) = jitter(x, y, step, params.break_code, &mut rng);
                let draw_pattern = match (use_shape_bucket, use_pattern_bucket) {
                    (true, true) => rng.bool(),
                    (false, true) => true,
                    _ => false,
                };
                if params.break_code && rng.chance(0.3) {
                    map.set(px as i32, py as i32, brightness);
                } else if draw_pattern {
                    let pattern = params.patterns[rng.index(params.patterns.len())];
                    draw_pattern_at(&mut map, &mut rng, px, py, pattern, params, brightness);
                } else {
                    let shape = shapes[rng.index(shapes.len())];
                    draw_shape_at(&mut map, &mut rng, px, py, shape, step, brightness);
                }
            }
            x += step;
        }
        y += step;
    }

    if params.break_code {
        let extras = (width as usize * height as usize) / 2500;
        for _ in 0..extras {
            let x = rng.u32(0, width.saturating_sub(1)) as i32;
            let y = rng.u32(0, height.saturating_sub(1)) as i32;
            map.set(x, y, rng.u32(params.min_brightness as u32, 255) as u8);
        }
    }
    map
}

fn jitter(x: u32, y: u32, step: u32, break_code: bool, rng: &mut Rng) -> (u32, u32) {
    if !break_code {
        return (x, y);
    }
    let kick = (step / 4).max(1) as i32;
    let px = (x as i32 + rng.i32(-kick, kick)).max(0) as u32;
    let py = (y as i32 + rng.i32(-kick, kick)).max(0) as u32;
    (px, py)
}

fn draw_shape_at(
    map: &mut Map,
    rng: &mut Rng,
    x: u32,
    y: u32,
    shape: Shape,
    scale: u32,
    value: u8,
) {
    let max_size = scale.max(1);
    let min_size = (scale / 5).max(1);
    let t = rng.f32().powf(1.35);
    let size = min_size + (t * (max_size.saturating_sub(min_size)) as f32).round() as u32;
    let size = size.max(1) as i32;
    let cx = x as i32 + size / 2;
    let cy = y as i32 + size / 2;
    let thickness = (size / 6).max(1);
    match shape {
        Shape::Square => map.fill_rect(x as i32, y as i32, size, size, value),
        Shape::Box => map.stroke_rect(x as i32, y as i32, size, size, thickness, value),
        Shape::Circle => map.fill_circle(cx, cy, size / 2, value),
        Shape::Ring => map.stroke_circle(cx, cy, size / 2, thickness, value),
        Shape::Diamond => map.fill_diamond(cx, cy, size / 2, value),
        Shape::Gem => map.stroke_diamond(cx, cy, size / 2, thickness, value),
    }
}

fn draw_pattern_at(
    map: &mut Map,
    rng: &mut Rng,
    x: u32,
    y: u32,
    pattern: Pattern,
    params: &Params,
    value: u8,
) {
    let length = rng.u32(1, params.max_length.max(1)) as i32;
    let dot = (params.scale / 7).max(1) as i32;
    let pitch = (params.scale / 4).max((dot + 1) as u32) as i32;
    let ox = x as i32;
    let oy = y as i32;
    match pattern {
        Pattern::Row => {
            for index in 0..length {
                dot_at(map, ox + index * pitch, oy, dot, value);
            }
        }
        Pattern::Col => {
            for index in 0..length {
                dot_at(map, ox, oy + index * pitch, dot, value);
            }
        }
        Pattern::Rake => {
            for index in 0..length {
                dot_at(map, ox + index * pitch, oy, dot, value);
            }
            for index in 0..(length / 2).max(1) {
                dot_at(map, ox + index * pitch, oy + pitch, dot, value);
            }
        }
        Pattern::Tee => {
            for index in 0..length {
                dot_at(map, ox + index * pitch, oy, dot, value);
            }
            let mid = (length - 1) / 2;
            for index in 1..=(length / 2).max(1) {
                dot_at(map, ox + mid * pitch, oy + index * pitch, dot, value);
            }
        }
        Pattern::Tri => {
            for row in 0..length {
                for col in 0..=row {
                    dot_at(map, ox + col * pitch, oy + row * pitch, dot, value);
                }
            }
        }
        Pattern::Diag => {
            for index in 0..length {
                dot_at(map, ox + index * pitch, oy + index * pitch, dot, value);
                if index > 0 {
                    dot_at(map, ox + index * pitch, oy, dot, value);
                }
            }
        }
        Pattern::Grid => {
            for row in 0..length {
                for col in 0..length {
                    dot_at(map, ox + col * pitch, oy + row * pitch, dot, value);
                }
            }
        }
    }
}

fn dot_at(map: &mut Map, x: i32, y: i32, size: i32, value: u8) {
    map.fill_rect(x, y, size, size, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_chance_covers_the_grid_with_squares() {
        let params = Params {
            scale: 8,
            chance: 100.0,
            shapes: vec![Shape::Square],
            patterns: Vec::new(),
            min_brightness: 255,
            ..Params::default()
        };
        let map = generate(&params, 32, 32, 1);
        assert!(map.pixels().iter().any(|pixel| *pixel == 255));
        let again = generate(&params, 32, 32, 1);
        assert_eq!(map.pixels(), again.pixels());
    }
}
