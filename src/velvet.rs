use crate::map::Map;
use crate::rng::Rng;

pub const DEFAULT_ITERATIONS: u32 = 100;
pub const DEFAULT_SUB_ITERATIONS: u32 = 50;
pub const DEFAULT_SPLITTER_ITERATIONS: u32 = 50;
pub const DEFAULT_BACKGROUND: u8 = 128;

/// How the generated block field is mirrored.
/// `X` mirrors top to bottom. `Y` mirrors left to right. `Both` does both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub enum Symmetry {
    Both,
    X,
    Y,
}

#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Params {
    pub iterations: u32,
    pub sub_iterations: u32,
    pub splitter_iterations: u32,
    pub background: u8,
    pub symmetry: Symmetry,
    pub use_splitter: bool,
    pub splitter_blend: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            iterations: DEFAULT_ITERATIONS,
            sub_iterations: DEFAULT_SUB_ITERATIONS,
            splitter_iterations: DEFAULT_SPLITTER_ITERATIONS,
            background: DEFAULT_BACKGROUND,
            symmetry: Symmetry::Both,
            use_splitter: true,
            splitter_blend: true,
        }
    }
}

#[derive(Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    value: u8,
}

pub fn generate(params: &Params, width: u32, height: u32, seed: u64) -> Map {
    let mut rng = Rng::new(seed);
    if width == 0 || height == 0 {
        return Map::filled(width, height, params.background);
    }
    let (region_w, region_h) = region_size(width, height, params.symmetry);
    let mut rects = vec![Rect {
        x: 0,
        y: 0,
        w: region_w as i32,
        h: region_h as i32,
        value: params.background,
    }];
    // Target roughly 12px blocks at the default sliders, and scale with them.
    let target = ((region_w as f32) * (region_h as f32) / 144.0).max(4.0);
    let coarse_scale = (params.iterations as f32 / DEFAULT_ITERATIONS as f32).clamp(0.0, 8.0);
    let detail = (params.sub_iterations as f32 / DEFAULT_SUB_ITERATIONS as f32).clamp(0.0, 8.0);
    split_passes(
        &mut rects,
        &mut rng,
        (target * 0.22 * coarse_scale).round() as u32,
        0.28,
    );
    split_passes(&mut rects, &mut rng, (target * detail).round() as u32, 0.08);

    let mut region = Map::filled(region_w, region_h, params.background);
    for rect in &rects {
        region.fill_rect(rect.x, rect.y, rect.w, rect.h, rect.value);
    }
    if params.use_splitter {
        paint_splitters(&mut region, &rects, &mut rng, params);
    }
    mirror(region, width, height, params.symmetry)
}

fn region_size(width: u32, height: u32, symmetry: Symmetry) -> (u32, u32) {
    match symmetry {
        Symmetry::Both => ((width + 1) / 2, (height + 1) / 2),
        Symmetry::X => (width, (height + 1) / 2),
        Symmetry::Y => ((width + 1) / 2, height),
    }
}

fn split_passes(rects: &mut Vec<Rect>, rng: &mut Rng, count: u32, min_ratio: f32) {
    for _ in 0..count {
        if rects.is_empty() {
            return;
        }
        let index = pick_area(rects, rng);
        let rect = rects[index];
        let Some((left, right)) = split_rect(rect, rng, min_ratio) else {
            continue;
        };
        rects.swap_remove(index);
        rects.push(left);
        rects.push(right);
    }
}

fn pick_area(rects: &[Rect], rng: &mut Rng) -> usize {
    let total: u64 = rects.iter().map(|rect| area(rect)).sum();
    if total == 0 {
        return 0;
    }
    let mut ticket = rng.u64(0, total - 1);
    for (index, rect) in rects.iter().enumerate() {
        let weight = area(rect);
        if ticket < weight {
            return index;
        }
        ticket -= weight;
    }
    rects.len() - 1
}

fn area(rect: &Rect) -> u64 {
    (rect.w.max(0) as u64) * (rect.h.max(0) as u64)
}

fn split_rect(rect: Rect, rng: &mut Rng, min_ratio: f32) -> Option<(Rect, Rect)> {
    if rect.w < 4 || rect.h < 4 {
        return None;
    }
    let vertical = if rect.w > rect.h * 2 {
        true
    } else if rect.h > rect.w * 2 {
        false
    } else {
        rng.bool()
    };
    let span = if vertical { rect.w } else { rect.h };
    let min_cut = ((span as f32) * min_ratio).round().max(1.0) as i32;
    if span < min_cut * 2 + 1 {
        return None;
    }
    let cut = rng.i32(min_cut, span - min_cut);
    let keep = rng.chance(0.65);
    let first_value = if keep {
        rect.value
    } else {
        rng.u32(0, 255) as u8
    };
    let second_value = rng.u32(0, 255) as u8;
    if vertical {
        Some((
            Rect {
                w: cut,
                value: first_value,
                ..rect
            },
            Rect {
                x: rect.x + cut,
                w: rect.w - cut,
                value: second_value,
                ..rect
            },
        ))
    } else {
        Some((
            Rect {
                h: cut,
                value: first_value,
                ..rect
            },
            Rect {
                y: rect.y + cut,
                h: rect.h - cut,
                value: second_value,
                ..rect
            },
        ))
    }
}

fn paint_splitters(map: &mut Map, rects: &[Rect], rng: &mut Rng, params: &Params) {
    if rects.is_empty() {
        return;
    }
    for _ in 0..params.splitter_iterations {
        let rect = rects[rng.index(rects.len())];
        let value = if rng.bool() { 255 } else { 0 };
        if params.splitter_blend {
            map.blend_stroke_rect(rect.x, rect.y, rect.w, rect.h, 1, value, 0.55);
        } else {
            map.stroke_rect(rect.x, rect.y, rect.w, rect.h, 1, value);
        }
    }
}

fn mirror(region: Map, width: u32, height: u32, symmetry: Symmetry) -> Map {
    let mut out = Map::filled(width, height, 0);
    match symmetry {
        Symmetry::Both => {
            let qw = region.width();
            let qh = region.height();
            for y in 0..qh {
                for x in 0..qw {
                    let value = region.get(x as i32, y as i32);
                    out.set(x as i32, y as i32, value);
                    out.set(width as i32 - 1 - x as i32, y as i32, value);
                    out.set(x as i32, height as i32 - 1 - y as i32, value);
                    out.set(
                        width as i32 - 1 - x as i32,
                        height as i32 - 1 - y as i32,
                        value,
                    );
                }
            }
        }
        Symmetry::X => {
            let qh = region.height();
            for y in 0..qh {
                for x in 0..width {
                    let value = region.get(x as i32, y as i32);
                    out.set(x as i32, y as i32, value);
                    out.set(x as i32, height as i32 - 1 - y as i32, value);
                }
            }
        }
        Symmetry::Y => {
            let qw = region.width();
            for y in 0..height {
                for x in 0..qw {
                    let value = region.get(x as i32, y as i32);
                    out.set(x as i32, y as i32, value);
                    out.set(width as i32 - 1 - x as i32, y as i32, value);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_way_symmetry_matches_on_both_axes() {
        let map = generate(&Params::default(), 64, 48, 5);
        for y in 0..map.height() / 2 {
            for x in 0..map.width() / 2 {
                let value = map.get(x as i32, y as i32);
                assert_eq!(value, map.get(map.width() as i32 - 1 - x as i32, y as i32));
                assert_eq!(value, map.get(x as i32, map.height() as i32 - 1 - y as i32));
            }
        }
    }

    #[test]
    fn same_seed_is_stable() {
        let a = generate(&Params::default(), 48, 48, 9);
        let b = generate(&Params::default(), 48, 48, 9);
        assert_eq!(a.pixels(), b.pixels());
    }
}
