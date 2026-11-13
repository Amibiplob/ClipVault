//! Signed Linux updates. Package ownership selects the running installation, never ID_LIKE.
//! No clipboard contents or administrator passwords enter this module.
use reqwest::{blocking::Client, redirect::Policy, Url};
use ring::{digest, signature};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, String>;
const RELEASE_ROOT: &str = "https://github.com/Amibiplob/ClipVault/releases/download/";
const MAX_ARTIFACT: u64 = 1024 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Package {
    Deb,
    Rpm,
    Appimage,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub id: String,
    pub release: String,
}
#[derive(Clone, Debug)]
pub struct Installation {
    pub path: PathBuf,
    pub package: Option<Package>,
    pub arch: String,
    pub platform: Platform,
    pub repository_managed: bool,
    pub detail: String,
}
impl Installation {
    pub fn label(&self) -> String {
        format!(
            "{} · {} · {} {}",
            self.detail, self.arch, self.platform.id, self.platform.release
        )
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub filename: String,
    pub url: String,
    pub package: Package,
    pub arch: String,
    pub platforms: Vec<Platform>,
    pub sha256: String,
    pub size: u64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub repository: String,
    pub version: String,
    pub release_notes: String,
    pub artifacts: Vec<Artifact>,
}
#[derive(Clone)]
pub struct Plan {
    pub installation: Installation,
    pub artifact: Artifact,
    pub version: String,
}
#[derive(Clone, Debug)]
pub enum State {
    Idle,
    Detecting,
    Checking,
    Ready,
    Downloading { received: u64, total: u64 },
    Verifying,
    Authorization,
    Installing,
    RestartRequired(String),
    Completed,
    Saved(PathBuf),
    Cancelled,
    Failed(String),
}
impl State {
    pub fn message(&self) -> String {
        match self {
            Self::Idle => "Check for a newer release to prepare an update.".into(),
            Self::Detecting => "Detecting the running installation…".into(),
            Self::Checking => "Checking signed release metadata…".into(),
            Self::Ready => "Update available. Signature verified; ready to download.".into(),
            Self::Downloading { received, total } => format!("Downloading: {:.1} / {:.1} MiB", *received as f64 / 1048576., *total as f64 / 1048576.),
            Self::Verifying => "Verifying download…".into(),
            Self::Authorization => "Waiting for administrator authorization. Use the system authentication dialog; ClipVault never receives your password.".into(),
            Self::Installing => "Installing. Package installation cannot safely be interrupted; please keep ClipVault running.".into(),
            Self::RestartRequired(v) => format!("Restart required to use ClipVault {v}. Save or cancel any open edit first."),
            Self::Completed => format!("Update completed. Running ClipVault {}.", env!("CARGO_PKG_VERSION")),
            Self::Saved(path) => format!("Verified AppImage saved to {}. Launch it manually; existing launcher references still point to the original location.", path.display()),
            Self::Cancelled => "Update cancelled. The existing installation is unchanged.".into(),
            Self::Failed(e) => format!("Failed: {e}"),
        }
    }
    pub fn cancellable(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Downloading { .. } | Self::Verifying
        )
    }
    pub fn critical(&self) -> bool {
        matches!(self, Self::Authorization | Self::Installing)
    }
}
enum Event {
    Detection(Installation),
    State(State),
    Prepared(Plan),
}
pub struct Updater {
    pub installation: Option<Installation>,
    pub state: State,
    pub plan: Option<Plan>,
    receiver: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    initialized: bool,
}
impl Default for Updater {
    fn default() -> Self {
        Self {
            installation: None,
            state: State::Idle,
            plan: None,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            initialized: false,
        }
    }
}
impl Updater {
    pub fn busy(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn cancel(&self) {
        if self.state.cancellable() {
            self.cancel.store(true, Ordering::SeqCst);
        }
    }
    fn worker(
        &mut self,
        ctx: eframe::egui::Context,
        work: impl FnOnce(&dyn Fn(Event), &AtomicBool) -> Result<()> + Send + 'static,
    ) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            let emit = |event| {
                let _ = tx.send(event);
                ctx.request_repaint();
            };
            if let Err(error) = work(&emit, &cancel) {
                emit(Event::State(if error == "Cancelled" {
                    State::Cancelled
                } else {
                    State::Failed(error)
                }));
            }
        });
    }
    pub fn initialize(&mut self, ctx: &eframe::egui::Context) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.state = State::Detecting;
        self.worker(ctx.clone(), |emit, _| {
            emit(Event::Detection(detect()));
            emit(Event::State(State::Idle));
            Ok(())
        });
    }
    pub fn poll(&mut self) {
        let Some(rx) = &self.receiver else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(Event::Detection(i)) => self.installation = Some(i),
                Ok(Event::State(s)) => self.state = s,
                Ok(Event::Prepared(plan)) => {
                    self.plan = Some(plan);
                    self.state = State::Ready;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if matches!(
                        self.state,
                        State::Detecting
                            | State::Checking
                            | State::Downloading { .. }
                            | State::Verifying
                            | State::Authorization
                            | State::Installing
                    ) {
                        self.state=State::Failed("The update worker stopped unexpectedly. Check your system package manager before retrying if installation had started.".into());
                    }
                    self.receiver = None;
                    break;
                }
            }
        }
    }
    pub fn prepare(&mut self, version: String, release: String, ctx: &eframe::egui::Context) {
        if self.busy() {
            return;
        }
        self.plan = None;
        self.state = State::Checking;
        self.worker(ctx.clone(), move |emit, cancel| {
            let installation = detect(); emit(Event::Detection(installation.clone()));
            if installation.package.is_none() { return Err(installation.detail); }
            if installation.repository_managed { return Err("This installation is repository-managed. Use your system Update Manager to coordinate this update, or review the official release page.".into()); }
            let tag = release.strip_prefix("https://github.com/Amibiplob/ClipVault/releases/tag/").filter(|_| crate::updates::official_release(&release)).ok_or("Unexpected release source")?;
            let base = format!("{RELEASE_ROOT}{tag}/");
            let keys: Vec<String> = serde_json::from_str(include_str!("../packaging/update-keys.json")).map_err(|_| "Invalid embedded update trust store")?;
            if keys.is_empty() { return Err("Automatic installation is unavailable: the maintainer has not configured a trusted release-signing key. Use official manual downloads until a signed build is published.".into()); }
            let client = client()?;
            let manifest = fetch_small(&client, &format!("{base}clipvault-update.json"), 512 * 1024, cancel)?;
            let sig = fetch_small(&client, &format!("{base}clipvault-update.json.sig"), 64, cancel)?;
            let metadata = verify_manifest(&manifest, &sig, &keys)?;
            if metadata.version != version || crate::updates::parse_version(&version)?.pre != semver::Prerelease::EMPTY { return Err("Signed manifest version does not match the stable release".into()); }
            if !crate::updates::parse_version(&version)?.cmp_precedence(&crate::updates::parse_version(env!("CARGO_PKG_VERSION"))?).is_gt() { return Err("The release is not newer than this app".into()); }
            let artifact = select(&metadata, &installation)?.clone();
            if !artifact.url.starts_with(&base) { return Err("Artifact is outside the selected official release".into()); }
            check_cancel(cancel)?;
            emit(Event::Prepared(Plan { installation, artifact, version })); Ok(())
        });
    }
    pub fn install(&mut self, ctx: &eframe::egui::Context) {
        self.install_inner(ctx, false);
    }
    pub fn save_appimage(&mut self, ctx: &eframe::egui::Context) {
        self.install_inner(ctx, true);
    }
    fn install_inner(&mut self, ctx: &eframe::egui::Context, save_copy: bool) {
        if self.busy() {
            return;
        }
        let Some(plan) = self.plan.clone() else {
            return;
        };
        self.state = State::Downloading {
            received: 0,
            total: plan.artifact.size,
        };
        self.worker(ctx.clone(), move |emit, cancel| {
            let current = detect();
            if current.path != plan.installation.path
                || current.package != plan.installation.package
                || current.arch != plan.installation.arch
                || current.repository_managed
            {
                return Err("Installation changed; check for updates again".into());
            }
            let destination = if save_copy {
                if plan.artifact.package != Package::Appimage {
                    return Err("Save copy is only available for AppImages".into());
                }
                Some(choose_save_path(&plan.artifact.filename)?)
            } else {
                None
            };
            let temp = Temp::new()?;
            let path = temp.0.join(&plan.artifact.filename);
            download(&client()?, &plan.artifact, &path, cancel, &|received| {
                emit(Event::State(State::Downloading {
                    received,
                    total: plan.artifact.size,
                }))
            })?;
            emit(Event::State(State::Verifying));
            verify_file(&path, &plan.artifact)?;
            verify_package(&path, &plan, &temp.0)?;
            check_cancel(cancel)?;
            if plan.artifact.package == Package::Appimage {
                emit(Event::State(State::Installing));
                if let Some(destination) = destination {
                    save_appimage_copy(&path, &destination, &plan.artifact)?;
                    emit(Event::State(State::Saved(destination)));
                    return Ok(());
                }
                replace_appimage(&path, &plan.installation.path, &plan.artifact)?;
            } else {
                emit(Event::State(State::Authorization));
                install_package(&path, &plan, &temp.0, &|| {
                    emit(Event::State(State::Installing))
                })?;
            }
            emit(Event::State(State::RestartRequired(plan.version)));
            Ok(())
        });
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}
fn arch(value: &str) -> String {
    match value.trim() {
        "amd64" | "x86_64" => "x86_64",
        "arm64" | "aarch64" => "aarch64",
        _ => value.trim(),
    }
    .to_string()
}
fn platform() -> Platform {
    let text = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let value = |key| {
        text.lines()
            .find_map(|l| {
                l.strip_prefix(&format!("{key}="))
                    .map(|s| s.trim_matches('"').to_string())
            })
            .unwrap_or_default()
    };
    Platform {
        id: value("ID"),
        release: value("VERSION_ID"),
    }
}
/// Each command has a timeout and bounded output. Never invoke a shell with release data.
fn output(program: &str, args: &[&std::ffi::OsStr]) -> Result<String> {
    let temp = Temp::new()?;
    let out = File::create(temp.0.join("stdout")).map_err(|e| e.to_string())?;
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let mut bytes = Vec::new();
            File::open(temp.0.join("stdout"))
                .and_then(|f| f.take(1024 * 1024 + 1).read_to_end(&mut bytes))
                .map_err(|e| e.to_string())?;
            if !status.success() || bytes.len() > 1024 * 1024 {
                return Err(format!(
                    "{program} could not validate this installation/package"
                ));
            }
            return String::from_utf8(bytes).map_err(|e| e.to_string());
        }
        if start.elapsed() > Duration::from_secs(20)
            || fs::metadata(temp.0.join("stdout")).is_ok_and(|m| m.len() > 1024 * 1024)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program} timed out or produced too much output"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn args(values: &[&str]) -> Vec<std::ffi::OsString> {
    values.iter().map(std::ffi::OsString::from).collect()
}
fn query(program: &str, values: &[&str]) -> Result<String> {
    let a = args(values);
    output(
        program,
        &a.iter().map(|x| x.as_os_str()).collect::<Vec<_>>(),
    )
}
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}
fn appimage_runtime(exe: &Path, image: &Path, appdir: &Path) -> bool {
    exe.starts_with(appdir)
        && image.is_absolute()
        && image.is_file()
        && appdir.is_absolute()
        && appdir != Path::new("/")
        && is_appimage(image).is_ok()
}
fn is_appimage(path: &Path) -> Result<String> {
    let mut header = [0; 64];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|e| e.to_string())?;
    if &header[..4] != b"\x7fELF" || &header[8..11] != b"AI\x02" || header[5] != 1 {
        return Err("Not a supported type-2 AppImage".into());
    }
    match u16::from_le_bytes([header[18], header[19]]) {
        62 => Ok("x86_64".into()),
        183 => Ok("aarch64".into()),
        _ => Err("Unsupported AppImage CPU architecture".into()),
    }
}
fn classify(deb: bool, rpm: bool, appimage: bool) -> Option<Package> {
    match (deb, rpm, appimage) {
        (true, false, false) => Some(Package::Deb),
        (false, true, false) => Some(Package::Rpm),
        (false, false, true) => Some(Package::Appimage),
        _ => None,
    }
}
fn detect() -> Installation {
    let exe = std::env::current_exe().unwrap_or_default();
    let mut path = exe.clone();
    let appimage = match (std::env::var_os("APPIMAGE"), std::env::var_os("APPDIR")) {
        (Some(image), Some(dir)) if appimage_runtime(&exe, Path::new(&image), Path::new(&dir)) => {
            path = fs::canonicalize(&image).unwrap_or_else(|_| image.into());
            true
        }
        _ => false,
    };
    // Ownership is queried for this executable, not inferred from installed package managers.
    let deb_owner = query("dpkg-query", &["-S", &exe.to_string_lossy()]).is_ok_and(|s| {
        s.lines().any(|l| {
            l.strip_suffix(&format!(": {}", exe.display()))
                .is_some_and(|p| p == "clipvault" || p.starts_with("clipvault:"))
        })
    });
    let deb = deb_owner
        && query(
            "dpkg-query",
            &["-W", "-f=${Status}\n${Architecture}", "clipvault"],
        )
        .is_ok_and(|s| {
            s.starts_with("install ok installed\n")
                && s.lines()
                    .nth(1)
                    .is_some_and(|a| arch(a) == arch(std::env::consts::ARCH))
        })
        && same_file(&exe, Path::new("/usr/bin/clipvault"));
    let rpm = query(
        "rpm",
        &["-qf", "--qf", "%{NAME}\n%{ARCH}\n", &exe.to_string_lossy()],
    )
    .is_ok_and(|s| {
        s.lines().next() == Some("clipvault")
            && s.lines()
                .nth(1)
                .is_some_and(|a| arch(a) == arch(std::env::consts::ARCH))
    }) && same_file(&exe, Path::new("/usr/bin/clipvault"));
    let package = classify(deb && !appimage, rpm && !appimage, appimage);
    let repository_managed = match package {
        Some(Package::Deb) => query("apt-cache", &["policy", "clipvault"]).is_ok_and(|s| {
            s.lines()
                .any(|l| l.contains("http://") || l.contains("https://") || l.contains("file:"))
        }),
        Some(Package::Rpm) => query(
            "dnf",
            &[
                "-q",
                "--cacheonly",
                "repoquery",
                "--available",
                "--qf",
                "%{name}",
                "clipvault",
            ],
        )
        .is_ok_and(|s| s.lines().any(|l| l == "clipvault")),
        _ => false,
    };
    let detail = match package { Some(Package::Deb) => ".deb (package-owned executable)", Some(Package::Rpm) => ".rpm (package-owned executable)", Some(Package::Appimage) => "AppImage (running runtime)", None => "Automatic detection unavailable: development, extracted, unsupported or ambiguous installation. Choose a manual download for your system." }.into();
    Installation {
        path,
        package,
        arch: arch(std::env::consts::ARCH),
        platform: platform(),
        repository_managed,
        detail,
    }
}
fn official_asset(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("github.com")
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && u.as_str().starts_with(RELEASE_ROOT)
            && u.path_segments().is_some_and(|s| s.count() == 6)
            && !u.path().contains('%')
            && !u.path().contains('\\')
    })
}
fn redirect_allowed(u: &Url) -> bool {
    u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && (official_asset(u.as_str())
            || (u.host_str() == Some("release-assets.githubusercontent.com")
                && u.path().starts_with("/github-production-release-asset/")))
}
fn client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("ClipVault/", env!("CARGO_PKG_VERSION")))
        .redirect(Policy::custom(|a| {
            if a.previous().len() < 4 && redirect_allowed(a.url()) {
                a.follow()
            } else {
                a.stop()
            }
        }))
        .build()
        .map_err(|e| e.to_string())
}
fn fetch_small(client: &Client, url: &str, limit: u64, cancel: &AtomicBool) -> Result<Vec<u8>> {
    if !official_asset(url) {
        return Err("Unexpected metadata source".into());
    }
    check_cancel(cancel)?;
    let response = client
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .map_err(|e| format!("Metadata download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Signed metadata is unavailable; open the release page. {e}"))?;
    if !response.status().is_success() || !redirect_allowed(response.url()) {
        return Err("Unexpected metadata destination".into());
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    check_cancel(cancel)?;
    if bytes.len() as u64 > limit {
        return Err("Metadata exceeds its size limit".into());
    }
    Ok(bytes)
}
fn unhex(value: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid hex encoding".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
fn verify_manifest(bytes: &[u8], sig: &[u8], keys: &[String]) -> Result<Manifest> {
    if keys.is_empty()
        || !keys.iter().any(|key| {
            unhex(key).is_ok_and(|key| {
                key.len() == 32
                    && signature::UnparsedPublicKey::new(&signature::ED25519, key)
                        .verify(bytes, sig)
                        .is_ok()
            })
        })
    {
        return Err("Release metadata signature is invalid or its key is not trusted. Nothing was installed.".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid signed manifest: {e}"))?;
    if manifest.schema != 1 || manifest.repository != "Amibiplob/ClipVault" {
        return Err("Wrong manifest schema or application identity".into());
    }
    Ok(manifest)
}
fn select<'a>(m: &'a Manifest, i: &Installation) -> Result<&'a Artifact> {
    let matches: Vec<_> = m
        .artifacts
        .iter()
        .filter(|a| {
            i.package.as_ref() == Some(&a.package)
                && a.arch == i.arch
                && (a.package == Package::Appimage || a.platforms.contains(&i.platform))
        })
        .collect();
    if matches.len() != 1 {
        return Err("No unique compatible release asset exists for this installation, distribution/release and CPU. Open the release page for manual choices; another package type will not be selected.".into());
    }
    let a = matches[0];
    if !official_asset(&a.url)
        || a.filename.is_empty()
        || a.filename.contains(['/', '\\'])
        || a.filename == "."
        || a.filename == ".."
        || !a.url.ends_with(&format!("/{}", a.filename))
        || unhex(&a.sha256)?.len() != 32
        || a.size == 0
        || a.size > MAX_ARTIFACT
    {
        return Err("Invalid artifact metadata".into());
    }
    let extension = match a.package {
        Package::Deb => ".deb",
        Package::Rpm => ".rpm",
        Package::Appimage => ".AppImage",
    };
    if !a.filename.ends_with(extension) {
        return Err("Artifact package type and filename disagree".into());
    }
    Ok(a)
}
fn free_space(path: &Path, needed: u64) -> Result<()> {
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err("Could not check available disk space".into());
    }
    let stats = unsafe { stats.assume_init() };
    if (stats.f_bavail as u128) * (stats.f_frsize as u128) < needed as u128 {
        return Err("Insufficient disk space. Free space and retry; the existing installation was not replaced.".into());
    }
    Ok(())
}
fn download(
    client: &Client,
    a: &Artifact,
    path: &Path,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64),
) -> Result<()> {
    if !official_asset(&a.url) {
        return Err("Unexpected artifact source".into());
    }
    free_space(
        path.parent().ok_or("Missing download directory")?,
        a.size.saturating_mul(3).saturating_add(32 * 1024 * 1024),
    )?;
    let mut response = client
        .get(&a.url)
        .send()
        .map_err(|e| format!("Download failed: {e}. Retry the update."))?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success()
        || !redirect_allowed(response.url())
        || response.content_length().is_some_and(|n| n != a.size)
    {
        return Err("Unexpected download destination or size".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    copy_download(&mut response, &mut file, a.size, cancel, progress)?;
    file.sync_all().map_err(|e| e.to_string())
}
fn copy_download(
    reader: &mut impl Read,
    file: &mut impl Write,
    size: u64,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64),
) -> Result<()> {
    let mut received = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        check_cancel(cancel)?;
        let n = reader
            .read(&mut buffer)
            .map_err(|e| format!("Download interrupted: {e}. Retry the update."))?;
        if n == 0 {
            break;
        }
        received += n as u64;
        if received > size {
            return Err("Download exceeds signed size".into());
        }
        file.write_all(&buffer[..n])
            .map_err(|e| format!("Could not save download (check disk space): {e}"))?;
        progress(received);
    }
    if received != size {
        return Err("Download interrupted or incomplete. Retry the update.".into());
    }
    Ok(())
}

fn verify_file(path: &Path, a: &Artifact) -> Result<()> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() != a.size {
        return Err("Download size does not match signed metadata".into());
    }
    let mut context = digest::Context::new(&digest::SHA256);
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let n = f.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        context.update(&buffer[..n]);
    }
    if context.finish().as_ref() != unhex(&a.sha256)? {
        return Err("SHA-256 checksum mismatch. Nothing was installed.".into());
    }
    Ok(())
}
fn package_fields(text: &str, version: &str, expected_arch: &str, rpm: bool) -> Result<()> {
    let fields: Vec<_> = text.trim().split('\n').collect();
    let package_version = if rpm {
        version.to_string()
    } else {
        format!("{version}-1")
    };
    if fields.len() != 3
        || fields[0] != "clipvault"
        || fields[1] != package_version
        || arch(fields[2]) != expected_arch
    {
        return Err(
            "Package identity, version or CPU architecture does not match signed release metadata"
                .into(),
        );
    }
    Ok(())
}
fn verify_package(path: &Path, p: &Plan, temp: &Path) -> Result<()> {
    let path_str = path.to_str().ok_or("Non-UTF8 download path")?;
    match p.artifact.package {
        Package::Deb => {
            let mut text = String::new();
            for key in ["Package", "Version", "Architecture"] {
                text.push_str(query("dpkg-deb", &["-f", path_str, key])?.trim());
                text.push('\n');
            }
            package_fields(&text, &p.version, &p.artifact.arch, false)?;
            if !query("dpkg-deb", &["-c", path_str])?
                .lines()
                .any(|l| l.ends_with(" ./usr/bin/clipvault"))
            {
                return Err("Package does not contain the ClipVault executable".into());
            }
        }
        Package::Rpm => {
            package_fields(
                &query(
                    "rpm",
                    &["-qp", "--qf", "%{NAME}\n%{VERSION}\n%{ARCH}\n", path_str],
                )?,
                &p.version,
                &p.artifact.arch,
                true,
            )?;
            let distribution = query("rpm", &["-qp", "--qf", "%{DISTRIBUTION}", path_str])?;
            if p.installation.platform.id != "fedora"
                || distribution.trim() != format!("Fedora {}", p.installation.platform.release)
            {
                return Err("RPM distribution/release is incompatible. Only the tested matching Fedora release is supported.".into());
            }
            if !query("rpm", &["-qpl", path_str])?
                .lines()
                .any(|l| l == "/usr/bin/clipvault")
            {
                return Err("RPM is missing the ClipVault executable".into());
            }
        }
        Package::Appimage => {
            if is_appimage(path)? != p.artifact.arch {
                return Err("AppImage architecture mismatch".into());
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            // The code is executed only AFTER signature and checksum verification.
            // Extraction avoids requiring FUSE and keeps inherited AppImage libraries out.
            let out = File::create(temp.join("extract.log")).map_err(|e| e.to_string())?;
            let mut command = Command::new(path);
            command
                .arg("--appimage-extract")
                .current_dir(temp)
                .env_remove("LD_LIBRARY_PATH")
                .env_remove("APPIMAGE")
                .env_remove("APPDIR")
                .stdin(Stdio::null())
                .stdout(out)
                .stderr(Stdio::null());
            let mut child = command.spawn().map_err(|e| e.to_string())?;
            let start = Instant::now();
            loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    if !status.success() {
                        return Err(
                            "Verified AppImage could not be extracted for identity validation"
                                .into(),
                        );
                    }
                    break;
                }
                if start.elapsed() > Duration::from_secs(60) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("AppImage validation timed out".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let binary = temp.join("squashfs-root/usr/bin/clipvault");
            if crate::restart::probe(&binary, Duration::from_secs(5))?
                != crate::updates::parse_version(&p.version)?
            {
                return Err("AppImage application identity/version mismatch".into());
            }
        }
    }
    Ok(())
}
fn package_failure(code: Option<i32>, text: &str) -> String {
    let lower = text.to_lowercase();
    if matches!(code, Some(126 | 127)) {
        "Administrator authorization was cancelled or denied. Retry when ready.".into()
    } else if lower.contains("lock") {
        "The system package manager is busy or locked. Let the other installation finish, then retry. No locks were removed.".into()
    } else if lower.contains("space") {
        "Insufficient disk space during package installation. Free space, check your system package manager and retry.".into()
    } else {
        format!("System package installation failed (exit {code:?}). Review dependencies in your system package manager, then retry. {}", text.chars().rev().take(1600).collect::<String>().chars().rev().collect::<String>())
    }
}
fn install_package(path: &Path, p: &Plan, temp: &Path, installing: &dyn Fn()) -> Result<()> {
    let log = temp.join("install.log");
    let stdout = File::create(&log).map_err(|e| e.to_string())?;
    let mut command = Command::new("pkexec");
    match p.artifact.package {
        Package::Deb => {
            command
                .arg("/usr/bin/apt-get")
                .args(["-y", "--no-remove", "install", "--"])
                .arg(path);
        }
        Package::Rpm => {
            command
                .arg("/usr/bin/dnf")
                .args(["-y", "--setopt=install_weak_deps=False", "install", "--"])
                .arg(path);
        }
        _ => return Err("Unexpected package installer".into()),
    }
    let mut child = command
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
        .stderr(stdout)
        .spawn()
        .map_err(|e| {
            format!("System authorization could not start: {e}. Use your system package installer.")
        })?;
    // pkexec's child starts only after authorization; the package log becomes nonempty then.
    let mut reported = false;
    let status = loop {
        if !reported && fs::metadata(&log).is_ok_and(|m| m.len() > 0) {
            installing();
            reported = true;
        }
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut text = String::new();
    File::open(&log)
        .and_then(|f| f.take(64 * 1024).read_to_string(&mut text))
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(package_failure(status.code(), &text));
    }
    let installed = match p.artifact.package {
        Package::Deb => query(
            "dpkg-query",
            &[
                "-W",
                "-f=${Package}\n${Version}\n${Architecture}",
                "clipvault",
            ],
        ),
        _ => query(
            "rpm",
            &["-q", "--qf", "%{NAME}\n%{VERSION}\n%{ARCH}\n", "clipvault"],
        ),
    }?;
    package_fields(
        &installed,
        &p.version,
        &p.artifact.arch,
        p.artifact.package == Package::Rpm,
    )?;
    if crate::restart::probe(&p.installation.path, Duration::from_secs(5))?
        != crate::updates::parse_version(&p.version)?
    {
        return Err("Package manager returned success, but the installed executable did not report the expected version. Restart manually after checking your system package manager.".into());
    }
    Ok(())
}
fn choose_save_path(filename: &str) -> Result<PathBuf> {
    // Native chooser runs as the current user. No shell interpolation or overwrite is allowed.
    let result = Command::new("zenity").args(["--file-selection", "--save", "--confirm-overwrite", "--title=Save verified ClipVault AppImage", "--filename"]).arg(filename).output()
        .or_else(|_| Command::new("kdialog").args(["--getsavefilename", filename, "*.AppImage"]).output())
        .map_err(|_| "No desktop Save As dialog is available. Open the official release page and use your browser to save the matching AppImage to a writable location.")?;
    if !result.status.success() {
        return Err("Cancelled".into());
    }
    let path = PathBuf::from(
        String::from_utf8(result.stdout)
            .map_err(|e| e.to_string())?
            .trim(),
    );
    if !path.is_absolute() || path.exists() {
        return Err("Choose a new absolute filename in a writable folder; existing files are never overwritten by Save copy.".into());
    }
    Ok(path)
}
fn save_appimage_copy(source: &Path, destination: &Path, a: &Artifact) -> Result<()> {
    let parent = destination.parent().ok_or("No destination folder")?;
    free_space(parent, a.size.saturating_add(16 * 1024 * 1024))?;
    let staged = parent.join(format!(".clipvault-save-{}", std::process::id()));
    let result = (|| {
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staged)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut File::open(source).map_err(|e| e.to_string())?, &mut f)
            .map_err(|e| e.to_string())?;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        verify_file(&staged, a)?;
        // An atomic link publishes a complete file and fails rather than overwriting anything.
        fs::hard_link(&staged, destination).map_err(|e| format!("Could not save AppImage: {e}"))?;
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(staged);
    result
}
fn replace_appimage(source: &Path, destination: &Path, artifact: &Artifact) -> Result<()> {
    let parent = destination.parent().ok_or("Missing AppImage directory")?;
    free_space(parent, artifact.size.saturating_add(16 * 1024 * 1024))?;
    let staged = parent.join(format!(".clipvault-{}-new", std::process::id()));
    let backup = parent.join(format!(".clipvault-{}-rollback", std::process::id()));
    let result = (|| {
        let old = fs::symlink_metadata(destination).map_err(|e| e.to_string())?;
        if !old.is_file() {
            return Err("The running AppImage is no longer a regular file".into());
        }
        let mut file = OpenOptions::new().write(true).create_new(true).open(&staged).map_err(|e| format!("AppImage location is not writable: {e}. Download the matching AppImage from the release page and save it to a writable location using your browser's Save As dialog."))?;
        std::io::copy(
            &mut File::open(source).map_err(|e| e.to_string())?,
            &mut file,
        )
        .map_err(|e| e.to_string())?;
        fs::set_permissions(
            &staged,
            fs::Permissions::from_mode((old.permissions().mode() & 0o777) | 0o100),
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        verify_file(&staged, artifact)?;
        fs::hard_link(destination, &backup).map_err(|e| format!("Cannot create AppImage rollback copy: {e}. Existing image retained; use a manual download."))?;
        fs::rename(&staged, destination).map_err(|e| e.to_string())?;
        if let Err(e) = File::open(parent).and_then(|f| f.sync_all()) {
            let _ = fs::rename(&backup, destination);
            return Err(format!("AppImage replacement could not be committed: {e}. Rollback attempted; check the original location."));
        }
        // Keep the previous complete image for manual rollback until the next launch is confirmed.
        Ok(())
    })();
    let _ = fs::remove_file(staged);
    result
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Result<Self> {
        for n in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "clipvault-update-{}-{}-{n}",
                std::process::id(),
                crate::storage::now()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                        .map_err(|e| e.to_string())?;
                    return Ok(Self(path));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Could not allocate a private update directory".into())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };
    fn artifact(package: Package) -> Artifact {
        Artifact {
            filename: "clipvault.deb".into(),
            url: format!("{RELEASE_ROOT}v2.2.0/clipvault.deb"),
            package,
            arch: "x86_64".into(),
            platforms: vec![Platform {
                id: "fedora".into(),
                release: "44".into(),
            }],
            sha256: "00".repeat(32),
            size: 3,
        }
    }
    fn installation(package: Package) -> Installation {
        Installation {
            path: "/usr/bin/clipvault".into(),
            package: Some(package),
            arch: "x86_64".into(),
            platform: Platform {
                id: "fedora".into(),
                release: "44".into(),
            },
            repository_managed: false,
            detail: String::new(),
        }
    }
    fn manifest(a: Artifact) -> Manifest {
        Manifest {
            schema: 1,
            repository: "Amibiplob/ClipVault".into(),
            version: "2.2.0".into(),
            release_notes: "test".into(),
            artifacts: vec![a],
        }
    }
    #[test]
    fn downloads_report_actual_bytes_and_reject_interruption_overflow_and_cancellation() {
        use std::cell::Cell;
        let cancel = AtomicBool::new(false);
        let progress = Cell::new(0);
        let mut out = Vec::new();
        copy_download(
            &mut std::io::Cursor::new(b"abc"),
            &mut out,
            3,
            &cancel,
            &|n| progress.set(n),
        )
        .unwrap();
        assert_eq!(out, b"abc");
        assert_eq!(progress.get(), 3);
        assert!(copy_download(
            &mut std::io::Cursor::new(b"ab"),
            &mut Vec::new(),
            3,
            &cancel,
            &|_| {}
        )
        .unwrap_err()
        .contains("incomplete"));
        assert!(copy_download(
            &mut std::io::Cursor::new(b"abcd"),
            &mut Vec::new(),
            3,
            &cancel,
            &|_| {}
        )
        .is_err());
        struct Interrupted;
        impl Read for Interrupted {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::ConnectionReset.into())
            }
        }
        assert!(
            copy_download(&mut Interrupted, &mut Vec::new(), 3, &cancel, &|_| {})
                .unwrap_err()
                .contains("interrupted")
        );
        struct Full;
        impl Write for Full {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from_raw_os_error(libc::ENOSPC))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(copy_download(
            &mut std::io::Cursor::new(b"abc"),
            &mut Full,
            3,
            &cancel,
            &|_| {}
        )
        .unwrap_err()
        .contains("disk space"));
        cancel.store(true, Ordering::SeqCst);
        assert_eq!(
            copy_download(
                &mut std::io::Cursor::new(b"abc"),
                &mut Vec::new(),
                3,
                &cancel,
                &|_| {}
            )
            .unwrap_err(),
            "Cancelled"
        );
    }
    #[test]
    fn worker_disconnect_is_failure_never_installation_success() {
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let mut u = Updater {
            receiver: Some(rx),
            state: State::Installing,
            ..Default::default()
        };
        u.poll();
        assert!(!u.busy());
        assert!(matches!(u.state, State::Failed(_)));
    }
    #[test]
    fn second_attempt_cannot_replace_active_worker_or_plan() {
        let (_tx, rx) = mpsc::channel();
        let mut u = Updater {
            receiver: Some(rx),
            state: State::Verifying,
            ..Default::default()
        };
        u.prepare(
            "99.0.0".into(),
            "bad".into(),
            &eframe::egui::Context::default(),
        );
        u.install(&eframe::egui::Context::default());
        assert!(matches!(u.state, State::Verifying));
        assert!(u.plan.is_none());
        assert!(u.busy());
    }
    #[test]
    fn appimage_runtime_must_contain_running_executable_and_save_does_not_overwrite() {
        let t = Temp::new().unwrap();
        let p = t.0.join("image");
        let mut elf = [0; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[8..11].copy_from_slice(b"AI\x02");
        elf[5] = 1;
        elf[18] = 62;
        fs::write(&p, elf).unwrap();
        assert!(appimage_runtime(
            Path::new("/tmp/.mount-test/usr/bin/clipvault"),
            &p,
            Path::new("/tmp/.mount-test")
        ));
        assert!(!appimage_runtime(
            Path::new("/usr/bin/clipvault"),
            &p,
            Path::new("/tmp/.mount-test")
        ));
        assert!(!appimage_runtime(
            Path::new("/usr/bin/clipvault"),
            &p,
            Path::new("/")
        ));
        assert_eq!(is_appimage(&p).unwrap(), "x86_64");
        elf[18] = 183;
        fs::write(&p, elf).unwrap();
        assert_eq!(is_appimage(&p).unwrap(), "aarch64");
        let mut a = artifact(Package::Appimage);
        a.size = 64;
        a.sha256 = digest::digest(&digest::SHA256, &elf)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let dest = t.0.join("save with spaces.AppImage");
        save_appimage_copy(&p, &dest, &a).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), elf);
        assert!(save_appimage_copy(&p, &dest, &a).is_err());
        assert_eq!(fs::read(&dest).unwrap(), elf);
    }
    #[test]
    fn detection_requires_unique_running_owner() {
        assert_eq!(classify(true, false, false), Some(Package::Deb));
        assert_eq!(classify(false, true, false), Some(Package::Rpm));
        assert_eq!(classify(false, false, true), Some(Package::Appimage));
        for flags in [
            (true, true, false),
            (false, false, false),
            (true, false, true),
        ] {
            assert!(classify(flags.0, flags.1, flags.2).is_none());
        }
    }
    #[test]
    fn exact_package_arch_and_distribution_required() {
        for package in [Package::Deb, Package::Rpm, Package::Appimage] {
            let mut a = artifact(package.clone());
            let ext = match package {
                Package::Deb => "deb",
                Package::Rpm => "rpm",
                Package::Appimage => "AppImage",
            };
            a.filename = format!("clipvault.{ext}");
            a.url = format!("{RELEASE_ROOT}v2.2.0/{}", a.filename);
            let m = manifest(a);
            let mut i = installation(package);
            assert!(select(&m, &i).is_ok());
            i.arch = "aarch64".into();
            assert!(select(&m, &i).is_err());
            i.arch = "x86_64".into();
            i.package = None;
            assert!(select(&m, &i).is_err());
            if m.artifacts[0].package != Package::Appimage {
                i.package = Some(m.artifacts[0].package.clone());
                i.platform.release = "45".into();
                assert!(select(&m, &i).is_err());
                i.platform.id = "opensuse".into();
                assert!(select(&m, &i).is_err());
            }
        }
        let i = installation(Package::Deb);
        assert!(select(&manifest(artifact(Package::Rpm)), &i).is_err());
        let mut m = manifest(artifact(Package::Deb));
        m.artifacts.push(m.artifacts[0].clone());
        assert!(select(&m, &i).is_err());
        m.artifacts.clear();
        assert!(select(&m, &i).is_err());
    }
    #[test]
    fn signed_bytes_must_be_authentic_before_parsing() {
        let key = Ed25519KeyPair::from_pkcs8(
            Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .unwrap()
                .as_ref(),
        )
        .unwrap();
        let trusted = vec![key
            .public_key()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()];
        let bytes = serde_json::to_vec(&manifest(artifact(Package::Deb))).unwrap();
        let sig = key.sign(&bytes);
        assert!(verify_manifest(&bytes, sig.as_ref(), &trusted).is_ok());
        assert!(verify_manifest(&bytes, sig.as_ref(), &[]).is_err());
        assert!(verify_manifest(&bytes, &[0; 64], &trusted).is_err());
        let mut tampered = bytes.clone();
        tampered.push(b' ');
        assert!(verify_manifest(&tampered, sig.as_ref(), &trusted).is_err());
    }
    #[test]
    fn checksum_size_and_atomic_appimage_replacement() {
        let temp = Temp::new().unwrap();
        let source = temp.0.join("new");
        fs::write(&source, b"new").unwrap();
        let mut a = artifact(Package::Appimage);
        a.sha256 = digest::digest(&digest::SHA256, b"new")
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(verify_file(&source, &a).is_ok());
        let dest = temp.0.join("path with spaces.AppImage");
        fs::write(&dest, b"old").unwrap();
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o750)).unwrap();
        replace_appimage(&source, &dest, &a).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert_eq!(
            fs::metadata(&dest).unwrap().permissions().mode() & 0o777,
            0o750
        );
        fs::write(&source, b"bad").unwrap();
        assert!(replace_appimage(&source, &dest, &a).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        a.size = 4;
        assert!(verify_file(&dest, &a).is_err());
        assert!(
            replace_appimage(&source, Path::new("/proc/clipvault-readonly.AppImage"), &a).is_err()
        );
    }
    #[test]
    fn package_errors_cancel_space_and_source_policy() {
        assert!(package_failure(Some(126), "").contains("cancelled"));
        assert!(package_failure(Some(100), "Could not get lock").contains("busy"));
        assert!(package_failure(Some(1), "No space left").contains("disk space"));
        assert!(package_fields("other\n2.2.0-1\namd64", "2.2.0", "x86_64", false).is_err());
        assert!(package_fields("clipvault\n2.2.0-1\narm64", "2.2.0", "x86_64", false).is_err());
        assert!(package_fields("clipvault\n2.2.0-1\namd64", "2.2.0", "x86_64", false).is_ok());
        let cancel = AtomicBool::new(true);
        assert_eq!(check_cancel(&cancel).unwrap_err(), "Cancelled");
        assert!(!State::Installing.cancellable());
        assert!(State::Installing.critical());
        assert!(free_space(Path::new("/tmp"), u64::MAX).is_err());
        assert!(official_asset(&format!("{RELEASE_ROOT}v2.2.0/file.deb")));
        for url in [
            "http://github.com/Amibiplob/ClipVault/releases/download/v2/a.deb",
            "https://evil.test/file",
            "https://github.com/other/ClipVault/releases/download/v2/a.deb",
            "https://github.com/Amibiplob/ClipVault/releases/download/v2/..%2fa.deb",
        ] {
            assert!(!official_asset(url));
        }
        for url in [
            "http://release-assets.githubusercontent.com/github-production-release-asset/1",
            "https://evil.test/github-production-release-asset/1",
            "https://api.github.com/anything",
        ] {
            assert!(!redirect_allowed(&Url::parse(url).unwrap()));
        }
    }
}
