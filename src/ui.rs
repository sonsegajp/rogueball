//! Arcade-style UI: a backbox panel with a dot-matrix display, an item tray, game-styled screens.
//! Immediate-mode, drawn in canvas pixels with the skin kit.

use crate::content::*;
use crate::draw::{PANEL_W, TABLE_CX, TABLE_VIEW_W, TABLE_X, VIEW_H, VIEW_W};
use crate::game::{fmt, hex, num, Game, OfferKind, State};
use crate::run::*;
use crate::skin::*;
use macroquad::prelude::*;

const RIGHT_X: f32 = VIEW_W - PANEL_W;

pub const TEXT: u32 = 0xeef0f7;
pub const DIM: u32 = 0x9a99b3;
pub const PTS: u32 = 0x49c6ec;
pub const MULT: u32 = 0xee4040;
pub const GOLD: u32 = 0xffcc4d;
pub const PINK: u32 = 0xff6aa8;

#[derive(Default, Clone, Copy)]
pub struct Mouse { pub pos: Vec2, pub click: bool, pub rclick: bool, pub down: bool, pub released: bool, pub wheel: f32 }

pub fn rarity_color(r: Rarity) -> u32 { match r { Rarity::Common => 0x49c6ec, Rarity::Uncommon => 0x3fc46a, Rarity::Rare => 0xff6aa8 } }
fn rarity_plate(r: Rarity) -> (u32, u32, u32) {
    match r { Rarity::Common => (0x12669a, 0x1c95c8, 0x0f3f6b), Rarity::Uncommon => (0x1f8a4c, 0x3fc46a, 0x145233), Rarity::Rare => (0xa81c66, 0xe0337f, 0x6b1048) }
}

impl Game {
    // ------------------------------------------------------------------ text
    /// rich text with [p] [m] [x] [c] [k] colour tags, wrapped; returns height used
    pub fn rich(&self, s: &str, x: f32, y: f32, w: f32, scale: f32) -> f32 {
        let mut words: Vec<(String, u32)> = vec![];
        let mut col = TEXT;
        let mut cur = String::new();
        let flush = |cur: &mut String, words: &mut Vec<(String, u32)>, col: u32| {
            for (k, part) in cur.split(' ').enumerate() {
                if k > 0 { words.push((" ".into(), col)); }
                if !part.is_empty() { words.push((part.to_string(), col)); }
            }
            cur.clear();
        };
        let mut i = 0;
        while i < s.len() {
            if s[i..].starts_with('[') {
                if let Some(end) = s[i..].find(']') {
                    let ncol = match &s[i + 1..i + end] { "p" => Some(PTS), "m" => Some(PINK), "x" => Some(MULT), "c" => Some(GOLD), "k" => Some(0xa3ecff), "/" => Some(TEXT), _ => None };
                    if let Some(c) = ncol { flush(&mut cur, &mut words, col); col = c; i += end + 1; continue; }
                }
            }
            let ch = s[i..].chars().next().unwrap();
            cur.push(ch);
            i += ch.len_utf8();
        }
        flush(&mut cur, &mut words, col);
        let (mut cx, mut cy) = (x, y);
        let lh = 9.0 * scale;
        for (wd, c) in words {
            if wd == " " { cx += 4.0 * scale; continue; }
            let ww = self.font.width(&wd, scale);
            if cx + ww > x + w && cx > x { cx = x; cy += lh; }
            self.font.draw_shadow(&wd, cx, cy, scale, hex(c));
            cx += ww;
        }
        cy - y + lh
    }

    /// kind: 0 violet, 1 pink (primary), 2 green, 3 gold
    fn button(&mut self, m: &Mouse, r: Rect, label: &str, kind: u8, enabled: bool) -> bool {
        // the gamepad's A presses the main (pink) button of whatever screen is up
        let pad = self.pad_confirm && kind == 1 && enabled;
        let hover = r.contains(m.pos) && enabled;
        self.skin.button_face(&self.font, r, label, kind, hover || (kind == 1 && enabled && self.pad.now.connected), m.down, enabled);
        if (hover && m.click) || pad { self.pad_confirm = false; self.sfx.play("ui"); return true; }
        false
    }

    // ------------------------------------------------------------------ art
    pub fn draw_art(&self, id: &str, name: &str, x: f32, y: f32, size: f32, accent: u32) {
        if let Some(t) = self.card_art.get(id) {
            let k = (size / t.width()).floor().max(1.0);
            let d = t.width() * k;
            draw_texture_ex(t, (x + (size - d) / 2.0).floor(), (y + (size - d) / 2.0).floor(), WHITE, DrawTextureParams { dest_size: Some(vec2(d, d)), ..Default::default() });
            return;
        }
        let initials: String = name.split(' ').filter_map(|w| w.chars().next()).take(2).collect();
        self.font.draw_outlined_centered(&initials, x + size / 2.0, y + size / 2.0 - 7.0, 2.0, hex(accent));
    }

    /// an item as a little framed card: rarity-coloured frame, felt window, art, edition glint
    fn item_card(&self, it: &ItemInst, x: f32, y: f32, w: f32, h: f32, hl: bool) {
        let d = &self.db.items[it.def];
        let (base, hi, lo) = rarity_plate(d.rarity);
        let pulse = self.hits.get(&format!("item:{}", it.uid)).map(|t| (1.0 - (self.time - t) / 0.35).max(0.0) as f32).unwrap_or(0.0);
        let lift = if hl { 2.0 } else { 0.0 } + pulse * 5.0;
        let r = Rect::new(x, y - lift, w, h);
        draw_rectangle(x + 2.0, y + 3.0, w, h, Color::new(0.0, 0.0, 0.0, 0.4));
        self.skin.plate(r, base, hi, lo);
        let win = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0);
        draw_rectangle(win.x, win.y, win.w, win.h, hex(FELT_D));
        draw_rectangle(win.x, win.y, win.w, 1.0, hex(OUTLINE));
        self.draw_art(d.id, d.name, win.x, win.y, win.w, rarity_color(d.rarity));
        if let Some(ed) = it.edition {
            let c = match ed { Edition::Foil => 0xa3ecff, Edition::Holo => 0xff6aa8, Edition::Poly => 0xffcc4d };
            let t = ((get_time() * 0.7 + it.uid as f64 * 0.37) % 1.6) as f32;
            let gx = win.x + (t - 0.3) * win.w;
            for k in 0..3 {
                let bx = gx + k as f32 * 3.0;
                if bx > win.x && bx < win.x + win.w - 2.0 { draw_rectangle(bx, win.y + 1.0, 2.0, win.h - 2.0, Color::new(hex(c).r, hex(c).g, hex(c).b, 0.35)); }
            }
            self.skin.icon("gem", r.x + r.w - 8.0, r.y + 1.0, 1.0, hex(c));
        }
        if pulse > 0.0 { draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, pulse * 0.4)); }
    }

    fn card_thumb(&self, def: usize, x: f32, y: f32, w: f32, h: f32, key: Option<usize>, hl: bool) {
        let d = &self.db.cards[def];
        let (base, hi, lo) = if d.chip { (0x12669a, 0x49c6ec, 0x0f3f6b) } else { (0x42308a, 0x7d68d4, 0x2e2063) };
        let lift = if hl { 2.0 } else { 0.0 };
        let r = Rect::new(x, y - lift, w, h);
        draw_rectangle(x + 2.0, y + 3.0, w, h, Color::new(0.0, 0.0, 0.0, 0.4));
        self.skin.plate(r, base, hi, lo);
        draw_rectangle(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0, hex(FELT_D));
        self.draw_art(d.id, d.name, r.x + 3.0, r.y + 3.0, r.w - 6.0, hi);
        if let Some(k) = key {
            if k < 9 {
                self.skin.plate(Rect::new(r.x + r.w - 11.0, r.y + r.h - 11.0, 11.0, 11.0), 0xe89a1c, 0xffcc4d, 0x7a3d0c);
                self.font.draw(&format!("{}", k + 1), r.x + r.w - 8.0, r.y + r.h - 9.0, 1.0, hex(0x3d1d08));
            }
        }
    }

    // ------------------------------------------------------------------ tooltip: a pop-out card
    fn tooltip(&self, at: Vec2, title: &str, sub: &str, plate: (u32, u32, u32), body: &[String]) {
        let w = 184.0;
        let mut h = 30.0;
        for l in body { h += self.font.wrap(&strip_tags(l), w - 14.0, 1.0).len() as f32 * 9.0 + 4.0; }
        let mut x = at.x + 14.0;
        if x + w > VIEW_W - 4.0 { x = at.x - w - 14.0; }
        let x = x.max(4.0);
        let y = (at.y + 8.0).min(VIEW_H - h - 4.0).max(4.0);
        draw_rectangle(x + 3.0, y + 4.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        self.skin.plate(Rect::new(x, y, w, h), FELT_D, 0x2e2063, OUTLINE);
        self.skin.plate(Rect::new(x, y, w, 22.0), plate.0, plate.1, plate.2);
        self.font.draw_outlined(&title.to_uppercase(), x + 7.0, y + 4.0, 1.0, hex(TEXT));
        self.font.draw(sub, x + 7.0, y + 13.0, 1.0, hex(plate.1));
        let mut yy = y + 28.0;
        for l in body { yy += self.rich(l, x + 7.0, yy, w - 14.0, 1.0) + 4.0; }
    }

    // ------------------------------------------------------------------ left panel: the backbox
    pub fn draw_left(&mut self, _m: &Mouse) {
        if self.state == State::Title && !self.show_info { return; }
        let pr = Rect::new(5.0, 5.0, PANEL_W - 10.0, VIEW_H - 10.0);
        self.skin.bezel(pr);
        self.skin.tray(pr);
        let x = 12.0;
        let w = PANEL_W - 24.0;
        let info = table_info(&mut self.run, &self.db);
        let ribbon = Rect::new(x + 6.0, 12.0, w - 12.0, 22.0);
        if info.boss.is_some() { self.skin.banner(&self.font, ribbon, &info.name.to_uppercase(), 0x74141f, 0xee4040, 0x3a0b10, 1.0); }
        else { self.skin.banner(&self.font, ribbon, &info.name.to_uppercase(), 0x12669a, 0x49c6ec, 0x0f3f6b, 1.0); }
        let mut y = 40.0;
        if let Some(b) = info.boss {
            self.skin.icon("skull", x, y + 1.0, 1.0, WHITE);
            y += self.rich(self.db.bosses[b].desc, x + 10.0, y, w - 10.0, 1.0);
        }

        // dot-matrix display: goal, table score, progress
        let dr = Rect::new(x, y + 6.0, w, 72.0);
        self.skin.dmd(dr);
        let live = matches!(self.state, State::Play | State::Ending | State::Cashout);
        let (score, target) = self.table.as_ref().map(|t| (t.score, t.info.target)).unwrap_or((0.0, info.target));
        let on = hex(DMD_ON);
        let goal = format!("GOAL {}", num(target));
        let gp = if self.skin.dmd_width(&self.font, &goal, 2.0) > dr.w - 8.0 { 1.0 } else { 2.0 };
        self.skin.dmd_text(&self.font, &goal, dr.x + 4.0, dr.y + 4.0, gp, Color::new(on.r, on.g, on.b, 0.75));
        let sc = num(if live { score } else { 0.0 });
        let pitch = if self.skin.dmd_width(&self.font, &sc, 3.0) > dr.w - 8.0 { 2.0 } else { 3.0 };
        let sw = self.skin.dmd_width(&self.font, &sc, pitch);
        self.skin.dmd_text(&self.font, &sc, dr.x + (dr.w - sw) / 2.0, dr.y + 26.0, pitch, on);
        let frac = if live { (score / target).min(1.0) as f32 } else { 0.0 };
        self.skin.leds(Rect::new(dr.x + 3.0, dr.y + dr.h - 9.0, dr.w - 6.0, 6.0), 20, frac, if frac >= 1.0 { 0x3fc46a } else { 0xff8a1c }, 0x2a1306);
        y = dr.y + dr.h + 12.0;

        // points × mult plates
        let (p, mu) = match self.end.as_ref() { Some(e) => e.show, None => (self.ball.points, self.ball.mult.max(1.0)) };
        let bw = (w - 18.0) / 2.0;
        let bump_p = self.hits.get("tally:p").map(|t| (1.0 - (self.time - t) / 0.15).max(0.0) as f32).unwrap_or(0.0);
        let bump_m = self.hits.get("tally:m").map(|t| (1.0 - (self.time - t) / 0.15).max(0.0) as f32).unwrap_or(0.0);
        let pr1 = Rect::new(x, y - bump_p * 3.0, bw, 36.0);
        let pr2 = Rect::new(x + bw + 18.0, y - bump_m * 3.0, bw, 36.0);
        self.skin.plate(pr1, 0x12669a, 0x49c6ec, 0x0a2340);
        self.skin.plate(pr2, 0xb8222f, 0xff8a7a, 0x3a0b10);
        let ps = num(p);
        let ms = fmt((mu * 100.0).round() / 100.0);
        let s1 = if self.font.width(&ps, 2.0) > bw - 8.0 { 1.0 } else { 2.0 };
        let s2 = if self.font.width(&ms, 2.0) > bw - 8.0 { 1.0 } else { 2.0 };
        self.font.draw("POINTS", pr1.x + 4.0, pr1.y + 3.0, 1.0, hex(0xa3ecff));
        self.font.draw("MULT", pr2.x + 4.0, pr2.y + 3.0, 1.0, hex(0xffb3d2));
        self.font.draw_outlined_centered(&ps, pr1.x + bw / 2.0, pr1.y + 21.0 - 3.5 * s1, s1, hex(TEXT));
        self.font.draw_outlined_centered(&ms, pr2.x + bw / 2.0, pr2.y + 21.0 - 3.5 * s2, s2, hex(TEXT));
        self.font.draw_outlined_centered("×", x + bw + 9.0, y + 14.0, 1.0, hex(GOLD));
        y += 44.0;
        if let Some(e) = self.end.as_ref() {
            if e.total_shown {
                let s = format!("= {}", num(self.ball.score));
                self.font.draw_outlined_centered(&s, x + w / 2.0, y, 2.0, hex(0x8ff09e));
            }
        }
        y += 22.0;

        // balls, ante, money
        let br = Rect::new(x, y, w, 46.0);
        self.skin.plate(br, 0x1e1545, 0x2e2063, OUTLINE);
        let (left, total) = self.table.as_ref().map(|t| (t.balls_left + if self.state == State::Play { 1 } else { 0 }, t.balls)).unwrap_or((self.run.balls_per_table, self.run.balls_per_table));
        self.font.draw("BALLS", br.x + 6.0, br.y + 5.0, 1.0, hex(DIM));
        for i in 0..total.min(7) {
            self.skin.icon(if i < left { "ball" } else { "ball_off" }, br.x + 6.0 + i as f32 * 11.0, br.y + 15.0, 1.0, WHITE);
        }
        if total > 7 { self.font.draw(&format!("x{left}"), br.x + 6.0 + 7.0 * 11.0, br.y + 17.0, 1.0, hex(TEXT)); }
        self.font.draw("ANTE", br.x + 6.0, br.y + 30.0, 1.0, hex(DIM));
        for i in 0..8u32 {
            let lit = i < self.run.ante.min(8);
            self.skin.plate(Rect::new(br.x + 34.0 + i as f32 * 8.0, br.y + 29.0, 7.0, 9.0), if lit { 0xffcc4d } else { 0x2e2b47 }, if lit { 0xfff1a8 } else { 0x4a4766 }, OUTLINE);
        }
        let money = format!("{}", self.run.money);
        let mx = br.x + br.w - 8.0 - self.font.width(&money, 2.0);
        self.skin.icon("coin", mx - 13.0, br.y + 10.0, 1.0, WHITE);
        self.font.draw_outlined(&money, mx, br.y + 9.0, 2.0, hex(GOLD));
        y += 54.0;

        // tilt meter
        self.font.draw_outlined("TILT", x, y, 1.0, hex(DIM));
        let tl = (self.rules.tilt / (3.2 * self.run.tilt_tol)).min(1.0) as f32;
        self.skin.leds(Rect::new(x + 28.0, y - 1.0, w - 28.0, 9.0), 12, tl, if tl > 0.65 { 0xee4040 } else { 0xffcc4d }, 0x2e2b47);
        y += 18.0;

        // shot ticker: the last things that scored, like a backbox readout
        let tr = Rect::new(x, y + 4.0, w, VIEW_H - y - 46.0);
        if tr.h > 30.0 {
            self.skin.dmd(tr);
            let now = self.time;
            for (i, (txt, col, t0)) in self.ticker.iter().rev().take(((tr.h - 6.0) / 16.0) as usize).enumerate() {
                let age = (now - t0) as f32;
                let a = if age < 6.0 { 1.0 } else { (1.0 - (age - 6.0) / 3.0).max(0.25) };
                let c = hex(*col);
                let p = if self.skin.dmd_width(&self.font, txt, 2.0) > tr.w - 8.0 { 1.0 } else { 2.0 };
                self.skin.dmd_text(&self.font, txt, tr.x + 4.0, tr.y + 4.0 + i as f32 * 16.0 + if p < 2.0 { 3.0 } else { 0.0 }, p, Color::new(c.r, c.g, c.b, a));
            }
            if self.ticker.is_empty() {
                self.skin.dmd_text(&self.font, "INSERT BALL", tr.x + 4.0, tr.y + 4.0, 2.0, Color::new(1.0, 0.54, 0.11, 0.5));
            }
        }
        self.font.draw_outlined("TAB INFO  ESC PAUSE", x + 4.0, VIEW_H - 24.0, 1.0, hex(0x7d68d4));
    }

    // ------------------------------------------------------------------ right panel: the item tray
    pub fn draw_right(&mut self, m: &Mouse) {
        if self.state == State::Title && !self.show_info { return; }
        let pr = Rect::new(RIGHT_X + 5.0, 5.0, PANEL_W - 10.0, VIEW_H - 10.0);
        self.skin.bezel(pr);
        draw_rectangle(pr.x, pr.y, pr.w, pr.h, hex(0x1b1238));
        let x = RIGHT_X + 12.0;
        let w = PANEL_W - 24.0;
        let in_shop = self.state == State::Shop;
        let n_items = self.run.items.len();
        self.skin.banner(&self.font, Rect::new(x + 10.0, 12.0, w - 20.0, 18.0), &format!("ITEMS  {}", n_items), 0x42308a, 0x7d68d4, 0x1e1545, 1.0);
        let (cw, ch, gap) = (58.0, 58.0, 8.0);
        let cols = 2;
        let tray = Rect::new(x, 36.0, w, 286.0);
        self.skin.tray(tray);
        let top = tray.y + 6.0;
        let area_h = tray.h - 10.0;
        let rows = (n_items + cols - 1) / cols;
        let content_h = rows as f32 * (ch + gap);
        if tray.contains(m.pos) && m.wheel != 0.0 { self.item_scroll -= m.wheel.signum() * 31.0; }
        self.item_scroll = self.item_scroll.clamp(0.0, (content_h - area_h).max(0.0));
        let mut hover_item = None;
        let x0 = tray.x + (tray.w - (cols as f32 * cw + (cols as f32 - 1.0) * gap)) / 2.0;
        for i in 0..n_items {
            let rx = x0 + (i % cols) as f32 * (cw + gap);
            let ry = top + (i / cols) as f32 * (ch + gap) - self.item_scroll;
            if ry + ch < top - 2.0 || ry > top + area_h { continue; }
            let rect = Rect::new(rx, ry, cw, ch);
            if rect.contains(m.pos) { hover_item = Some(i); }
            let hl = rect.contains(m.pos) && self.drag.is_none();
            if self.drag != Some(i) { let it = self.run.items[i].clone(); self.item_card(&it, rx, ry, cw, ch, hl); }
        }
        // the panel edge hides cards scrolled out of view
        draw_rectangle(tray.x, tray.y - 6.0, tray.w, 6.0, hex(0x1b1238));
        draw_rectangle(tray.x, tray.y + tray.h, tray.w, 6.0, hex(0x1b1238));
        if content_h > area_h {
            let bar_h = (area_h * area_h / content_h).max(12.0);
            let by = top + (area_h - bar_h) * self.item_scroll / (content_h - area_h);
            self.skin.plate(Rect::new(tray.x + tray.w - 5.0, by, 4.0, bar_h), 0x7d68d4, 0xa596ec, 0x2e2063);
        }
        if n_items == 0 { self.font.draw_outlined_centered("EMPTY RACK", tray.x + tray.w / 2.0, tray.y + 20.0, 1.0, hex(0x5b45b0)); }
        if let Some(i) = hover_item {
            if m.click { self.drag = Some(i); }
            if m.rclick && in_shop { self.sell_item(i); return; }
        }
        if let Some(d) = self.drag {
            if d < self.run.items.len() { let it = self.run.items[d].clone(); self.item_card(&it, m.pos.x - cw / 2.0, m.pos.y - ch / 2.0, cw, ch, true); }
            if m.released {
                if let Some(t) = hover_item { if t != d { self.move_item(d, t); } }
                self.drag = None;
            }
        }

        // cards
        let n_cards = self.run.cards.len();
        let cy0 = tray.y + tray.h + 12.0;
        self.skin.banner(&self.font, Rect::new(x + 10.0, cy0, w - 20.0, 18.0), &format!("CARDS  {}", n_cards), 0x12669a, 0x49c6ec, 0x0a2340, 1.0);
        let ktray = Rect::new(x, cy0 + 24.0, w, VIEW_H - cy0 - 36.0);
        self.skin.tray(ktray);
        let (kw, kh, kg) = (54.0, 54.0, 8.0);
        let kcols = 2;
        let ktop = ktray.y + 6.0;
        let karea = ktray.h - 10.0;
        let krows = (n_cards + kcols - 1) / kcols;
        let kcontent = krows as f32 * (kh + kg);
        if ktray.contains(m.pos) && m.wheel != 0.0 { self.card_scroll -= m.wheel.signum() * 24.0; }
        self.card_scroll = self.card_scroll.clamp(0.0, (kcontent - karea).max(0.0));
        let kx0 = ktray.x + (ktray.w - (kcols as f32 * kw + (kcols as f32 - 1.0) * kg)) / 2.0;
        let mut hover_card = None;
        for i in 0..n_cards {
            let rx = kx0 + (i % kcols) as f32 * (kw + kg);
            let ry = ktop + (i / kcols) as f32 * (kh + kg) - self.card_scroll;
            if ry + kh < ktop - 2.0 || ry > ktop + karea { continue; }
            let hit = Rect::new(rx, ry, kw, kh).contains(m.pos);
            if hit { hover_card = Some(i); }
            self.card_thumb(self.run.cards[i].def, rx, ry, kw, kh, Some(i), hit);
        }
        draw_rectangle(ktray.x, ktray.y + ktray.h, ktray.w, 8.0, hex(0x1b1238));
        if n_cards == 0 { self.font.draw_outlined_centered("NO CARDS", ktray.x + ktray.w / 2.0, ktray.y + 20.0, 1.0, hex(0x1c95c8)); }
        if let Some(i) = hover_card {
            if m.click { self.use_card(i); return; }
            if m.rclick && in_shop { self.sell_card(i); return; }
        }
        if self.drag.is_none() {
            if let Some(i) = hover_item {
                let it = self.run.items[i].clone();
                let d = &self.db.items[it.def];
                let mut body = vec![d.desc.to_string()];
                if it.stat != 0.0 { body.push(format!("[k]Currently[/] {}", fmt((it.stat * 1000.0).round() / 1000.0))); }
                if let Some(ed) = it.edition { body.push(format!("[k]{}:[/] {}", edition_name(ed), edition_desc(ed))); }
                let sv = self.run.sell_value(&self.db, &it);
                body.push(if in_shop { format!("[c]Right-click: sell for ${sv}[/]") } else { "Drag to reorder. Items fire top to bottom".into() });
                let rar = match d.rarity { Rarity::Common => "COMMON", Rarity::Uncommon => "UNCOMMON", Rarity::Rare => "RARE" };
                self.tooltip(m.pos, d.name, rar, rarity_plate(d.rarity), &body);
            } else if let Some(i) = hover_card {
                let d = &self.db.cards[self.run.cards[i].def];
                let body = vec![d.desc.to_string(), if in_shop { "[c]Right-click: sell for $1[/]".into() } else { format!("Click or press [c]{}[/] to use", i + 1) }];
                let plate = if d.chip { (0x12669a, 0x49c6ec, 0x0f3f6b) } else { (0x42308a, 0x7d68d4, 0x2e2063) };
                self.tooltip(m.pos, d.name, if d.chip { "CHIP" } else { "CHARM" }, plate, &body);
            }
        }
    }

    // ------------------------------------------------------------------ in-play overlays
    pub fn draw_hud_overlays(&mut self) {
        let cx = TABLE_CX;
        if let Some(b) = self.banner.as_ref() {
            let age = (get_time() - (b.until - 1.4)) as f32;
            let s = 4.0;
            let w = self.font.width(&b.text, s);
            let slide = (1.0 - (age / 0.12).min(1.0)) * 30.0;
            let r = Rect::new(cx - w / 2.0 - 16.0, 44.0 - slide, w + 32.0, 44.0);
            let c = b.color;
            self.skin.banner(&self.font, r, "", 0x1b1830, 0x4a4766, OUTLINE, 1.0);
            draw_rectangle(r.x + 2.0, r.y + 2.0, r.w - 4.0, 3.0, c);
            draw_rectangle(r.x + 2.0, r.y + r.h - 5.0, r.w - 4.0, 3.0, c);
            self.font.draw_outlined_centered(&b.text, cx, r.y + 12.0, s, c);
        }
        for (i, (t, _)) in self.toasts.iter().enumerate() {
            let w = self.font.width(t, 1.0);
            let y = VIEW_H - 30.0 - i as f32 * 18.0;
            self.skin.plate(Rect::new(cx - w / 2.0 - 8.0, y - 4.0, w + 16.0, 15.0), 0x1b1830, 0x4a4766, OUTLINE);
            self.font.draw_outlined(t, (cx - w / 2.0).round(), y, 1.0, hex(TEXT));
        }
        for p in &self.popups {
            if p.world.is_some() { continue; }
            let a = if p.age < 0.8 { 1.0 } else { 1.0 - (p.age - 0.8) / 0.3 };
            let s = if p.big { 2.0 } else { 1.0 };
            let w = self.font.width(&p.text, s);
            self.font.draw_outlined(&p.text, (RIGHT_X - w - 8.0).round(), (170.0 - p.age * 40.0).round(), s, Color::new(p.color.r, p.color.g, p.color.b, a));
        }
        if self.show_fps {
            self.font.draw_outlined(&format!("{} FPS", get_fps()), TABLE_X + 6.0, 6.0, 1.0, hex(0x8ff09e));
        }
    }

    // ------------------------------------------------------------------ screens
    /// a framed window over the table with a ribbon title
    fn window(&self, w: f32, h: f32, title: &str, plate: (u32, u32, u32)) -> Rect {
        draw_rectangle(TABLE_X, 0.0, TABLE_VIEW_W, VIEW_H, Color::new(0.03, 0.02, 0.07, 0.62));
        let r = Rect::new((TABLE_CX - w / 2.0).round(), ((VIEW_H - h) / 2.0 + 8.0).round(), w, h);
        self.skin.bezel(r);
        self.skin.tray(r);
        if !title.is_empty() {
            let tw = self.font.width(title, 2.0) + 40.0;
            self.skin.banner(&self.font, Rect::new(r.x + (r.w - tw) / 2.0, r.y - 14.0, tw, 26.0), title, plate.0, plate.1, plate.2, 2.0);
        }
        r
    }

    pub fn draw_screens(&mut self, m: &Mouse) {
        if self.show_info { self.screen_info(m); return; }
        match self.state {
            State::Title => self.screen_title(m),
            State::Select => self.screen_select(m),
            State::Cashout => self.screen_cashout(m),
            State::Shop => self.screen_shop(m),
            State::Over | State::Win => self.screen_over(m),
            State::Play | State::Ending => { if self.paused { self.screen_pause(m); } }
        }
    }

    /// the side panels make way for a starfield over a scrolling neon grid
    fn title_backdrop(&self, t: f64) {
        for (x0, x1) in [(0.0, TABLE_X), (TABLE_X + TABLE_VIEW_W, VIEW_W)] {
            let w = x1 - x0;
            // night sky fading into the horizon glow
            for k in 0..18 {
                let y = k as f32 * 18.0;
                let c = [0x0b0a14, 0x120c2b, 0x1e1545, 0x2e2063][(k / 5).min(3) as usize];
                draw_rectangle(x0, y, w, 18.0, hex(c));
            }
            let hz = 324.0;
            draw_rectangle(x0, hz - 4.0, w, 4.0, hex(0xa81c66));
            draw_rectangle(x0, hz - 1.0, w, 1.0, hex(0xff6aa8));
            // stars: fixed spots that twinkle
            for i in 0..42u32 {
                let hsh = i.wrapping_mul(2654435761);
                let sx = x0 + (hsh % 1000) as f32 / 1000.0 * w;
                let sy = ((hsh >> 10) % 1000) as f32 / 1000.0 * (hz - 30.0);
                let tw = ((t * (1.0 + (i % 5) as f64 * 0.4) + i as f64).sin() * 0.5 + 0.5) as f32;
                let c = if i % 7 == 0 { 0xa3ecff } else if i % 5 == 0 { 0xffb3d2 } else { 0xeef0f7 };
                let col = hex(c);
                draw_rectangle(sx.round(), sy.round(), 1.0, 1.0, Color::new(col.r, col.g, col.b, 0.3 + 0.7 * tw));
                if tw > 0.9 && i % 3 == 0 {
                    draw_rectangle(sx.round() - 1.0, sy.round(), 3.0, 1.0, Color::new(col.r, col.g, col.b, 0.5));
                    draw_rectangle(sx.round(), sy.round() - 1.0, 1.0, 3.0, Color::new(col.r, col.g, col.b, 0.5));
                }
            }
            // the grid floor rushing toward the viewer
            draw_rectangle(x0, hz, w, VIEW_H - hz, hex(0x120c2b));
            let phase = (t * 0.6).fract() as f32;
            for k in 0..14 {
                let d = (k as f32 + phase) / 14.0;
                let y = hz + (VIEW_H - hz) * d * d;
                let a = 0.25 + 0.75 * d;
                draw_rectangle(x0, y.round(), w, 1.0, Color::new(1.0, 0.42, 0.66, a * 0.8));
            }
            let vx = if x0 < 1.0 { TABLE_X + 40.0 } else { TABLE_X + TABLE_VIEW_W - 40.0 };
            for k in -10..=10 {
                let bx = vx + k as f32 * 70.0;
                let steps = 24;
                for s in 0..steps {
                    let f = s as f32 / steps as f32;
                    let x = vx + (bx - vx) * f;
                    let y = hz + (VIEW_H - hz) * f;
                    if x >= x0 && x < x1 { draw_rectangle(x.round(), y.round(), 1.0, 1.0, Color::new(0.29, 0.78, 0.93, 0.25 + 0.6 * f)); }
                }
            }
        }
        // bevelled frames where the panels usually sit
        for x in [TABLE_X - 3.0, TABLE_X + TABLE_VIEW_W] {
            draw_rectangle(x, 0.0, 3.0, VIEW_H, hex(METAL_D));
            draw_rectangle(x + 1.0, 0.0, 1.0, VIEW_H, hex(METAL_L));
        }
    }

    /// the logo: chunky gradient block letters on a wave, with a chrome shine and a ball hopping across
    fn title_logo(&self, cx: f32, y0: f32, t: f64) -> f32 {
        if let Some((tex, white)) = &self.logo {
            let (w, h) = (tex.width(), tex.height());
            let x = (cx - w / 2.0).round();
            let y = (y0 + ((t * 1.8).sin() * 3.0) as f32).round();
            draw_texture(tex, x, y, WHITE);
            // a chrome glint sweeping across: slices of the white silhouette
            let sweep = ((t * 0.4).fract() as f32) * (w + 160.0) - 80.0;
            for (bw, a) in [(26.0, 0.18), (12.0, 0.35), (4.0, 0.6)] {
                let sx = (sweep - bw / 2.0).max(0.0);
                let ex = (sweep + bw / 2.0).min(w);
                if ex > sx {
                    draw_texture_ex(white, x + sx, y, Color::new(1.0, 1.0, 1.0, a), DrawTextureParams { source: Some(Rect::new(sx, 0.0, ex - sx, h)), ..Default::default() });
                }
            }
            return y0 + h;
        }
        let px = 7.0;
        let word = "ROGUEBALL";
        let widths: Vec<usize> = word.chars().map(|c| self.font.bits(c).first().map(|r| r.len()).unwrap_or(5)).collect();
        let total: f32 = widths.iter().map(|w| (*w as f32 + 1.0) * px).sum::<f32>() - px + px;
        let x0 = (cx - total / 2.0).round();
        let pink = [0xffb3d2, 0xff6aa8, 0xff6aa8, 0xe0337f, 0xe0337f, 0xa81c66, 0x6b1048];
        let cyan = [0xa3ecff, 0x49c6ec, 0x49c6ec, 0x1c95c8, 0x1c95c8, 0x12669a, 0x0f3f6b];
        let shine = ((t * 0.45).fract() as f32) * (total + 300.0) - 150.0;
        let mut x = x0;
        let mut tops = vec![];
        for (i, c) in word.chars().enumerate() {
            let bits = self.font.bits(c);
            let wave = ((t * 2.6 - i as f64 * 0.55).sin() * 4.0).round() as f32;
            let ly = y0 + wave;
            tops.push((x + widths[i] as f32 * px / 2.0, ly));
            let ramp = if i < 5 { &pink } else { &cyan };
            // deep shadow, then a dark outline, then the faces
            for pass in 0..3 {
                for (j, row) in bits.iter().enumerate() {
                    for (k, on) in row.iter().enumerate() {
                        if !*on { continue; }
                        let bx = x + k as f32 * px;
                        let by = ly + j as f32 * px;
                        match pass {
                            0 => {
                                draw_rectangle(bx + 5.0, by + 7.0, px, px, hex(0x0b0a14));
                                draw_rectangle(bx + 3.0, by + 4.0, px, px, hex(0x2e2063));
                            }
                            1 => draw_rectangle(bx - 2.0, by - 2.0, px + 4.0, px + 4.0, hex(0x0b0a14)),
                            _ => {
                                let mut col = ramp[j.min(6)];
                                // the chrome sweep: a slanted band that brightens what it crosses
                                let d = (bx - x0) + (by - y0) * 0.6 - shine;
                                if d.abs() < 10.0 { col = 0xffffff; } else if d.abs() < 18.0 { col = ramp[0]; }
                                draw_rectangle(bx, by, px, px, hex(col));
                                // a bevel on each block: light top edge, dark bottom edge
                                draw_rectangle(bx, by, px, 1.0, Color::new(1.0, 1.0, 1.0, 0.35));
                                draw_rectangle(bx, by + px - 1.0, px, 1.0, Color::new(0.0, 0.0, 0.0, 0.25));
                            }
                        }
                    }
                }
            }
            x += (widths[i] as f32 + 1.0) * px;
        }
        // a ball hopping letter to letter
        let n = tops.len();
        let period = 0.55;
        let u = t / period;
        let hop = (u.floor() as usize) % (n + 3);
        let f = u.fract() as f32;
        if hop + 1 < n {
            let (ax, ay) = tops[hop];
            let (bx, by) = tops[hop + 1];
            let bxp = ax + (bx - ax) * f;
            let byp = (ay + (by - ay) * f) - 6.0 - (f * std::f32::consts::PI).sin() * 26.0;
            let kit = &self.kits[0];
            if !kit.draw("ball", 0, vec2(bxp, byp), WHITE) { draw_circle(bxp, byp, 12.0, hex(0xc7c7d9)); }
            if f < 0.25 {
                // sparks where it just landed
                for s in 0..6 {
                    let a = s as f32 * 1.05 + hop as f32;
                    let r = 6.0 + f * 60.0;
                    let c = if s % 2 == 0 { hex(0xffcc4d) } else { hex(0xffffff) };
                    draw_rectangle((ax + a.cos() * r).round(), (ay - 6.0 - a.sin().abs() * r * 0.6).round(), 2.0, 2.0, Color::new(c.r, c.g, c.b, 1.0 - f * 4.0));
                }
            }
        }
        y0 + 7.0 * px
    }

    fn screen_title(&mut self, m: &Mouse) {
        let t = get_time();
        let cx = TABLE_CX;
        self.title_backdrop(t);
        // the demo game plays on under a dark veil, darker behind the logo
        draw_rectangle(TABLE_X, 0.0, TABLE_VIEW_W, VIEW_H, Color::new(0.03, 0.02, 0.07, 0.42));
        for k in 0..12 {
            draw_rectangle(TABLE_X, k as f32 * 10.0, TABLE_VIEW_W, 10.0, Color::new(0.03, 0.02, 0.07, 0.5 - k as f32 * 0.04));
        }
        let logo_bottom = self.title_logo(cx, 4.0, t);
        let ry = (logo_bottom - 2.0).min(244.0);
        self.skin.banner(&self.font, Rect::new(cx - 130.0, ry, 260.0, 20.0), "A PINBALL ROGUELIKE", 0x42308a, 0x7d68d4, 0x1e1545, 1.0);
        let top = ry + 28.0;

        // cabinet carousel
        let n = self.db.cabinets.len();
        let step = |g: &mut Game, d: i32| { g.pick_cabinet = ((g.pick_cabinet as i32 + d).rem_euclid(n as i32)) as usize; g.sfx.play("ui"); };
        use crate::platform::btn;
        if is_key_pressed(KeyCode::Left) || self.pad.nav(btn::LEFT) || self.pad.pressed(btn::LB) { step(self, -1); }
        if is_key_pressed(KeyCode::Right) || self.pad.nav(btn::RIGHT) || self.pad.pressed(btn::RB) { step(self, 1); }
        let i = self.pick_cabinet;
        let ok = self.unlocked(i);
        let card = Rect::new(cx - 110.0, top, 220.0, 96.0);
        draw_rectangle(card.x + 3.0, card.y + 5.0, card.w, card.h, Color::new(0.0, 0.0, 0.0, 0.5));
        self.skin.bezel(card);
        self.skin.plate(Rect::new(card.x + 4.0, card.y + 4.0, card.w - 8.0, card.h - 8.0), 0x2e2063, 0x5b45b0, OUTLINE);
        // a little pinball cabinet in this cabinet's colour
        let col = [0x9a99b3, 0xee4040, 0x49c6ec, 0xffcc4d, 0xa3ecff, 0x2e2b47][i % 6];
        let bx = card.x + 22.0;
        let by = card.y + 10.0 + ((t * 2.0).sin() * 2.0).round() as f32;
        self.skin.plate(Rect::new(bx + 6.0, by, 44.0, 26.0), col, 0xeef0f7, 0x0b0a14);
        draw_rectangle(bx + 11.0, by + 5.0, 34.0, 14.0, hex(0x120c2b));
        self.skin.dmd_text(&self.font, "RB", bx + 15.0, by + 7.0, 2.0, hex(DMD_ON));
        self.skin.plate(Rect::new(bx, by + 26.0, 56.0, 40.0), col, 0xeef0f7, 0x0b0a14);
        draw_rectangle(bx + 6.0, by + 31.0, 44.0, 30.0, hex(0x120c2b));
        for k in 0..3 { draw_rectangle(bx + 12.0 + k as f32 * 13.0, by + 38.0, 6.0, 6.0, hex([0x49c6ec, 0xff6aa8, 0xffcc4d][k])); }
        draw_rectangle(bx + 6.0, by + 66.0, 6.0, 10.0, hex(0x0b0a14));
        draw_rectangle(bx + 44.0, by + 66.0, 6.0, 10.0, hex(0x0b0a14));
        if !ok {
            draw_rectangle(bx - 2.0, by - 2.0, 60.0, 80.0, Color::new(0.03, 0.02, 0.07, 0.6));
            self.skin.icon("lock", bx + 21.0, by + 28.0, 2.0, WHITE);
        }
        let nm = self.db.cabinets[i].name.to_uppercase();
        let tx = card.x + 92.0;
        self.font.draw_outlined(&nm, tx, card.y + 18.0, 1.0, hex(if ok { GOLD } else { 0x6f6d8c }));
        let desc = if ok { self.db.cabinets[i].desc.to_string() } else { format!("[x]LOCKED[/] {}", self.db.cabinets[i].unlock) };
        self.rich(&desc, tx, card.y + 34.0, card.w - 102.0, 1.0);
        // page dots and arrows
        for k in 0..n {
            let on = k == i;
            draw_rectangle(cx - n as f32 * 6.0 + k as f32 * 12.0, card.y + card.h + 8.0, 7.0, 7.0, hex(if on { 0xff6aa8 } else { 0x42308a }));
        }
        let pulse = ((t * 4.0).sin() * 2.0).round() as f32;
        for (dir, ax) in [(-1, card.x - 34.0 - pulse), (1, card.x + card.w + 8.0 + pulse)] {
            let r = Rect::new(ax, card.y + card.h / 2.0 - 13.0, 26.0, 26.0);
            let hover = r.contains(m.pos);
            self.skin.plate(r, if hover { 0x5b45b0 } else { 0x2e2063 }, 0xa596ec, OUTLINE);
            self.font.draw_outlined_centered(if dir < 0 { "<" } else { ">" }, r.x + 13.0, r.y + 6.0, 2.0, hex(TEXT));
            if hover && m.click { step(self, dir); }
        }

        // start: a glowing pulse behind the button
        let br = Rect::new(cx - 100.0, card.y + card.h + 22.0, 200.0, 36.0);
        let glow = (0.5 + 0.5 * (t * 3.0).sin()) as f32;
        for g in 0..4 {
            let e = (4 - g) as f32 * 3.0;
            draw_rectangle(br.x - e, br.y - e, br.w + 2.0 * e, br.h + 2.0 * e, Color::new(1.0, 0.42, 0.66, 0.06 * glow * (g + 1) as f32));
        }
        let start = ok && (is_key_pressed(KeyCode::Enter) || self.button(m, br, "START RUN", 1, ok));
        if start { self.start_run(); return; }
        if !ok { self.font.draw_outlined_centered("PICK AN UNLOCKED CABINET", cx, br.y + br.h + 6.0, 1.0, hex(0xff8a7a)); }

        // stats on a little dot-matrix readout
        let sr = Rect::new(cx - 150.0, br.y + br.h + 14.0, 300.0, 22.0);
        self.skin.dmd(sr);
        let st = format!("BEST ANTE {}  RUNS {}  WINS {}", self.meta.best_ante.max(1), self.meta.runs, self.meta.wins);
        let sw = self.skin.dmd_width(&self.font, &st, 1.0);
        self.skin.dmd_text(&self.font, &st, sr.x + (sr.w - sw) / 2.0, sr.y + 7.0, 1.0, hex(DMD_ON));

        // the controls scroll past on a marquee
        let mr = Rect::new(TABLE_X + 18.0, VIEW_H - 34.0, TABLE_VIEW_W - 36.0, 26.0);
        self.skin.bezel(mr);
        self.skin.dmd(mr);
        let msg = "Z / L-SHIFT LEFT FLIPPER  *  / / R-SHIFT RIGHT FLIPPER  *  SPACE LAUNCH  *  A D W NUDGE  *  1-9 USE CARDS  *  TAB RUN INFO  *  F11 FULLSCREEN  *  CONTROLLERS WORK TOO  *  ";
        let mw = self.skin.dmd_width(&self.font, msg, 2.0);
        let off = ((t * 60.0) as f32) % mw;
        let mut mx = mr.x + 4.0 - off;
        while mx < mr.x + mr.w {
            // clip to the display by drawing glyph by glyph
            for ch in msg.chars() {
                let s = ch.to_string();
                let gw = self.skin.dmd_width(&self.font, &s, 2.0) + 2.0;
                if mx > mr.x + 2.0 && mx + gw < mr.x + mr.w - 2.0 { self.skin.dmd_text(&self.font, &s, mx, mr.y + 6.0, 2.0, hex(DMD_ON)); }
                mx += gw;
            }
        }
        if !crate::platform::IS_WEB && self.button(m, Rect::new(TABLE_X + TABLE_VIEW_W - 76.0, 10.0, 60.0, 18.0), "QUIT", 0, true) { std::process::exit(0); }
    }

    fn screen_select(&mut self, m: &Mouse) {
        let r = self.window(500.0, 368.0, &format!("ANTE {}{}", self.run.ante, if self.run.ante > 8 { " ENDLESS" } else { "" }), (0x42308a, 0x7d68d4, 0x1e1545));
        let saved = self.run.blind;
        for i in 0..3u32 {
            self.run.blind = i;
            let info = table_info(&mut self.run, &self.db);
            self.run.blind = saved;
            let tag = if i < 2 { Some(self.run.tag_for(self.run.ante, i)) } else { None };
            let current = i == saved;
            let bob = if current { ((get_time() * 3.0).sin() * 2.0).round() as f32 } else { 0.0 };
            let col = Rect::new(r.x + 14.0 + i as f32 * 160.0, r.y + 22.0 + bob, 150.0, 334.0);
            let (b0, b1, b2) = if info.boss.is_some() { (0x74141f, 0xee4040, 0x3a0b10) } else if i == 1 { (0x7a3d0c, 0xe89a1c, 0x3d1d08) } else { (0x12669a, 0x49c6ec, 0x0a2340) };
            draw_rectangle(col.x + 3.0, col.y + 4.0, col.w, col.h, Color::new(0.0, 0.0, 0.0, 0.45));
            self.skin.plate(col, if current { 0x2e2063 } else { 0x1b1830 }, if current { 0x5b45b0 } else { 0x2e2b47 }, OUTLINE);
            self.skin.plate(Rect::new(col.x + 4.0, col.y + 4.0, col.w - 8.0, 24.0), b0, b1, b2);
            self.font.draw_outlined_centered(&info.name.to_uppercase(), col.x + col.w / 2.0, col.y + 12.0, 1.0, hex(TEXT));
            if info.boss.is_some() { self.skin.icon("skull", col.x + 8.0, col.y + 9.0, 1.0, WHITE); }
            // the generated machine itself, once the factory has built and play-tested it
            let key = (self.run.ante, i);
            let pr = Rect::new(col.x + 10.0, col.y + 34.0, col.w - 20.0, 110.0);
            draw_rectangle(pr.x, pr.y, pr.w, pr.h, hex(FELT_D));
            if let Some(tex) = self.previews.get(&key) {
                let s = (pr.h / tex.height()).min(pr.w / tex.width());
                let (tw, th) = (tex.width() * s, tex.height() * s);
                draw_texture_ex(tex, (pr.x + (pr.w - tw) / 2.0).round(), (pr.y + (pr.h - th) / 2.0).round(), WHITE, DrawTextureParams { dest_size: Some(vec2(tw, th)), ..Default::default() });
                if let Some(l) = self.factory.get(key) {
                    let mut f = vec![];
                    if !l.ramps.is_empty() { f.push(format!("{} RAMP", l.ramps.len())); }
                    if !l.features.orbits.is_empty() { f.push(format!("{} ORBIT", l.features.orbits.len())); }
                    let up = l.flippers.iter().filter(|f| !f.main).count();
                    if up > 0 { f.push(format!("{} FLIP", up + 2)); }
                    if l.hw > 0.29 { f.push("WIDE".into()); }
                    self.font.draw_outlined_centered(&f.join(" "), col.x + col.w / 2.0, col.y + 148.0, 1.0, hex(0xa3ecff));
                }
            } else {
                let dots = ".".repeat(((get_time() * 3.0) as usize) % 4);
                self.font.draw_outlined_centered(&format!("BUILDING{dots}"), pr.x + pr.w / 2.0, pr.y + pr.h / 2.0 - 3.0, 1.0, hex(0x7d68d4));
            }
            let desc = info.boss.map(|b| self.db.bosses[b].desc).unwrap_or("No modifiers");
            for (k, l) in self.font.wrap(desc, col.w - 14.0, 1.0).iter().enumerate() {
                self.font.draw_outlined_centered(l, col.x + col.w / 2.0, col.y + 162.0 + k as f32 * 10.0, 1.0, hex(if info.boss.is_some() { 0xff8a7a } else { DIM }));
            }
            let gr = Rect::new(col.x + 10.0, col.y + 188.0, col.w - 20.0, 30.0);
            self.skin.dmd(gr);
            let gs = num(info.target);
            let gw = self.skin.dmd_width(&self.font, &gs, 3.0);
            self.skin.dmd_text(&self.font, &gs, gr.x + (gr.w - gw) / 2.0, gr.y + 6.0, 3.0, hex(DMD_ON));
            for k in 0..info.reward {
                self.skin.icon("coin", col.x + col.w / 2.0 - (info.reward as f32 * 11.0) / 2.0 + k as f32 * 11.0, col.y + 224.0, 1.0, WHITE);
            }
            if current {
                let ready = self.factory.get(key).is_some();
                if self.button(m, Rect::new(col.x + 14.0, col.y + 242.0, col.w - 28.0, 30.0), if ready { "PLAY" } else { "BUILDING" }, 1, ready) { self.play_table(); return; }
                if let Some(t) = tag {
                    let (tn, td) = tag_info(t);
                    if self.button(m, Rect::new(col.x + 14.0, col.y + 278.0, col.w - 28.0, 20.0), &format!("SKIP: {}", tn.to_uppercase()), 0, true) { self.skip_table(); return; }
                    self.rich(td, col.x + 10.0, col.y + 304.0, col.w - 20.0, 1.0);
                }
            } else {
                self.font.draw_outlined_centered(if i < saved { "CLEARED" } else { "UPCOMING" }, col.x + col.w / 2.0, col.y + 256.0, 1.0, hex(if i < saved { 0x3fc46a } else { 0x6f6d8c }));
            }
        }
    }

    fn screen_cashout(&mut self, m: &Mouse) {
        let lines = self.cash_lines.clone();
        let r = self.window(300.0, 116.0 + lines.len() as f32 * 16.0, "CLEARED!", (0x1f8a4c, 0x3fc46a, 0x145233));
        let cx = r.x + r.w / 2.0;
        let (score, target) = self.table.as_ref().map(|t| (t.score, t.info.target)).unwrap_or((0.0, 0.0));
        self.font.draw_outlined_centered(&format!("{} / {}", num(score), num(target)), cx, r.y + 22.0, 1.0, hex(DIM));
        let rr = Rect::new(r.x + 20.0, r.y + 38.0, r.w - 40.0, lines.len() as f32 * 16.0 + 8.0);
        self.skin.plate(rr, 0xd39a62, 0xfff1a8, 0x7a4524);
        let mut y = rr.y + 6.0;
        for (l, v) in &lines {
            self.font.draw(&l.to_uppercase(), rr.x + 8.0, y, 1.0, hex(0x2a160c));
            if *v > 0 { let s = format!("${v}"); self.font.draw(&s, rr.x + rr.w - 8.0 - self.font.width(&s, 1.0), y, 1.0, hex(0x2a160c)); }
            y += 16.0;
        }
        let total: i64 = lines.iter().map(|l| l.1).sum();
        if self.button(m, Rect::new(cx - 80.0, rr.y + rr.h + 10.0, 160.0, 30.0), &format!("CASH OUT ${total}"), 1, true) { self.cash_out(); }
    }

    fn screen_shop(&mut self, m: &Mouse) {
        let r = self.window(512.0, 470.0, "SHOP", (0xa81c66, 0xff6aa8, 0x6b1048));
        let ms = format!("{}", self.run.money);
        let mw = self.font.width(&ms, 2.0) + 24.0;
        self.skin.plate(Rect::new(r.x + r.w - mw - 12.0, r.y + 8.0, mw + 4.0, 22.0), 0x3d1d08, 0x7a3d0c, OUTLINE);
        self.skin.icon("coin", r.x + r.w - mw - 6.0, r.y + 14.0, 1.0, WHITE);
        self.font.draw_outlined(&ms, r.x + r.w - mw + 8.0, r.y + 12.0, 2.0, hex(GOLD));
        let Some(shop) = self.shop.as_ref() else { return };
        let n = shop.offers.len();
        let mut tip: Option<(String, String, (u32, u32, u32), Vec<String>)> = None;
        let mut action: Option<(usize, bool)> = None;
        for i in 0..n {
            let (kind_item, def, ed, price, sold) = {
                let o = &self.shop.as_ref().unwrap().offers[i];
                match o.kind { OfferKind::Item(d, e) => (true, d, e, o.price, o.sold), OfferKind::Card(d) => (false, d, None, o.price, o.sold) }
            };
            let tall = n <= 4;
            let (bh, art) = if tall { (330.0, 96.0) } else { (162.0, 48.0) };
            let bx = r.x + 14.0 + (i % 4) as f32 * 122.0;
            let by = r.y + 38.0 + (i / 4) as f32 * 168.0;
            let ped = Rect::new(bx, by, 114.0, bh);
            if sold {
                self.skin.plate(ped, 0x1b1830, 0x2e2b47, OUTLINE);
                self.font.draw_outlined_centered("SOLD", bx + 57.0, by + bh / 2.0 - 7.0, 2.0, hex(0x4a4766));
                continue;
            }
            let (name, desc, plate) = if kind_item {
                let d = &self.db.items[def];
                (d.name, d.desc, rarity_plate(d.rarity))
            } else {
                let d = &self.db.cards[def];
                (d.name, d.desc, if d.chip { (0x12669a, 0x49c6ec, 0x0f3f6b) } else { (0x42308a, 0x7d68d4, 0x2e2063) })
            };
            let id = if kind_item { self.db.items[def].id } else { self.db.cards[def].id };
            let hover = ped.contains(m.pos);
            self.skin.plate(ped, 0x1b1830, 0x2e2b47, OUTLINE);
            let face = Rect::new(bx + (114.0 - art - 12.0) / 2.0, by + 6.0 - if hover { 2.0 } else { 0.0 }, art + 12.0, art + 12.0);
            draw_rectangle(face.x + 2.0, face.y + 3.0, face.w, face.h, Color::new(0.0, 0.0, 0.0, 0.4));
            self.skin.plate(face, plate.0, plate.1, plate.2);
            draw_rectangle(face.x + 6.0, face.y + 6.0, art, art, hex(FELT_D));
            self.draw_art(id, name, face.x + 6.0, face.y + 6.0, art, plate.1);
            if let Some(e) = ed {
                let c = match e { Edition::Foil => 0xa3ecff, Edition::Holo => 0xff6aa8, Edition::Poly => 0xffcc4d };
                self.skin.icon("gem", face.x + face.w - 8.0, face.y + 2.0, 1.0, hex(c));
            }
            let ty = face.y + face.h + 6.0;
            let nl = self.font.wrap(&name.to_uppercase(), 106.0, 1.0);
            for (k, l) in nl.iter().enumerate() { self.font.draw_outlined_centered(l, bx + 57.0, ty + k as f32 * 10.0, 1.0, hex(plate.1)); }
            if tall { self.rich(desc, bx + 7.0, ty + nl.len() as f32 * 10.0 + 4.0, 100.0, 1.0); }
            let bottom = by + bh;
            let can = self.run.money >= price;
            let single = kind_item || self.db.cards[def].play_only;
            let tag_r = Rect::new(bx + 30.0, bottom - if single { 62.0 } else { 84.0 }, 54.0, 18.0);
            self.skin.plate(tag_r, 0xe89a1c, 0xffcc4d, 0x7a3d0c);
            self.skin.icon("coin", tag_r.x + 5.0, tag_r.y + 4.0, 1.0, WHITE);
            self.font.draw_outlined(&format!("{price}"), tag_r.x + 18.0, tag_r.y + 5.0, 1.0, hex(if can { TEXT } else { 0xff8a7a }));
            if single {
                if self.button(m, Rect::new(bx + 8.0, bottom - 36.0, 98.0, 26.0), "BUY", 2, can) { action = Some((i, false)); }
            } else {
                if self.button(m, Rect::new(bx + 8.0, bottom - 58.0, 98.0, 22.0), "BUY", 2, can) { action = Some((i, false)); }
                if self.button(m, Rect::new(bx + 8.0, bottom - 30.0, 98.0, 22.0), "BUY & USE", 0, can) { action = Some((i, true)); }
            }
            if hover && !tall {
                let mut body = vec![desc.to_string()];
                if let Some(e) = ed { body.push(format!("[k]{}:[/] {}", edition_name(e), edition_desc(e))); }
                tip = Some((name.to_string(), if kind_item { format!("{:?}", self.db.items[def].rarity).to_uppercase() } else { "CARD".into() }, plate, body));
            }
        }
        if let Some((i, u)) = action { self.buy(i, u); return; }
        // voucher coupon
        let vy = r.y + 378.0;
        let (v, vp, vsold) = { let s = self.shop.as_ref().unwrap(); (s.voucher, s.voucher_price, s.voucher_sold) };
        let vr = Rect::new(r.x + 14.0, vy, r.w - 28.0, 40.0);
        self.skin.plate(vr, 0x2e2063, 0x7d68d4, OUTLINE);
        let mut k = vr.x + 4.0;
        while k < vr.x + vr.w - 4.0 { draw_rectangle(k, vr.y + 3.0, 3.0, 1.0, hex(0xa596ec)); draw_rectangle(k, vr.y + vr.h - 4.0, 3.0, 1.0, hex(0xa596ec)); k += 6.0; }
        match (v, vsold) {
            (Some(v), false) => {
                let d = &self.db.vouchers[v];
                self.font.draw_outlined(&format!("VOUCHER: {}", d.name.to_uppercase()), vr.x + 10.0, vr.y + 9.0, 1.0, hex(0xa596ec));
                self.rich(d.desc, vr.x + 10.0, vr.y + 22.0, 300.0, 1.0);
                if self.button(m, Rect::new(vr.x + vr.w - 110.0, vr.y + 7.0, 100.0, 26.0), &format!("BUY ${vp}"), 3, self.run.money >= vp) { self.buy_voucher(); return; }
            }
            (Some(_), true) => self.font.draw_outlined("VOUCHER REDEEMED", vr.x + 10.0, vr.y + 16.0, 1.0, hex(DIM)),
            _ => self.font.draw_outlined("NO VOUCHER THIS ANTE", vr.x + 10.0, vr.y + 16.0, 1.0, hex(DIM)),
        }
        let rc = self.reroll_cost();
        if self.button(m, Rect::new(r.x + 14.0, r.y + r.h - 40.0, 140.0, 30.0), &format!("REROLL ${rc}"), 3, self.run.money >= rc) { self.reroll(); return; }
        if self.button(m, Rect::new(r.x + r.w - 174.0, r.y + r.h - 40.0, 160.0, 30.0), "NEXT TABLE", 1, true) { self.shop = None; self.next_table(); return; }
        if let Some((t, s, p, b)) = tip { self.tooltip(m.pos, &t, &s, p, &b); }
    }

    fn screen_over(&mut self, m: &Mouse) {
        let won = self.state == State::Win;
        let r = self.window(320.0, 200.0, if won { "MACHINE BROKEN" } else { "DRAINED" }, if won { (0x1f8a4c, 0x3fc46a, 0x145233) } else { (0x74141f, 0xee4040, 0x3a0b10) });
        let cx = r.x + r.w / 2.0;
        let s = &self.run.stats;
        let rows = [("ANTE REACHED", format!("{}", self.run.ante)), ("TABLES CLEARED", format!("{}", s.tables)), ("BEST BALL", num(s.best_ball)), ("ITEMS BOUGHT", format!("{}", s.bought))];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = r.y + 24.0 + i as f32 * 22.0;
            self.skin.plate(Rect::new(r.x + 16.0, y, r.w - 32.0, 18.0), 0x1b1830, 0x2e2b47, OUTLINE);
            self.font.draw(k, r.x + 24.0, y + 6.0, 1.0, hex(DIM));
            self.font.draw_outlined(v, r.x + r.w - 24.0 - self.font.width(v, 1.0), y + 6.0, 1.0, hex(TEXT));
        }
        if won {
            if self.button(m, Rect::new(cx - 146.0, r.y + 124.0, 140.0, 30.0), "KEEP GOING", 1, true) { self.continue_endless(); return; }
            if self.button(m, Rect::new(cx + 6.0, r.y + 124.0, 140.0, 30.0), "NEW RUN", 0, true) { self.state = State::Title; return; }
        } else if self.button(m, Rect::new(cx - 80.0, r.y + 128.0, 160.0, 30.0), "NEW RUN", 1, true) { self.state = State::Title; }
    }

    fn screen_pause(&mut self, m: &Mouse) {
        let r = self.window(240.0, 228.0, "PAUSED", (0x42308a, 0x7d68d4, 0x1e1545));
        let cx = r.x + r.w / 2.0;
        if self.button(m, Rect::new(cx - 80.0, r.y + 24.0, 160.0, 30.0), "RESUME", 1, true) { self.paused = false; }
        if self.button(m, Rect::new(cx - 80.0, r.y + 62.0, 160.0, 24.0), "RUN INFO", 0, true) { self.show_info = true; }
        let step = |v: f32| if v >= 0.95 { 0.0 } else { (v + 0.2).min(1.0) };
        let sv = format!("SOUND {}%", (self.sfx.volume * 100.0).round());
        if self.button(m, Rect::new(cx - 80.0, r.y + 94.0, 160.0, 24.0), &sv, 0, true) {
            self.sfx.volume = step(self.sfx.volume);
            self.meta.sfx_vol = self.sfx.volume;
            self.save_meta();
        }
        let mv = format!("MUSIC {}%", (self.music.volume * 100.0).round());
        if self.button(m, Rect::new(cx - 80.0, r.y + 126.0, 160.0, 24.0), &mv, 0, true) {
            self.music.volume = step(self.music.volume);
            self.meta.music_vol = self.music.volume;
            self.save_meta();
        }
        if self.button(m, Rect::new(cx - 80.0, r.y + 158.0, 160.0, 24.0), "ABANDON RUN", 0, true) {
            self.paused = false;
            self.physics.balls.clear();
            self.state = State::Title;
        }
        self.font.draw_outlined_centered("ESC TO RESUME", cx, r.y + r.h - 18.0, 1.0, hex(0x7d68d4));
    }

    /// shot levels, enhancements and vouchers
    fn screen_info(&mut self, m: &Mouse) {
        let r = self.window(420.0, 420.0, "RUN INFO", (0x12669a, 0x49c6ec, 0x0a2340));
        let x = r.x + 16.0;
        let mut y = r.y + 24.0;
        for (i, h) in ["SHOT", "LV", "POINTS", "MULT"].iter().enumerate() {
            self.font.draw_outlined(h, x + [6.0, 140.0, 190.0, 260.0][i], y, 1.0, hex(DIM));
        }
        y += 12.0;
        for s in SCORING {
            let (p, mm) = self.run.shot_value(s);
            let lv = self.run.level(s);
            self.skin.plate(Rect::new(x, y, r.w - 32.0, 15.0), if lv > 1 { 0x2e2063 } else { 0x1b1830 }, 0x2e2b47, OUTLINE);
            self.font.draw(shot_def(s).name, x + 6.0, y + 4.0, 1.0, hex(TEXT));
            self.font.draw_outlined(&format!("{lv}"), x + 140.0, y + 4.0, 1.0, hex(GOLD));
            self.font.draw_outlined(&num(p), x + 190.0, y + 4.0, 1.0, hex(PTS));
            self.font.draw_outlined(&format!("+{}", fmt(mm)), x + 260.0, y + 4.0, 1.0, hex(PINK));
            y += 17.0;
        }
        y += 8.0;
        self.font.draw_outlined("ENHANCED PIECES", x, y, 1.0, hex(DIM));
        y += 12.0;
        let enh: Vec<String> = PIECES.iter().filter_map(|p| self.run.enh.get(p.0).map(|e| format!("[k]{}[/] {}", enh_name(*e), p.2))).collect();
        if enh.is_empty() { self.font.draw("NONE YET", x, y, 1.0, hex(0x6f6d8c)); y += 10.0; } else { y += self.rich(&enh.join(",  "), x, y, r.w - 32.0, 1.0); }
        y += 6.0;
        self.font.draw_outlined("VOUCHERS", x, y, 1.0, hex(DIM));
        y += 12.0;
        let vs: Vec<&str> = self.run.vouchers.iter().map(|v| self.db.vouchers[*v].name).collect();
        if vs.is_empty() { self.font.draw("NONE YET", x, y, 1.0, hex(0x6f6d8c)); } else { self.rich(&vs.join(",  "), x, y, r.w - 32.0, 1.0); }
        if self.button(m, Rect::new(r.x + r.w / 2.0 - 60.0, r.y + r.h - 38.0, 120.0, 28.0), "BACK", 1, true) { self.show_info = false; }
    }
}

pub fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i..].starts_with('[') { if let Some(e) = s[i..].find(']') { if e <= 2 { i += e + 1; continue; } } }
        let c = s[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    out
}
