//! Browser entry points. The studio UI sends one JSON spec and receives
//! grayscale pixels. PNG export uses the same encoders as the desktop app.

use std::cell::RefCell;
use std::io::Cursor;

use wasm_bindgen::prelude::*;

use crate::classic;
use crate::dots;
use crate::map::{Map, Palette};
use crate::panels::{self, Sprite};
use crate::velvet;
use crate::wyre;

thread_local! {
    static SPRITES: RefCell<Vec<Sprite>> = const { RefCell::new(Vec::new()) };
}

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn clear_sprites() {
    SPRITES.with(|sprites| sprites.borrow_mut().clear());
}

#[wasm_bindgen]
pub fn add_sprite(width: u32, height: u32, gray: &[u8], mask: &[u8]) -> Result<(), JsValue> {
    let count = (width as usize).saturating_mul(height as usize);
    if width == 0 || height == 0 || gray.len() != count || mask.len() != count {
        return Err(JsValue::from_str(
            "sprite dimensions do not match its buffers",
        ));
    }
    SPRITES.with(|sprites| {
        sprites.borrow_mut().push(Sprite {
            w: width,
            h: height,
            gray: gray.to_vec(),
            mask: mask.to_vec(),
        });
    });
    Ok(())
}

fn de_seed<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visit;
    impl serde::de::Visitor<'_> for Visit {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u64 or a decimal string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<u64, E> {
            u64::try_from(value).map_err(E::custom)
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<u64, E> {
            value.parse::<u64>().map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Visit)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spec {
    mode: String,
    size: u32,
    #[serde(deserialize_with = "de_seed")]
    seed: u64,
    classic: Option<classic::Params>,
    panels: Option<panels::Params>,
    velvet: Option<velvet::Params>,
    wyre: Option<wyre::Params>,
    dots: Option<dots::Params>,
}

#[wasm_bindgen]
pub struct MapBytes {
    width: u32,
    height: u32,
    gray: Vec<u8>,
}

#[wasm_bindgen]
impl MapBytes {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn take_gray(self) -> Vec<u8> {
        self.gray
    }
}

#[wasm_bindgen]
pub fn generate(spec_json: &str) -> Result<MapBytes, JsValue> {
    let spec: Spec = serde_json::from_str(spec_json).map_err(|err| JsValue::from_str(&err.to_string()))?;
    if spec.size < 16 || spec.size > 2048 {
        return Err(JsValue::from_str("size must be from 16 to 2048"));
    }
    let size = spec.size;
    let map = match spec.mode.as_str() {
        "classic" => classic::generate(&spec.classic.unwrap_or_default(), size, size, spec.seed),
        "panels" => {
            let mut params = spec.panels.unwrap_or_default();
            if params.pack == panels::Pack::Custom {
                params.custom = SPRITES.with(|sprites| sprites.borrow().clone());
                if params.custom.is_empty() {
                    return Err(JsValue::from_str(
                        "Load sprite images for the custom pack",
                    ));
                }
            }
            panels::generate(&params, size, size, spec.seed)
        }
        "velvet" => velvet::generate(&spec.velvet.unwrap_or_default(), size, size, spec.seed),
        "wyre" => wyre::generate(&spec.wyre.unwrap_or_default(), size, size, spec.seed),
        "dots" => dots::generate(&spec.dots.unwrap_or_default(), size, size, spec.seed),
        other => return Err(JsValue::from_str(&format!("unknown mode {other}"))),
    };
    Ok(MapBytes {
        width: map.width(),
        height: map.height(),
        gray: map.pixels().to_vec(),
    })
}

fn map_from(gray: &[u8], width: u32, height: u32) -> Result<Map, JsValue> {
    Map::from_pixels(width, height, gray.to_vec())
        .ok_or_else(|| JsValue::from_str("height buffer does not match the size"))
}

fn palette_from(name: &str) -> Result<Palette, JsValue> {
    match name {
        "metal" => Ok(Palette::Metal),
        "cyan" => Ok(Palette::Cyan),
        "copper" => Ok(Palette::Copper),
        "ember" => Ok(Palette::Ember),
        other => Err(JsValue::from_str(&format!("unknown palette {other}"))),
    }
}

fn write_png<P, Container>(image: &image::ImageBuffer<P, Container>) -> Result<Vec<u8>, JsValue>
where
    P: image::Pixel + image::PixelWithColorType,
    [P::Subpixel]: image::EncodableLayout,
    Container: std::ops::Deref<Target = [P::Subpixel]>,
{
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    Ok(cursor.into_inner())
}

#[wasm_bindgen]
pub fn height_png(gray: &[u8], width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    let map = map_from(gray, width, height)?;
    let buffer = image::GrayImage::from_raw(width, height, map.pixels().to_vec())
        .ok_or_else(|| JsValue::from_str("height buffer does not match the size"))?;
    write_png(&buffer)
}

#[wasm_bindgen]
pub fn normal_png(
    gray: &[u8],
    width: u32,
    height: u32,
    strength: f32,
    wrap: bool,
) -> Result<Vec<u8>, JsValue> {
    let map = map_from(gray, width, height)?;
    let buffer = image::RgbImage::from_raw(width, height, map.normal_rgb(strength, wrap))
        .ok_or_else(|| JsValue::from_str("normal buffer does not match the size"))?;
    write_png(&buffer)
}

#[wasm_bindgen]
pub fn color_png(
    gray: &[u8],
    width: u32,
    height: u32,
    palette: &str,
) -> Result<Vec<u8>, JsValue> {
    let map = map_from(gray, width, height)?;
    let buffer = image::RgbImage::from_raw(width, height, map.color_rgb(palette_from(palette)?))
        .ok_or_else(|| JsValue::from_str("color buffer does not match the size"))?;
    write_png(&buffer)
}
