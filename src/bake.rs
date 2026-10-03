//! Bakes a generated layout into pixel-art layers at load time: the playfield print, walls and rails,
//! ramps, posts and the apron. A small software rasterizer draws everything in the same palette the
//! Blender kit uses, with 1px dark outlines around solid parts, so the two read as one piece of art.
//! The moving parts (flippers, bumpers, targets, ...) are Blender kit sprites drawn on top at runtime.

use crate::assets::{Kit, Proj};
use crate::font::Font;
use crate::table::*;
use macroquad::prelude::*;

// ------------------------------------------------------------------ palette (matches art/pixel.py)
pub const STEEL: [u32; 8] = [0x0b0a14, 0x1b1830, 0x2e2b47, 0x4a4766, 0x6f6d8c, 0x9a99b3, 0xc7c7d9, 0xeef0f7];
pub const VIOLET: [u32; 7] = [0x120c2b, 0x1e1545, 0x2e2063, 0x42308a, 0x5b45b0, 0x7d68d4, 0xa596ec];
pub const PINK: [u32; 6] = [0x3a0a2a, 0x6b1048, 0xa81c66, 0xe0337f, 0xff6aa8, 0xffb3d2];
pub const CYAN: [u32; 6] = [0x0a2340, 0x0f3f6b, 0x12669a, 0x1c95c8, 0x49c6ec, 0xa3ecff];
pub const GOLD: [u32; 6] = [0x3d1d08, 0x7a3d0c, 0xb8650f, 0xe89a1c, 0xffcc4d, 0xfff1a8];
pub const RED: [u32; 5] = [0x3a0b10, 0x74141f, 0xb8222f, 0xee4040, 0xff8a7a];
pub const GREEN: [u32; 5] = [0x0c2a1c, 0x145233, 0x1f8a4c, 0x3fc46a, 0x8ff09e];
pub const WOOD: [u32; 5] = [0x2a160c, 0x4d2a16, 0x7a4524, 0xa8693a, 0xd39a62];
const OUTLINE: u32 = 0x0b0a14;

/// playfield themes: (floor ramp dark→light, accent ramp, second accent)
pub struct Theme { pub floor: [u32; 5], pub accent: [u32; 5], pub accent2: [u32; 5] }
pub fn theme(i: usize) -> Theme {
    let v = |r: &[u32]| [r[0], r[1], r[2], r[3], r[4]];
    match i {
        0 => Theme { floor: v(&VIOLET), accent: v(&PINK), accent2: v(&CYAN) },
        1 => Theme { floor: v(&CYAN), accent: v(&GOLD), accent2: v(&PINK) },
        2 => Theme { floor: v(&GREEN), accent: v(&GOLD), accent2: v(&CYAN) },
        3 => Theme { floor: [0x1e1545, 0x2e2063, 0x42308a, 0x5b45b0, 0x7d68d4], accent: v(&GOLD), accent2: v(&GREEN) },
        4 => Theme { floor: v(&WOOD), accent: v(&CYAN), accent2: v(&RED) },
        _ => Theme { floor: v(&RED), accent: v(&GOLD), accent2: v(&PINK) },
    }
}

fn rgb(c: u32) -> [u8; 4] { [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255] }
fn scale(c: u32, k: f32) -> u32 {
    let ch = |s: u32| (((c >> s) & 0xff) as f32 * k).min(255.0) as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}
const BAYER: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];
/// ordered dither between two palette colours: t in 0..1
fn dither(x: i32, y: i32, t: f32, a: u32, b: u32) -> u32 {
    if t > (BAYER[(y & 3) as usize][(x & 3) as usize] + 0.5) / 16.0 { b } else { a }
}

// ------------------------------------------------------------------ raster

/// coverage mask over the whole canvas (bounding box tracked for speed)
pub struct Mask { w: i32, h: i32, bits: Vec<bool>, x0: i32, y0: i32, x1: i32, y1: i32 }
impl Mask {
    fn new(w: i32, h: i32) -> Mask { Mask { w, h, bits: vec![false; (w * h) as usize], x0: w, y0: h, x1: -1, y1: -1 } }
    fn clear(&mut self) {
        if self.x1 >= self.x0 {
            for y in self.y0..=self.y1 { for x in self.x0..=self.x1 { self.bits[(y * self.w + x) as usize] = false; } }
        }
        self.x0 = self.w; self.y0 = self.h; self.x1 = -1; self.y1 = -1;
    }
    #[inline]
    fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { return; }
        self.bits[(y * self.w + x) as usize] = true;
        self.x0 = self.x0.min(x); self.y0 = self.y0.min(y); self.x1 = self.x1.max(x); self.y1 = self.y1.max(y);
    }
    #[inline]
    fn get(&self, x: i32, y: i32) -> bool { x >= 0 && y >= 0 && x < self.w && y < self.h && self.bits[(y * self.w + x) as usize] }
    /// even-odd scanline fill, sampling pixel centres
    fn poly(&mut self, pts: &[Vec2]) {
        if pts.len() < 3 { return; }
        let ymin = pts.iter().fold(f32::MAX, |m, p| m.min(p.y)).floor().max(0.0) as i32;
        let ymax = pts.iter().fold(f32::MIN, |m, p| m.max(p.y)).ceil().min(self.h as f32 - 1.0) as i32;
        let mut xs: Vec<f32> = vec![];
        for y in ymin..=ymax {
            let sy = y as f32 + 0.5;
            xs.clear();
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (a.y <= sy && b.y > sy) || (b.y <= sy && a.y > sy) {
                    xs.push(a.x + (sy - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks(2) {
                if pair.len() < 2 { break; }
                let xa = (pair[0] - 0.5).ceil() as i32;
                let xb = (pair[1] - 0.5).floor() as i32;
                for x in xa..=xb { self.set(x, y); }
            }
        }
    }
    fn disc(&mut self, c: Vec2, r: f32) {
        for y in (c.y - r).floor() as i32..=(c.y + r).ceil() as i32 {
            for x in (c.x - r).floor() as i32..=(c.x + r).ceil() as i32 {
                let (dx, dy) = (x as f32 + 0.5 - c.x, y as f32 + 0.5 - c.y);
                if dx * dx + dy * dy <= r * r { self.set(x, y); }
            }
        }
    }
    /// a thick polyline: quads per segment plus round joints
    fn stroke(&mut self, pts: &[Vec2], width: f32) {
        let r = width / 2.0;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let len = d.length();
            if len < 1e-4 { continue; }
            let n = vec2(-d.y, d.x) / len * r;
            self.poly(&[a + n, b + n, b - n, a - n]);
        }
        if r >= 1.0 { for p in pts { self.disc(*p, r); } }
    }
}

pub struct Canvas { pub w: i32, pub h: i32, px: Vec<[u8; 4]>, m: Mask, ring: Mask }
impl Canvas {
    pub fn new(w: i32, h: i32) -> Canvas { Canvas { w, h, px: vec![[0; 4]; (w * h) as usize], m: Mask::new(w, h), ring: Mask::new(w, h) } }
    #[inline]
    fn put(&mut self, x: i32, y: i32, c: u32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { return; }
        self.px[(y * self.w + x) as usize] = rgb(c);
    }
    /// fill whatever `draw` puts in the mask, colouring each pixel with `shade`; optional 1px outer outline
    pub fn shape(&mut self, draw: impl FnOnce(&mut Mask), outline: Option<u32>, shade: impl Fn(i32, i32) -> u32) {
        self.m.clear();
        draw(&mut self.m);
        if self.m.x1 < self.m.x0 { return; }
        if let Some(oc) = outline {
            for y in self.m.y0 - 1..=self.m.y1 + 1 {
                for x in self.m.x0 - 1..=self.m.x1 + 1 {
                    if !self.m.get(x, y) && (self.m.get(x - 1, y) || self.m.get(x + 1, y) || self.m.get(x, y - 1) || self.m.get(x, y + 1)) {
                        self.put(x, y, oc);
                    }
                }
            }
        }
        for y in self.m.y0..=self.m.y1 {
            for x in self.m.x0..=self.m.x1 {
                if self.m.get(x, y) { let c = shade(x, y); self.put(x, y, c); }
            }
        }
    }
    /// like `shape` but only where the clip mask is set (the playfield print stays inside the cabinet)
    pub fn shape_clipped(&mut self, clip: &Mask, draw: impl FnOnce(&mut Mask), shade: impl Fn(i32, i32) -> u32) {
        self.m.clear();
        draw(&mut self.m);
        if self.m.x1 < self.m.x0 { return; }
        for y in self.m.y0..=self.m.y1 {
            for x in self.m.x0..=self.m.x1 {
                if self.m.get(x, y) && clip.get(x, y) { let c = shade(x, y); self.put(x, y, c); }
            }
        }
    }
    /// draw a ring of the outline colour around everything `draw` covers, without filling it
    pub fn outline_only(&mut self, draw: impl FnOnce(&mut Mask), oc: u32) {
        self.ring.clear();
        draw(&mut self.ring);
        if self.ring.x1 < self.ring.x0 { return; }
        for y in self.ring.y0 - 1..=self.ring.y1 + 1 {
            for x in self.ring.x0 - 1..=self.ring.x1 + 1 {
                if !self.ring.get(x, y) && (self.ring.get(x - 1, y) || self.ring.get(x + 1, y) || self.ring.get(x, y - 1) || self.ring.get(x, y + 1)) {
                    self.put(x, y, oc);
                }
            }
        }
    }
    /// alpha-tested blit of one frame of a kit sprite; `at` is the pixel the sprite's origin lands on
    pub fn stamp(&mut self, kit: &Kit, name: &str, frame: u32, at: Vec2) {
        let Some(s) = kit.sprites.get(name) else { return };
        let f = frame.min(s.frames - 1);
        let (ox, oy) = ((at.x + s.ox).round() as i32, (at.y + s.oy).round() as i32);
        for y in 0..s.h as i32 {
            for x in 0..s.w as i32 {
                let c = s.img.get_pixel(f * s.w + x as u32, y as u32);
                if c.a > 0.5 {
                    let (px, py) = (ox + x, oy + y);
                    if px >= 0 && py >= 0 && px < self.w && py < self.h {
                        self.px[(py * self.w + px) as usize] = [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, 255];
                    }
                }
            }
        }
    }
    pub fn image(&self) -> Image {
        let mut bytes = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px { bytes.extend_from_slice(p); }
        Image { bytes, width: self.w as u16, height: self.h as u16 }
    }
}

// ------------------------------------------------------------------ baking

pub struct Baked { pub base: Image, pub over: Image, pub glass: Image, pub lit: Vec<(Image, Vec2)> }

pub fn proj_for(l: &Layout) -> Proj {
    let phi = 20f64.to_radians();
    let span = l.x_max() - l.x_min() + 0.02;
    // wide machines are drawn a little smaller so they still fit beside the panels
    let ppm = if span * 1000.0 <= crate::draw::TABLE_VIEW_W as f64 { 1000.0 } else { 800.0 };
    let x0 = l.x_min() - 0.01;
    let sy1 = (l.top + 0.015) * phi.cos() + 0.075 * phi.sin();
    let sy0 = -0.02 * phi.cos();
    Proj { x0, sy1, ppm, phi, w: (span * ppm).ceil() as u32, h: ((sy1 - sy0) * ppm).ceil() as u32 }
}

pub fn bake(l: &Layout, kit: &Kit, font: &Font) -> Baked {
    let proj = proj_for(l);
    let (w, h) = (proj.w as i32, proj.h as i32);
    let k = (proj.ppm / 1000.0) as f32;
    let p = |x: f64, y: f64, z: f64| proj.to_px(x, y, z);
    let th = theme(l.theme);
    let mut rng = crate::tablegen::Rng::new(l.seed ^ 0xa27);
    let mut base = Canvas::new(w, h);
    let mut over = Canvas::new(w, h);
    let mut glass = Canvas::new(w, h);

    // ---- the playfield: everything inside the cabinet outline
    let mut floor = Mask::new(w, h);
    let outline_px: Vec<Vec2> = l.outline.iter().map(|q| p(q[0], q[1], 0.0)).collect();
    let mut closed = outline_px.clone();
    closed.push(p(l.x_max(), -0.01, 0.0));
    floor.poly(&closed);
    // base wash with a soft dithered falloff toward the top and the walls
    let (fl, fd, fm) = (th.floor[2], th.floor[1], th.floor[3]);
    let cy = p(0.0, 0.55, 0.0).y;
    base.shape(|m| m.poly(&closed), None, |x, y| {
        let t = ((y as f32 - cy).abs() / (h as f32 * 0.55)).min(1.0);
        dither(x, y, t * 0.9, fl, fd)
    });

    // sunburst behind the bumpers
    if !l.bumpers.is_empty() {
        let (bx, by) = l.bumpers.iter().fold((0.0, 0.0), |a, b| (a.0 + b.x, a.1 + b.y));
        let n = l.bumpers.len() as f64;
        let (cx, cyw) = (bx / n, by / n);
        let rays = 28;
        for i in (0..rays).step_by(2) {
            let a0 = std::f64::consts::TAU * i as f64 / rays as f64;
            let a1 = a0 + std::f64::consts::TAU / rays as f64 * 0.55;
            let pts = vec![p(cx, cyw, 0.0), p(cx + 0.32 * a0.cos(), cyw + 0.32 * a0.sin(), 0.0), p(cx + 0.32 * a1.cos(), cyw + 0.32 * a1.sin(), 0.0)];
            base.shape_clipped(&floor, |m| m.poly(&pts), |x, y| {
                let d = ((x as f32 - pts[0].x).powi(2) + (y as f32 - pts[0].y).powi(2)).sqrt() / (300.0 * k);
                dither(x, y, d, fd, fl)
            });
        }
        // a lighter halo right under the cluster
        let c = p(cx, cyw, 0.0);
        base.shape_clipped(&floor, |m| m.disc(c, 95.0 * k), |x, y| {
            let d = ((x as f32 - c.x).powi(2) + (y as f32 - c.y).powi(2)).sqrt() / (95.0 * k);
            dither(x, y, 1.0 - d, fl, fm)
        });
    }

    // lane bands: outlanes with hazard stripes, the shooter lane, orbit lanes
    let band = |c: &mut Canvas, x0: f64, x1: f64, y0: f64, y1: f64, col: u32| {
        let pts = vec![p(x0, y0, 0.0), p(x1, y0, 0.0), p(x1, y1, 0.0), p(x0, y1, 0.0)];
        c.shape_clipped(&floor, |m| m.poly(&pts), |_, _| col);
    };
    for s in [-1.0, 1.0] {
        let (x0, x1) = if s < 0.0 { (-0.235, -0.201) } else { (0.201, 0.235) };
        band(&mut base, x0, x1, 0.04, 0.272, RED[1]);
        for kk in 0..12 {
            let y0 = 0.05 + kk as f64 * 0.018;
            let pts = if s < 0.0 {
                vec![p(x0, y0, 0.0), p(x1, y0 + 0.012, 0.0), p(x1, y0 + 0.019, 0.0), p(x0, y0 + 0.007, 0.0)]
            } else {
                vec![p(x0, y0 + 0.012, 0.0), p(x1, y0, 0.0), p(x1, y0 + 0.007, 0.0), p(x0, y0 + 0.019, 0.0)]
            };
            base.shape_clipped(&floor, |m| m.poly(&pts), |_, _| STEEL[6]);
        }
    }
    band(&mut base, l.hw + 0.004, l.x_max() - 0.004, -0.01, l.arch_y - 0.01, th.floor[0]);
    for (i, sn) in l.sensors.iter().enumerate() {
        if let SensorKind::OrbitEntry(_) = sn.kind {
            let s = sn.x.signum();
            let (xa, xb) = if s < 0.0 { (-l.hw + 0.004, -l.hw + 0.057) } else { (l.hw - 0.057, l.hw - 0.004) };
            band(&mut base, xa, xb, sn.y - 0.05, sn.y + 0.2, th.accent2[1]);
            let _ = i;
        }
    }

    // dot grid in open floor
    let near_any = |x: f64, y: f64, r: f64| {
        l.bumpers.iter().any(|b| (b.x - x).hypot(b.y - y) < b.r + r)
            || l.posts.iter().any(|q| (q.x - x).hypot(q.y - y) < r + 0.01)
            || l.drops.iter().chain(&l.standups).any(|t| ((t.a[0] + t.b[0]) / 2.0 - x).hypot((t.a[1] + t.b[1]) / 2.0 - y) < r + 0.025)
            || l.inserts.iter().any(|q| (q.x - x).hypot(q.y - y) < q.r + r)
            || l.walls.iter().any(|wl| wl.pts.iter().any(|q| (q[0] - x).hypot(q[1] - y) < r))
    };
    let mut gy = 0.36;
    let mut row = 0;
    while gy < l.arch_y {
        let mut gx = -l.hw + 0.03 + if row % 2 == 1 { 0.0112 } else { 0.0 };
        while gx < l.hw - 0.03 {
            if !near_any(gx, gy, 0.03) && rng.chance(0.85) {
                let c = p(gx, gy, 0.0);
                base.shape_clipped(&floor, |m| m.disc(c, 2.2 * k), |_, _| th.floor[3]);
            }
            gx += 0.0225;
        }
        gy += 0.02;
        row += 1;
    }

    // big card-suit prints in the emptiest spots
    let suits = ['♠', '♥', '♦', '♣'];
    let mut placed = 0;
    for _ in 0..200 {
        if placed >= 3 { break; }
        let (x, y) = (rng.range(-l.hw + 0.05, l.hw - 0.05), rng.range(0.45, l.arch_y));
        if near_any(x, y, 0.05) { continue; }
        let c = p(x, y, 0.0);
        let glyph = font.bits(suits[placed % 4]).clone();
        let px_size = (5.0 * k).round().max(3.0) as i32;
        let gw = glyph.first().map(|r| r.len()).unwrap_or(5) as i32;
        let (ox, oy) = (c.x as i32 - gw * px_size / 2, c.y as i32 - 7 * px_size / 2);
        let col = if placed % 2 == 0 { th.floor[3] } else { th.accent[1] };
        base.shape_clipped(&floor, |m| {
            for (j, r) in glyph.iter().enumerate() {
                for (i, on) in r.iter().enumerate() {
                    if *on { for yy in 0..px_size { for xx in 0..px_size { m.set(ox + i as i32 * px_size + xx, oy + j as i32 * px_size + yy); } } }
                }
            }
        }, |_, _| col);
        placed += 1;
    }

    // painted arrows under the shot inserts
    for ins in &l.inserts {
        if ins.shape != "arrow" { continue; }
        let (x, y) = (ins.x, ins.y + 0.0);
        let (l_, w_) = (0.05, 0.021);
        let pts: Vec<Vec2> = [(-0.45, -0.4), (0.45, -0.4), (0.45, 0.35), (1.0, 0.35), (0.0, 1.0), (-1.0, 0.35), (-0.45, 0.35)]
            .iter().map(|(a, b)| p(x + w_ * a, y - 0.01 + l_ * b, 0.0)).collect();
        let col = if ins.id.starts_with("ramp") { th.accent[1] } else { th.accent2[1] };
        base.shape_clipped(&floor, |m| m.poly(&pts), |_, _| col);
    }

    // closed-off areas: solid cabinet with a light hatch
    for poly in &l.blocked {
        let pts: Vec<Vec2> = poly.iter().map(|q| p(q[0], q[1], 0.0)).collect();
        base.shape(|m| m.poly(&pts), Some(OUTLINE), |x, y| if (x + y) % 6 == 0 { STEEL[2] } else { STEEL[1] });
    }

    // insert sockets (white ring, dark rim) and the unlit insert itself; lit versions become sprites
    let mut lit = vec![];
    for ins in &l.inserts {
        let c = p(ins.x, ins.y, 0.0);
        let r = (ins.r * proj.ppm) as f32;
        let poly = insert_poly(ins, &p);
        if ins.shape == "circle" {
            base.shape(|m| m.disc(c, r + 3.5 * k), None, |_, _| STEEL[6]);
            base.shape(|m| m.disc(c, r + 2.2 * k), None, |_, _| VIOLET[0]);
        }
        let off = scale(ins.color, 0.32);
        base.shape(|m| m.poly(&poly), None, |_, _| off);
        // lit sprite: bright core with a highlight
        let (minx, miny) = poly.iter().fold((f32::MAX, f32::MAX), |a, q| (a.0.min(q.x), a.1.min(q.y)));
        let (maxx, maxy) = poly.iter().fold((f32::MIN, f32::MIN), |a, q| (a.0.max(q.x), a.1.max(q.y)));
        let (ox, oy) = (minx.floor() as i32 - 1, miny.floor() as i32 - 1);
        let (sw, sh) = ((maxx.ceil() as i32 - ox + 2).max(1), (maxy.ceil() as i32 - oy + 2).max(1));
        let mut cv = Canvas::new(sw, sh);
        let local: Vec<Vec2> = poly.iter().map(|q| *q - vec2(ox as f32, oy as f32)).collect();
        let hi = scale(ins.color, 1.25) | 0x202020;
        let cl = c - vec2(ox as f32, oy as f32);
        cv.shape(|m| m.poly(&local), None, |x, y| {
            let d = ((x as f32 + 0.5 - cl.x + 1.0).powi(2) + (y as f32 + 0.5 - cl.y + 1.0).powi(2)).sqrt();
            if d < r * 0.45 { hi } else { ins.color }
        });
        lit.push((cv.image(), vec2(ox as f32, oy as f32)));
    }

    // rollover wires and their slots
    for ro in &l.rollovers {
        let a = p(ro.x, ro.y - 0.012, 0.0);
        let b = p(ro.x, ro.y + 0.012, 0.0);
        base.shape(|m| m.stroke(&[a, b], 3.0 * k), None, |_, _| OUTLINE);
        if kit.sprites.contains_key("rollover") {
            base.stamp(kit, "rollover", 0, p(ro.x, ro.y, 0.0));
        } else {
            base.shape(|m| m.stroke(&[a, b], 1.5), None, |_, _| STEEL[6]);
        }
    }

    // ---- walls: steel guides, the cabinet, solid ramp ends; far ones first
    let mut walls: Vec<&Wall> = l.walls.iter().filter(|w| w.vis == "rail" || w.vis == "outer").collect();
    walls.sort_by(|a, b| {
        let ya = a.pts.iter().fold(f64::MIN, |m, q| m.max(q[1]));
        let yb = b.pts.iter().fold(f64::MIN, |m, q| m.max(q[1]));
        yb.partial_cmp(&ya).unwrap()
    });
    for wl in walls { draw_wall(&mut base, wl, &p, k); }
    // the solid descending ends of ramps sit on dark skirts
    for rp in &l.ramps {
        for side in [&rp.left, &rp.right] {
            let pts: Vec<[f64; 4]> = side[rp.solid_from.saturating_sub(1)..].iter().map(|q| [q[0], q[1], 0.0, (q[3] - 0.028).max(0.004)]).collect();
            if pts.len() >= 2 {
                draw_wall(&mut base, &Wall { pts, r: 0.0015, mat: "metal", vis: "skirt" }, &p, k);
            }
        }
    }

    // posts, far to near
    let mut posts: Vec<&Post> = l.posts.iter().collect();
    posts.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap());
    for q in posts {
        let name = if q.mat == "rubber" { "post_rubber" } else if q.r > 0.005 { "post_metal_l" } else { "post_metal" };
        let at = p(q.x, q.y, 0.0);
        if kit.sprites.contains_key(name) {
            base.stamp(kit, name, 0, at);
        } else {
            let top = p(q.x, q.y, q.zt);
            let r = (q.r * proj.ppm) as f32;
            base.shape(|m| { m.disc(at, r); m.disc(top, r); m.poly(&[at - vec2(r, 0.0), at + vec2(r, 0.0), top + vec2(r, 0.0), top - vec2(r, 0.0)]); },
                Some(OUTLINE), |_, y| if (y as f32) < top.y + 1.0 { STEEL[7] } else if q.mat == "rubber" { STEEL[1] } else { STEEL[5] });
        }
    }

    // ---- apron across the bottom
    let ap = &l.apron;
    let apron: Vec<Vec2> = [(ap.x0, ap.y0), (ap.x1, ap.y0), (ap.x1, ap.y1 - 0.014), (ap.x1 - 0.035, ap.y1), (ap.x0 + 0.035, ap.y1), (ap.x0, ap.y1 - 0.014)]
        .iter().map(|(x, y)| p(*x, *y, 0.028)).collect();
    let apron_front: Vec<Vec2> = vec![p(ap.x0, ap.y0, 0.028), p(ap.x1, ap.y0, 0.028), p(ap.x1, ap.y0, 0.0), p(ap.x0, ap.y0, 0.0)];
    base.shape(|m| { m.poly(&apron); m.poly(&apron_front); }, Some(OUTLINE), |x, y| dither(x, y, 0.35, STEEL[2], STEEL[3]));
    let lip: Vec<Vec2> = [(ap.x0, ap.y1 - 0.014), (ap.x0 + 0.035, ap.y1), (ap.x1 - 0.035, ap.y1), (ap.x1, ap.y1 - 0.014)]
        .iter().map(|(x, y)| p(*x, *y, 0.028)).collect();
    base.shape(|m| m.stroke(&lip, 3.0 * k), None, |_, _| STEEL[6]);
    for (x0, x1) in [(-0.205, -0.065), (0.065, 0.205)] {
        let card = vec![p(x0, 0.004, 0.0283), p(x1, 0.004, 0.0283), p(x1, 0.042, 0.0283), p(x0, 0.042, 0.0283)];
        base.shape(|m| m.poly(&card), Some(OUTLINE), |_, _| STEEL[7]);
    }
    for x in [-0.222, -0.05, 0.05, 0.222] {
        let c = p(x, 0.02, 0.028);
        base.shape(|m| m.disc(c, 2.0 * k), Some(STEEL[1]), |_, _| STEEL[6]);
    }

    // ---- ramps: clear plastic up-ramp (glass), steel lips and the wire-form return (over)
    for (ri, rp) in l.ramps.iter().enumerate() {
        let n = rp.split + 1;
        let edge = |i: usize, lat: f64, dz: f64| -> Vec2 {
            let q = rp.path[i];
            let a = rp.path[i.saturating_sub(1)];
            let b = rp.path[(i + 1).min(rp.path.len() - 1)];
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let tl = (tx * tx + ty * ty).sqrt().max(1e-9);
            p(q[0] - ty / tl * lat, q[1] + tx / tl * lat, q[2] + dz)
        };
        let hw = rp.half_w + 0.006;
        // floor
        for i in 0..rp.split {
            let quad = vec![edge(i, hw, 0.0), edge(i + 1, hw, 0.0), edge(i + 1, -hw, 0.0), edge(i, -hw, 0.0)];
            glass.shape(|m| m.poly(&quad), None, |x, y| dither(x, y, 0.5 + 0.3 * ((i % 4) as f32 / 4.0), CYAN[3], CYAN[4]));
        }
        // side walls of the up-ramp
        for side in [&rp.left, &rp.right] {
            for i in 0..n.min(side.len()) - 1 {
                let (a, b) = (side[i], side[i + 1]);
                let quad = vec![p(a[0], a[1], a[2]), p(b[0], b[1], b[2]), p(b[0], b[1], b[3]), p(a[0], a[1], a[3])];
                glass.shape(|m| m.poly(&quad), None, |_, _| CYAN[5]);
            }
        }
        // entrance flap on the playfield
        let q0 = rp.path[0];
        let flap = vec![p(q0[0] - hw, q0[1] - 0.011, 0.0), p(q0[0] + hw, q0[1] - 0.011, 0.0), p(q0[0] + hw, q0[1] + 0.001, 0.0), p(q0[0] - hw, q0[1] + 0.001, 0.0)];
        base.shape(|m| m.poly(&flap), Some(OUTLINE), |_, y| if y % 2 == 0 { STEEL[6] } else { STEEL[5] });
        // sticker chevrons on the up-ramp
        for i in (3..rp.split.saturating_sub(1)).step_by(3) {
            let q = rp.path[i];
            let (a, b) = (rp.path[i - 1], rp.path[i + 1]);
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let tl = (tx * tx + ty * ty).sqrt().max(1e-9);
            let (tx, ty) = (tx / tl, ty / tl);
            let (nx, ny) = (-ty, tx);
            let chev: Vec<Vec2> = [(0.0, 0.006), (0.012, -0.004), (0.012, -0.0005), (0.0, 0.0095), (-0.012, -0.0005), (-0.012, -0.004)]
                .iter().map(|(cx, cy)| p(q[0] + nx * cx + tx * cy, q[1] + ny * cx + ty * cy, q[2] + 0.0006)).collect();
            over.shape(|m| m.poly(&chev), None, |_, _| PINK[4]);
        }
        // steel lips along the top of the up-ramp walls
        for side in [&rp.left, &rp.right] {
            let lip: Vec<Vec2> = side[..n.min(side.len())].iter().map(|q| p(q[0], q[1], q[3])).collect();
            over.shape(|m| m.stroke(&lip, 3.0 * k), Some(OUTLINE), |_, _| STEEL[6]);
        }
        // the wire-form: rails under the ball, side rails, top rails and hoops
        let idx: Vec<usize> = (rp.split - 1..rp.solid_from.min(rp.path.len())).collect();
        let lift = BALL_R - (BALL_R * BALL_R - 0.0095f64.powi(2)).sqrt() - 0.0012;
        let wires: [(f64, f64); 6] = [(0.0095, lift), (-0.0095, lift), (rp.half_w + 0.003, 0.012), (-(rp.half_w + 0.003), 0.012), (rp.half_w + 0.003, 0.026), (-(rp.half_w + 0.003), 0.026)];
        for (lat, dz) in wires {
            let pts: Vec<Vec2> = idx.iter().map(|&i| edge(i, lat, dz)).collect();
            over.outline_only(|m| m.stroke(&pts, 2.0 * k), OUTLINE);
        }
        for (lat, dz) in wires {
            let pts: Vec<Vec2> = idx.iter().map(|&i| edge(i, lat, dz)).collect();
            over.shape(|m| m.stroke(&pts, 1.6 * k), None, |_, _| STEEL[6]);
        }
        for &i in idx.iter().skip(1).step_by(3) {
            let hoop: Vec<Vec2> = (0..=10).map(|j| {
                let a = std::f64::consts::PI * j as f64 / 10.0;
                edge(i, (rp.half_w + 0.003) * a.cos(), 0.012 - 0.012 * a.sin())
            }).collect();
            over.shape(|m| m.stroke(&hoop, 1.3 * k), None, |_, _| STEEL[5]);
        }
        let _ = ri;
    }

    // plastic cover over the top lane guides, with lane numbers
    for (idx, _) in &l.features.lanes {
        if idx.is_empty() { continue; }
        let xs: Vec<f64> = idx.iter().map(|&i| l.rollovers[i].x).collect();
        let y = l.rollovers[idx[0]].y + 0.028;
        let (x0, x1) = (xs[0] - 0.037, xs[xs.len() - 1] + 0.037);
        let quad = vec![p(x0, y, 0.0445), p(x1, y, 0.0445), p(x1, y + 0.026, 0.0445), p(x0, y + 0.026, 0.0445)];
        over.shape(|m| m.poly(&quad), Some(OUTLINE), |x, y| dither(x, y, 0.3, th.accent[3], th.accent[4]));
        for (n, &x) in xs.iter().enumerate() {
            let c = p(x, y + 0.013, 0.0447);
            let digit = char::from_digit((n + 1) as u32 % 10, 10).unwrap();
            let g = font.bits(digit).clone();
            let gw = g.first().map(|r| r.len()).unwrap_or(5) as i32;
            let (ox, oy) = (c.x as i32 - gw / 2, c.y as i32 - 3);
            over.shape(|m| {
                for (j, r) in g.iter().enumerate() { for (i, on) in r.iter().enumerate() { if *on { m.set(ox + i as i32, oy + j as i32); } } }
            }, None, |_, _| th.accent[0]);
        }
    }

    Baked { base: base.image(), over: over.image(), glass: glass.image(), lit }
}

fn insert_poly(ins: &Insert, p: &impl Fn(f64, f64, f64) -> Vec2) -> Vec<Vec2> {
    let r = ins.r;
    let loc: Vec<(f64, f64)> = match ins.shape {
        "arrow" => vec![(r, 0.0), (-r * 0.6, r * 0.75), (-r * 0.2, 0.0), (-r * 0.6, -r * 0.75)],
        "chevron" => vec![(r * 0.5, 0.0), (-r * 0.5, r), (-r * 0.9, r), (0.1 * r, 0.0), (-r * 0.9, -r), (-r * 0.5, -r)],
        _ => (0..20).map(|i| { let a = std::f64::consts::TAU * i as f64 / 20.0; (a.cos() * r, a.sin() * r) }).collect(),
    };
    let (c, s) = (ins.rot.to_radians().cos(), ins.rot.to_radians().sin());
    loc.iter().map(|(x, y)| p(ins.x + x * c - y * s, ins.y + x * s + y * c, 0.0006)).collect()
}

/// a wall as a solid slab: the face toward the player (shaded by its angle) and a lit top cap
fn draw_wall(cv: &mut Canvas, wl: &Wall, p: &impl Fn(f64, f64, f64) -> Vec2, k: f32) {
    let r = wl.r;
    let outer = wl.vis == "outer";
    let skirt = wl.vis == "skirt";
    let mut faces: Vec<(Vec<Vec2>, f64)> = vec![];
    let mut caps: Vec<Vec2> = vec![];
    for seg in wl.pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 { continue; }
        // the side facing the player (normal pointing down the table)
        let (mut nx, mut ny) = (-dy / len, dx / len);
        if ny > 0.0 { nx = -nx; ny = -ny; }
        let quad = vec![
            p(a[0] + nx * r, a[1] + ny * r, a[2]), p(b[0] + nx * r, b[1] + ny * r, b[2]),
            p(b[0] + nx * r, b[1] + ny * r, b[3]), p(a[0] + nx * r, a[1] + ny * r, a[3]),
        ];
        // light from the upper left: faces turned toward it are brighter
        let lit = (-nx * 0.5 - ny * 0.3 + 0.35).clamp(0.0, 1.0);
        faces.push((quad, lit));
        if caps.is_empty() { caps.push(p(a[0], a[1], a[3])); }
        caps.push(p(b[0], b[1], b[3]));
    }
    let cap_w = ((r * 2.0 + if outer { 0.003 } else { 0.0 }) * 1000.0) as f32 * k;
    cv.outline_only(|m| { for (q, _) in &faces { m.poly(q); } m.stroke(&caps, cap_w.max(1.0)); }, OUTLINE);
    for (q, lit) in &faces {
        let (lo, hi) = if outer { (VIOLET[0], VIOLET[1]) } else if skirt { (STEEL[1], STEEL[2]) } else { (STEEL[3], STEEL[5]) };
        let t = *lit as f32;
        cv.shape(|m| m.poly(q), None, |x, y| dither(x, y, t, lo, hi));
    }
    if !skirt {
        cv.shape(|m| m.stroke(&caps, cap_w.max(1.0)), None, |_, _| STEEL[6]);
        // a one-pixel highlight along the cap
        let hl: Vec<Vec2> = caps.iter().map(|q| *q - vec2(0.0, (cap_w * 0.25).floor())).collect();
        cv.shape(|m| m.stroke(&hl, 1.0), None, |_, _| STEEL[7]);
    }
}
