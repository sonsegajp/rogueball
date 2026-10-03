#![windows_subsystem = "windows"]
//! Rogueball: a pinball roguelike.

use macroquad::prelude::*;
use rogueball::draw::{VIEW_H, VIEW_W};
use rogueball::game::{hex, Game, State};
use rogueball::ui::Mouse;

fn conf() -> Conf {
    Conf {
        window_title: "Rogueball".to_owned(),
        window_width: 1920,
        window_height: 1080,
        window_resizable: true,
        high_dpi: false,
        sample_count: 1,
        ..Default::default()
    }
}

/// where the 960×540 canvas lands on screen: whole-number scale when it fits, letterboxed
fn canvas_rect() -> Rect {
    let (sw, sh) = (screen_width(), screen_height());
    let fit = (sw / VIEW_W).min(sh / VIEW_H);
    // fill the window; whole-number scales only when they lose almost nothing, for the crispest pixels
    let s = if fit >= 2.0 && fit - fit.floor() < 0.08 { fit.floor() } else { fit };
    let (w, h) = (VIEW_W * s, VIEW_H * s);
    Rect::new(((sw - w) / 2.0).floor(), ((sh - h) / 2.0).floor(), w, h)
}

#[macroquad::main(conf)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let shot = args.iter().position(|a| a == "--shot").and_then(|i| args.get(i + 1).cloned());
    let shot_state = args.iter().position(|a| a == "--state").and_then(|i| args.get(i + 1).cloned()).unwrap_or_default();
    let arg_num = |k: &str, d: u32| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(d);
    let shot_at = arg_num("--at", 150);
    let launch_at = arg_num("--launch", 90);

    let rt = render_target(VIEW_W as u32, VIEW_H as u32);
    rt.texture.set_filter(FilterMode::Nearest);
    let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, VIEW_W, VIEW_H));
    cam.render_target = Some(rt.clone());

    rogueball::log::install_panic_hook();
    // the browser build has no OS entropy source for fastrand, so seed it from the clock
    fastrand::seed((macroquad::miniquad::date::now() * 1000.0) as u64);
    rogueball::log::line("start");
    let mut g = Game::new().await;
    let mut bad_seen = 0u32;
    if shot.is_some() {
        match shot_state.as_str() {
            "play" => { g.start_run(); g.play_table(); }
            "shop" => { g.start_run(); g.run.money = 30; g.to_shop(); }
            "select" => { g.start_run(); }
            "rich" | "richshop" => {
                g.start_run();
                for id in ["chrome_dome", "momentum", "piggy_bank", "glass_cannon", "blueprint", "hot_streak", "black_hole", "golden_ticket", "jester", "orbit_oracle", "bumper_cars"] {
                    let d = g.db.item(id);
                    let it = g.run.new_item(d, if id == "jester" { Some(rogueball::run::Edition::Holo) } else { None });
                    g.run.items.push(it);
                }
                for id in ["multiball", "chip_ramp", "gold_leaf", "safety_net", "overclock"] { let d = g.db.card(id); let c = g.run.new_card(d); g.run.cards.push(c); }
                g.run.money = 42;
                if shot_state == "rich" { g.play_table(); } else { g.to_shop(); }
            }
            _ => {}
        }
    }
    let mut frame = 0u32;
    let mut fullscreen = false;
    loop {
        let dt = get_frame_time().min(0.05) as f64;
        let cr = canvas_rect();
        let (mx, my) = mouse_position();
        let m = Mouse {
            pos: vec2((mx - cr.x) / cr.w * VIEW_W, (my - cr.y) / cr.h * VIEW_H),
            click: is_mouse_button_pressed(MouseButton::Left),
            rclick: is_mouse_button_pressed(MouseButton::Right),
            down: is_mouse_button_down(MouseButton::Left),
            released: is_mouse_button_released(MouseButton::Left),
            wheel: mouse_wheel().1,
        };
        let t_frame = macroquad::miniquad::date::now();
        if is_key_pressed(KeyCode::F11) || ((is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt)) && is_key_pressed(KeyCode::Enter)) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        let raw_dt = get_frame_time();
        g.update(dt);
        g.update_scroll(dt as f32);
        g.update_drops(dt as f32);
        let t_update = (macroquad::miniquad::date::now() - t_frame) as f32;

        set_camera(&cam);
        clear_background(hex(0x0b0a14));
        let t = get_time();
        g.draw_table(t);
        g.draw_left(&m);
        g.draw_right(&m);
        g.draw_hud_overlays();
        g.draw_screens(&m);

        set_default_camera();
        clear_background(BLACK);
        draw_texture_ex(&rt.texture, cr.x, cr.y, WHITE, DrawTextureParams { dest_size: Some(vec2(cr.w, cr.h)), flip_y: true, ..Default::default() });

        let t_total = (macroquad::miniquad::date::now() - t_frame) as f32;
        // log hitches: the previous frame gap, and how long this frame's update and draw took
        if raw_dt > 0.15 || t_total > 0.1 {
            let balls: Vec<String> = g.physics.balls.iter().map(|b| format!("({:.3},{:.3},{:.3} v{:.2})", b.x, b.y, b.z, b.speed())).collect();
            rogueball::log::line(&format!("HITCH gap {:.0}ms update {:.0}ms total {:.0}ms state {:?} balls [{}]", raw_dt * 1000.0, t_update * 1000.0, t_total * 1000.0, g.state, balls.join(" ")));
        }
        if g.physics.bad_balls != bad_seen {
            bad_seen = g.physics.bad_balls;
            rogueball::log::line(&format!("ANOMALY non-finite ball removed (total {bad_seen})"));
        }
        frame += 1;
        if let Some(path) = &shot {
            if frame == launch_at && g.state == State::Play {
                // launch the ball for a more interesting capture
                g.physics.release_plunger(Some(0.9));
            }
            if frame == shot_at {
                // the canvas has translucent overlays; the screen shows it over black, so the capture does too
                let mut img = rt.texture.get_texture_data();
                for p in img.bytes.chunks_mut(4) { let a = p[3] as u32; for c in &mut p[..3] { *c = (*c as u32 * a / 255) as u8; } p[3] = 255; }
                img.export_png(path);
                std::process::exit(0);
            }
        }
        next_frame().await;
    }
}
