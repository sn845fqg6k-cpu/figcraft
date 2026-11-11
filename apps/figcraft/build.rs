//! Windows: embed the icon and version info in figcraft.exe and size its main thread's stack
//! (no-op elsewhere).
fn main() {
    println!("cargo::rustc-check-cfg=cfg(windows_slim_errors)");
    println!("cargo::rerun-if-changed=../../assets/app-icon/figcraft.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        let ico = "../../assets/app-icon/figcraft.ico";
        if std::path::Path::new(ico).exists() {
            res.set_icon(ico);
        }
        res.set("ProductName", "FigCraft");
        res.set("FileDescription", "FigCraft");
        res.set("CompanyName", "ArtCraft");
        if let Err(e) = res.compile() {
            println!("cargo::warning=winresource: {e}");
        }
        // Windows starts the main thread with 1 MB of stack, Linux and macOS with 8 MB. Layers
        // nest up to 256 deep in an export, and that recursion needs the same room everywhere.
        let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        println!("cargo::rustc-link-arg-bins={}", if msvc { "/STACK:8388608" } else { "-Wl,--stack,8388608" });
    }
    let sha = std::env::var("FIGCRAFT_BUILD_SHA").unwrap_or_default();
    println!("cargo::rustc-env=FIGCRAFT_BUILD_SHA={sha}");
    println!("cargo::rerun-if-env-changed=FIGCRAFT_BUILD_SHA");
    deploy_config();
}

/// `cargo run` (debug) embeds `config/dev.toml`. A release build embeds `config/prod.toml`.
/// `FIGCRAFT_ENV=dev|test|prod` picks one of the three for a package.
fn deploy_config() {
    println!("cargo::rerun-if-env-changed=FIGCRAFT_ENV");
    println!("cargo::rerun-if-changed=config/dev.toml");
    println!("cargo::rerun-if-changed=config/test.toml");
    println!("cargo::rerun-if-changed=config/prod.toml");
    let name = match std::env::var("FIGCRAFT_ENV") {
        Ok(raw) => {
            let name = raw.trim().to_ascii_lowercase();
            if matches!(name.as_str(), "dev" | "test" | "prod") {
                name
            } else {
                println!("cargo::warning=FIGCRAFT_ENV={raw} is not dev, test or prod");
                profile_env()
            }
        }
        Err(_) => profile_env(),
    };
    println!("cargo::rustc-env=FIGCRAFT_ENV_NAME={name}");
    let src = std::path::Path::new("config").join(format!("{name}.toml"));
    let Ok(text) = std::fs::read_to_string(&src) else {
        println!("cargo::error=missing {}", src.display());
        std::process::exit(1);
    };
    let Ok(dir) = std::env::var("OUT_DIR") else {
        println!("cargo::error=OUT_DIR is not set");
        std::process::exit(1);
    };
    let out = std::path::Path::new(&dir).join("deploy.toml");
    if std::fs::write(&out, text).is_err() {
        println!("cargo::error=couldn't write {}", out.display());
        std::process::exit(1);
    }
}

fn profile_env() -> String {
    if std::env::var("PROFILE").ok().as_deref() == Some("release") { "prod".into() } else { "dev".into() }
}
