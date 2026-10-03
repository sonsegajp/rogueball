//! Synthesized sound effects: every sound is generated at startup from oscillators and filtered noise.

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use std::collections::HashMap;

use crate::chip::{crunch, pulse, vol4, wav as chip_wav, wave, wrap, Lfsr, SR, WAVE_ORGAN, WAVE_TRI};

/// Sine and Tri play on the 4-bit wave channel, Square and Saw on the pulse channels (50% and 25% duty)
#[derive(Clone, Copy)]
enum Wave { Sine, Square, Saw, Tri }

struct Buf(Vec<f32>);
impl Buf {
    fn new(sec: f32) -> Buf { Buf(vec![0.0; (sec * SR) as usize]) }
    fn tone(&mut self, freq: f32, dur: f32, wave_: Wave, vol: f32, slide: f32, delay: f32) {
        let start = (delay * SR) as usize;
        let n = (dur * SR) as usize;
        let tick = (SR / 64.0) as usize;
        let mut ph = 0.0f32;
        let (mut env, mut f) = (0.0f32, freq);
        for i in 0..n {
            // envelope at the PSG's 64 Hz tick in 16 steps; pitch slides step like the sweep unit
            if i % tick == 0 {
                let t = i as f32 / n as f32;
                env = vol4((1.0 - t).powf(1.6));
                if slide != 0.0 { f = freq * slide.powf(t); }
            }
            wrap(&mut ph, f);
            let s = match wave_ {
                Wave::Sine => wave(ph, &WAVE_TRI),
                Wave::Tri => wave(ph, &WAVE_ORGAN),
                Wave::Square => pulse(ph, 0.5),
                Wave::Saw => pulse(ph, 0.25),
            };
            let att = (i as f32 / (0.002 * SR)).min(1.0);
            if let Some(v) = self.0.get_mut(start + i) { *v += s * vol * env * att * 0.8; }
        }
    }
    /// noise burst from the PSG noise channel; higher `freq` clocks it faster, high `q` picks the metallic short mode
    fn hit(&mut self, dur: f32, vol: f32, freq: f32, q: f32, delay: f32) {
        let start = (delay * SR) as usize;
        let n = (dur * SR) as usize;
        let tick = (SR / 64.0) as usize;
        let mut lf = Lfsr::new(q >= 2.5);
        let mut env = 0.0f32;
        for i in 0..n {
            if i % tick == 0 { env = vol4((1.0 - i as f32 / n as f32).powf(2.5)); }
            let s = lf.next(freq * 8.0);
            if let Some(v) = self.0.get_mut(start + i) { *v += s * vol * env * 0.55; }
        }
    }
    fn wav(&mut self) -> Vec<u8> {
        crunch(&mut self.0);
        chip_wav(&self.0)
    }
}

fn recipe(name: &str) -> Buf {
    use Wave::*;
    let mut b = Buf::new(1.2);
    match name {
        "flipper" => { b.hit(0.06, 0.35, 900.0, 0.8, 0.0); b.tone(90.0, 0.08, Sine, 0.4, 0.5, 0.0); }
        "flipper_down" => { b.hit(0.04, 0.12, 600.0, 1.0, 0.0); }
        "bumper" => { b.tone(180.0, 0.12, Square, 0.2, 0.4, 0.0); b.hit(0.08, 0.35, 3000.0, 2.0, 0.0); }
        "sling" => { b.hit(0.07, 0.4, 1500.0, 1.5, 0.0); b.tone(120.0, 0.08, Tri, 0.3, 0.6, 0.0); }
        "drop" => { b.hit(0.05, 0.3, 2500.0, 1.0, 0.0); b.tone(660.0, 0.18, Tri, 0.15, 0.0, 0.0); }
        "standup" => { b.hit(0.04, 0.3, 3500.0, 1.0, 0.0); b.tone(880.0, 0.12, Tri, 0.12, 0.0, 0.0); }
        "rollover" => { b.tone(1320.0, 0.12, Sine, 0.15, 0.0, 0.0); b.tone(1760.0, 0.12, Sine, 0.12, 0.0, 0.05); }
        "spin" => { b.tone(2200.0, 0.025, Square, 0.05, 0.0, 0.0); }
        "ramp" => for (i, f) in [523.0, 659.0, 784.0, 1047.0].iter().enumerate() { b.tone(*f, 0.14, Square, 0.08, 0.0, i as f32 * 0.06); },
        "orbit" => for (i, f) in [392.0, 523.0, 659.0, 784.0].iter().enumerate() { b.tone(*f, 0.12, Tri, 0.14, 0.0, i as f32 * 0.05); },
        "lanes" => for (i, f) in [784.0, 988.0, 1175.0, 1568.0].iter().enumerate() { b.tone(*f, 0.15, Square, 0.07, 0.0, i as f32 * 0.07); },
        "bank" => for (i, f) in [330.0, 440.0, 554.0, 659.0, 880.0].iter().enumerate() { b.tone(*f, 0.15, Saw, 0.06, 0.0, i as f32 * 0.06); },
        "combo" => for (i, f) in [659.0, 831.0, 988.0, 1319.0].iter().enumerate() { b.tone(*f, 0.2, Saw, 0.07, 0.0, i as f32 * 0.05); },
        "wall" => { b.hit(0.03, 0.18, 4000.0, 3.0, 0.0); }
        "rubber" => { b.hit(0.04, 0.15, 700.0, 1.0, 0.0); }
        "click" => { b.hit(0.03, 0.25, 5000.0, 4.0, 0.0); }
        "plunger" => { b.hit(0.12, 0.35, 400.0, 1.0, 0.0); b.tone(70.0, 0.15, Sine, 0.4, 0.6, 0.0); }
        "drain" => { b.tone(392.0, 0.5, Saw, 0.12, 0.35, 0.0); }
        "save" => for (i, f) in [880.0, 1175.0, 880.0, 1175.0].iter().enumerate() { b.tone(*f, 0.1, Square, 0.07, 0.0, i as f32 * 0.08); },
        "multiball" => for (i, f) in [523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0].iter().enumerate() { b.tone(*f, 0.18, Square, 0.07, 0.0, i as f32 * 0.07); },
        "tilt" => { b.tone(110.0, 0.8, Saw, 0.2, 0.5, 0.0); }
        "warn" => { b.tone(220.0, 0.15, Square, 0.1, 0.0, 0.0); }
        "nudge" => { b.hit(0.1, 0.3, 200.0, 1.0, 0.0); }
        "coin" => { b.tone(988.0, 0.08, Square, 0.08, 0.0, 0.0); b.tone(1319.0, 0.2, Square, 0.08, 0.0, 0.07); }
        "mult" => { b.tone(700.0, 0.09, Square, 0.07, 1.3, 0.0); }
        "xmult" => { b.tone(300.0, 0.25, Saw, 0.1, 3.0, 0.0); }
        "pts" => { b.tone(900.0, 0.05, Tri, 0.08, 0.0, 0.0); }
        "score" => for (i, f) in [262.0, 330.0, 392.0, 523.0].iter().enumerate() { b.tone(*f, 0.25, Tri, 0.14, 0.0, i as f32 * 0.04); },
        "win" => for (i, f) in [523.0, 659.0, 784.0, 1047.0, 784.0, 1047.0, 1319.0].iter().enumerate() { b.tone(*f, 0.22, Square, 0.07, 0.0, i as f32 * 0.1); },
        "lose" => for (i, f) in [392.0, 330.0, 262.0, 196.0].iter().enumerate() { b.tone(*f, 0.4, Saw, 0.09, 0.0, i as f32 * 0.22); },
        "buy" => { b.tone(660.0, 0.08, Square, 0.09, 0.0, 0.0); b.tone(990.0, 0.12, Square, 0.09, 0.0, 0.06); }
        "sell" => { b.tone(990.0, 0.08, Square, 0.09, 0.0, 0.0); b.tone(660.0, 0.12, Square, 0.09, 0.0, 0.06); }
        "ui" => { b.tone(1200.0, 0.03, Square, 0.05, 0.0, 0.0); }
        "deny" => { b.tone(140.0, 0.15, Square, 0.09, 0.0, 0.0); }
        "shatter" => { b.hit(0.4, 0.35, 6000.0, 0.5, 0.0); }
        _ => {}
    }
    // trim trailing silence
    let last = b.0.iter().rposition(|v| v.abs() > 1e-4).unwrap_or(0);
    b.0.truncate(last + 1);
    b
}

const NAMES: &[&str] = &[
    "flipper", "flipper_down", "bumper", "sling", "drop", "standup", "rollover", "spin", "ramp", "orbit", "lanes", "bank", "combo",
    "wall", "rubber", "click", "plunger", "drain", "save", "multiball", "tilt", "warn", "nudge", "coin", "mult", "xmult", "pts",
    "score", "win", "lose", "buy", "sell", "ui", "deny", "shatter",
];

pub struct Sfx {
    sounds: HashMap<&'static str, Sound>,
    last: HashMap<&'static str, f64>,
    pub volume: f32,
}

impl Sfx {
    pub async fn load() -> Sfx {
        let mut sounds = HashMap::new();
        for n in NAMES {
            if let Ok(s) = load_sound_from_bytes(&recipe(n).wav()).await {
                sounds.insert(*n, s);
            }
        }
        Sfx { sounds, last: HashMap::new(), volume: 0.6 }
    }
    pub fn play(&mut self, name: &'static str) { self.play_v(name, 1.0); }
    pub fn play_v(&mut self, name: &'static str, v: f32) {
        let now = macroquad::time::get_time();
        if let Some(t) = self.last.get(name) { if now - t < 0.025 { return; } }
        self.last.insert(name, now);
        if let Some(s) = self.sounds.get(name) {
            play_sound(s, PlaySoundParams { looped: false, volume: (self.volume * v).clamp(0.0, 1.0) });
        }
    }
}
