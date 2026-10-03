//! Table layout: the single source of truth for the physics, the rules and the art.
//! Units are meters. x = right, y = up the table (away from the player), z = up off the playfield.
//! Layouts are made by gen.rs; this file holds the types and the shared geometry helpers.

use serde::Serialize;

pub const BALL_R: f64 = 0.0135; // standard 27mm ball
pub const SLOPE_DEG: f64 = 7.0;
pub const DRAIN_Y: f64 = 0.045; // the ball is gone once it passes under the apron
pub const LANE_W: f64 = 0.038; // shooter lane width

#[derive(Serialize, Clone)]
pub struct Wall {
    /// polyline points: x, y, bottom z, top z
    pub pts: Vec<[f64; 4]>,
    pub r: f64,
    pub mat: &'static str,
    /// how it's drawn: outer (cabinet), rail (steel guide), rampwall, hidden
    pub vis: &'static str,
}
#[derive(Serialize, Clone)]
pub struct Post { pub x: f64, pub y: f64, pub r: f64, pub zt: f64, pub mat: &'static str }
#[derive(Serialize, Clone)]
pub struct Bumper { pub x: f64, pub y: f64, pub r: f64, pub color: u8 }
#[derive(Serialize, Clone)]
pub struct Sling { pub side: i8, pub pts: [[f64; 2]; 3], pub kick: [usize; 2], pub r: f64, pub zt: f64 }
#[derive(Serialize, Clone)]
pub struct Flipper {
    /// -1 left button, +1 right button
    pub side: i8,
    pub x: f64, pub y: f64,
    pub len: f64, pub r0: f64, pub r1: f64,
    /// degrees, counter-clockwise from +x
    pub rest: f64, pub up: f64,
    pub h: f64,
    /// one of the pair at the bottom (the others are upper flippers)
    pub main: bool,
}
#[derive(Serialize, Clone)]
pub struct Target { pub a: [f64; 2], pub b: [f64; 2], pub r: f64, pub zt: f64, pub n: [f64; 2] }
#[derive(Serialize, Clone)]
pub struct Spinner { pub a: [f64; 2], pub b: [f64; 2], pub z: f64 }
#[derive(Serialize, Clone)]
pub struct Rollover { pub group: &'static str, pub set: usize, pub x: f64, pub y: f64, pub r: f64 }
#[derive(Serialize, Clone)]
pub struct Gate { pub a: [f64; 2], pub b: [f64; 2], pub dir: [f64; 2] }
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
pub enum SensorKind { OrbitEntry(usize), Top, RampEnter(usize), RampMade(usize) }
#[derive(Serialize, Clone)]
pub struct Sensor { pub kind: SensorKind, pub x: f64, pub y: f64, pub r: f64, pub z0: f64, pub z1: f64 }
#[derive(Serialize, Clone)]
pub struct Insert { pub id: String, pub x: f64, pub y: f64, pub r: f64, pub shape: &'static str, pub rot: f64, pub color: u32 }
#[derive(Serialize, Clone)]
pub struct Ramp {
    pub path: Vec<[f64; 3]>,
    pub half_w: f64,
    pub left: Vec<[f64; 4]>,
    pub right: Vec<[f64; 4]>,
    /// plastic up-ramp before this index, wire-form after
    pub split: usize,
    /// from here the descending end is solid down to the playfield
    pub solid_from: usize,
    /// which inlane it returns to: -1 left, +1 right
    pub side: i8,
}
#[derive(Serialize, Clone)]
pub struct Plunger { pub x: f64, pub y0: f64, pub travel: f64, pub x0: f64, pub x1: f64 }
#[derive(Serialize, Clone)]
pub struct Apron { pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64 }
#[derive(Serialize, Clone)]
pub struct Label { pub text: &'static str, pub x: f64, pub y: f64 }

/// groups of parts that score together, so the rules work on any generated table
#[derive(Serialize, Clone, Default)]
pub struct Features {
    /// drop target banks: indices into `drops`, and where the bank shot scores
    pub banks: Vec<(Vec<usize>, [f64; 2])>,
    /// standup sets: indices into `standups`
    pub stands: Vec<(Vec<usize>, [f64; 2])>,
    /// top rollover lane sets: indices into `rollovers`
    pub lanes: Vec<(Vec<usize>, [f64; 2])>,
    /// where each ramp and orbit shot scores
    pub ramps: Vec<[f64; 2]>,
    pub orbits: Vec<[f64; 2]>,
}

#[derive(Serialize, Clone)]
pub struct Layout {
    pub seed: u64,
    pub ball_r: f64,
    pub slope_deg: f64,
    /// playfield half-width: the left wall is at -hw, the shooter lane divider at +hw
    pub hw: f64,
    /// where the side walls end and the top arch begins, and the top of the arch
    pub arch_y: f64,
    pub top: f64,
    pub theme: usize,
    pub walls: Vec<Wall>,
    pub posts: Vec<Post>,
    pub bumpers: Vec<Bumper>,
    pub slings: Vec<Sling>,
    pub flippers: Vec<Flipper>,
    pub drops: Vec<Target>,
    pub standups: Vec<Target>,
    pub spinners: Vec<Spinner>,
    pub rollovers: Vec<Rollover>,
    pub gates: Vec<Gate>,
    pub ramps: Vec<Ramp>,
    pub sensors: Vec<Sensor>,
    pub inserts: Vec<Insert>,
    pub labels: Vec<Label>,
    /// closed-off floor areas (behind shoulders, under ramp ends), drawn as solid cabinet
    pub blocked: Vec<Vec<[f64; 2]>>,
    pub outline: Vec<[f64; 2]>,
    pub plunger: Plunger,
    pub apron: Apron,
    pub features: Features,
}

impl Layout {
    /// right of this is the shooter lane
    pub fn lane_x(&self) -> f64 { self.hw }
    pub fn x_min(&self) -> f64 { -self.hw }
    pub fn x_max(&self) -> f64 { self.hw + LANE_W }
    pub fn in_lane(&self, x: f64, y: f64) -> bool { x > self.hw && y < 0.2 }
}

pub fn arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64, n: usize) -> Vec<[f64; 2]> {
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians();
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

pub fn line(pts: &[[f64; 2]], zb: f64, zt: f64) -> Vec<[f64; 4]> {
    pts.iter().map(|p| [p[0], p[1], zb, zt]).collect()
}

pub fn build_ramp(path: Vec<[f64; 3]>, half_w: f64, split: usize, side: i8) -> Ramp {
    let mut left = vec![];
    let mut right = vec![];
    let n = path.len();
    for i in 0..n {
        let p = path[i];
        let a = path[i.saturating_sub(1)];
        let b = path[(i + 1).min(n - 1)];
        let (mut tx, mut ty) = (b[0] - a[0], b[1] - a[1]);
        let tl = (tx * tx + ty * ty).sqrt();
        tx /= tl;
        ty /= tl;
        let (nx, ny) = (-ty, tx);
        let h = p[2];
        // walls reach the floor wherever the ramp is too low to roll under
        let zb = if h < 0.035 || i < split { 0.0 } else { (h - 0.004).max(0.03) };
        let zt = h + 0.03;
        let o = half_w + 0.003;
        left.push([p[0] + nx * o, p[1] + ny * o, zb, zt]);
        right.push([p[0] - nx * o, p[1] - ny * o, zb, zt]);
    }
    Ramp { path, half_w, left, right, split, solid_from: n, side }
}
