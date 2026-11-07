mod clipboard;
mod icons;
mod storage;
mod ui;
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
    if args.iter().any(|a| a == "--help") {
        println!("ClipVault — private native clipboard history\n\n  --toggle              Toggle the running floating window\n  --background          Start with the window hidden\n  --screenshot FILE.png Draw 120 frames, save the window, and exit\n  --data-dir DIRECTORY  Use a separate local history directory\n  --diagnostics         Print desktop capability information\n\nUse Ctrl+V in your target app after selecting a history item.\nWayland: assign clipvault --toggle to Super+V in desktop shortcuts.");
        return Ok(());
    }
    if args.iter().any(|a| a == "--diagnostics") {
        println!("Session: {}\nDisplay: {}\nWayland: {}\nClipboard: native X11 / Wayland ext or wlr data-control\nWayland shortcuts: compositor binding to clipvault --toggle\nGNOME Wayland: requires a data-control extension or X11 login",std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),std::env::var("DISPLAY").unwrap_or_default(),std::env::var("WAYLAND_DISPLAY").unwrap_or_default());
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
    let (ipt, ipr) = mpsc::channel();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(100)));
            let mut data = [0; 32];
            if let Ok(n) = stream.read(&mut data) {
                let _ = ipt.send(String::from_utf8_lossy(&data[..n]).into_owned());
            }
        }
    });
    let store = Arc::new(Mutex::new(storage::Store::open(root)?));
    let service = clipboard::start(store.clone());
    let hidden = args.iter().any(|a| a == "--background") && shot.is_none();
    let settings = store.lock().unwrap().settings.clone();
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
        .with_inner_size(settings.window_size)
        .with_min_inner_size([440., 420.])
        .with_clamp_size_to_monitor_size(true)
        .with_decorations(true)
        .with_resizable(true)
        .with_visible(!hidden);
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
                cc, store, service, ipr, shot, hidden,
            )))
        }),
    )
    .map_err(|e| e.to_string());
    let _ = fs::remove_file(socket);
    drop(lock);
    r
}
