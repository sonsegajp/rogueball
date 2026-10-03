//! Art: the projection shared with the Blender pipeline, the parts kit rendered by art/kit.py, and the
//! per-table art built by bake.rs from a generated layout.

use crate::font::Font;
use crate::table::Layout;
use macroquad::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Copy)]
pub struct Proj { pub x0: f64, pub sy1: f64, pub ppm: f64, pub phi: f64, pub w: u32, pub h: u32 }

impl Proj {
    /// layout (x, y, z) → pixel in the table images
    pub fn to_px(&self, x: f64, y: f64, z: f64) -> Vec2 {
        let sy = y * self.phi.cos() + z * self.phi.sin();
        vec2(((x - self.x0) * self.ppm) as f32, ((self.sy1 - sy) * self.ppm) as f32)
    }
}

mod embedded { include!(concat!(env!("OUT_DIR"), "/embedded.rs")); }

/// an art file built into the binary (see build.rs), or read from disk/the server as a fallback
pub async fn read(rel: &str) -> Option<Vec<u8>> {
    if let Some((_, b)) = embedded::FILES.iter().find(|(p, _)| *p == rel) { return Some(b.to_vec()); }
    macroquad::file::load_file(&asset_path(rel)).await.ok()
}

/// where an asset lives: next to the page on the web, found on disk on desktop
pub fn asset_path(rel: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    { format!("assets/{rel}") }
    #[cfg(not(target_arch = "wasm32"))]
    { asset_dir().join(rel).to_string_lossy().to_string() }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn asset_dir() -> std::path::PathBuf {
    // next to the exe in a release build, or the crate root during development
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    for dir in [exe.clone(), exe.as_ref().and_then(|p| p.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf())), Some(std::env::current_dir().unwrap())] {
        if let Some(d) = dir {
            if d.join("assets").join("kit").exists() {
                return d.join("assets");
            }
        }
    }
    std::path::PathBuf::from("assets")
}

#[derive(Deserialize)]
struct KitDef { file: String, frames: u32, w: u32, h: u32, ox: f32, oy: f32 }

/// one rendered part: a strip of frames, and where its origin sits relative to the top-left corner
pub struct KitSprite { pub img: Image, pub tex: Texture2D, pub frames: u32, pub w: u32, pub h: u32, pub ox: f32, pub oy: f32 }

pub struct Kit { pub sprites: HashMap<String, KitSprite> }

impl Kit {
    /// the parts kit at one scale ("s100" is 1 px = 1 mm, "s80" is 0.8 px per mm for wide machines)
    pub async fn load(scale: &str) -> Kit {
        let mut sprites = HashMap::new();
        let defs: HashMap<String, KitDef> = match read(&format!("kit/{scale}/kit.json")).await {
            Some(b) => serde_json::from_slice(&b).unwrap_or_default(),
            None => HashMap::new(),
        };
        for (name, d) in defs {
            if let Some(bytes) = read(&format!("kit/{scale}/{}", d.file)).await {
                let img = Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).unwrap_or_else(|_| Image::gen_image_color(1, 1, BLANK));
                let tex = Texture2D::from_image(&img);
                tex.set_filter(FilterMode::Nearest);
                sprites.insert(name, KitSprite { img, tex, frames: d.frames, w: d.w, h: d.h, ox: d.ox, oy: d.oy });
            }
        }
        Kit { sprites }
    }

    /// draw a frame with the sprite's origin at pixel `at`
    pub fn draw(&self, name: &str, frame: u32, at: Vec2, tint: Color) -> bool {
        let Some(s) = self.sprites.get(name) else { return false };
        let f = frame.min(s.frames - 1);
        draw_texture_ex(&s.tex, (at.x + s.ox).round(), (at.y + s.oy).round(), tint, DrawTextureParams {
            source: Some(Rect::new((f * s.w) as f32, 0.0, s.w as f32, s.h as f32)),
            ..Default::default()
        });
        true
    }

    pub fn frames(&self, name: &str) -> u32 { self.sprites.get(name).map(|s| s.frames).unwrap_or(1) }
}

/// the art for one generated table
pub struct TableArt {
    pub proj: Proj,
    pub base: Texture2D,
    pub over: Texture2D,
    pub glass: Texture2D,
    /// lit insert sprites, parallel to layout.inserts: texture and its top-left pixel
    pub lit: Vec<(Texture2D, Vec2)>,
    /// which kit to draw parts from: 0 = s100, 1 = s80
    pub kit: usize,
}

fn tex(img: &Image) -> Texture2D {
    let t = Texture2D::from_image(img);
    t.set_filter(FilterMode::Nearest);
    t
}

impl TableArt {
    pub fn build(l: &Layout, kits: &[Kit; 2], font: &Font) -> TableArt {
        let proj = crate::bake::proj_for(l);
        let kit = if proj.ppm >= 999.0 { 0 } else { 1 };
        let b = crate::bake::bake(l, &kits[kit], font);
        TableArt {
            proj,
            base: tex(&b.base),
            over: tex(&b.over),
            glass: tex(&b.glass),
            lit: b.lit.iter().map(|(img, at)| (tex(img), *at)).collect(),
            kit,
        }
    }
}
