//! Draws the table: the baked layers, kit sprites for every moving part, balls, sparks and popups,
//! scrolled to follow the ball.

use crate::game::{hex, Game, State};
use macroquad::prelude::*;

pub const VIEW_W: f32 = 960.0;
pub const VIEW_H: f32 = 540.0;
/// the side panels are this wide; the table gets the middle
pub const PANEL_W: f32 = 172.0;
pub const TABLE_X: f32 = PANEL_W;
pub const TABLE_VIEW_W: f32 = VIEW_W - 2.0 * PANEL_W;
pub const TABLE_CX: f32 = TABLE_X + TABLE_VIEW_W / 2.0;

fn env(g: &Game, key: &str, attack: f64, release: f64) -> f32 {
    match g.hits.get(key) {
        Some(t0) => {
            let a = g.time - t0;
            if a < 0.0 { 0.0 } else if a < attack { (a / attack) as f32 } else { (1.0 - (a - attack) / release).max(0.0) as f32 }
        }
        None => 0.0,
    }
}

/// kit sprite name for a drop target or standup from its a→b direction
pub fn target_sprite(kind: &str, a: [f64; 2], b: [f64; 2], step: f64, lo: f64, hi: f64) -> String {
    let deg = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
    let mut q = (deg / step).round() * step;
    if kind == "stand" { q = q.rem_euclid(360.0); }
    let q = q.clamp(lo, hi) as i32;
    format!("{kind}_{q}")
}

impl Game {
    /// top-left pixel of the table art on the canvas (centred in the table area, scrolled, shaken)
    pub fn origin(&self) -> Vec2 {
        let sx = if self.shake > 0.0 { (fastrand::f32() - 0.5) * self.shake * 4.0 } else { 0.0 };
        let sy = if self.shake > 0.0 { (fastrand::f32() - 0.5) * self.shake * 4.0 } else { 0.0 };
        let x = (TABLE_CX - self.art.proj.w as f32 / 2.0).round();
        vec2(x + sx.round(), -self.scroll.round() + sy.round())
    }

    pub fn update_scroll(&mut self, dt: f32) {
        let h = self.art.proj.h as f32;
        let max = (h - VIEW_H).max(0.0);
        let target = match self.physics.balls.iter().min_by(|a, b| a.y.partial_cmp(&b.y).unwrap()) {
            Some(b) if matches!(self.state, State::Play | State::Ending | State::Title) => {
                // follow the lowest ball, keeping the flippers in view when it is near them
                let p = self.art.proj.to_px(b.x, b.y, b.z);
                (p.y - VIEW_H * 0.58).clamp(0.0, max)
            }
            _ => max,
        };
        let k = (dt * 6.0).min(1.0);
        self.scroll += (target - self.scroll) * k;
        self.scroll = self.scroll.clamp(0.0, max);
    }

    pub fn draw_table(&self, t: f64) {
        let o = self.origin();
        let art = &self.art;
        let kit = &self.kits[art.kit];
        let proj = art.proj;
        let k = (proj.ppm / 1000.0) as f32;
        let at = |x: f64, y: f64, z: f64| proj.to_px(x, y, z) + o;
        let lerp = |p: f64, c: f64| p + (c - p) * self.alpha;
        // the cabinet sides beside a narrow table
        draw_rectangle(TABLE_X, 0.0, TABLE_VIEW_W, VIEW_H, hex(0x0b0a14));
        for (x0, x1) in [(TABLE_X, o.x), (o.x + proj.w as f32, TABLE_X + TABLE_VIEW_W)] {
            if x1 - x0 > 2.0 {
                draw_rectangle(x0, 0.0, x1 - x0, VIEW_H, hex(0x120c2b));
                let mut y = (-self.scroll).rem_euclid(24.0) - 24.0;
                while y < VIEW_H { draw_rectangle(x0, y, x1 - x0, 1.0, hex(0x1e1545)); y += 24.0; }
            }
        }
        draw_texture(&art.base, o.x, o.y, WHITE);

        // playfield print labels and the apron cards
        for lb in &self.layout.labels {
            let p = at(lb.x, lb.y, 0.0);
            let w = self.font.width(lb.text, 1.0);
            self.font.draw_shadow(lb.text, (p.x - w / 2.0).round(), (p.y - 3.0).round(), 1.0, hex(0xeef0f7));
        }
        let card = |x0: f64, lines: &[&str]| {
            let p = at(x0, 0.038, 0.0283);
            for (i, l) in lines.iter().enumerate() { self.font.draw(l, p.x + 4.0 * k, p.y + 3.0 + i as f32 * 9.0 * k.max(0.9), 1.0, hex(0x1b1830)); }
        };
        card(-0.205, &["EACH BALL SCORES", "POINTS × MULT", "BEAT THE TARGET"]);
        card(0.065, &[self.rules.mb_hint(), "2 BIG SHOTS: COMBO", "A D W: NUDGE"]);

        // lit inserts
        for (i, ins) in self.layout.inserts.iter().enumerate() {
            let lv = if matches!(self.state, State::Title | State::Select | State::Shop) {
                (0.5 + 0.5 * ((t * 2.5 + ins.x * 20.0 + ins.y * 13.0).sin())) as f32
            } else { self.light_level(&ins.id, t) };
            if lv > 0.02 {
                if let Some((tx, p)) = art.lit.get(i) { draw_texture(tx, (o.x + p.x).round(), (o.y + p.y).round(), Color::new(1.0, 1.0, 1.0, lv.min(1.0))); }
            }
        }

        // ball shadows on the playfield
        for b in &self.physics.balls {
            let (x, y, z) = (lerp(b.px, b.x), lerp(b.py, b.y), lerp(b.pz, b.z));
            let ground = if z > 0.02 { z - 0.0135 } else { 0.0 };
            let p = at(x + 0.004, y - 0.004, ground);
            draw_circle(p.x, p.y, 11.0 * k, Color::new(0.02, 0.01, 0.06, 0.38));
        }

        // floor-level parts and balls, far to near
        enum D { S(String, u32, Vec2), Ball(usize) }
        let mut list: Vec<(f64, D)> = vec![];
        for (i, f) in self.physics.flippers.iter().enumerate() {
            let a = lerp(f.prev_ang, f.ang);
            let fr = (((a - f.rest) / (f.up - f.rest)) * 11.0).round().clamp(0.0, 11.0) as u32;
            let lf = &self.layout.flippers[i];
            let name = format!("flipper{}_{}", if lf.main { "" } else { "S" }, if lf.side < 0 { "L" } else { "R" });
            list.push((f.y, D::S(name, fr, at(f.x, f.y, 0.0))));
        }
        for (i, b) in self.layout.bumpers.iter().enumerate() {
            let slam = env(self, &format!("bumper:{i}"), 0.012, 0.09);
            let glow = env(self, &format!("bumper:{i}"), 0.005, 0.25);
            let fr = (slam * 3.0).round() as u32 + if glow > 0.3 { 4 } else { 0 };
            list.push((b.y, D::S(format!("bumper_{}", b.color), fr, at(b.x, b.y, 0.0))));
        }
        for (i, s) in self.layout.slings.iter().enumerate() {
            let kick = env(self, &format!("sling:{i}"), 0.01, 0.08);
            let glow = env(self, &format!("sling:{i}"), 0.005, 0.2);
            let fr = (kick * 2.0).round() as u32 + if glow > 0.3 { 3 } else { 0 };
            let side = if s.side < 0 { "L" } else { "R" };
            list.push((s.pts[0][1], D::S(format!("sling_{side}"), fr, at(s.pts[1][0], s.pts[1][1], 0.0))));
        }
        for (i, d) in self.layout.drops.iter().enumerate() {
            let down = self.physics.drops[i].down;
            let kk = self.drop_anim.get(i).copied().unwrap_or(if down { 1.0 } else { 0.0 });
            let name = target_sprite("drop", d.a, d.b, 10.0, -30.0, 30.0);
            list.push((d.a[1].max(d.b[1]), D::S(name, (kk * 5.0).round() as u32, at((d.a[0] + d.b[0]) / 2.0, (d.a[1] + d.b[1]) / 2.0, 0.0))));
        }
        for (i, s) in self.layout.standups.iter().enumerate() {
            let fr = match self.hits.get(&format!("standup:{i}")) {
                Some(t0) if self.time - t0 < 0.8 => {
                    let age = self.time - t0;
                    let ang = (age * 55.0).sin() * (-age * 9.0).exp() * 0.32;
                    ((ang / 0.24) * 2.0 + 2.0).round().clamp(0.0, 4.0) as u32
                }
                _ => 2,
            };
            let name = target_sprite("stand", s.a, s.b, 15.0, 0.0, 345.0);
            list.push((s.a[1].max(s.b[1]), D::S(name, fr, at((s.a[0] + s.b[0]) / 2.0, (s.a[1] + s.b[1]) / 2.0, 0.0))));
        }
        let pl = &self.physics.plunger;
        list.push((0.0, D::S("plunger".into(), ((-pl.pos / pl.travel) * 7.0).round().clamp(0.0, 7.0) as u32, at(pl.x, 0.0, 0.0))));
        for (i, b) in self.physics.balls.iter().enumerate() {
            if b.z < 0.026 { list.push((b.y, D::Ball(i))); }
        }
        list.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let draw_ball = |b: &crate::physics::Ball| {
            let p = at(lerp(b.px, b.x), lerp(b.py, b.y), lerp(b.pz, b.z));
            if !kit.draw("ball", 0, p, WHITE) {
                draw_circle(p.x, p.y, 13.5 * k, hex(0x9a99b3));
                draw_circle(p.x - 4.0 * k, p.y - 4.0 * k, 4.0 * k, hex(0xeef0f7));
            }
        };
        for (_, d) in &list {
            match d {
                D::S(n, f, p) => { kit.draw(n, *f, *p, WHITE); }
                D::Ball(i) => draw_ball(&self.physics.balls[*i]),
            }
        }

        // above the ball: clear ramp, wire-forms, plastics, spinner, gate
        draw_texture(&art.glass, o.x, o.y, Color::new(1.0, 1.0, 1.0, 0.45));
        draw_texture(&art.over, o.x, o.y, WHITE);
        for (i, s) in self.layout.slings.iter().enumerate() {
            let side = if s.side < 0 { "L" } else { "R" };
            let p = at(s.pts[1][0], s.pts[1][1], 0.0);
            kit.draw(&format!("slingplastic_{side}"), 0, p, WHITE);
            let glow = env(self, &format!("sling:{i}"), 0.005, 0.2);
            if glow > 0.02 { kit.draw(&format!("slinglit_{side}"), 0, p, Color::new(1.0, 1.0, 1.0, glow * 0.8)); }
        }
        for sp in &self.physics.spinners {
            let a = sp.angle.rem_euclid(std::f64::consts::TAU);
            kit.draw("spinner", ((a / std::f64::consts::TAU) * 12.0).floor() as u32 % 12, at((sp.a[0] + sp.b[0]) / 2.0, sp.a[1], 0.0), WHITE);
        }
        for (i, g) in self.physics.gates.iter().enumerate() {
            let lg = &self.layout.gates[i];
            kit.draw("gate", ((g.swing / 1.2) * 4.0).round().clamp(0.0, 4.0) as u32, at((lg.a[0] + lg.b[0]) / 2.0, lg.a[1], 0.0), WHITE);
        }
        for b in &self.physics.balls {
            if b.z >= 0.026 { draw_ball(b); }
        }

        // the Fog boss: only a pool of light around the ball
        if self.table.as_ref().map(|t| t.flags.fog).unwrap_or(false) && matches!(self.state, State::Play | State::Ending) {
            let dark = Color::new(0.01, 0.0, 0.03, 0.9);
            let c = self.physics.balls.first().map(|b| at(b.x, b.y, b.z)).unwrap_or(vec2(-999.0, -999.0));
            let r = 70.0;
            let (x0, x1) = (TABLE_X, TABLE_X + TABLE_VIEW_W);
            draw_rectangle(x0, 0.0, x1 - x0, (c.y - r).max(0.0), dark);
            draw_rectangle(x0, c.y + r, x1 - x0, VIEW_H, dark);
            draw_rectangle(x0, c.y - r, (c.x - r - x0).max(0.0), r * 2.0, dark);
            draw_rectangle(c.x + r, c.y - r, (x1 - c.x - r).max(0.0), r * 2.0, dark);
            for kk in 0..6 {
                let rr = r - kk as f32 * 9.0;
                draw_circle_lines(c.x, c.y, rr, 9.0, Color::new(0.01, 0.0, 0.03, 0.9 - kk as f32 * 0.15));
            }
        }

        // sparks
        for s in &self.sparks {
            let p = s.pos + o;
            let a = (s.life * 3.0).min(1.0);
            draw_rectangle(p.x.round(), p.y.round(), 2.0, 2.0, Color::new(s.color.r, s.color.g, s.color.b, a));
        }
        // world popups (points and mult at the shot)
        for p in &self.popups {
            if let Some((x, y)) = p.world {
                let q = at(x, y, 0.04) - vec2(0.0, p.age * 30.0);
                let a = if p.age < 0.8 { 1.0 } else { 1.0 - (p.age - 0.8) / 0.3 };
                let w = self.font.width(&p.text, 1.0);
                self.font.draw_shadow(&p.text, (q.x - w / 2.0).round(), q.y.round(), 1.0, Color::new(p.color.r, p.color.g, p.color.b, a));
            }
        }
    }
}
