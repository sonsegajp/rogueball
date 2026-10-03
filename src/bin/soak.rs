//! Physics soak test: many simulated minutes of random play, looking for hangs, non-finite state and wedged balls.
use rogueball::physics::Physics;
use rogueball::tablegen::{generate, GenParams};
use std::time::Instant;

fn main() {
    let mut worst_step = 0.0f64;
    let (mut slow1, mut total_steps, mut total_time) = (0u64, 0u64, 0.0f64);
    let (mut drains, mut bad, mut wedged) = (0, 0, 0);
    let runs = 60;
    for run in 0..runs {
        // a different generated table every run
        let l = generate(&GenParams { seed: 77 + run as u64, ante: 1, boss: false });
        let mut p = Physics::new(&l);
        p.serve_ball();
        let mut launch_at = 300;
        let mut still = 0.0;
        let (mut ax, mut ay) = (0.0, 0.0);
        for step in 0..60_000 {
            if step % 37 == 0 { p.set_flipper(0, fastrand::f64() < 0.4); }
            if step % 41 == 0 { p.set_flipper(1, fastrand::f64() < 0.4); }
            if step % 2500 == 0 && fastrand::f64() < 0.3 { p.nudge((fastrand::f64() - 0.5) * 0.5, 0.1); }
            if step % 9000 == 4500 && p.balls.len() < 3 && !p.any_in_lane() { p.serve_ball(); launch_at = step + 300; }
            let t = Instant::now();
            p.step();
            let e = t.elapsed().as_secs_f64();
            worst_step = worst_step.max(e);
            total_steps += 1; total_time += e;
            if e > 0.001 { slow1 += 1; }
            p.events.clear();
            if p.balls.is_empty() { drains += 1; p.serve_ball(); launch_at = step + 300; }
            if step == launch_at { p.release_plunger(Some(0.35 + fastrand::f64() * 0.65)); }
            // relaunch a ball that fell back down the shooter lane
            if step > launch_at + 2500 && p.balls.iter().any(|b| p.ball_in_lane(b) && b.speed() < 0.01) { launch_at = step + 1; }
            if let Some(b) = p.balls.first() {
                if ((b.x - ax).powi(2) + (b.y - ay).powi(2)).sqrt() > 0.012 { ax = b.x; ay = b.y; still = 0.0; } else { still += 0.001; }
                if still > 6.0 && b.x < p.lane_x {
                    wedged += 1;
                    println!("run {run} step {step}: wedged at ({:.3},{:.3},{:.3}) speed {:.3}", b.x, b.y, b.z, b.speed());
                    still = 0.0;
                    p.balls.clear();
                }
            }
        }
        bad += p.bad_balls;
    }
    println!("{runs} runs × 60s: drains {drains}, non-finite {bad}, wedged {wedged}, slowest step {:.3} ms, mean {:.4} ms, steps over 1ms: {} of {}", worst_step * 1000.0, total_time / total_steps as f64 * 1000.0, slow1, total_steps);
}
