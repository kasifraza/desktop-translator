//! Secrets (OS credential store) and OAuth sign-in.
//!
//! OpenRouter supports OAuth PKCE for desktop apps with a localhost callback:
//! the user signs in in their browser and we receive a personal API key.
//! Docs: https://openrouter.ai/docs/use-cases/oauth-pkce

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const SERVICE: &str = "tech.openforge.translatebubble";

// ---------- secret storage (Windows Credential Manager) ----------

pub fn secret_get(name: &str) -> String {
    keyring::Entry::new(SERVICE, name)
        .and_then(|e| e.get_password())
        .unwrap_or_default()
}

pub fn secret_set(name: &str, value: &str) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}

// ---------- OAuth (PKCE + loopback redirect) ----------

/// Bumped on every new login so a stale, still-waiting login aborts.
static LOGIN_GEN: AtomicU64 = AtomicU64::new(0);

fn random_b64(n: usize) -> Result<String, String> {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).map_err(|e| e.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

pub struct PendingLogin {
    pub auth_url: String,
    listener: TcpListener,
    verifier: String,
    state: String,
    gen: u64,
}

pub fn openrouter_begin() -> Result<PendingLogin, String> {
    let verifier = random_b64(48)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_b64(16)?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let callback = format!("http://localhost:{port}/callback");
    let auth_url = format!(
        "https://openrouter.ai/auth?callback_url={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&key_label=Translate%20Bubble",
        urlencode(&callback)
    );
    let gen = LOGIN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    Ok(PendingLogin { auth_url, listener, verifier, state, gen })
}

pub fn cancel_logins() {
    LOGIN_GEN.fetch_add(1, Ordering::SeqCst);
}

const PAGE_OK: &str = "<!doctype html><meta charset=utf-8><title>Signed in</title><body style=\"font:16px Segoe UI,sans-serif;background:#111318;color:#e7e9ee;display:grid;place-items:center;height:100vh;margin:0\"><div style=text-align:center><h2>&#10003; Signed in to Translate Bubble</h2><p>You can close this tab.</p></div>";
const PAGE_ERR: &str = "<!doctype html><meta charset=utf-8><title>Sign-in failed</title><body style=\"font:16px Segoe UI,sans-serif;background:#111318;color:#f87171;display:grid;place-items:center;height:100vh;margin:0\"><h2>Sign-in was cancelled or failed. Return to the app and try again.</h2>";

/// Blocks until the browser hits our callback (max 5 min). Returns the auth code.
pub fn wait_for_code(p: &PendingLogin) -> Result<String, String> {
    p.listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if LOGIN_GEN.load(Ordering::SeqCst) != p.gen {
            return Err("Sign-in cancelled".into());
        }
        if Instant::now() > deadline {
            return Err("Sign-in timed out".into());
        }
        let (mut sock, _) = match p.listener.accept() {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let _ = sock.set_nonblocking(false);
        let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 4096];
        let n = sock.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req.split_whitespace().nth(1).unwrap_or("");
        if !path.starts_with("/callback") {
            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }
        let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
        let param = |k: &str| {
            query.split('&').find_map(|kv| {
                let (a, b) = kv.split_once('=')?;
                (a == k).then(|| urldecode(b))
            })
        };
        let code = param("code");
        let state_ok = param("state").map_or(true, |s| s == p.state); // state isn't echoed in some modes
        let ok = code.is_some() && state_ok;
        let body = if ok { PAGE_OK } else { PAGE_ERR };
        let _ = write!(
            sock,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        return match (code, state_ok) {
            (Some(c), true) => Ok(c),
            (_, false) => Err("Sign-in failed: state mismatch".into()),
            _ => Err("Sign-in was cancelled".into()),
        };
    }
}

pub async fn openrouter_exchange(c: &reqwest::Client, p: &PendingLogin, code: &str) -> Result<String, String> {
    let r = c
        .post("https://openrouter.ai/api/v1/auth/keys")
        .json(&serde_json::json!({
            "code": code,
            "code_verifier": p.verifier,
            "code_challenge_method": "S256"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = r.status();
    let v: serde_json::Value = r.json().await.map_err(|e| e.to_string())?;
    match v["key"].as_str() {
        Some(k) if status.is_success() => Ok(k.to_string()),
        _ => Err(format!(
            "OpenRouter {status}: {}",
            v["error"]["message"].as_str().or(v["error"].as_str()).unwrap_or("key exchange failed")
        )),
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => {
                        out.push(v);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_roundtrip() {
        let s = "http://localhost:5555/callback?a=b c";
        assert_eq!(urldecode(&urlencode(s)), s);
        assert_eq!(urldecode("abc%2"), "abc%2");
    }

    #[test]
    fn credential_store_roundtrip() {
        secret_set("unit-test", "s3cret").unwrap();
        assert_eq!(secret_get("unit-test"), "s3cret");
        secret_set("unit-test", "").unwrap();
        assert_eq!(secret_get("unit-test"), "");
    }

    #[test]
    fn begin_builds_pkce_url() {
        let p = openrouter_begin().unwrap();
        assert!(p.auth_url.starts_with("https://openrouter.ai/auth?callback_url=http%3A%2F%2Flocalhost%3A"));
        assert!(p.auth_url.contains("code_challenge_method=S256"));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(p.verifier.as_bytes()));
        assert!(p.auth_url.contains(&challenge));
    }
}
