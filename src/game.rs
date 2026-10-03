//! Game flow: title → table select → play → cash out → shop → … plus the physics loop and input.

use crate::assets::{Kit, TableArt};
use crate::audio::Sfx;
use crate::content::*;
use crate::font::Font;
use crate::physics::{Physics, STEP};
use crate::rules::{Out, Rules};
use crate::run::*;
use crate::tablegen::{generate, GenParams};
use crate::sim::Factory;
use crate::table::Layout;
use macroquad::prelude::*;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State { Title, Select, Play, Ending, Cashout, Shop, Over, Win }

pub struct Popup { pub text: String, pub color: Color, pub pos: Vec2, pub world: Option<(f64, f64)>, pub age: f32, pub big: bool }
pub struct Spark { pub pos: Vec2, pub vel: Vec2, pub life: f32, pub color: Color }
pub struct Banner { pub text: String, pub color: Color, pub until: f64 }

pub struct TableSession { pub info: TableInfo, pub score: f64, pub balls: i32, pub balls_left: i32, pub flags: BossFlags }
#[derive(Default, Clone, Copy)]
pub struct BossFlags { pub mute: bool, pub mirror: bool, pub fog: bool, pub tax: bool, pub jitter: bool, pub miser: bool }

pub struct ShopOffer { pub kind: OfferKind, pub price: i64, pub sold: bool }
pub enum OfferKind { Item(usize, Option<Edition>), Card(usize) }
pub struct Shop { pub offers: Vec<ShopOffer>, pub voucher: Option<usize>, pub voucher_price: i64, pub voucher_sold: bool, pub rerolls: i64 }

pub struct EndSeq { pub fx: VecDeque<Fx>, pub timer: f32, pub show: (f64, f64), pub total_shown: bool, pub done_at: f32 }

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Meta {
    pub best_ante: u32, pub wins: u32, pub runs: u32, pub bosses: u32,
    #[serde(default = "default_vol")] pub sfx_vol: f32,
    #[serde(default = "default_vol")] pub music_vol: f32,
}
fn default_vol() -> f32 { 0.6 }

pub struct Game {
    pub layout: Layout,
    pub db: Db,
    pub run: Run,
    pub physics: Physics,
    pub rules: Rules,
    pub art: TableArt,
    /// the Blender parts kit at 1.0 and 0.8 px/mm
    pub kits: [Kit; 2],
    /// generates and bot-tests upcoming tables in the background
    pub factory: Factory,
    /// small renders of the tables on offer, for the select screen
    pub previews: HashMap<(u32, u32), Texture2D>,
    pub font: Font,
    pub sfx: Sfx,
    pub card_art: HashMap<String, Texture2D>,
    pub state: State,
    pub paused: bool,
    pub time: f64,
    pub acc: f64,
    pub alpha: f64,
    pub table: Option<TableSession>,
    pub ball: Ball,
    pub mods: Mods,
    pub shop: Option<Shop>,
    pub end: Option<EndSeq>,
    pub cash_lines: Vec<(String, i64)>,
    pub popups: Vec<Popup>,
    pub sparks: Vec<Spark>,
    pub banner: Option<Banner>,
    pub toasts: Vec<(String, f64)>,
    pub hits: HashMap<String, f64>,
    pub shake: f32,
    pub scroll: f32,
    pub meta: Meta,
    pub pick_cabinet: usize,
    pub item_scroll: f32,
    pub card_scroll: f32,
    pub drag: Option<usize>,
    pub show_fps: bool,
    pub jitter_at: f64,
    pub drop_anim: Vec<f32>,
    plunger_held: bool,
    /// a made ramp holds that side's flipper up so the returning ball is caught: (since, side)
    pub ramp_catch: Option<(f64, i8)>,
    /// pressing flip during a catch drops the flipper for a moment, then fires it
    catch_dip: Option<(f64, i8)>,
    prev_btn: [bool; 2],
    pub skin: crate::skin::Skin,
    pub show_info: bool,
    /// recent scoring shots for the backbox readout: (text, colour, time)
    pub ticker: VecDeque<(String, u32, f64)>,
    pub music: crate::music::Music,
    prev_state: State,
    pub pad: crate::platform::Pad,
    /// A was pressed this frame: the screen's main button takes it
    pub pad_confirm: bool,
    stick_armed: bool,
    /// the title logo and its white silhouette (for the shine)
    pub logo: Option<(Texture2D, Texture2D)>,
    /// attract mode: a bot plays the table behind the title
    demo: (Vec<(f64, f64)>, crate::tablegen::Rng, f64),
}

impl Default for Meta {
    fn default() -> Meta { Meta { best_ante: 0, wins: 0, runs: 0, bosses: 0, sfx_vol: 0.6, music_vol: 0.6 } }
}


/// one frame of the loading screen: the logo, a progress bar and what's happening
async fn loading(font: &Font, logo: &Option<(Texture2D, Texture2D)>, frac: f32, label: &str) {
    clear_background(hex(0x0b0a14));
    let (w, h) = (screen_width(), screen_height());
    let k = ((w / 960.0).min(h / 540.0)).floor().max(1.0);
    let mut y = h * 0.28;
    if let Some((tex, _)) = logo {
        let (lw, lh) = (tex.width() * k, tex.height() * k);
        draw_texture_ex(tex, ((w - lw) / 2.0).round(), (h * 0.42 - lh).round().max(0.0), WHITE, DrawTextureParams { dest_size: Some(vec2(lw, lh)), ..Default::default() });
        y = h * 0.42 + 24.0 * k;
    }
    let bw = 300.0 * k;
    let bx = ((w - bw) / 2.0).round();
    draw_rectangle(bx - 2.0 * k, y - 2.0 * k, bw + 4.0 * k, 12.0 * k, hex(0x2e2063));
    draw_rectangle(bx, y, bw, 8.0 * k, hex(0x120c2b));
    let segs = 30;
    let lit = (frac.clamp(0.0, 1.0) * segs as f32).round() as i32;
    for i in 0..lit {
        let sx = bx + i as f32 * bw / segs as f32;
        draw_rectangle(sx + k, y + k, bw / segs as f32 - 2.0 * k, 6.0 * k, hex(if i % 2 == 0 { 0xff6aa8 } else { 0xe0337f }));
    }
    font.draw_outlined_centered(label, w / 2.0, y + 20.0 * k, 2.0 * k, hex(0xa596ec));
    next_frame().await;
}

pub fn hex(c: u32) -> Color { Color::from_rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255) }

impl Game {
    pub async fn new() -> Game {
        // the logo and font come first so the loading screen has something to show
        let font = Font::new();
        let logo = match crate::assets::read("logo.png").await {
            Some(bytes) => Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok().map(|img| {
                // trim the empty margins, and make an all-white copy for the shine sweep
                let (w, h) = (img.width as u32, img.height as u32);
                let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
                for y in 0..h { for x in 0..w { if img.get_pixel(x, y).a > 0.5 { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y); } } }
                let img = img.sub_image(Rect::new(x0 as f32, y0 as f32, (x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32));
                let mut white = img.clone();
                for y in 0..white.height as u32 { for x in 0..white.width as u32 { if white.get_pixel(x, y).a > 0.5 { white.set_pixel(x, y, WHITE); } } }
                let (a, b) = (Texture2D::from_image(&img), Texture2D::from_image(&white));
                a.set_filter(FilterMode::Nearest);
                b.set_filter(FilterMode::Nearest);
                (a, b)
            }),
            None => None,
        };
        let steps = 4.0 + crate::music::SONGS.len() as f32;
        loading(&font, &logo, 0.0, "WAXING THE PLAYFIELD").await;
        let db = Db::new();
        let run = Run::new(&db, 0);
        let meta: Meta = crate::platform::load("save.json").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut card_art = HashMap::new();
        // one painting per item and card, rendered by art/cards.py
        let ids: Vec<&str> = db.items.iter().map(|d| d.id).chain(db.cards.iter().map(|d| d.id)).collect();
        for id in ids {
            if let Some(bytes) = crate::assets::read(&format!("cards/{id}.png")).await {
                let t = Texture2D::from_file_with_format(&bytes, Some(ImageFormat::Png));
                t.set_filter(FilterMode::Nearest);
                card_art.insert(id.to_string(), t);
            }
        }
        loading(&font, &logo, 1.0 / steps, "POLISHING THE FLIPPERS").await;
        let kits = [Kit::load("s100").await, Kit::load("s80").await];
        loading(&font, &logo, 2.0 / steps, "BUILDING A MACHINE").await;
        let layout = generate(&GenParams { seed: fastrand::u64(..), ante: 1, boss: false });
        let physics = Physics::new(&layout);
        let rules = Rules::new(&layout);
        let art = TableArt::build(&layout, &kits, &font);
        let drops = layout.drops.len();
        loading(&font, &logo, 3.0 / steps, "TUNING THE SOUND CHIP").await;
        let mut sfx = Sfx::load().await;
        sfx.volume = meta.sfx_vol;
        // render the soundtrack, one song per step
        let mut music = crate::music::Music::new(meta.music_vol);
        for (i, name) in crate::music::SONGS.iter().enumerate() {
            loading(&font, &logo, (4.0 + i as f32) / steps, "COMPOSING THE SOUNDTRACK").await;
            music.load(name).await;
        }
        let mods = run.mods(&db);
        Game {
            art, kits, factory: Factory::new(), previews: HashMap::new(), font, sfx, card_art,
            layout, db, run, physics, rules, state: State::Title, paused: false, time: 0.0, acc: 0.0, alpha: 1.0,
            table: None, ball: Ball::default(), mods, shop: None, end: None, cash_lines: vec![],
            popups: vec![], sparks: vec![], banner: None, toasts: vec![], hits: HashMap::new(), shake: 0.0, scroll: 1055.0,
            meta, pick_cabinet: 0, item_scroll: 0.0, card_scroll: 0.0, drag: None, show_fps: false, jitter_at: 0.0, drop_anim: vec![0.0; drops], plunger_held: false, ramp_catch: None, catch_dip: None, prev_btn: [false; 2],
            skin: crate::skin::Skin::new(), show_info: false, ticker: VecDeque::new(), music, prev_state: State::Title, pad: Default::default(), pad_confirm: false, stick_armed: true, logo, demo: (vec![], crate::tablegen::Rng::new(7), 0.0),
        }
    }

    pub fn save_meta(&self) {
        crate::platform::save("save.json", &serde_json::to_vec(&self.meta).unwrap());
    }

    pub fn unlocked(&self, i: usize) -> bool {
        let m = &self.meta;
        match self.db.cabinets[i].id {
            "red" => m.best_ante >= 3,
            "blue" => m.bosses >= 1,
            "gold" => m.best_ante >= 5,
            "glass" => m.best_ante >= 4,
            "black" => m.wins >= 1,
            _ => true,
        }
    }

    pub fn toast(&mut self, s: impl Into<String>) { self.toasts.push((s.into(), get_time() + 2.4)); }
    pub fn banner(&mut self, s: impl Into<String>, c: u32) { self.banner = Some(Banner { text: s.into(), color: hex(c), until: get_time() + 1.4 }); }
    pub fn deny(&mut self, s: impl Into<String>) { self.sfx.play("deny"); self.toast(s); }

    // ------------------------------------------------------------------ flow
    pub fn start_run(&mut self) {
        self.run = Run::new(&self.db, self.pick_cabinet);
        self.meta.runs += 1;
        self.save_meta();
        self.mods = self.run.mods(&self.db);
        self.state = State::Select;
        self.physics.balls.clear();
        self.factory.clear();
        self.previews.clear();
    }

    /// the seed and kind of a table in this run
    pub fn table_params(&self, ante: u32, blind: u32) -> GenParams {
        let seed = self.run.seed ^ (ante as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (blind as u64 + 1).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
        GenParams { seed, ante, boss: blind == 2 }
    }

    /// swap in a new table: physics, rules and art all come from the layout
    pub fn set_table(&mut self, l: Layout) {
        self.physics = Physics::new(&l);
        self.rules = Rules::new(&l);
        self.art = TableArt::build(&l, &self.kits, &self.font);
        self.drop_anim = vec![0.0; l.drops.len()];
        self.layout = l;
        self.scroll = (self.art.proj.h as f32 - crate::draw::VIEW_H).max(0.0);
        self.ramp_catch = None;
        self.catch_dip = None;
    }

    /// keep the tables of the current ante coming, and their previews
    pub fn feed_factory(&mut self, ms: f64) {
        let ante = self.run.ante;
        for blind in self.run.blind..3 { let p = self.table_params(ante, blind); self.factory.want((ante, blind), p); }
        self.factory.work(ms);
        for blind in 0..3 {
            let key = (ante, blind);
            if self.previews.contains_key(&key) { continue; }
            if let Some(l) = self.factory.get(key) {
                let art = TableArt::build(l, &self.kits, &self.font);
                art.base.set_filter(FilterMode::Linear);
                self.previews.insert(key, art.base);
            }
        }
        self.previews.retain(|k, _| k.0 == ante);
    }

    pub fn skip_table(&mut self) {
        let tag = self.run.tag_for(self.run.ante, self.run.blind);
        match tag {
            Tag::Cash => self.run.money += 8,
            Tag::Chips => { for _ in 0..2 { let d = self.random_chip(); let c = self.run.new_card(d); self.run.cards.push(c); } }
            Tag::Rare => {
                let pool: Vec<usize> = (0..self.db.items.len()).filter(|i| self.db.items[*i].rarity == Rarity::Rare).collect();
                let it = self.run.new_item(pool[fastrand::usize(..pool.len())], None);
                self.run.items.push(it);
            }
            Tag::Polish => { let n = self.run.items.len(); if n > 0 { self.run.items[fastrand::usize(..n)].edition = Some(Edition::Foil); } }
            Tag::Level => {
                let top = self.run.stats.shot_hits.iter().max_by_key(|(_, v)| **v).map(|(k, _)| *k).unwrap_or(Shot::Ramp);
                *self.run.levels.entry(top).or_insert(1) += 2;
            }
            Tag::Economy => self.run.money += self.run.money.clamp(0, 30),
        }
        self.toast(format!("{}!", tag_info(tag).0));
        self.sfx.play("coin");
        self.run.blind += 1;
        self.mods = self.run.mods(&self.db);
    }

    pub fn play_table(&mut self) {
        let key = (self.run.ante, self.run.blind);
        let params = self.table_params(key.0, key.1);
        let layout = match self.factory.take(key) { Some(l) => l, None => self.factory.finish(key, params) };
        self.set_table(layout);
        self.mods = self.run.mods(&self.db);
        let info = table_info(&mut self.run, &self.db);
        let boss = info.boss.map(|b| self.db.bosses[b].id);
        let p = &mut self.physics;
        p.balls.clear();
        p.events.clear();
        p.gravity_scale = self.mods.gravity;
        p.drift = 0.0;
        p.ramp_blocked = false;
        p.flipper_power = self.mods.flipper_power;
        p.flipper_power_l = 1.0;
        p.flipper_power_r = 1.0;
        p.bumper_kick = 1.15 * self.mods.bumper_kick;
        p.sling_kick = 1.25;
        p.flipper_return = 1.0;
        let mut flags = BossFlags::default();
        let mut fewer = 0;
        match boss {
            Some(Boss::Lead) => p.gravity_scale *= 1.3,
            Some(Boss::Mute) => flags.mute = true,
            Some(Boss::Limp) => p.flipper_power_l = 0.55,
            Some(Boss::Mirror) => flags.mirror = true,
            Some(Boss::Drift) => p.drift = -0.035,
            Some(Boss::Short) => fewer = 1,
            Some(Boss::Barricade) => p.ramp_blocked = true,
            Some(Boss::Fog) => flags.fog = true,
            Some(Boss::Slick) => { p.bumper_kick *= 2.0; p.sling_kick *= 1.6; }
            Some(Boss::Tax) => flags.tax = true,
            Some(Boss::Sticky) => p.flipper_return = 0.35,
            Some(Boss::Jitter) => flags.jitter = true,
            Some(Boss::Miser) => flags.miser = true,
            Some(Boss::Crown) => p.gravity_scale *= 1.25,
            _ => {}
        }
        self.rules.reset_table(&mut self.physics);
        let balls = (self.run.balls_per_table + self.mods.balls - fewer).max(1);
        let c = info.boss.map(|b| self.db.bosses[b].color).unwrap_or(0x49c6ec);
        self.banner(info.name.to_uppercase(), c);
        self.table = Some(TableSession { info, score: 0.0, balls, balls_left: balls, flags });
        for it in self.run.items.iter_mut() { it.used = false; }
        self.state = State::Play;
        self.time = 0.0;
        self.jitter_at = 4.0;
        self.serve_ball();
    }

    pub fn serve_ball(&mut self) {
        let t = self.table.as_mut().unwrap();
        let first = t.balls_left == t.balls;
        t.balls_left -= 1;
        self.ball = Ball { mult: 1.0, is_last: t.balls_left == 0, is_first: first, start: self.time, ..Default::default() };
        self.rules.start_ball();
        if self.mods.ball_save > 0.0 { self.rules.ball_save(self.mods.ball_save); }
        self.physics.serve_ball();
        let mut ctx = self.ctx();
        ctx.trigger(|on| matches!(on, On::BallStart));
        let fx = std::mem::take(&mut ctx.fx);
        drop(ctx);
        self.show_fx(fx);
    }

    fn ctx(&mut self) -> Ctx<'_> {
        let flags = self.table.as_ref().map(|t| t.flags).unwrap_or_default();
        Ctx {
            db: &self.db, run: &mut self.run, ball: &mut self.ball, fx: vec![], at: None, src: None,
            muted_bumpers: flags.mute, miser: flags.miser, destroyed: vec![], extra_balls: 0, saved: false, first_drain: false,
        }
    }

    fn on_drain(&mut self, ball_id: u32) {
        if self.state != State::Play { return; }
        let others = self.physics.balls.iter().filter(|b| b.alive && b.id != ball_id).count();
        if self.rules.save_timer > 0.0 && !self.rules.tilted {
            self.rules.pending_launch += 1;
            self.banner("BALL SAVED", 0xff8a7a);
            self.sfx.play("save");
            return;
        }
        if self.table.as_ref().unwrap().flags.tax { self.run.money -= 1; self.toast("The Tax: -$1"); }
        if others > 0 || self.rules.pending_launch > 0 || self.rules.launch_at > 0.0 { self.sfx.play("drain"); return; }
        if !self.rules.tilted {
            let mut ctx = self.ctx();
            ctx.first_drain = true;
            ctx.trigger(|on| matches!(on, On::Drain));
            let saved = ctx.saved;
            drop(ctx);
            if saved {
                self.rules.pending_launch += 1;
                self.banner("LUCKY SAVE", 0x3fc46a);
                self.sfx.play("save");
                return;
            }
        }
        self.sfx.play("drain");
        self.end_ball();
    }

    fn end_ball(&mut self) {
        self.state = State::Ending;
        self.physics.set_side(-1, false);
        self.physics.set_side(1, false);
        self.ramp_catch = None;
        if self.rules.tilted { self.ball.tilted = true; }
        let before = (self.ball.points, self.ball.mult);
        let now = self.time;
        let mut ctx = self.ctx();
        ctx.end_ball(now);
        let fx = std::mem::take(&mut ctx.fx);
        drop(ctx);
        self.end = Some(EndSeq { fx: fx.into(), timer: 0.5, show: before, total_shown: false, done_at: 0.0 });
    }

    fn finish_ball(&mut self) {
        let score = self.ball.score;
        let t = self.table.as_mut().unwrap();
        t.score += score;
        self.run.stats.best_ball = self.run.stats.best_ball.max(score);
        let (cleared, left) = (t.score >= t.info.target, t.balls_left);
        self.end = None;
        if cleared { self.table_cleared(); } else if left > 0 { self.state = State::Play; self.serve_ball(); } else { self.game_over(); }
    }

    fn table_cleared(&mut self) {
        self.state = State::Cashout;
        self.physics.balls.clear();
        self.sfx.play("win");
        let t = self.table.as_ref().unwrap();
        let mut lines = vec![(format!("{} cleared", t.info.name), t.info.reward)];
        if t.balls_left > 0 { lines.push((format!("Unused balls x{}", t.balls_left), t.balls_left as i64 * (1 + self.mods.spare_change))); }
        let before = self.run.money;
        let mut ctx = self.ctx();
        ctx.trigger(|on| matches!(on, On::TableEnd));
        let destroyed = std::mem::take(&mut ctx.destroyed);
        drop(ctx);
        let item_cash = self.run.money - before;
        if item_cash != 0 { lines.push((format!("Item payouts +${item_cash} (paid)"), 0)); }
        for uid in destroyed {
            if let Some(pos) = self.run.items.iter().position(|i| i.uid == uid) {
                let name = self.db.items[self.run.items[pos].def].name;
                self.run.items.remove(pos);
                self.toast(format!("{name} shattered!"));
                self.sfx.play("shatter");
            }
        }
        let interest = (self.run.money.max(0) / 5).min(self.mods.interest_cap);
        if interest > 0 { lines.push((format!("Interest (max ${})", self.mods.interest_cap), interest)); }
        self.run.stats.tables += 1;
        if self.run.blind == 2 { self.run.stats.bosses += 1; self.meta.bosses += 1; }
        self.meta.best_ante = self.meta.best_ante.max(self.run.ante + if self.run.blind == 2 { 1 } else { 0 });
        self.save_meta();
        self.cash_lines = lines;
    }

    pub fn cash_out(&mut self) {
        let total: i64 = self.cash_lines.iter().map(|l| l.1).sum();
        self.run.money += total;
        self.sfx.play("coin");
        if self.run.blind == 2 && self.run.ante == 8 && !self.run.endless {
            self.meta.wins += 1;
            self.save_meta();
            self.state = State::Win;
        } else {
            self.to_shop();
        }
    }

    pub fn to_shop(&mut self) {
        self.state = State::Shop;
        self.mods = self.run.mods(&self.db);
        self.shop = Some(self.make_shop());
    }

    pub fn continue_endless(&mut self) { self.run.endless = true; self.to_shop(); }

    pub fn next_table(&mut self) {
        self.run.blind += 1;
        if self.run.blind > 2 { self.run.blind = 0; self.run.ante += 1; }
        self.state = State::Select;
    }

    fn game_over(&mut self) {
        self.state = State::Over;
        self.physics.balls.clear();
        self.sfx.play("lose");
        self.save_meta();
    }

    // ------------------------------------------------------------------ shop
    pub fn random_chip(&self) -> usize {
        let chips: Vec<usize> = (0..self.db.cards.len()).filter(|i| self.db.cards[*i].chip).collect();
        chips[fastrand::usize(..chips.len())]
    }
    fn random_item(&self, exclude: &[usize]) -> usize {
        let roll = fastrand::f64() * 100.0;
        let rarity = if roll < 5.0 { Rarity::Rare } else if roll < 30.0 { Rarity::Uncommon } else { Rarity::Common };
        let owned: Vec<usize> = self.run.items.iter().map(|i| i.def).collect();
        let mut pool: Vec<usize> = (0..self.db.items.len()).filter(|i| self.db.items[*i].rarity == rarity && !exclude.contains(i) && !owned.contains(i)).collect();
        if pool.is_empty() { pool = (0..self.db.items.len()).filter(|i| !exclude.contains(i)).collect(); }
        pool[fastrand::usize(..pool.len())]
    }
    fn make_shop(&mut self) -> Shop {
        let mut offers = vec![];
        let mut picked = vec![];
        for _ in 0..self.run.shop_items {
            let d = self.random_item(&picked);
            picked.push(d);
            let r = fastrand::f64();
            let ed = if r < 0.04 { Some(Edition::Poly) } else if r < 0.1 { Some(Edition::Holo) } else if r < 0.18 { Some(Edition::Foil) } else { None };
            offers.push(ShopOffer { kind: OfferKind::Item(d, ed), price: self.run.item_price(&self.db, d, ed), sold: false });
        }
        let mut picked_cards: Vec<usize> = vec![];
        for _ in 0..self.run.shop_cards {
            // no two of the same card on one shelf
            let mut d = 0;
            for _ in 0..24 {
                d = if fastrand::f64() < 0.5 { self.random_chip() } else {
                    let charms: Vec<usize> = (0..self.db.cards.len()).filter(|i| !self.db.cards[*i].chip).collect();
                    charms[fastrand::usize(..charms.len())]
                };
                if !picked_cards.contains(&d) { break; }
            }
            picked_cards.push(d);
            offers.push(ShopOffer { kind: OfferKind::Card(d), price: self.run.card_price(&self.db, d), sold: false });
        }
        if self.run.voucher_offer.map(|v| v.0) != Some(self.run.ante) {
            let pool: Vec<usize> = (0..self.db.vouchers.len()).filter(|v| !self.run.vouchers.contains(v)).collect();
            self.run.voucher_offer = if pool.is_empty() { None } else { Some((self.run.ante, pool[fastrand::usize(..pool.len())])) };
        }
        let voucher = self.run.voucher_offer.map(|v| v.1).filter(|v| !self.run.vouchers.contains(v));
        let voucher_price = voucher.map(|v| ((self.db.vouchers[v].cost as f64) * self.run.discount).round() as i64).unwrap_or(0);
        Shop { offers, voucher, voucher_price, voucher_sold: false, rerolls: 0 }
    }
    pub fn reroll_cost(&self) -> i64 { (self.run.reroll_base + self.shop.as_ref().map(|s| s.rerolls).unwrap_or(0)).max(0) }
    pub fn reroll(&mut self) {
        let cost = self.reroll_cost();
        if self.run.money < cost { return self.deny(format!("Need ${cost}")); }
        self.run.money -= cost;
        let rerolls = self.shop.as_ref().unwrap().rerolls + 1;
        let voucher = self.shop.as_ref().map(|s| (s.voucher, s.voucher_price, s.voucher_sold)).unwrap();
        let mut s = self.make_shop();
        s.rerolls = rerolls;
        s.voucher = voucher.0;
        s.voucher_price = voucher.1;
        s.voucher_sold = voucher.2;
        self.shop = Some(s);
        self.sfx.play("ui");
    }
    pub fn buy(&mut self, i: usize, use_now: bool) {
        let Some(shop) = self.shop.as_mut() else { return };
        let o = &shop.offers[i];
        if o.sold { return; }
        let price = o.price;
        if self.run.money < price { return self.deny(format!("Need ${price}")); }
        match o.kind {
            OfferKind::Item(d, ed) => {
                self.run.money -= price;
                let it = self.run.new_item(d, ed);
                self.run.items.push(it);
                self.run.stats.bought += 1;
            }
            OfferKind::Card(d) => {
                if use_now {
                    if self.db.cards[d].play_only { return self.deny("That card is used during play"); }
                    self.run.money -= price;
                    if !self.apply_card(d) { self.run.money += price; return self.deny("It would do nothing right now"); }
                } else {
                    self.run.money -= price;
                    let c = self.run.new_card(d);
                    self.run.cards.push(c);
                }
            }
        }
        self.shop.as_mut().unwrap().offers[i].sold = true;
        self.sfx.play("buy");
        self.after_inventory();
    }
    pub fn buy_voucher(&mut self) {
        let Some(shop) = self.shop.as_ref() else { return };
        let Some(v) = shop.voucher else { return };
        if shop.voucher_sold { return; }
        let price = shop.voucher_price;
        if self.run.money < price { return self.deny(format!("Need ${price}")); }
        self.run.money -= price;
        match self.db.vouchers[v].eff {
            VEff::ShopItems(n) => self.run.shop_items += n,
            VEff::Balls(n) => self.run.balls_per_table += n,
            VEff::RerollBase(n) => self.run.reroll_base += n,
            VEff::InterestCap(n) => self.run.interest_cap += n,
            VEff::Flipper(x) => self.run.flipper_power *= x,
            VEff::BallSave(s) => self.run.ball_save = self.run.ball_save.max(s),
            VEff::Discount(x) => self.run.discount *= x,
            VEff::ShopCards(n) => self.run.shop_cards += n,
            VEff::TiltTolerance(x) => self.run.tilt_tol *= x,
            VEff::Luck(n) => self.run.luck *= n,
            VEff::StartCash(n) => self.run.money += n,
            VEff::PriceFreeze => self.run.price_freeze = true,
        }
        self.run.vouchers.push(v);
        self.shop.as_mut().unwrap().voucher_sold = true;
        self.sfx.play("buy");
        self.toast(format!("{} redeemed", self.db.vouchers[v].name));
        self.after_inventory();
    }
    pub fn sell_item(&mut self, idx: usize) {
        if idx >= self.run.items.len() { return; }
        let v = self.run.sell_value(&self.db, &self.run.items[idx]);
        self.run.items.remove(idx);
        self.run.money += v;
        self.sfx.play("sell");
        self.after_inventory();
    }
    pub fn sell_card(&mut self, idx: usize) {
        if idx >= self.run.cards.len() { return; }
        self.run.cards.remove(idx);
        self.run.money += 1;
        self.sfx.play("sell");
    }
    pub fn use_card(&mut self, idx: usize) {
        if idx >= self.run.cards.len() { return; }
        let d = self.run.cards[idx].def;
        if self.db.cards[d].play_only && self.state != State::Play { return self.deny("Use this during play"); }
        let c = self.run.cards.remove(idx);
        if !self.apply_card(d) { self.run.cards.insert(idx, c); return self.deny("It would do nothing right now"); }
        self.sfx.play("buy");
        self.after_inventory();
    }
    pub fn move_item(&mut self, from: usize, to: usize) {
        let n = self.run.items.len();
        if from >= n || to >= n || from == to { return; }
        let it = self.run.items.remove(from);
        self.run.items.insert(to, it);
        self.after_inventory();
    }
    fn after_inventory(&mut self) {
        self.mods = self.run.mods(&self.db);
        if let Some(shop) = self.shop.as_mut() {
            // prices follow the rack size and discounts
            for o in shop.offers.iter_mut() {
                if o.sold { continue; }
                o.price = match o.kind {
                    OfferKind::Item(d, ed) => self.run.item_price(&self.db, d, ed),
                    OfferKind::Card(d) => self.run.card_price(&self.db, d),
                };
            }
        }
    }

    fn apply_card(&mut self, d: usize) -> bool {
        let eff = self.db.cards[d].eff;
        match eff {
            CardEff::Level(shots, n) => {
                for s in shots { *self.run.levels.entry(*s).or_insert(1) += n; }
                self.toast(format!("{} leveled up", shots.iter().map(|s| shot_def(*s).name).collect::<Vec<_>>().join(" & ")));
            }
            CardEff::Enhance(e) => {
                let open: Vec<usize> = (0..PIECES.len()).filter(|i| self.run.enh.get(PIECES[*i].0) != Some(&e)).collect();
                if open.is_empty() { return false; }
                let p = PIECES[open[fastrand::usize(..open.len())]];
                self.run.enh.insert(p.0, e);
                self.toast(format!("{} is now {}", p.2, enh_name(e)));
            }
            CardEff::Multiball => {
                if self.state != State::Play { return false; }
                let mut out = vec![];
                self.rules.start_multiball(1, &mut out);
                self.ball.multiball = true;
                self.apply_out(out);
            }
            CardEff::SafetyNet => { if self.state != State::Play { return false; } self.rules.ball_save(15.0); }
            CardEff::Photocopy => {
                if self.run.items.is_empty() { return false; }
                let src = self.run.items[fastrand::usize(..self.run.items.len())].def;
                let it = self.run.new_item(src, None);
                self.toast(format!("Copied {}", self.db.items[src].name));
                self.run.items.push(it);
            }
            CardEff::Hermit => { let g = self.run.money.clamp(0, 20); self.run.money += g; self.toast(format!("+${g}")); }
            CardEff::PrismWheel => {
                let plain: Vec<usize> = (0..self.run.items.len()).filter(|i| self.run.items[*i].edition.is_none()).collect();
                if plain.is_empty() { return false; }
                if fastrand::f64() < 1.0 / 3.0 {
                    let i = plain[fastrand::usize(..plain.len())];
                    let ed = [Edition::Foil, Edition::Foil, Edition::Holo, Edition::Holo, Edition::Poly][fastrand::usize(..5)];
                    self.run.items[i].edition = Some(ed);
                    self.toast(format!("{} became {}", self.db.items[self.run.items[i].def].name, edition_name(ed)));
                } else { self.toast("Nope!"); }
            }
            CardEff::Recycler => { for _ in 0..2 { let d = self.random_chip(); let c = self.run.new_card(d); self.run.cards.push(c); } self.toast("Made 2 chips"); }
            CardEff::Overclock => {
                let s = SCORING[fastrand::usize(..SCORING.len())];
                *self.run.levels.entry(s).or_insert(1) += 3;
                self.toast(format!("{} +3 levels", shot_def(s).name));
            }
            CardEff::FortuneCookie => { let g = fastrand::i64(1..=10); self.run.money += g; self.toast(format!("+${g}")); }
            CardEff::Sledgehammer => {
                if self.run.items.is_empty() { return false; }
                let i = fastrand::usize(..self.run.items.len());
                let v = self.run.sell_value(&self.db, &self.run.items[i]) * 3;
                let name = self.db.items[self.run.items[i].def].name;
                self.run.items.remove(i);
                self.run.money += v;
                self.toast(format!("Smashed {name}: +${v}"));
            }
            CardEff::TuneUp => { self.run.flipper_power *= 1.05; self.toast("Flippers +5%"); }
        }
        true
    }

    // ------------------------------------------------------------------ per frame
    pub fn update(&mut self, dt: f64) {
        self.input();
        if matches!(self.state, State::Select | State::Shop | State::Cashout) { self.feed_factory(6.0); }
        if self.state == State::Title { self.attract(dt); }
        let live = matches!(self.state, State::Play | State::Ending) && !self.paused;
        if live {
            self.acc += dt;
            let mut n = 0;
            while self.acc >= STEP && n < 80 {
                self.physics.step();
                if !self.physics.events.is_empty() {
                    let evs = std::mem::take(&mut self.physics.events);
                    let mut out = vec![];
                    let wild = self.mods.wild_lanes;
                    let now = self.time;
                    for e in &evs { self.rules.handle(e, &self.physics, now, wild, &mut out); }
                    self.apply_out(out);
                }
                self.acc -= STEP;
                n += 1;
            }
            if n == 80 { self.acc = 0.0; }
            self.alpha = self.acc / STEP;
            self.time += dt;
            if self.state == State::Play {
                let mut out = vec![];
                let now = self.time;
                self.rules.update(&mut self.physics, now, dt, &mut out);
                self.apply_out(out);
                if self.table.as_ref().map(|t| t.flags.jitter).unwrap_or(false) && self.time > self.jitter_at {
                    self.jitter_at = self.time + 3.0 + fastrand::f64() * 3.0;
                    let dx = (fastrand::f64() - 0.5) * 0.5;
                    self.physics.nudge(dx, 0.1);
                    self.shake = 1.0;
                    self.sfx.play("nudge");
                }
            }
        }
        if self.state == State::Ending && !self.paused { self.update_end(dt as f32); }
        self.update_music();
        // effects
        for p in self.popups.iter_mut() { p.age += dt as f32; }
        self.popups.retain(|p| p.age < 1.1);
        for s in self.sparks.iter_mut() {
            s.life -= dt as f32;
            s.vel.y += 260.0 * dt as f32;
            s.pos += s.vel * dt as f32;
        }
        self.sparks.retain(|s| s.life > 0.0);
        self.shake = (self.shake - dt as f32 * 4.0).max(0.0);
        let now = get_time();
        self.toasts.retain(|t| t.1 > now);
        if self.banner.as_ref().map(|b| b.until < now).unwrap_or(false) { self.banner = None; }
    }

    /// attract mode: the bot plays the title table, bumpers and slings flash, the camera follows
    fn attract(&mut self, dt: f64) {
        if self.demo.0.len() != self.physics.flippers.len() { self.demo.0 = vec![(0.0, 0.0); self.physics.flippers.len()]; }
        if self.physics.balls.is_empty() && !self.physics.any_in_lane() {
            self.physics.serve_ball();
            self.demo.2 = self.physics.time + 0.8;
        }
        self.acc += dt;
        let mut n = 0;
        while self.acc >= STEP && n < 40 {
            let t = self.physics.time;
            if self.demo.2 > 0.0 && t > self.demo.2 {
                let s = 0.7 + self.demo.1.f() * 0.3;
                self.physics.release_plunger(Some(s));
                self.demo.2 = 0.0;
            }
            crate::sim::bot(&mut self.physics, &mut self.demo.0, t, &mut self.demo.1);
            self.physics.step();
            for e in std::mem::take(&mut self.physics.events) {
                match e {
                    crate::physics::Ev::Bumper { id, x, y } => self.apply_out(vec![Out::Hit { kind: "bumper", id, x, y }]),
                    crate::physics::Ev::Sling { id, x, y } => self.apply_out(vec![Out::Hit { kind: "sling", id, x, y }]),
                    crate::physics::Ev::Standup { id, x, y } => self.apply_out(vec![Out::Hit { kind: "standup", id, x, y }]),
                    _ => {}
                }
            }
            self.acc -= STEP;
            n += 1;
        }
        if n == 40 { self.acc = 0.0; }
        self.alpha = self.acc / STEP;
        self.time += dt;
        // a ball that stops moving in the demo is simply cleared
        let still = self.physics.balls.iter().all(|b| b.speed() < 0.02) && !self.physics.any_in_lane();
        if still && !self.physics.balls.is_empty() {
            self.demo.2 -= dt;
            if self.demo.2 < -3.0 { self.physics.balls.clear(); }
        }
    }

    /// the song for the current screen, and jingles when a table ends
    fn update_music(&mut self) {
        let now = get_time();
        if self.state != self.prev_state {
            match self.state {
                State::Cashout => self.music.jingle("cleared", now),
                State::Over => self.music.jingle("over", now),
                State::Win => self.music.jingle("win", now),
                _ => {}
            }
            self.prev_state = self.state;
        }
        let want = match self.state {
            State::Title | State::Select => Some("title"),
            State::Play | State::Ending => Some(match self.run.blind { 2 => "boss", 1 => "table_b", _ => "table_a" }),
            State::Shop | State::Cashout => Some("shop"),
            State::Over | State::Win => None,
        };
        self.music.set(want);
        self.music.update(now, self.paused);
    }

    fn update_end(&mut self, dt: f32) {
        let Some(end) = self.end.as_mut() else { return };
        end.timer -= dt;
        if end.timer > 0.0 { return; }
        if let Some(fx) = end.fx.pop_front() {
            end.show = (fx.points, fx.mult);
            end.timer = if fx.kind == FxKind::XMult { 0.42 } else { 0.3 };
            self.show_fx(vec![fx]);
            return;
        }
        if !end.total_shown {
            end.total_shown = true;
            end.show = (self.ball.points, self.ball.mult);
            end.timer = 1.1;
            self.sfx.play("score");
            return;
        }
        self.finish_ball();
    }

    pub fn apply_out(&mut self, out: Vec<Out>) {
        for o in out {
            match o {
                Out::Sound(s) => {
                    if s == "multiball" { self.music.sting("extra", get_time()); }
                    self.sfx.play(s);
                }
                Out::Banner(t, c) => self.banner(t, c),
                Out::Shake(s) => self.shake = self.shake.max(s),
                Out::Catch(side) => {
                    if self.ramp_catch.is_none() {
                        self.ramp_catch = Some((self.time, side));
                        self.toast("RAMP CATCH: FLIP TO SHOOT");
                    }
                }
                Out::Drain(b) => self.on_drain(b),
                Out::Hit { kind, id, x, y } => {
                    let key = format!("{kind}:{id}");
                    self.hits.insert(key, self.time);
                    let col = match kind { "bumper" => hex(0x49c6ec), "sling" => hex(0xff6aa8), "drop" => hex(0xffcc4d), "standup" => hex(0xff6aa8), _ => hex(0xfff1a8) };
                    let n = match kind { "bumper" => 14, "sling" => 10, "spinner" => 2, _ => 6 };
                    let at = self.art.proj.to_px(x, y, 0.02);
                    for _ in 0..n {
                        let a = fastrand::f32() * std::f32::consts::TAU;
                        let sp = 40.0 + fastrand::f32() * 110.0;
                        self.sparks.push(Spark { pos: at, vel: vec2(a.cos() * sp, a.sin() * sp - 60.0), life: 0.25 + fastrand::f32() * 0.35, color: col });
                    }
                }
                Out::Shot { shot, piece, x, y, quiet } => {
                    let now = self.time;
                    let mut ctx = self.ctx();
                    ctx.at = if x != 0.0 || y != 0.0 { Some((x, y)) } else { None };
                    ctx.hit(shot, piece, now);
                    let extra = ctx.extra_balls;
                    let fx = std::mem::take(&mut ctx.fx);
                    drop(ctx);
                    if extra > 0 {
                        if let Some(t) = self.table.as_mut() { t.balls_left += extra as i32; t.balls += extra as i32; }
                        self.banner("EXTRA BALL", 0x3fc46a);
                        self.music.sting("extra", get_time());
                    }
                    if !quiet {
                        let pts: f64 = fx.iter().filter(|f| f.kind == FxKind::Pts).map(|f| f.value).sum();
                        let mul: f64 = fx.iter().filter(|f| f.kind == FxKind::Mult).map(|f| f.value).sum();
                        let mut t = format!("{} {}", shot_def(shot).name.to_uppercase(), pts.round() as i64);
                        if mul > 0.0 { t += &format!(" +{}X", fmt(mul)); }
                        self.ticker.push_back((t, if mul > 0.0 { 0xff6aa8 } else { 0xff8a1c }, self.time));
                        while self.ticker.len() > 12 { self.ticker.pop_front(); }
                    }
                    let fx: Vec<Fx> = if quiet { fx.into_iter().filter(|f| f.kind != FxKind::Pts || f.src.is_some()).collect() } else { fx };
                    self.show_fx(fx);
                }
            }
        }
    }

    pub fn show_fx(&mut self, fx: Vec<Fx>) {
        for f in fx {
            let (text, color) = match f.kind {
                FxKind::Pts => (format!("+{}", f.value.round() as i64), hex(0x49c6ec)),
                FxKind::Mult => (format!("+{} MULT", fmt(f.value)), hex(0xff6aa8)),
                FxKind::XMult => (format!("×{} MULT", fmt(f.value)), hex(0xee4040)),
                FxKind::Cash => (format!("+${}", f.value as i64), hex(0xffcc4d)),
            };
            match f.kind {
                FxKind::Mult => self.sfx.play("mult"),
                FxKind::XMult => self.sfx.play("xmult"),
                FxKind::Cash => self.sfx.play("coin"),
                FxKind::Pts => {}
            }
            self.popups.push(Popup { text, color, pos: vec2(0.0, 0.0), world: f.at, age: 0.0, big: f.src.is_some() || f.kind == FxKind::XMult });
            if let Some(uid) = f.src { self.hits.insert(format!("item:{uid}"), self.time); }
            match f.kind { FxKind::Pts => { self.hits.insert("tally:p".into(), self.time); } FxKind::Mult | FxKind::XMult => { self.hits.insert("tally:m".into(), self.time); } _ => {} }
        }
    }

    // ------------------------------------------------------------------ input
    fn key_held(&self, codes: &[KeyCode], vks: &[u16]) -> bool {
        if codes.iter().any(|k| is_key_down(*k)) { return true; }
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetActiveWindow, GetAsyncKeyState};
            // read the real key state from Windows: the event stream can drop a Shift release
            if !GetActiveWindow().is_null() {
                return vks.iter().any(|vk| (GetAsyncKeyState(*vk as i32) as u16 & 0x8000) != 0);
            }
        }
        let _ = vks;
        false
    }

    fn input(&mut self) {
        use crate::platform::btn;
        self.pad.poll();
        self.pad_confirm = self.pad.pressed(btn::A) && !matches!(self.state, State::Play | State::Ending);
        if self.pad.pressed(btn::BACK) { self.show_info = !self.show_info; }
        if self.pad.pressed(btn::START) && matches!(self.state, State::Play | State::Ending) { self.paused = !self.paused; }
        if self.pad.pressed(btn::B) && (self.show_info || self.paused) { self.show_info = false; self.paused = false; }
        if is_key_pressed(KeyCode::F) { self.show_fps = !self.show_fps; }
        if is_key_pressed(KeyCode::Tab) { self.show_info = !self.show_info; }
        if self.show_info && is_key_pressed(KeyCode::Escape) { self.show_info = false; return; }
        if matches!(self.state, State::Play | State::Ending) && (is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::P)) {
            self.paused = !self.paused;
        }
        let playing = self.state == State::Play && !self.paused && !self.rules.tilted;
        // VK codes: Z=0x5A, LSHIFT=0xA0, LEFT=0x25, OEM_2('/')=0xBF, M=0x4D, RSHIFT=0xA1, RIGHT=0x27
        let pad_l = self.pad.held(btn::LB) || self.pad.held(btn::LT) || self.pad.held(btn::LEFT);
        let pad_r = self.pad.held(btn::RB) || self.pad.held(btn::RT) || self.pad.held(btn::RIGHT);
        let l = playing && (pad_l || self.key_held(&[KeyCode::Z, KeyCode::Left], &[0x5A, 0xA0, 0x25]));
        let r = playing && (pad_r || self.key_held(&[KeyCode::Slash, KeyCode::M, KeyCode::Right], &[0xBF, 0x4D, 0xA1, 0x27]));
        let mirror = self.table.as_ref().map(|t| t.flags.mirror).unwrap_or(false);
        let (l, r) = if mirror { (r, l) } else { (l, r) };
        self.physics.set_side(-1, l);
        self.physics.set_side(1, r);
        // ramp catch: that side's main flipper stays up until the player flips, or the ball is clearly elsewhere.
        // A press during the catch drops the flipper for a moment and fires it again, so it shoots the ball.
        let pressed = [l && !self.prev_btn[0], r && !self.prev_btn[1]];
        self.prev_btn = [l, r];
        if let Some((t0, side)) = self.ramp_catch {
            let si = if side < 0 { 0 } else { 1 };
            let near = self.physics.balls.iter().any(|b| b.x * side as f64 > 0.0 && b.y < 0.27);
            if pressed[si] { self.catch_dip = Some((self.time + 0.1, side)); }
            if !playing || pressed[si] || (self.time - t0 > 3.0 && !near) { self.ramp_catch = None; }
        }
        let main_of = |g: &Game, side: i8| g.layout.flippers.iter().position(|f| f.side == side && f.main);
        if let Some((_, side)) = self.ramp_catch {
            if let Some(i) = main_of(self, side) { self.physics.set_flipper(i, true); }
        }
        if let Some((until, side)) = self.catch_dip {
            if self.time < until {
                if let Some(i) = main_of(self, side) { self.physics.set_flipper(i, false); }
            } else {
                self.catch_dip = None;
            }
        }
        let p = playing && (self.pad.held(btn::A) || self.key_held(&[KeyCode::Space, KeyCode::Enter, KeyCode::Down], &[]));
        if p != self.plunger_held {
            self.plunger_held = p;
            if p { if self.physics.any_in_lane() { self.physics.pull_plunger(true); } } else { self.physics.pull_plunger(false); }
        }
        if playing {
            let tol = self.run.tilt_tol;
            let no_tilt = self.mods.no_tilt;
            let mut out = vec![];
            if is_key_pressed(KeyCode::A) { self.rules.nudge(&mut self.physics, -0.28, 0.05, no_tilt, tol, &mut out); }
            if is_key_pressed(KeyCode::D) { self.rules.nudge(&mut self.physics, 0.28, 0.05, no_tilt, tol, &mut out); }
            if is_key_pressed(KeyCode::W) { self.rules.nudge(&mut self.physics, 0.0, 0.3, no_tilt, tol, &mut out); }
            // a flick of the stick nudges; it has to come back to centre before the next one
            let (sx, sy) = (self.pad.now.lx, self.pad.now.ly);
            if self.stick_armed && (sx.abs() > 0.8 || sy < -0.8) {
                self.stick_armed = false;
                if sx < -0.8 { self.rules.nudge(&mut self.physics, -0.28, 0.05, no_tilt, tol, &mut out); }
                else if sx > 0.8 { self.rules.nudge(&mut self.physics, 0.28, 0.05, no_tilt, tol, &mut out); }
                else { self.rules.nudge(&mut self.physics, 0.0, 0.3, no_tilt, tol, &mut out); }
            }
            if sx.abs() < 0.3 && sy.abs() < 0.3 { self.stick_armed = true; }
            if self.pad.pressed(btn::Y) { self.use_card(0); }
            if self.rules.tilted { self.ball.tilted = true; }
            self.apply_out(out);
            let digits = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9];
            for (i, k) in digits.iter().enumerate() { if is_key_pressed(*k) { self.use_card(i); } }
        }
    }

    /// drop targets sink fast when hit and pop back up on reset
    pub fn update_drops(&mut self, dt: f32) {
        for (i, d) in self.physics.drops.iter().enumerate() {
            let target = if d.down { 1.0 } else { 0.0 };
            let rate = if d.down { 30.0 } else { 14.0 };
            let a = &mut self.drop_anim[i];
            *a += (target - *a) * (dt * rate).min(1.0);
        }
    }

    pub fn light_level(&self, id: &str, t: f64) -> f32 {
        if let Some(rest) = id.strip_prefix("drop:") {
            let mut it = rest.split(':').filter_map(|v| v.parse::<usize>().ok());
            if let (Some(g), Some(k)) = (it.next(), it.next()) {
                if let Some(&i) = self.layout.features.banks.get(g).and_then(|b| b.0.get(k)) {
                    if let Some(until) = self.rules.flash.get(id) { if self.time < *until { return if (t * 50.0).sin() > 0.0 { 1.0 } else { 0.15 }; } }
                    return if self.physics.drops[i].down { 1.0 } else if (t * 9.4).sin() > 0.0 { 0.4 } else { 0.06 };
                }
            }
        }
        self.rules.light(id, t, self.time)
    }
}

pub fn fmt(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else if v < 10.0 { format!("{:.2}", v).trim_end_matches('0').trim_end_matches('.').to_string() } else { format!("{:.1}", v).trim_end_matches('0').trim_end_matches('.').to_string() }
}
pub fn num(v: f64) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}
