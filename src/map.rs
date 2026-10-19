use std::path::Path;

/// 8-bit grayscale height map. Brighter pixels are raised.
#[derive(Clone)]
pub struct Map {
    width: u32,
    height: u32,
    pix: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    Metal,
    Cyan,
    Copper,
    Ember,
}

impl Map {
    pub fn filled(width: u32, height: u32, value: u8) -> Self {
        let n = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pix: vec![value; n],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pix
    }

    pub fn from_pixels(width: u32, height: u32, pix: Vec<u8>) -> Option<Self> {
        let expected = (width as usize).saturating_mul(height as usize);
        if pix.len() != expected {
            return None;
        }
        Some(Self { width, height, pix })
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        if !self.contains(x, y) {
            return 0;
        }
        self.pix[self.index(x, y)]
    }

    pub fn set(&mut self, x: i32, y: i32, value: u8) {
        if self.contains(x, y) {
            let i = self.index(x, y);
            self.pix[i] = value;
        }
    }

    pub fn blend_at(&mut self, x: i32, y: i32, value: u8, alpha: f32) {
        if !self.contains(x, y) {
            return;
        }
        let alpha = alpha.clamp(0.0, 1.0);
        let i = self.index(x, y);
        let old = self.pix[i] as f32;
        let mixed = old + (value as f32 - old) * alpha;
        self.pix[i] = mixed.round().clamp(0.0, 255.0) as u8;
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, value: u8) {
        self.visit_rect(x, y, w, h, |map, px, py| map.set(px, py, value));
    }

    pub fn blend_rect(&mut self, x: i32, y: i32, w: i32, h: i32, value: u8, alpha: f32) {
        self.visit_rect(x, y, w, h, |map, px, py| map.blend_at(px, py, value, alpha));
    }

    pub fn stroke_rect(&mut self, x: i32, y: i32, w: i32, h: i32, thickness: i32, value: u8) {
        self.stroke_rect_with(x, y, w, h, thickness, |map, rx, ry, rw, rh| {
            map.fill_rect(rx, ry, rw, rh, value);
        });
    }

    pub fn blend_stroke_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        thickness: i32,
        value: u8,
        alpha: f32,
    ) {
        self.stroke_rect_with(x, y, w, h, thickness, |map, rx, ry, rw, rh| {
            map.blend_rect(rx, ry, rw, rh, value, alpha);
        });
    }

    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, value: u8) {
        if radius <= 0 {
            self.set(cx, cy, value);
            return;
        }
        let r2 = radius * radius;
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r2 {
                    self.set(x, y, value);
                }
            }
        }
    }

    pub fn stroke_circle(&mut self, cx: i32, cy: i32, radius: i32, thickness: i32, value: u8) {
        let thickness = thickness.max(1);
        if radius <= 0 {
            self.set(cx, cy, value);
            return;
        }
        let outer = radius * radius;
        let inner_r = (radius - thickness).max(0);
        let inner = inner_r * inner_r;
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let d = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                if d <= outer && d > inner {
                    self.set(x, y, value);
                }
            }
        }
    }

    pub fn fill_diamond(&mut self, cx: i32, cy: i32, radius: i32, value: u8) {
        if radius <= 0 {
            self.set(cx, cy, value);
            return;
        }
        for y in (cy - radius)..=(cy + radius) {
            let reach = radius - (y - cy).abs();
            self.hline(cx - reach, cx + reach, y, value);
        }
    }

    pub fn stroke_diamond(&mut self, cx: i32, cy: i32, radius: i32, thickness: i32, value: u8) {
        let thickness = thickness.max(1);
        if radius <= 0 {
            self.set(cx, cy, value);
            return;
        }
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let d = (x - cx).abs() + (y - cy).abs();
                if d <= radius && d > radius - thickness {
                    self.set(x, y, value);
                }
            }
        }
    }

    /// Solid capsule stroke. Width is the full diameter in pixels.
    pub fn stroke_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: u32, value: u8) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        if width <= 1 {
            self.line_1px(
                x0.round() as i32,
                y0.round() as i32,
                x1.round() as i32,
                y1.round() as i32,
                value,
            );
            return;
        }
        let radius = width as f32 * 0.5;
        let r2 = radius * radius;
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len2 = dx * dx + dy * dy;
        if len2 < 0.01 {
            self.fill_circle(
                x0.round() as i32,
                y0.round() as i32,
                radius.ceil() as i32,
                value,
            );
            return;
        }
        let min_x = ((x0.min(x1) - radius).floor() as i32).max(0);
        let max_x = ((x0.max(x1) + radius).ceil() as i32).min(self.width as i32 - 1);
        let min_y = ((y0.min(y1) - radius).floor() as i32).max(0);
        let max_y = ((y0.max(y1) + radius).ceil() as i32).min(self.height as i32 - 1);
        if min_x > max_x || min_y > max_y {
            return;
        }
        let len = len2.sqrt();
        let cap = 1.5 / len;
        for y in min_y..=max_y {
            let py = y as f32 - y0;
            let row = (y as u32 * self.width) as usize;
            for x in min_x..=max_x {
                let px = x as f32 - x0;
                let t_raw = (px * dx + py * dy) / len2;
                if t_raw < -cap || t_raw > 1.0 + cap {
                    continue;
                }
                let t = t_raw.clamp(0.0, 1.0);
                let ex = px - dx * t;
                let ey = py - dy * t;
                if ex * ex + ey * ey <= r2 {
                    self.pix[row + x as usize] = value;
                }
            }
        }
    }

    pub fn invert(&mut self) {
        for pixel in &mut self.pix {
            *pixel = 255 - *pixel;
        }
    }

    /// Stamp `gray` where `mask` is non-zero.
    /// `opacity` of 1 replaces. Lower values mix with the pixel already there.
    /// Seamless stamps wrap around both edges so the map can tile.
    pub fn blit(
        &mut self,
        gray: &[u8],
        mask: &[u8],
        src_w: u32,
        src_h: u32,
        dest_x: i32,
        dest_y: i32,
        opacity: f32,
        seamless: bool,
    ) {
        if self.width == 0 || self.height == 0 || src_w == 0 || src_h == 0 {
            return;
        }
        let opacity = opacity.clamp(0.0, 1.0);
        for sy in 0..src_h {
            for sx in 0..src_w {
                let src_i = (sy * src_w + sx) as usize;
                let coverage = mask[src_i] as f32 / 255.0;
                if coverage <= 0.0 {
                    continue;
                }
                let (dx, dy) = if seamless {
                    (
                        (dest_x + sx as i32).rem_euclid(self.width as i32),
                        (dest_y + sy as i32).rem_euclid(self.height as i32),
                    )
                } else {
                    let dx = dest_x + sx as i32;
                    let dy = dest_y + sy as i32;
                    if !self.contains(dx, dy) {
                        continue;
                    }
                    (dx, dy)
                };
                let alpha = coverage * opacity;
                if alpha >= 0.999 {
                    self.set(dx, dy, gray[src_i]);
                } else {
                    self.blend_at(dx, dy, gray[src_i], alpha);
                }
            }
        }
    }

    pub fn save_height(&self, path: &Path) -> Result<(), image::ImageError> {
        let buffer = image::GrayImage::from_raw(self.width, self.height, self.pix.clone())
            .expect("height buffer length matches dimensions");
        buffer.save(path)
    }

    /// Tangent-space normal map.
    /// A raised (brighter) bump points toward the camera: +Z is out of the surface.
    /// `strength` scales slope. `wrap` samples neighbors across the tile boundary.
    /// Packed RGB tangent-space normals. See [`Map::save_normal`].
    pub fn normal_rgb(&self, strength: f32, wrap: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity((self.width as usize) * (self.height as usize) * 3);
        for y in 0..self.height {
            for x in 0..self.width {
                let left = self.sample(x as i32 - 1, y as i32, wrap) as f32;
                let right = self.sample(x as i32 + 1, y as i32, wrap) as f32;
                let up = self.sample(x as i32, y as i32 - 1, wrap) as f32;
                let down = self.sample(x as i32, y as i32 + 1, wrap) as f32;
                let nx = (left - right) / 255.0 * strength;
                let ny = (up - down) / 255.0 * strength;
                let nz = 1.0;
                let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
                out.extend_from_slice(&[
                    encode_normal(nx / len),
                    encode_normal(ny / len),
                    encode_normal(nz / len),
                ]);
            }
        }
        out
    }

    /// Packed RGB colorizer image.
    pub fn color_rgb(&self, palette: Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pix.len().saturating_mul(3));
        for &height in &self.pix {
            out.extend_from_slice(&palette_color(height, palette));
        }
        out
    }

    pub fn save_normal(
        &self,
        path: &Path,
        strength: f32,
        wrap: bool,
    ) -> Result<(), image::ImageError> {
        let image = image::RgbImage::from_raw(self.width, self.height, self.normal_rgb(strength, wrap))
            .expect("normal buffer length matches dimensions");
        image.save(path)
    }

    pub fn save_color(&self, path: &Path, palette: Palette) -> Result<(), image::ImageError> {
        let image = image::RgbImage::from_raw(self.width, self.height, self.color_rgb(palette))
            .expect("color buffer length matches dimensions");
        image.save(path)
    }

    /// RGBA preview. `None` is grayscale height. `Some` paints the colorizer palette.
    pub fn preview_rgba(&self, colorize: Option<Palette>) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pix.len().saturating_mul(4));
        for &height in &self.pix {
            let [r, g, b] = match colorize {
                Some(palette) => palette_color(height, palette),
                None => [height, height, height],
            };
            out.extend_from_slice(&[r, g, b, 255]);
        }
        out
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y as u32 * self.width + x as u32) as usize
    }

    fn sample(&self, x: i32, y: i32, wrap: bool) -> u8 {
        let (x, y) = if wrap {
            (
                x.rem_euclid(self.width as i32),
                y.rem_euclid(self.height as i32),
            )
        } else if self.width == 0 || self.height == 0 {
            return 0;
        } else {
            (
                x.clamp(0, self.width as i32 - 1),
                y.clamp(0, self.height as i32 - 1),
            )
        };
        self.pix[self.index(x, y)]
    }

    fn hline(&mut self, x0: i32, x1: i32, y: i32, value: u8) {
        if y < 0 || y >= self.height as i32 || self.width == 0 {
            return;
        }
        let mut x = x0.min(x1).max(0);
        let end = x0.max(x1).min(self.width as i32 - 1);
        if x > end {
            return;
        }
        let row = (y as u32 * self.width) as usize;
        while x <= end {
            self.pix[row + x as usize] = value;
            x += 1;
        }
    }

    fn line_1px(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, value: u8) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            self.set(x0, y0, value);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 > -dy {
                err -= dy;
                x0 += sx;
            }
            if e2 < dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn visit_rect<F>(&mut self, x: i32, y: i32, w: i32, h: i32, mut visit: F)
    where
        F: FnMut(&mut Self, i32, i32),
    {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        for py in y0..y1 {
            for px in x0..x1 {
                visit(self, px, py);
            }
        }
    }

    fn stroke_rect_with<F>(&mut self, x: i32, y: i32, w: i32, h: i32, thickness: i32, mut paint: F)
    where
        F: FnMut(&mut Self, i32, i32, i32, i32),
    {
        if w <= 0 || h <= 0 || thickness <= 0 {
            return;
        }
        let t = thickness.min(w).min(h);
        paint(self, x, y, w, t);
        if h > t {
            paint(self, x, y + h - t, w, t);
        }
        if h > t * 2 {
            paint(self, x, y + t, t, h - t * 2);
            if w > t {
                paint(self, x + w - t, y + t, t, h - t * 2);
            }
        }
    }
}

fn encode_normal(component: f32) -> u8 {
    ((component * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8
}

fn palette_color(height: u8, palette: Palette) -> [u8; 3] {
    let t = height as f32 / 255.0;
    let stops: &[[u8; 3]] = match palette {
        Palette::Metal => &[
            [14, 16, 20],
            [48, 52, 58],
            [108, 114, 122],
            [176, 182, 190],
            [236, 240, 244],
        ],
        Palette::Cyan => &[
            [6, 14, 20],
            [16, 48, 64],
            [28, 112, 132],
            [150, 220, 226],
            [236, 255, 255],
        ],
        Palette::Copper => &[
            [28, 14, 10],
            [92, 42, 24],
            [168, 86, 42],
            [214, 150, 86],
            [245, 224, 186],
        ],
        Palette::Ember => &[
            [12, 6, 8],
            [92, 16, 18],
            [180, 54, 16],
            [232, 140, 42],
            [255, 236, 210],
        ],
    };
    sample_stops(stops, t)
}

fn sample_stops(stops: &[[u8; 3]], t: f32) -> [u8; 3] {
    if stops.is_empty() {
        return [0, 0, 0];
    }
    if stops.len() == 1 || t <= 0.0 {
        return stops[0];
    }
    if t >= 1.0 {
        return stops[stops.len() - 1];
    }
    let scaled = t * (stops.len() - 1) as f32;
    let index = scaled.floor() as usize;
    let frac = scaled - index as f32;
    let a = stops[index];
    let b = stops[(index + 1).min(stops.len() - 1)];
    [
        lerp_u8(a[0], b[0], frac),
        lerp_u8(a[1], b[1], frac),
        lerp_u8(a[2], b[2], frac),
    ]
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

pub fn shade(base: u8, delta: i32) -> u8 {
    (base as i32 + delta).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_rect_clips_to_the_map() {
        let mut map = Map::filled(4, 4, 0);
        map.fill_rect(-2, -2, 4, 4, 200);
        assert_eq!(map.get(0, 0), 200);
        assert_eq!(map.get(1, 1), 200);
        assert_eq!(map.get(2, 2), 0);
    }

    #[test]
    fn blit_wraps_across_the_seam() {
        let mut map = Map::filled(8, 4, 0);
        let gray = [255u8, 10];
        let mask = [255u8, 255];
        map.blit(&gray, &mask, 2, 1, 7, 0, 1.0, true);
        assert_eq!(map.get(7, 0), 255);
        assert_eq!(map.get(0, 0), 10);
    }

    #[test]
    fn invert_flips_every_pixel() {
        let mut map = Map::filled(2, 1, 30);
        map.set(1, 0, 200);
        map.invert();
        assert_eq!(map.get(0, 0), 225);
        assert_eq!(map.get(1, 0), 55);
    }

    #[test]
    fn preview_rgba_is_gray_height() {
        let mut map = Map::filled(2, 2, 10);
        map.set(1, 0, 200);
        let rgba = map.preview_rgba(None);
        assert_eq!(rgba.len(), 16);
        assert_eq!(&rgba[0..4], &[10, 10, 10, 255]);
        assert_eq!(&rgba[4..8], &[200, 200, 200, 255]);
    }

    #[test]
    fn diagonal_stroke_is_solid() {
        let mut map = Map::filled(40, 40, 0);
        map.stroke_line(4.0, 4.0, 34.0, 34.0, 7, 255);
        let x = 18;
        let y = 18;
        assert_eq!(map.get(x, y), 255);
        let neighbors = [
            map.get(x - 1, y),
            map.get(x + 1, y),
            map.get(x, y - 1),
            map.get(x, y + 1),
        ];
        let adjacent = neighbors.iter().filter(|pixel| **pixel == 255).count();
        assert!(adjacent >= 2, "stroke broke into dots, adjacent={adjacent}");
    }

    #[test]
    fn flat_normal_points_outward() {
        let map = Map::filled(3, 3, 128);
        let dir = std::env::temp_dir().join("displacement-gen-flat-normal.png");
        map.save_normal(&dir, 4.0, false).unwrap();
        let image = image::open(&dir).unwrap().to_rgb8();
        let pixel = image.get_pixel(1, 1).0;
        assert!((pixel[0] as i32 - 128).abs() <= 1);
        assert!((pixel[1] as i32 - 128).abs() <= 1);
        assert!(pixel[2] > 250);
        let _ = std::fs::remove_file(&dir);
    }
}
