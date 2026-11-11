//! Desktop settings are owned by ClipVault only. Never remove another action.
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Clone, Debug, PartialEq)]
pub enum Desktop {
    Cinnamon,
    Gnome,
    Mate,
    Xfce,
    Plasma,
    Other,
}
pub fn detect() -> Desktop {
    detect_name(&format!(
        "{}:{}",
        std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        std::env::var("DESKTOP_SESSION").unwrap_or_default()
    ))
}
fn detect_name(name: &str) -> Desktop {
    let n = name.to_lowercase();
    if n.contains("cinnamon") {
        Desktop::Cinnamon
    } else if n.contains("gnome") || n.contains("ubuntu") {
        Desktop::Gnome
    } else if n.contains("mate") {
        Desktop::Mate
    } else if n.contains("xfce") {
        Desktop::Xfce
    } else if n.contains("kde") || n.contains("plasma") {
        Desktop::Plasma
    } else {
        Desktop::Other
    }
}
pub fn executable() -> Result<PathBuf, String> {
    static EXECUTABLE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    if let Some(path) = EXECUTABLE.get() {
        return Ok(path.clone());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| {
            p.is_absolute()
                && p.is_file()
                && std::env::var_os("APPDIR").is_some_and(|d| exe.starts_with(d))
        })
        .unwrap_or(exe);
    let _ = EXECUTABLE.set(path.clone());
    Ok(path)
}
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
pub fn toggle_command(root: &Path) -> String {
    format!(
        "{} --toggle --data-dir {}",
        shell_quote(
            &executable()
                .unwrap_or_else(|_| PathBuf::from("clipvault"))
                .to_string_lossy()
        ),
        shell_quote(&root.to_string_lossy())
    )
}
pub fn accelerator(s: &str) -> Result<String, String> {
    let parts: Vec<_> = s.split('+').map(str::trim).collect();
    let key = parts.last().ok_or("Choose a shortcut")?;
    if key.len() != 1 || !key.as_bytes()[0].is_ascii_alphanumeric() || parts.len() < 2 {
        return Err("Use modifiers and one letter or number".into());
    }
    let mut value = String::new();
    for modifier in &parts[..parts.len() - 1] {
        value.push_str(match modifier.to_lowercase().as_str() {
            "super" | "meta" => "<Super>",
            "ctrl" | "control" => "<Primary>",
            "alt" => "<Alt>",
            "shift" => "<Shift>",
            _ => return Err("Unknown modifier".into()),
        });
    }
    if value == "<Primary>" && key.eq_ignore_ascii_case("v") {
        return Err("Ctrl+V is used to paste in applications. Choose another combination, such as Ctrl+Alt+V.".into());
    }
    value.push_str(&key.to_lowercase());
    Ok(value)
}
pub fn combination(s: &str) -> Option<String> {
    let s = s
        .trim_matches(['\'', '"'])
        .replace("<Control>", "<Primary>");
    let mut parts = Vec::new();
    for (tag, name) in [
        ("<Primary>", "Ctrl"),
        ("<Alt>", "Alt"),
        ("<Shift>", "Shift"),
        ("<Super>", "Super"),
    ] {
        if s.contains(tag) {
            parts.push(name.to_owned());
        }
    }
    let key = s.rsplit('>').next()?;
    if parts.is_empty() || key.len() != 1 || !key.as_bytes()[0].is_ascii_alphanumeric() {
        return None;
    }
    parts.push(key.to_uppercase());
    Some(parts.join("+"))
}
fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
fn strings(value: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut quote = None;
    let mut escape = false;
    let mut current = String::new();
    for ch in value.chars() {
        if let Some(delimiter) = quote {
            if escape {
                current.push(match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => ch,
                });
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == delimiter {
                strings.push(std::mem::take(&mut current));
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
    }
    if quote.is_some() || escape {
        Vec::new()
    } else {
        strings
    }
}
fn variant(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}
#[derive(Clone)]
pub struct Integration {
    desktop: Desktop,
    root: PathBuf,
}
impl Integration {
    pub fn new(root: PathBuf) -> Self {
        Self {
            desktop: detect(),
            root,
        }
    }
    pub fn automatic(&self) -> bool {
        matches!(
            self.desktop,
            Desktop::Cinnamon | Desktop::Gnome | Desktop::Mate | Desktop::Xfce
        )
    }
    fn schema(&self) -> (&str, &str, &str, &str) {
        match self.desktop {
            Desktop::Cinnamon => (
                "org.cinnamon.desktop.keybindings",
                "custom-list",
                "org.cinnamon.desktop.keybindings.custom-keybinding",
                "/org/cinnamon/desktop/keybindings/custom-keybindings/clipvault/",
            ),
            Desktop::Mate => (
                "",
                "",
                "org.mate.control-center.keybinding",
                "/org/mate/desktop/keybindings/clipvault/",
            ),
            _ => (
                "org.gnome.settings-daemon.plugins.media-keys",
                "custom-keybindings",
                "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding",
                "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/clipvault/",
            ),
        }
    }
    fn owned_schema(&self) -> String {
        let (_, _, schema, path) = self.schema();
        format!("{schema}:{path}")
    }
    fn binding_value(&self, binding: &str) -> String {
        if self.desktop == Desktop::Cinnamon {
            format!("[{}]", variant(binding))
        } else {
            variant(binding)
        }
    }
    fn owned(&self) -> bool {
        run("gsettings", &["get", &self.owned_schema(), "name"])
            .is_ok_and(|n| strings(&n) == ["ClipVault"])
            && run("gsettings", &["get", &self.owned_schema(), "command"]).is_ok_and(|c| {
                strings(&c)
                    .first()
                    .is_some_and(|v| v == &toggle_command(&self.root))
            })
    }
    pub fn watch(&self) -> std::sync::mpsc::Receiver<()> {
        use std::os::unix::process::CommandExt;
        use std::{
            io::{BufRead, BufReader},
            process::Stdio,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        if !self.automatic() {
            return rx;
        }
        let mut jobs = Vec::new();
        if self.desktop == Desktop::Xfce {
            jobs.push((
                "xfconf-query",
                vec!["-c".into(), "xfce4-keyboard-shortcuts".into(), "-m".into()],
            ));
        } else {
            jobs.push(("gsettings", vec!["monitor".into(), self.owned_schema()]));
            if self.desktop != Desktop::Mate {
                jobs.push((
                    "gsettings",
                    vec![
                        "monitor".into(),
                        self.schema().0.into(),
                        self.schema().1.into(),
                    ],
                ));
            }
        }
        for (program, args) in jobs {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut command = Command::new(program);
                command
                    .args(args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());
                // Watchers must die with their parent, including an exec-based upgrade restart.
                unsafe {
                    command.pre_exec(|| {
                        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                        Ok(())
                    });
                }
                if let Ok(mut child) = command.spawn() {
                    if let Some(stdout) = child.stdout.take() {
                        for line in BufReader::new(stdout).lines() {
                            if line.is_err() || tx.send(()).is_err() {
                                break;
                            }
                        }
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                }
            });
        }
        rx
    }
    pub fn read(&self) -> Result<Option<String>, String> {
        if self.desktop == Desktop::Xfce {
            let all = run("xfconf-query", &["-c", "xfce4-keyboard-shortcuts", "-lv"])?;
            return Ok(all.lines().find_map(|line| {
                let (key, cmd) = line.split_once(char::is_whitespace)?;
                if cmd.trim() == toggle_command(&self.root) {
                    combination(key.trim_start_matches("/commands/custom/"))
                } else {
                    None
                }
            }));
        }
        if !self.automatic() || !self.owned() {
            return Ok(None);
        }
        if self.desktop != Desktop::Mate {
            let (parent, list, _, path) = self.schema();
            let entries = strings(&run("gsettings", &["get", parent, list])?);
            let name = path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(path);
            if !entries
                .iter()
                .any(|entry| entry == path || (self.desktop == Desktop::Cinnamon && entry == name))
            {
                return Ok(None);
            }
        }
        let value = run("gsettings", &["get", &self.owned_schema(), "binding"])?;
        Ok(strings(&value).first().and_then(|s| combination(s)))
    }
    pub fn register(&self, shortcut: &str) -> Result<(), String> {
        let accel = accelerator(shortcut)?;
        if !self.automatic() {
            return Err(
                "Automatic desktop setup is unavailable. Use the exact command shown below.".into(),
            );
        }
        if self.desktop == Desktop::Xfce {
            let property = format!("/commands/custom/{accel}");
            if let Ok(value) = run(
                "xfconf-query",
                &["-c", "xfce4-keyboard-shortcuts", "-p", &property],
            ) {
                if value != toggle_command(&self.root) {
                    return Err(unavailable(shortcut));
                }
            }
            // Create the replacement first. Remove only our old command after verification.
            let previous = self.read()?;
            run(
                "xfconf-query",
                &[
                    "-c",
                    "xfce4-keyboard-shortcuts",
                    "-p",
                    &property,
                    "-n",
                    "-t",
                    "string",
                    "-s",
                    &toggle_command(&self.root),
                ],
            )?;
            if self.read()?.as_deref() != Some(shortcut)
                && run(
                    "xfconf-query",
                    &["-c", "xfce4-keyboard-shortcuts", "-p", &property],
                )? != toggle_command(&self.root)
            {
                return Err("Desktop did not retain the binding".into());
            }
            if let Some(old) = previous.filter(|old| old != shortcut) {
                let old = format!("/commands/custom/{}", accelerator(&old)?);
                if run(
                    "xfconf-query",
                    &["-c", "xfce4-keyboard-shortcuts", "-p", &old],
                )? == toggle_command(&self.root)
                {
                    run(
                        "xfconf-query",
                        &["-c", "xfce4-keyboard-shortcuts", "-p", &old, "-r"],
                    )?;
                }
            }
            return Ok(());
        }
        let (parent, list, schema, path) = self.schema();
        // Inspect the desktop's own actions and every custom action, never reset them.
        let schemas: &[&str] = match self.desktop {
            Desktop::Cinnamon => &[
                "org.cinnamon.desktop.keybindings.wm",
                "org.cinnamon.desktop.keybindings.media-keys",
            ],
            Desktop::Mate => &["org.mate.Marco.global-keybindings"],
            _ => &[
                "org.gnome.desktop.wm.keybindings",
                "org.gnome.settings-daemon.plugins.media-keys",
                "org.gnome.shell.keybindings",
            ],
        };
        for schema in schemas {
            let bindings = run("gsettings", &["list-recursively", schema])?;
            if bindings.lines().any(|line| {
                strings(line)
                    .iter()
                    .any(|s| combination(s).as_deref() == Some(shortcut))
            }) {
                return Err(unavailable(shortcut));
            }
        }
        let mut paths = if self.desktop == Desktop::Mate {
            run("dconf", &["list", "/org/mate/desktop/keybindings/"])?
                .lines()
                .filter(|p| p.ends_with('/'))
                .map(|p| format!("/org/mate/desktop/keybindings/{p}"))
                .collect()
        } else {
            strings(&run("gsettings", &["get", parent, list])?)
                .into_iter()
                .map(|p| {
                    if p.starts_with('/') {
                        p
                    } else {
                        format!("/org/cinnamon/desktop/keybindings/custom-keybindings/{p}/")
                    }
                })
                .collect::<Vec<_>>()
        };
        for other in &paths {
            if other == path {
                if !self.owned() {
                    return Err("ClipVault's desktop settings path is occupied by another command; it was left unchanged.".into());
                }
                continue;
            }
            let other = format!("{schema}:{other}");
            if strings(&run("gsettings", &["get", &other, "binding"])?)
                .iter()
                .any(|s| combination(s).as_deref() == Some(shortcut))
            {
                return Err(unavailable(shortcut));
            }
        }
        let owned = self.owned_schema();
        // An unlisted existing path must also be ours or empty.
        let existing = run("gsettings", &["get", &owned, "command"])?;
        if !strings(&existing).first().is_none_or(String::is_empty) && !self.owned() {
            return Err("Existing desktop action was left unchanged".into());
        }
        run("gsettings", &["set", &owned, "name", &variant("ClipVault")])?;
        run(
            "gsettings",
            &[
                "set",
                &owned,
                "command",
                &variant(&toggle_command(&self.root)),
            ],
        )?;
        run(
            "gsettings",
            &["set", &owned, "binding", &self.binding_value(&accel)],
        )?;
        if self.desktop != Desktop::Mate {
            if !paths.iter().any(|p| p == path) {
                paths.push(path.into());
            }
            let entries: Vec<_> = paths
                .iter()
                .map(|p| {
                    variant(if self.desktop == Desktop::Cinnamon {
                        p.trim_end_matches('/').rsplit('/').next().unwrap_or(p)
                    } else {
                        p
                    })
                })
                .collect();
            run(
                "gsettings",
                &["set", parent, list, &format!("[{}]", entries.join(","))],
            )?;
        }
        if self.read()?.as_deref() != Some(shortcut) {
            return Err("Desktop binding could not be read back; use Keyboard settings.".into());
        }
        Ok(())
    }
    pub fn suspend(&self) -> Result<(), String> {
        if !self.automatic() {
            return Ok(());
        }
        if self.desktop == Desktop::Xfce {
            if let Some(old) = self.read()? {
                let p = format!("/commands/custom/{}", accelerator(&old)?);
                run(
                    "xfconf-query",
                    &["-c", "xfce4-keyboard-shortcuts", "-p", &p, "-r"],
                )?;
            }
        } else if self.owned() {
            run(
                "gsettings",
                &[
                    "set",
                    &self.owned_schema(),
                    "binding",
                    &self.binding_value(""),
                ],
            )?;
        }
        Ok(())
    }
}
pub fn unavailable(shortcut: &str) -> String {
    format!("{shortcut} is already used by another action. Choose a different shortcut, or change the existing binding in your desktop’s Keyboard settings.")
}
pub fn open_keyboard() -> Result<(), String> {
    let (program, args): (&str, &[&str]) = match detect() {
        Desktop::Cinnamon => ("cinnamon-settings", &["keyboard"]),
        Desktop::Gnome => ("gnome-control-center", &["keyboard"]),
        Desktop::Mate => ("mate-keybinding-properties", &[]),
        Desktop::Xfce => ("xfce4-keyboard-settings", &[]),
        Desktop::Plasma => ("systemsettings", &["kcm_keys"]),
        _ => {
            return Err(
                "Open your desktop's Keyboard settings and add the command shown here.".into(),
            )
        }
    };
    Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_and_accelerator_roundtrips() {
        assert_eq!(detect_name("X-Cinnamon:GNOME"), Desktop::Cinnamon);
        for s in ["Super+V", "Ctrl+Alt+V", "Ctrl+Alt+Shift+Super+9"] {
            assert_eq!(combination(&accelerator(s).unwrap()).unwrap(), s);
        }
        assert!(accelerator("Ctrl+V").unwrap_err().contains("used to paste"));
        assert!(accelerator("Super").is_err());
        assert!(accelerator("V").is_err());
        assert_eq!(
            strings("@as ['custom0', 'clipvault']"),
            ["custom0", "clipvault"]
        );
    }
    #[test]
    fn gvariant_command_strings_preserve_shell_quoting() {
        let command = "'/tmp/Clip Vault' --toggle --data-dir '/tmp/a\'b'";
        assert_eq!(strings(&variant(command)), [command]);
        assert_eq!(
            strings("\"'/tmp/Clip Vault' --toggle\""),
            ["'/tmp/Clip Vault' --toggle"]
        );
        assert_eq!(
            strings("['<Super>v', '<Control><Alt>v']"),
            ["<Super>v", "<Control><Alt>v"]
        );
    }
    #[test]
    fn command_quoting_preserves_arguments() {
        assert_eq!(shell_quote("/tmp/a'b $c"), "'/tmp/a'\\''b $c'");
    }
}
