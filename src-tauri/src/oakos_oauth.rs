//! Public desktop OAuth client. Credentials and PKCE material never cross the IPC boundary.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::{Url, blocking::Client};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub const CLIENT_ID: &str = "eavesdrop-desktop";
pub const REDIRECT_URI: &str = "http://127.0.0.1:43821/oauth/oakos";
const ISSUER: &str = "https://os.oak.dk/api/auth";
const RESOURCE: &str = "https://os.oak.dk/api/v1";
const SCOPES: &str = "projects:read recordings:write offline_access";
const KEYRING_SERVICE: &str = "com.eavesdrop.recorder";
const KEYRING_ACCOUNT: &str = "oakos-oauth";

// Serializes rotating refresh credentials and disconnect/save operations.
pub static CREDENTIALS: Mutex<()> = Mutex::new(());
static LOGIN: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    token_type: String,
    expires_in: u64,
    scope: Option<String>,
}

pub struct LoginGuard(Arc<AtomicBool>);
impl LoginGuard {
    pub fn start() -> AppResult<Self> {
        let mut current = LOGIN
            .lock()
            .map_err(|_| failure("login state is unavailable"))?;
        if current.is_some() {
            return Err(failure("a sign-in is already in progress"));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        *current = Some(cancelled.clone());
        Ok(Self(cancelled))
    }
    fn check(&self) -> AppResult<()> {
        if self.0.load(Ordering::SeqCst) {
            Err(failure("sign-in canceled"))
        } else {
            Ok(())
        }
    }
}
impl Drop for LoginGuard {
    fn drop(&mut self) {
        if let Ok(mut current) = LOGIN.lock() {
            *current = None;
        }
    }
}

pub fn cancel() -> AppResult<()> {
    if let Some(cancelled) = LOGIN
        .lock()
        .map_err(|_| failure("login state is unavailable"))?
        .as_ref()
    {
        cancelled.store(true, Ordering::SeqCst);
    }
    Ok(())
}

fn entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|error| AppError::Crypto(error.to_string()))
}

pub fn load() -> AppResult<Option<Credentials>> {
    match entry()?.get_password() {
        Ok(value) => serde_json::from_str(&value).map(Some).map_err(|_| {
            failure("saved connection is unreadable. Sign in again from Integrations")
        }),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(AppError::Crypto(error.to_string())),
    }
}

pub fn delete() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::Crypto(error.to_string())),
    }
}

fn save(credentials: &Credentials) -> AppResult<()> {
    let value =
        serde_json::to_string(credentials).map_err(|_| failure("could not save the connection"))?;
    entry()?
        .set_password(&value)
        .map_err(|error| AppError::Crypto(error.to_string()))
}

pub fn http() -> AppResult<Client> {
    Client::builder()
        .user_agent(concat!("Eavesdrop/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| failure("could not initialize sign-in"))
}

pub fn connect(login: LoginGuard) -> AppResult<()> {
    login.check()?;
    // Reserve the exact registered loopback address before opening the browser.
    let listener = TcpListener::bind("127.0.0.1:43821")
        .map_err(|_| failure("could not open the sign-in callback on port 43821. Close another Eavesdrop sign-in and retry"))?;
    listener.set_nonblocking(true)?;
    let verifier = random_secret();
    let state = random_secret();
    let url = authorization_url(&verifier, &state);
    open_browser(url.as_str())?;
    let code = wait_for_code(&listener, &state, &login, Duration::from_secs(300))?;
    login.check()?;
    let client = http()?;
    let credentials = exchange(
        &client,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("resource", RESOURCE),
            ("code", &code),
            ("code_verifier", &verifier),
        ],
        None,
    )?;
    login.check()?;
    // Confirm that the resource accepts the grant before marking the app connected.
    let response = client
        .get(format!("{RESOURCE}/integrations/eavesdrop/connection"))
        .bearer_auth(&credentials.access_token)
        .send()
        .map_err(|_| failure("could not verify the connection. Try signing in again"))?;
    if !response.status().is_success() {
        return Err(failure(
            "could not verify recording access. Try signing in again",
        ));
    }
    let _lock = CREDENTIALS
        .lock()
        .map_err(|_| failure("credential storage is busy"))?;
    login.check()?;
    save(&credentials)
}

// Caller holds CREDENTIALS so a rotated refresh token cannot race another request.
pub fn access_token(mut credentials: Credentials) -> AppResult<String> {
    if credentials.expires_at <= now().saturating_add(60) {
        credentials = exchange(
            &http()?,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("resource", RESOURCE),
                ("refresh_token", &credentials.refresh_token),
            ],
            Some(&credentials.refresh_token),
        )?;
        save(&credentials)?;
    }
    Ok(credentials.access_token)
}

pub fn revoke(credentials: Credentials) -> AppResult<()> {
    let token = match access_token(credentials) {
        Ok(token) => token,
        Err(AppError::OakOsReauthentication) => return Ok(()),
        Err(error) => return Err(error),
    };
    let response = http()?
        .delete(format!("{RESOURCE}/integrations/eavesdrop/connection"))
        .bearer_auth(token)
        .send()
        .map_err(|_| failure("could not disconnect. Check your connection and retry"))?;
    if response.status().is_success() || response.status() == reqwest::StatusCode::UNAUTHORIZED {
        Ok(())
    } else {
        Err(failure("could not revoke access. Try disconnecting again"))
    }
}

fn exchange(
    client: &Client,
    form: &[(&str, &str)],
    previous_refresh: Option<&str>,
) -> AppResult<Credentials> {
    exchange_at(
        client,
        &format!("{ISSUER}/oauth2/token"),
        form,
        previous_refresh,
    )
}

fn exchange_at(
    client: &Client,
    endpoint: &str,
    form: &[(&str, &str)],
    previous_refresh: Option<&str>,
) -> AppResult<Credentials> {
    let response = client.post(endpoint).form(form).send().map_err(|_| {
        failure("could not reach the sign-in service. Check your connection and retry")
    })?;
    if !response.status().is_success() {
        if response.status().is_server_error()
            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            return Err(failure(
                "the sign-in service is temporarily unavailable. Try again shortly",
            ));
        }
        if response.status() == reqwest::StatusCode::BAD_REQUEST {
            let body: serde_json::Value = response.json().unwrap_or_default();
            if body.get("error").and_then(|value| value.as_str()) == Some("invalid_grant") {
                return Err(AppError::OakOsReauthentication);
            }
        }
        // Never include response bodies, codes, or credentials in diagnostics / IPC errors.
        return Err(failure(
            "sign-in expired or access was revoked. Reconnect OakOS in Integrations",
        ));
    }
    let token: TokenResponse = response
        .json()
        .map_err(|_| failure("the sign-in response was unreadable"))?;
    token.credentials(previous_refresh)
}

impl TokenResponse {
    fn credentials(self, previous_refresh: Option<&str>) -> AppResult<Credentials> {
        if !self.token_type.eq_ignore_ascii_case("bearer")
            || self.access_token.is_empty()
            || self.expires_in == 0
        {
            return Err(failure(
                "the sign-in service returned an invalid credential",
            ));
        }
        if let Some(scope) = &self.scope
            && SCOPES
                .split_whitespace()
                .any(|required| !scope.split_whitespace().any(|value| value == required))
        {
            return Err(failure(
                "approve project access, recording uploads, and staying connected to use Eavesdrop",
            ));
        }
        let refresh_token = self
            .refresh_token
            .or_else(|| previous_refresh.map(str::to_owned))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| failure("the sign-in service did not allow staying connected"))?;
        Ok(Credentials {
            access_token: self.access_token,
            refresh_token,
            expires_at: now().saturating_add(self.expires_in),
        })
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn authorization_url(verifier: &str, state: &str) -> Url {
    let mut url = Url::parse(&format!("{ISSUER}/oauth2/authorize")).expect("constant OAuth URL");
    url.query_pairs_mut().extend_pairs(&[
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("resource", RESOURCE),
        ("scope", SCOPES),
        ("code_challenge_method", "S256"),
        ("code_challenge", &challenge(verifier)),
        ("state", state),
    ]);
    url
}

fn wait_for_code(
    listener: &TcpListener,
    state: &str,
    login: &LoginGuard,
    timeout: Duration,
) -> AppResult<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        login.check()?;
        match listener.accept() {
            Ok((mut stream, _)) => {
                let callback = read_callback(&mut stream, state);
                let (status, message) = match &callback {
                    Ok(Some(Ok(_))) => (
                        "200 OK",
                        "Authorization received. Return to Eavesdrop to finish connecting. You can close this tab.",
                    ),
                    Ok(Some(Err(_))) => (
                        "200 OK",
                        "Sign-in was declined. Return to Eavesdrop to try again.",
                    ),
                    _ => (
                        "400 Bad Request",
                        "This sign-in callback is invalid. Return to Eavesdrop and try again.",
                    ),
                };
                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{message}",
                    message.len()
                );
                if let Ok(Some(result)) = callback {
                    return result;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(_) => return Err(failure("could not receive the sign-in callback")),
        }
    }
    Err(failure("sign-in timed out. Try connecting again"))
}

fn read_callback(
    stream: &mut TcpStream,
    state: &str,
) -> std::io::Result<Option<AppResult<String>>> {
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut request = Vec::new();
    let mut chunk = [0u8; 1024];
    while request.len() < 8192 && Instant::now() < deadline {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..count]);
        if request.windows(4).any(|part| part == b"\r\n\r\n") {
            return Ok(std::str::from_utf8(&request)
                .ok()
                .and_then(|value| parse_callback(value, state)));
        }
    }
    Ok(None)
}

fn parse_callback(request: &str, state: &str) -> Option<AppResult<String>> {
    let mut lines = request.split("\r\n");
    let parts: Vec<_> = lines.next()?.split_whitespace().collect();
    if parts.len() != 3
        || parts[0] != "GET"
        || parts[2] != "HTTP/1.1"
        || !parts[1].starts_with("/oauth/oakos?")
    {
        return None;
    }
    let hosts: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim())
        .collect();
    if hosts != ["127.0.0.1:43821"] {
        return None;
    }
    let url = Url::parse(&format!("http://127.0.0.1:43821{}", parts[1])).ok()?;
    if url.path() != "/oauth/oakos" || url.fragment().is_some() {
        return None;
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let one = |name: &str| -> Option<String> {
        let values: Vec<_> = pairs.iter().filter(|(key, _)| key == name).collect();
        if values.len() == 1 {
            Some(values[0].1.to_string())
        } else {
            None
        }
    };
    if one("state").as_deref() != Some(state) || one("iss").as_deref() != Some(ISSUER) {
        return None;
    }
    let code = one("code");
    let error = one("error");
    match (code, error) {
        (Some(code), None) if !code.is_empty() && !pairs.iter().any(|(key, _)| key == "error") => {
            Some(Ok(code))
        }
        (None, Some(_)) if !pairs.iter().any(|(key, _)| key == "code") => {
            Some(Err(failure("sign-in was declined. Try connecting again")))
        }
        _ => None,
    }
}

fn open_browser(url: &str) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = std::process::Command::new("xdg-open");
    let status = command
        .arg(url)
        .status()
        .map_err(|_| failure("could not open your browser"))?;
    if status.success() {
        Ok(())
    } else {
        Err(failure("could not open your browser"))
    }
}
fn failure(message: &str) -> AppError {
    AppError::Other(format!("OakOS: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callback(query: &str) -> String {
        format!("GET /oauth/oakos?{query} HTTP/1.1\r\nHost: 127.0.0.1:43821\r\n\r\n")
    }
    fn valid_query() -> String {
        format!("state=expected&iss={ISSUER}&code=approved")
    }

    #[test]
    fn pkce_matches_rfc7636_and_requests_only_needed_permissions() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let url = authorization_url("verifier", "random-state");
        let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(query["redirect_uri"], REDIRECT_URI);
        assert_eq!(query["client_id"], CLIENT_ID);
        assert_eq!(query["scope"], SCOPES);
        assert_eq!(query["resource"], RESOURCE);
        assert_eq!(query["code_challenge_method"], "S256");
        assert!(!url.as_str().contains("code_verifier"));
        assert_eq!(random_secret().len(), 43);
        assert_ne!(random_secret(), random_secret());
    }

    #[test]
    fn callbacks_require_exact_state_issuer_path_host_and_unambiguous_parameters() {
        assert_eq!(
            parse_callback(&callback(&valid_query()), "expected")
                .unwrap()
                .unwrap(),
            "approved"
        );
        for query in [
            valid_query().replace("expected", "attacker"),
            valid_query().replace(ISSUER, "https://attacker.example"),
            "state=expected&code=approved".into(),
            format!("{}&state=expected", valid_query()),
            format!("{}&code=another", valid_query()),
            format!("{}&error=denied", valid_query()),
            format!("{}&error=a&error=b", valid_query()),
        ] {
            assert!(parse_callback(&callback(&query), "expected").is_none());
        }
        for request in [
            callback(&valid_query()).replace("Host: 127.0.0.1:43821", "Host: attacker.example"),
            callback(&valid_query()).replace("GET", "POST"),
            callback(&valid_query()).replace("/oauth/oakos?", "/other?"),
        ] {
            assert!(parse_callback(&request, "expected").is_none());
        }
        assert!(
            parse_callback(
                &callback(&format!("state=expected&iss={ISSUER}&error=access_denied")),
                "expected"
            )
            .unwrap()
            .is_err()
        );
    }

    fn token(refresh: Option<&str>) -> TokenResponse {
        TokenResponse {
            access_token: "access".into(),
            refresh_token: refresh.map(str::to_owned),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: Some(SCOPES.into()),
        }
    }
    #[test]
    fn refresh_rotation_and_grant_validation() {
        assert_eq!(
            token(Some("rotated"))
                .credentials(Some("old"))
                .unwrap()
                .refresh_token,
            "rotated"
        );
        assert_eq!(
            token(None).credentials(Some("old")).unwrap().refresh_token,
            "old"
        );
        assert!(token(None).credentials(None).is_err());
        let mut denied = token(Some("refresh"));
        denied.scope = Some("projects:read offline_access".into());
        assert!(denied.credentials(None).is_err());
        let mut invalid = token(Some("refresh"));
        invalid.token_type = "DPoP".into();
        assert!(invalid.credentials(None).is_err());
    }

    // Exercises real HTTP encoding/response handling without touching the user's keychain.
    fn token_server(
        status: &str,
        body: &str,
        headers: &str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let status = status.to_owned();
        let body = body.to_owned();
        let headers = headers.to_owned();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&chunk[..count]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let header = std::str::from_utf8(&request[..end]).unwrap();
                    let length: usize = header
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                        .unwrap()
                        .1
                        .trim()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n{body}", body.len()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (endpoint, worker)
    }

    #[test]
    fn token_exchange_encodes_secrets_and_handles_refresh_rotation() {
        let (endpoint, worker) = token_server(
            "200 OK",
            r#"{"access_token":"access","refresh_token":"rotated","token_type":"Bearer","expires_in":3600,"scope":"projects:read recordings:write offline_access"}"#,
            "",
        );
        let result = exchange_at(
            &http().unwrap(),
            &endpoint,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("resource", RESOURCE),
                ("refresh_token", "old+&=token"),
            ],
            Some("old+&=token"),
        )
        .unwrap();
        assert_eq!(result.refresh_token, "rotated");
        assert!(result.expires_at > now() + 3500);
        let request = worker.join().unwrap();
        assert!(
            request
                .to_lowercase()
                .contains("content-type: application/x-www-form-urlencoded")
        );
        let body = request.split_once("\r\n\r\n").unwrap().1;
        let fields: std::collections::HashMap<_, _> =
            Url::parse(&format!("http://localhost/?{body}"))
                .unwrap()
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
        assert_eq!(fields["refresh_token"], "old+&=token");
        assert_eq!(fields["client_id"], CLIENT_ID);
        assert_eq!(fields["resource"], RESOURCE);
        assert!(!fields.contains_key("client_secret"));
    }

    #[test]
    fn exchange_does_not_follow_redirects_or_leak_error_bodies() {
        for (status, body, headers) in [
            (
                "400 Bad Request",
                r#"{"error":"invalid_grant","error_description":"secret-code"}"#,
                "",
            ),
            ("503 Service Unavailable", "secret-code", ""),
            (
                "302 Found",
                "secret-code",
                "Location: http://127.0.0.1:1/steal\r\n",
            ),
        ] {
            let (endpoint, worker) = token_server(status, body, headers);
            let error = exchange_at(
                &http().unwrap(),
                &endpoint,
                &[("code", "secret-code")],
                None,
            )
            .err()
            .unwrap();
            if status.starts_with("400") {
                assert!(matches!(error, AppError::OakOsReauthentication));
            }
            assert!(!error.to_string().contains("secret-code"));
            worker.join().unwrap();
        }
    }

    #[test]
    fn cancellation_and_timeout_release_login_for_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let login = LoginGuard::start().unwrap();
        assert!(LoginGuard::start().is_err());
        cancel().unwrap();
        assert!(
            wait_for_code(&listener, "state", &login, Duration::from_secs(1))
                .unwrap_err()
                .to_string()
                .contains("canceled")
        );
        drop(login);
        let retry = LoginGuard::start().unwrap();
        assert!(
            wait_for_code(&listener, "state", &retry, Duration::ZERO)
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        drop(retry);
        let login = LoginGuard::start().unwrap();
        let address = listener.local_addr().unwrap();
        let sender = std::thread::spawn(move || {
            for state in ["attacker", "expected"] {
                let mut stream = TcpStream::connect(address).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream
                    .write_all(
                        callback(&format!("state={state}&iss={ISSUER}&code=approved")).as_bytes(),
                    )
                    .unwrap();
                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();
                assert!(response.contains(if state == "attacker" {
                    "400 Bad Request"
                } else {
                    "200 OK"
                }));
                assert!(!response.contains("code=approved"));
            }
        });
        assert_eq!(
            wait_for_code(&listener, "expected", &login, Duration::from_secs(3)).unwrap(),
            "approved"
        );
        sender.join().unwrap();
    }
}
