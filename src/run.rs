//! Run state (antes, money, inventory, upgrades) and the scorer that evaluates item rules.

use crate::content::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edition { Foil, Holo, Poly }
pub fn edition_name(e: Edition) -> &'static str { match e { Edition::Foil => "Foil", Edition::Holo => "Holographic", Edition::Poly => "Polychrome" } }
pub fn edition_desc(e: Edition) -> &'static str {
    match e { Edition::Foil => "[p]+50 Points[/] at ball end", Edition::Holo => "[m]+10 Mult[/] at ball end", Edition::Poly => "[x]×1.5 Mult[/] at ball end" }
}

#[derive(Clone)]
pub struct ItemInst { pub uid: u32, pub def: usize, pub stat: f64, pub edition: Option<Edition>, pub used: bool }
#[derive(Clone)]
pub struct CardInst { pub uid: u32, pub def: usize }

/// enhancements apply to a kind of piece, so they carry over from one generated table to the next
pub const PIECES: [(&str, Shot, &str); 4] = [
    ("bumper", Shot::Bumper, "Bumpers"), ("drop", Shot::Drop, "Drop Targets"),
    ("sling", Shot::Sling, "Slingshots"), ("standup", Shot::Standup, "Standups"),
];

#[derive(Default)]
pub struct Stats { pub shot_hits: HashMap<Shot, u64>, pub best_ball: f64, pub tables: u32, pub bosses: u32, pub bought: u32, pub piece_hits: HashMap<&'static str, u32> }

pub struct Db { pub items: Vec<ItemDef>, pub cards: Vec<CardDef>, pub vouchers: Vec<VoucherDef>, pub bosses: Vec<BossDef>, pub cabinets: Vec<CabinetDef> }
impl Db {
    pub fn new() -> Db { Db { items: items(), cards: cards(), vouchers: vouchers(), bosses: bosses(), cabinets: cabinets() } }
    pub fn item(&self, id: &str) -> usize { self.items.iter().position(|d| d.id == id).unwrap() }
    pub fn card(&self, id: &str) -> usize { self.cards.iter().position(|d| d.id == id).unwrap() }
}

pub struct Run {
    pub cabinet: usize,
    pub ante: u32,
    pub blind: u32,
    pub money: i64,
    pub items: Vec<ItemInst>,
    pub cards: Vec<CardInst>,
    pub levels: HashMap<Shot, u32>,
    pub enh: HashMap<&'static str, Enh>,
    pub vouchers: Vec<usize>,
    pub balls_per_table: i32,
    pub reroll_base: i64,
    pub interest_cap: i64,
    pub shop_items: u32,
    pub shop_cards: u32,
    pub discount: f64,
    pub flipper_power: f64,
    pub ball_save: f64,
    pub tilt_tol: f64,
    pub luck: u32,
    pub price_freeze: bool,
    pub item_rebate: i64,
    pub bosses: HashMap<u32, usize>,
    pub tags: HashMap<(u32, u32), Tag>,
    pub voucher_offer: Option<(u32, usize)>,
    pub endless: bool,
    /// every table in the run is generated from this
    pub seed: u64,
    pub stats: Stats,
    next_uid: u32,
}

#[derive(Default, Clone)]
pub struct Mods {
    pub balls: i32, pub flipper_power: f64, pub bumper_kick: f64, pub ball_save: f64, pub interest_cap: i64,
    pub no_tilt: bool, pub wild_lanes: bool, pub spare_change: i64, pub discount: f64, pub gravity: f64,
}

impl Run {
    pub fn new(db: &Db, cabinet: usize) -> Run {
        let mut r = Run {
            cabinet, ante: 1, blind: 0, money: 4, items: vec![], cards: vec![], levels: HashMap::new(), enh: HashMap::new(),
            vouchers: vec![], balls_per_table: 3, reroll_base: 5, interest_cap: 5, shop_items: 2, shop_cards: 2, discount: 1.0,
            flipper_power: 1.0, ball_save: 0.0, tilt_tol: 1.0, luck: 1, price_freeze: false, item_rebate: 0,
            bosses: HashMap::new(), tags: HashMap::new(), voucher_offer: None, endless: false, stats: Stats::default(), next_uid: 1, seed: fastrand::u64(..),
        };
        match db.cabinets[cabinet].id {
            "red" => r.balls_per_table += 1,
            "blue" => { let a = r.new_card(db.card("safety_net")); let b = r.new_card(db.card("multiball")); r.cards.push(a); r.cards.push(b); }
            "gold" => { r.money = 15; r.interest_cap = 10; }
            "glass" => { r.enh.insert("bumper", Enh::Glass); }
            "black" => { r.item_rebate = 1; r.balls_per_table -= 1; }
            _ => {}
        }
        r
    }
    pub fn uid(&mut self) -> u32 { self.next_uid += 1; self.next_uid }
    pub fn new_item(&mut self, def: usize, edition: Option<Edition>) -> ItemInst { ItemInst { uid: self.uid(), def, stat: 0.0, edition, used: false } }
    pub fn new_card(&mut self, def: usize) -> CardInst { CardInst { uid: self.uid(), def } }
    pub fn level(&self, s: Shot) -> u32 { *self.levels.get(&s).unwrap_or(&1) }
    pub fn shot_value(&self, s: Shot) -> (f64, f64) {
        let d = shot_def(s);
        let lv = (self.level(s) - 1) as f64;
        (d.pts + d.lp * lv, d.mult + d.lm * lv)
    }
    pub fn sell_value(&self, db: &Db, it: &ItemInst) -> i64 {
        (db.items[it.def].cost / 2).max(1) + if it.edition.is_some() { 1 } else { 0 }
    }
    pub fn item_price(&self, db: &Db, def: usize, edition: Option<Edition>) -> i64 {
        let ed = match edition { Some(Edition::Foil) => 2, Some(Edition::Holo) => 3, Some(Edition::Poly) => 5, None => 0 };
        // with no slot cap, a growing rack slowly raises prices
        let inflation = if self.price_freeze { 0 } else { self.items.len() as i64 / 3 };
        let m = self.mods(db);
        (((db.items[def].cost + ed + inflation - self.item_rebate) as f64 * self.discount * m.discount).round() as i64).max(1)
    }
    pub fn card_price(&self, db: &Db, def: usize) -> i64 {
        ((db.cards[def].cost as f64 * self.discount * self.mods(db).discount).round() as i64).max(1)
    }

    /// effective rules of item at index i, following Blueprint/Brainstorm copies
    pub fn rules_of(&self, db: &Db, i: usize) -> (usize, Vec<Rule>) {
        let mut j = i;
        for _ in 0..8 {
            let rules = &db.items[self.items[j].def].rules;
            match rules.first().map(|r| r.eff) {
                Some(Eff::CopyBelow) if j + 1 < self.items.len() => j += 1,
                Some(Eff::CopyTop) if j != 0 => j = 0,
                Some(Eff::CopyBelow) | Some(Eff::CopyTop) => return (j, vec![]),
                _ => return (j, rules.clone()),
            }
        }
        (j, vec![])
    }

    pub fn mods(&self, db: &Db) -> Mods {
        let mut m = Mods { flipper_power: self.flipper_power, bumper_kick: 1.0, ball_save: self.ball_save, interest_cap: self.interest_cap, discount: 1.0, gravity: 1.0, ..Default::default() };
        for i in 0..self.items.len() {
            let (_, rules) = self.rules_of(db, i);
            for r in rules {
                if !matches!(r.on, On::Passive) { continue; }
                match r.eff {
                    Eff::Balls(n) => m.balls += n,
                    Eff::FlipperPower(x) => m.flipper_power *= x,
                    Eff::BumperKick(x) => m.bumper_kick *= x,
                    Eff::BallSave(s) => m.ball_save = m.ball_save.max(s),
                    Eff::InterestCap(n) => m.interest_cap += n,
                    Eff::NoTilt => m.no_tilt = true,
                    Eff::WildLanes => m.wild_lanes = true,
                    Eff::SpareChange(n) => m.spare_change += n,
                    Eff::Discount(x) => m.discount *= x,
                    Eff::Gravity(x) => m.gravity *= x,
                    _ => {}
                }
            }
        }
        m
    }

    pub fn boss_for(&mut self, db: &Db, ante: u32) -> usize {
        if let Some(b) = self.bosses.get(&ante) { return *b; }
        let b = if ante % 8 == 0 {
            db.bosses.iter().position(|b| b.id == Boss::Crown).unwrap()
        } else {
            let used: HashSet<usize> = self.bosses.values().copied().collect();
            let pool: Vec<usize> = (0..db.bosses.len()).filter(|i| db.bosses[*i].id != Boss::Crown && !used.contains(i)).collect();
            let pool = if pool.is_empty() { (0..db.bosses.len() - 1).collect() } else { pool };
            pool[fastrand::usize(..pool.len())]
        };
        self.bosses.insert(ante, b);
        b
    }
    pub fn tag_for(&mut self, ante: u32, blind: u32) -> Tag {
        *self.tags.entry((ante, blind)).or_insert_with(|| TAGS[fastrand::usize(..TAGS.len())])
    }
    pub fn chance(&self, n: u32) -> bool { fastrand::f64() < self.luck as f64 / n as f64 }
}

pub struct TableInfo { pub name: String, pub target: f64, pub reward: i64, pub boss: Option<usize> }
pub fn table_info(run: &mut Run, db: &Db) -> TableInfo {
    let (name, mult, reward) = [("Small Table", 1.0, 3), ("Big Table", 1.5, 4), ("Boss Table", 2.0, 5)][run.blind.min(2) as usize];
    let boss = if run.blind == 2 { Some(run.boss_for(db, run.ante)) } else { None };
    let bx = match boss.map(|b| db.bosses[b].id) { Some(Boss::Wall) | Some(Boss::Crown) => 1.5, _ => 1.0 };
    let target = ((ante_target(run.ante) * mult * bx) / 50.0).round() * 50.0;
    TableInfo { name: boss.map(|b| db.bosses[b].name.to_string()).unwrap_or(name.to_string()), target, reward, boss }
}

// ------------------------------------------------------------------ scoring
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FxKind { Pts, Mult, XMult, Cash }
#[derive(Clone, Debug)]
pub struct Fx { pub kind: FxKind, pub value: f64, pub src: Option<u32>, pub at: Option<(f64, f64)>, pub points: f64, pub mult: f64 }

#[derive(Default, Clone)]
pub struct Ball {
    pub points: f64, pub mult: f64, pub hits: HashMap<Shot, u32>, pub kinds: HashSet<Shot>, pub total: u32,
    pub streak: u32, pub last_hit: f64, pub multiball: bool, pub is_last: bool, pub is_first: bool,
    pub start: f64, pub duration: f64, pub tilted: bool, pub first_hit_done: bool, pub score: f64,
}

pub struct Ctx<'a> {
    pub db: &'a Db,
    pub run: &'a mut Run,
    pub ball: &'a mut Ball,
    pub fx: Vec<Fx>,
    pub at: Option<(f64, f64)>,
    pub src: Option<u32>,
    pub muted_bumpers: bool,
    pub miser: bool,
    pub destroyed: Vec<u32>,
    pub extra_balls: u32,
    pub saved: bool,
    pub first_drain: bool,
}

impl<'a> Ctx<'a> {
    fn out(&mut self, kind: FxKind, value: f64) {
        self.fx.push(Fx { kind, value, src: self.src, at: self.at, points: self.ball.points, mult: self.ball.mult });
    }
    pub fn pts(&mut self, v: f64) { if v != 0.0 { self.ball.points += v; self.out(FxKind::Pts, v); } }
    pub fn mult(&mut self, v: f64) { if v != 0.0 { self.ball.mult += v; self.out(FxKind::Mult, v); } }
    pub fn xmult(&mut self, v: f64) { if (v - 1.0).abs() > 1e-9 { self.ball.mult *= v; self.out(FxKind::XMult, v); } }
    pub fn cash(&mut self, v: i64) {
        if v != 0 && !(self.miser && self.src.is_some()) {
            self.run.money += v;
            self.out(FxKind::Cash, v as f64);
        }
    }

    fn per(&self, per: Per, idx: usize, stat: f64) -> f64 {
        match per {
            Per::One => 1.0,
            Per::Money5 => (self.run.money.max(0) / 5) as f64,
            Per::Items => self.run.items.len() as f64,
            Per::Items3 => (self.run.items.len() / 3) as f64,
            Per::BallSeconds10 => (self.ball.duration / 10.0).floor(),
            Per::SpinsThisBall10 => (*self.ball.hits.get(&Shot::Spin).unwrap_or(&0) / 10) as f64,
            Per::OtherSellValue => self.run.items.iter().enumerate().filter(|(j, _)| *j != idx).map(|(_, it)| self.run.sell_value(self.db, it)).sum::<i64>() as f64,
            Per::Stat => stat,
        }
    }

    fn cond(&self, c: Cond, item_i: usize) -> bool {
        match c {
            Cond::Always => true,
            Cond::Chance(n) => self.run.chance(n),
            Cond::KindsAtLeast(n) => self.ball.kinds.len() >= n,
            Cond::Multiball => self.ball.multiball,
            Cond::NoMultiball => !self.ball.multiball,
            Cond::LastBall => self.ball.is_last,
            Cond::FirstBall => self.ball.is_first,
            Cond::FirstHit => !self.ball.first_hit_done,
            Cond::LongBall(s) => self.ball.duration >= s,
            Cond::FirstDrainOfTable => self.first_drain && !self.run.items[item_i].used,
        }
    }

    /// run every item's rules that match the trigger, top to bottom
    pub fn trigger(&mut self, matches: impl Fn(&On) -> bool) {
        for i in 0..self.run.items.len() {
            let (_, rules) = self.run.rules_of(self.db, i);
            self.src = Some(self.run.items[i].uid);
            for r in rules {
                if !matches(&r.on) || !self.cond(r.cond, i) { continue; }
                let stat = self.run.items[i].stat;
                match r.eff {
                    Eff::Pts(v, p) => { let x = v * self.per(p, i, stat); self.pts(x.floor().max(if x > 0.0 { 1.0 } else { 0.0 })); }
                    Eff::Mult(v, p) => { let x = v * self.per(p, i, stat); self.mult(x); }
                    Eff::XMult(b, k, p) => { let x = b + k * self.per(p, i, stat); if x > 1.0 { self.xmult(x); } }
                    Eff::Cash(v, p) => { let x = (v * self.per(p, i, stat)).floor() as i64; self.cash(x.min(15)); }
                    Eff::Grow(v) => self.run.items[i].stat += v,
                    Eff::Save => { self.run.items[i].used = true; if self.run.chance(2) { self.saved = true; } }
                    Eff::Destroy => self.destroyed.push(self.run.items[i].uid),
                    Eff::ExtraBall => self.extra_balls += 1,
                    _ => {}
                }
            }
        }
        self.src = None;
    }

    pub fn hit(&mut self, shot: Shot, piece: Option<&'static str>, now: f64) {
        if self.ball.tilted { return; }
        let scoring = !matches!(shot, Shot::Flipper | Shot::Nudge);
        if scoring {
            self.ball.streak = if now - self.ball.last_hit < 1.2 { self.ball.streak + 1 } else { 1 };
            self.ball.last_hit = now;
            *self.ball.hits.entry(shot).or_insert(0) += 1;
            self.ball.kinds.insert(shot);
            self.ball.total += 1;
            *self.run.stats.shot_hits.entry(shot).or_insert(0) += 1;
        }
        let muted = shot == Shot::Bumper && self.muted_bumpers;
        if scoring && !muted {
            let (p, m) = self.run.shot_value(shot);
            self.pts(p);
            if m != 0.0 { self.mult(m); }
            if let Some(pc) = piece {
                if let Some(e) = self.run.enh.get(pc).copied() { self.enhance(e, pc); }
            }
        }
        if muted { return; }
        let n = *self.ball.hits.get(&shot).unwrap_or(&0);
        let streak = self.ball.streak;
        self.trigger(|on| match *on {
            On::Hit(set) => set.contains(&shot),
            On::AnyHit => scoring,
            On::EveryNth(s, k) => s == shot && n > 0 && n % k == 0,
            On::Streak(k) => scoring && streak > 0 && streak % k == 0,
            _ => false,
        });
        if scoring { self.ball.first_hit_done = true; }
    }

    fn enhance(&mut self, e: Enh, piece: &'static str) {
        match e {
            Enh::Gold => {
                let c = self.run.stats.piece_hits.entry(piece).or_insert(0);
                *c += 1;
                if *c % 4 == 0 { self.cash(1); }
            }
            Enh::Glass => {
                self.xmult(1.25);
                if self.run.chance(10) { self.run.enh.remove(piece); }
            }
            Enh::Bonus => self.pts(40.0),
            Enh::Mult => self.mult(1.0),
            Enh::Lucky => {
                if self.run.chance(5) { self.mult(5.0); }
                if self.run.chance(15) { self.cash(5); }
            }
        }
    }

    pub fn end_ball(&mut self, now: f64) {
        self.ball.duration = now - self.ball.start;
        if !self.ball.tilted {
            for i in 0..self.run.items.len() {
                let uid = self.run.items[i].uid;
                // the item's own ball-end rules, then its edition
                let (_, rules) = self.run.rules_of(self.db, i);
                self.src = Some(uid);
                let stat = self.run.items[i].stat;
                for r in rules {
                    if !matches!(r.on, On::BallEnd) || !self.cond(r.cond, i) { continue; }
                    let stat = if matches!(r.eff, Eff::Grow(_)) { stat } else { self.run.items[i].stat };
                    match r.eff {
                        Eff::Pts(v, p) => { let x = v * self.per(p, i, stat); self.pts(x.floor()); }
                        Eff::Mult(v, p) => { let x = v * self.per(p, i, stat); self.mult(x); }
                        Eff::XMult(b, k, p) => { let x = b + k * self.per(p, i, stat); if x > 1.0 { self.xmult(x); } }
                        Eff::Grow(v) => self.run.items[i].stat += v,
                        _ => {}
                    }
                }
                match self.run.items[i].edition {
                    Some(Edition::Foil) => self.pts(50.0),
                    Some(Edition::Holo) => self.mult(10.0),
                    Some(Edition::Poly) => self.xmult(1.5),
                    None => {}
                }
            }
            self.src = None;
        }
        self.ball.score = if self.ball.tilted { 0.0 } else { (self.ball.points * self.ball.mult).round() };
    }
}
