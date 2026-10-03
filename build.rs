//! Embeds the art (kit sprites, card paintings, logo) in the binary, so the browser build downloads
//! one file instead of fetching ~190 small ones at startup.
use std::fmt::Write;
use std::path::Path;

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, root, out);
        } else if matches!(p.extension().and_then(|x| x.to_str()), Some("png") | Some("json")) {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((rel, p.canonicalize().unwrap().to_string_lossy().replace('\\', "/")));
        }
    }
}

fn main() {
    let root = Path::new("assets");
    println!("cargo:rerun-if-changed=assets");
    let mut files = vec![];
    for sub in ["kit", "cards"] { walk(&root.join(sub), root, &mut files); }
    if root.join("logo.png").exists() {
        files.push(("logo.png".into(), root.join("logo.png").canonicalize().unwrap().to_string_lossy().replace('\\', "/")));
    }
    let mut src = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in &files {
        let abs = abs.trim_start_matches("//?/");
        writeln!(src, "    ({rel:?}, include_bytes!({abs:?})),").unwrap();
    }
    src.push_str("];\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("embedded.rs"), src).unwrap();
}
