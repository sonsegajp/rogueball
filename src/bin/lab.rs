//! Physics and generator lab.  cargo run --release --bin lab -- <mode>
//!   --gen N        generate and bot-test N tables; pass rate, failure reasons, timing
//!   --feel SEED    flip exit speeds, cradling and a stuck-ball scan on one generated table
//!   --export SEED  write build/layout.json for a generated table

use rogueball::physics::Physics;
use rogueball::sim::Tester;
use rogueball::table::BALL_R;
use rogueball::tablegen::{generate, GenParams};
use std::collections::BTreeMap;

fn run(p: &mut Physics, secs: f64, mut each: impl FnMut(&mut Physics, f64)) {
    for i in 0..(secs * 1000.0) as usize {
        each(p, i as f64 / 1000.0);
        p.step();
    }
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    if let Some(n) = arg("--gen") {
        let n: u64 = n.parse().unwrap_or(20);
        let mut reasons: BTreeMap<String, u32> = BTreeMap::new();
        let (mut pass, mut steps, mut ms_total) = (0, 0u64, 0.0);
        let mut feats: BTreeMap<&str, u32> = BTreeMap::new();
        for seed in 1..=n {
            let t0 = std::time::Instant::now();
            let l = generate(&GenParams { seed: seed * 1_000_003, ante: 1, boss: seed % 3 == 0 });
            let gen_ms = t0.elapsed().as_secs_f64() * 1000.0;
            let mut t = Tester::new(l);
            while !t.work(50_000) {}
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            ms_total += ms;
            steps += t.report.steps;
            let l = &t.layout;
            *feats.entry("ramps").or_default() += l.ramps.len() as u32;
            *feats.entry("orbits").or_default() += l.features.orbits.len() as u32;
            *feats.entry("upper flippers").or_default() += l.flippers.iter().filter(|f| !f.main).count() as u32;
            *feats.entry("banks").or_default() += l.features.banks.len() as u32;
            *feats.entry("bumpers").or_default() += l.bumpers.len() as u32;
            *feats.entry("spinners").or_default() += l.spinners.len() as u32;
            match t.report.problem(l) {
                None => pass += 1,
                Some(why) => {
                    let key = why.split('(').next().unwrap().trim().to_string();
                    *reasons.entry(key).or_default() += 1;
                }
            }
            let r = &t.report;
            println!("seed {seed:3}: hw {:.3} h {:.2} ramps {} orbits {} upper {} | {:>5.0} ms (gen {gen_ms:.1}) life {:.1}s ramps {} orbits {} drops {} bumps {} stuck {} {}",
                l.hw, l.top, l.ramps.len(), l.features.orbits.len(), l.flippers.iter().filter(|f| !f.main).count(),
                ms, r.life / r.games.max(1) as f64, r.ramps, r.orbits, r.drops, r.bumpers, r.stuck,
                r.problem(l).map(|w| format!("FAIL: {w}")).unwrap_or_else(|| "ok".into()));
        }
        println!("\n{pass}/{n} passed. mean {:.0} ms per candidate, {:.2} us per physics step", ms_total / n as f64, ms_total * 1000.0 / steps as f64);
        println!("failures: {reasons:?}");
        println!("features over {n} tables: {feats:?}");
        return;
    }

    if let Some(seed) = arg("--diag") {
        let sd = seed.parse::<u64>().unwrap();
        let l = generate(&GenParams { seed: sd * 1_000_003, ante: 1, boss: sd % 3 == 0 });
        println!("hw {:.3} arch {:.3} top {:.3}", l.hw, l.arch_y, l.top);
        for s in &l.sensors { println!("  sensor {:?} at ({:.3},{:.3})", s.kind, s.x, s.y); }
        for w in &l.walls { if w.vis == "rail" { println!("  rail {:?}", w.pts.iter().map(|q| format!("({:.3},{:.3})", q[0], q[1])).collect::<Vec<_>>()); } }
        for b in &l.bumpers { println!("  bumper ({:.3},{:.3})", b.x, b.y); }
        for d in &l.drops { println!("  drop ({:.3},{:.3})", (d.a[0]+d.b[0])/2.0, (d.a[1]+d.b[1])/2.0); }
        for f in &l.flippers { println!("  flipper side {} at ({:.3},{:.3}) main {}", f.side, f.x, f.y, f.main); }
        let mut t = Tester::new(l);
        while !t.work(50_000) {}
        for (f, d, x, y) in &t.report.aims { println!("  aim flipper {f} delay {d:.3}: peak ({x:.3},{y:.3})"); }
        println!("{:?}", t.report.problem(&t.layout));
        println!("stuck at {:?}", t.report.stuck_at);
        plot(&t.layout, t.report.stuck_at, &format!("build/plot_{sd}.png"));
        return;
    }
    let seed: u64 = arg("--feel").or_else(|| arg("--export")).and_then(|s| s.parse().ok()).unwrap_or(1);
    let l = generate(&GenParams { seed, ante: 1, boss: false });
    if arg("--export").is_some() {
        std::fs::create_dir_all("build").unwrap();
        std::fs::write("build/layout.json", serde_json::to_string(&l).unwrap()).unwrap();
        println!("wrote build/layout.json");
        return;
    }

    println!("--- flip exit speed (ball resting on the lowered flipper)");
    for fi in 0..2 {
        for d in [0.015, 0.03, 0.045, 0.06, 0.07] {
            let mut p = Physics::new(&l);
            let f = &p.flippers[fi];
            let (c, s) = (f.ang.cos(), f.ang.sin());
            let r = f.r0 + (f.r1 - f.r0) * d / f.len;
            let (mut nx, mut ny) = (-s, c);
            if ny < 0.0 { nx = -nx; ny = -ny; }
            let (bx, by) = (f.x + c * d + nx * (r + BALL_R + 0.0005), f.y + s * d + ny * (r + BALL_R + 0.0005));
            p.add_ball(bx, by);
            p.set_flipper(fi, true);
            let mut out: f64 = 0.0;
            run(&mut p, 0.12, |p, _| { if let Some(b) = p.balls.first() { out = out.max((b.vx * b.vx + b.vy * b.vy).sqrt()); } });
            println!("  flipper {fi} d={d}: exit {out:.2} m/s");
        }
    }

    println!("--- cradle on a held flipper");
    {
        let mut p = Physics::new(&l);
        p.add_ball(-0.18, 0.3);
        p.set_flipper(0, true);
        run(&mut p, 4.0, |_, _| {});
        match p.balls.first() {
            Some(b) => println!("  rests at ({:.3},{:.3}) speed {:.3}", b.x, b.y, b.speed()),
            None => println!("  ball drained"),
        }
    }
}

/// top-down plot of a layout (2 px per mm), stuck point in red
fn plot(l: &rogueball::table::Layout, mark: Option<(f64, f64)>, path: &str) {
    use macroquad::prelude::{Color, Image};
    let k = 1000.0;
    let (x0, y1) = (l.x_min() - 0.02, l.top + 0.02);
    let (w, h) = (((l.x_max() + 0.02 - x0) * k) as u32, ((y1 + 0.03) * k) as u32);
    let mut img = Image::gen_image_color(w as u16, h as u16, Color::new(0.08, 0.06, 0.14, 1.0));
    let px = |x: f64, y: f64| (((x - x0) * k) as i32, ((y1 - y) * k) as i32);
    let mut dot = |img: &mut Image, x: i32, y: i32, c: Color| { if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h { img.set_pixel(x as u32, y as u32, c); } };
    let mut seg = |img: &mut Image, a: (f64, f64), b: (f64, f64), c: Color| {
        let n = (((b.0 - a.0).hypot(b.1 - a.1)) * k * 2.0) as i32 + 1;
        for i in 0..=n { let t = i as f64 / n as f64; let (x, y) = px(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t); dot(img, x, y, c); }
    };
    let circ = |img: &mut Image, x: f64, y: f64, r: f64, c: Color, seg: &mut dyn FnMut(&mut Image, (f64, f64), (f64, f64), Color)| {
        for i in 0..24 { let a0 = i as f64 / 24.0 * 6.2832; let a1 = (i + 1) as f64 / 24.0 * 6.2832; seg(img, (x + r * a0.cos(), y + r * a0.sin()), (x + r * a1.cos(), y + r * a1.sin()), c); }
    };
    for wl in &l.walls {
        let c = match wl.vis { "outer" => Color::new(0.6, 0.6, 0.7, 1.0), "rampwall" => Color::new(0.3, 0.8, 1.0, 1.0), "hidden" => Color::new(0.3, 0.3, 0.5, 1.0), _ => Color::new(0.85, 0.85, 0.9, 1.0) };
        for p in wl.pts.windows(2) {
            let c = if p[0][2] > 0.01 { Color::new(0.2, 0.5, 0.7, 1.0) } else { c };
            seg(&mut img, (p[0][0], p[0][1]), (p[1][0], p[1][1]), c);
        }
    }
    for q in &l.posts { circ(&mut img, q.x, q.y, q.r, Color::new(1.0, 1.0, 1.0, 1.0), &mut seg); }
    for b in &l.bumpers { circ(&mut img, b.x, b.y, b.r, Color::new(1.0, 0.4, 0.7, 1.0), &mut seg); }
    for t in l.drops.iter() { seg(&mut img, (t.a[0], t.a[1]), (t.b[0], t.b[1]), Color::new(1.0, 0.8, 0.2, 1.0)); }
    for t in l.standups.iter() { seg(&mut img, (t.a[0], t.a[1]), (t.b[0], t.b[1]), Color::new(1.0, 0.3, 0.8, 1.0)); }
    for s in &l.slings { for i in 0..3 { let (a, b) = (s.pts[i], s.pts[(i + 1) % 3]); seg(&mut img, (a[0], a[1]), (b[0], b[1]), Color::new(1.0, 0.5, 0.5, 1.0)); } }
    for f in &l.flippers { let a = f.rest.to_radians(); seg(&mut img, (f.x, f.y), (f.x + f.len * a.cos(), f.y + f.len * a.sin()), Color::new(0.5, 1.0, 0.5, 1.0)); }
    for s in &l.spinners { seg(&mut img, (s.a[0], s.a[1]), (s.b[0], s.b[1]), Color::new(0.4, 1.0, 1.0, 1.0)); }
    for s in &l.sensors { circ(&mut img, s.x, s.y, s.r, Color::new(0.4, 0.4, 1.0, 1.0), &mut seg); }
    for g in &l.gates { seg(&mut img, (g.a[0], g.a[1]), (g.b[0], g.b[1]), Color::new(1.0, 1.0, 0.0, 1.0)); }
    if let Some((x, y)) = mark { for r in [0.0135, 0.016, 0.02] { circ(&mut img, x, y, r, Color::new(1.0, 0.0, 0.0, 1.0), &mut seg); } }
    img.export_png(path);
    println!("plotted {path}");
}
