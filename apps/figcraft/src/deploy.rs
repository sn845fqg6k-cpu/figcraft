//! The ERP and share server baked in for this build. Debug embeds `config/dev.toml`, a release
//! embeds `config/prod.toml`, and `FIGCRAFT_ENV=test` embeds the test file (see `build.rs`).

use figcraft_engine::Session;

struct Deploy {
    erp_url: String,
    erp_appid: String,
    erp_secret: String,
    erp_client: String,
    share_server: String,
}

/// Read the environment file and lock sharing to its server. A file that doesn't parse leaves
/// the public server in place and says so in the log.
pub fn install() {
    let raw = include_str!(concat!(env!("OUT_DIR"), "/deploy.toml"));
    let name = option_env!("FIGCRAFT_ENV_NAME").unwrap_or("dev");
    if let Some(repo) = value(raw, "update_repo").filter(|s| !s.is_empty()) {
        match figcraft_ui_egui::install_update_repo(&repo) {
            Ok(()) => log::info!("deployment {name}: updates come from {repo}"),
            Err(e) => log::error!("{name}: update config: {e}"),
        }
    }
    let Some(cfg) = parse(raw) else {
        log::error!("deployment config is incomplete; sharing stays on the public server");
        return;
    };
    if let Err(e) = figcraft_ui_egui::erp::install(&cfg.erp_url, &cfg.erp_appid, &cfg.erp_secret, &cfg.erp_client) {
        log::error!("{name}: ERP config: {e}");
        return;
    }
    figcraft_engine::live::install_deployment(&cfg.share_server);
    log::info!("deployment {name}: sharing uses {}", cfg.share_server);
}

/// Preferences saved earlier may name another server. A locked build keeps the configured one.
pub fn pin(session: &mut Session) {
    if figcraft_engine::live::internal_locked() {
        session.prefs.publish_server = figcraft_engine::live::share_server().to_string();
    }
}

fn parse(text: &str) -> Option<Deploy> {
    let erp_url = value(text, "erp_url")?;
    let erp_appid = value(text, "erp_appid")?;
    let erp_secret = value(text, "erp_secret")?;
    let share_server = value(text, "share_server")?;
    if erp_url.is_empty() || erp_appid.is_empty() || erp_secret.is_empty() || share_server.is_empty() {
        return None;
    }
    let erp_client = value(text, "erp_client").filter(|s| !s.is_empty()).unwrap_or_else(|| "EnterpriseCenterWEB".to_string());
    Some(Deploy { erp_url, erp_appid, erp_secret, erp_client, share_server })
}

fn value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            continue;
        }
        let v = v.trim().trim_matches('"').trim();
        return Some(v.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn every_environment_file_names_an_api_and_a_server() {
        for name in ["dev", "test", "prod"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config").join(format!("{name}.toml"));
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let cfg = parse(&text).unwrap_or_else(|| panic!("{name} is incomplete"));
            assert!(cfg.erp_url.starts_with("http://") || cfg.erp_url.starts_with("https://"), "{name}");
            assert!(cfg.share_server.starts_with("ws://") || cfg.share_server.starts_with("wss://"), "{name}");
            assert!(!cfg.erp_secret.is_empty(), "{name}");
            let repo = super::value(&text, "update_repo").unwrap_or_default();
            assert_eq!(repo, "sn845fqg6k-cpu/figcraft", "{name} must check our releases, not upstream");
        }
    }
}
