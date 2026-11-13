mod clipboard;
mod desktop;
mod fonts;
mod icons;
mod paste;
mod restart;
mod shortcut;
mod storage;
mod ui;
#[cfg(test)]
mod ui_capture;
mod updater;
mod updates;
mod wayland;
use eframe::egui;
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};
fn main() {
    if let Err(e) = launch() {
        eprintln!("ClipVault: {e}");
        std::process::exit(1)
    }
}
fn launch() -> Result<(), String> {
    unsafe {
        libc::umask(0o077);
    }
    let args: Vec<String> = std::env::args().collect();
    let option = |key: &str| {
        args.iter()
            .position(|s| s == key)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    if args.iter().any(|a| a == "--version") {
        println!("ClipVault {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help") {
        println!("ClipVault — private native clipboard history\n\n  --toggle              Toggle the running floating window\n  --background          Start with the window hidden\n  --autostart           Login launch; honor Start hidden\n  --screenshot FILE.png Draw 120 frames, save the window, and exit\n  --data-dir DIRECTORY  Use a separate local history directory\n  --diagnostics         Print desktop capability information\n\nSelect a history item to attempt paste. If unavailable, press Ctrl+V in your target app.\nDesktop shortcut setup and status are in Settings → Shortcuts.");
        return Ok(());
    }
    if args.iter().any(|a| a == "--diagnostics") {
        println!("Session: {}\nDisplay: {}\nWayland: {}\nClipboard: native X11 / Wayland ext or wlr data-control\nWayland shortcuts: desktop integration where available; otherwise compositor binding to clipvault --toggle\nGNOME Wayland: requires a data-control extension or X11 login",std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),std::env::var("DISPLAY").unwrap_or_default(),std::env::var("WAYLAND_DISPLAY").unwrap_or_default());
        return Ok(());
    }
    let shot = option("--screenshot");
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    let root = option("--data-dir").unwrap_or_else(|| {
        if let Some(p) = &shot {
            p.parent()
                .unwrap_or(std::path::Path::new("."))
                .join(".capture-history")
        } else {
            base.join("clipvault")
        }
    });
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    // Linux resolves this short path to the same private directory, even when a
    // capture workspace exceeds sockaddr_un's 108-byte pathname limit.
    let root_handle = fs::File::open(&root).map_err(|e| e.to_string())?;
    let socket = PathBuf::from(format!(
        "/proc/self/fd/{}/instance.sock",
        root_handle.as_raw_fd()
    ));
    // Serialize the brief socket-creation phase, so a simultaneous second
    // launch cannot observe an instance lock before its IPC socket is bound.
    let startup_guard = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("startup.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(startup_guard.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err("Could not lock instance startup".into());
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("instance.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        if shot.is_some() {
            return Err("This capture data directory is already in use".into());
        }
        if args
            .iter()
            .any(|a| a == "--autostart" || a == "--background")
        {
            return Ok(());
        }
        let mut stream = UnixStream::connect(&socket)
            .map_err(|e| format!("Could not reach the running instance: {e}"))?;
        stream
            .write_all(if args.iter().any(|a| a == "--toggle") {
                b"toggle"
            } else {
                b"show"
            })
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
    drop(startup_guard);
    let (ipt, ipr) = mpsc::channel();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));
            let mut data = Vec::new();
            if (&mut stream).take(32).read_to_end(&mut data).is_ok()
                && matches!(data.as_slice(), b"toggle" | b"show")
            {
                let _ = ipt.send(String::from_utf8_lossy(&data).into_owned());
            }
        }
    });
    let store = Arc::new(Mutex::new(storage::Store::open(root)?));
    let service = if shot.is_some() {
        // Rendering screenshots must never register shortcuts or inspect the real clipboard.
        let (tx, commands) = mpsc::channel();
        let (out, rx) = mpsc::channel();
        let _ = out.send(clipboard::Notice::Status(
            "Screenshot mode · monitoring disabled".into(),
        ));
        std::thread::spawn(move || {
            while let Ok(command) = commands.recv() {
                if matches!(command, clipboard::Command::Stop) {
                    break;
                }
            }
        });
        clipboard::Service { tx, rx }
    } else {
        clipboard::start(store.clone())
    };
    let _ = service.tx.send(clipboard::Command::RememberTarget);
    let settings = store.lock().unwrap().settings.clone();
    let toggle_launch = args.iter().any(|a| a == "--toggle");
    if toggle_launch {
        let _ = service.tx.send(clipboard::Command::Toggle);
    }
    let hidden = (startup_hidden(&args, &settings) || toggle_launch) && shot.is_none();
    // eframe 0.31 unconditionally maps the first rendered root window. Do not
    // initialize it at all during silent startup: monitoring and IPC run alone.
    let service = if hidden {
        match wait_for_window(service, &ipr) {
            Ok(service) => service,
            Err(_) if restart::requested() => {
                let _ = fs::remove_file(&socket);
                drop(lock);
                return restart::exec_new();
            }
            Err(error) => return Err(error),
        }
    } else {
        service
    };
    let image = image::load_from_memory(include_bytes!("../assets/icons/256x256/clipvault.png"))
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ClipVault")
        .with_app_id("io.clipvault.desktop")
        .with_icon(egui::IconData {
            rgba: image.into_raw(),
            width: 256,
            height: 256,
        })
        .with_inner_size(settings.geometry().size)
        .with_maximized(settings.geometry().maximized)
        .with_min_inner_size([if settings.compact { 440. } else { 640. }, 420.])
        .with_clamp_size_to_monitor_size(true)
        .with_decorations(settings.system_title_bar)
        .with_resizable(true)
        .with_visible(true);
    if settings.always_on_top {
        viewport = viewport.with_always_on_top();
    }
    if let Some([x, y]) = settings.window_position {
        viewport = viewport.with_position([x.max(0.), y.max(0.)]);
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: settings.window_position.is_none(),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let r = eframe::run_native(
        "ClipVault",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::VaultApp::new(
                cc, store, service, ipr, shot, false,
            )))
        }),
    )
    .map_err(|e| e.to_string());
    let _ = fs::remove_file(socket);
    drop(lock);
    if restart::requested() && r.is_ok() {
        return restart::exec_new();
    }
    r
}

fn wait_for_window(
    service: clipboard::Service,
    ipc: &mpsc::Receiver<String>,
) -> Result<clipboard::Service, String> {
    let updates = restart::watch();
    let mut pending = std::collections::VecDeque::new();
    let mut service_closed = false;
    loop {
        let mut newer = false;
        for version in updates.try_iter() {
            if let Some(message) = version.strip_prefix("manual:") {
                pending.push_back(clipboard::Notice::Error(message.into()));
            } else if !version.is_empty() {
                newer = true;
            }
        }
        if newer {
            let (tx, rx) = mpsc::channel();
            if service.tx.send(clipboard::Command::Flush(tx)).is_ok()
                && matches!(
                    rx.recv_timeout(std::time::Duration::from_secs(3)),
                    Ok(Ok(()))
                )
            {
                let _ = service.tx.send(clipboard::Command::Stop);
                restart::request();
                return Err("Restarting into installed update".into());
            }
            pending.push_back(clipboard::Notice::Error(
                "An update is installed; restart manually after resolving the storage error."
                    .into(),
            ));
        }
        match ipc.recv_timeout(std::time::Duration::from_millis(40)) {
            Ok(message) if message == "toggle" => {
                let _ = service.tx.send(clipboard::Command::Toggle);
            }
            Ok(message) if message == "show" => {
                let _ = service.tx.send(clipboard::Command::RememberTarget);
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) if service_closed => {
                return Err("Background services stopped".into())
            }
            _ => {}
        }
        loop {
            match service.rx.try_recv() {
                Ok(clipboard::Notice::Open) => return forward_pending(service, pending),
                Ok(clipboard::Notice::Changed) => {} // The store is authoritative when opened.
                Ok(notice) => {
                    if pending.len() == 64 {
                        pending.pop_front();
                    }
                    pending.push_back(notice);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    service_closed = true;
                    break;
                }
            }
        }
    }
    forward_pending(service, pending)
}
fn forward_pending(
    service: clipboard::Service,
    pending: std::collections::VecDeque<clipboard::Notice>,
) -> Result<clipboard::Service, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for notice in pending.into_iter().chain(service.rx) {
            if tx.send(notice).is_err() {
                break;
            }
        }
    });
    Ok(clipboard::Service { tx: service.tx, rx })
}
fn startup_hidden(args: &[String], settings: &storage::Settings) -> bool {
    args.iter().any(|a| a == "--background")
        || (args.iter().any(|a| a == "--autostart") && settings.startup && settings.start_hidden)
}
#[cfg(test)]
mod startup_tests {
    #[test]
    fn only_login_launch_honors_start_hidden() {
        for startup in [false, true] {
            for hidden in [false, true] {
                let settings = crate::storage::Settings {
                    startup,
                    start_hidden: hidden,
                    ..Default::default()
                };
                assert!(!super::startup_hidden(&[], &settings));
                assert_eq!(
                    super::startup_hidden(&["--autostart".into()], &settings),
                    startup && hidden
                );
                assert!(super::startup_hidden(&["--background".into()], &settings));
            }
        }
    }
    #[test]
    fn silent_monitor_waits_for_shortcut_or_manual_launch_without_creating_ui() {
        use crate::clipboard::{Notice, Service};
        use std::sync::mpsc;
        for shortcut in [true, false] {
            let (command, _commands) = mpsc::channel();
            let (notice, notices) = mpsc::channel();
            let (ipc, requests) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                super::wait_for_window(
                    Service {
                        tx: command,
                        rx: notices,
                    },
                    &requests,
                )
                .unwrap()
            });
            notice.send(Notice::Status("Monitoring".into())).unwrap();
            assert!(!worker.is_finished());
            if shortcut {
                notice.send(Notice::Open).unwrap();
            } else {
                ipc.send("show".into()).unwrap();
            }
            let service = worker.join().unwrap();
            assert!(matches!(
                service
                    .rx
                    .recv_timeout(std::time::Duration::from_secs(1))
                    .unwrap(),
                Notice::Status(_)
            ));
        }
    }
}
