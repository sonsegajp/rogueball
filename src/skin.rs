//! Arcade UI kit: beveled metal frames, felt trays, chunky buttons, ribbon banners, a dot-matrix display,
//! LED bars and hand-drawn pixel icons. Everything is drawn in canvas pixels from the game palette.

use crate::font::Font;
use crate::game::hex;
use macroquad::prelude::*;
use std::collections::HashMap;

pub const OUTLINE: u32 = 0x0b0a14;
pub const METAL_HI: u32 = 0xeef0f7;
pub const METAL_L: u32 = 0xc7c7d9;
pub const METAL_M: u32 = 0x9a99b3;
pub const METAL_D: u32 = 0x4a4766;
pub const FELT: u32 = 0x1e1545;
pub const FELT_D: u32 = 0x120c2b;
pub const DMD_ON: u32 = 0xff8a1c;
pub const DMD_DIM: u32 = 0x3d1d08;

const ICONS: &[(&str, &str, &[(char, u32)])] = &[
    ("coin", "..kkkkk..|.kyyyyyk.|kyywyyygk|kywyyyygk|kyyyyyygk|kyyyyyygk|kgyyyyggk|.kgggggk.|..kkkkk..",
     &[('k', 0x3d1d08), ('y', 0xffcc4d), ('w', 0xfff1a8), ('g', 0xe89a1c)]),
    ("ball", "..kkkkk..|.kmlllmk.|kmlwwllmk|kmlwllmdk|kmllllmdk|kmmlmmddk|kdmmmdddk|.kdddddk.|..kkkkk..",
     &[('k', 0x1b1830), ('d', 0x4a4766), ('m', 0x9a99b3), ('l', 0xc7c7d9), ('w', 0xffffff)]),
    ("ball_off", "..kkkkk..|.kdddddk.|kdddddddk|kdddddddk|kdddddddk|kdddddddk|kdddddddk|.kdddddk.|..kkkkk..",
     &[('k', 0x1b1830), ('d', 0x2e2b47)]),
    ("lock", ".kkkkk.|.k...k.|.k...k.|kkkkkkk|kyyyyyk|kyykyyk|kyykyyk|kyyyyyk|kkkkkkk",
     &[('k', 0x1b1830), ('y', 0x9a99b3)]),
    ("star", "....k....|...kyk...|kkkkykkkk|kyyyyyyyk|.kyyyyyk.|..kyyyk..|.kyykyyk.|kyk...kyk|kk.....kk",
     &[('k', 0x3d1d08), ('y', 0xffcc4d)]),
    ("gem", "..k..|.kwk.|kwcck|.kck.|..k..", &[('k', 0x0b0a14), ('w', 0xffffff), ('c', 0xffffff)]),
    ("skull", ".kkkkk.|kwwwwwk|kwkwkwk|kwwwwwk|.kwkwk.|..kkk..", &[('k', 0x1b1830), ('w', 0xeef0f7)]),
];

pub struct Skin {
    felt: Texture2D,
    grid: Texture2D,
    icons: HashMap<&'static str, Texture2D>,
}

fn rgba(c: u32) -> Color { hex(c) }

impl Skin {
    pub fn new() -> Skin {
        // felt: a quiet two-tone dither so trays read as cloth, not flat fill
        let (w, h) = (960u16, 540u16);
        let mut felt = Image::gen_image_color(w, h, rgba(FELT));
        let bayer = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        for y in 0..h as u32 {
            for x in 0..w as u32 {
                if bayer[(y % 4) as usize][(x % 4) as usize] < 3 { felt.set_pixel(x, y, rgba(0x241a52)); }
                if bayer[(y % 4) as usize][(x % 4) as usize] == 15 { felt.set_pixel(x, y, rgba(0x1b1238)); }
            }
        }
        let felt = Texture2D::from_image(&felt);
        felt.set_filter(FilterMode::Nearest);
        // dot-matrix grid: unlit dots on a 3px pitch
        let (gw, gh) = (420u16, 240u16);
        let mut grid = Image::gen_image_color(gw, gh, rgba(0x120804));
        for y in 0..gh as u32 {
            for x in 0..gw as u32 {
                if x % 3 < 2 && y % 3 < 2 { grid.set_pixel(x, y, rgba(0x2a1306)); }
            }
        }
        let grid = Texture2D::from_image(&grid);
        grid.set_filter(FilterMode::Nearest);
        let mut icons = HashMap::new();
        for (name, art, pal) in ICONS {
            let rows: Vec<&str> = art.split('|').collect();
            let mut img = Image::gen_image_color(rows[0].len() as u16, rows.len() as u16, Color::new(0.0, 0.0, 0.0, 0.0));
            for (y, r) in rows.iter().enumerate() {
                for (x, ch) in r.chars().enumerate() {
                    if let Some((_, c)) = pal.iter().find(|(k, _)| *k == ch) { img.set_pixel(x as u32, y as u32, rgba(*c)); }
                }
            }
            let t = Texture2D::from_image(&img);
            t.set_filter(FilterMode::Nearest);
            icons.insert(*name, t);
        }
        Skin { felt, grid, icons }
    }

    pub fn icon(&self, name: &str, x: f32, y: f32, scale: f32, tint: Color) {
        if let Some(t) = self.icons.get(name) {
            draw_texture_ex(t, x.round(), y.round(), tint, DrawTextureParams { dest_size: Some(vec2(t.width() * scale, t.height() * scale)), ..Default::default() });
        }
    }
    pub fn icon_w(&self, name: &str) -> f32 { self.icons.get(name).map(|t| t.width()).unwrap_or(0.0) }

    /// felt-textured tray recessed into the surface
    pub fn tray(&self, r: Rect) {
        draw_texture_ex(&self.felt, r.x, r.y, WHITE, DrawTextureParams { source: Some(Rect::new(r.x, r.y, r.w, r.h)), dest_size: Some(vec2(r.w, r.h)), ..Default::default() });
        // inner shadow: dark top/left, light bottom/right lip
        draw_rectangle(r.x, r.y, r.w, 2.0, rgba(OUTLINE));
        draw_rectangle(r.x, r.y, 2.0, r.h, rgba(OUTLINE));
        draw_rectangle(r.x, r.y + r.h - 1.0, r.w, 1.0, rgba(0x42308a));
        draw_rectangle(r.x + r.w - 1.0, r.y, 1.0, r.h, rgba(0x42308a));
    }

    /// heavy brushed-metal frame with rivets, drawn around `r` (r is the inside)
    pub fn bezel(&self, r: Rect) {
        let (x, y, w, h) = (r.x - 5.0, r.y - 5.0, r.w + 10.0, r.h + 10.0);
        draw_rectangle(x, y, w, h, rgba(OUTLINE));
        draw_rectangle(x + 1.0, y + 1.0, w - 2.0, h - 2.0, rgba(METAL_M));
        draw_rectangle(x + 1.0, y + 1.0, w - 2.0, 1.0, rgba(METAL_HI));
        draw_rectangle(x + 1.0, y + 1.0, 1.0, h - 2.0, rgba(METAL_L));
        draw_rectangle(x + 1.0, y + h - 2.0, w - 2.0, 1.0, rgba(METAL_D));
        draw_rectangle(x + w - 2.0, y + 1.0, 1.0, h - 2.0, rgba(METAL_D));
        // brushed streaks
        let mut k = 0.0;
        while k < w - 8.0 {
            draw_rectangle(x + 4.0 + k, y + 2.0, 3.0, 1.0, rgba(METAL_L));
            k += 11.0;
        }
        draw_rectangle(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, rgba(OUTLINE));
        for (cx, cy) in [(x + 2.0, y + 2.0), (x + w - 4.0, y + 2.0), (x + 2.0, y + h - 4.0), (x + w - 4.0, y + h - 4.0)] {
            draw_rectangle(cx, cy, 2.0, 2.0, rgba(METAL_HI));
            draw_rectangle(cx + 1.0, cy + 1.0, 1.0, 1.0, rgba(METAL_D));
        }
    }

    /// beveled plate: outline, highlight top/left, shadow bottom/right
    pub fn plate(&self, r: Rect, base: u32, hi: u32, lo: u32) {
        draw_rectangle(r.x, r.y, r.w, r.h, rgba(OUTLINE));
        draw_rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, rgba(base));
        draw_rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, 1.0, rgba(hi));
        draw_rectangle(r.x + 1.0, r.y + 1.0, 1.0, r.h - 2.0, rgba(hi));
        draw_rectangle(r.x + 1.0, r.y + r.h - 2.0, r.w - 2.0, 1.0, rgba(lo));
        draw_rectangle(r.x + r.w - 2.0, r.y + 1.0, 1.0, r.h - 2.0, rgba(lo));
    }

    /// ribbon banner with notched tails and an outlined title
    pub fn banner(&self, font: &Font, r: Rect, text: &str, base: u32, hi: u32, lo: u32, scale: f32) {
        // folded tails sit a little lower and darker behind the face
        let tail = 9.0;
        for tx in [r.x - tail + 3.0, r.x + r.w - 3.0] {
            self.plate(Rect::new(tx, r.y + 4.0, tail, r.h - 2.0), lo, base, OUTLINE);
        }
        self.plate(r, base, hi, lo);
        font.draw_outlined_centered(text, r.x + r.w / 2.0, r.y + (r.h - 7.0 * scale) / 2.0, scale, rgba(METAL_HI));
    }

    /// chunky arcade button; returns (hovered, pressed-visual)
    pub fn button_face(&self, font: &Font, r: Rect, label: &str, kind: u8, hover: bool, down: bool, enabled: bool) {
        let (base, hi, lo, depth) = match (enabled, kind) {
            (false, _) => (0x2e2b47, 0x4a4766, 0x1b1830, 0x0b0a14),
            (true, 1) => if hover { (0xff6aa8, 0xffb3d2, 0xa81c66, 0x6b1048) } else { (0xe0337f, 0xff6aa8, 0xa81c66, 0x6b1048) },
            (true, 2) => if hover { (0x3fc46a, 0x8ff09e, 0x1f8a4c, 0x145233) } else { (0x1f8a4c, 0x3fc46a, 0x145233, 0x0c2a1c) },
            (true, 3) => if hover { (0xffcc4d, 0xfff1a8, 0xb8650f, 0x7a3d0c) } else { (0xe89a1c, 0xffcc4d, 0xb8650f, 0x7a3d0c) },
            _ => if hover { (0x5b45b0, 0x7d68d4, 0x2e2063, 0x1e1545) } else { (0x42308a, 0x5b45b0, 0x2e2063, 0x1e1545) },
        };
        let press = if down && hover && enabled { 2.0 } else { 0.0 };
        // drop + depth
        draw_rectangle(r.x + 1.0, r.y + 3.0, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.35));
        draw_rectangle(r.x, r.y + 2.0, r.w, r.h, rgba(OUTLINE));
        draw_rectangle(r.x + 1.0, r.y + r.h - 1.0, r.w - 2.0, 2.0, rgba(depth));
        let face = Rect::new(r.x, r.y + press, r.w, r.h - 1.0);
        self.plate(face, base, hi, lo);
        let s = if r.h >= 26.0 { 2.0 } else { 1.0 };
        let col = if enabled { rgba(METAL_HI) } else { rgba(METAL_M) };
        font.draw_outlined_centered(label, r.x + r.w / 2.0, face.y + (face.h - 7.0 * s) / 2.0, s, col);
    }

    /// dot-matrix screen: unlit dot grid inside a bezel
    pub fn dmd(&self, r: Rect) {
        self.bezel(r);
        draw_texture_ex(&self.grid, r.x, r.y, WHITE, DrawTextureParams { source: Some(Rect::new(0.0, 0.0, r.w, r.h)), dest_size: Some(vec2(r.w, r.h)), ..Default::default() });
    }

    /// text as lit dots on the dot-matrix (pitch 3 = 2px dots; pitch 2 = 1px dots)
    pub fn dmd_text(&self, font: &Font, s: &str, x: f32, y: f32, pitch: f32, color: Color) {
        let dot = if pitch >= 3.0 { 2.0 } else { 1.0 };
        let mut cx = (x / pitch).round() * pitch;
        let y = (y / pitch).round() * pitch;
        let glow = Color::new(color.r, color.g, color.b, 0.25);
        for c in s.chars() {
            let bits = font.bits(c);
            let w = bits.first().map(|r| r.len()).unwrap_or(3);
            for (j, row) in bits.iter().enumerate() {
                for (i, on) in row.iter().enumerate() {
                    if *on {
                        let px = cx + i as f32 * pitch;
                        let py = y + j as f32 * pitch;
                        if dot > 1.0 { draw_rectangle(px - 1.0, py - 1.0, dot + 2.0, dot + 2.0, glow); }
                        draw_rectangle(px, py, dot, dot, color);
                    }
                }
            }
            cx += (w as f32 + 1.0) * pitch;
        }
    }
    pub fn dmd_width(&self, font: &Font, s: &str, pitch: f32) -> f32 { font.width(s, 1.0) * pitch }

    /// segmented LED bar
    pub fn leds(&self, r: Rect, segs: usize, frac: f32, on: u32, off: u32) {
        draw_rectangle(r.x, r.y, r.w, r.h, rgba(OUTLINE));
        let gap = 1.0;
        let sw = ((r.w - 2.0 - gap * (segs as f32 - 1.0)) / segs as f32).floor();
        let lit = (frac * segs as f32).ceil() as usize;
        for i in 0..segs {
            let sx = r.x + 1.0 + i as f32 * (sw + gap);
            let c = if i < lit { on } else { off };
            draw_rectangle(sx, r.y + 1.0, sw, r.h - 2.0, rgba(c));
            if i < lit { draw_rectangle(sx, r.y + 1.0, sw, 1.0, Color::new(1.0, 1.0, 1.0, 0.45)); }
        }
    }
}
