//! The person sharing a file, read from the ERP login cache.
//!
//! The desktop app signs in with the ERP and sends that access token on publish and on each new
//! share link. When `redis_url` is set in the server config (or `FIGCRAFT_REDIS_URL`), this server reads `user_token:{token}` — the
//! value ThinkPHP stores with PHP `serialize` — and records that person's name on the link.
//! A client-supplied label is not used then. With the variable unset, links stay as they were.
//!
//! The parser accepts PHP arrays and scalars only. It rejects objects, so a value in the cache
//! cannot name a class to construct.

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;

struct Settings {
    url: String,
    appid: String,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

/// Remember the Redis address and app id from the server's config. The environment variables
/// still win when they are set. Tests do not open Redis unless `FIGCRAFT_REDIS_TEST=1`.
pub fn configure(redis_url: &str, appid: &str) {
    let _ = SETTINGS.set(Settings { url: redis_url.trim().to_string(), appid: appid.trim().to_string() });
}

/// Why a share was refused, without the token or the Redis address.
const SIGN_IN: &str = "log in before sharing";
const EXPIRED: &str = "login expired: sign in again";
const STORE: &str = "can't reach the login store";
const UNREADABLE: &str = "the login store returned a user this server can't read";

/// `Ok(())` when logins are not checked, or the token names a user. `Err` when they are checked
/// and the token is missing or no longer valid.
pub fn require_login(token: &str) -> Result<(), String> {
    lookup(token).map(|_| ())
}

/// The link label to keep: the signed-in person's name when logins are checked, otherwise `label`.
pub fn label_for(token: &str, label: &str) -> Result<String, String> {
    match lookup(token)? {
        Some(name) => Ok(name),
        None => Ok(clean_label(label)),
    }
}

/// `Ok(None)` when `FIGCRAFT_REDIS_URL` is unset (tests leave it unset). `Ok(Some(name))` for a
/// live token. `Err` when a store is configured and the token cannot be used.
fn lookup(token: &str) -> Result<Option<String>, String> {
    let Some(url) = redis_url() else { return Ok(None) };
    let token = token.trim();
    if token.is_empty() || token.len() > 256 || token.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(SIGN_IN.into());
    }
    let client = redis::Client::open(url).map_err(|_| STORE.to_string())?;
    let mut con = client.get_connection_with_timeout(Duration::from_secs(3)).map_err(|_| STORE.to_string())?;
    let key = format!("user_token:{token}");
    let raw: Option<Vec<u8>> = redis::cmd("GET").arg(&key).query(&mut con).map_err(|_| STORE.to_string())?;
    let Some(raw) = raw.filter(|b| !b.is_empty()) else { return Err(EXPIRED.into()) };
    let user = php_array(&raw).map_err(|_| UNREADABLE.to_string())?;
    if let Some(app) = app_id()
        && user.get("from_appid").map(String::as_str) != Some(app.as_str())
    {
        return Err(EXPIRED.into());
    }
    Ok(Some(display_name(&user)))
}

/// Tests never open Redis, even when the machine has `FIGCRAFT_REDIS_URL` set. A test that wants
/// the real store sets `FIGCRAFT_REDIS_TEST=1` as well.
fn redis_url() -> Option<String> {
    #[cfg(test)]
    if std::env::var("FIGCRAFT_REDIS_TEST").ok().as_deref() != Some("1") {
        return None;
    }
    if let Some(url) = std::env::var("FIGCRAFT_REDIS_URL").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        return Some(url);
    }
    SETTINGS.get().map(|s| s.url.clone()).filter(|s| !s.is_empty())
}

fn app_id() -> Option<String> {
    if let Some(app) = std::env::var("FIGCRAFT_ERP_APPID").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        return Some(app);
    }
    SETTINGS.get().map(|s| s.appid.clone()).filter(|s| !s.is_empty())
}

fn display_name(user: &BTreeMap<String, String>) -> String {
    let nickname = user.get("nickname").map(String::as_str).unwrap_or("");
    let username = user.get("username").map(String::as_str).unwrap_or("");
    let name = if nickname.is_empty() { username } else { nickname };
    let name = clean_label(name);
    if name.is_empty() { "user".into() } else { name }
}

fn clean_label(label: &str) -> String {
    label.trim().chars().filter(|c| !c.is_control()).take(80).collect()
}

const MAX_DEPTH: usize = 16;
const MAX_ITEMS: usize = 10_000;
const MAX_BYTES: usize = 1_048_576;

struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn bump(&mut self) -> Result<u8, ()> {
        let c = *self.b.get(self.i).ok_or(())?;
        self.i += 1;
        Ok(c)
    }

    fn expect(&mut self, want: u8) -> Result<(), ()> {
        if self.bump()? == want { Ok(()) } else { Err(()) }
    }

    fn until(&mut self, stop: u8) -> Result<&'a [u8], ()> {
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i] != stop {
            self.i += 1;
        }
        if self.i >= self.b.len() {
            return Err(());
        }
        let out = &self.b[start..self.i];
        self.i += 1;
        Ok(out)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ()> {
        if n > MAX_BYTES || self.i.checked_add(n).filter(|end| *end <= self.b.len()).is_none() {
            return Err(());
        }
        let out = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(out)
    }
}

/// Top-level string fields of a PHP-serialized array. Nested arrays are skipped. Objects are refused.
fn php_array(raw: &[u8]) -> Result<BTreeMap<String, String>, ()> {
    let mut c = Cur { b: raw, i: 0 };
    c.expect(b'a')?;
    c.expect(b':')?;
    let n = parse_usize(c.until(b':')?)?;
    c.expect(b'{')?;
    let mut out = BTreeMap::new();
    if n > MAX_ITEMS {
        return Err(());
    }
    for _ in 0..n {
        let key = php_key(&mut c)?;
        if let Some(value) = php_value(&mut c, 1)? {
            out.insert(key, value);
        }
    }
    c.expect(b'}')?;
    Ok(out)
}

fn parse_usize(bytes: &[u8]) -> Result<usize, ()> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if text.is_empty() || text.len() > 9 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

fn php_key(c: &mut Cur<'_>) -> Result<String, ()> {
    match c.bump()? {
        b's' => php_string(c),
        b'i' => {
            c.expect(b':')?;
            let n = c.until(b';')?;
            let text = std::str::from_utf8(n).map_err(|_| ())?;
            if text.is_empty() || text.len() > 18 || !text.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
                return Err(());
            }
            Ok(text.to_string())
        }
        _ => Err(()),
    }
}

fn php_string(c: &mut Cur<'_>) -> Result<String, ()> {
    c.expect(b':')?;
    let n = parse_usize(c.until(b':')?)?;
    c.expect(b'"')?;
    let bytes = c.take(n)?;
    c.expect(b'"')?;
    c.expect(b';')?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

/// `Some` for a scalar this server can show. `None` for null or a nested array (already consumed).
fn php_value(c: &mut Cur<'_>, depth: usize) -> Result<Option<String>, ()> {
    match c.bump()? {
        b'N' => {
            c.expect(b';')?;
            Ok(None)
        }
        b'b' | b'i' | b'd' => {
            c.expect(b':')?;
            let n = c.until(b';')?;
            Ok(Some(String::from_utf8_lossy(n).into_owned()))
        }
        b's' => php_string(c).map(Some),
        b'a' => {
            if depth >= MAX_DEPTH {
                return Err(());
            }
            c.expect(b':')?;
            let n = parse_usize(c.until(b':')?)?;
            c.expect(b'{')?;
            if n > MAX_ITEMS {
                return Err(());
            }
            for _ in 0..n {
                let _ = php_key(c)?;
                let _ = php_value(c, depth + 1)?;
            }
            c.expect(b'}')?;
            Ok(None)
        }
        // Objects and references are not user fields. Refuse the whole value.
        b'O' | b'C' | b'R' | b'r' => Err(()),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{label_for, php_array, require_login};

    fn user() -> Vec<u8> {
        // a:4:{s:8:"username";s:4:"demo";s:8:"nickname";s:6:"张三";s:10:"from_appid";s:4:"1001";s:4:"role";a:0:{}}
        let mut raw = Vec::new();
        raw.extend_from_slice(b"a:4:{s:8:\"username\";s:4:\"demo\";s:8:\"nickname\";s:6:\"");
        raw.extend_from_slice("张三".as_bytes());
        raw.extend_from_slice(b"\";s:10:\"from_appid\";s:4:\"1001\";s:4:\"role\";a:0:{}}");
        raw
    }

    #[test]
    fn a_php_user_array_yields_the_nickname() {
        let fields = php_array(&user()).unwrap();
        assert_eq!(fields.get("username").map(String::as_str), Some("demo"));
        assert_eq!(fields.get("nickname").map(String::as_str), Some("张三"));
        assert_eq!(fields.get("from_appid").map(String::as_str), Some("1001"));
        assert!(!fields.contains_key("role"));
    }

    #[test]
    fn an_object_is_refused() {
        assert!(php_array(b"a:1:{s:1:\"o\";O:8:\"stdClass\":0:{}}").is_err());
    }

    #[test]
    fn without_a_login_store_the_clients_label_is_kept() {
        assert_eq!(label_for("", "Review").unwrap(), "Review");
        require_login("").unwrap();
    }
}
