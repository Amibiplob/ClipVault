use eframe::egui::{Event, Key, Modifiers};
/// XKB detectable repeat gives one release per physical press.
#[derive(Default)]
pub struct PressGate {
    held: bool,
}
impl PressGate {
    pub fn press(&mut self) -> bool {
        let first = !self.held;
        self.held = true;
        first
    }
    pub fn release(&mut self) {
        self.held = false;
    }
}
#[derive(Default)]
pub struct Recorder {
    pub recording: bool,
    pub ready: bool,
    pub candidate: Option<String>,
    key: Option<Key>,
    invalid: bool,
}
pub enum Result {
    Saved(String),
    Cancelled,
    Invalid,
}
impl Recorder {
    pub fn begin(&mut self) {
        *self = Self {
            recording: true,
            ..Self::default()
        };
    }
    pub fn display(&self) -> String {
        self.candidate
            .as_ref()
            .map(|s| s.replace('+', " + "))
            .unwrap_or_else(|| "Press your shortcut".into())
    }
    pub fn input(
        &mut self,
        events: &[Event],
        modifiers: Modifiers,
        keys_down: bool,
        focused: bool,
    ) -> Option<Result> {
        if !self.recording || !self.ready {
            return None;
        }
        if !focused
            || events.iter().any(|e| {
                matches!(
                    e,
                    Event::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    }
                )
            })
        {
            self.recording = false;
            return Some(Result::Cancelled);
        }
        for e in events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = e
            {
                if self.key.is_some_and(|k| k != *key) {
                    self.invalid = true;
                }
                self.key = Some(*key);
                let name = key.name();
                if name.len() != 1
                    || !name.as_bytes()[0].is_ascii_alphanumeric()
                    || !has_modifier(*modifiers)
                {
                    self.invalid = true;
                } else {
                    let mut parts = Vec::new();
                    if modifiers.ctrl {
                        parts.push("Ctrl");
                    }
                    if modifiers.alt {
                        parts.push("Alt");
                    }
                    if modifiers.shift {
                        parts.push("Shift");
                    }
                    if modifiers.mac_cmd {
                        parts.push("Super");
                    }
                    parts.push(name);
                    self.candidate = Some(parts.join("+"));
                }
            }
        }
        if self.key.is_some() && !keys_down && !has_modifier(modifiers) {
            self.recording = false;
            return Some(if self.invalid {
                Result::Invalid
            } else {
                Result::Saved(self.candidate.clone().unwrap())
            });
        }
        None
    }
}
fn has_modifier(m: Modifiers) -> bool {
    m.ctrl || m.alt || m.shift || m.mac_cmd
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(k: Key, pressed: bool, m: Modifiers, repeat: bool) -> Event {
        Event::Key {
            key: k,
            physical_key: None,
            pressed,
            repeat,
            modifiers: m,
        }
    }
    #[test]
    fn held_key_repeat_toggles_once_per_physical_press() {
        let mut gate = PressGate::default();
        for _ in 0..100 {
            assert!(gate.press());
            for _ in 0..50 {
                assert!(!gate.press());
            }
            gate.release();
        }
    }
    #[test]
    fn records_super_v_after_all_keys_released_and_ignores_repeat() {
        let mut r = Recorder::default();
        r.begin();
        r.ready = true;
        let m = Modifiers {
            mac_cmd: true,
            ..Modifiers::NONE
        };
        assert!(r.input(&[], m, false, true).is_none()); // modifier only
        assert!(r
            .input(&[key(Key::V, true, m, false)], m, true, true)
            .is_none());
        assert_eq!(r.display(), "Super + V");
        assert!(r
            .input(&[key(Key::V, true, m, true)], m, true, true)
            .is_none());
        assert!(r
            .input(&[key(Key::V, false, m, false)], m, false, true)
            .is_none());
        assert!(
            matches!(r.input(&[],Modifiers::NONE,false,true),Some(Result::Saved(s)) if s=="Super+V")
        );
    }
    #[test]
    fn escape_focus_loss_invalid_and_all_modifiers() {
        for focused in [true, false] {
            let mut r = Recorder::default();
            r.begin();
            r.ready = true;
            assert!(matches!(
                r.input(
                    &[key(Key::Escape, true, Modifiers::NONE, false)],
                    Modifiers::NONE,
                    true,
                    focused
                ),
                Some(Result::Cancelled)
            ));
        }
        for k in [Key::V, Key::Enter] {
            let mut r = Recorder::default();
            r.begin();
            r.ready = true;
            r.input(
                &[key(k, true, Modifiers::NONE, false)],
                Modifiers::NONE,
                true,
                true,
            );
            assert!(matches!(
                r.input(&[], Modifiers::NONE, false, true),
                Some(Result::Invalid)
            ));
        }
        let mut r = Recorder::default();
        r.begin();
        r.ready = true;
        let m = Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
            mac_cmd: true,
            command: false,
        };
        r.input(&[key(Key::V, true, m, false)], m, true, true);
        assert!(
            matches!(r.input(&[],Modifiers::NONE,false,true),Some(Result::Saved(s)) if s=="Ctrl+Alt+Shift+Super+V")
        );
    }
}
