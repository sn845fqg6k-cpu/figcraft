//! ERP login for a private build. The request shape follows the enterprise client's MD5 fallback:
//! sort the sign fields, skip nothing that is present, MD5 the `key=value` pairs joined by `&`,
//! and send the body as `{ "data": "<json>" }`. The browser's WASM signer is not used here.
//!
//! The API address, app id and secret come from the environment file the desktop app installs
//! at startup (`FIGCRAFT_ERP_URL`, `FIGCRAFT_ERP_APPID` and `FIGCRAFT_ERP_SECRET` still override
//! a missing install, for a one-off run). `FIGCRAFT_ERP_CLIENT` defaults to `EnterpriseCenterWEB`.

use std::sync::OnceLock;

use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::{Value, json};

const MISSING: &str = "ERP login is not configured.";
const BAD_JSON: &str = "the login server sent something that isn't JSON";
pub(crate) const EXPIRED: &str = "login expired: sign in again";

#[derive(Clone)]
struct Erp {
    base: String,
    appid: String,
    secret: String,
    client: String,
    language: String,
}

static INSTALLED: OnceLock<Erp> = OnceLock::new();

/// What a successful login leaves in this process. `expires` and `refresh_expires` are unix times.
pub struct SignedIn {
    pub access: String,
    pub refresh: String,
    pub name: String,
    pub expires: i64,
    pub refresh_expires: i64,
}

/// Use this ERP for the rest of the process. The desktop app calls it once, from the environment
/// file baked in for this build.
pub fn install(base: &str, appid: &str, secret: &str, client: &str) -> Result<(), String> {
    let cfg = assemble(base, appid, secret, client)?;
    let _ = INSTALLED.set(cfg);
    Ok(())
}

/// An environment file was installed, so the app asks for a login before the document.
pub fn configured() -> bool {
    INSTALLED.get().is_some()
}

/// The access token is still worth sending. Unknown expiry (`0`) counts as still good; a known
/// one is refreshed a little early so a share doesn't fail on the last seconds.
pub fn access_current(token: &str, expires: i64) -> bool {
    if token.is_empty() {
        return false;
    }
    expires <= 0 || unix_now().saturating_add(120) < expires
}

/// Sign in with the ERP. The password is not kept.
pub fn login(username: &str, password: &str) -> Result<SignedIn, String> {
    let username = clean(username, 80)?;
    let password = clean(password, 200)?;
    if username.is_empty() || password.is_empty() {
        return Err("enter a username and a password".into());
    }
    let cfg = config()?;
    let body = Login { password: &password, username: &username };
    let data = post(&cfg, "/open/Login/userLogin", &body, None)?;
    let access = field(&data, "accessToken")?;
    let refresh = field(&data, "refreshToken").unwrap_or_default();
    let name = user_name(&cfg, &access).unwrap_or(username);
    Ok(SignedIn { access, refresh, name, expires: expiry(&data, "expires", 864_000), refresh_expires: expiry(&data, "expires_refresh", 8_640_000) })
}

/// Ask the ERP for a new access token. Used when a share is refused because the old one expired.
pub fn refresh(refresh_token: &str) -> Result<SignedIn, String> {
    let refresh_token = clean(refresh_token, 512)?;
    if refresh_token.is_empty() {
        return Err(EXPIRED.into());
    }
    let cfg = config()?;
    let body = Refresh { refresh_token: &refresh_token };
    let data = post(&cfg, "/open/Login/refreshToken", &body, None)?;
    let access = field(&data, "accessToken")?;
    let refresh = field(&data, "refreshToken").unwrap_or(refresh_token);
    let name = user_name(&cfg, &access).unwrap_or_else(|| "user".into());
    Ok(SignedIn { access, refresh, name, expires: expiry(&data, "expires", 864_000), refresh_expires: expiry(&data, "expires_refresh", 8_640_000) })
}

#[derive(Serialize)]
struct Login<'a> {
    password: &'a str,
    username: &'a str,
}

#[derive(Serialize)]
struct Refresh<'a> {
    #[serde(rename = "refreshToken")]
    refresh_token: &'a str,
}

fn config() -> Result<Erp, String> {
    if let Some(cfg) = INSTALLED.get() {
        return Ok(cfg.clone());
    }
    let base = std::env::var("FIGCRAFT_ERP_URL").map_err(|_| MISSING.to_string())?;
    let appid = std::env::var("FIGCRAFT_ERP_APPID").map_err(|_| MISSING.to_string())?;
    let secret = std::env::var("FIGCRAFT_ERP_SECRET").map_err(|_| MISSING.to_string())?;
    let client = std::env::var("FIGCRAFT_ERP_CLIENT").unwrap_or_else(|_| "EnterpriseCenterWEB".into());
    assemble(&base, &appid, &secret, &client)
}

fn assemble(base: &str, appid: &str, secret: &str, client: &str) -> Result<Erp, String> {
    let base = base.trim().trim_end_matches('/').to_string();
    let appid = appid.trim().to_string();
    let secret = secret.trim().to_string();
    let client = {
        let client = client.trim();
        if client.is_empty() { "EnterpriseCenterWEB".to_string() } else { client.to_string() }
    };
    if base.is_empty() || appid.is_empty() || secret.is_empty() || !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err(MISSING.into());
    }
    let language = std::env::var("FIGCRAFT_ERP_LOCALE").unwrap_or_else(|_| "zh-CN".into());
    Ok(Erp { base, appid, secret, client, language })
}

fn unix_now() -> i64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| i64::try_from(d.as_secs()).unwrap_or(0)).unwrap_or(0)
}

/// `expires` from the ERP is a unix time. A small number is a duration in seconds.
fn expiry(data: &Value, key: &str, fallback: i64) -> i64 {
    let now = unix_now();
    match data.get(key).and_then(Value::as_i64) {
        Some(n) if n > 1_000_000_000 => n,
        Some(n) if n > 0 => now.saturating_add(n),
        _ => now.saturating_add(fallback),
    }
}

fn clean(value: &str, max: usize) -> Result<String, String> {
    if value.chars().any(|c| c.is_control()) {
        return Err("that text can't be sent to the login server".into());
    }
    Ok(value.trim().chars().take(max).collect())
}

fn field(data: &Value, key: &str) -> Result<String, String> {
    data.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).ok_or_else(|| "the login server didn't return a token".into())
}

fn user_name(cfg: &Erp, access: &str) -> Option<String> {
    let data = post(cfg, "/core/User/info", &Empty {}, Some(access)).ok()?;
    let pick = |v: &Value| {
        ["realName", "nickname", "username"].into_iter().find_map(|k| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string))
    };
    pick(&data).or_else(|| data.get("user").and_then(pick))
}

#[derive(Serialize)]
struct Empty {}

fn post(cfg: &Erp, path: &str, body: &impl Serialize, token: Option<&str>) -> Result<Value, String> {
    let payload = serde_json::to_string(body).map_err(|_| "couldn't build the login request".to_string())?;
    let timestamp = unix_now();
    let signature = sign(&[("appid", Part::Text(&cfg.appid)), ("data", Part::Text(&payload)), ("secret", Part::Text(&cfg.secret)), ("timestamp", Part::Int(timestamp))]);
    let url = format!("{}{path}", cfg.base);
    let mut req = ureq::post(&url)
        .set("Accept-Language", &cfg.language)
        .set("Appid", &cfg.appid)
        .set("Client-Type", &cfg.client)
        .set("Location-Href", "figcraft://app")
        .set("Zip", "0")
        .set("Location-Pathname", "/")
        .set("Location-Source", "1")
        .set("timestamp", &timestamp.to_string())
        .set("Sign", &signature);
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        let bearer = format!("Bearer {token}");
        req = req.set("Authorization", &bearer);
    }
    let response = req.send_json(json!({ "data": payload })).map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("the login server answered {code}"),
        _ => "can't reach the login server".into(),
    })?;
    if response.status() != 200 {
        return Err(format!("the login server answered {}", response.status()));
    }
    let body: Value = response.into_json().map_err(|_| BAD_JSON.to_string())?;
    let code = body.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code == 8001 {
        return Err(EXPIRED.into());
    }
    if code != 1 {
        let message = body.get("message").and_then(Value::as_str).unwrap_or("the login server refused");
        let message: String = message.chars().filter(|c| !c.is_control()).take(200).collect();
        return Err(if message.is_empty() { "the login server refused".into() } else { message });
    }
    Ok(body.get("data").cloned().unwrap_or(Value::Null))
}

enum Part<'a> {
    Text(&'a str),
    Int(i64),
}

fn sign(parts: &[(&str, Part<'_>)]) -> String {
    md5_hex(&canonical(parts))
}

fn canonical(parts: &[(&str, Part<'_>)]) -> String {
    let mut rows: Vec<(&str, String)> = parts
        .iter()
        .map(|(key, value)| {
            let text = match value {
                Part::Text(s) => (*s).to_string(),
                Part::Int(n) => n.to_string(),
            };
            (*key, text)
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    rows.into_iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join("&")
}

fn md5_hex(text: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{Login, Part, access_current, canonical, expiry, md5_hex, sign, unix_now};

    #[test]
    fn the_sign_is_the_sorted_md5_fallback() {
        let data = serde_json::to_string(&Login { password: "p", username: "u" }).unwrap();
        assert_eq!(data, r#"{"password":"p","username":"u"}"#);
        let parts = [("appid", Part::Text("1001")), ("data", Part::Text(&data)), ("secret", Part::Text("sek")), ("timestamp", Part::Int(1_700_000_000))];
        let joined = r#"appid=1001&data={"password":"p","username":"u"}&secret=sek&timestamp=1700000000"#;
        assert_eq!(canonical(&parts), joined);
        assert_eq!(sign(&parts), md5_hex(joined));
        assert_eq!(md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn an_absolute_expiry_is_kept_and_a_duration_is_added_to_now() {
        let absolute = serde_json::json!({"expires": 1_700_000_000});
        assert_eq!(expiry(&absolute, "expires", 10), 1_700_000_000);
        let before = unix_now();
        let got = expiry(&serde_json::json!({"expires": 90}), "expires", 10);
        let after = unix_now();
        assert!(got >= before + 90 && got <= after + 90);
        assert!(!access_current("", 1_700_000_000));
        assert!(access_current("QC1", 0));
        assert!(access_current("QC1", after.saturating_add(10_000)));
        assert!(!access_current("QC1", after.saturating_sub(10)));
    }
}
