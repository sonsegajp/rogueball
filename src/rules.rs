//! Table rules: physics events become shots; runs the lights, combos, multiball, ball save and tilt.
//! Works from the layout's feature groups, so any generated table gets the same rules.

use crate::content::Shot;
use crate::physics::{Ev, Physics};
use crate::table::{Layout, SensorKind};
use std::collections::HashMap;

/// what the rules want the game to do after an event
#[derive(Clone, Debug)]
pub enum Out {
    Shot { shot: Shot, piece: Option<&'static str>, x: f64, y: f64, quiet: bool },
    Sound(&'static str),
    Banner(String, u32),
    Hit { kind: &'static str, id: usize, x: f64, y: f64 },
    Drain(u32),
    Shake(f32),
    /// a ramp was made; its ball comes back down to the flipper on this side
    Catch(i8),
}

pub struct Rules {
    pub lanes: Vec<Vec<bool>>,
    pub stands: Vec<Vec<bool>>,
    pub ramp_count: u32,
    /// majors of the table's multiball shot so far
    pub mb_count: u32,
    pub mb_shot: Shot,
    pub combo: u32,
    pub last_major: f64,
    pub save_timer: f64,
    pub flash: HashMap<String, f64>,
    pub bank_reset_at: Vec<f64>,
    pub tilt: f64,
    pub tilted: bool,
    pub mb_active: bool,
    pub pending_launch: u32,
    pub launch_at: f64,
    pub launched: bool,
    lane_time: HashMap<u32, f64>,
    /// per rollover: group, set, index in set
    roll: Vec<(&'static str, usize, usize)>,
    sensors: Vec<SensorKind>,
    n_rollovers: usize,
    /// drop target / standup → (group, index in group)
    drop_of: Vec<(usize, usize)>,
    stand_of: Vec<(usize, usize)>,
    banks: Vec<(Vec<usize>, [f64; 2])>,
    ramps: Vec<([f64; 2], i8)>,
    orbits: Vec<[f64; 2]>,
}

impl Rules {
    pub fn new(l: &Layout) -> Rules {
        let f = &l.features;
        let mut roll: Vec<(&'static str, usize, usize)> = l.rollovers.iter().map(|r| (r.group, r.set, 0)).collect();
        for (g, (idx, _)) in f.lanes.iter().enumerate() {
            for (k, &i) in idx.iter().enumerate() { roll[i] = ("top", g, k); }
        }
        let mut drop_of = vec![(0, 0); l.drops.len()];
        for (g, (idx, _)) in f.banks.iter().enumerate() { for (k, &i) in idx.iter().enumerate() { drop_of[i] = (g, k); } }
        let mut stand_of = vec![(0, 0); l.standups.len()];
        for (g, (idx, _)) in f.stands.iter().enumerate() { for (k, &i) in idx.iter().enumerate() { stand_of[i] = (g, k); } }
        let mb_shot = if !f.ramps.is_empty() { Shot::Ramp } else if !f.orbits.is_empty() { Shot::Orbit } else if !f.banks.is_empty() { Shot::Bank } else { Shot::Lanes };
        Rules {
            lanes: f.lanes.iter().map(|(v, _)| vec![false; v.len()]).collect(),
            stands: f.stands.iter().map(|(v, _)| vec![false; v.len()]).collect(),
            ramp_count: 0, mb_count: 0, mb_shot, combo: 0, last_major: -99.0, save_timer: 0.0,
            flash: HashMap::new(), bank_reset_at: vec![0.0; f.banks.len()], tilt: 0.0, tilted: false, mb_active: false,
            pending_launch: 0, launch_at: 0.0, launched: false, lane_time: HashMap::new(),
            roll, sensors: l.sensors.iter().map(|s| s.kind).collect(), n_rollovers: l.rollovers.len(),
            drop_of, stand_of, banks: f.banks.clone(),
            ramps: f.ramps.iter().zip(&l.ramps).map(|(p, r)| (*p, r.side)).collect(),
            orbits: f.orbits.clone(),
        }
    }

    /// short text for the apron card: what lights multiball on this table
    pub fn mb_hint(&self) -> &'static str {
        match self.mb_shot {
            Shot::Ramp => "3 RAMPS: MULTIBALL",
            Shot::Orbit => "3 ORBITS: MULTIBALL",
            Shot::Bank => "3 BANKS: MULTIBALL",
            _ => "3 LANE SETS: MULTIBALL",
        }
    }

    pub fn reset_table(&mut self, p: &mut Physics) {
        for l in &mut self.lanes { l.iter_mut().for_each(|b| *b = false); }
        for s in &mut self.stands { s.iter_mut().for_each(|b| *b = false); }
        self.ramp_count = 0;
        self.mb_count = 0;
        self.combo = 0;
        self.last_major = -99.0;
        self.save_timer = 0.0;
        self.flash.clear();
        self.bank_reset_at.iter_mut().for_each(|t| *t = 0.0);
        self.pending_launch = 0;
        self.launch_at = 0.0;
        p.reset_drops();
        self.start_ball();
    }

    pub fn start_ball(&mut self) {
        self.tilt = 0.0;
        self.tilted = false;
        self.combo = 0;
        self.mb_active = false;
        self.launched = false;
        self.save_timer = 0.0;
    }

    pub fn ball_save(&mut self, secs: f64) { self.save_timer = self.save_timer.max(secs); }

    pub fn start_multiball(&mut self, n: u32, out: &mut Vec<Out>) {
        self.pending_launch += n;
        self.mb_active = true;
        out.push(Out::Banner("MULTIBALL".into(), 0x3fc46a));
        out.push(Out::Sound("multiball"));
        self.ball_save(self.save_timer.max(6.0));
    }

    fn blink(&mut self, id: &str, now: f64, dur: f64) { self.flash.insert(id.to_string(), now + dur); }

    fn shot(&self, out: &mut Vec<Out>, shot: Shot, piece: Option<&'static str>, x: f64, y: f64, quiet: bool) {
        if !self.tilted { out.push(Out::Shot { shot, piece, x, y, quiet }); }
    }

    fn major(&mut self, shot: Shot, x: f64, y: f64, now: f64, out: &mut Vec<Out>) {
        self.shot(out, shot, None, x, y, false);
        if now - self.last_major < 4.0 {
            self.combo = (self.combo + 1).min(5);
            out.push(Out::Sound("combo"));
            out.push(Out::Banner(format!("COMBO ×{}", self.combo + 1), 0x7d68d4));
            for i in 0..self.combo { self.shot(out, Shot::Combo, None, 0.0, 0.3, i > 0); }
        } else {
            self.combo = 0;
        }
        self.last_major = now;
        if shot == self.mb_shot {
            self.mb_count += 1;
            if self.mb_count % 3 == 0 && !self.mb_active { self.start_multiball(1, out); }
        }
    }

    /// translate one physics event; `wild` = Wild Lanes item
    pub fn handle(&mut self, e: &Ev, p: &Physics, now: f64, wild: bool, out: &mut Vec<Out>) {
        match *e {
            Ev::Bumper { id, x, y } => {
                out.push(Out::Sound("bumper"));
                out.push(Out::Hit { kind: "bumper", id, x, y });
                out.push(Out::Shake(0.25));
                self.shot(out, Shot::Bumper, Some("bumper"), x, y, false);
            }
            Ev::Sling { id, x, y } => {
                out.push(Out::Sound("sling"));
                out.push(Out::Hit { kind: "sling", id, x, y });
                self.shot(out, Shot::Sling, Some("sling"), x, y, false);
            }
            Ev::Drop { id, x, y } => {
                out.push(Out::Sound("drop"));
                out.push(Out::Hit { kind: "drop", id, x, y });
                self.shot(out, Shot::Drop, Some("drop"), x, y, false);
                let (g, _) = self.drop_of[id];
                let (idx, at) = self.banks[g].clone();
                if idx.iter().all(|&i| p.drops[i].down) {
                    out.push(Out::Sound("bank"));
                    for k in 0..idx.len() { self.blink(&format!("drop:{g}:{k}"), now, 1.0); }
                    self.major(Shot::Bank, at[0], at[1], now, out);
                    self.bank_reset_at[g] = now + 1.2;
                }
            }
            Ev::Standup { id, x, y } => {
                out.push(Out::Sound("standup"));
                out.push(Out::Hit { kind: "standup", id, x, y });
                let (g, k) = self.stand_of[id];
                self.stands[g][k] = true;
                self.shot(out, Shot::Standup, Some("standup"), x, y, false);
                if self.stands[g].iter().all(|b| *b) {
                    let n = self.stands[g].len();
                    self.stands[g].iter_mut().for_each(|b| *b = false);
                    for k in 0..n { self.blink(&format!("stand:{g}:{k}"), now, 1.0); }
                    self.shot(out, Shot::Standup, None, x, y, false);
                }
            }
            Ev::Spin { id } => {
                out.push(Out::Sound("spin"));
                let s = &p.spinners[id];
                let (x, y) = ((s.a[0] + s.b[0]) / 2.0, s.a[1]);
                out.push(Out::Hit { kind: "spinner", id, x, y });
                self.shot(out, Shot::Spin, None, x, y, true);
            }
            Ev::Rollover { idx, x, y } => {
                out.push(Out::Sound("rollover"));
                out.push(Out::Hit { kind: "rollover", id: idx, x, y });
                let (group, g, k) = self.roll[idx];
                let side = if x < 0.0 { "L" } else { "R" };
                match group {
                    "top" => {
                        self.lanes[g][k] = true;
                        self.shot(out, Shot::Rollover, None, x, y, false);
                        let n = self.lanes[g].len();
                        let lit = self.lanes[g].iter().filter(|b| **b).count();
                        if lit == n || (lit + 1 >= n && wild) {
                            self.lanes[g].iter_mut().for_each(|b| *b = false);
                            for k in 0..n { self.blink(&format!("lane:{g}:{k}"), now, 1.0); }
                            out.push(Out::Sound("lanes"));
                            self.major(Shot::Lanes, x, y, now, out);
                        }
                    }
                    "inlane" => {
                        self.blink(&format!("in:{side}"), now, 0.6);
                        self.shot(out, Shot::Inlane, None, x, y, false);
                    }
                    _ => self.blink(&format!("out:{side}"), now, 1.5),
                }
            }
            Ev::Sensor { idx, ball, vy } => {
                // physics lists the rollovers first, then the other sensors
                match self.sensors[idx - self.n_rollovers] {
                    SensorKind::OrbitEntry(_) => { if vy > 0.25 { self.lane_time.insert(ball, now); } }
                    SensorKind::Top => {
                        if let Some(t) = self.lane_time.get(&ball).copied() {
                            if now - t < 1.8 {
                                self.lane_time.remove(&ball);
                                out.push(Out::Sound("orbit"));
                                for k in 0..self.orbits.len() { self.blink(&format!("orbit:{k}"), now, 1.0); }
                                let at = self.orbits.first().copied().unwrap_or([0.0, 0.9]);
                                self.major(Shot::Orbit, at[0], at[1], now, out);
                            }
                        }
                    }
                    SensorKind::RampMade(k) => {
                        out.push(Out::Sound("ramp"));
                        self.blink(&format!("ramp:{k}"), now, 1.0);
                        self.ramp_count += 1;
                        let (at, side) = self.ramps[k];
                        out.push(Out::Catch(side));
                        self.major(Shot::Ramp, at[0], at[1], now, out);
                    }
                    SensorKind::RampEnter(_) => {}
                }
            }
            Ev::FlipperUp(i) => {
                out.push(Out::Sound("flipper"));
                // lane change: lit top lanes shift toward the flipper's side
                let left = p.flippers[i].side < 0;
                for l in &mut self.lanes { if left { l.rotate_left(1) } else { l.rotate_right(1) } }
            }
            Ev::FlipperDown(_) => out.push(Out::Sound("flipper_down")),
            Ev::FlipperHit { .. } => self.shot(out, Shot::Flipper, None, 0.0, 0.0, true),
            Ev::Plunger { .. } => { out.push(Out::Sound("plunger")); self.launched = true; }
            Ev::Wall { .. } => out.push(Out::Sound("wall")),
            Ev::Rubber { .. } => out.push(Out::Sound("rubber")),
            Ev::BallClick { .. } => out.push(Out::Sound("click")),
            Ev::Drain { ball } => out.push(Out::Drain(ball)),
            _ => {}
        }
    }

    /// returns true if the nudge tilted the machine
    pub fn nudge(&mut self, p: &mut Physics, dx: f64, dy: f64, no_tilt: bool, tolerance: f64, out: &mut Vec<Out>) {
        if self.tilted || p.balls.is_empty() { return; }
        p.nudge(dx, dy);
        out.push(Out::Sound("nudge"));
        out.push(Out::Shake(1.0));
        self.shot(out, Shot::Nudge, None, 0.0, 0.0, true);
        if no_tilt { return; }
        self.tilt += 1.0;
        if self.tilt > 3.2 * tolerance {
            self.tilted = true;
            p.set_side(-1, false);
            p.set_side(1, false);
            out.push(Out::Sound("tilt"));
            out.push(Out::Banner("TILT".into(), 0xee4040));
        } else if self.tilt > 2.1 * tolerance {
            out.push(Out::Sound("warn"));
            out.push(Out::Banner("DANGER".into(), 0xffcc4d));
        }
    }

    pub fn update(&mut self, p: &mut Physics, now: f64, dt: f64, out: &mut Vec<Out>) {
        self.tilt = (self.tilt - dt * 0.45).max(0.0);
        if self.save_timer > 0.0 && self.launched { self.save_timer -= dt; }
        for g in 0..self.bank_reset_at.len() {
            let t = self.bank_reset_at[g];
            if t > 0.0 && now > t {
                let at = self.banks[g].1;
                let blocked = p.balls.iter().any(|b| (b.x - at[0]).abs() < 0.11 && (b.y - at[1]).abs() < 0.07);
                if !blocked {
                    for &i in &self.banks[g].0 { p.reset_drop(i); }
                    self.bank_reset_at[g] = 0.0;
                }
            }
        }
        if now - self.last_major > 4.0 { self.combo = 0; }
        // queued multiball / saved balls launch from the shooter lane one at a time
        if self.pending_launch > 0 && !p.any_in_lane() && !p.plunger.firing && self.launch_at == 0.0 {
            p.serve_ball();
            self.launch_at = now + 0.6;
        }
        if self.launch_at > 0.0 && now > self.launch_at {
            p.release_plunger(Some(0.85 + fastrand::f64() * 0.1));
            self.launch_at = 0.0;
            self.pending_launch = self.pending_launch.saturating_sub(1);
        }
        if self.mb_active && p.balls.len() <= 1 && self.pending_launch == 0 && self.launch_at == 0.0 { self.mb_active = false; }
        // stuck-ball rescue: judged by how far the ball has moved, so a ball jittering in a pocket
        // counts as stuck too. Two nudges, then it is lifted back to the shooter lane.
        let mut reserve = 0;
        let lane_x = p.lane_x;
        let flippers: Vec<(f64, f64, f64, bool)> = p.flippers.iter().map(|f| (f.x, f.y, f.len, f.on)).collect();
        for b in p.balls.iter_mut() {
            if Physics::in_lane(lane_x, b) { b.still = 0.0; b.anchor = (b.x, b.y); b.rescues = 0; continue; }
            // a ball cradled on a held flipper is the player's choice, not stuck
            let cradled = flippers.iter().any(|&(fx, fy, len, on)| on && ((b.x - fx).powi(2) + (b.y - fy).powi(2)).sqrt() < len + 0.03);
            let moved = ((b.x - b.anchor.0).powi(2) + (b.y - b.anchor.1).powi(2)).sqrt();
            if moved > 0.012 || cradled { b.anchor = (b.x, b.y); b.still = 0.0; b.rescues = 0; continue; }
            b.still += dt;
            if b.still > 3.0 {
                b.still = 0.0;
                b.rescues += 1;
                if b.rescues >= 3 {
                    b.alive = false;
                    reserve += 1;
                } else {
                    b.vx += (fastrand::f64() - 0.5) * 0.8;
                    b.vy += 0.5;
                    b.vz += 0.35;
                    out.push(Out::Banner("BALL FREED".into(), 0x9a99b3));
                }
            }
        }
        if reserve > 0 {
            p.balls.retain(|b| b.alive);
            self.pending_launch += reserve;
            out.push(Out::Banner("BALL RETURNED".into(), 0x9a99b3));
        }
    }

    /// light level 0..1 for an insert id
    pub fn light(&self, id: &str, t: f64, now: f64) -> f32 {
        let blink = |hz: f64| -> f64 { if (t * std::f64::consts::TAU * hz).sin() > 0.0 { 1.0 } else { 0.15 } };
        if let Some(until) = self.flash.get(id) { if now < *until { return blink(8.0) as f32; } }
        let mut parts = id.split(':');
        let kind = parts.next().unwrap_or("");
        let a: usize = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let b: usize = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let mb_ready = self.mb_count % 3 == 2;
        let v = match kind {
            "lane" => self.lanes.get(a).and_then(|l| l.get(b)).map(|v| *v as u8 as f64).unwrap_or(0.0),
            "stand" => if self.stands.get(a).and_then(|l| l.get(b)).copied().unwrap_or(false) { 1.0 } else { blink(1.2) * 0.35 },
            "drop" => 0.25,
            "ramp" => if self.mb_shot == Shot::Ramp && mb_ready { blink(4.0) } else { blink(1.2) * 0.6 },
            "orbit" => if self.mb_shot == Shot::Orbit && mb_ready { blink(4.0) } else { blink(1.0) * 0.6 },
            "mb" => if self.mb_active { blink(6.0) } else if mb_ready { blink(2.0) } else { (self.mb_count % 3) as f64 * 0.35 },
            "save" => if self.save_timer > 0.0 { if self.save_timer < 2.0 { blink(6.0) } else { 1.0 } } else { 0.0 },
            "in" | "out" => 0.15,
            "cl" => {
                let i = a as u32;
                if self.combo > i { 1.0 } else if now - self.last_major < 4.0 && i == self.combo { blink(4.0) * 0.6 } else { 0.0 }
            }
            _ => 0.0,
        };
        v as f32
    }
}
