//! Installed-file replacement detection, independent of network update preferences.
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
static REQUESTED: AtomicBool = AtomicBool::new(false);
pub fn request() {
    REQUESTED.store(true, Ordering::SeqCst);
}
pub fn requested() -> bool {
    REQUESTED.load(Ordering::SeqCst)
}
fn probe(path: &std::path::Path, timeout: Duration) -> Result<semver::Version, String> {
    let mut child = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err("Version probe failed".into());
                }
                let mut text = String::new();
                if let Some(output) = child.stdout.take() {
                    output
                        .take(256)
                        .read_to_string(&mut text)
                        .map_err(|e| e.to_string())?;
                }
                return text
                    .trim()
                    .strip_prefix("ClipVault ")
                    .and_then(|s| semver::Version::parse(s).ok())
                    .ok_or_else(|| {
                        "Installed executable did not report a supported version".into()
                    });
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
            _ if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Installed executable did not respond to its version probe".into());
            }
            _ => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}
pub fn watch() -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    if let Ok(path) = crate::desktop::executable() {
        std::thread::spawn(move || {
            use std::os::unix::fs::MetadataExt;
            let fingerprint = |p: &PathBuf| {
                std::fs::metadata(p)
                    .ok()
                    .map(|m| (m.dev(), m.ino(), m.len(), m.mtime()))
            };
            let mut initial = fingerprint(&path);
            let mut stable = None;
            loop {
                std::thread::sleep(Duration::from_secs(30));
                if tx.send(String::new()).is_err() {
                    break;
                }
                let current = fingerprint(&path);
                if current.is_none() || current == initial {
                    stable = None;
                    continue;
                }
                if stable != current {
                    stable = current;
                    continue;
                }
                // Probe without opening storage, binding keys or creating another instance.
                match probe(&path, Duration::from_secs(5)) {
                    Ok(version)
                        if version > semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap() =>
                    {
                        let _ = tx.send(version.to_string());
                        break;
                    }
                    Ok(_) => {
                        initial = current;
                        stable = None;
                    }
                    Err(error) => {
                        let _=tx.send(format!("manual:An installed executable changed, but automatic restart is unavailable: {error}. Restart ClipVault manually when safe."));
                        initial = current;
                        stable = None;
                    }
                }
            }
        });
    }
    rx
}
pub fn exec_new() -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let exe = crate::desktop::executable()?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let error = std::process::Command::new(exe).args(args).exec();
    Err(format!(
        "Automatic restart failed: {error}. Open ClipVault manually; saved history is preserved."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_probe_is_bounded_and_requires_valid_version() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("cv-probe-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("candidate");
        let write = |body: &str| {
            std::fs::write(&script, format!("#!/usr/bin/python3\n{body}\n")).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        };
        write("print('ClipVault 9.1.0')");
        assert_eq!(
            probe(&script, Duration::from_secs(2)).unwrap(),
            semver::Version::new(9, 1, 0)
        );
        write("print('unknown')");
        assert!(probe(&script, Duration::from_secs(2)).is_err());
        write("import time; time.sleep(10)");
        let start = Instant::now();
        assert!(probe(&script, Duration::from_millis(100)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        std::fs::remove_dir_all(root).unwrap();
    }
}
