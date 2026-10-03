//! A small Game Boy Advance style sound chip, used for both the music and the sound effects.
//! The GBA mixed the old Game Boy PSG (two pulse channels, a 4-bit wavetable and an LFSR noise channel)
//! with 8-bit PCM samples played at a low mixing rate through a short echo. Everything here renders
//! offline into f32 buffers at 44.1 kHz; `crunch` gives the PCM bus its low-rate 8-bit grit.

/// output rate: the PCM bus is crunched to 13 kHz anyway, so 22 kHz loses nothing and renders twice as fast
pub const SR: f32 = 22050.0;
/// a common mixing rate for GBA sound engines
pub const PCM_RATE: f32 = 13379.0;

const TAU: f32 = std::f32::consts::TAU;

/// pulse wave with a duty cycle (0.125, 0.25 or 0.5 on the real chip); phase in cycles, kept in [0, 1) by wrap()
pub fn pulse(phase: f32, duty: f32) -> f32 { if phase < duty { 1.0 } else { -1.0 } }

/// advance an oscillator phase by f Hz for one sample, wrapping at one cycle (cheaper than fract())
#[inline]
pub fn wrap(ph: &mut f32, f: f32) { *ph += f / SR; if *ph >= 1.0 { *ph -= 1.0; } }

/// 4-bit, 32-step wavetables for the wave channel
pub const WAVE_TRI: [u8; 32] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
pub const WAVE_BASS: [u8; 32] = [15, 15, 14, 13, 12, 12, 11, 10, 9, 9, 8, 8, 7, 7, 6, 6, 9, 9, 8, 8, 7, 6, 5, 5, 4, 3, 3, 2, 1, 1, 0, 0];
pub const WAVE_ORGAN: [u8; 32] = [8, 11, 13, 14, 15, 14, 13, 11, 9, 8, 8, 9, 10, 10, 9, 8, 7, 6, 5, 5, 6, 7, 7, 6, 4, 2, 1, 0, 1, 2, 4, 6];

pub fn wave(phase: f32, table: &[u8; 32]) -> f32 {
    let i = ((phase * 32.0) as usize) & 31;
    table[i] as f32 / 7.5 - 1.0
}

/// 4-bit volume, like the PSG envelope registers
pub fn vol4(v: f32) -> f32 { (v.clamp(0.0, 1.0) * 15.0).round() / 15.0 }

/// the PSG noise channel: a 15-bit (or 7-bit "short") linear feedback shift register clocked at `clock` Hz
pub struct Lfsr { state: u16, short: bool, acc: f32 }
impl Lfsr {
    pub fn new(short: bool) -> Lfsr { Lfsr { state: 0x7fff, short, acc: 0.0 } }
    pub fn next(&mut self, clock: f32) -> f32 {
        self.acc += clock / SR;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let bit = (self.state ^ (self.state >> 1)) & 1;
            self.state = (self.state >> 1) | (bit << 14);
            if self.short { self.state = (self.state & !(1 << 6)) | (bit << 6); }
        }
        if self.state & 1 == 0 { 1.0 } else { -1.0 }
    }
}

/// cheap deterministic white noise for PCM "samples"
pub struct Noise(u32);
impl Noise {
    pub fn new(seed: u32) -> Noise { Noise(seed | 1) }
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// one-pole low-pass filter
#[derive(Default)]
pub struct Lp(f32);
impl Lp {
    pub fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        // w/(1+w) tracks 1-exp(-w) closely at audio cutoffs and avoids an exp per sample
        let w = TAU * cutoff.min(SR * 0.45) / SR;
        let a = w / (1.0 + w);
        self.0 += a * (x - self.0);
        self.0
    }
}

pub fn midi_freq(n: f32) -> f32 { 440.0 * 2f32.powf((n - 69.0) / 12.0) }

/// sample-and-hold to the PCM mixing rate, then quantize to 8 bits: the GBA's crunchy sample sound
pub fn crunch(buf: &mut [f32]) {
    let step = SR / PCM_RATE;
    let mut next = 0.0f32;
    let mut held = 0.0f32;
    for (i, s) in buf.iter_mut().enumerate() {
        if i as f32 >= next {
            held = (*s * 127.0).round().clamp(-128.0, 127.0) / 127.0;
            next += step;
        }
        *s = held;
    }
}

/// a short two-tap feedback echo, the sort of "reverb" GBA sound engines applied to the mix
pub struct Echo { buf: Vec<f32>, pos: usize, taps: [usize; 2], fb: f32, wet: f32 }
impl Echo {
    pub fn new(fb: f32, wet: f32) -> Echo {
        let taps = [(0.087 * SR) as usize, (0.131 * SR) as usize];
        Echo { buf: vec![0.0; taps[1] + 1], pos: 0, taps, fb, wet }
    }
    pub fn run(&mut self, x: f32) -> f32 {
        let n = self.buf.len();
        let a = self.buf[(self.pos + n - self.taps[0]) % n];
        let b = self.buf[(self.pos + n - self.taps[1]) % n];
        let d = (a + b) * 0.5;
        self.buf[self.pos] = x + d * self.fb;
        self.pos = (self.pos + 1) % n;
        x + d * self.wet
    }
}

/// 16-bit mono WAV at SR
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(SR as u32).to_le_bytes());
    out.extend_from_slice(&(SR as u32 * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    out
}

/// gentle saturation so loud peaks round off instead of clipping
pub fn soft_clip(x: f32) -> f32 { if x.abs() < 0.6 { x } else { x.signum() * (0.6 + (1.0 - (-(x.abs() - 0.6) * 2.5).exp()) * 0.4) } }

const SINE_N: usize = 4096;
static SINE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();

/// table sine, `phase` in cycles (any sign): far cheaper than f32::sin per sample
#[inline]
pub fn sine(phase: f32) -> f32 {
    let t = SINE.get_or_init(|| (0..SINE_N).map(|i| (TAU * i as f32 / SINE_N as f32).sin()).collect());
    t[((phase * SINE_N as f32) as i32 as usize) & (SINE_N - 1)]
}

/// per-sample multiplier for an exponential decay with time constant `tau` seconds
pub fn decay(tau: f32) -> f32 { (-1.0 / (tau * SR)).exp() }
