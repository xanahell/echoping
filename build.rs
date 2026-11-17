use std::{env, fs, path::PathBuf};

fn main() {
    // Скачивает (или подключает локальную) TDLib и настраивает линковку.
    tdlib_rs::build::build(None);

    // На Windows кладём DLL рядом с echoping.exe, чтобы exe запускался отдельно от cargo.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if let Ok(out) = env::var("OUT_DIR") {
            let out = PathBuf::from(out);
            let bin = out.join("tdlib").join("bin");
            if let (Some(profile_dir), Ok(rd)) = (out.ancestors().nth(3), fs::read_dir(&bin)) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("dll") {
                        let _ = fs::copy(&p, profile_dir.join(e.file_name()));
                    }
                }
            }
        }
    }
}
