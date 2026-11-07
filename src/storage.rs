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
    pub pinned: bool,
    pub formats: Vec<Format>,
    pub thumbnail: Option<String>,
    pub source: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub limit: usize,
    pub days: i64,
    pub light: bool,
    pub paused: bool,
    pub shortcut: String,
    pub exclude_content: String,
    pub exclude_apps: String,
    pub max_mb: usize,
    pub startup: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            limit: 500,
            days: 30,
            light: false,
            paused: false,
            shortcut: "Super+V".into(),
            exclude_content: String::new(),
            exclude_apps: "keepass\n1password\nbitwarden".into(),
            max_mb: 32,
            startup: false,
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
            out.run("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS history (id TEXT PRIMARY KEY, json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY, json TEXT NOT NULL);")?;
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
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("items")).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let db = Database::open(&root.join("history.sqlite3"))?;
        let items = db
            .rows("SELECT json FROM history")?
            .into_iter()
            .filter_map(|v| serde_json::from_str(&v).ok())
            .collect();
        let settings = db
            .rows("SELECT json FROM settings WHERE id=1")?
            .first()
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default();
        let mut s = Self {
            root,
            db,
            items,
            settings,
        };
        s.cleanup()?;
        s.sort();
        s.collect_orphans();
        Ok(s)
    }
    pub fn sort(&mut self) {
        self.items.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.time.cmp(&a.time))
                .then(b.id.cmp(&a.id))
        });
    }
    fn save_item(&self, item: &Item) -> Result<(), String> {
        self.db.run(&format!(
            "INSERT OR REPLACE INTO history VALUES ({},{});",
            quote(&item.id),
            quote(&serde_json::to_string(item).unwrap())
        ))
    }
    pub fn save_settings(&mut self) -> Result<(), String> {
        self.settings.limit = self.settings.limit.clamp(10, 10000);
        self.settings.days = self.settings.days.clamp(0, 3650);
        self.settings.max_mb = self.settings.max_mb.clamp(1, 128);
        self.db.run(&format!(
            "INSERT OR REPLACE INTO settings VALUES (1,{});",
            quote(&serde_json::to_string(&self.settings).unwrap())
        ))?;
        self.cleanup()
    }
    pub fn ingest(&mut self, data: &Snapshot, source: &str) -> Result<bool, String> {
        if data.is_empty() || self.settings.paused {
            return Ok(false);
        }
        let bytes: usize = data.values().map(Vec::len).sum();
        if bytes > self.settings.max_mb * 1024 * 1024 {
            return Ok(false);
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
            let mut it = self.items[i].clone();
            it.time = now();
            self.save_item(&it)?;
            self.items[i] = it;
            self.sort();
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
        let item = Item {
            id,
            hash,
            kind,
            title,
            preview: preview.chars().take(200000).collect(),
            time: now(),
            pinned: false,
            formats,
            thumbnail,
            source: source.into(),
        };
        self.save_item(&item)?;
        self.items.push(item);
        self.sort();
        self.cleanup()?;
        Ok(true)
    }
    pub fn snapshot(&self, item: &Item) -> Result<Snapshot, String> {
        let mut d = Snapshot::new();
        for f in &item.formats {
            d.insert(
                f.mime.clone(),
                fs::read(self.root.join(&f.file)).map_err(|e| e.to_string())?,
            );
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
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.db
            .run(&format!("DELETE FROM history WHERE id={}", quote(id)))?;
        self.items.retain(|i| i.id != id);
        let _ = fs::remove_dir_all(self.root.join("items").join(id));
        Ok(())
    }
    pub fn clear(&mut self, include_pinned: bool) -> Result<(), String> {
        let ids: Vec<_> = self
            .items
            .iter()
            .filter(|i| include_pinned || !i.pinned)
            .map(|i| i.id.clone())
            .collect();
        for id in ids {
            self.delete(&id)?;
        }
        Ok(())
    }
    pub fn cleanup(&mut self) -> Result<(), String> {
        self.sort();
        let mut unpinned = 0;
        let ids: Vec<_> = self
            .items
            .iter()
            .filter(|i| {
                if i.pinned {
                    false
                } else {
                    unpinned += 1;
                    unpinned > self.settings.limit
                        || (self.settings.days > 0 && i.time < now() - self.settings.days * 86400)
                }
            })
            .map(|i| i.id.clone())
            .collect();
        for id in ids {
            self.delete(&id)?;
        }
        Ok(())
    }
    fn collect_orphans(&self) {
        if let Ok(dirs) = fs::read_dir(self.root.join("items")) {
            for d in dirs.flatten() {
                if !self
                    .items
                    .iter()
                    .any(|i| d.file_name().to_str() == Some(&i.id))
                {
                    let _ = fs::remove_dir_all(d.path());
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
        assert!(!s.ingest(&data, "test").unwrap());
        assert!(s.items.is_empty());
        assert_eq!(fs::read_dir(s.root.join("items")).unwrap().count(), 0);
        fs::remove_dir_all(&s.root).unwrap();
    }
}
