use crate::map::Map;
use crate::rng::Rng;

pub const DEFAULT_LAYERS: u32 = 35;
pub const DEFAULT_WIRES: u32 = 3;
pub const DEFAULT_MAX_WIDTH: u32 = 64;
pub const DEFAULT_SPREAD: u32 = 0;
pub const DEFAULT_MIN_STEP: u32 = 128;
pub const DEFAULT_MAX_STEP: u32 = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub enum Ramp {
    Linear,
    InvertedLinear,
    Curve,
    InvertedCurve,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub enum Directions {
    Any,
    Cardinal,
    Diagonal,
    Conical,
}

#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Deserialize))]
#[cfg_attr(target_arch = "wasm32", serde(rename_all = "camelCase"))]
pub struct Params {
    pub layers: u32,
    pub wires_per_layer: u32,
    pub max_width: u32,
    /// How far a wire may start from its layer anchor. Zero pins every wire in the layer to that point.
    pub origin_spread: u32,
    pub min_step: u32,
    pub max_step: u32,
    pub color: Ramp,
    pub width: Ramp,
    pub directions: Directions,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            layers: DEFAULT_LAYERS,
            wires_per_layer: DEFAULT_WIRES,
            max_width: DEFAULT_MAX_WIDTH,
            origin_spread: DEFAULT_SPREAD,
            min_step: DEFAULT_MIN_STEP,
            max_step: DEFAULT_MAX_STEP,
            color: Ramp::Linear,
            width: Ramp::Random,
            directions: Directions::Any,
        }
    }
}

pub fn generate(params: &Params, width: u32, height: u32, seed: u64) -> Map {
    let mut rng = Rng::new(seed);
    let mut map = Map::filled(width, height, 0);
    if width == 0 || height == 0 || params.layers == 0 || params.wires_per_layer == 0 {
        return map;
    }
    let (min_step, max_step) = ordered(params.min_step.max(1), params.max_step.max(1));
    let max_width = params.max_width.max(1);
    let layer_span = params.layers.saturating_sub(1).max(1) as f32;

    for layer in 0..params.layers {
        let t = layer as f32 / layer_span;
        let layer_color = ramp(params.color, t, &mut rng);
        let layer_width = ramp(params.width, t, &mut rng);
        let anchor_x = rng.i32(0, width.saturating_sub(1) as i32) as f32;
        let anchor_y = rng.i32(0, height.saturating_sub(1) as i32) as f32;

        for _ in 0..params.wires_per_layer {
            let color_t = if params.color == Ramp::Random {
                rng.f32()
            } else {
                layer_color
            };
            let width_t = if params.width == Ramp::Random {
                rng.f32()
            } else {
                layer_width
            };
            let brightness = (24.0 + color_t * 231.0).round().clamp(0.0, 255.0) as u8;
            let stroke = (1.0 + width_t * (max_width.saturating_sub(1) as f32))
                .round()
                .max(1.0) as u32;
            let (mut x, mut y) = jitter(anchor_x, anchor_y, params.origin_spread, &mut rng);
            let mut heading = None;
            let steps = rng.u32(3, 6);
            for _ in 0..steps {
                let angle = next_angle(params.directions, heading, &mut rng);
                heading = Some(angle);
                let length = rng.u32(min_step, max_step) as f32;
                let nx = x + angle.cos() * length;
                let ny = y + angle.sin() * length;
                map.stroke_line(x, y, nx, ny, stroke, brightness);
                x = nx;
                y = ny;
            }
        }
    }
    map
}

fn jitter(anchor_x: f32, anchor_y: f32, spread: u32, rng: &mut Rng) -> (f32, f32) {
    if spread == 0 {
        return (anchor_x, anchor_y);
    }
    let spread = spread as i32;
    let x = anchor_x + rng.i32(-spread, spread) as f32;
    let y = anchor_y + rng.i32(-spread, spread) as f32;
    (x, y)
}

fn next_angle(mode: Directions, heading: Option<f32>, rng: &mut Rng) -> f32 {
    let quarter = std::f32::consts::FRAC_PI_2;
    match mode {
        Directions::Any => rng.f32() * std::f32::consts::TAU,
        Directions::Cardinal => rng.u32(0, 3) as f32 * quarter,
        Directions::Diagonal => std::f32::consts::FRAC_PI_4 + rng.u32(0, 3) as f32 * quarter,
        Directions::Conical => {
            let base = heading.unwrap_or_else(|| rng.f32() * std::f32::consts::TAU);
            base + (rng.f32() - 0.5) * 0.7
        }
    }
}

fn ramp(mode: Ramp, t: f32, rng: &mut Rng) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match mode {
        Ramp::Linear => t,
        Ramp::InvertedLinear => 1.0 - t,
        Ramp::Curve => t * t,
        Ramp::InvertedCurve => 1.0 - t * t,
        Ramp::Random => rng.f32(),
    }
}

fn ordered(a: u32, b: u32) -> (u32, u32) {
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
    fn wires_are_deterministic_and_visible() {
        let params = Params {
            layers: 6,
            wires_per_layer: 2,
            max_width: 3,
            min_step: 16,
            max_step: 40,
            ..Params::default()
        };
        let a = generate(&params, 80, 80, 4);
        let b = generate(&params, 80, 80, 4);
        assert_eq!(a.pixels(), b.pixels());
        assert!(a.pixels().iter().any(|pixel| *pixel > 0));
    }
}
