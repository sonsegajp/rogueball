//! Pinball physics: fixed 1 kHz steps, a rolling/spinning ball against analytic table primitives.
//! Coordinates are layout coords (see table.rs): x right, y up-table, z off the playfield.

use crate::table::{Layout, BALL_R, DRAIN_Y};
use std::collections::HashSet;

const R: f64 = BALL_R;
const BALL_MASS: f64 = 0.08;
pub const STEP: f64 = 1.0 / 1000.0;

#[derive(Clone, Copy)]
pub struct Mat { pub e: f64, pub fall: f64, pub min: f64, pub mu: f64 }
// e: restitution at low speed, fall: loss per m/s of impact, min: floor (rubber goes dead when hit hard)
pub const METAL: Mat = Mat { e: 0.42, fall: 0.03, min: 0.3, mu: 0.12 };
pub const RUBBER: Mat = Mat { e: 0.82, fall: 0.06, min: 0.45, mu: 0.6 };
pub const PLASTIC: Mat = Mat { e: 0.3, fall: 0.02, min: 0.2, mu: 0.15 };
pub const FLOOR: Mat = Mat { e: 0.22, fall: 0.0, min: 0.22, mu: 0.25 };
pub const FLIPPER: Mat = Mat { e: 0.78, fall: 0.11, min: 0.32, mu: 0.5 };
pub const BUMPER: Mat = Mat { e: 0.55, fall: 0.04, min: 0.35, mu: 0.3 };
pub const TARGET: Mat = Mat { e: 0.45, fall: 0.04, min: 0.25, mu: 0.3 };
pub const PLUNGER: Mat = Mat { e: 0.0, fall: 0.0, min: 0.0, mu: 0.4 };

fn mat_by_name(n: &str) -> Mat {
    match n { "rubber" => RUBBER, "plastic" => PLASTIC, _ => METAL }
}
fn restitution(m: &Mat, vn: f64) -> f64 { (m.e - m.fall * vn).max(m.min) }
fn clamp(v: f64, a: f64, b: f64) -> f64 { v.max(a).min(b) }

#[derive(Clone, Debug)]
pub enum Ev {
    Bumper { id: usize, x: f64, y: f64 },
    Sling { id: usize, x: f64, y: f64 },
    Drop { id: usize, x: f64, y: f64 },
    Standup { id: usize, x: f64, y: f64 },
    Spin { id: usize },
    SpinnerHit { v: f64 },
    Rollover { idx: usize, x: f64, y: f64 },
    Sensor { idx: usize, ball: u32, vy: f64 },
    FlipperUp(usize),
    FlipperDown(usize),
    FlipperHit { v: f64 },
    Plunger { strength: f64 },
    Nudge,
    Wall { v: f64 },
    Rubber { v: f64 },
    BallClick { v: f64 },
    Drain { ball: u32 },
}

#[derive(Clone)]
pub struct Ball {
    pub id: u32,
    pub x: f64, pub y: f64, pub z: f64,
    pub vx: f64, pub vy: f64, pub vz: f64,
    pub wx: f64, pub wy: f64, pub wz: f64,
    /// previous step position, for render interpolation
    pub px: f64, pub py: f64, pub pz: f64,
    /// accumulated rolling angle for the sprite
    pub roll: f64,
    pub inside: HashSet<usize>,
    pub spin_side: Vec<Option<i8>>,
    pub on_floor: bool,
    pub alive: bool,
    pub still: f64,
    /// where the ball was when the stuck timer last reset
    pub anchor: (f64, f64),
    pub rescues: u32,
}
impl Ball {
    pub fn speed(&self) -> f64 { (self.vx * self.vx + self.vy * self.vy + self.vz * self.vz).sqrt() }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SegKind { Wall, Sling(usize), Drop(usize), Standup(usize), Gate(usize), Plunger, RampBlock }

#[derive(Clone)]
pub struct Seg {
    pub ax: f64, pub ay: f64, pub bx: f64, pub by: f64, pub r: f64,
    pub zb0: f64, pub zt0: f64, pub zb1: f64, pub zt1: f64,
    pub mat: Mat, pub kind: SegKind, pub on: bool,
    dx: f64, dy: f64, len2: f64, minx: f64, maxx: f64, miny: f64, maxy: f64,
}
impl Seg {
    fn new(a: [f64; 4], b: [f64; 4], r: f64, mat: Mat, kind: SegKind) -> Seg {
        let mut s = Seg {
            ax: a[0], ay: a[1], bx: b[0], by: b[1], r,
            zb0: a[2], zt0: a[3], zb1: b[2], zt1: b[3],
            mat, kind, on: true, dx: 0.0, dy: 0.0, len2: 0.0, minx: 0.0, maxx: 0.0, miny: 0.0, maxy: 0.0,
        };
        s.refresh();
        s
    }
    fn refresh(&mut self) {
        self.dx = self.bx - self.ax;
        self.dy = self.by - self.ay;
        self.len2 = (self.dx * self.dx + self.dy * self.dy).max(1e-12);
        let pad = self.r + R;
        self.minx = self.ax.min(self.bx) - pad;
        self.maxx = self.ax.max(self.bx) + pad;
        self.miny = self.ay.min(self.by) - pad;
        self.maxy = self.ay.max(self.by) + pad;
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum CircleKind { Post, Bumper(usize) }
#[derive(Clone)]
pub struct Circle { pub x: f64, pub y: f64, pub r: f64, pub zb: f64, pub zt: f64, pub mat: Mat, pub kind: CircleKind }

pub struct FlipperState {
    pub x: f64, pub y: f64, pub len: f64, pub r0: f64, pub r1: f64, pub h: f64,
    pub rest: f64, pub up: f64, pub dir: f64,
    /// -1 left button, +1 right button
    pub side: i8,
    pub ang: f64, pub prev_ang: f64, pub w: f64, pub on: bool, pinned: bool,
    inertia: f64, accel: f64, max_w: f64, ret_accel: f64, ret_max_w: f64,
}
pub struct BumperState { pub x: f64, pub y: f64, pub r: f64, cool: f64 }
pub struct SlingState { pub kick_seg: usize, cool: f64 }
pub struct DropState { pub seg: usize, pub down: bool }
pub struct StandupState { pub seg: usize, pub n: [f64; 2], cool: f64 }
pub struct GateState { pub dir: [f64; 2], pub swing: f64 }
pub struct SpinnerState { pub a: [f64; 2], pub b: [f64; 2], pub z: f64, pub angle: f64, pub w: f64, acc: f64 }
pub struct PlungerState { pub x: f64, pub y0: f64, pub travel: f64, pub pos: f64, pub vel: f64, pub pull: f64, pub held: bool, pub firing: bool, stop_next: bool, seg: usize }
pub struct SensorDef { pub x: f64, pub y: f64, pub r: f64, pub z0: f64, pub z1: f64, pub rollover: bool }

struct RampSeg { ax: f64, ay: f64, ah: f64, tx: f64, ty: f64, len: f64, slope: f64 }
struct RampFloor { segs: Vec<RampSeg>, half_w: f64 }

enum KickKind { Bumper(usize), Sling { id: usize, nx: f64, ny: f64 } }
struct Kick { t: f64, ball: u32, kind: KickKind }

pub struct Physics {
    pub balls: Vec<Ball>,
    pub time: f64,
    pub events: Vec<Ev>,
    pub gravity_scale: f64,
    pub drift: f64,
    pub bumper_kick: f64,
    pub sling_kick: f64,
    pub flipper_power: f64,
    pub flipper_power_l: f64,
    pub flipper_power_r: f64,
    /// return-stroke speed scale (the Sticky boss slows it)
    pub flipper_return: f64,
    pub ramp_blocked: bool,
    /// count of balls removed for a non-finite state (logged by the game)
    pub bad_balls: u32,
    slope: f64,
    pub segs: Vec<Seg>,
    pub circles: Vec<Circle>,
    pub flippers: Vec<FlipperState>,
    pub bumpers: Vec<BumperState>,
    pub slings: Vec<SlingState>,
    pub drops: Vec<DropState>,
    pub standups: Vec<StandupState>,
    pub gates: Vec<GateState>,
    pub spinners: Vec<SpinnerState>,
    pub plunger: PlungerState,
    pub sensors: Vec<SensorDef>,
    ramp_floors: Vec<RampFloor>,
    ramp_block: Vec<usize>,
    /// shooter lane divider x, and the box outside which a ball is lost
    pub lane_x: f64,
    bounds: [f64; 4],
    kicks: Vec<Kick>,
    next_ball: u32,
}

impl Physics {
    pub fn new(l: &Layout) -> Physics {
        let mut segs: Vec<Seg> = vec![];
        let mut circles: Vec<Circle> = vec![];
        for w in &l.walls {
            for p in w.pts.windows(2) {
                segs.push(Seg::new(p[0], p[1], w.r, mat_by_name(w.mat), SegKind::Wall));
            }
        }
        for p in &l.posts {
            circles.push(Circle { x: p.x, y: p.y, r: p.r, zb: 0.0, zt: p.zt, mat: mat_by_name(p.mat), kind: CircleKind::Post });
        }
        let bumpers = l.bumpers.iter().enumerate().map(|(i, b)| {
            circles.push(Circle { x: b.x, y: b.y, r: b.r, zb: 0.0, zt: 0.04, mat: BUMPER, kind: CircleKind::Bumper(i) });
            BumperState { x: b.x, y: b.y, r: b.r, cool: 0.0 }
        }).collect();
        let slings = l.slings.iter().enumerate().map(|(i, s)| {
            let p = |k: usize| [s.pts[k][0], s.pts[k][1], 0.0, s.zt];
            segs.push(Seg::new(p(0), p(1), s.r, RUBBER, SegKind::Wall));
            segs.push(Seg::new(p(1), p(2), s.r, RUBBER, SegKind::Wall));
            segs.push(Seg::new(p(0), p(2), s.r, RUBBER, SegKind::Sling(i)));
            let kick_seg = segs.len() - 1;
            for q in &s.pts {
                circles.push(Circle { x: q[0], y: q[1], r: s.r, zb: 0.0, zt: s.zt, mat: RUBBER, kind: CircleKind::Post });
            }
            SlingState { kick_seg, cool: 0.0 }
        }).collect();
        let drops = l.drops.iter().enumerate().map(|(i, d)| {
            segs.push(Seg::new([d.a[0], d.a[1], 0.0, d.zt], [d.b[0], d.b[1], 0.0, d.zt], d.r, TARGET, SegKind::Drop(i)));
            DropState { seg: segs.len() - 1, down: false }
        }).collect();
        let standups = l.standups.iter().enumerate().map(|(i, d)| {
            segs.push(Seg::new([d.a[0], d.a[1], 0.0, d.zt], [d.b[0], d.b[1], 0.0, d.zt], d.r, TARGET, SegKind::Standup(i)));
            StandupState { seg: segs.len() - 1, n: d.n, cool: 0.0 }
        }).collect();
        let gates = l.gates.iter().enumerate().map(|(i, g)| {
            segs.push(Seg::new([g.a[0], g.a[1], 0.0, 0.045], [g.b[0], g.b[1], 0.0, 0.045], 0.002, METAL, SegKind::Gate(i)));
            GateState { dir: g.dir, swing: 0.0 }
        }).collect();
        let mut ramp_block = vec![];
        let ramp_floors = l.ramps.iter().map(|rp| {
            let p0 = rp.path[0];
            let hw = rp.half_w + 0.004;
            let mut s = Seg::new([p0[0] - hw, p0[1] + 0.005, 0.0, 0.05], [p0[0] + hw, p0[1] + 0.005, 0.0, 0.05], 0.004, METAL, SegKind::RampBlock);
            s.on = false;
            segs.push(s);
            ramp_block.push(segs.len() - 1);
            let segs = rp.path.windows(2).map(|w| {
                let (a, b) = (w[0], w[1]);
                let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                RampSeg { ax: a[0], ay: a[1], ah: a[2], tx: (b[0] - a[0]) / len, ty: (b[1] - a[1]) / len, len, slope: (b[2] - a[2]) / len }
            }).collect();
            RampFloor { segs, half_w: rp.half_w + 0.006 }
        }).collect();
        let spinners = l.spinners.iter().map(|s| SpinnerState { a: s.a, b: s.b, z: s.z, angle: 0.0, w: 0.0, acc: 0.0 }).collect();
        let mut sensors: Vec<SensorDef> = l.rollovers.iter().map(|r| SensorDef { x: r.x, y: r.y, r: r.r, z0: 0.0, z1: 0.03, rollover: true }).collect();
        sensors.extend(l.sensors.iter().map(|s| SensorDef { x: s.x, y: s.y, r: s.r, z0: s.z0, z1: s.z1, rollover: false }));
        let flippers = l.flippers.iter().map(|f| {
            let (rest, up) = (f.rest.to_radians(), f.up.to_radians());
            FlipperState {
                x: f.x, y: f.y, len: f.len, r0: f.r0, r1: f.r1, h: f.h, rest, up, dir: (up - rest).signum(), side: f.side,
                ang: rest, prev_ang: rest, w: 0.0, on: false, pinned: true,
                // effective rotational inertia (bat + solenoid plunger), kg·m²
                inertia: 0.0014, accel: 3400.0, max_w: 54.0, ret_accel: 1300.0, ret_max_w: 32.0,
            }
        }).collect();
        let pl = &l.plunger;
        segs.push(Seg::new([pl.x0, pl.y0, 0.0, 0.03], [pl.x1, pl.y0, 0.0, 0.03], 0.002, PLUNGER, SegKind::Plunger));
        let plunger = PlungerState { x: pl.x, y0: pl.y0, travel: pl.travel, pos: 0.0, vel: 0.0, pull: 0.0, held: false, firing: false, stop_next: false, seg: segs.len() - 1 };

        Physics {
            balls: vec![], time: 0.0, events: vec![],
            gravity_scale: 1.0, drift: 0.0, bumper_kick: 1.15, sling_kick: 1.25,
            flipper_power: 1.0, flipper_power_l: 1.0, flipper_power_r: 1.0, flipper_return: 1.0, ramp_blocked: false, bad_balls: 0,
            slope: l.slope_deg.to_radians(),
            segs, circles, flippers, bumpers, slings, drops, standups, gates, spinners, plunger, sensors,
            ramp_floors, ramp_block, kicks: vec![], next_ball: 1,
            lane_x: l.lane_x(), bounds: [l.x_min() - 0.06, l.x_max() + 0.04, -0.1, l.top + 0.15],
        }
    }

    // ------------------------------------------------------------------ balls & controls
    pub fn add_ball(&mut self, x: f64, y: f64) -> u32 {
        let id = self.next_ball;
        self.next_ball += 1;
        self.balls.push(Ball {
            id, x, y, z: R, vx: 0.0, vy: 0.0, vz: 0.0, wx: 0.0, wy: 0.0, wz: 0.0, px: x, py: y, pz: R, roll: 0.0,
            inside: HashSet::new(), spin_side: vec![None; self.spinners.len()], on_floor: false, alive: true, still: 0.0, anchor: (x, y), rescues: 0,
        });
        id
    }
    pub fn serve_ball(&mut self) -> u32 {
        let (x, y) = (self.plunger.x, self.plunger.y0 + R + 0.003);
        self.add_ball(x, y)
    }
    pub fn in_lane(lane_x: f64, b: &Ball) -> bool { b.x > lane_x && b.y < 0.2 }
    pub fn ball_in_lane(&self, b: &Ball) -> bool { Self::in_lane(self.lane_x, b) }
    pub fn any_in_lane(&self) -> bool { self.balls.iter().any(|b| self.ball_in_lane(b)) }

    /// every flipper on one button: -1 left, +1 right
    pub fn set_side(&mut self, side: i8, on: bool) {
        for i in 0..self.flippers.len() { if self.flippers[i].side == side { self.set_flipper(i, on); } }
    }
    pub fn set_flipper(&mut self, i: usize, on: bool) {
        if self.flippers[i].on != on {
            self.flippers[i].on = on;
            self.events.push(if on { Ev::FlipperUp(i) } else { Ev::FlipperDown(i) });
        }
    }
    pub fn pull_plunger(&mut self, held: bool) {
        self.plunger.held = held;
        if !held && self.plunger.pull > 0.0 { self.release_plunger(None); }
    }
    pub fn release_plunger(&mut self, strength: Option<f64>) {
        let p = &mut self.plunger;
        let s = strength.unwrap_or(p.pull);
        if strength.is_some() { p.pos = p.pos.min(-s * p.travel); } // auto-launch starts from a pulled position
        p.vel = 0.5 + 2.7 * s;
        p.firing = true;
        p.pull = 0.0;
        p.held = false;
        self.events.push(Ev::Plunger { strength: s });
    }
    pub fn nudge(&mut self, dx: f64, dy: f64) {
        for b in &mut self.balls { b.vx += dx; b.vy += dy; }
        self.events.push(Ev::Nudge);
    }
    pub fn reset_drop(&mut self, i: usize) {
        let d = &mut self.drops[i];
        d.down = false;
        self.segs[d.seg].on = true;
    }
    pub fn reset_drops(&mut self) {
        for d in &mut self.drops { d.down = false; self.segs[d.seg].on = true; }
    }

    // ------------------------------------------------------------------ simulation
    pub fn step(&mut self) {
        let dt = STEP;
        self.time += dt;
        let gs = 9.81 * self.gravity_scale;
        let (gx, gy, gz) = (gs * self.drift, -gs * self.slope.sin(), -gs * self.slope.cos());
        for b in &mut self.balls { b.px = b.x; b.py = b.y; b.pz = b.z; }
        for f in &mut self.flippers { f.prev_ang = f.ang; }
        self.fire_kicks();
        self.update_flippers(dt);
        self.update_plunger(dt);
        for i in 0..self.spinners.len() { self.update_spinner(i, dt); }
        for b in &mut self.bumpers { b.cool -= dt; }
        for s in &mut self.slings { s.cool -= dt; }
        for s in &mut self.standups { s.cool -= dt; }
        for g in &mut self.gates { g.swing *= 1.0 - dt * 6.0; }
        for &rb in &self.ramp_block { self.segs[rb].on = self.ramp_blocked; }

        let mut balls = std::mem::take(&mut self.balls);
        for b in balls.iter_mut() {
            if !b.alive { continue; }
            b.vx += gx * dt; b.vy += gy * dt; b.vz += gz * dt;
            b.x += b.vx * dt; b.y += b.vy * dt; b.z += b.vz * dt;
            b.on_floor = false;
            self.collide_ball(b);
            if b.on_floor {
                // rolling resistance and spin decay
                let sp = (b.vx * b.vx + b.vy * b.vy).sqrt();
                // only on a rolling ball: a ball balanced on a post tip must still be able to roll off
                if sp > 0.02 {
                    let dec = (0.09 * dt).min(sp) / sp;
                    b.vx -= b.vx * dec;
                    b.vy -= b.vy * dec;
                }
                b.wz *= 1.0 - 0.8 * dt;
            }
            let v = b.speed();
            if v > 7.0 { let k = 7.0 / v; b.vx *= k; b.vy *= k; b.vz *= k; }
            b.roll += (b.vx * b.vx + b.vy * b.vy).sqrt() * dt / R;
            self.check_sensors(b);
            let broken = !(b.x.is_finite() && b.y.is_finite() && b.z.is_finite() && b.vx.is_finite() && b.vy.is_finite() && b.vz.is_finite());
            if broken { self.bad_balls += 1; }
            // a ball with a non-finite state would otherwise vanish without ever draining
            let [bx0, bx1, by0, by1] = self.bounds;
            if broken || (b.y < DRAIN_Y && b.x < self.lane_x) || b.z < -0.05 || b.y > by1 || b.y < by0 || b.x < bx0 || b.x > bx1 {
                b.alive = false;
                self.events.push(Ev::Drain { ball: b.id });
            }
        }
        for i in 0..balls.len() {
            for j in i + 1..balls.len() {
                let (l, r) = balls.split_at_mut(j);
                self.collide_balls(&mut l[i], &mut r[0]);
            }
        }
        balls.retain(|b| b.alive);
        self.balls = balls;
    }

    fn update_flippers(&mut self, dt: f64) {
        let ret = self.flipper_return;
        for f in self.flippers.iter_mut() {
            let pw = self.flipper_power * if f.side < 0 { self.flipper_power_l } else { self.flipper_power_r };
            if f.on {
                f.w += f.dir * f.accel * pw * dt;
                let max = f.max_w * pw.sqrt();
                if f.w * f.dir > max { f.w = max * f.dir; }
                f.ang += f.w * dt;
                if (f.ang - f.up) * f.dir >= 0.0 { f.ang = f.up; f.w = 0.0; }
                if (f.ang - f.rest) * f.dir < 0.0 { f.ang = f.rest; if f.w * f.dir < 0.0 { f.w = 0.0; } }
            } else {
                f.w -= f.dir * f.ret_accel * ret * dt;
                if -f.w * f.dir > f.ret_max_w * ret { f.w = -f.ret_max_w * ret * f.dir; }
                f.ang += f.w * dt;
                if (f.ang - f.rest) * f.dir <= 0.0 { f.ang = f.rest; f.w = 0.0; }
            }
            f.pinned = f.w == 0.0;
        }
    }

    fn update_plunger(&mut self, dt: f64) {
        let p = &mut self.plunger;
        if p.stop_next { p.vel = 0.0; p.stop_next = false; }
        if p.held && !p.firing {
            p.pull = (p.pull + dt * 1.1).min(1.0);
            p.pos = -p.pull * p.travel;
            p.vel = 0.0;
        } else if p.firing {
            p.pos += p.vel * dt;
            // keep the velocity for this step's contact, stop on the next
            if p.pos >= 0.0 { p.pos = 0.0; p.firing = false; p.stop_next = true; }
        } else if !p.stop_next {
            p.pos = 0.0;
            p.vel = 0.0;
        }
        let y = p.y0 + p.pos;
        let s = &mut self.segs[p.seg];
        s.ay = y;
        s.by = y;
        s.refresh();
    }

    fn update_spinner(&mut self, i: usize, dt: f64) {
        let s = &mut self.spinners[i];
        if s.w != 0.0 {
            let sign = s.w.signum();
            let da = s.w * dt;
            s.angle += da;
            s.acc += da.abs();
            if !s.acc.is_finite() || !s.w.is_finite() { s.acc = 0.0; s.w = 0.0; }
            let mut guard = 0;
            while s.acc >= std::f64::consts::TAU && guard < 8 {
                s.acc -= std::f64::consts::TAU;
                self.events.push(Ev::Spin { id: i });
                guard += 1;
            }
            s.w -= sign * 38.0 * dt;
            if s.w.signum() != sign { s.w = 0.0; }
        } else {
            let tgt = (s.angle / std::f64::consts::TAU).round() * std::f64::consts::TAU;
            s.angle += (tgt - s.angle) * (dt * 8.0).min(1.0);
        }
    }

    fn fire_kicks(&mut self) {
        if self.kicks.is_empty() { return; }
        let now = self.time;
        let kicks = std::mem::take(&mut self.kicks);
        for k in kicks {
            if k.t > now { self.kicks.push(k); continue; }
            let Some(b) = self.balls.iter_mut().find(|b| b.id == k.ball && b.alive) else { continue };
            match k.kind {
                KickKind::Bumper(i) => {
                    let bs = &self.bumpers[i];
                    let (mut dx, mut dy) = (b.x - bs.x, b.y - bs.y);
                    let d = (dx * dx + dy * dy).sqrt().max(1e-9);
                    // the ring only catches a ball still against the skirt
                    if d < bs.r + R + 0.006 {
                        dx /= d;
                        dy /= d;
                        let out = b.vx * dx + b.vy * dy;
                        let target = (out + self.bumper_kick * 0.6).max(self.bumper_kick + 0.35);
                        b.vx += dx * (target - out);
                        b.vy += dy * (target - out);
                    }
                    self.events.push(Ev::Bumper { id: i, x: b.x, y: b.y });
                }
                KickKind::Sling { id, nx, ny } => {
                    let s = &self.segs[self.slings[id].kick_seg];
                    let t = clamp(((b.x - s.ax) * s.dx + (b.y - s.ay) * s.dy) / s.len2, 0.0, 1.0);
                    let (qx, qy) = (s.ax + s.dx * t, s.ay + s.dy * t);
                    if ((b.x - qx).powi(2) + (b.y - qy).powi(2)).sqrt() < s.r + R + 0.008 {
                        let out = b.vx * nx + b.vy * ny;
                        let target = out.max(0.0) + self.sling_kick;
                        b.vx += nx * (target - out);
                        b.vy += ny * (target - out);
                    }
                    self.events.push(Ev::Sling { id, x: b.x, y: b.y });
                }
            }
        }
    }

    /// Contact against a (possibly moving) surface of infinite mass. Returns (impact speed, normal impulse).
    fn resolve(b: &mut Ball, n: [f64; 3], depth: f64, sv: [f64; 3], mat: &Mat, mass_ratio: f64) -> Option<(f64, f64)> {
        let [nx, ny, nz] = n;
        b.x += nx * depth; b.y += ny * depth; b.z += nz * depth;
        let (rvx, rvy, rvz) = (b.vx - sv[0], b.vy - sv[1], b.vz - sv[2]);
        let vn = rvx * nx + rvy * ny + rvz * nz;
        if vn >= 0.0 { return None; }
        let mut e = restitution(mat, -vn);
        if -vn < 0.06 { e = 0.0; }
        let jn = (-(1.0 + e) * vn) / (1.0 + mass_ratio);
        b.vx += jn * nx; b.vy += jn * ny; b.vz += jn * nz;
        // friction at the contact point, coupled with spin
        let (rvx, rvy, rvz) = (b.vx - sv[0], b.vy - sv[1], b.vz - sv[2]);
        let (rx, ry, rz) = (-nx * R, -ny * R, -nz * R);
        let mut cx = rvx + (b.wy * rz - b.wz * ry);
        let mut cy = rvy + (b.wz * rx - b.wx * rz);
        let mut cz = rvz + (b.wx * ry - b.wy * rx);
        let cn = cx * nx + cy * ny + cz * nz;
        cx -= cn * nx; cy -= cn * ny; cz -= cn * nz;
        let vt = (cx * cx + cy * cy + cz * cz).sqrt();
        if vt > 1e-7 {
            let jt = (vt * (2.0 / 7.0)).min(mat.mu * jn);
            let (ux, uy, uz) = (cx / vt, cy / vt, cz / vt);
            b.vx -= ux * jt; b.vy -= uy * jt; b.vz -= uz * jt;
            let k = jt / (0.4 * R);
            b.wx += (ny * uz - nz * uy) * k;
            b.wy += (nz * ux - nx * uz) * k;
            b.wz += (nx * uy - ny * ux) * k;
        }
        Some((-vn, jn))
    }

    fn collide_ball(&mut self, b: &mut Ball) {
        if b.z < R {
            Self::resolve(b, [0.0, 0.0, 1.0], R - b.z, [0.0; 3], &FLOOR, 0.0);
            b.on_floor = true;
        }
        for rf in &self.ramp_floors {
            if Self::collide_ramp_floor(b, rf) { b.on_floor = true; }
        }
        for si in 0..self.segs.len() {
            let s = &self.segs[si];
            if !s.on || b.x < s.minx || b.x > s.maxx || b.y < s.miny || b.y > s.maxy { continue; }
            let t = clamp(((b.x - s.ax) * s.dx + (b.y - s.ay) * s.dy) / s.len2, 0.0, 1.0);
            let (qx, qy) = (s.ax + s.dx * t, s.ay + s.dy * t);
            let (mut dx, mut dy) = (b.x - qx, b.y - qy);
            let d2 = dx * dx + dy * dy;
            let rr = R + s.r;
            if d2 >= rr * rr { continue; }
            let zb = s.zb0 + (s.zb1 - s.zb0) * t;
            let zt = s.zt0 + (s.zt1 - s.zt0) * t;
            if b.z < zb - R * 0.3 || b.z > zt + R * 0.4 { continue; }
            let d = d2.sqrt();
            if d < 1e-9 { continue; }
            dx /= d;
            dy /= d;
            let (kind, mat) = (s.kind, s.mat);
            // past the gate's ends it is just a rounded wire end, not a one-way shelf
            if let (SegKind::Gate(_), true) = (kind, t <= 0.0 || t >= 1.0) {
                Self::resolve(b, [dx, dy, 0.0], rr - d, [0.0; 3], &mat, 0.0);
                continue;
            }
            if let SegKind::Gate(gi) = kind {
                let g = &mut self.gates[gi];
                let side = (b.x - qx) * g.dir[0] + (b.y - qy) * g.dir[1];
                let along = b.vx * g.dir[0] + b.vy * g.dir[1];
                if side < 0.0 || along > 0.0 {
                    g.swing = g.swing.max((along.abs() * 0.8).min(1.2));
                    continue;
                }
                let dir = g.dir;
                Self::resolve(b, [dir[0], dir[1], 0.0], (rr - side).max(0.0), [0.0; 3], &mat, 0.0);
                continue;
            }
            let svy = if kind == SegKind::Plunger { self.plunger.vel } else { 0.0 };
            if let Some((v, _)) = Self::resolve(b, [dx, dy, 0.0], rr - d, [0.0, svy, 0.0], &mat, 0.0) {
                self.on_seg_hit(b, si, v, dx, dy);
            }
        }
        for ci in 0..self.circles.len() {
            let c = &self.circles[ci];
            let (mut dx, mut dy) = (b.x - c.x, b.y - c.y);
            let rr = R + c.r;
            if dx.abs() > rr || dy.abs() > rr { continue; }
            let d2 = dx * dx + dy * dy;
            if d2 >= rr * rr { continue; }
            if b.z < c.zb - R * 0.3 || b.z > c.zt + R * 0.4 { continue; }
            let d = d2.sqrt().max(1e-9);
            dx /= d;
            dy /= d;
            let (kind, mat) = (c.kind, c.mat);
            let hit = Self::resolve(b, [dx, dy, 0.0], rr - d, [0.0; 3], &mat, 0.0);
            match kind {
                CircleKind::Bumper(i) => {
                    if self.bumpers[i].cool <= 0.0 {
                        // the skirt switch closes; the coil pulls the ring down a few ms later
                        self.bumpers[i].cool = 0.09;
                        self.kicks.push(Kick { t: self.time + 0.005, ball: b.id, kind: KickKind::Bumper(i) });
                    }
                }
                CircleKind::Post => {
                    if let Some((v, _)) = hit { if v > 0.25 { self.events.push(Ev::Rubber { v }); } }
                }
            }
        }
        for fi in 0..self.flippers.len() { self.collide_flipper(b, fi); }
    }

    fn collide_ramp_floor(b: &mut Ball, rf: &RampFloor) -> bool {
        let mut best: Option<(&RampSeg, f64)> = None;
        let mut best_lat = f64::INFINITY;
        for s in &rf.segs {
            let (rx, ry) = (b.x - s.ax, b.y - s.ay);
            let t = rx * s.tx + ry * s.ty;
            if t < -0.002 || t > s.len + 0.002 { continue; }
            let lat = (rx * -s.ty + ry * s.tx).abs();
            if lat > rf.half_w || lat >= best_lat { continue; }
            best_lat = lat;
            best = Some((s, clamp(t, 0.0, s.len)));
        }
        let Some((s, t)) = best else { return false };
        let h = s.ah + s.slope * t;
        if h < 0.001 { return false; } // flush with the playfield
        let nl = (s.slope * s.slope + 1.0).sqrt();
        let n = [-s.tx * s.slope / nl, -s.ty * s.slope / nl, 1.0 / nl];
        let dist = (b.z - h) * n[2];
        if dist >= 0.0 && dist < R {
            Self::resolve(b, n, R - dist, [0.0; 3], &PLASTIC, 0.0);
            return true;
        }
        // underneath: the ramp is a thin sheet, push the ball back down
        if dist < 0.0 && b.z + R > h - 0.003 && dist > -R * 2.5 {
            Self::resolve(b, [-n[0], -n[1], -n[2]], (b.z + R) - (h - 0.003), [0.0; 3], &PLASTIC, 0.0);
        }
        false
    }

    fn on_seg_hit(&mut self, b: &mut Ball, si: usize, v: f64, nx: f64, ny: f64) {
        match self.segs[si].kind {
            SegKind::Sling(i) => {
                if self.slings[i].cool <= 0.0 && v > 0.15 {
                    self.slings[i].cool = 0.14;
                    self.kicks.push(Kick { t: self.time + 0.007, ball: b.id, kind: KickKind::Sling { id: i, nx, ny } });
                }
            }
            SegKind::Drop(i) => {
                // only a hit on the front face knocks it down
                if !self.drops[i].down && ny < -0.4 && v > 0.12 {
                    self.drops[i].down = true;
                    self.segs[si].on = false;
                    self.events.push(Ev::Drop { id: i, x: b.x, y: b.y });
                }
            }
            SegKind::Standup(i) => {
                let st = &mut self.standups[i];
                let facing = nx * st.n[0] + ny * st.n[1];
                if st.cool <= 0.0 && facing > 0.4 && v > 0.1 {
                    st.cool = 0.25;
                    self.events.push(Ev::Standup { id: i, x: b.x, y: b.y });
                }
            }
            _ => { if v > 0.4 { self.events.push(Ev::Wall { v }); } }
        }
    }

    fn collide_flipper(&mut self, b: &mut Ball, fi: usize) {
        let f = &mut self.flippers[fi];
        let (px, py) = (b.x - f.x, b.y - f.y);
        let reach = f.len + f.r0 + R;
        if px * px + py * py > reach * reach || b.z > f.h + R * 0.6 { return; }
        let (c, s) = (f.ang.cos(), f.ang.sin());
        let u = c * px + s * py; // along the bat
        let v = -s * px + c * py; // across it
        // signed distance to the tapered capsule (iq's uneven capsule)
        let (ax, ay) = (v.abs(), u);
        let bb = (f.r0 - f.r1) / f.len;
        let aa = (1.0 - bb * bb).sqrt();
        let k = -bb * ax + aa * ay;
        let (dist, gx, gy);
        if k < 0.0 {
            let l = (ax * ax + ay * ay).sqrt().max(1e-9);
            dist = l - f.r0; gx = ax / l; gy = ay / l;
        } else if k > aa * f.len {
            let l = (ax * ax + (ay - f.len).powi(2)).sqrt().max(1e-9);
            dist = l - f.r1; gx = ax / l; gy = (ay - f.len) / l;
        } else {
            dist = ax * aa + ay * bb - f.r0; gx = aa; gy = bb;
        }
        if dist >= R { return; }
        let (nu, nv) = (gy, gx * if v < 0.0 { -1.0 } else { 1.0 });
        let (nx, ny) = (c * nu - s * nv, s * nu + c * nv);
        let cx = b.x - nx * dist.max(0.0) - f.x;
        let cy = b.y - ny * dist.max(0.0) - f.y;
        let (svx, svy) = (-f.w * cy, f.w * cx);
        let rn = cx * ny - cy * nx;
        let mass_ratio = if f.pinned { 0.0 } else { BALL_MASS * rn * rn / f.inertia };
        let rel_vn = (b.vx - svx) * nx + (b.vy - svy) * ny;
        if let Some((_, jn)) = Self::resolve(b, [nx, ny, 0.0], R - dist, [svx, svy, 0.0], &FLIPPER, mass_ratio) {
            if !f.pinned {
                // reaction torque slows the bat
                f.w -= jn * BALL_MASS * rn / f.inertia;
                if f.w * f.dir < 0.0 && f.on { f.w = 0.0; }
            }
            if -rel_vn > 0.3 { self.events.push(Ev::FlipperHit { v: -rel_vn }); }
        }
    }

    fn collide_balls(&mut self, a: &mut Ball, b: &mut Ball) {
        let (mut dx, mut dy, mut dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
        let d2 = dx * dx + dy * dy + dz * dz;
        if d2 >= 4.0 * R * R || d2 < 1e-12 { return; }
        let d = d2.sqrt();
        dx /= d; dy /= d; dz /= d;
        let pen = (2.0 * R - d) / 2.0;
        a.x -= dx * pen; a.y -= dy * pen; a.z -= dz * pen;
        b.x += dx * pen; b.y += dy * pen; b.z += dz * pen;
        let vn = (b.vx - a.vx) * dx + (b.vy - a.vy) * dy + (b.vz - a.vz) * dz;
        if vn >= 0.0 { return; }
        let j = -(1.0 + 0.9) * vn / 2.0;
        a.vx -= j * dx; a.vy -= j * dy; a.vz -= j * dz;
        b.vx += j * dx; b.vy += j * dy; b.vz += j * dz;
        if -vn > 0.3 { self.events.push(Ev::BallClick { v: -vn }); }
    }

    fn check_sensors(&mut self, b: &mut Ball) {
        for (i, s) in self.sensors.iter().enumerate() {
            let (dx, dy) = (b.x - s.x, b.y - s.y);
            let inside = dx * dx + dy * dy < s.r * s.r && b.z > s.z0 && b.z < s.z1 + R;
            let was = b.inside.contains(&i);
            if inside && !was {
                b.inside.insert(i);
                self.events.push(if s.rollover { Ev::Rollover { idx: i, x: s.x, y: s.y } } else { Ev::Sensor { idx: i, ball: b.id, vy: b.vy } });
            } else if !inside && was {
                b.inside.remove(&i);
            }
        }
        for (i, sp) in self.spinners.iter_mut().enumerate() {
            if b.x < sp.a[0] || b.x > sp.b[0] || b.z > sp.z {
                b.spin_side[i] = None;
                continue;
            }
            let side = if b.y >= sp.a[1] { 1 } else { -1 };
            if let Some(prev) = b.spin_side[i] {
                if prev != side {
                    sp.w = sp.w.abs().max(b.vy.abs() * 55.0) * if b.vy > 0.0 { 1.0 } else { -1.0 };
                    self.events.push(Ev::SpinnerHit { v: b.vy.abs() });
                }
            }
            b.spin_side[i] = Some(side);
        }
    }
}
