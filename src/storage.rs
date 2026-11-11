use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub type Snapshot = BTreeMap<String, Vec<u8>>;
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Format {
    pub mime: String,
    pub file: String,
    pub size: usize,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Item {
    pub id: String,
    pub hash: String,
    pub kind: String,
    pub title: String,
    pub preview: String,
    pub time: i64,
    #[serde(default)]
    pub order: u64,
    /// Original capture time is unknown for legacy records whose time was reused.
    #[serde(default)]
    pub captured_at: Option<i64>,
    #[serde(default)]
    pub last_used: Option<i64>,
    pub pinned: bool,
    pub formats: Vec<Format>,
    pub thumbnail: Option<String>,
    pub source: String,
}
impl Item {
    pub fn recency(&self) -> i64 {
        self.last_used.unwrap_or(self.time)
    }
    pub fn copied_label(&self) -> String {
        self.captured_at
            .map(|t| format!("Copied {}", relative_time(t)))
            .unwrap_or_else(|| "Original capture time unavailable (legacy clip)".into())
    }
    pub fn exact_times(&self) -> String {
        let local = |t| {
            chrono::DateTime::from_timestamp(t, 0)
                .map(|v| v.with_timezone(&chrono::Local).format("%c %Z").to_string())
                .unwrap_or_else(|| "Unknown".into())
        };
        format!(
            "{}\nLast used: {}",
            self.captured_at
                .map(|t| format!("Copied: {}", local(t)))
                .unwrap_or_else(|| "Original capture time unavailable (legacy clip)".into()),
            self.last_used
                .map(local)
                .unwrap_or_else(|| "Not recorded".into())
        )
    }
}
pub fn relative_time(time: i64) -> String {
    let seconds = now().saturating_sub(time).max(0);
    if seconds < 60 {
        "Just now".into()
    } else if seconds < 3600 {
        format!(
            "{} minute{} ago",
            seconds / 60,
            if seconds / 60 == 1 { "" } else { "s" }
        )
    } else if seconds < 86400 {
        format!(
            "{} hour{} ago",
            seconds / 3600,
            if seconds / 3600 == 1 { "" } else { "s" }
        )
    } else if seconds < 172800 {
        "Yesterday".into()
    } else {
        format!("{} days ago", seconds / 86400)
    }
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Geometry {
    pub size: [f32; 2],
    pub maximized: bool,
}
impl Settings {
    pub fn geometry(&self) -> Geometry {
        (if self.compact {
            &self.compact_geometry
        } else {
            &self.comfortable_geometry
        })
        .clone()
        .unwrap_or(Geometry {
            size: self.window_size,
            maximized: false,
        })
    }
    pub fn set_geometry(&mut self, geometry: Geometry) {
        if self.compact {
            self.compact_geometry = Some(geometry);
        } else {
            self.comfortable_geometry = Some(geometry);
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub limit: usize,
    pub days: i64,
    pub light: bool,
    pub paused: bool,
    pub shortcut: String,
    pub updates: crate::updates::Preferences,
    pub exclude_content: String,
    pub exclude_apps: String,
    pub max_mb: usize,
    pub startup: bool,
    pub start_hidden: bool,
    pub system_title_bar: bool,
    pub app_font: String,
    pub confirm_before_delete: bool,
    pub changelog_dismissed_version: String,
    pub age_enabled: bool,
    pub count_enabled: bool,
    pub skip_consecutive: bool,
    pub hide_after_copy: bool,
    pub compact: bool,
    pub always_on_top: bool,
    pub window_size: [f32; 2],
    pub window_position: Option<[f32; 2]>,
    pub compact_geometry: Option<Geometry>,
    pub comfortable_geometry: Option<Geometry>,
    pub divider: f32,
    pub font_size: f32,
    pub theme: String,
    pub clear_on_reboot: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            limit: 500,
            days: 30,
            light: false,
            paused: false,
            shortcut: "Super+V".into(),
            updates: Default::default(),
            exclude_content: String::new(),
            exclude_apps: "keepass\n1password\nbitwarden".into(),
            max_mb: 32,
            startup: false,
            start_hidden: false,
            system_title_bar: true,
            app_font: String::new(),
            confirm_before_delete: true,
            changelog_dismissed_version: String::new(),
            age_enabled: true,
            count_enabled: true,
            skip_consecutive: true,
            hide_after_copy: false,
            compact: true,
            always_on_top: false,
            window_size: [440., 540.],
            window_position: None,
            compact_geometry: None,
            comfortable_geometry: None,
            divider: 0.44,
            font_size: 14.,
            theme: String::new(),
            clear_on_reboot: false,
        }
    }
}

// Bind the system SQLite ABI dynamically: no database server or network is involved.
pub struct Database {
    _lib: libloading::Library,
    db: *mut c_void,
    exec: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        Option<
            unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
        >,
        *mut c_void,
        *mut *mut c_char,
    ) -> c_int,
    close: unsafe extern "C" fn(*mut c_void) -> c_int,
}
impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        unsafe {
            let lib = libloading::Library::new("libsqlite3.so.0").map_err(|e| e.to_string())?;
            let open: unsafe extern "C" fn(
                *const c_char,
                *mut *mut c_void,
                c_int,
                *const c_char,
            ) -> c_int = *lib.get(b"sqlite3_open_v2\0").map_err(|e| e.to_string())?;
            let mut db = std::ptr::null_mut();
            if open(
                CString::new(path.to_string_lossy().as_bytes())
                    .unwrap()
                    .as_ptr(),
                &mut db,
                6,
                std::ptr::null(),
            ) != 0
            {
                return Err("Cannot open local history database".into());
            }
            let exec = *lib.get(b"sqlite3_exec\0").map_err(|e| e.to_string())?;
            let close = *lib.get(b"sqlite3_close\0").map_err(|e| e.to_string())?;
            let out = Self {
                _lib: lib,
                db,
                exec,
                close,
            };
            out.run("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS history (id TEXT PRIMARY KEY, json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY, json TEXT NOT NULL);")?;
            Ok(out)
        }
    }
    pub fn run(&self, sql: &str) -> Result<(), String> {
        unsafe {
            if (self.exec)(
                self.db,
                CString::new(sql).map_err(|e| e.to_string())?.as_ptr(),
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
            {
                Ok(())
            } else {
                Err("SQLite could not save clipboard history".into())
            }
        }
    }
    pub fn transaction(&self, sql: &str) -> Result<(), String> {
        self.run("BEGIN IMMEDIATE")?;
        if let Err(e) = self.run(sql).and_then(|_| self.run("COMMIT")) {
            let _ = self.run("ROLLBACK");
            return Err(e);
        }
        Ok(())
    }
    pub fn rows(&self, sql: &str) -> Result<Vec<String>, String> {
        unsafe extern "C" fn cb(
            data: *mut c_void,
            n: c_int,
            values: *mut *mut c_char,
            _: *mut *mut c_char,
        ) -> c_int {
            if n > 0 && !(*values).is_null() {
                (*(data as *mut Vec<String>))
                    .push(CStr::from_ptr(*values).to_string_lossy().into_owned());
            }
            0
        }
        let mut rows: Vec<String> = Vec::new();
        unsafe {
            if (self.exec)(
                self.db,
                CString::new(sql).unwrap().as_ptr(),
                Some(cb),
                &mut rows as *mut _ as *mut c_void,
                std::ptr::null_mut(),
            ) != 0
            {
                return Err("Could not read history".into());
            }
        }
        Ok(rows)
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.db);
        }
    }
}
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub struct Store {
    pub root: PathBuf,
    db: Database,
    pub items: Vec<Item>,
    pub settings: Settings,
    pub revision: u64,
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("items")).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let db = Database::open(&root.join("history.sqlite3"))?;
        db.run(
            "CREATE TABLE IF NOT EXISTS clip_search (id TEXT PRIMARY KEY, content TEXT NOT NULL)",
        )?;
        // Read migration input in bounded batches; retain summaries, not full text payloads.
        let mut items = Vec::new();
        let mut offset = 0;
        loop {
            let batch = db.rows(&format!(
                "SELECT json FROM history ORDER BY id LIMIT 128 OFFSET {offset}"
            ))?;
            if batch.is_empty() {
                break;
            }
            let mut search = String::new();
            for value in batch {
                let mut item: Item = serde_json::from_str(&value)
                    .map_err(|e| format!("History could not be read; no data was removed: {e}"))?;
                search.push_str(&format!(
                    "INSERT OR IGNORE INTO clip_search VALUES ({},{});",
                    quote(&item.id),
                    quote(&format!("{} {}", item.title, item.preview).to_lowercase())
                ));
                item.preview = item.preview.chars().take(512).collect();
                items.push(item);
            }
            db.transaction(&search)?;
            offset += 128;
        }
        let mut settings = match db.rows("SELECT json FROM settings WHERE id=1")?.first() {
            Some(v) => serde_json::from_str(v).map_err(|e| {
                format!("Saved settings are unreadable; history has not been modified: {e}")
            })?,
            None => Settings::default(),
        };
        // Legacy zero-day retention meant disabled; keep that behavior with
        // the new independent switch and a usable value when enabled later.
        if settings.days <= 0 {
            settings.age_enabled = false;
            settings.days = 30;
        }
        // Migrate the current mode's legacy geometry once; initialize the other independently.
        if settings.compact_geometry.is_none() && settings.comfortable_geometry.is_none() {
            settings.set_geometry(Geometry {
                size: settings.window_size,
                maximized: false,
            });
        }
        settings.compact_geometry.get_or_insert(Geometry {
            size: [440., 540.],
            maximized: false,
        });
        settings.comfortable_geometry.get_or_insert(Geometry {
            size: [780., 540.],
            maximized: false,
        });
        // Additive JSON fields retain the existing tables and payload paths.
        db.transaction("PRAGMA user_version=1;")?;
        let mut s = Self {
            root,
            db,
            items,
            settings,
            revision: 0,
        };
        if let Ok(boot) = fs::read_to_string("/proc/sys/kernel/random/boot_id") {
            s.apply_boot_id(boot.trim())?;
        }
        s.cleanup()?;
        s.sort();
        s.collect_orphans();
        Ok(s)
    }
    pub fn apply_boot_id(&mut self, boot: &str) -> Result<(), String> {
        if boot.is_empty() {
            return Ok(());
        }
        self.db.run(
            "CREATE TABLE IF NOT EXISTS runtime_state (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )?;
        let previous = self
            .db
            .rows("SELECT value FROM runtime_state WHERE key='boot'")?;
        let clear = self.settings.clear_on_reboot && previous.first().is_some_and(|v| v != boot);
        let ids: Vec<_> = self
            .items
            .iter()
            .filter(|i| clear && !i.pinned)
            .map(|i| i.id.clone())
            .collect();
        let deletes: String = ids
            .iter()
            .map(|id| {
                format!(
                    "DELETE FROM history WHERE id={}; DELETE FROM clip_search WHERE id={0};",
                    quote(id)
                )
            })
            .collect();
        self.db.transaction(&format!(
            "{}INSERT OR REPLACE INTO runtime_state VALUES ('boot',{});",
            deletes,
            quote(boot)
        ))?;
        self.revision = self.revision.wrapping_add(1);
        self.items.retain(|i| !ids.contains(&i.id));
        Ok(())
    }
    pub fn flush(&self) -> Result<(), String> {
        self.db.run("PRAGMA wal_checkpoint(FULL)")
    }
    pub fn search_ids(&self, query: &str) -> Result<std::collections::HashSet<String>, String> {
        let escaped = query
            .to_lowercase()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        Ok(self
            .db
            .rows(&format!(
                "SELECT id FROM clip_search WHERE content LIKE {} ESCAPE '\\'",
                quote(&format!("%{escaped}%"))
            ))?
            .into_iter()
            .collect())
    }
    pub fn sort(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.items.sort_by(|a, b| {
            b.recency()
                .cmp(&a.recency())
                .then(b.order.cmp(&a.order))
                .then(b.id.cmp(&a.id))
        });
    }
    fn save_item(&self, item: &Item) -> Result<(), String> {
        self.db.transaction(&format!(
            "INSERT OR REPLACE INTO history VALUES ({},{}); INSERT OR IGNORE INTO clip_search VALUES ({},{});",
            quote(&item.id), quote(&serde_json::to_string(item).unwrap()),
            quote(&item.id), quote(&format!("{} {}",item.title,item.preview).to_lowercase())
        ))
    }
    pub fn save_settings(&mut self) -> Result<(), String> {
        self.settings.limit = self.settings.limit.clamp(1, 10000);
        self.settings.days = self.settings.days.clamp(0, 3650);
        self.settings.max_mb = self.settings.max_mb.clamp(1, 128);
        let ids = self.cleanup_ids(&self.settings);
        let deletes: String = ids
            .iter()
            .map(|id| {
                format!(
                    "DELETE FROM history WHERE id={}; DELETE FROM clip_search WHERE id={0};",
                    quote(id)
                )
            })
            .collect();
        self.db.transaction(&format!(
            "INSERT OR REPLACE INTO settings VALUES (1,{});{}",
            quote(&serde_json::to_string(&self.settings).unwrap()),
            deletes
        ))?;
        self.revision = self.revision.wrapping_add(1);
        self.items.retain(|i| !ids.contains(&i.id));
        self.collect_orphans();
        Ok(())
    }
    pub fn ingest(&mut self, data: &Snapshot, source: &str) -> Result<bool, String> {
        if data.is_empty() || self.settings.paused {
            return Ok(false);
        }
        let bytes: usize = data.values().map(Vec::len).sum();
        if bytes > self.settings.max_mb * 1024 * 1024 {
            return Err("Clipboard item exceeds the configured maximum clip size".into());
        }
        let preview = text_of(data);
        if self.settings.exclude_content.lines().any(|x| {
            !x.trim().is_empty()
                && data.iter().any(|(mime, bytes)| {
                    (mime.starts_with("text/")
                        || [
                            "UTF8_STRING",
                            "STRING",
                            "TEXT",
                            "application/rtf",
                            "x-special/gnome-copied-files",
                        ]
                        .contains(&mime.as_str()))
                        && String::from_utf8_lossy(bytes)
                            .to_lowercase()
                            .contains(&x.trim().to_lowercase())
                })
        }) || self.settings.exclude_apps.lines().any(|x| {
            !x.trim().is_empty() && source.to_lowercase().contains(&x.trim().to_lowercase())
        }) {
            return Ok(false);
        }
        // Password-manager hints are used by several Linux clipboard providers.
        if data.keys().any(|m| crate::clipboard::sensitive_mime(m)) {
            return Ok(false);
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut h);
        let hash = format!("{:016x}", h.finish());
        if let Some(i) = self
            .items
            .iter()
            .position(|i| i.hash == hash && self.snapshot(i).as_ref() == Ok(data))
        {
            let id = self.items[i].id.clone();
            if !self.settings.skip_consecutive || i != 0 {
                self.touch(&id)?;
            }
            return Ok(false);
        }
        let id = format!(
            "{}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            hash
        );
        let dir = self.root.join("items").join(&id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut formats = Vec::new();
        for (n, (mime, b)) in data.iter().enumerate() {
            let file = format!("items/{id}/{n}.bin");
            fs::write(self.root.join(&file), b).map_err(|e| e.to_string())?;
            formats.push(Format {
                mime: mime.clone(),
                file,
                size: b.len(),
            });
        }
        let kind = classify(data, &preview);
        let mut thumbnail = None;
        if kind == "Images" {
            for (mime, b) in data {
                if mime.starts_with("image/") {
                    let mut limits = image::Limits::default();
                    limits.max_alloc = Some(256 * 1024 * 1024);
                    let mut reader = image::ImageReader::new(std::io::Cursor::new(b))
                        .with_guessed_format()
                        .map_err(|e| e.to_string())?;
                    reader.limits(limits);
                    if let Ok(im) = reader.decode() {
                        let p = format!("items/{id}/thumb.png");
                        let _ = im.thumbnail(480, 240).save(self.root.join(&p));
                        thumbnail = Some(p);
                        break;
                    }
                }
            }
        }
        if kind == "Images" && thumbnail.is_none() {
            if let Some(f) = formats.iter().find(|f| f.mime.starts_with("image/")) {
                let p = format!("items/{id}/thumb.png");
                if native_thumbnail(&self.root.join(&f.file), &self.root.join(&p)).is_ok() {
                    thumbnail = Some(p);
                }
            }
        }
        let title = match kind.as_str() {
            "Images" => "Copied image".into(),
            "Files" => file_title(&preview),
            "Links" => preview
                .lines()
                .next()
                .unwrap_or("Link")
                .chars()
                .take(100)
                .collect(),
            _ => preview
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("Formatted content")
                .chars()
                .take(100)
                .collect(),
        };
        let mut item = Item {
            id,
            hash,
            kind,
            title,
            preview: preview.chars().take(200000).collect(),
            time: now(),
            captured_at: Some(now()),
            last_used: None,
            order: self
                .items
                .iter()
                .map(|i| i.order)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            pinned: false,
            formats,
            thumbnail,
            source: source.into(),
        };
        // Flush payloads before committing metadata, including on sudden power loss.
        for f in &item.formats {
            fs::File::open(self.root.join(&f.file))
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        fs::File::open(&dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        fs::File::open(self.root.join("items"))
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        self.save_item(&item)?;
        item.preview = item.preview.chars().take(512).collect();
        self.items.push(item);
        self.sort();
        self.cleanup()?;
        Ok(true)
    }
    pub fn snapshot(&self, item: &Item) -> Result<Snapshot, String> {
        let mut d = Snapshot::new();
        for f in &item.formats {
            d.insert(f.mime.clone(), {
                let path = fs::canonicalize(self.root.join(&f.file)).map_err(|e| e.to_string())?;
                let items = fs::canonicalize(self.root.join("items")).map_err(|e| e.to_string())?;
                if !path.starts_with(items) {
                    return Err("Saved payload path is outside the history directory".into());
                }
                fs::read(path).map_err(|e| e.to_string())?
            });
        }
        Ok(d)
    }
    pub fn pin(&mut self, id: &str) -> Result<(), String> {
        if let Some(i) = self.items.iter().position(|x| x.id == id) {
            let mut it = self.items[i].clone();
            it.pinned = !it.pinned;
            self.save_item(&it)?;
            self.items[i] = it;
            self.sort();
        }
        Ok(())
    }
    pub fn touch(&mut self, id: &str) -> Result<(), String> {
        if let Some(i) = self.items.iter().position(|x| x.id == id) {
            let mut item = self.items[i].clone();
            item.last_used = Some(now());
            item.order = self
                .items
                .iter()
                .map(|i| i.order)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            self.save_item(&item)?;
            self.items[i] = item;
            self.sort();
        }
        self.cleanup()
    }
    pub fn full_text(&self, item: &Item) -> Result<String, String> {
        Ok(text_of(&self.snapshot(item)?))
    }
    pub fn edit_new(&mut self, original: &Item, text: &str) -> Result<Option<Item>, String> {
        if text == self.full_text(original)? {
            return Ok(None);
        }
        if text.is_empty() {
            return Err("Enter some text before saving".into());
        }
        let data = Snapshot::from([("text/plain;charset=utf-8".into(), text.as_bytes().to_vec())]);
        if self
            .items
            .iter()
            .any(|i| self.snapshot(i).as_ref() == Ok(&data))
        {
            return Err("This edited text already exists; no new clip was created".into());
        }
        // Explicit edits are allowed while monitoring is paused, but still respect exclusions/limits.
        let paused = self.settings.paused;
        self.settings.paused = false;
        let result = self.ingest(&data, "ClipVault editor");
        self.settings.paused = paused;
        match result? {
            true => Ok(self.items.iter().find(|i| self.snapshot(i).as_ref() == Ok(&data)).cloned()),
            false => Err("This text already exists or is excluded by Privacy settings; no new clip was created".into()),
        }
    }
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.remove_ids(&[id.to_owned()])
    }
    fn remove_ids(&mut self, ids: &[String]) -> Result<(), String> {
        let sql: String = ids
            .iter()
            .map(|id| {
                format!(
                    "DELETE FROM history WHERE id={}; DELETE FROM clip_search WHERE id={0};",
                    quote(id)
                )
            })
            .collect();
        self.db.transaction(&sql)?;
        self.revision = self.revision.wrapping_add(1);
        self.items.retain(|i| !ids.contains(&i.id));
        self.collect_orphans();
        Ok(())
    }
    pub fn clear(&mut self, include_pinned: bool) -> Result<(), String> {
        let ids: Vec<_> = self
            .items
            .iter()
            .filter(|i| include_pinned || !i.pinned)
            .map(|i| i.id.clone())
            .collect();
        self.remove_ids(&ids)
    }
    pub fn cleanup_ids(&self, settings: &Settings) -> Vec<String> {
        let mut items: Vec<_> = self.items.iter().filter(|i| !i.pinned).collect();
        items.sort_by(|a, b| {
            b.recency()
                .cmp(&a.recency())
                .then(b.order.cmp(&a.order))
                .then(b.id.cmp(&a.id))
        });
        items
            .into_iter()
            .enumerate()
            .filter(|(n, i)| {
                (settings.count_enabled && *n >= settings.limit)
                    || (settings.age_enabled
                        && settings.days > 0
                        && i.recency() < now() - settings.days * 86400)
            })
            .map(|(_, i)| i.id.clone())
            .collect()
    }
    pub fn cleanup(&mut self) -> Result<(), String> {
        self.sort();
        self.remove_ids(&self.cleanup_ids(&self.settings))
    }
    fn collect_orphans(&self) {
        // A migrated clip may share another clip's payload. Keep every referenced
        // file and never follow symlinks or untrusted paths outside items/.
        let mut referenced: std::collections::HashSet<PathBuf> = self
            .items
            .iter()
            .flat_map(|i| {
                i.formats
                    .iter()
                    .map(|f| PathBuf::from(&f.file))
                    .chain(i.thumbnail.iter().map(PathBuf::from))
            })
            .collect();
        for item in &self.items {
            referenced.insert(PathBuf::from("items").join(&item.id));
        }
        if let Ok(dirs) = fs::read_dir(self.root.join("items")) {
            for dir in dirs.flatten() {
                if !dir.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                let relative = PathBuf::from("items").join(dir.file_name());
                if !referenced.iter().any(|p| p.starts_with(&relative)) {
                    let _ = fs::remove_dir_all(dir.path());
                }
            }
        }
    }
}
pub fn text_of(data: &Snapshot) -> String {
    for wanted in [
        "x-special/gnome-copied-files",
        "text/uri-list",
        "text/plain;charset=utf-8",
        "text/plain;charset=UTF-8",
        "UTF8_STRING",
        "text/plain",
        "TEXT",
        "STRING",
        "text/html",
        "text/rtf",
        "application/rtf",
    ] {
        if let Some(b) = data.get(wanted) {
            return String::from_utf8_lossy(b).trim_end_matches('\0').into();
        }
    }
    String::new()
}
pub fn classify(data: &Snapshot, text: &str) -> String {
    if data
        .keys()
        .any(|m| m == "text/uri-list" || m == "x-special/gnome-copied-files")
        || text.starts_with("file://")
        || text.starts_with('/')
    {
        "Files"
    } else if data.keys().any(|m| m.starts_with("image/")) {
        "Images"
    } else if (text.starts_with("https://") || text.starts_with("http://"))
        && !text.trim().contains(char::is_whitespace)
    {
        "Links"
    } else {
        "Text"
    }
    .into()
}
pub fn file_title(text: &str) -> String {
    let lines: Vec<_> = text
        .lines()
        .filter(|l| !l.starts_with('#') && *l != "copy" && *l != "cut" && !l.is_empty())
        .collect();
    let first = lines
        .first()
        .unwrap_or(&"File")
        .rsplit('/')
        .next()
        .unwrap_or("File");
    // Percent escapes encode bytes, including multibyte UTF-8 filenames.
    let raw = first.as_bytes();
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' && index + 2 < raw.len() {
            if let Ok(pair) = std::str::from_utf8(&raw[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(pair, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        decoded.push(raw[index]);
        index += 1;
    }
    let name = String::from_utf8_lossy(&decoded);
    if lines.len() > 1 {
        format!("{name} + {} more", lines.len() - 1)
    } else {
        name.into_owned()
    }
}

// Decode the actual saved GIF/WebP image for Comfortable preview. Never replace its payload.
pub fn native_image(path: &Path) -> Result<image::DynamicImage, image::ImageError> {
    fn decode(path: &Path) -> Result<image::DynamicImage, String> {
        unsafe {
            let lib =
                libloading::Library::new("libgdk_pixbuf-2.0.so.0").map_err(|e| e.to_string())?;
            let load: libloading::Symbol<
                unsafe extern "C" fn(
                    *const c_char,
                    c_int,
                    c_int,
                    c_int,
                    *mut *mut c_void,
                ) -> *mut c_void,
            > = lib
                .get(b"gdk_pixbuf_new_from_file_at_scale\0")
                .map_err(|e| e.to_string())?;
            let get = |name: &[u8]| -> Result<
                libloading::Symbol<unsafe extern "C" fn(*mut c_void) -> c_int>,
                String,
            > { lib.get(name).map_err(|e| e.to_string()) };
            let width = get(b"gdk_pixbuf_get_width\0")?;
            let height = get(b"gdk_pixbuf_get_height\0")?;
            let stride = get(b"gdk_pixbuf_get_rowstride\0")?;
            let channels = get(b"gdk_pixbuf_get_n_channels\0")?;
            let pixels: libloading::Symbol<unsafe extern "C" fn(*mut c_void) -> *mut u8> = lib
                .get(b"gdk_pixbuf_get_pixels\0")
                .map_err(|e| e.to_string())?;
            let unref: libloading::Symbol<unsafe extern "C" fn(*mut c_void)> =
                lib.get(b"g_object_unref\0").map_err(|e| e.to_string())?;
            let filename =
                CString::new(path.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
            let pix = load(filename.as_ptr(), 1600, 1200, 1, std::ptr::null_mut());
            if pix.is_null() {
                return Err("Image decoder unavailable".into());
            }
            let (w, h, row, n) = (width(pix), height(pix), stride(pix), channels(pix));
            if w <= 0
                || h <= 0
                || (w as u64 * h as u64) > 64_000_000
                || !(3..=4).contains(&n)
                || row < w * n
            {
                unref(pix);
                return Err("Image exceeds preview limits".into());
            }
            let data = pixels(pix);
            if data.is_null() {
                unref(pix);
                return Err("Image data unavailable".into());
            }
            let rgba = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                let p = data.add(y as usize * row as usize + x as usize * n as usize);
                image::Rgba([
                    *p,
                    *p.add(1),
                    *p.add(2),
                    if n == 4 { *p.add(3) } else { 255 },
                ])
            });
            unref(pix);
            Ok(image::DynamicImage::ImageRgba8(rgba))
        }
    }
    decode(path).map_err(|e| image::ImageError::IoError(std::io::Error::other(e)))
}

// GdkPixbuf supplies GIF and WebP thumbnail decoding on Linux. Originals stay untouched.
fn native_thumbnail(input: &Path, output: &Path) -> Result<(), String> {
    unsafe {
        let lib = libloading::Library::new("libgdk_pixbuf-2.0.so.0").map_err(|e| e.to_string())?;
        let load: libloading::Symbol<
            unsafe extern "C" fn(
                *const c_char,
                c_int,
                c_int,
                c_int,
                *mut *mut c_void,
            ) -> *mut c_void,
        > = lib
            .get(b"gdk_pixbuf_new_from_file_at_scale\0")
            .map_err(|e| e.to_string())?;
        let save: libloading::Symbol<
            unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                *const *const c_char,
                *const *const c_char,
                *mut *mut c_void,
            ) -> c_int,
        > = lib.get(b"gdk_pixbuf_savev\0").map_err(|e| e.to_string())?;
        let unref: libloading::Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"g_object_unref\0").map_err(|e| e.to_string())?;
        let pix = load(
            CString::new(input.to_string_lossy().as_bytes())
                .unwrap()
                .as_ptr(),
            480,
            240,
            1,
            std::ptr::null_mut(),
        );
        if pix.is_null() {
            return Err("No system image decoder for this format".into());
        }
        let result = save(
            pix,
            CString::new(output.to_string_lossy().as_bytes())
                .unwrap()
                .as_ptr(),
            c"png".as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
        );
        unref(pix);
        if result != 0 {
            Ok(())
        } else {
            Err("Cannot create thumbnail".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> Store {
        let p = std::env::temp_dir().join(format!(
            "cv-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        Store::open(p).unwrap()
    }
    #[test]
    fn hide_after_copy_defaults_off_and_preserves_explicit_saved_preferences() {
        assert!(!Settings::default().hide_after_copy);
        assert!(
            !serde_json::from_str::<Settings>(r#"{"shortcut":"Ctrl+Alt+V"}"#)
                .unwrap()
                .hide_after_copy
        );
        for hide in [false, true] {
            let mut s = store();
            assert!(!s.settings.hide_after_copy);
            s.ingest(
                &Snapshot::from([("text/plain".into(), b"keep history".to_vec())]),
                "test",
            )
            .unwrap();
            let id = s.items[0].id.clone();
            s.pin(&id).unwrap();
            s.settings.hide_after_copy = hide;
            s.settings.shortcut = "Ctrl+Alt+V".into();
            s.save_settings().unwrap();
            let root = s.root.clone();
            drop(s);
            let s = Store::open(root).unwrap();
            assert_eq!(s.settings.hide_after_copy, hide);
            assert_eq!(s.settings.shortcut, "Ctrl+Alt+V");
            assert_eq!(s.items.len(), 1);
            assert_eq!(s.items[0].id, id);
            assert!(s.items[0].pinned);
            assert_eq!(
                s.snapshot(&s.items[0]).unwrap()["text/plain"],
                b"keep history"
            );
            fs::remove_dir_all(&s.root).unwrap();
        }
    }
    #[test]
    fn multi_format_roundtrip_and_dedup() {
        let mut s = store();
        let d = Snapshot::from([
            ("text/plain".into(), b"hello".to_vec()),
            ("text/html".into(), b"<b>hello</b>".to_vec()),
        ]);
        assert!(s.ingest(&d, "test").unwrap());
        assert!(!s.ingest(&d, "test").unwrap());
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.snapshot(&s.items[0]).unwrap(), d);
        let root = s.root.clone();
        drop(s);
        let s = Store::open(root).unwrap();
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.snapshot(&s.items[0]).unwrap(), d);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn pin_cleanup_privacy_and_delete() {
        let mut s = store();
        let d = Snapshot::from([("text/plain".into(), b"safe".to_vec())]);
        s.ingest(&d, "test").unwrap();
        let id = s.items[0].id.clone();
        s.pin(&id).unwrap();
        s.clear(false).unwrap();
        assert_eq!(s.items.len(), 1);
        s.settings.exclude_content = "secret".into();
        assert!(!s
            .ingest(
                &Snapshot::from([("text/plain".into(), b"a secret".to_vec())]),
                "test"
            )
            .unwrap());
        s.settings.paused = true;
        assert!(!s.ingest(&d, "test").unwrap());
        s.clear(true).unwrap();
        assert!(s.items.is_empty());
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn uri_and_url_classification() {
        let d = Snapshot::from([(
            "text/uri-list".into(),
            b"file:///home/user/My%20File.pdf\r\n".to_vec(),
        )]);
        assert_eq!(classify(&d, &text_of(&d)), "Files");
        assert_eq!(file_title(&text_of(&d)), "My File.pdf");
        assert_eq!(classify(&Snapshot::new(), "https://example.com"), "Links");
    }
    #[test]
    fn exclusions_scan_all_formats_and_password_hints() {
        let mut s = store();
        s.settings.exclude_content = "secret".into();
        let formats = Snapshot::from([
            ("text/plain".into(), b"innocent preview".to_vec()),
            (
                "text/html".into(),
                b"<span data-token='SECRET'>innocent preview</span>".to_vec(),
            ),
        ]);
        assert!(!s.ingest(&formats, "editor").unwrap());
        s.settings.exclude_content.clear();
        let mut hinted = formats.clone();
        hinted.insert("x-kde-passwordManagerHint".into(), b"secret".to_vec());
        assert!(!s.ingest(&hinted, "editor").unwrap());
        assert!(!s.ingest(&formats, "KeePassXC").unwrap());
        assert!(s.items.is_empty());
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn legacy_text_and_unicode_filenames() {
        for mime in ["STRING", "TEXT", "text/rtf", "application/rtf"] {
            let data = Snapshot::from([(mime.into(), b"searchable text".to_vec())]);
            assert_eq!(text_of(&data), "searchable text");
        }
        assert_eq!(file_title("file:///tmp/caf%C3%A9.pdf"), "café.pdf");
        assert_eq!(file_title("file:///tmp/%E6%96%87%E4%BB%B6.txt"), "文件.txt");
        assert_eq!(file_title("file:///tmp/100%XX.txt"), "100%XX.txt");
    }
    #[test]
    fn limits_retention_and_settings_survive_restart() {
        let mut s = store();
        s.settings.limit = 10;
        s.settings.shortcut = "Ctrl+Alt+V".into();
        s.save_settings().unwrap();
        for n in 0..12 {
            s.ingest(
                &Snapshot::from([("text/plain".into(), format!("clip {n}").into_bytes())]),
                "test",
            )
            .unwrap();
        }
        assert_eq!(s.items.len(), 10);
        let pinned = s.items[0].id.clone();
        s.pin(&pinned).unwrap();
        for item in &mut s.items {
            item.time = now() - 31 * 86400;
        }
        s.cleanup().unwrap();
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].id, pinned);
        let root = s.root.clone();
        drop(s);
        let s = Store::open(root).unwrap();
        assert_eq!(s.settings.shortcut, "Ctrl+Alt+V");
        assert_eq!(s.settings.limit, 10);
        assert_eq!(s.items.len(), 1);
        assert!(s.items[0].pinned);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn image_thumbnail_preserves_original_payload() {
        let mut s = store();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(32, 16)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let data = Snapshot::from([("image/png".into(), png.into_inner())]);
        assert!(s.ingest(&data, "screenshot").unwrap());
        let item = &s.items[0];
        assert_eq!(item.kind, "Images");
        let thumb = image::open(s.root.join(item.thumbnail.as_ref().unwrap())).unwrap();
        assert!(thumb.width() <= 480 && thumb.height() <= 240);
        assert_eq!(s.snapshot(item).unwrap(), data);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn oversize_items_are_not_partially_saved() {
        let mut s = store();
        s.settings.max_mb = 1;
        let data = Snapshot::from([("text/plain".into(), vec![b'x'; 1024 * 1024 + 1])]);
        assert!(s.ingest(&data, "test").is_err());
        assert!(s.items.is_empty());
        assert_eq!(fs::read_dir(s.root.join("items")).unwrap().count(), 0);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn legacy_migration_keeps_text_images_pins_and_settings() {
        let mut s = store();
        let text = Snapshot::from([("text/plain".into(), b"legacy saved text".to_vec())]);
        s.ingest(&text, "test").unwrap();
        let mut item = s.items[0].clone();
        item.pinned = true;
        let mut legacy = serde_json::to_value(&item).unwrap();
        legacy.as_object_mut().unwrap().remove("order");
        s.db.run(&format!(
            "UPDATE history SET json={}",
            quote(&legacy.to_string())
        ))
        .unwrap();
        s.db.run("INSERT OR REPLACE INTO settings VALUES(1,'{\"limit\":42,\"days\":0,\"light\":true,\"paused\":true}')").unwrap();
        let root = s.root.clone();
        drop(s);
        let s = Store::open(root).unwrap();
        assert_eq!(s.items.len(), 1);
        assert!(s.items[0].pinned);
        assert_eq!(s.snapshot(&s.items[0]).unwrap(), text);
        assert!(s.settings.light);
        assert!(s.settings.paused);
        assert_eq!(s.settings.limit, 42);
        assert!(!s.settings.age_enabled);
        assert_eq!(s.settings.days, 30);
        assert_eq!(s.db.rows("PRAGMA user_version").unwrap(), vec!["1"]);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn reuse_moves_to_top_without_duplicate_and_keeps_pin() {
        let mut s = store();
        for text in ["one", "two", "three"] {
            s.ingest(
                &Snapshot::from([("text/plain".into(), text.as_bytes().to_vec())]),
                "test",
            )
            .unwrap();
        }
        let id = s.items[2].id.clone();
        s.pin(&id).unwrap();
        s.touch(&id).unwrap();
        assert_eq!(s.items[0].id, id);
        assert!(s.items[0].pinned);
        assert_eq!(s.items.len(), 3);
        let root = s.root.clone();
        drop(s);
        let s = Store::open(root).unwrap();
        assert_eq!(s.items[0].id, id);
        assert!(s.items[0].pinned);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn edit_is_separate_unpinned_and_unchanged_is_noop() {
        let mut s = store();
        s.ingest(
            &Snapshot::from([("text/plain".into(), b"original\nsecond line".to_vec())]),
            "test",
        )
        .unwrap();
        let id = s.items[0].id.clone();
        s.pin(&id).unwrap();
        let original = s.items[0].clone();
        assert!(s
            .edit_new(&original, "original\nsecond line")
            .unwrap()
            .is_none());
        let edit = s
            .edit_new(&original, "edited\nsecond line")
            .unwrap()
            .unwrap();
        assert_ne!(edit.id, id);
        assert!(!edit.pinned);
        assert_eq!(s.items[0].id, edit.id);
        assert_eq!(
            serde_json::to_string(s.items.iter().find(|i| i.id == id).unwrap()).unwrap(),
            serde_json::to_string(&original).unwrap()
        );
        assert_eq!(s.items.len(), 2);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn independent_cleanup_rules_and_pins() {
        for (age, count, expected) in [
            (false, false, 6),
            (true, false, 4),
            (false, true, 3),
            (true, true, 3),
        ] {
            let mut s = store();
            s.settings.age_enabled = false;
            s.settings.count_enabled = false;
            for n in 0..6 {
                s.ingest(
                    &Snapshot::from([("text/plain".into(), format!("clip {n}").into_bytes())]),
                    "test",
                )
                .unwrap();
            }
            let id = s.items[5].id.clone();
            s.pin(&id).unwrap();
            for n in [0, 4] {
                s.items[n].time = now() - 40 * 86400;
            }
            s.settings.limit = 2;
            s.settings.days = 30;
            s.settings.age_enabled = age;
            s.settings.count_enabled = count;
            s.cleanup().unwrap();
            assert_eq!(s.items.len(), expected, "age={age}, count={count}");
            assert!(s.items.iter().any(|i| i.id == id && i.pinned));
            s.clear(false).unwrap();
            assert_eq!(s.items.len(), 1);
            fs::remove_dir_all(&s.root).unwrap();
        }
    }
    #[test]
    fn referenced_image_payload_is_not_removed_with_another_clip() {
        let mut s = store();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let data = Snapshot::from([("image/png".into(), png.into_inner())]);
        s.ingest(&data, "test").unwrap();
        let original = s.items[0].clone();
        let mut shared = original.clone();
        shared.id = "shared-image".into();
        s.save_item(&shared).unwrap();
        s.items.push(shared);
        s.delete(&original.id).unwrap();
        assert_eq!(s.snapshot(&s.items[0]).unwrap(), data);
        let root = s.root.clone();
        drop(s);
        let mut s = Store::open(root).unwrap();
        assert_eq!(s.snapshot(&s.items[0]).unwrap(), data);
        s.clear(false).unwrap();
        assert!(!s.root.join("items").join(original.id).exists());
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn corrupt_history_fails_closed_without_erasing_payloads() {
        let s = store();
        let root = s.root.clone();
        fs::create_dir_all(root.join("items/precious")).unwrap();
        fs::write(root.join("items/precious/0.bin"), b"preserve").unwrap();
        s.db.run("INSERT INTO history VALUES('broken','not json')")
            .unwrap();
        drop(s);
        assert!(Store::open(root.clone()).is_err());
        assert!(root.join("items/precious/0.bin").is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn all_new_settings_persist_and_cleanup_preview_does_not_mutate() {
        let mut s = store();
        s.settings.age_enabled = false;
        s.settings.count_enabled = false;
        s.settings.hide_after_copy = false;
        s.settings.skip_consecutive = false;
        s.settings.compact = false;
        s.settings.always_on_top = true;
        s.settings.window_size = [900., 700.];
        s.settings.window_position = Some([40., 60.]);
        s.save_settings().unwrap();
        for n in 0..3 {
            s.ingest(
                &Snapshot::from([("text/plain".into(), format!("clip {n}").into_bytes())]),
                "test",
            )
            .unwrap();
        }
        let mut candidate = s.settings.clone();
        candidate.count_enabled = true;
        candidate.limit = 1;
        assert_eq!(s.cleanup_ids(&candidate).len(), 2);
        assert_eq!(s.items.len(), 3);
        let root = s.root.clone();
        drop(s);
        let s = Store::open(root).unwrap();
        assert!(!s.settings.hide_after_copy);
        assert!(!s.settings.skip_consecutive);
        assert!(!s.settings.compact);
        assert!(s.settings.always_on_top);
        assert_eq!(s.settings.window_size, [900., 700.]);
        assert_eq!(s.settings.window_position, Some([40., 60.]));
        assert_eq!(s.items.len(), 3);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn transaction_rolls_back_on_failure() {
        let s = store();
        assert!(s
            .db
            .transaction("INSERT INTO settings VALUES(1,'{}'); INSERT INTO nonexistent VALUES(1);")
            .is_err());
        assert!(s.db.rows("SELECT json FROM settings").unwrap().is_empty());
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn upgrade_from_v110_preserves_all_old_preferences_and_original_payloads() {
        let mut s = store();
        s.settings.shortcut = "Ctrl+Alt+V".into();
        s.settings.light = true;
        s.settings.compact = false;
        s.settings.startup = true;
        s.settings.hide_after_copy = true;
        s.settings.exclude_apps = "private-editor".into();
        s.settings.exclude_content = "secret".into();
        s.settings.updates.automatic = true;
        s.settings.age_enabled = false;
        s.settings.count_enabled = false;
        s.settings.window_size = [980., 710.];
        s.settings.window_position = Some([42., 73.]);
        let text = Snapshot::from([
            ("text/plain".into(), b"preserved text".to_vec()),
            ("text/html".into(), b"<b>preserved text</b>".to_vec()),
        ]);
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(37, 19)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let image = Snapshot::from([("image/png".into(), png.into_inner())]);
        for data in [&text, &image] {
            s.ingest(data, "legacy").unwrap();
        }
        s.pin(&s.items[0].id.clone()).unwrap();
        let metadata = serde_json::to_value(&s.items).unwrap();
        let mut legacy = serde_json::to_value(&s.settings).unwrap();
        for key in [
            "start_hidden",
            "system_title_bar",
            "app_font",
            "confirm_before_delete",
            "changelog_dismissed_version",
        ] {
            legacy.as_object_mut().unwrap().remove(key);
        }
        s.db.run(&format!(
            "INSERT OR REPLACE INTO settings VALUES(1,{})",
            quote(&legacy.to_string())
        ))
        .unwrap();
        let root = s.root.clone();
        drop(s);
        let mut upgraded = Store::open(root.clone()).unwrap();
        assert_eq!(serde_json::to_value(&upgraded.items).unwrap(), metadata);
        assert_eq!(
            upgraded
                .snapshot(upgraded.items.iter().find(|i| i.kind == "Text").unwrap())
                .unwrap(),
            text
        );
        assert_eq!(
            upgraded
                .snapshot(upgraded.items.iter().find(|i| i.kind == "Images").unwrap())
                .unwrap(),
            image
        );
        let actual = serde_json::to_value(&upgraded.settings).unwrap();
        for (key, value) in legacy.as_object().unwrap() {
            assert_eq!(&actual[key], value, "existing setting {key} changed");
        }
        assert!(upgraded.settings.confirm_before_delete);
        assert!(upgraded.settings.system_title_bar);
        assert!(upgraded.settings.app_font.is_empty());
        assert!(upgraded.settings.changelog_dismissed_version.is_empty());
        upgraded.settings.app_font = "Saved font family".into();
        upgraded.settings.start_hidden = true;
        upgraded.settings.system_title_bar = false;
        upgraded.settings.confirm_before_delete = false;
        upgraded.settings.changelog_dismissed_version = "1.2.0".into();
        upgraded.save_settings().unwrap();
        drop(upgraded);
        let restarted = Store::open(root.clone()).unwrap();
        assert_eq!(serde_json::to_value(&restarted.items).unwrap(), metadata);
        assert_eq!(restarted.settings.app_font, "Saved font family");
        assert!(restarted.settings.start_hidden);
        assert!(!restarted.settings.system_title_bar);
        assert!(!restarted.settings.confirm_before_delete);
        assert_eq!(restarted.settings.changelog_dismissed_version, "1.2.0");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v200_reuse_preserves_capture_time_identity_and_pin() {
        let mut s = store();
        s.ingest(
            &Snapshot::from([("text/plain".into(), b"Original".to_vec())]),
            "test",
        )
        .unwrap();
        let original = s.items[0].clone();
        s.pin(&original.id).unwrap();
        s.ingest(
            &Snapshot::from([("text/plain".into(), b"Newer".to_vec())]),
            "test",
        )
        .unwrap();
        s.touch(&original.id).unwrap();
        assert_eq!(s.items[0].id, original.id);
        assert!(s.items[0].pinned);
        assert_eq!(s.items[0].time, original.time);
        assert_eq!(s.items[0].captured_at, original.captured_at);
        assert!(s.items[0].last_used.is_some());
        assert_eq!(s.items.len(), 2);
        let path = s.root.clone();
        drop(s);
        let reopened = Store::open(path.clone()).unwrap();
        assert_eq!(reopened.items[0].captured_at, original.captured_at);
        assert!(reopened.items[0].last_used.is_some());
        drop(reopened);
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn v200_boot_cleanup_distinguishes_process_restart_and_unknown_boot() {
        let mut s = store();
        s.settings.age_enabled = false;
        s.settings.count_enabled = false;
        s.settings.clear_on_reboot = true;
        s.ingest(
            &Snapshot::from([("text/plain".into(), b"Keep pinned".to_vec())]),
            "test",
        )
        .unwrap();
        let pin = s.items[0].id.clone();
        s.pin(&pin).unwrap();
        s.ingest(
            &Snapshot::from([("text/plain".into(), b"Unpinned".to_vec())]),
            "test",
        )
        .unwrap();
        s.db.run("DELETE FROM runtime_state").unwrap();
        s.apply_boot_id("first-known-boot").unwrap();
        assert_eq!(s.items.len(), 2);
        s.apply_boot_id("first-known-boot").unwrap();
        assert_eq!(s.items.len(), 2);
        s.apply_boot_id("").unwrap();
        assert_eq!(s.items.len(), 2);
        s.apply_boot_id("actual-next-boot").unwrap();
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0].id, pin);
        s.apply_boot_id("actual-next-boot").unwrap();
        assert_eq!(s.items.len(), 1);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn v200_legacy_times_are_not_invented_and_deep_search_survives_summary_truncation() {
        let mut s = store();
        s.settings.age_enabled = false;
        let text = format!("{} UniqueNeedle_% end", "x".repeat(18000));
        s.ingest(
            &Snapshot::from([("text/plain".into(), text.as_bytes().to_vec())]),
            "test",
        )
        .unwrap();
        let mut clip = s.items[0].clone();
        assert!(clip.preview.len() <= 512);
        assert!(s.search_ids("uniqueneedle_%").unwrap().contains(&clip.id));
        clip.captured_at = None;
        clip.last_used = None;
        let saved_time = clip.time;
        s.save_item(&clip).unwrap();
        s.items[0] = clip.clone();
        s.touch(&clip.id).unwrap();
        assert_eq!(s.items[0].time, saved_time);
        assert_eq!(s.items[0].captured_at, None);
        assert!(s.items[0].copied_label().contains("unavailable"));
        assert_eq!(s.full_text(&clip).unwrap(), text);
        fs::remove_dir_all(&s.root).unwrap();
    }
    #[test]
    fn v200_independent_geometry_and_new_preferences_survive_restart() {
        let mut s = store();
        s.settings.compact = true;
        s.settings.set_geometry(Geometry {
            size: [450., 550.],
            maximized: false,
        });
        s.settings.compact = false;
        s.settings.set_geometry(Geometry {
            size: [850., 650.],
            maximized: true,
        });
        s.settings.theme = "system".into();
        s.settings.font_size = 20.;
        s.settings.divider = 0.38;
        s.settings.clear_on_reboot = true;
        s.save_settings().unwrap();
        let root = s.root.clone();
        drop(s);
        let mut s = Store::open(root.clone()).unwrap();
        assert_eq!(s.settings.geometry().size, [850., 650.]);
        assert!(s.settings.geometry().maximized);
        s.settings.compact = true;
        assert_eq!(s.settings.geometry().size, [450., 550.]);
        assert!(!s.settings.geometry().maximized);
        assert_eq!(s.settings.theme, "system");
        assert_eq!(s.settings.font_size, 20.);
        assert_eq!(s.settings.divider, 0.38);
        assert!(s.settings.clear_on_reboot);
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }
}
