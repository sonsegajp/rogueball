//! Procedural tables. A seed picks the width, the height, a colour theme and the features: what each side
//! wall carries (a ramp return, an orbit lane, an upper flipper, standups), the top lanes, and the toys in
//! the middle (bumpers, drop banks, standups, spinner lanes, rubber posts). The lower playfield (flippers,
//! slings, in/outlanes) is the proven module from the original table, so every machine still plays right.
//! sim.rs then play-tests each layout with a bot before anyone sees it.

use crate::table::*;
use std::f64::consts::PI;

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng { Rng(seed ^ 0x2545_f491_4f6c_dd1d) }
    /// splitmix64
    pub fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    pub fn f(&mut self) -> f64 { (self.u64() >> 11) as f64 / (1u64 << 53) as f64 }
    pub fn range(&mut self, a: f64, b: f64) -> f64 { a + (b - a) * self.f() }
    pub fn chance(&mut self, p: f64) -> bool { self.f() < p }
    /// inclusive
    pub fn int(&mut self, a: usize, b: usize) -> usize { a + (self.u64() % (b - a + 1) as u64) as usize }
}

#[derive(Clone, Copy, Debug)]
pub struct GenParams { pub seed: u64, pub ante: u32, pub boss: bool }

#[derive(Clone, Copy, PartialEq, Debug)]
enum SideKind { Ramp, Orbit, Flipper, Standups, Empty }

/// palette themes for the playfield art (see bake.rs); the last one is kept for boss tables
pub const THEMES: usize = 6;

const INS_LANE: u32 = 0xffd933;
const INS_IN: u32 = 0x4dffe6;
const INS_OUT: u32 = 0xff404d;
const INS_SAVE: u32 = 0xff6633;
const INS_STAND: u32 = 0xff59cc;
const INS_RAMP: u32 = 0xff4dbf;
const INS_ORBIT: u32 = 0x4de6ff;
const INS_COMBO: u32 = 0x8c59ff;
const INS_MB: u32 = 0x4dff80;
const INS_DROP: u32 = 0xffd933;

/// what has been placed on the floor so far, as circles, for spacing checks
struct Space(Vec<(f64, f64, f64)>);
impl Space {
    fn free(&self, x: f64, y: f64, r: f64, gap: f64) -> bool {
        self.0.iter().all(|&(sx, sy, sr)| ((x - sx).powi(2) + (y - sy).powi(2)).sqrt() >= r + sr + gap)
    }
    fn add(&mut self, x: f64, y: f64, r: f64) { self.0.push((x, y, r)); }
}

struct Arch { xc: f64, rx: f64, ry: f64, ya: f64 }
impl Arch {
    fn y(&self, x: f64) -> f64 {
        let u = ((x - self.xc) / self.rx).clamp(-1.0, 1.0);
        self.ya + self.ry * (1.0 - u * u).sqrt()
    }
    /// the lowest point of the arch over an x range
    fn min_y(&self, x0: f64, x1: f64) -> f64 { self.y(x0).min(self.y(x1)) }
}

struct B {
    walls: Vec<Wall>,
    posts: Vec<Post>,
    bumpers: Vec<Bumper>,
    slings: Vec<Sling>,
    flippers: Vec<Flipper>,
    drops: Vec<Target>,
    standups: Vec<Target>,
    spinners: Vec<Spinner>,
    rollovers: Vec<Rollover>,
    gates: Vec<Gate>,
    ramps: Vec<Ramp>,
    sensors: Vec<Sensor>,
    inserts: Vec<Insert>,
    labels: Vec<Label>,
    blocked: Vec<Vec<[f64; 2]>>,
    features: Features,
    space: Space,
    /// shot paths from a flipper to a ramp or orbit mouth, kept clear of toys
    corridors: Vec<([f64; 2], [f64; 2])>,
}

impl B {
    fn rail(&mut self, pts: &[[f64; 2]], zt: f64) { self.walls.push(Wall { pts: line(pts, 0.0, zt), r: 0.003, mat: "metal", vis: "rail" }); }
    fn post(&mut self, x: f64, y: f64, r: f64, zt: f64, mat: &'static str) { self.posts.push(Post { x, y, r, zt, mat }); }
    fn ins(&mut self, id: String, x: f64, y: f64, r: f64, shape: &'static str, rot: f64, color: u32) {
        self.inserts.push(Insert { id, x, y, r, shape, rot, color });
    }
    fn label(&mut self, text: &'static str, x: f64, y: f64) { self.labels.push(Label { text, x, y }); }
}

pub fn generate(p: &GenParams) -> Layout {
    let mut r = Rng::new(p.seed);

    // ---- size: about half the machines are classic width, the rest up to ~1.5x
    let w = r.f();
    let hw = if w < 0.45 { r.range(0.236, 0.26) } else if w < 0.8 { r.range(0.26, 0.31) } else { r.range(0.31, 0.355) };
    let x_min = -hw;
    let x_max = hw + LANE_W;
    let xc = (x_min + x_max) / 2.0;
    let rx = (x_max - x_min) / 2.0;
    let ya = r.range(0.78, 0.93) + (hw - 0.237) * 0.6;
    let ry = r.range(0.2, 0.27).min(rx);
    let arch = Arch { xc, rx, ry, ya };
    let top = ya + ry;
    let theme = if p.boss { THEMES - 1 } else { r.int(0, THEMES - 2) };

    let mut b = B {
        walls: vec![], posts: vec![], bumpers: vec![], slings: vec![], flippers: vec![], drops: vec![], standups: vec![],
        spinners: vec![], rollovers: vec![], gates: vec![], ramps: vec![], sensors: vec![], inserts: vec![], labels: vec![],
        blocked: vec![], features: Features::default(), space: Space(vec![]), corridors: vec![],
    };

    // ---- cabinet: outer wall with the top arch, shoulders down to the outlanes on wide machines
    let wide = hw > 0.241;
    let y_sh = 0.29 + (hw - 0.237) * 0.85;
    let mut outer: Vec<[f64; 2]> = if wide {
        vec![[-0.237, -0.01], [-0.237, 0.29], [-hw, y_sh], [-hw, ya]]
    } else {
        vec![[-hw, -0.01], [-hw, ya]]
    };
    let n_arch = 72;
    for i in 1..n_arch {
        let a = PI * (1.0 - i as f64 / n_arch as f64);
        outer.push([xc + rx * a.cos(), ya + ry * a.sin()]);
    }
    outer.extend_from_slice(&[[x_max, ya], [x_max, -0.01]]);
    b.walls.push(Wall { pts: line(&outer, 0.0, 0.065), r: 0.004, mat: "metal", vis: "outer" });
    if wide {
        // the right outlane's outer wall and shoulder; the lane divider sits further out
        b.rail(&[[0.237, -0.01], [0.237, 0.29], [hw, y_sh]], 0.045);
        b.blocked.push(vec![[-0.237, -0.01], [-0.237, 0.29], [-hw, y_sh], [-hw, -0.01]]);
        b.blocked.push(vec![[0.237, -0.01], [0.237, 0.29], [hw, y_sh], [hw, -0.01]]);
    }
    // shooter lane divider and its one-way gate, slanted so a ball can't rest on it
    b.walls.push(Wall { pts: line(&[[hw, -0.01], [hw, ya - 0.04]], 0.0, 0.055), r: 0.004, mat: "metal", vis: "rail" });
    b.post(hw, ya - 0.04, 0.006, 0.055, "metal");
    b.gates.push(Gate { a: [hw + 0.004, ya - 0.018], b: [hw + 0.034, ya], dir: [-0.514, 0.858] });

    // ---- lower playfield module, mirrored
    for s in [-1.0f64, 1.0] {
        let f = |q: [f64; 2]| [q[0] * -s, q[1]];
        b.rail(&[f([-0.198, 0.275]), f([-0.198, 0.045])], 0.045);
        b.rail(&[f([-0.198, 0.215]), f([-0.0915, 0.147])], 0.045);
        let q = f([-0.198, 0.275]);
        b.post(q[0], q[1], 0.0055, 0.045, "rubber");
        b.slings.push(Sling { side: s as i8, pts: [f([-0.158, 0.335]), f([-0.158, 0.245]), f([-0.106, 0.215])], kick: [0, 2], r: 0.0055, zt: 0.04 });
        let side = if s < 0.0 { "L" } else { "R" };
        b.rollovers.push(Rollover { group: "inlane", set: 0, x: s * 0.18, y: 0.25, r: 0.011 });
        b.rollovers.push(Rollover { group: "outlane", set: 0, x: s * 0.2155, y: 0.19, r: 0.011 });
        b.ins(format!("in:{side}"), s * 0.18, 0.225, 0.007, "circle", 0.0, INS_IN);
        b.ins(format!("out:{side}"), s * 0.2155, 0.165, 0.007, "circle", 0.0, INS_OUT);
        b.flippers.push(Flipper {
            side: s as i8, x: s * 0.088, y: 0.128, len: 0.074, r0: 0.0125, r1: 0.0065,
            rest: if s < 0.0 { -27.0 } else { 207.0 }, up: if s < 0.0 { 30.0 } else { 150.0 }, h: 0.024, main: true,
        });
    }
    b.ins("save".into(), 0.0, 0.19, 0.011, "circle", 0.0, INS_SAVE);
    for i in 0..5 { b.ins(format!("cl:{i}"), 0.0, 0.27 + i as f64 * 0.03, 0.008, "chevron", 90.0, INS_COMBO); }
    b.ins("mb".into(), 0.0, 0.43, 0.012, "circle", 0.0, INS_MB);
    b.label("SHOOT AGAIN", 0.0, 0.167);
    b.label("COMBO", 0.035, 0.33);
    b.label("MULTIBALL", 0.0, 0.452);
    // keep the lower playfield and the combo ladder clear
    b.space.add(0.0, 0.3, 0.12);
    b.space.add(0.0, 0.43, 0.02);
    for s in [-1.0, 1.0] { b.space.add(s * 0.17, 0.3, 0.07); }

    // ---- top lanes, tucked under the arch
    let nl = [2, 3, 3, 4][r.int(0, 3)];
    let sp = 0.06;
    let span = nl as f64 * sp;
    let lo = x_min + 0.07 + span / 2.0;
    let hi = hw - 0.035 - span / 2.0;
    let lane_cx = (xc + r.range(-0.06, 0.06)).clamp(lo.min(hi), hi.max(lo));
    let gx: Vec<f64> = (0..=nl).map(|k| lane_cx - span / 2.0 + k as f64 * sp).collect();
    let y_guide_top = arch.min_y(gx[0], gx[nl]) - 0.09;
    let yl0 = y_guide_top - 0.063;
    let mut lane_idx = vec![];
    for (k, &x) in gx.iter().enumerate() {
        b.rail(&[[x, yl0], [x, yl0 + 0.063]], 0.04);
        b.post(x, yl0, 0.005, 0.04, "rubber");
        if k < nl {
            let lx = x + sp / 2.0;
            lane_idx.push(b.rollovers.len());
            b.rollovers.push(Rollover { group: "top", set: 0, x: lx, y: yl0 + 0.035, r: 0.012 });
            b.ins(format!("lane:0:{k}"), lx, yl0 - 0.02, 0.008, "circle", 0.0, INS_LANE);
        }
    }
    b.features.lanes.push((lane_idx, [lane_cx, yl0 + 0.035]));
    b.label("LANES", lane_cx, yl0 - 0.045);
    b.space.add(lane_cx, yl0 + 0.02, span / 2.0 + 0.03);
    let lanes_x = (gx[0] - 0.03, gx[nl] + 0.03);

    // ---- what each side wall carries
    let mut kinds = [SideKind::Empty; 2];
    let ramp_side = if r.chance(0.78) { Some(if r.chance(0.55) { -1.0 } else { 1.0 }) } else { None };
    for (i, s) in [-1.0f64, 1.0].iter().enumerate() {
        kinds[i] = if ramp_side == Some(*s) {
            SideKind::Ramp
        } else if r.chance(0.6) {
            SideKind::Orbit
        } else if r.chance(0.45) {
            SideKind::Flipper
        } else if r.chance(0.6) {
            SideKind::Standups
        } else {
            SideKind::Empty
        };
    }
    if !kinds.contains(&SideKind::Ramp) && !kinds.contains(&SideKind::Orbit) {
        let i = r.int(0, 1);
        kinds[i] = SideKind::Orbit;
    }

    let mut has_spinner = false;
    for (i, s) in [-1.0f64, 1.0].iter().copied().enumerate() {
        match kinds[i] {
            SideKind::Ramp => {
                if !build_ramp_side(&mut b, &mut r, &arch, s, hw, lanes_x, yl0) { kinds[i] = SideKind::Empty; }
            }
            SideKind::Orbit => {
                // a lane along the wall with a flared mouth that funnels shots in
                let x_w = s * (hw - 0.06);
                // the mouth sits where a cross shot from the opposite flipper (about 55-60 degrees) reaches this wall
                let y0 = (0.15 + (x_w.abs() + 0.03) * r.range(1.45, 1.75)).clamp(0.42, ya - 0.2);
                let y1 = (ya - 0.04).min(y0 + r.range(0.18, 0.28));
                let flare = [s * (hw - 0.09), y0 - 0.035];
                b.rail(&[flare, [x_w, y0], [x_w, y1]], 0.05);
                b.post(flare[0], flare[1], 0.006, 0.05, "rubber");
                b.post(x_w, y1, 0.006, 0.05, "rubber");
                let lx = s * (hw - 0.0305);
                if r.chance(0.65) {
                    let ys = (y0 + y1) / 2.0;
                    b.spinners.push(Spinner { a: [lx - 0.0225, ys], b: [lx + 0.0225, ys], z: 0.042 });
                    has_spinner = true;
                }
                let k = b.features.orbits.len();
                b.sensors.push(Sensor { kind: SensorKind::OrbitEntry(k), x: lx, y: y0 + 0.05, r: 0.02, z0: 0.0, z1: 0.03 });
                let mouth = [s * (hw - 0.045), y0 - 0.06];
                b.ins(format!("orbit:{k}"), mouth[0], mouth[1], 0.012, "arrow", 90.0, INS_ORBIT);
                b.label("ORBIT", mouth[0], mouth[1] - 0.038);
                b.features.orbits.push([xc, top - 0.05]);
                b.space.add(mouth[0], mouth[1], 0.045);
                for fx in [-0.03, 0.03] { b.corridors.push(([fx, 0.17], mouth)); }
            }
            SideKind::Flipper => {
                let yf = r.range(0.5, ya - 0.16);
                let px = s * (hw - 0.032);
                b.flippers.push(Flipper {
                    side: s as i8, x: px, y: yf, len: 0.055, r0: 0.011, r1: 0.006,
                    rest: if s < 0.0 { -27.0 } else { 207.0 }, up: if s < 0.0 { 30.0 } else { 150.0 }, h: 0.024, main: false,
                });
                // a guide off the wall feeds balls rolling down the side onto the flipper
                b.rail(&[[s * (hw - 0.002), yf + 0.07], [px - s * 0.0035, yf + 0.019]], 0.045);
                b.space.add(s * (hw - 0.06), yf - 0.01, 0.065);
            }
            SideKind::Standups => {
                let n = r.int(2, 3);
                let yc = r.range(0.5, ya - 0.12);
                let x = s * (hw - 0.012);
                let mut idx = vec![];
                for k in 0..n {
                    let y = yc + (k as f64 - (n - 1) as f64 / 2.0) * 0.034;
                    idx.push(b.standups.len());
                    // a→b runs so the face (n) is on the right of it, matching the kit art
                    let (ya, yb) = if s < 0.0 { (y - 0.013, y + 0.013) } else { (y + 0.013, y - 0.013) };
                    b.standups.push(Target { a: [x, ya], b: [x, yb], r: 0.004, zt: 0.035, n: [-s, 0.0] });
                    let g = b.features.stands.len();
                    b.ins(format!("stand:{g}:{k}"), x - s * 0.026, y, 0.007, "circle", 0.0, INS_STAND);
                }
                b.features.stands.push((idx, [x - s * 0.03, yc]));
                b.space.add(s * (hw - 0.03), yc, 0.03 + n as f64 * 0.017);
                // a deflector over the top one, so a ball rolling down the wall is turned inward instead of resting on it
                let y_top = yc + (n - 1) as f64 / 2.0 * 0.034 + 0.013;
                b.rail(&[[s * (hw - 0.002), y_top + 0.045], [s * (hw - 0.028), y_top + 0.006]], 0.045);
            }
            SideKind::Empty => {}
        }
    }
    if !b.features.orbits.is_empty() {
        b.sensors.push(Sensor { kind: SensorKind::Top, x: xc, y: top - 0.031, r: 0.03, z0: 0.0, z1: 0.03 });
    }

    // ---- toys in the middle, a ball-width-plus clear of every wall and shot path
    let floor_walls: Vec<(f64, f64, f64)> = b.walls.iter().flat_map(|w| {
        w.pts.windows(2).filter(|p| p[0][2] < 0.02 && p[1][2] < 0.02).flat_map(move |p| {
            let n = ((p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]) / 0.012).ceil().max(1.0) as usize;
            (0..=n).map(move |i| { let t = i as f64 / n as f64; (p[0][0] + (p[1][0] - p[0][0]) * t, p[0][1] + (p[1][1] - p[0][1]) * t, w.r) })
        })
    }).collect();
    for (x, y, rr) in floor_walls { b.space.add(x, y, rr); }
    for q in b.posts.clone() { b.space.add(q.x, q.y, q.r); }
    for (a, c) in b.corridors.clone() {
        let n = ((c[0] - a[0]).hypot(c[1] - a[1]) / 0.03).ceil() as usize;
        for i in 0..=n { let t = i as f64 / n as f64; b.space.add(a[0] + (c[0] - a[0]) * t, a[1] + (c[1] - a[1]) * t, 0.022); }
    }
    let margin = |k: SideKind| if k == SideKind::Empty { 0.05 } else { 0.08 };
    let mx0 = x_min + margin(kinds[0]);
    let mx1 = hw - margin(kinds[1]);
    let my0 = 0.48;
    let my1 = yl0 - 0.03;
    // bumpers, nearly always, in the upper part of the middle
    let nb = [2, 3, 3, 3, 4][r.int(0, 4)];
    let d = r.range(0.098, 0.112);
    let shape: Vec<[f64; 2]> = match nb {
        2 => vec![[-d / 2.0, 0.0], [d / 2.0, 0.0]],
        3 => {
            let flip = if r.chance(0.7) { 1.0 } else { -1.0 };
            vec![[-d / 2.0, d * 0.29 * flip], [d / 2.0, d * 0.29 * flip], [0.0, -d * 0.58 * flip]]
        }
        _ => vec![[-d / 2.0, 0.0], [d / 2.0, 0.0], [0.0, d * 0.8], [0.0, -d * 0.8]],
    };
    let mut placed = false;
    for _ in 0..120 {
        let cx = r.range(mx0 + d * 0.5 + 0.03, mx1 - d * 0.5 - 0.03);
        let cy = r.range(my0 + (my1 - my0) * 0.35, my1 - d * 0.6);
        if shape.iter().all(|q| b.space.free(cx + q[0], cy + q[1], 0.03, 0.035)) {
            let c0 = r.int(0, 2) as u8;
            for (k, q) in shape.iter().enumerate() {
                b.bumpers.push(Bumper { x: cx + q[0], y: cy + q[1], r: 0.03, color: (c0 + k as u8) % 3 });
                b.space.add(cx + q[0], cy + q[1], 0.03);
            }
            placed = true;
            break;
        }
    }
    if !placed {
        // fall back to a single bumper wherever it fits
        for _ in 0..200 {
            let (x, y) = (r.range(mx0, mx1), r.range(my0, my1));
            if b.space.free(x, y, 0.03, 0.035) { b.bumpers.push(Bumper { x, y, r: 0.03, color: 0 }); b.space.add(x, y, 0.03); break; }
        }
    }

    // drop banks: one usually, two on wide machines sometimes
    let n_banks = if r.chance(0.85) { 1 } else { 0 } + if hw > 0.3 && r.chance(0.5) { 1 } else { 0 };
    for _ in 0..n_banks {
        let n = r.int(2, if hw > 0.27 { 5 } else { 4 });
        // never level: a flat bank is a shelf a ball comes to rest against from behind
        let th: f64 = [-30.0, -20.0, -10.0, 10.0, 20.0, 30.0][r.int(0, 5)];
        let (c, s) = (th.to_radians().cos(), th.to_radians().sin());
        let half = n as f64 * 0.0165;
        for _ in 0..120 {
            let cx = r.range(mx0 + half, mx1 - half);
            let cy = r.range(my0, my1 - 0.03);
            let pts: Vec<[f64; 2]> = (0..n).map(|i| {
                let o = (i as f64 - (n - 1) as f64 / 2.0) * 0.033;
                [cx + o * c, cy + o * s]
            }).collect();
            // room in front of the bank to hit it
            let (fx, fy) = (cx + s * 0.05, cy - c * 0.05);
            if pts.iter().all(|q| b.space.free(q[0], q[1], 0.018, 0.03)) && b.space.free(fx, fy, 0.02, 0.0) {
                let g = b.features.banks.len();
                let mut idx = vec![];
                for (i, q) in pts.iter().enumerate() {
                    idx.push(b.drops.len());
                    b.drops.push(Target {
                        a: [q[0] - 0.0145 * c, q[1] - 0.0145 * s], b: [q[0] + 0.0145 * c, q[1] + 0.0145 * s],
                        r: 0.004, zt: 0.035, n: [s, -c],
                    });
                    b.ins(format!("drop:{g}:{i}"), q[0] + s * 0.025, q[1] - c * 0.025, 0.008, "circle", 0.0, INS_DROP);
                    b.space.add(q[0], q[1], 0.018);
                }
                b.features.banks.push((idx, [cx, cy + 0.03]));
                b.label("BANK", cx + s * 0.058, cy - c * 0.058);
                b.space.add(fx, fy, 0.02);
                break;
            }
        }
    }

    // a free-standing standup set aimed at the flippers
    if b.features.stands.is_empty() || r.chance(0.35) {
        let n = r.int(1, 2);
        for _ in 0..120 {
            let cx = r.range(mx0, mx1);
            let cy = r.range(my0, my1);
            // face toward one flipper, in 15 degree steps so the art has a matching rotation, and never
            // straight down: a level back is a shelf a ball comes to rest against
            let fx = if r.chance(0.5) { -0.05 } else { 0.05 };
            let mut face = ((0.16 - cy).atan2(fx - cx).to_degrees() / 15.0).round() * 15.0;
            if (face + 90.0).abs() < 15.0 { face = if face < -90.0 || (face == -90.0 && cx > 0.0) { -105.0 } else { -75.0 }; }
            let (nx, ny) = (face.to_radians().cos(), face.to_radians().sin());
            let (tx, ty) = (-ny, nx);
            let pts: Vec<[f64; 2]> = (0..n).map(|i| {
                let o = (i as f64 - (n - 1) as f64 / 2.0) * 0.034;
                [cx + o * tx, cy + o * ty]
            }).collect();
            if pts.iter().all(|q| b.space.free(q[0], q[1], 0.017, 0.03)) && b.space.free(cx + nx * 0.05, cy + ny * 0.05, 0.02, 0.0) {
                let g = b.features.stands.len();
                let mut idx = vec![];
                for (i, q) in pts.iter().enumerate() {
                    idx.push(b.standups.len());
                    b.standups.push(Target { a: [q[0] - 0.013 * tx, q[1] - 0.013 * ty], b: [q[0] + 0.013 * tx, q[1] + 0.013 * ty], r: 0.004, zt: 0.035, n: [nx, ny] });
                    b.ins(format!("stand:{g}:{i}"), q[0] + nx * 0.024, q[1] + ny * 0.024, 0.007, "circle", 0.0, INS_STAND);
                    b.space.add(q[0], q[1], 0.017);
                }
                b.features.stands.push((idx, [cx, cy]));
                b.label("TARGETS", cx + nx * 0.055, cy + ny * 0.055);
                break;
            }
        }
    }

    // a spinner lane in the open if the sides have none
    if !has_spinner && r.chance(0.55) {
        for _ in 0..80 {
            let cx = r.range(mx0 + 0.03, mx1 - 0.03);
            let cy = r.range(my0, my1 - 0.09);
            if b.space.free(cx, cy + 0.045, 0.06, 0.02) {
                for sx in [-1.0, 1.0] {
                    b.rail(&[[cx + sx * 0.0255, cy], [cx + sx * 0.0255, cy + 0.09]], 0.045);
                    b.post(cx + sx * 0.0255, cy, 0.0055, 0.045, "rubber");
                }
                b.spinners.push(Spinner { a: [cx - 0.0225, cy + 0.045], b: [cx + 0.0225, cy + 0.045], z: 0.042 });
                b.space.add(cx, cy + 0.045, 0.06);
                b.label("SPINNER", cx, cy - 0.022);
                break;
            }
        }
    }

    // a few lone rubber posts to break up the open floor
    for _ in 0..r.int(0, 3) {
        for _ in 0..60 {
            let (x, y) = (r.range(mx0, mx1), r.range(my0, my1));
            if b.space.free(x, y, 0.006, 0.045) {
                b.post(x, y, 0.0055, 0.045, "rubber");
                b.space.add(x, y, 0.006);
                break;
            }
        }
    }

    Layout {
        seed: p.seed,
        ball_r: BALL_R,
        slope_deg: SLOPE_DEG,
        hw, arch_y: ya, top, theme,
        walls: b.walls, posts: b.posts, bumpers: b.bumpers, slings: b.slings, flippers: b.flippers,
        drops: b.drops, standups: b.standups, spinners: b.spinners, rollovers: b.rollovers, gates: b.gates,
        ramps: b.ramps, sensors: b.sensors, inserts: b.inserts, labels: b.labels, blocked: b.blocked,
        outline: outer,
        plunger: Plunger { x: hw + 0.019, y0: 0.0465, travel: 0.045, x0: hw + 0.0045, x1: hw + 0.0335 },
        apron: Apron { x0: -0.233, x1: 0.233, y0: -0.01, y1: 0.052 },
        features: b.features,
    }
}

/// a plastic up-ramp from the middle, a U-turn toward side `s`, and a wire-form back down into that inlane
fn build_ramp_side(b: &mut B, r: &mut Rng, arch: &Arch, s: f64, hw: f64, lanes_x: (f64, f64), yl0: f64) -> bool {
    let xr = s * 0.197;
    let rr = r.range(0.05, 0.07);
    let xe = xr - s * 2.0 * rr;
    let ye = r.range(0.42, 0.46);
    let flat = r.range(0.045, 0.08);
    let mut rise = r.range(0.22, 0.3);
    // the U-turn has to clear the arch and the top lanes
    let (ux0, ux1) = (xe.min(xr) - 0.03, xe.max(xr) + 0.03);
    let mut limit = arch.min_y(ux0, ux1) - 0.05;
    if ux1 > lanes_x.0 && ux0 < lanes_x.1 { limit = limit.min(yl0 - 0.03); }
    let mut y_u = ye + rise + flat;
    if y_u + rr + 0.03 > limit {
        y_u = limit - rr - 0.03;
        rise = y_u - ye - flat;
        if rise < 0.16 { return false; }
    }
    let mut path: Vec<[f64; 3]> = vec![];
    for i in 0..=14 {
        let t = i as f64 / 14.0;
        path.push([xe, ye + t * rise, 0.058 * (t * t * (3.0 - 2.0 * t))]);
    }
    path.push([xe, ye + rise + flat / 2.0, 0.06]);
    path.push([xe, y_u, 0.062]);
    let split = path.len();
    let cx = (xe + xr) / 2.0;
    for i in 1..=16 {
        let t = i as f64 / 16.0;
        let a = if s < 0.0 { PI * t } else { PI * (1.0 - t) };
        path.push([cx + rr * a.cos(), y_u + rr * a.sin(), 0.062 + 0.002 * t]);
    }
    // straight down the side to the low end
    let n_down = (((y_u - 0.05) - 0.45) / 0.06).ceil().max(1.0) as usize;
    for k in 0..=n_down {
        let t = k as f64 / n_down as f64;
        path.push([xr, (y_u - 0.05) + (0.45 - (y_u - 0.05)) * t, 0.064 - 0.006 * t]);
    }
    for q in [[-0.197, 0.425, 0.05], [-0.191, 0.40, 0.034], [-0.184, 0.38, 0.016], [-0.178, 0.362, 0.004], [-0.176, 0.35, 0.0]] {
        path.push([q[0] * -s, q[1], q[2]]);
    }
    let mut ramp = build_ramp(path, 0.023, split, s as i8);
    let solid_from = ramp.path.iter().enumerate().position(|(i, q)| i > split && q[1] <= 0.451).unwrap_or(ramp.path.len());
    for i in solid_from..ramp.path.len() {
        ramp.left[i][2] = 0.0;
        ramp.right[i][2] = 0.0;
    }
    ramp.solid_from = solid_from;
    // cap the tunnel under the up-ramp so nothing rolls in from under the U-turn
    let (lc, rc, hc) = (ramp.left[split - 1], ramp.right[split - 1], ramp.path[split - 1][2]);
    b.walls.push(Wall { pts: vec![[lc[0], lc[1], 0.0, hc - 0.004], [rc[0], rc[1], 0.0, hc - 0.004]], r: 0.003, mat: "plastic", vis: "hidden" });
    b.walls.push(Wall { pts: ramp.left.clone(), r: 0.003, mat: "plastic", vis: "rampwall" });
    b.walls.push(Wall { pts: ramp.right.clone(), r: 0.003, mat: "plastic", vis: "rampwall" });
    // deflector over the low end: balls coming down the side bounce inward instead of wedging
    let yd = 0.452 + (hw - 0.17) * 0.94;
    b.rail(&[[s * (hw - 0.002), yd], [s * 0.168, 0.452]], 0.045);
    b.post(s * 0.168, 0.452, 0.004, 0.045, "metal");
    // guide rail from the ramp exit down to the outlane divider post: the return always feeds the inlane
    let (le, re) = (*ramp.left.last().unwrap(), *ramp.right.last().unwrap());
    let e = if le[0].abs() > re[0].abs() { le } else { re };
    b.rail(&[[e[0], e[1]], [s * 0.1985, 0.31], [s * 0.198, 0.278]], 0.045);
    b.blocked.push(vec![[s * (hw - 0.002), yd], [s * 0.168, 0.452], [s * 0.15, 0.354], [s * 0.198, 0.278], [s * hw, 0.278]]);

    let k = b.ramps.len();
    b.sensors.push(Sensor { kind: SensorKind::RampEnter(k), x: xe, y: ye + 0.04, r: 0.02, z0: 0.0, z1: 0.04 });
    b.sensors.push(Sensor { kind: SensorKind::RampMade(k), x: cx, y: y_u + rr, r: 0.03, z0: 0.05, z1: 0.12 });
    b.ins(format!("ramp:{k}"), xe, ye - 0.04, 0.013, "arrow", 90.0, INS_RAMP);
    b.label("RAMP", xe, ye - 0.078);
    b.features.ramps.push([cx, y_u + rr + 0.04]);
    for fx in [-0.03, 0.03] { b.corridors.push(([fx, 0.17], [xe, ye - 0.02])); }
    // floor footprint: the up-ramp channel, the U-turn, the approach, and the side it comes down
    let mut y = ye - 0.01;
    while y < y_u { b.space.add(xe, y, 0.03); y += 0.03; }
    b.space.add(cx, y_u, rr + 0.03);
    b.space.add(xe, ye - 0.06, 0.045);
    let mut y = 0.35;
    while y < y_u { b.space.add(xr, y, 0.035); y += 0.04; }
    b.ramps.push(ramp);
    true
}
