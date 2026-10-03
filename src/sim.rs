//! Bot play-testing for generated tables. Each candidate layout is played by a simple bot (a handful of
//! games plus an aim sweep from each main flipper) and rejected if a ball gets trapped, a ramp or orbit
//! can't be reached, or balls drain too fast. Work is done in slices so it can run between frames.

use crate::tablegen::{generate, GenParams, Rng};
use crate::physics::{Ev, Physics};
use crate::table::{Layout, SensorKind, BALL_R};

#[derive(Default, Clone, Debug)]
pub struct Report {
    pub games: u32,
    pub stuck: u32,
    pub life: f64,
    pub ramps: u32,
    pub orbits: u32,
    pub drops: u32,
    pub bumpers: u32,
    pub stands: u32,
    pub lanes: u32,
    pub spins: u32,
    pub steps: u64,
    /// where a trapped ball sat, and per aim trial: (flipper, delay, highest point reached)
    pub stuck_at: Option<(f64, f64)>,
    pub aims: Vec<(usize, f64, f64, f64)>,
}

impl Report {
    /// why this layout should be thrown away, if it should
    pub fn problem(&self, l: &Layout) -> Option<String> {
        if self.stuck > 0 { return Some(format!("{} stuck balls", self.stuck)); }
        if !l.ramps.is_empty() && self.ramps == 0 { return Some("ramp never made".into()); }
        if !l.features.orbits.is_empty() && self.orbits == 0 { return Some("orbit never entered".into()); }
        if !l.drops.is_empty() && self.drops == 0 { return Some("drop targets never hit".into()); }
        if self.games > 0 && self.life / (self.games as f64) < 5.0 { return Some(format!("balls drain too fast ({:.1}s)", self.life / self.games as f64)); }
        None
    }
}

enum Trial {
    /// a launched ball played by the bot until it drains or time runs out
    Game { strength: f64 },
    /// a ball cradled on main flipper `f`, released and flipped `delay` seconds later
    Aim { f: usize, delay: f64 },
}

struct Running {
    p: Physics,
    trial: Trial,
    t: f64,
    /// per flipper: held until, cooldown until
    flip: Vec<(f64, f64)>,
    anchor: (f64, f64, f64),
    phase: u8,
    /// highest point the ball reached after the flip
    peak: (f64, f64),
}

pub struct Tester {
    pub layout: Layout,
    trials: Vec<Trial>,
    cur: Option<Running>,
    rng: Rng,
    n_roll: usize,
    pub report: Report,
}

impl Tester {
    pub fn new(layout: Layout) -> Tester {
        let mut rng = Rng::new(layout.seed ^ 0xb07);
        let mut trials = vec![];
        for _ in 0..6 { trials.push(Trial::Game { strength: rng.range(0.55, 1.0) }); }
        let mains: Vec<usize> = layout.flippers.iter().enumerate().filter(|(_, f)| f.main).map(|(i, _)| i).collect();
        for f in mains {
            for k in 0..16 { trials.push(Trial::Aim { f, delay: 0.12 + k as f64 * 0.03 }); }
        }
        trials.reverse();
        let n_roll = layout.rollovers.len();
        Tester { layout, trials, cur: None, rng, n_roll, report: Report::default() }
    }

    /// run up to `budget` physics steps; true once every trial is done
    pub fn work(&mut self, budget: usize) -> bool {
        let mut left = budget;
        while left > 0 {
            if self.cur.is_none() {
                let Some(trial) = self.trials.pop() else { return true };
                self.cur = Some(self.start(trial));
            }
            let done = self.advance(&mut left);
            if done { self.cur = None; }
            // stop early once the verdict is already bad
            if self.report.stuck > 0 { self.trials.clear(); self.cur = None; return true; }
        }
        self.trials.is_empty() && self.cur.is_none()
    }

    fn start(&mut self, trial: Trial) -> Running {
        let mut p = Physics::new(&self.layout);
        let nf = p.flippers.len();
        match trial {
            Trial::Game { .. } => { p.serve_ball(); }
            Trial::Aim { f, .. } => {
                let fl = &p.flippers[f];
                // drop it onto the held flipper from the inlane above it
                let s = fl.side as f64;
                p.add_ball(s * 0.18, 0.29);
                p.set_flipper(f, true);
            }
        }
        Running { p, trial, t: 0.0, flip: vec![(0.0, 0.0); nf], anchor: (0.0, 0.0, 0.0), phase: 0, peak: (0.0, 0.0) }
    }

    /// returns true when the running trial is finished
    fn advance(&mut self, left: &mut usize) -> bool {
        let n_roll = self.n_roll;
        let run = self.cur.as_mut().unwrap();
        while *left > 0 {
            *left -= 1;
            self.report.steps += 1;
            let dt = crate::physics::STEP;
            run.t += dt;
            match run.trial {
                Trial::Game { strength } => {
                    if run.phase == 0 && run.t > 0.4 { run.p.release_plunger(Some(strength)); run.phase = 1; }
                    bot(&mut run.p, &mut run.flip, run.t, &mut self.rng);
                }
                Trial::Aim { f, delay } => {
                    // settle on the held flipper, then let go and flip after `delay`
                    if run.phase == 0 && run.t > 2.0 { run.p.set_flipper(f, false); run.phase = 1; run.anchor.2 = run.t; }
                    if run.phase == 1 && run.t > run.anchor.2 + delay { run.p.set_flipper(f, true); run.phase = 2; run.anchor.2 = run.t; }
                    if run.phase == 2 && run.t > run.anchor.2 + 0.25 { run.p.set_flipper(f, false); run.phase = 3; }
                }
            }
            run.p.step();
            if run.phase >= 2 { if let Some(b) = run.p.balls.first() { if b.y > run.peak.1 { run.peak = (b.x, b.y); } } }
            for e in run.p.events.drain(..) {
                match e {
                    Ev::Bumper { .. } => self.report.bumpers += 1,
                    Ev::Drop { .. } => self.report.drops += 1,
                    Ev::Standup { .. } => self.report.stands += 1,
                    Ev::Spin { .. } => self.report.spins += 1,
                    Ev::Rollover { idx, .. } => { if self.layout.rollovers[idx].group == "top" { self.report.lanes += 1; } }
                    Ev::Sensor { idx, vy, .. } => match self.layout.sensors[idx - n_roll].kind {
                        SensorKind::RampMade(_) => self.report.ramps += 1,
                        SensorKind::OrbitEntry(_) if vy > 0.15 => self.report.orbits += 1,
                        _ => {}
                    },
                    _ => {}
                }
            }
            // trapped: a ball that hasn't moved for 3 seconds outside the lane and off a held flipper
            if let Some(b) = run.p.balls.first() {
                let lane = run.p.ball_in_lane(b);
                let cradled = run.p.flippers.iter().any(|f| f.on && ((b.x - f.x).powi(2) + (b.y - f.y).powi(2)).sqrt() < f.len + 0.03);
                let moved = ((b.x - run.anchor.0).powi(2) + (b.y - run.anchor.1).powi(2)).sqrt();
                if lane || cradled || moved > 0.012 {
                    run.anchor.0 = b.x;
                    run.anchor.1 = b.y;
                    if matches!(run.trial, Trial::Game { .. }) { run.anchor.2 = run.t; }
                } else if matches!(run.trial, Trial::Game { .. }) && run.t - run.anchor.2 > 3.0 {
                    self.report.stuck += 1;
                    self.report.stuck_at = Some((b.x, b.y));
                    return true;
                }
            }
            let over = match run.trial {
                Trial::Game { .. } => run.p.balls.is_empty() || run.t > 35.0,
                Trial::Aim { .. } => run.p.balls.is_empty() || (run.phase == 3 && run.t > run.anchor.2 + 3.0),
            };
            if over {
                if let Trial::Game { .. } = run.trial { self.report.games += 1; self.report.life += run.t - 0.4; }
                if let Trial::Aim { f, delay } = run.trial { self.report.aims.push((f, delay, run.peak.0, run.peak.1)); }
                return true;
            }
        }
        false
    }
}

/// flip any flipper a ball is about to reach, with a little random timing so shots vary
pub fn bot(p: &mut Physics, flip: &mut [(f64, f64)], t: f64, rng: &mut Rng) {
    for i in 0..p.flippers.len() {
        let (held_until, cool_until) = flip[i];
        let f = &p.flippers[i];
        let near = p.balls.iter().any(|b| {
            let d = ((b.x - f.x).powi(2) + (b.y - f.y).powi(2)).sqrt();
            d < f.len + BALL_R + 0.012 && b.y < f.y + 0.05 && b.vy < 0.3
        });
        if f.on {
            if t > held_until { p.set_flipper(i, false); flip[i].1 = t + 0.18; }
        } else if near && t > cool_until && rng.chance(0.08) {
            p.set_flipper(i, true);
            flip[i].0 = t + rng.range(0.1, 0.22);
        }
    }
}

/// generate a table for these params, play-testing candidates until one passes (or the best of a few)
pub fn make_table(params: GenParams, max_tries: u32) -> (Layout, Report, u32) {
    let mut best: Option<(Layout, Report)> = None;
    for k in 0..max_tries {
        let l = generate(&GenParams { seed: params.seed.wrapping_add(k as u64 * 7919), ..params });
        let mut t = Tester::new(l);
        while !t.work(50_000) {}
        let ok = t.report.problem(&t.layout).is_none();
        if ok { return (t.layout, t.report, k + 1); }
        if best.is_none() { best = Some((t.layout, t.report)); }
    }
    let (l, r) = best.unwrap();
    (l, r, max_tries)
}

/// builds the tables a run is about to need in the background, a few milliseconds per frame
pub struct Factory {
    ready: std::collections::HashMap<(u32, u32), Layout>,
    queue: std::collections::VecDeque<((u32, u32), GenParams)>,
    cur: Option<Job>,
}

struct Job { key: (u32, u32), params: GenParams, tries: u32, tester: Tester, best: Option<Layout> }

const MAX_TRIES: u32 = 8;

fn candidate(params: GenParams, tries: u32) -> Tester {
    Tester::new(generate(&GenParams { seed: params.seed.wrapping_add(tries as u64 * 7919), ..params }))
}

impl Factory {
    pub fn new() -> Factory { Factory { ready: Default::default(), queue: Default::default(), cur: None } }
    pub fn clear(&mut self) { self.ready.clear(); self.queue.clear(); self.cur = None; }
    pub fn want(&mut self, key: (u32, u32), params: GenParams) {
        if self.ready.contains_key(&key) || self.queue.iter().any(|(k, _)| *k == key) || self.cur.as_ref().map(|j| j.key == key).unwrap_or(false) { return; }
        self.queue.push_back((key, params));
    }
    pub fn get(&self, key: (u32, u32)) -> Option<&Layout> { self.ready.get(&key) }
    pub fn take(&mut self, key: (u32, u32)) -> Option<Layout> { self.ready.remove(&key) }
    pub fn busy(&self) -> bool { self.cur.is_some() || !self.queue.is_empty() }

    /// spend about `ms` milliseconds generating and testing
    pub fn work(&mut self, ms: f64) {
        let t0 = macroquad::miniquad::date::now();
        loop {
            if self.cur.is_none() {
                let Some((key, params)) = self.queue.pop_front() else { return };
                self.cur = Some(Job { key, params, tries: 0, tester: candidate(params, 0), best: None });
            }
            let job = self.cur.as_mut().unwrap();
            if job.tester.work(2500) {
                let ok = job.tester.report.problem(&job.tester.layout).is_none();
                job.tries += 1;
                if ok || job.tries >= MAX_TRIES {
                    let next = candidate(job.params, job.tries);
                    let done = std::mem::replace(&mut job.tester, next);
                    let layout = if ok { done.layout } else { job.best.take().unwrap_or(done.layout) };
                    let key = job.key;
                    self.ready.insert(key, layout);
                    self.cur = None;
                } else {
                    let next = candidate(job.params, job.tries);
                    let done = std::mem::replace(&mut job.tester, next);
                    if job.best.is_none() { job.best = Some(done.layout); }
                }
            }
            if (macroquad::miniquad::date::now() - t0) * 1000.0 > ms { return; }
        }
    }

    /// block until this table is ready
    pub fn finish(&mut self, key: (u32, u32), params: GenParams) -> Layout {
        self.want(key, params);
        // move it to the front of the queue
        if let Some(i) = self.queue.iter().position(|(k, _)| *k == key) {
            let item = self.queue.remove(i).unwrap();
            if self.cur.as_ref().map(|j| j.key != key).unwrap_or(false) {
                let cur = self.cur.take().unwrap();
                self.queue.push_front((cur.key, cur.params));
            }
            self.queue.push_front(item);
        }
        while !self.ready.contains_key(&key) { self.work(1000.0); }
        self.ready.remove(&key).unwrap()
    }
}
