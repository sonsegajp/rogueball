//! Diagnostics log (frame hitches, anomalies, crashes): %APPDATA%\Rogueball\log.txt on desktop, the console on the web.

pub fn line(msg: &str) { crate::platform::log(msg); }

/// record panics (message and location) before the process dies
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        line(&format!("CRASH {info}"));
        prev(info);
    }));
}
