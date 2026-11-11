//! X11 focus restoration and bounded, capability-aware paste attempts.
//! XTEST submission is not proof the target application consumed the clipboard.
use crate::clipboard::Notice;
use std::time::{Duration, Instant};
use x11rb::{
    connection::Connection,
    protocol::{xproto::*, xtest::ConnectionExt as _},
    rust_connection::RustConnection,
};
fn may_restore(active: Option<Window>, target: Window, active_is_panel: bool) -> bool {
    active.is_none() || active == Some(target) || active_is_panel
}
fn can_inject(
    active: Option<Window>,
    target: Window,
    modifiers_held: bool,
    target_is_panel: bool,
) -> bool {
    active == Some(target) && !modifiers_held && !target_is_panel
}
pub struct Controller {
    connection: RustConnection,
    root: Window,
    last_external: Option<Window>,
    target: Option<Window>,
    pending: Option<(Instant, bool)>,
}
impl Controller {
    pub fn new() -> Result<Self, String> {
        let (connection, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = connection.setup().roots[screen].root;
        let mut s = Self {
            connection,
            root,
            last_external: None,
            target: None,
            pending: None,
        };
        s.observe();
        s.target = s.last_external;
        Ok(s)
    }
    fn atom(&self, name: &str) -> Option<Atom> {
        self.connection
            .intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()
            .map(|r| r.atom)
    }
    fn active(&self) -> Option<Window> {
        self.connection
            .get_property(
                false,
                self.root,
                self.atom("_NET_ACTIVE_WINDOW")?,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .next()
            .filter(|w| *w != 0)
    }
    fn panel(&self, w: Window) -> bool {
        self.connection
            .get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 128)
            .ok()
            .and_then(|v| v.reply().ok())
            .is_some_and(|r| {
                String::from_utf8_lossy(&r.value)
                    .to_lowercase()
                    .split('\0')
                    .any(|s| s == "clipvault")
            })
    }
    pub fn observe(&mut self) {
        if let Some(w) = self.active() {
            if !self.panel(w) {
                self.last_external = Some(w);
            }
        }
    }
    pub fn freeze(&mut self) {
        self.observe();
        self.target = self.last_external;
        self.pending = None;
    }
    pub fn begin(&mut self) {
        self.pending = Some((Instant::now(), false));
    }
    fn code(&self, symbol: u32) -> Option<u8> {
        let setup = self.connection.setup();
        let m = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .ok()?
            .reply()
            .ok()?;
        m.keysyms
            .chunks(m.keysyms_per_keycode as usize)
            .position(|keys| keys.contains(&symbol))
            .map(|n| setup.min_keycode + n as u8)
    }
    pub fn tick(&mut self) -> Option<Notice> {
        let (since, requested) = self.pending?;
        let fallback = || Notice::PasteFeedback("Copied—press Ctrl+V to paste.".into());
        let Some(target) = self.target else {
            self.pending = None;
            return Some(fallback());
        };
        if since.elapsed() > Duration::from_millis(800)
            || self
                .connection
                .get_window_attributes(target)
                .ok()
                .and_then(|v| v.reply().ok())
                .is_none_or(|r| r.map_state != MapState::VIEWABLE)
        {
            self.pending = None;
            return Some(fallback());
        }
        let active = self.active();
        if !may_restore(active, target, active.is_some_and(|w| self.panel(w))) {
            self.pending = None;
            return Some(fallback());
        }
        if !requested {
            let atom = self.atom("_NET_ACTIVE_WINDOW")?;
            let event =
                ClientMessageEvent::new(32, target, atom, [1, x11rb::CURRENT_TIME, 0, 0, 0]);
            let _ = self.connection.send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
            let _ = self.connection.flush();
            self.pending = Some((since, true));
            return None;
        }
        if active != Some(target) {
            return None;
        }
        let state = self
            .connection
            .query_pointer(self.root)
            .ok()?
            .reply()
            .ok()?
            .mask;
        if state.intersects(
            KeyButMask::SHIFT
                | KeyButMask::CONTROL
                | KeyButMask::MOD1
                | KeyButMask::MOD4
                | KeyButMask::MOD5,
        ) {
            return None;
        }
        self.pending = None;
        let (Some(ctrl), Some(v)) = (self.code(0xffe3), self.code(b'v' as u32)) else {
            return Some(fallback());
        };
        // Recheck immediately before injecting. Never inject into ClipVault or another window.
        if !can_inject(self.active(), target, false, self.panel(target)) {
            return Some(fallback());
        }
        for (kind, key) in [
            (KEY_PRESS_EVENT, ctrl),
            (KEY_PRESS_EVENT, v),
            (KEY_RELEASE_EVENT, v),
            (KEY_RELEASE_EVENT, ctrl),
        ] {
            if self
                .connection
                .xtest_fake_input(kind, key, 0, self.root, 0, 0, 0)
                .ok()
                .is_none_or(|c| c.check().is_err())
            {
                for key in [v, ctrl] {
                    let _ = self.connection.xtest_fake_input(
                        KEY_RELEASE_EVENT,
                        key,
                        0,
                        self.root,
                        0,
                        0,
                        0,
                    );
                }
                let _ = self.connection.flush();
                return Some(fallback());
            }
        }
        let _ = self.connection.flush();
        Some(Notice::PasteFeedback(
            "Paste requested. If nothing appeared, press Ctrl+V.".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_policy_rejects_unintended_windows_and_held_modifiers() {
        assert!(may_restore(Some(10), 42, true));
        assert!(may_restore(None, 42, false));
        assert!(!may_restore(Some(10), 42, false));
        assert!(!can_inject(None, 42, false, false));
        assert!(!can_inject(Some(10), 42, false, false));
        assert!(!can_inject(Some(42), 42, true, false));
        assert!(!can_inject(Some(42), 42, false, true));
        assert!(can_inject(Some(42), 42, false, false));
    }
}
