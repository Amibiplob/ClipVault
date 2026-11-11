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
    RestoreItem(String),
    Toggle,
    RememberTarget,
    Paste,
    RecordShortcut(bool),
    SetShortcut(String),
    Flush(Sender<Result<(), String>>),
    Stop,
}
#[derive(Debug)]
pub enum Notice {
    Changed,
    PasteFeedback(String),
    ShortcutConfigured(String),
    Open,
    Activated(String),
    ActivationFailed(String, String),
    ShortcutApplied(String),
    ShortcutActive(String),
    ShortcutInactive,
    ShortcutFailed(String),
    RecordingReady,
    ShortcutCandidate(String),
    RecordingCancelled,
    Status(String),
    Error(String),
    Unavailable(String),
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
            run_wayland_service(store, rx, out.clone())
        } else {
            run_x11(store, rx, out.clone())
        };
        if let Err(e) = result {
            let _ = out.send(Notice::Unavailable(e));
        }
    });
    Service { tx, rx: events }
}
fn run_wayland_service(
    store: Shared,
    commands: Receiver<Command>,
    out: Sender<Notice>,
) -> Result<(), String> {
    let desktop = crate::desktop::Integration::new(store.lock().unwrap().root.clone());
    let shortcut = store.lock().unwrap().settings.shortcut.clone();
    match desktop.register(&shortcut) {
        Ok(()) => {
            let _ = out.send(Notice::ShortcutConfigured(shortcut));
        }
        Err(e) => {
            let _ = out.send(Notice::ShortcutFailed(e));
        }
    }
    let (tx, rx) = mpsc::channel();
    let capture_store = store.clone();
    let capture_out = out.clone();
    std::thread::spawn(move || {
        if let Err(e) = crate::wayland::run(capture_store, rx, capture_out.clone()) {
            let _ = capture_out.send(Notice::Error(e));
        }
    });
    let mut recording = false;
    let desktop_events = desktop.watch();
    loop {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Flush(reply)) => {
                let _ = reply.send(store.lock().unwrap().flush());
            }
            Ok(Command::Stop) => {
                let _ = tx.send(Command::Stop);
                return Ok(());
            }
            Ok(Command::Toggle) if !recording => {
                let _ = out.send(Notice::Open);
            }
            Ok(Command::Toggle | Command::RememberTarget) => {}
            Ok(Command::Paste) => {
                let _ = out.send(Notice::PasteFeedback(
                    "Copied—press Ctrl+V to paste.".into(),
                ));
            }
            Ok(Command::RecordShortcut(start)) => {
                if start {
                    match desktop.suspend() {
                        Ok(()) => {
                            recording = true;
                            let _ = out.send(Notice::RecordingReady);
                        }
                        Err(e) => {
                            let _ = out.send(Notice::ShortcutFailed(e));
                        }
                    }
                } else {
                    recording = false;
                    let old = store.lock().unwrap().settings.shortcut.clone();
                    match desktop.register(&old) {
                        Ok(()) => {
                            let _ = out.send(Notice::ShortcutConfigured(old));
                            let _ = out.send(Notice::RecordingCancelled);
                        }
                        Err(e) => {
                            let _ = out.send(Notice::ShortcutFailed(e));
                        }
                    }
                }
            }
            Ok(Command::SetShortcut(next)) => {
                recording = false;
                let old = store.lock().unwrap().settings.shortcut.clone();
                match change_binding(
                    &old,
                    &next,
                    |s| desktop.register(s),
                    |next| {
                        let mut s = store.lock().unwrap();
                        s.settings.shortcut = next.into();
                        let result = s.save_settings();
                        if result.is_err() {
                            s.settings.shortcut = old.clone();
                        }
                        result
                    },
                ) {
                    Ok(()) => {
                        let _ = out.send(Notice::ShortcutConfigured(next));
                    }
                    Err((e, restored)) => {
                        if restored {
                            let _ = out.send(Notice::ShortcutConfigured(old));
                        }
                        let _ = out.send(Notice::ShortcutFailed(e));
                    }
                }
            }
            Ok(command @ (Command::Restore(..) | Command::RestoreItem(..))) => {
                if let Err(error) = tx.send(command) {
                    if let Command::Restore(id, _) | Command::RestoreItem(id) = error.0 {
                        let _=out.send(Notice::ActivationFailed(id,"Clipboard unavailable on this compositor. See Privacy and Shortcuts for supported capabilities.".into()));
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !recording && desktop_events.try_iter().next().is_some() {
            if let Ok(Some(next)) = desktop.read() {
                let mut s = store.lock().unwrap();
                if s.settings.shortcut != next {
                    s.settings.shortcut = next.clone();
                    if let Err(e) = s.save_settings() {
                        let _ = out.send(Notice::Error(e));
                    }
                }
                let _ = out.send(Notice::ShortcutConfigured(next));
            } else {
                let _ = out.send(Notice::ShortcutInactive);
            }
        }
    }
}
/// Resolve disk payloads on the clipboard worker, never on the UI thread.
pub fn resolve_restore(command: Command, store: &Shared, out: &Sender<Notice>) -> Option<Command> {
    if let Command::RestoreItem(id) = command {
        let data = {
            let store = store.lock().unwrap();
            store
                .items
                .iter()
                .find(|item| item.id == id)
                .ok_or_else(|| "This clip is no longer in history".to_owned())
                .and_then(|item| store.snapshot(item))
        };
        match data {
            Ok(data) => Some(Command::Restore(id, data)),
            Err(e) => {
                let _ = out.send(Notice::ActivationFailed(
                    id,
                    format!("Could not read the saved clip: {e}"),
                ));
                None
            }
        }
    } else {
        Some(command)
    }
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
    desktop: Option<crate::desktop::Integration>,
    out: Sender<Notice>,
    press_gate: crate::shortcut::PressGate,
    repeat_supported: bool,
    desktop_gate: crate::shortcut::PressGate,
    desktop_code: Option<u8>,
    recorder: crate::shortcut::Recorder,
    recording_keys: std::collections::HashSet<u8>,
    recorded_result: Option<crate::shortcut::Result>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn parse_shortcut(shortcut: &str) -> Result<(u32, ModMask), String> {
    crate::desktop::accelerator(shortcut)?;
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
    if mods == ModMask::default() {
        return Err("Include Super, Ctrl, Alt, or Shift in the shortcut".into());
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
        // Observe release edges even when the desktop owns the passive shortcut grab.
        use x11rb::protocol::xinput::{ConnectionExt as _, EventMask as XIEvents, XIEventMask};
        if c.xinput_xi_query_version(2, 0)
            .ok()
            .and_then(|v| v.reply().ok())
            .is_some()
        {
            let _ = c
                .xinput_xi_select_events(
                    c.setup().roots[screen].root,
                    &[XIEvents {
                        deviceid: 1,
                        mask: vec![XIEventMask::RAW_KEY_RELEASE],
                    }],
                )
                .ok()
                .and_then(|v| v.check().ok());
        }
        c.flush().map_err(err)?;
        // Detectable repeat produces one real release, rather than synthetic
        // release/press pairs while held. Never enable an unreliable binding.
        use x11rb::protocol::xkb::{BoolCtrl, ConnectionExt as _, PerClientFlag, ID};
        let repeat_supported = c
            .xkb_use_extension(1, 0)
            .ok()
            .and_then(|v| v.reply().ok())
            .filter(|v| v.supported)
            .and_then(|_| {
                c.xkb_per_client_flags(
                    ID::USE_CORE_KBD.into(),
                    PerClientFlag::DETECTABLE_AUTO_REPEAT,
                    PerClientFlag::DETECTABLE_AUTO_REPEAT,
                    BoolCtrl::default(),
                    BoolCtrl::default(),
                    BoolCtrl::default(),
                )
                .ok()
            })
            .and_then(|v| v.reply().ok())
            .is_some_and(|v| {
                v.value & PerClientFlag::DETECTABLE_AUTO_REPEAT != PerClientFlag::default()
            });
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
            desktop: None,
            press_gate: Default::default(),
            repeat_supported,
            desktop_gate: Default::default(),
            desktop_code: None,
            recorder: Default::default(),
            recording_keys: Default::default(),
            recorded_result: None,
            out,
        })
    }
    fn bind(&mut self, shortcut: &str) -> Result<(), String> {
        if let Some(desktop) = self.desktop.clone() {
            if desktop.automatic() {
                if desktop.read()?.as_deref() == Some(shortcut) {
                    return Ok(());
                }
                self.bind_native(shortcut)?;
                let result = desktop.register(shortcut);
                self.unbind();
                return result;
            }
        }
        self.bind_native(shortcut)
    }
    fn bind_native(&mut self, shortcut: &str) -> Result<(), String> {
        if !self.repeat_supported {
            return Err("Reliable X11 global shortcuts are unavailable. Configure your desktop to run clipvault --toggle.".into());
        }
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
            if result.is_err() {
                for mask in grabbed {
                    let _ = self.c.ungrab_key(code, root, mask);
                }
                let _ = self.c.flush();
                return Err(crate::desktop::unavailable(shortcut));
            }
            grabbed.push(mask);
        }
        if let Some((old_code, old_mods)) = self.key {
            for extra in extras {
                let _ = self.c.ungrab_key(old_code, root, old_mods | extra);
            }
        }
        self.key = Some((code, mods));
        self.press_gate.release();
        if self
            .c
            .query_keymap()
            .map_err(err)?
            .reply()
            .map_err(err)?
            .keys[code as usize / 8]
            & (1 << (code % 8))
            != 0
        {
            self.press_gate.press();
        }
        self.c.flush().map_err(err)?;
        Ok(())
    }
    fn unbind(&mut self) {
        let _ = self.out.send(Notice::ShortcutInactive);
        if let Some((code, mods)) = self.key.take() {
            for extra in [
                ModMask::default(),
                ModMask::LOCK,
                ModMask::M2,
                ModMask::LOCK | ModMask::M2,
            ] {
                let _ =
                    self.c
                        .ungrab_key(code, self.c.setup().roots[self.screen].root, mods | extra);
            }
        }
        self.press_gate.release();
        let _ = self.c.flush();
    }
    fn begin_recording(&mut self) -> Result<(), String> {
        if !self.repeat_supported {
            return Err("Native recording is unavailable on this X11 server. Configure a desktop shortcut for clipvault --toggle.".into());
        }
        if let Some(desktop) = &self.desktop {
            desktop.suspend()?;
        }
        self.unbind();
        let reply = self
            .c
            .grab_keyboard(
                false,
                self.c.setup().roots[self.screen].root,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .map_err(err)?
            .reply()
            .map_err(err)?;
        if reply.status != GrabStatus::SUCCESS {
            return Err(
                "Could not capture the keyboard. Close other shortcut recorders and try again."
                    .into(),
            );
        }
        self.recorder.begin();
        self.recorder.ready = true;
        self.recording_keys.clear();
        // Track keys already held when the control was clicked too.
        let map = self.c.query_keymap().map_err(err)?.reply().map_err(err)?;
        for code in 8..=255u8 {
            if map.keys[code as usize / 8] & (1 << (code % 8)) != 0 {
                self.recording_keys.insert(code);
            }
        }
        let _ = self.out.send(Notice::RecordingReady);
        Ok(())
    }
    fn end_recording(&mut self) {
        self.recorder.recording = false;
        self.recording_keys.clear();
        let _ = self.c.ungrab_keyboard(x11rb::CURRENT_TIME);
        let _ = self.c.flush();
    }
    fn record_key(&mut self, event: &Event) -> Option<crate::shortcut::Result> {
        use eframe::egui::{Event as E, Key, Modifiers};
        let (e, pressed) = match event {
            Event::KeyPress(e) => (e, true),
            Event::KeyRelease(e) => (e, false),
            _ => return None,
        };
        let repeat = pressed && !self.recording_keys.insert(e.detail);
        if !pressed {
            self.recording_keys.remove(&e.detail);
        }
        let symbol = self
            .c
            .get_keyboard_mapping(e.detail, 1)
            .ok()?
            .reply()
            .ok()?
            .keysyms[0];
        let modifier_mask = match symbol {
            0xffe1 | 0xffe2 => u16::from(ModMask::SHIFT),
            0xffe3 | 0xffe4 => u16::from(ModMask::CONTROL),
            0xffe9 | 0xffea => u16::from(ModMask::M1),
            0xffeb | 0xffec | 0xffe7 | 0xffe8 => u16::from(ModMask::M4),
            _ => 0,
        };
        let mut state = u16::from(e.state);
        if !pressed {
            state &= !modifier_mask;
        }
        let mods = Modifiers {
            ctrl: state & u16::from(ModMask::CONTROL) != 0,
            alt: state & u16::from(ModMask::M1) != 0,
            shift: state & u16::from(ModMask::SHIFT) != 0,
            mac_cmd: state & u16::from(ModMask::M4) != 0,
            command: false,
        };
        let key = if symbol == 0xff1b {
            Key::Escape
        } else {
            Key::from_name(
                &(char::from_u32(symbol).unwrap_or(' '))
                    .to_ascii_uppercase()
                    .to_string(),
            )
            .unwrap_or(Key::Enter)
        };
        let events = if modifier_mask == 0 {
            vec![E::Key {
                key,
                physical_key: None,
                pressed,
                repeat,
                modifiers: mods,
            }]
        } else {
            vec![]
        };
        let result = self
            .recorder
            .input(&events, mods, !self.recording_keys.is_empty(), true);
        if result.is_none() {
            let _ = self
                .out
                .send(Notice::ShortcutCandidate(self.recorder.display()));
        }
        result
    }
    fn serve(&mut self, e: Event) {
        // Clipboard transfers also pump X11 events. Capture keyboard input here
        // so a concurrent clipboard request cannot swallow recording releases.
        if self.recorder.recording && matches!(e, Event::KeyPress(_) | Event::KeyRelease(_)) {
            if let Some(result) = self.record_key(&e) {
                self.end_recording();
                self.recorded_result = Some(result);
            }
            return;
        }
        match e {
            Event::XinputRawKeyRelease(event) => {
                if self.desktop_code == u8::try_from(event.detail).ok() {
                    self.desktop_gate.release();
                    self.desktop_code = None;
                }
            }
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
            Event::KeyPress(e)
                if !self.recorder.recording
                    && self.key.is_some_and(|(code, _)| code == e.detail) =>
            {
                if self.press_gate.press() {
                    let _ = self.out.send(Notice::Open);
                }
            }
            Event::KeyRelease(e)
                if !self.recorder.recording
                    && self.key.is_some_and(|(code, _)| code == e.detail) =>
            {
                self.press_gate.release();
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
fn complete_restore(
    out: &Sender<Notice>,
    shortcut: String,
    result: Result<(), String>,
    feedback: Notice,
) {
    match result {
        Ok(()) => {
            let _ = out.send(Notice::ShortcutActive(shortcut));
            let _ = out.send(feedback);
        }
        Err(e) => {
            let _ = out.send(Notice::ShortcutInactive);
            let _ = out.send(Notice::ShortcutFailed(format!("Previous shortcut could not be restored: {e}. Configure a desktop shortcut or choose another combination.")));
        }
    }
}
fn shortcut_status(x: &X11, shortcut: String) -> Notice {
    if x.desktop.as_ref().is_some_and(|d| d.automatic()) {
        Notice::ShortcutConfigured(shortcut)
    } else {
        Notice::ShortcutActive(shortcut)
    }
}
fn restore_shortcut(x: &mut X11, store: &Shared, out: &Sender<Notice>, feedback: Notice) {
    let shortcut = store.lock().unwrap().settings.shortcut.clone();
    let result = x.bind(&shortcut);
    if result.is_ok() && x.desktop.as_ref().is_some_and(|d| d.automatic()) {
        let _ = out.send(shortcut_status(x, shortcut));
        let _ = out.send(feedback);
    } else {
        complete_restore(out, shortcut, result, feedback);
    }
}
/// Register before persisting, and restore the prior binding on either failure.
/// The bool on an error says whether restoration was confirmed, never guessed.
fn change_binding(
    old: &str,
    next: &str,
    mut bind: impl FnMut(&str) -> Result<(), String>,
    save: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), (String, bool)> {
    if let Err(error) = bind(next).and_then(|_| save(next)) {
        return match bind(old) {
            Ok(()) => Err((format!("{error} This shortcut is unavailable. Your current shortcut, {old}, is still active."), true)),
            Err(restore) => Err((format!("{error} Previous shortcut could not be restored: {restore}. Use desktop setup."), false)),
        };
    }
    Ok(())
}
fn apply_shortcut(x: &mut X11, store: &Shared, out: &Sender<Notice>, shortcut: &str) {
    let old = store.lock().unwrap().settings.shortcut.clone();
    let result = change_binding(
        &old,
        shortcut,
        |s| x.bind(s),
        |next| {
            let mut s = store.lock().unwrap();
            s.settings.shortcut = next.into();
            let result = s.save_settings();
            if result.is_err() {
                s.settings.shortcut = old.clone();
            }
            result.map_err(|e| format!("Could not save shortcut: {e}"))
        },
    );
    match result {
        Ok(()) => {
            let _ = out.send(Notice::ShortcutApplied(shortcut.into()));
            let _ = out.send(shortcut_status(x, shortcut.into()));
        }
        Err((mut error, restored)) => {
            if x.desktop.as_ref().is_some_and(|d| d.automatic()) {
                error = error.replace(" is still active.", " is still configured.");
            }
            let _ = out.send(if restored {
                shortcut_status(x, old)
            } else {
                Notice::ShortcutInactive
            });
            let _ = out.send(Notice::ShortcutFailed(error));
        }
    }
}
fn finish_recording(x: &mut X11, store: &Shared, out: &Sender<Notice>) {
    if let Some(result) = x.recorded_result.take() {
        match result {
            crate::shortcut::Result::Saved(s) => apply_shortcut(x, store, out, &s),
            crate::shortcut::Result::Cancelled => {
                restore_shortcut(x, store, out, Notice::RecordingCancelled);
            }
            crate::shortcut::Result::Invalid => {
                restore_shortcut(x, store, out, Notice::ShortcutFailed("Use modifiers and one letter or number. Your previous setting is unchanged.".into()));
            }
        }
    }
}
pub fn run_x11(store: Shared, rx: Receiver<Command>, out: Sender<Notice>) -> Result<(), String> {
    let mut x = X11::new(out.clone())?;
    x.desktop = Some(crate::desktop::Integration::new(
        store.lock().unwrap().root.clone(),
    ));
    let mut paste = crate::paste::Controller::new().ok();
    let desktop_events = x.desktop.as_ref().map(|desktop| desktop.watch());

    let shortcut = store.lock().unwrap().settings.shortcut.clone();
    let _ = out.send(Notice::Status("X11 · monitoring clipboard".into()));
    match x.bind(&shortcut) {
        Ok(()) => {
            let _ = out.send(shortcut_status(&x, shortcut));
        }
        Err(e) => {
            let _ = out.send(Notice::ShortcutFailed(e));
        }
    }
    let mut dirty = true;
    let mut last_poll = Instant::now();
    let mut last_owner = 0;
    loop {
        finish_recording(&mut x, &store, &out);
        while let Some(e) = x.c.poll_for_event().map_err(err)? {
            if matches!(&e,Event::XfixesSelectionNotify(n) if n.selection==x.clipboard&&n.owner!=x.win&&n.owner!=0)
            {
                dirty = true;
            }
            if matches!(&e, Event::KeyPress(_)) && !x.recorder.recording {
                if let Some(p) = &mut paste {
                    p.freeze();
                }
            }
            x.serve(e);
        }
        while let Ok(cmd) = rx.try_recv() {
            let Some(cmd) = resolve_restore(cmd, &store, &out) else {
                continue;
            };
            match cmd {
                Command::Flush(reply) => {
                    let _ = reply.send(store.lock().unwrap().flush());
                }
                Command::Stop => return Ok(()),
                Command::RestoreItem(_) => unreachable!("resolved above"),
                Command::SetShortcut(next) => apply_shortcut(&mut x, &store, &out, &next),
                Command::RememberTarget => {
                    if let Some(p) = &mut paste {
                        p.freeze();
                    }
                }
                Command::Paste => {
                    if let Some(p) = &mut paste {
                        p.begin();
                    } else {
                        let _ = out.send(Notice::PasteFeedback(
                            "Copied—press Ctrl+V to paste.".into(),
                        ));
                    }
                }
                Command::Toggle => {
                    if x.recorder.recording {
                        continue;
                    }
                    let shortcut = store.lock().unwrap().settings.shortcut.clone();
                    let symbol = parse_shortcut(&shortcut).ok().map(|(symbol, _)| symbol);
                    let setup = x.c.setup();
                    x.desktop_code = symbol.and_then(|symbol| {
                        x.c.get_keyboard_mapping(
                            setup.min_keycode,
                            setup.max_keycode - setup.min_keycode + 1,
                        )
                        .ok()?
                        .reply()
                        .ok()
                        .and_then(|m| {
                            m.keysyms
                                .chunks(m.keysyms_per_keycode as usize)
                                .position(|v| v.contains(&symbol))
                                .map(|n| setup.min_keycode + n as u8)
                        })
                    });
                    if x.desktop_gate.press() {
                        if let Some(p) = &mut paste {
                            p.freeze();
                        }
                        let _ = out.send(Notice::Open);
                    }
                }
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
                Command::RecordShortcut(start) => {
                    if start && !x.recorder.recording {
                        if let Err(e) = x.begin_recording() {
                            x.end_recording();
                            restore_shortcut(&mut x, &store, &out, Notice::ShortcutFailed(e));
                        }
                    } else if !start {
                        x.end_recording();
                        restore_shortcut(&mut x, &store, &out, Notice::RecordingCancelled);
                    }
                }
            }
        }
        finish_recording(&mut x, &store, &out);
        if let Some(p) = &mut paste {
            if let Some(notice) = p.tick() {
                let _ = out.send(notice);
            }
        }
        if let Some(code) = x.desktop_code {
            if x.c
                .query_keymap()
                .ok()
                .and_then(|v| v.reply().ok())
                .is_some_and(|r| r.keys[code as usize / 8] & (1 << (code % 8)) == 0)
            {
                x.desktop_gate.release();
                x.desktop_code = None;
            }
        }
        if !x.recorder.recording
            && desktop_events
                .as_ref()
                .is_some_and(|events| events.try_iter().next().is_some())
        {
            if let Some(d) = &x.desktop {
                if d.automatic() {
                    match d.read() {
                        Ok(Some(s)) => {
                            let mut store = store.lock().unwrap();
                            if store.settings.shortcut != s {
                                store.settings.shortcut = s.clone();
                                if let Err(e) = store.save_settings() {
                                    let _ = out.send(Notice::Error(e));
                                }
                            }
                            let _ = out.send(Notice::ShortcutConfigured(s));
                        }
                        _ => {
                            let _ = out.send(Notice::ShortcutInactive);
                        }
                    }
                }
            }
        }
        if dirty || last_poll.elapsed() > Duration::from_millis(250) {
            last_poll = Instant::now();
            if let Some(p) = &mut paste {
                p.observe();
            }
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
                                    && compatible_text(data.clone()) != x.owned
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
        std::thread::sleep(Duration::from_millis(40));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_does_not_claim_restoration_after_registration_failure() {
        for success in [true, false] {
            let (tx, rx) = mpsc::channel();
            complete_restore(
                &tx,
                "Super+V".into(),
                if success {
                    Ok(())
                } else {
                    Err("Desktop grabbed it".into())
                },
                Notice::RecordingCancelled,
            );
            let events: Vec<_> = rx.try_iter().collect();
            assert_eq!(events.len(), 2);
            if success {
                assert!(matches!(events[0], Notice::ShortcutActive(_)));
                assert!(matches!(events[1], Notice::RecordingCancelled));
            } else {
                assert!(matches!(events[0], Notice::ShortcutInactive));
                assert!(matches!(events[1], Notice::ShortcutFailed(_)));
            }
        }
    }
    #[test]
    fn conflicts_and_failed_save_keep_previous_binding_and_settings() {
        use std::cell::RefCell;
        for failure in ["conflict", "save", "none"] {
            let bound = RefCell::new("Super+V".to_string());
            let saved = RefCell::new("Super+V".to_string());
            let result = change_binding(
                "Super+V",
                "Ctrl+Alt+V",
                |s| {
                    if failure == "conflict" && s == "Ctrl+Alt+V" {
                        return Err("Already in use".into());
                    }
                    *bound.borrow_mut() = s.into();
                    Ok(())
                },
                |s| {
                    if failure == "save" {
                        return Err("Disk error".into());
                    }
                    *saved.borrow_mut() = s.into();
                    Ok(())
                },
            );
            if failure == "none" {
                assert!(result.is_ok());
                assert_eq!(*saved.borrow(), "Ctrl+Alt+V");
            } else {
                assert!(matches!(result, Err((_, true))));
                assert_eq!(*saved.borrow(), "Super+V");
                assert_eq!(*bound.borrow(), "Super+V");
            }
        }
        assert!(matches!(
            change_binding(
                "Super+V",
                "Ctrl+V",
                |_| Err("Desktop refused".into()),
                |_| Ok(())
            ),
            Err((_, false))
        ));
    }
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
        for invalid in ["V", "Super+", "Super+Enter", "Unknown+V", ""] {
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
