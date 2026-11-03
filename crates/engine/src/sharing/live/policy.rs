//! Who may share, in a private build. The commands call [`share_target`] with
//! [`super::internal_locked`]; tests call it with the flag passed in, so `cargo test` does not
//! depend on the compile-time switch.

/// Where a share may go.
///
/// `locked`: the private build. `configured`: the only server that build may use.
/// `local`: hosting on this machine. `requested`: the address the caller asked for (empty: the
/// configured one). `token`: the ERP access token.
///
/// Returns the server to use. A build that is not locked returns `requested` unchanged.
pub fn share_target<'a>(locked: bool, configured: &'a str, local: bool, requested: &'a str, token: &str) -> Result<&'a str, &'static str> {
    if !locked {
        return Ok(requested);
    }
    if local {
        return Err("local sharing is turned off in this build");
    }
    if configured.is_empty() {
        return Err("this build has no share server configured");
    }
    let requested = requested.trim();
    if !requested.is_empty() && requested != configured {
        return Err("sharing only uses the configured server");
    }
    if token.trim().is_empty() {
        return Err("log in before sharing");
    }
    Ok(configured)
}

/// A preference may name `next` as the share server.
pub fn server_pref_allowed(locked: bool, configured: &str, next: &str) -> bool {
    !locked || next == configured
}

/// Whether `link` (already resolved: an invite URL, not a bare code) may be joined.
pub fn join_allowed(locked: bool, configured: &str, link: &str) -> Result<(), &'static str> {
    if !locked {
        return Ok(());
    }
    if configured.is_empty() {
        return Err("this build has no share server configured");
    }
    if link.contains("host=") && !link.contains("server=") {
        return Err("local sharing is turned off in this build");
    }
    let configured = configured.trim().trim_end_matches('/');
    let encoded = configured.replace("://", "%3A%2F%2F").replace('/', "%2F");
    let mentioned = link.split("server=").nth(1).map(|s| s.split(['&', '#']).next().unwrap_or(s)).unwrap_or(link);
    let mentioned = mentioned.trim().trim_end_matches('/');
    if mentioned.starts_with(configured) || mentioned.starts_with(&encoded) || link.starts_with(configured) {
        Ok(())
    } else {
        Err("sharing only uses the configured server")
    }
}

#[cfg(test)]
mod tests {
    use super::{join_allowed, server_pref_allowed, share_target};

    #[test]
    fn an_unlocked_build_keeps_the_address_it_was_given() {
        assert_eq!(share_target(false, "wss://only/", true, "ws://other/", ""), Ok("ws://other/"));
        assert!(server_pref_allowed(false, "wss://only/", ""));
    }

    #[test]
    fn a_locked_build_shares_only_on_its_server_after_login() {
        let server = "wss://share.internal/";
        assert_eq!(share_target(true, server, true, server, "QC1"), Err("local sharing is turned off in this build"));
        assert_eq!(share_target(true, server, false, "wss://elsewhere/", "QC1"), Err("sharing only uses the configured server"));
        assert_eq!(share_target(true, server, false, "", ""), Err("log in before sharing"));
        assert_eq!(share_target(true, server, false, "", "QC1"), Ok(server));
        assert_eq!(share_target(true, server, false, server, "QC1"), Ok(server));
        assert_eq!(share_target(true, "", false, "", "QC1"), Err("this build has no share server configured"));
        assert!(!server_pref_allowed(true, server, "wss://elsewhere/"));
        assert!(server_pref_allowed(true, server, server));
        assert_eq!(join_allowed(true, server, "figcraft://join?host=10.0.0.2&port=1&token=ab"), Err("local sharing is turned off in this build"));
        assert_eq!(join_allowed(true, server, "figcraft://join?server=wss://elsewhere/&token=ab"), Err("sharing only uses the configured server"));
        assert_eq!(join_allowed(true, server, "figcraft://join?server=wss://share.internal/&code=K7M"), Ok(()));
    }

    #[test]
    fn signing_in_keeps_the_token_out_of_the_command_log() {
        let mut s = crate::Session::new();
        s.execute("auth.signIn", &serde_json::json!({"token": "QC-token", "name": "Ada", "refresh": "QC-refresh"})).unwrap();
        assert_eq!(s.auth_token, "QC-token");
        assert_eq!(s.auth_name, "Ada");
        assert!(s.journal.iter().all(|(id, _)| id != "auth.signIn"));
        s.execute("auth.signOut", &serde_json::json!({})).unwrap();
        assert!(s.auth_token.is_empty());
        s.execute("auth.signIn", &serde_json::json!({"token": "has space"})).unwrap_err();
    }
}
