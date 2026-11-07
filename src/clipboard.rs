use crate::storage::{Snapshot, Store};
use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use x11rb::rust_connection::RustConnection;
use x11rb::{
    connection::Connection,
    protocol::{
        xfixes::{ConnectionExt as _, SelectionEventMask},
        xproto::*,
        Event,
    },
    wrapper::ConnectionExt as _,
};

pub enum Command {
    Restore(String, Snapshot),
    Shortcut(String),
    Stop,
}
#[derive(Debug)]
pub enum Notice {
    Changed,
    Open,
    Activated(String),
    ActivationFailed(String, String),
    ShortcutApplied(String),
    Status(String),
    Error(String),
}
pub struct Service {
    pub tx: Sender<Command>,
    pub rx: Receiver<Notice>,
}
pub type Shared = Arc<Mutex<Store>>;
unsafe impl Send for crate::storage::Database {} // All use is serialized by Store's mutex.
pub fn start(store: Shared) -> Service {
    let (tx, rx) = mpsc::channel();
    let (out, events) = mpsc::channel();
    std::thread::spawn(move || {
        let result = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            crate::wayland::run(store, rx, out.clone())
        } else {
            run_x11(store, rx, out.clone())
        };
        if let Err(e) = result {
            let _ = out.send(Notice::Error(e));
        }
    });
    Service { tx, rx: events }
}
pub fn compatible_text(mut data: Snapshot) -> Snapshot {
    if let Some(bytes) = data
        .get("text/plain;charset=utf-8")
        .or_else(|| data.get("text/plain;charset=UTF-8"))
        .or_else(|| data.get("text/plain"))
        .or_else(|| data.get("UTF8_STRING"))
        .cloned()
    {
        if std::str::from_utf8(&bytes).is_ok() {
            for mime in ["UTF8_STRING", "text/plain;charset=utf-8", "text/plain"] {
                data.entry(mime.into()).or_insert_with(|| bytes.clone());
            }
        }
    }
    data
}
pub fn sensitive_mime(m: &str) -> bool {
    m.to_ascii_lowercase().contains("password")
}
pub fn supported(m: &str) -> bool {
    m.starts_with("text/")
        || m.starts_with("image/")
        || [
            "UTF8_STRING",
            "STRING",
            "TEXT",
            "x-special/gnome-copied-files",
            "application/x-kde-cutselection",
            "application/rtf",
            "text/rtf",
            "x-kde-passwordManagerHint",
            "application/x-keepassxc-password",
        ]
        .contains(&m)
}
pub fn record(store: &Shared, data: Snapshot, source: &str, out: &Sender<Notice>) {
    let result = store.lock().unwrap().ingest(&data, source);
    match result {
        Ok(true) => {
            let _ = out.send(Notice::Changed);
        }
        Err(e) => {
            let _ = out.send(Notice::Error(e));
        }
        _ => {}
    }
}
struct Transfer {
    data: Vec<u8>,
    offset: usize,
    target: Atom,
    touched: Instant,
}
struct X11 {
    c: RustConnection,
    screen: usize,
    win: Window,
    clipboard: Atom,
    targets: Atom,
    prop: Atom,
    incr: Atom,
    owned: Snapshot,
    transfers: HashMap<(Window, Atom), Transfer>,
    key: Option<(u8, ModMask)>,
    out: Sender<Notice>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn parse_shortcut(shortcut: &str) -> Result<(u32, ModMask), String> {
    let parts: Vec<_> = shortcut.split('+').map(str::trim).collect();
    let key = parts.last().ok_or("Choose a shortcut")?;
    let ch = key
        .chars()
        .next()
        .ok_or("Choose a key")?
        .to_ascii_lowercase() as u32;
    if key.chars().count() != 1 || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Use a letter or number for the shortcut, such as Super+V".into());
    }
    let mut mods = ModMask::default();
    for p in &parts[..parts.len() - 1] {
        match p.to_lowercase().as_str() {
            "super" | "meta" => mods |= ModMask::M4,
            "ctrl" | "control" => mods |= ModMask::CONTROL,
            "alt" => mods |= ModMask::M1,
            "shift" => mods |= ModMask::SHIFT,
            _ => return Err(format!("Unknown shortcut modifier: {p}")),
        }
    }
    if mods & (ModMask::M4 | ModMask::CONTROL | ModMask::M1) == ModMask::default() {
        return Err("Include Super, Ctrl, or Alt in the shortcut".into());
    }
    Ok((ch, mods))
}
impl X11 {
    fn atom(&self, s: &str) -> Result<Atom, String> {
        Ok(self
            .c
            .intern_atom(false, s.as_bytes())
            .map_err(err)?
            .reply()
            .map_err(err)?
            .atom)
    }
    fn new(out: Sender<Notice>) -> Result<Self, String> {
        let (c, screen) = x11rb::connect(None)
            .map_err(|e| format!("Cannot connect to the X11 clipboard: {e}"))?;
        let win = c.generate_id().map_err(err)?;
        c.create_window(
            0,
            win,
            c.setup().roots[screen].root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(err)?;
        let atom = |s: &str| -> Result<Atom, String> {
            Ok(c.intern_atom(false, s.as_bytes())
                .map_err(err)?
                .reply()
                .map_err(err)?
                .atom)
        };
        let (clipboard, targets, prop, incr) = (
            atom("CLIPBOARD")?,
            atom("TARGETS")?,
            atom("CLIPVAULT_DATA")?,
            atom("INCR")?,
        );
        let _ = c.xfixes_query_version(5, 0).map_err(err)?.reply();
        c.xfixes_select_selection_input(
            win,
            clipboard,
            SelectionEventMask::SET_SELECTION_OWNER
                | SelectionEventMask::SELECTION_WINDOW_DESTROY
                | SelectionEventMask::SELECTION_CLIENT_CLOSE,
        )
        .map_err(err)?;
        c.flush().map_err(err)?;
        Ok(Self {
            c,
            screen,
            win,
            clipboard,
            targets,
            prop,
            incr,
            owned: Snapshot::new(),
            transfers: HashMap::new(),
            key: None,
            out,
        })
    }
    fn bind(&mut self, shortcut: &str) -> Result<(), String> {
        let (ch, mods) = parse_shortcut(shortcut)?;
        let setup = self.c.setup();
        let mapping = self
            .c
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let code = mapping
            .keysyms
            .chunks(mapping.keysyms_per_keycode as usize)
            .position(|v| v.contains(&ch))
            .map(|i| setup.min_keycode + i as u8)
            .ok_or("Shortcut key is not on this keyboard")?;
        if self.key == Some((code, mods)) {
            return Ok(());
        }
        // Keep the previous shortcut working until every new grab succeeds.
        let extras = [
            ModMask::default(),
            ModMask::LOCK,
            ModMask::M2,
            ModMask::LOCK | ModMask::M2,
        ];
        let root = setup.roots[self.screen].root;
        let mut grabbed = Vec::new();
        for extra in extras {
            let mask = mods | extra;
            let result = self
                .c
                .grab_key(false, root, mask, code, GrabMode::ASYNC, GrabMode::ASYNC)
                .map_err(err)
                .and_then(|cookie| cookie.check().map_err(err));
            if let Err(e) = result {
                for mask in grabbed {
                    let _ = self.c.ungrab_key(code, root, mask);
                }
                let _ = self.c.flush();
                return Err(format!("{shortcut} is already used by your desktop. Your previous shortcut is still active. Choose another in Settings. ({e})"));
            }
            grabbed.push(mask);
        }
        if let Some((old_code, old_mods)) = self.key {
            for extra in extras {
                let _ = self.c.ungrab_key(old_code, root, old_mods | extra);
            }
        }
        self.key = Some((code, mods));
        self.c.flush().map_err(err)?;
        Ok(())
    }
    fn serve(&mut self, e: Event) {
        match e {
            Event::SelectionRequest(r) => {
                let prop = if r.property == 0 {
                    r.target
                } else {
                    r.property
                };
                let mut success = false;
                if r.target == self.targets {
                    let mut atoms = vec![self.targets];
                    for k in self.owned.keys() {
                        if let Ok(a) = self.atom(k) {
                            atoms.push(a);
                        }
                    }
                    success = self
                        .c
                        .change_property32(
                            PropMode::REPLACE,
                            r.requestor,
                            prop,
                            AtomEnum::ATOM,
                            &atoms,
                        )
                        .is_ok();
                } else {
                    let name = self
                        .c
                        .get_atom_name(r.target)
                        .ok()
                        .and_then(|v| v.reply().ok())
                        .map(|v| String::from_utf8_lossy(&v.name).into_owned())
                        .unwrap_or_default();
                    if let Some(data) = self.owned.get(&name) {
                        if data.len() < 60000 {
                            success = self
                                .c
                                .change_property8(
                                    PropMode::REPLACE,
                                    r.requestor,
                                    prop,
                                    r.target,
                                    data,
                                )
                                .is_ok();
                        } else {
                            let _ = self.c.change_window_attributes(
                                r.requestor,
                                &ChangeWindowAttributesAux::new()
                                    .event_mask(EventMask::PROPERTY_CHANGE),
                            );
                            success = self
                                .c
                                .change_property32(
                                    PropMode::REPLACE,
                                    r.requestor,
                                    prop,
                                    self.incr,
                                    &[data.len() as u32],
                                )
                                .is_ok();
                            self.transfers.insert(
                                (r.requestor, prop),
                                Transfer {
                                    data: data.clone(),
                                    offset: 0,
                                    target: r.target,
                                    touched: Instant::now(),
                                },
                            );
                        }
                    }
                }
                let n = SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: r.time,
                    requestor: r.requestor,
                    selection: r.selection,
                    target: r.target,
                    property: if success { prop } else { 0 },
                };
                let _ = self
                    .c
                    .send_event(false, r.requestor, EventMask::NO_EVENT, n);
                let _ = self.c.flush();
            }
            Event::PropertyNotify(r) if r.state == Property::DELETE => {
                let key = (r.window, r.atom);
                let mut done = false;
                if let Some(t) = self.transfers.get_mut(&key) {
                    let end = (t.offset + 60000).min(t.data.len());
                    let _ = self.c.change_property8(
                        PropMode::REPLACE,
                        r.window,
                        r.atom,
                        t.target,
                        &t.data[t.offset..end],
                    );
                    done = t.offset == t.data.len();
                    t.offset = end;
                    t.touched = Instant::now();
                }
                if done {
                    self.transfers.remove(&key);
                }
                let _ = self.c.flush();
            }
            Event::KeyPress(_) => {
                let _ = self.out.send(Notice::Open);
            }
            _ => {}
        }
    }
    fn fetch(&mut self, target: Atom, cap: usize) -> Result<Vec<u8>, String> {
        self.c.delete_property(self.win, self.prop).map_err(err)?;
        self.c
            .convert_selection(
                self.win,
                self.clipboard,
                target,
                self.prop,
                x11rb::CURRENT_TIME,
            )
            .map_err(err)?;
        self.c.flush().map_err(err)?;
        let deadline = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < deadline {
            if let Some(e) = self.c.poll_for_event().map_err(err)? {
                match e {
                    Event::SelectionNotify(n)
                        if n.selection == self.clipboard && n.target == target =>
                    {
                        if n.property == 0 {
                            return Err("Clipboard format unavailable".into());
                        }
                        let p = self
                            .c
                            .get_property(
                                false,
                                self.win,
                                self.prop,
                                AtomEnum::ANY,
                                0,
                                (cap / 4 + 1) as u32,
                            )
                            .map_err(err)?
                            .reply()
                            .map_err(err)?;
                        if p.type_ == self.incr {
                            self.c.delete_property(self.win, self.prop).map_err(err)?;
                            self.c.flush().map_err(err)?;
                            return self.fetch_incremental(cap);
                        }
                        let _ = self.c.delete_property(self.win, self.prop);
                        if p.bytes_after > 0 || p.value.len() > cap {
                            return Err("Clipboard item exceeds the configured size limit".into());
                        }
                        return Ok(p.value);
                    }
                    other => self.serve(other),
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Err("Clipboard owner did not respond".into())
    }
    fn fetch_incremental(&mut self, cap: usize) -> Result<Vec<u8>, String> {
        let mut data = Vec::new();
        let mut deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Some(e) = self.c.poll_for_event().map_err(err)? {
                match e {
                    Event::PropertyNotify(n)
                        if n.window == self.win
                            && n.atom == self.prop
                            && n.state == Property::NEW_VALUE =>
                    {
                        let p = self
                            .c
                            .get_property(
                                true,
                                self.win,
                                self.prop,
                                AtomEnum::ANY,
                                0,
                                (cap / 4 + 1) as u32,
                            )
                            .map_err(err)?
                            .reply()
                            .map_err(err)?;
                        self.c.flush().map_err(err)?;
                        if p.value.is_empty() {
                            return Ok(data);
                        }
                        if data.len() + p.value.len() > cap || p.bytes_after > 0 {
                            return Err("Clipboard item exceeds size limit".into());
                        }
                        data.extend(p.value);
                        deadline = Instant::now() + Duration::from_secs(3);
                    }
                    other => self.serve(other),
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Err("Clipboard transfer timed out".into())
    }
    fn capture(&mut self, cap: usize) -> Result<Snapshot, String> {
        let raw = self.fetch(self.targets, 256 * 1024)?;
        let mut data = Snapshot::new();
        let mut targets = Vec::new();
        for a in raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| u32::from_ne_bytes(*v))
        {
            let name = self.c.get_atom_name(a).map_err(err)?.reply().map_err(err)?;
            let name = String::from_utf8_lossy(&name.name).into_owned();
            if sensitive_mime(&name) {
                return Ok(data);
            }
            targets.push((a, name));
        }
        for (a, name) in targets {
            if supported(&name) {
                match self.fetch(a, cap.saturating_sub(data.values().map(Vec::len).sum())) {
                    Ok(b) if !b.is_empty() => {
                        data.insert(name, b);
                    }
                    Err(e) if e.contains("size limit") => return Err(e),
                    _ => {}
                }
            }
            if data.len() >= 24 {
                break;
            }
        }
        if data.is_empty() {
            return Err(
                "This clipboard content has no supported text, image, or file format".into(),
            );
        }
        Ok(data)
    }
    fn source(&self) -> String {
        let root = self.c.setup().roots[self.screen].root;
        let active = self.atom("_NET_ACTIVE_WINDOW").ok().and_then(|a| {
            self.c
                .get_property(false, root, a, AtomEnum::WINDOW, 0, 1)
                .ok()?
                .reply()
                .ok()?
                .value32()?
                .next()
        });
        if let Some(w) = active {
            if let Ok(r) =
                self.c
                    .get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
            {
                if let Ok(p) = r.reply() {
                    return String::from_utf8_lossy(&p.value).replace('\0', " ");
                }
            }
        }
        String::new()
    }
    fn restore(&mut self, data: Snapshot) -> Result<(), String> {
        if data.is_empty() {
            return Err("The saved clip has no readable clipboard formats".into());
        }
        self.owned = compatible_text(data);
        self.c
            .set_selection_owner(self.win, self.clipboard, x11rb::CURRENT_TIME)
            .map_err(err)?
            .check()
            .map_err(err)?;
        self.c.flush().map_err(err)?;
        if self
            .c
            .get_selection_owner(self.clipboard)
            .map_err(err)?
            .reply()
            .map_err(err)?
            .owner
            != self.win
        {
            return Err("Could not take clipboard ownership".into());
        }
        Ok(())
    }
}
pub fn run_x11(store: Shared, rx: Receiver<Command>, out: Sender<Notice>) -> Result<(), String> {
    let mut x = X11::new(out.clone())?;
    let shortcut = store.lock().unwrap().settings.shortcut.clone();
    let _ = out.send(Notice::Status("X11 · monitoring clipboard".into()));
    if let Err(e) = x.bind(&shortcut) {
        let _ = out.send(Notice::Error(e));
    }
    let mut dirty = true;
    let mut last_poll = Instant::now();
    let mut last_owner = 0;
    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Command::Stop => return Ok(()),
                Command::Restore(id, data) => {
                    match x.restore(data) {
                        Ok(()) => {
                            if let Err(e) = store.lock().unwrap().touch(&id) {
                                let _ = out.send(Notice::ActivationFailed(
                                    id,
                                    format!(
                                        "Clipboard is ready, but recency could not be saved: {e}"
                                    ),
                                ));
                            } else {
                                let _ = out.send(Notice::Activated(id));
                            }
                        }
                        Err(e) => {
                            let _ = out.send(Notice::ActivationFailed(id, format!("Could not copy this clip: {e}. Try again; the window will stay open.")));
                        }
                    }
                    dirty = false;
                }
                Command::Shortcut(s) => {
                    let notice = match x.bind(&s) {
                        Ok(()) => Notice::ShortcutApplied(s),
                        Err(e) => Notice::Error(e),
                    };
                    let _ = out.send(notice);
                }
            }
        }
        while let Some(e) = x.c.poll_for_event().map_err(err)? {
            if matches!(&e,Event::XfixesSelectionNotify(n) if n.selection==x.clipboard&&n.owner!=x.win&&n.owner!=0)
            {
                dirty = true;
            }
            x.serve(e);
        }
        if last_poll.elapsed() > Duration::from_millis(250) {
            last_poll = Instant::now();
            let owner =
                x.c.get_selection_owner(x.clipboard)
                    .map_err(err)?
                    .reply()
                    .map_err(err)?
                    .owner;
            if owner != last_owner {
                last_owner = owner;
                if owner != x.win && owner != 0 {
                    dirty = true;
                }
            }
            if owner == 0 || owner == x.win {
                dirty = false;
            }
            if dirty {
                dirty = false;
                let settings = store.lock().unwrap().settings.clone();
                if !settings.paused {
                    let source = x.source();
                    if !settings.exclude_apps.lines().any(|v| {
                        !v.trim().is_empty()
                            && source.to_lowercase().contains(&v.trim().to_lowercase())
                    }) {
                        match x.capture(settings.max_mb * 1024 * 1024) {
                            Ok(data) => {
                                if x.c
                                    .get_selection_owner(x.clipboard)
                                    .map_err(err)?
                                    .reply()
                                    .map_err(err)?
                                    .owner
                                    == owner
                                {
                                    record(&store, data, &source, &out);
                                }
                            }
                            Err(e) => {
                                let _ = out.send(Notice::Error(e));
                            }
                        }
                    }
                }
            }
            x.transfers
                .retain(|_, t| t.touched.elapsed() < Duration::from_secs(20));
        }
        std::thread::sleep(Duration::from_millis(12));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcuts_require_a_primary_modifier_and_single_key() {
        assert_eq!(
            parse_shortcut(" Ctrl + Alt + V ").unwrap(),
            (b'v' as u32, ModMask::CONTROL | ModMask::M1)
        );
        assert_eq!(
            parse_shortcut("Super+Shift+V").unwrap(),
            (b'v' as u32, ModMask::M4 | ModMask::SHIFT)
        );
        for invalid in ["V", "Shift+V", "Super+", "Super+Enter", "Unknown+V", ""] {
            assert!(parse_shortcut(invalid).is_err(), "{invalid}");
        }
    }
}

#[cfg(test)]
mod desktop_tests {
    use super::*;
    #[test]
    fn text_aliases_preserve_original_formats() {
        let original = Snapshot::from([
            ("text/plain".into(), "বাংলা text".as_bytes().to_vec()),
            ("text/html".into(), b"<b>text</b>".to_vec()),
        ]);
        let restored = compatible_text(original.clone());
        for (mime, bytes) in original {
            assert_eq!(restored.get(&mime), Some(&bytes));
        }
        assert_eq!(restored["UTF8_STRING"], "বাংলা text".as_bytes());
    }
    #[test]
    #[ignore = "Requires a real X11 desktop; changes its system clipboard"]
    fn x11_text_and_image_activation_roundtrip() {
        let root = std::env::temp_dir().join(format!("cv-x11-{}", std::process::id()));
        let mut store = Store::open(root.clone()).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(23, 17)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let snapshots = [
            Snapshot::from([(
                "text/plain".into(),
                b"ClipVault clipboard round trip".to_vec(),
            )]),
            Snapshot::from([("image/png".into(), png.into_inner())]),
        ];
        for data in &snapshots {
            store.ingest(data, "test").unwrap();
        }
        let store = Arc::new(Mutex::new(store));
        let (tx, rx) = mpsc::channel();
        let (out, events) = mpsc::channel();
        let shared = store.clone();
        let handle = std::thread::spawn(move || run_x11(shared, rx, out));
        for data in snapshots {
            let id = {
                let s = store.lock().unwrap();
                s.items
                    .iter()
                    .find(|i| s.snapshot(i).as_ref() == Ok(&data))
                    .unwrap()
                    .id
                    .clone()
            };
            tx.send(Command::Restore(id, data.clone())).unwrap();
            let end = Instant::now() + Duration::from_secs(5);
            loop {
                assert!(
                    Instant::now() < end,
                    "clipboard activation not acknowledged"
                );
                if let Ok(Notice::Activated(_)) = events.recv_timeout(Duration::from_millis(100)) {
                    break;
                }
            }
            let mut client = X11::new(mpsc::channel().0).unwrap();
            let mime = if data.contains_key("image/png") {
                "image/png"
            } else {
                "UTF8_STRING"
            };
            let atom = client.atom(mime).unwrap();
            let bytes = client.fetch(atom, 1024 * 1024).unwrap();
            assert_eq!(bytes, *data.values().next().unwrap());
            assert_eq!(store.lock().unwrap().items.len(), 2);
        }
        tx.send(Command::Stop).unwrap();
        handle.join().unwrap().unwrap();
        drop(store);
        let store = Store::open(root.clone()).unwrap();
        assert_eq!(store.items.len(), 2);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
