//! The soundtrack: a tiny tracker that renders songs written as note text through the GBA-style chip.
//!
//! Note text: one token per grid step. `C5`, `F#4`, `Bb3` play a note, `C4+E4+G4` a chord,
//! `.` holds the previous note, `-` rests, `|` is a bar line (ignored). A trailing `!` accents
//! a note and `'` makes it a ghost note. Drum tokens: k kick, s snare, h hat, o open hat,
//! c crash, t tom, r rim. Chord tracks are generated from a progression and a one-bar pattern
//! of chord degrees (`1 3 5 7`, `^` an octave up, `_` an octave down).

use crate::chip::*;
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use std::collections::HashMap;

const TAU: f32 = std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub enum Inst {
    /// PSG pulse channel: duty cycle, decay time to the sustain level, vibrato depth in semitones
    Pulse { duty: f32, decay: f32, sustain: f32, vib: f32 },
    /// PSG wave channel with a 4-bit table
    Wave(&'static [u8; 32]),
    // PCM "sampled" instruments, crunched to 8 bits
    Strings,
    Brass,
    Piano,
    Slap,
    Bell,
    Kit,
}

const LEAD: Inst = Inst::Pulse { duty: 0.25, decay: 0.35, sustain: 0.6, vib: 0.22 };
const LEAD50: Inst = Inst::Pulse { duty: 0.5, decay: 0.3, sustain: 0.55, vib: 0.18 };
const ARP: Inst = Inst::Pulse { duty: 0.125, decay: 0.12, sustain: 0.25, vib: 0.0 };
const BASS: Inst = Inst::Wave(&WAVE_BASS);

struct Ev { step: usize, len: usize, notes: Vec<f32>, drums: Vec<char>, vel: f32 }

pub struct Track { inst: Inst, vol: f32, steps: usize, evs: Vec<Ev> }

pub struct Song { pub bpm: f32, pub swing: f32, pub bars: usize, pub looped: bool, pub tracks: Vec<Track> }

// ------------------------------------------------------------------ parsing

fn pitch_class(b: &[u8]) -> Option<(i32, usize)> {
    let pc = match b.first()? { b'C' => 0, b'D' => 2, b'E' => 4, b'F' => 5, b'G' => 7, b'A' => 9, b'B' => 11, _ => return None };
    let mut i = 1;
    let mut acc = 0;
    while i < b.len() && (b[i] == b'#' || b[i] == b'b') {
        acc += if b[i] == b'#' { 1 } else { -1 };
        i += 1;
    }
    Some((pc + acc, i))
}

fn note(tok: &str) -> Option<f32> {
    let (pc, i) = pitch_class(tok.as_bytes())?;
    let oct: i32 = tok[i..].parse().ok()?;
    Some((12 * (oct + 1) + pc) as f32)
}

/// expand `[ ... ]xN` repeats into a flat token list
fn expand(text: &str) -> Vec<String> {
    let mut stack: Vec<Vec<String>> = vec![vec![]];
    for t in text.split_whitespace() {
        if t == "[" {
            stack.push(vec![]);
        } else if let Some(n) = t.strip_prefix("]x") {
            let body = stack.pop().expect("unbalanced ]");
            let n: usize = n.parse().expect("repeat count");
            let top = stack.last_mut().unwrap();
            for _ in 0..n { top.extend(body.iter().cloned()); }
        } else if t != "|" {
            stack.last_mut().unwrap().push(t.to_string());
        }
    }
    assert!(stack.len() == 1, "unbalanced [");
    stack.pop().unwrap()
}

fn velocity(t: &str) -> (&str, f32) {
    if let Some(b) = t.strip_suffix('!') { (b, 1.3) } else if let Some(b) = t.strip_suffix('\'') { (b, 0.55) } else { (t, 1.0) }
}

/// shared token walk: `.` extends, `-` rests, anything else becomes an event via `make`
fn walk(toks: &[String], grid: usize, mut make: impl FnMut(&str, usize) -> (Vec<f32>, Vec<char>)) -> (Vec<Ev>, usize) {
    let mut evs: Vec<Ev> = vec![];
    let mut step = 0;
    for t in toks {
        match t.as_str() {
            "." => { if let Some(e) = evs.last_mut() { if e.step + e.len == step { e.len += grid; } } }
            "-" => {}
            _ => {
                let (body, vel) = velocity(t);
                let (notes, drums) = make(body, step);
                evs.push(Ev { step, len: grid, notes, drums, vel });
            }
        }
        step += grid;
    }
    (evs, step)
}

fn track(inst: Inst, vol: f32, grid: usize, text: &str) -> Track {
    // every written bar must fill exactly 16 sixteenth steps
    if text.contains('|') {
        for (i, bar) in text.split('|').enumerate() {
            let n = bar.split_whitespace().count();
            assert!(n == 0 || n * grid == 16, "bar {} has {n} tokens at grid {grid}: {}", i + 1, bar.trim());
        }
    }
    let toks = expand(text);
    let (evs, steps) = walk(&toks, grid, |body, _| {
        let mut notes = vec![];
        let mut drums = vec![];
        for p in body.split('+') {
            if let Some(n) = note(p) { notes.push(n) } else if p.len() == 1 { drums.push(p.chars().next().unwrap()) } else { panic!("bad note token {p:?}") }
        }
        (notes, drums)
    });
    Track { inst, vol, steps, evs }
}

fn chord(name: &str) -> (i32, Vec<i32>) {
    let (pc, i) = pitch_class(name.as_bytes()).unwrap_or_else(|| panic!("bad chord {name:?}"));
    let ints = match &name[i..] {
        "" => vec![0, 4, 7],
        "m" => vec![0, 3, 7],
        "7" => vec![0, 4, 7, 10],
        "maj7" => vec![0, 4, 7, 11],
        "m7" => vec![0, 3, 7, 10],
        "dim" => vec![0, 3, 6],
        "sus4" => vec![0, 5, 7],
        q => panic!("unknown chord quality {q:?}"),
    };
    (pc, ints)
}

fn degree(d: &str, root: i32, ints: &[i32]) -> f32 {
    let ups = d.matches('^').count() as i32;
    let downs = d.matches('_').count() as i32;
    let core = d.trim_end_matches(['^', '_']);
    let iv = match core {
        "1" => 0, "2" => 2, "3" => ints[1], "4" => 5, "5" => ints[2], "6" => 9,
        "7" => *ints.get(3).unwrap_or(&12),
        _ => panic!("bad degree {d:?}"),
    };
    (root + iv + 12 * (ups - downs)) as f32
}

/// a track generated from a chord progression (one token per bar, `F,G` splits a bar) and a one-bar degree pattern
fn chords(inst: Inst, vol: f32, prog: &str, pattern: &str, grid: usize, oct: i32) -> Track {
    let bars: Vec<Vec<(i32, Vec<i32>)>> = prog.split_whitespace().filter(|t| *t != "|").map(|b| b.split(',').map(chord).collect()).collect();
    let pat = expand(pattern);
    assert!(pat.len() * grid == 16, "pattern must fill one bar");
    let mut toks = vec![];
    for _ in 0..bars.len() { toks.extend(pat.iter().cloned()); }
    let (evs, steps) = walk(&toks, grid, |body, step| {
        let bar = &bars[step / 16];
        let (pc, ints) = &bar[(step % 16) * bar.len() / 16];
        let root = 12 * (oct + 1) + pc;
        (body.split('+').map(|d| degree(d, root, ints)).collect(), vec![])
    });
    Track { inst, vol, steps, evs }
}

/// `n` bars of rest for a part that sits out a section
fn rest(bars: usize) -> String { "- - - - - - - - | ".repeat(bars) }

// ------------------------------------------------------------------ voices

fn add(buf: &mut [f32], i: usize, v: f32) { if let Some(x) = buf.get_mut(i) { *x += v; } }

fn release(t: f32, dur: f32, rel: f32) -> f32 { if t <= dur { 1.0 } else { (1.0 - (t - dur) / rel).max(0.0) } }

fn vibrato(f: f32, t: f32, depth: f32) -> f32 {
    if depth <= 0.0 || t < 0.16 { return f; }
    let ramp = ((t - 0.16) / 0.2).min(1.0);
    // small pitch swings: 2^(x/12) ~ 1 + x*ln2/12
    f * (1.0 + depth * ramp * (TAU * 5.5 * (t - 0.16)).sin() * 0.05776)
}

fn voice(inst: Inst, n: f32, dur: f32, vel: f32, start: usize, psg: &mut [f32], pcm: &mut [f32]) {
    let f = midi_freq(n);
    let saw = |p: f32| 2.0 * p - 1.0;
    match inst {
        Inst::Pulse { duty, decay, sustain, vib } => {
            let rel = 0.025;
            let mut ph = 0.0f32;
            let tick = (SR / 64.0) as usize;
            let (mut e, mut fv) = (0.0f32, f);
            for i in 0..((dur + rel) * SR) as usize {
                let t = i as f32 / SR;
                // the PSG envelope ticks at 64 Hz in 16 volume steps
                if i % tick == 0 {
                    e = vol4((sustain + (1.0 - sustain) * (1.0 - t / decay).max(0.0)) * release(t, dur, rel)) * vel * 0.12;
                    fv = vibrato(f, t, vib);
                }
                if t > dur { e = e.min(vol4(release(t, dur, rel)) * vel * 0.12); }
                wrap(&mut ph, fv);
                add(psg, start + i, pulse(ph, duty) * e);
            }
        }
        Inst::Wave(table) => {
            let rel = 0.012;
            let mut ph = 0.0f32;
            for i in 0..((dur + rel) * SR) as usize {
                let t = i as f32 / SR;
                wrap(&mut ph, f);
                let e = release(t, dur, rel) * (1.0 - t.min(2.0) * 0.15);
                add(psg, start + i, wave(ph, table) * vol4(e) * vel * 0.3);
            }
        }
        Inst::Strings => {
            let (att, rel) = (0.07, 0.3);
            let det = [1.0, 1.0042, 0.9958];
            let mut p = [0.0f32, 0.33, 0.67];
            let mut lp = Lp::default();
            for i in 0..((dur + rel) * SR) as usize {
                let t = i as f32 / SR;
                let mut s = 0.0;
                for k in 0..3 { wrap(&mut p[k], f * det[k]); s += saw(p[k]); }
                let e = (t / att).min(1.0) * release(t, dur, rel);
                add(pcm, start + i, lp.run(s / 3.0, 2400.0) * e * vel * 0.24);
            }
        }
        Inst::Brass => {
            let (att, rel) = (0.02, 0.09);
            let mut ph = 0.0f32;
            let mut lp = Lp::default();
            let mut kf = 1.0f32;
            let kd = (-1.0 / (0.08 * SR)).exp();
            for i in 0..((dur + rel) * SR) as usize {
                let t = i as f32 / SR;
                wrap(&mut ph, vibrato(f, t, 0.15));
                kf *= kd;
                let s = 0.7 * saw(ph) + 0.3 * pulse(ph, 0.3);
                let e = (t / att).min(1.0) * release(t, dur, rel);
                add(pcm, start + i, lp.run(s, 1400.0 + 3000.0 * kf) * e * vel * 0.26);
            }
        }
        Inst::Piano => {
            let rel = 0.15;
            let total = (dur + rel).min(2.5);
            let (mut pc, mut pm) = (0.0f32, 0.0f32);
            let (mut amp, mut idx) = (1.0f32, 1.0f32);
            let (ka, ki) = ((-1.0 / (1.0 * SR)).exp(), (-1.0 / (0.22 * SR)).exp());
            for i in 0..(total * SR) as usize {
                let t = i as f32 / SR;
                wrap(&mut pc, f);
                wrap(&mut pm, f);
                amp *= ka;
                idx *= ki;
                let s = sine(pc + (0.35 + 2.0 * idx) * sine(pm) / TAU);
                add(pcm, start + i, s * amp * release(t, dur, rel) * vel * 0.22);
            }
        }
        Inst::Slap => {
            let rel = 0.05;
            let mut ph = 0.0f32;
            let mut lp = Lp::default();
            let (mut kf, mut amp) = (1.0f32, 1.0f32);
            let (kd, ka) = ((-1.0 / (0.07 * SR)).exp(), (-1.0 / (0.7 * SR)).exp());
            for i in 0..((dur + rel) * SR) as usize {
                let t = i as f32 / SR;
                wrap(&mut ph, f);
                kf *= kd;
                amp *= ka;
                let s = 0.6 * saw(ph) + 0.4 * pulse(ph, 0.5);
                add(pcm, start + i, lp.run(s, 250.0 + 2800.0 * kf) * amp * release(t, dur, rel) * vel * 0.5);
            }
        }
        Inst::Bell => {
            let rel = 0.2;
            let total = (dur + rel).min(2.5);
            let (mut pc, mut pm) = (0.0f32, 0.0f32);
            let (mut amp, mut idx) = (1.0f32, 1.0f32);
            let (ka, ki) = ((-1.0 / (0.9 * SR)).exp(), (-1.0 / (0.3 * SR)).exp());
            for i in 0..(total * SR) as usize {
                let t = i as f32 / SR;
                wrap(&mut pc, f);
                wrap(&mut pm, f * 3.5);
                amp *= ka;
                idx *= ki;
                let s = sine(pc + 2.5 * idx * sine(pm) / TAU);
                add(pcm, start + i, s * amp * release(t, dur, rel) * vel * 0.2);
            }
        }
        Inst::Kit => {}
    }
}

fn drum(d: char, vel: f32, start: usize, psg: &mut [f32], pcm: &mut [f32]) {
    let mut nz = Noise::new(start as u32 ^ 0x5bd1e995);
    let len = |s: f32| (s * SR) as usize;
    match d {
        'k' => {
            let mut ph = 0.0f32;
            let (mut sweep, mut amp) = (1.0f32, 1.0f32);
            let (ks, ka) = (decay(0.028), decay(0.16));
            for i in 0..len(0.32) {
                sweep *= ks;
                amp *= ka;
                wrap(&mut ph, 48.0 + 120.0 * sweep);
                let mut s = sine(ph) * amp;
                if i < len(0.004) { s += nz.next() * 0.5; }
                add(pcm, start + i, s * vel * 0.6);
            }
        }
        's' => {
            let (mut lo, mut hi) = (Lp::default(), Lp::default());
            let (mut ph, mut at, mut an) = (0.0f32, 1.0f32, 1.0f32);
            let (kt, kn) = (decay(0.045), decay(0.11));
            for i in 0..len(0.25) {
                wrap(&mut ph, 190.0);
                at *= kt;
                an *= kn;
                let n = nz.next();
                let band = hi.run(n, 6000.0) - lo.run(n, 900.0);
                add(pcm, start + i, (sine(ph) * at * 0.35 + band * an * 0.9) * vel * 0.5);
            }
        }
        'h' | 'o' => {
            let (clock, tau, dur, amp) = if d == 'h' { (60000.0, 0.022, 0.07, 0.1) } else { (45000.0, 0.16, 0.35, 0.08) };
            let mut lf = Lfsr::new(false);
            let tick = (SR / 64.0) as usize;
            let mut e = 0.0;
            for i in 0..len(dur) {
                // the noise channel's envelope steps at 64 Hz
                if i % tick == 0 { e = vol4((-(i as f32 / SR) / tau).exp()) * vel * amp; }
                add(psg, start + i, lf.next(clock) * e);
            }
        }
        'c' => {
            let mut lp = Lp::default();
            let (mut amp, ka) = (1.0f32, decay(0.55));
            for i in 0..len(1.6) {
                amp *= ka;
                add(pcm, start + i, lp.run(nz.next(), 9000.0) * amp * vel * 0.22);
            }
        }
        't' => {
            let mut ph = 0.0f32;
            let (mut sweep, mut amp) = (1.0f32, 1.0f32);
            let (ks, ka) = (decay(0.05), decay(0.22));
            for i in 0..len(0.4) {
                sweep *= ks;
                amp *= ka;
                wrap(&mut ph, 95.0 + 70.0 * sweep);
                add(pcm, start + i, sine(ph) * amp * vel * 0.45);
            }
        }
        'r' => {
            for i in 0..len(0.04) {
                let t = i as f32 / SR;
                let s = pulse(1600.0 * t, 0.5) * (-t / 0.012).exp() * 0.6 + nz.next() * (-t / 0.01).exp() * 0.4;
                add(pcm, start + i, s * vel * 0.18);
            }
        }
        _ => panic!("unknown drum {d:?}"),
    }
}

// ------------------------------------------------------------------ rendering

pub fn render(song: &Song) -> Vec<f32> {
    let step = 60.0 / song.bpm / 4.0;
    let total = song.bars * 16;
    let time = |s: usize| s as f32 * step + if s % 4 == 2 { song.swing * step } else { 0.0 };
    let len = (total as f32 * step * SR) as usize;
    let tail = (3.0 * SR) as usize;
    let mut psg = vec![0.0f32; len + tail];
    let mut pcm = vec![0.0f32; len + tail];
    for tr in &song.tracks {
        let reps = total.div_ceil(tr.steps.max(1));
        for r in 0..reps {
            for e in &tr.evs {
                let s = e.step + r * tr.steps;
                if s >= total { continue; }
                let t0 = time(s);
                let dur = time(s + e.len) - t0;
                let start = (t0 * SR) as usize;
                for &n in &e.notes { voice(tr.inst, n, dur, e.vel * tr.vol, start, &mut psg, &mut pcm); }
                for &d in &e.drums { drum(d, e.vel * tr.vol, start, &mut psg, &mut pcm); }
            }
        }
    }
    if song.looped {
        // whatever rings past the loop point wraps around to the start
        for buf in [&mut psg, &mut pcm] {
            for i in len..buf.len() { let v = buf[i]; buf[i % len] += v; }
            buf.truncate(len);
        }
    }
    crunch(&mut pcm);
    let mut echo = Echo::new(0.35, 0.3);
    if song.looped { for &x in &pcm { echo.run(x); } } // warm the echo up so the loop is seamless
    for x in pcm.iter_mut() { *x = echo.run(*x); }
    let mut mix: Vec<f32> = psg.iter().zip(&pcm).map(|(a, b)| a + b).collect();
    if !song.looped {
        let last = mix.iter().rposition(|v| v.abs() > 1e-3).unwrap_or(0);
        mix.truncate(last + 1);
    }
    let peak = mix.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    let g = 0.9 / peak;
    for v in mix.iter_mut() { *v = soft_clip(*v * g); }
    mix
}

// ------------------------------------------------------------------ the songs

fn title() -> Song {
    let prog = "Dm Bb C A Dm Bb C A  Dm Bb C A Dm Bb Gm A  Gm Dm Bb C Gm Dm Bb A";
    let lead = "A4 D5 F5 A5 . . G5 F5 | G5 . F5 D5 . . Bb4 D5 | E5 . C5 E5 G5 . F5 E5 | C#5 . . . A4 . - - |
                A4 D5 F5 A5 . . C6 A5 | Bb5 . A5 F5 . . D5 F5 | G5 . E5 C5 E5 G5 C6 . | A5 . . . . . - - |
                D6 . A5 F5 D5 F5 A5 D6 | D6 . C6 Bb5 . . F5 G5 | E5 . G5 C6 . . Bb5 A5 | A5 . . . E5 . C#5 . |
                D5 . F5 . A5 . D6 . | F6 . E6 D6 . . Bb5 D6 | Bb5 . A5 G5 . . F5 E5 | E5 . . . C#5 . . . |
                Bb5 . . A5 G5 . D5 . | F5 . . E5 D5 . A4 . | D5 . F5 . Bb5 . A5 G5 | A5 . . . G5 . E5 . |
                Bb5 . . A5 G5 . D6 . | C6 . . Bb5 A5 . F5 . | G5 . Bb5 . D6 . C6 Bb5 | A5 . . . E5 . C#5 . |";
    let groove = "k . h . s . h . k . h k s . h . ";
    let groove2 = "k . h . s . h k . k h . s . h h ";
    let crash = "k+c . h . s . h . k . h k s . h . ";
    let fill = "k . h . s . s . s s s . t t t t ";
    let section = format!("{crash}{groove}{groove2}{groove}{groove}{groove2}{groove}{fill}");
    Song {
        bpm: 144.0, swing: 0.0, bars: 24, looped: true,
        tracks: vec![
            track(LEAD, 1.0, 2, lead),
            chords(ARP, 0.4, prog, "1 3 5 1^ 5 3 1 3 1 3 5 1^ 5 3 1 3", 1, 4),
            chords(BASS, 0.9, prog, "1 1^ 1 1^ 1 1^ 1 1^", 2, 2),
            chords(Inst::Strings, 0.55, prog, "1+3+5 . . . . . . .", 2, 4),
            track(Inst::Kit, 1.0, 1, &section.repeat(3)),
        ],
    }
}

fn table_a() -> Song {
    let prog = "Am G F E Am G F E  Am G F E Am G F E  F G Am Am F G E E";
    let a = "E5 . A5 . B5 C6 B5 A5 | D5 . G5 . A5 B5 A5 G5 | C5 . F5 . G5 A5 G5 F5 | E5 . G#5 . B5 . E6 . |
             E5 . A5 . B5 C6 D6 C6 | B5 . G5 . D5 . G5 B5 | A5 . F5 . C6 . A5 F5 | G#5 . . . E5 . - - |";
    let a2 = "A5 . . E5 A5 B5 C6 . | B5 . . G5 B5 C6 D6 . | C6 . . A5 F5 . A5 C6 | B5 . G#5 . E5 . B4 . |
              C6 . B5 . A5 . E6 . | D6 . C6 . B5 . G5 . | A5 . C6 . F6 . E6 D6 | E6 . . . B5 . G#5 . |";
    let b = "A5 . . C6 . . A5 . | B5 . . D6 . . B5 . | C6 . B5 . A5 . E5 . | A5 . . . - - E5 G5 |
             A5 . . C6 . . F6 . | E6 . . D6 . . B5 . | G#5 . B5 . E6 . D6 . | B5 . . . G#5 . . . |";
    let rock = "k . h . s . h k . k h . s . h . ";
    let rock16 = "k h h h s h h k h k h h s h h h ";
    let crash = "k+c . h . s . h k . k h . s . h . ";
    let fill = "k . h . s . h k s s s s t t s s ";
    let drums = format!("{crash}{rock}{rock}{rock}{rock}{rock}{rock}{fill}{crash}{rock16}{rock16}{rock16}{rock16}{rock16}{rock16}{fill}{crash}{rock}{rock16}{rock}{rock}{rock16}{rock}{fill}");
    Song {
        bpm: 152.0, swing: 0.0, bars: 24, looped: true,
        tracks: vec![
            track(LEAD50, 0.95, 2, &format!("{a} {a2} {}", rest(8))),
            track(Inst::Brass, 1.0, 2, &format!("{} {b}", rest(16))),
            chords(ARP, 0.35, prog, "1^ 5 3 5 1^ 5 3 5 1^ 5 3 5 1^ 5 3 5", 1, 4),
            chords(Inst::Slap, 0.9, prog, "1 . 1^ 1 - 1 5 1^", 2, 2),
            chords(Inst::Strings, 0.4, prog, "1+3+5 . . . . . . .", 2, 4),
            track(Inst::Kit, 1.0, 1, &drums),
        ],
    }
}

fn table_b() -> Song {
    let prog = "F Dm Bb C F Dm Bb C  F Dm Bb C F Dm Bb C  Bb C Am Dm Gm C F F";
    let a = "C5 . F5 . A5 . G5 F5 | A5 . . G5 F5 . D5 . | D5 . F5 . Bb5 . A5 G5 | G5 . . . E5 . C5 . |
             C5 . F5 . A5 . C6 A5 | D6 . . C6 A5 . F5 . | G5 . Bb5 . A5 . G5 E5 | F5 . . . - - C5 D5 |";
    let a2 = "F5 . A5 . C6 . A5 . | D6 . . . C6 . A5 . | Bb5 . D6 . F6 . E6 D6 | E6 . . . C6 . G5 . |
              A5 . C6 . F6 . E6 C6 | D6 . F6 . A6 . F6 . | E6 . D6 . C6 . Bb5 G5 | F5 . . . - - - - |";
    let b = "F5 . D5 . F5 . G5 . | E5 . C5 . E5 . G5 . | A5 . . G5 E5 . C5 . | D5 . . . F5 . A5 . |
             Bb5 . A5 . G5 . D5 . | E5 . G5 . C6 . Bb5 . | A5 . . . . . G5 . | F5 . . . - - - - |";
    let beat = "k . h . s . h h k . h . s . h . ";
    let beat2 = "k . h . s . h . k . h k s . h r ";
    let crash = "k+c . h . s . h h k . h . s . h . ";
    let fill = "k . h . s . h . s . s s t . t . ";
    let sec = format!("{crash}{beat}{beat2}{beat}{beat}{beat2}{beat}{fill}");
    Song {
        bpm: 132.0, swing: 0.22, bars: 24, looped: true,
        tracks: vec![
            track(LEAD, 1.0, 2, &format!("{a} {} {b}", rest(8))),
            track(Inst::Bell, 1.0, 2, &format!("{} {a2} {}", rest(8), rest(8))),
            chords(Inst::Piano, 0.55, prog, "- 1+3+5 - 1+3+5 - 1+3+5 - 1+3+5", 2, 4),
            chords(BASS, 0.9, prog, "1 . 5 . 1^ . 5 1", 2, 2),
            track(Inst::Kit, 0.9, 1, &sec.repeat(3)),
        ],
    }
}

fn boss() -> Song {
    let prog = "Cm Cm Ab G Cm Cm Db G  Ab Bb Cm Cm Ab Bb G G  Cm Cm Ab G Cm Cm Db G";
    let a = "C5 . . G4 C5 . Eb5 . | D5 . C5 . G4 . . . | Ab4 . C5 . Eb5 . F5 Eb5 | D5 . . . B4 . G4 . |
             C5 . . G4 C5 . Eb5 . | G5 . F5 . Eb5 . D5 . | Db5 . F5 . Ab5 . G5 F5 | G5 . . . B5 . D6 . |";
    let b = "C6 . . Bb5 Ab5 . Eb5 . | D6 . . C6 Bb5 . F5 . | Eb6 . D6 . C6 . G5 . | C6 . . . . . - - |
             Ab5 . C6 . Eb6 . C6 . | Bb5 . D6 . F6 . D6 . | D6 . . C6 B5 . G5 . | B5 . . . D6 . F6 . |";
    let drive = "k . h k s . h . k k h . s . h k ";
    let drive2 = "k . h k s . h k . k h k s . s . ";
    let crash = "k+c . h k s . h . k k h . s . h k ";
    let fill = "k . s . k s s . s s s s t t t t ";
    let sec = format!("{crash}{drive}{drive2}{drive}{drive}{drive2}{drive}{fill}");
    Song {
        bpm: 168.0, swing: 0.0, bars: 24, looped: true,
        tracks: vec![
            track(LEAD50, 0.9, 2, &format!("{a} {b} {a}")),
            track(Inst::Brass, 0.9, 2, &format!("{} {a}", rest(16))),
            chords(ARP, 0.3, prog, "[ 1^ 3^ 5^ 3^ ]x4", 1, 4),
            chords(BASS, 0.8, prog, "1 1 1^ 1 1 1 1^ 1 1 1 1^ 1 1 1^ 1 1^", 1, 2),
            chords(Inst::Strings, 0.5, prog, "1+3+5 . . . . . . .", 2, 4),
            track(Inst::Kit, 1.0, 1, &sec.repeat(3)),
        ],
    }
}

fn shop() -> Song {
    let prog = "Fmaj7 Em7 Dm7 G7 Cmaj7 Am7 Dm7 G7  Fmaj7 Em7 Dm7 G7 Cmaj7 Am7 Dm7 G7";
    let lead = "E5 . . G5 A5 . C6 . | B5 . . A5 G5 . E5 . | F5 . A5 . C6 . B5 A5 | G5 . . . F5 . D5 . |
                E5 . G5 . B5 . . A5 | G5 . E5 . C5 . . . | D5 . F5 . A5 . C6 . | B5 . . . G5 . - - |
                A5 . C6 . E6 . D6 C6 | B5 . . . G5 . E5 . | D6 . C6 . A5 . F5 . | G5 . B5 . D6 . F6 . |
                E6 . . D6 C6 . G5 . | A5 . . G5 E5 . C5 . | F5 . E5 . D5 . F5 . | G5 . . . - - - - |";
    let soft = Inst::Pulse { duty: 0.5, decay: 0.5, sustain: 0.45, vib: 0.3 };
    Song {
        bpm: 104.0, swing: 0.33, bars: 16, looped: true,
        tracks: vec![
            track(soft, 0.8, 2, lead),
            chords(Inst::Piano, 0.6, prog, "1+3+5+7 . . . - 3+5+7 . .", 2, 4),
            chords(BASS, 0.85, prog, "1 3 5 7", 4, 2),
            track(Inst::Kit, 0.6, 1, "k . h . r . h h' k . h . r . h . "),
        ],
    }
}

fn jingle_cleared() -> Song {
    Song {
        bpm: 150.0, swing: 0.0, bars: 2, looped: false,
        tracks: vec![
            track(LEAD, 1.0, 1, "C5 . E5 . G5 . C6 . . . G5 . C6 . . . | E6 . . . . . . . - - - - - - - -"),
            track(Inst::Brass, 0.8, 1, "C4+E4+G4 . . . . . . . F4+A4+C5 . . . G4+B4+D5 . . . | C4+E4+G4+C5 . . . . . . . . . . . - - - -"),
            track(BASS, 0.9, 4, "C2 . F2 G2 | C2 . . -"),
            track(Inst::Kit, 1.0, 1, "k+c . . . s . . . k . k . s s s s | k+c . . . . . . . - - - - - - - -"),
        ],
    }
}

fn jingle_extra() -> Song {
    Song {
        bpm: 170.0, swing: 0.0, bars: 1, looped: false,
        tracks: vec![
            track(ARP, 1.4, 1, "C5 E5 G5 C6 E5 G5 C6 E6 G5 C6 E6 G6 . . - -"),
            track(Inst::Bell, 0.7, 4, "C6 G6 C7 ."),
            track(Inst::Kit, 0.8, 1, "h h h h h h h h h h h h o . - -"),
        ],
    }
}

fn jingle_over() -> Song {
    Song {
        bpm: 96.0, swing: 0.0, bars: 3, looped: false,
        tracks: vec![
            track(LEAD50, 1.0, 2, "G5 . Eb5 . C5 . Ab4 . | F4 . . Ab4 G4 . D4 . | C4 . . . . . - -"),
            track(Inst::Strings, 0.8, 8, "C4+Eb4+G4 Ab3+C4+Eb4 | F3+Ab3+C4 G3+B3+D4 | C3+Eb3+G3 ."),
            track(BASS, 0.8, 8, "C2 Ab1 | F1 G1 | C2 -"),
        ],
    }
}

fn jingle_win() -> Song {
    Song {
        bpm: 140.0, swing: 0.0, bars: 3, looped: false,
        tracks: vec![
            track(LEAD, 1.0, 1, "G4 . C5 . E5 . G5 . . . E5 . G5 . . . | A5 . . . F5 . A5 . C6 . . . A5 . C6 . | B5 . . . D6 . . . C6 . . . . . - -"),
            track(Inst::Brass, 0.8, 4, "C4+E4+G4 . . . | F4+A4+C5 . . . | G4+B4+D5 . C4+E4+G4+C5 ."),
            track(BASS, 0.9, 4, "C2 G2 C3 G2 | F2 C3 F2 C3 | G2 . C2 ."),
            track(Inst::Kit, 1.0, 1, "k+c . h . s . h . k . h . s . h . | k+c . h . s . h . k . h . s s s s | k . s s k . s s k+c . . . - - - -"),
        ],
    }
}

pub fn song(name: &str) -> Song {
    match name {
        "title" => title(),
        "table_a" => table_a(),
        "table_b" => table_b(),
        "boss" => boss(),
        "shop" => shop(),
        "cleared" => jingle_cleared(),
        "extra" => jingle_extra(),
        "over" => jingle_over(),
        "win" => jingle_win(),
        _ => panic!("no song {name}"),
    }
}

pub const SONGS: &[&str] = &["title", "table_a", "table_b", "boss", "shop", "cleared", "extra", "over", "win"];

// ------------------------------------------------------------------ playback

/// plays one looping song at a time, plus jingles that either replace it or duck it
pub struct Music {
    sounds: HashMap<&'static str, (Sound, f64)>,
    playing: Option<&'static str>,
    want: Option<&'static str>,
    jingle_until: f64,
    duck_until: f64,
    applied: f32,
    pub volume: f32,
}

impl Music {
    pub fn new(volume: f32) -> Music {
        Music { sounds: HashMap::new(), playing: None, want: None, jingle_until: 0.0, duck_until: 0.0, applied: -1.0, volume }
    }

    /// render and load one song; returns false if it couldn't be decoded
    pub async fn load(&mut self, name: &'static str) -> bool {
        let pcm = render(&song(name));
        let secs = pcm.len() as f64 / SR as f64;
        match load_sound_from_bytes(&wav(&pcm)).await {
            Ok(s) => { self.sounds.insert(name, (s, secs)); true }
            Err(_) => false,
        }
    }

    pub fn set(&mut self, want: Option<&'static str>) { self.want = want; }

    /// a jingle that replaces the music (table cleared, game over); the wanted song resumes after it
    pub fn jingle(&mut self, name: &'static str, now: f64) {
        self.stop_current();
        if let Some((s, len)) = self.sounds.get(name) {
            play_sound(s, PlaySoundParams { looped: false, volume: self.volume });
            self.jingle_until = now + len;
        }
    }

    /// a short sting over the music, which ducks while it plays
    pub fn sting(&mut self, name: &'static str, now: f64) {
        if now < self.jingle_until { return; }
        if let Some((s, len)) = self.sounds.get(name) {
            play_sound(s, PlaySoundParams { looped: false, volume: self.volume });
            self.duck_until = now + len;
        }
    }

    fn stop_current(&mut self) {
        if let Some(p) = self.playing.take() { if let Some((s, _)) = self.sounds.get(p) { stop_sound(s); } }
        self.applied = -1.0;
    }

    pub fn update(&mut self, now: f64, paused: bool) {
        if now < self.jingle_until { return; }
        if self.want != self.playing {
            self.stop_current();
            if let Some(w) = self.want {
                if let Some((s, _)) = self.sounds.get(w) {
                    play_sound(s, PlaySoundParams { looped: true, volume: self.volume });
                    self.playing = Some(w);
                    self.applied = self.volume;
                }
            }
        }
        let v = self.volume * if paused { 0.4 } else { 1.0 } * if now < self.duck_until { 0.3 } else { 1.0 };
        if let Some(p) = self.playing {
            if (v - self.applied).abs() > 0.005 {
                if let Some((s, _)) = self.sounds.get(p) { set_sound_volume(s, v); }
                self.applied = v;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_song_parses_and_tracks_fit_the_loop() {
        for name in SONGS {
            let s = song(name);
            for (i, t) in s.tracks.iter().enumerate() {
                assert!(t.steps > 0, "{name} track {i} is empty");
                if s.looped {
                    assert!((s.bars * 16) % t.steps == 0, "{name} track {i}: {} steps doesn't divide the {}-bar loop", t.steps, s.bars);
                } else {
                    assert!(t.steps <= s.bars * 16, "{name} track {i} runs past the jingle");
                }
            }
        }
    }
}
