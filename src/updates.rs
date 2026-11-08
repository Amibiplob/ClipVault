//! Only public release metadata is requested. No Store or clipboard data enters this module.
use reqwest::{blocking::Client, header::HeaderMap, redirect::Policy, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub const API: &str = "https://api.github.com/repos/Amibiplob/ClipVault/releases/latest";
pub const DAY: i64 = 86400;
const MAX_RESPONSE: u64 = 512 * 1024;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub automatic: bool,
    pub cached_outcome: Option<Outcome>,
    pub last_attempt: Option<i64>,
    pub last_status: String,
    pub retry_after: i64,
    pub dismissed_version: Option<String>,
}
impl Preferences {
    pub fn due(&self, now: i64) -> bool {
        self.automatic
            && now >= self.retry_after
            && self
                .last_attempt
                .is_none_or(|last| now.saturating_sub(last) >= DAY)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Outcome {
    Current,
    Available { version: String, url: String },
    NoStable,
}
impl Outcome {
    pub fn message(&self) -> String {
        match self {
            Self::Current => "You’re up to date.".into(),
            Self::Available { version, .. } => format!("A newer version is available: {version}"),
            Self::NoStable => "No stable release is available.".into(),
        }
    }
}
#[derive(Debug)]
pub struct Failure {
    pub message: String,
    pub retry_after: i64,
}
impl Failure {
    fn unavailable(reason: &str) -> Self {
        Self {
            message: format!("Unable to check; try again. {reason}"),
            retry_after: 0,
        }
    }
}
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
}
pub fn official_release(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("github.com")
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && u.path().starts_with("/Amibiplob/ClipVault/releases/tag/")
            && u.path_segments().is_some_and(|s| s.count() == 5)
            && u.path().len() > "/Amibiplob/ClipVault/releases/tag/".len()
            && !u.path().to_ascii_lowercase().contains("%2f")
            && !u.path().to_ascii_lowercase().contains("%5c")
    })
}
fn version(tag: &str) -> Result<Version, Failure> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|_| {
        Failure::unavailable("Release version comparison is unavailable (invalid version tag).")
    })
}
fn compare(body: &[u8], installed: &str) -> Result<Outcome, Failure> {
    let r: Release = serde_json::from_slice(body)
        .map_err(|_| Failure::unavailable("GitHub returned an invalid release response."))?;
    if r.draft || r.prerelease {
        return Ok(Outcome::NoStable);
    }
    let latest = version(&r.tag_name)?;
    if !latest.pre.is_empty() {
        return Ok(Outcome::NoStable);
    }
    let installed = version(installed)?;
    if !official_release(&r.html_url) {
        return Err(Failure::unavailable(
            "GitHub returned an unexpected release link.",
        ));
    }
    // Build metadata does not affect SemVer precedence.
    if latest.cmp_precedence(&installed).is_gt() {
        Ok(Outcome::Available {
            version: latest.to_string(),
            url: r.html_url,
        })
    } else {
        Ok(Outcome::Current)
    }
}
fn rate_limit(status: u16, h: &HeaderMap, now: i64) -> Option<Failure> {
    if status != 403 && status != 429 {
        return None;
    }
    let integer = |key| {
        h.get(key)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
    };
    let retry = integer("retry-after")
        .map(|s| now.saturating_add(s.max(0)))
        .unwrap_or(now + 60);
    let reset = if integer("x-ratelimit-remaining") == Some(0) {
        integer("x-ratelimit-reset").unwrap_or(0)
    } else {
        0
    };
    Some(Failure {
        message: "Unable to check; try again. GitHub access is limited; wait before retrying."
            .into(),
        retry_after: retry.max(reset).max(now + 60),
    })
}
fn fetch_url(
    endpoint: &str,
    installed: &str,
    timeout: Duration,
    now: i64,
) -> Result<Outcome, Failure> {
    let client = Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout.min(Duration::from_secs(5)))
        .redirect(Policy::custom(|attempt| {
            let u = attempt.url();
            if attempt.previous().len() < 3
                && u.scheme() == "https"
                && u.host_str() == Some("api.github.com")
            {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .user_agent(concat!("ClipVault/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| Failure::unavailable("Could not initialize HTTPS."))?;
    let response = client
        .get(endpoint)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .send()
        .map_err(|e| {
            Failure::unavailable(if e.is_timeout() {
                "The request timed out."
            } else {
                "GitHub could not be reached."
            })
        })?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    process_response(status, &headers, response, installed, now)
}
fn process_response(
    status: u16,
    headers: &HeaderMap,
    body: impl Read,
    installed: &str,
    now: i64,
) -> Result<Outcome, Failure> {
    if let Some(f) = rate_limit(status, headers, now) {
        return Err(f);
    }
    if status == 404 {
        return Ok(Outcome::NoStable);
    }
    if status != 200 {
        return Err(Failure::unavailable(
            "GitHub returned an unsuccessful status.",
        ));
    }
    let mut bytes = Vec::new();
    body.take(MAX_RESPONSE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::unavailable("The release response was incomplete or timed out."))?;
    if bytes.len() as u64 > MAX_RESPONSE {
        return Err(Failure::unavailable("Release response was too large."));
    }
    compare(&bytes, installed)
}

#[derive(Default)]
pub struct Checker {
    receiver: Option<Receiver<Result<Outcome, Failure>>>,
    pub automatic: bool,
}
impl Checker {
    pub fn running(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn start(&mut self, automatic: bool, ctx: eframe::egui::Context) -> bool {
        if self.running() {
            return false;
        }
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.automatic = automatic;
        std::thread::spawn(move || {
            let _ = tx.send(fetch_url(
                API,
                env!("CARGO_PKG_VERSION"),
                Duration::from_secs(12),
                crate::storage::now(),
            ));
            ctx.request_repaint();
        });
        true
    }
    pub fn poll(&mut self) -> Option<Result<Outcome, Failure>> {
        let result = match self.receiver.as_ref()?.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(_) => Err(Failure::unavailable("The update worker stopped.")),
        };
        self.receiver = None;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn release(tag: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"tag_name": tag,"html_url":format!("https://github.com/Amibiplob/ClipVault/releases/tag/{tag}"),"draft":false,"prerelease":false})).unwrap()
    }
    #[test]
    fn semantic_versions_never_offer_downgrades() {
        for (installed, tag, newer) in [
            ("1.9.0", "v1.10.0", true),
            ("1.10.0", "1.9.0", false),
            ("1.1.0", "v1.1.0", false),
            ("1.1.0", "1.0.0", false),
            ("2.0.0", "1.1.0", false),
            ("1.1.0+local", "v1.1.0+other", false),
            ("1.1.0-rc.1", "1.1.0", true),
        ] {
            assert_eq!(
                matches!(
                    compare(&release(tag), installed).unwrap(),
                    Outcome::Available { .. }
                ),
                newer,
                "{installed} / {tag}"
            );
        }
    }
    #[test]
    fn rejects_malformed_and_ignores_unstable() {
        for tag in ["latest", "v1", "1.01.0", "", "1.2.3.4"] {
            assert!(compare(&release(tag), "1.1.0").is_err());
        }
        assert_eq!(
            compare(&release("v2.0.0-beta.1"), "1.1.0").unwrap(),
            Outcome::NoStable
        );
        for flag in ["draft", "prerelease"] {
            let mut r: serde_json::Value = serde_json::from_slice(&release("v2.0.0")).unwrap();
            r[flag] = true.into();
            assert_eq!(
                compare(&serde_json::to_vec(&r).unwrap(), "1.1.0").unwrap(),
                Outcome::NoStable
            );
        }
        for body in [b"offline".as_slice(), b"{}", b"[]"] {
            assert!(compare(body, "1.1.0").is_err());
        }
    }
    #[test]
    fn official_urls_only() {
        assert!(official_release(
            "https://github.com/Amibiplob/ClipVault/releases/tag/v1.2.0"
        ));
        for u in [
            "http://github.com/Amibiplob/ClipVault/releases/tag/v2",
            "https://evil.test/Amibiplob/ClipVault/releases/tag/v2",
            "https://github.com/other/ClipVault/releases/tag/v2",
            "https://github.com/Amibiplob/ClipVault/releases/tag/../download/x",
            "https://github.com@evil.test/Amibiplob/ClipVault/releases/tag/v2",
        ] {
            assert!(!official_release(u), "{u}");
        }
    }
    fn mock(status: &str, headers: &str, body: Vec<u8>, delay: Duration) -> String {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let status = status.to_string();
        let headers = headers.to_string();
        std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            let mut req = [0; 2048];
            let n = socket.read(&mut req).unwrap();
            let request = String::from_utf8_lossy(&req[..n]);
            assert!(
                request.contains("X-GitHub-Api-Version: 2026-03-10")
                    || request.contains("x-github-api-version: 2026-03-10")
            );
            assert!(!request.contains("clipboard"));
            std::thread::sleep(delay);
            let _ = write!(
                socket,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(&body);
        });
        format!("http://{addr}/release")
    }
    #[test]
    #[ignore = "Requires loopback sockets; sandbox denies TcpListener::bind. Equivalent response tests run without sockets."]
    fn mocked_http_success_empty_malformed_limits_timeout_offline() {
        let fetch = |url: &str| fetch_url(url, "1.1.0", Duration::from_secs(2), 1000);
        assert!(matches!(
            fetch(&mock("200 OK", "", release("v1.2.0"), Duration::ZERO)).unwrap(),
            Outcome::Available { .. }
        ));
        assert_eq!(
            fetch(&mock("404 Not Found", "", vec![], Duration::ZERO)).unwrap(),
            Outcome::NoStable
        );
        for (status, body) in [
            ("500 Error", vec![]),
            ("200 OK", b"{broken".to_vec()),
            ("200 OK", vec![b'x'; MAX_RESPONSE as usize + 1]),
        ] {
            assert!(fetch(&mock(status, "", body, Duration::ZERO)).is_err());
        }
        for status in ["403 Forbidden", "429 Too Many Requests"] {
            let e = fetch(&mock(
                status,
                "Retry-After: 120\r\nX-RateLimit-Remaining: 0\r\nX-RateLimit-Reset: 1400\r\n",
                vec![],
                Duration::ZERO,
            ))
            .unwrap_err();
            assert_eq!(e.retry_after, 1400);
        }
        let url = mock("200 OK", "", release("v1.2.0"), Duration::from_millis(200));
        assert!(fetch_url(&url, "1.1.0", Duration::from_millis(30), 1000).is_err());
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        drop(socket);
        assert!(fetch(&format!("http://{addr}/release")).is_err());
    }
    #[test]
    fn mocked_responses_without_network_cover_errors_and_rate_limits() {
        let mut h = HeaderMap::new();
        let fetch = |status, h: &HeaderMap, bytes: Vec<u8>| {
            process_response(status, h, std::io::Cursor::new(bytes), "1.1.0", 1000)
        };
        assert!(matches!(
            fetch(200, &h, release("v1.2.0")).unwrap(),
            Outcome::Available { .. }
        ));
        assert_eq!(fetch(404, &h, vec![]).unwrap(), Outcome::NoStable);
        assert!(fetch(500, &h, vec![]).is_err());
        for body in [b"{broken".to_vec(), vec![b'x'; MAX_RESPONSE as usize + 1]] {
            assert!(fetch(200, &h, body).is_err());
        }
        h.insert("retry-after", "120".parse().unwrap());
        h.insert("x-ratelimit-remaining", "0".parse().unwrap());
        h.insert("x-ratelimit-reset", "1400".parse().unwrap());
        for status in [403, 429] {
            assert_eq!(fetch(status, &h, vec![]).unwrap_err().retry_after, 1400);
        }
        struct Broken(std::io::ErrorKind);
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(self.0.into())
            }
        }
        for kind in [
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::NotConnected,
            std::io::ErrorKind::UnexpectedEof,
        ] {
            assert!(process_response(200, &HeaderMap::new(), Broken(kind), "1.1.0", 1000).is_err());
        }
    }
    #[test]
    fn concurrent_check_is_rejected_without_starting_another_worker() {
        let (_tx, rx) = mpsc::channel();
        let mut checker = Checker {
            receiver: Some(rx),
            automatic: false,
        };
        assert!(!checker.start(false, eframe::egui::Context::default()));
        assert!(checker.running());
    }
    #[test]
    fn optional_daily_schedule_backoff_and_restart() {
        let mut p = Preferences::default();
        assert!(!p.due(100000));
        p.automatic = true;
        assert!(p.due(100000));
        p.last_attempt = Some(100000);
        assert!(!p.due(100001));
        assert!(!p.due(99999));
        assert!(p.due(100000 + DAY));
        p.retry_after = 300000;
        assert!(!p.due(200000));
        p.dismissed_version = Some("1.2.0".into());
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(restored.dismissed_version, p.dismissed_version);
        assert!(!restored.due(200000));
        p.automatic = false;
        assert!(!p.due(900000));
    }
}
