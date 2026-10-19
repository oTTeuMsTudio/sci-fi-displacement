//! Procedural sci-fi displacement maps.
//!
//! Five generators follow the modes in JSplacement:
//! stacked plates (`classic`), greeble stamps (`panels`), mirrored blocks (`velvet`),
//! circuit traces (`wyre`), and dot fields (`dots`).
//! Height is an 8-bit grayscale image. Brighter pixels are raised.

pub mod classic;
pub mod dots;
pub mod map;
pub mod panels;
pub mod rng;
pub mod velvet;
pub mod wyre;

#[cfg(target_arch = "wasm32")]
mod wasm_api;
