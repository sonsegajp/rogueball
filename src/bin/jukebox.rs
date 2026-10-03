//! Renders every song to build/music/<name>.wav and reports how long each took.  cargo run --release --bin jukebox
use rogueball::chip::{wav, SR};
use rogueball::music::{render, song, SONGS};

fn main() {
    if std::env::args().any(|a| a == "--profile") {
        // time each track of each song on its own
        for name in SONGS {
            let n = song(name).tracks.len();
            let mut parts = vec![];
            for i in 0..n {
                let mut s = song(name);
                let t = s.tracks.swap_remove(i);
                s.tracks = vec![t];
                let t0 = std::time::Instant::now();
                render(&s);
                parts.push(format!("{:.0}", t0.elapsed().as_secs_f64() * 1000.0));
            }
            println!("{name:8} {}", parts.join(" "));
        }
        return;
    }
    std::fs::create_dir_all("build/music").unwrap();
    let mut total = 0.0;
    for name in SONGS {
        let t = std::time::Instant::now();
        let pcm = render(&song(name));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        total += ms;
        let rms = (pcm.iter().map(|v| v * v).sum::<f32>() / pcm.len() as f32).sqrt();
        std::fs::write(format!("build/music/{name}.wav"), wav(&pcm)).unwrap();
        println!("{name:8} {:5.1}s  rendered in {ms:6.0} ms  rms {rms:.3}", pcm.len() as f32 / SR);
    }
    println!("total {total:.0} ms");
}
