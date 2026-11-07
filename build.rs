use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=RATE_GEOIP_DB");
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rustc-check-cfg=cfg(rate_has_web_assets)");
    println!("cargo:rustc-check-cfg=cfg(rate_has_geoip_db)");

    let release = env::var("PROFILE").as_deref() == Ok("release");
    let dist = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("web/dist");
    if dist.join("index.html").is_file() {
        println!("cargo:rustc-cfg=rate_has_web_assets");
        println!("cargo:rustc-env=RATE_WEB_DIST={}", dist.display());
    } else if release {
        panic!(
            "Build the frontend before compiling rate in release mode (missing web/dist/index.html)"
        );
    }

    if let Some(path) = env::var_os("RATE_GEOIP_DB").map(PathBuf::from) {
        assert!(
            path.is_absolute() && path.is_file(),
            "RATE_GEOIP_DB must point to an existing absolute MMDB file path"
        );
        let path = fs::canonicalize(path).expect("canonicalize RATE_GEOIP_DB");
        println!("cargo:rerun-if-changed={}", path.display());
        println!("cargo:rustc-cfg=rate_has_geoip_db");
        println!("cargo:rustc-env=RATE_EMBEDDED_GEOIP_DB={}", path.display());
    } else if release {
        panic!(
            "Set RATE_GEOIP_DB to the downloaded MMDB file before compiling rate in release mode"
        );
    }
}
