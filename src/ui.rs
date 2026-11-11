use crate::{
    clipboard::{Command, Notice, Service, Shared},
    icons,
    storage::{now, Item, Settings},
};
use eframe::egui::{self, *};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};
const ACCENT: Color32 = Color32::from_rgb(190, 165, 239);
fn muted(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(161, 160, 181)
    } else {
        Color32::from_rgb(99, 94, 113)
    }
}

type ListCache = ((u64, usize, String, String), std::sync::Arc<Vec<Item>>);

pub struct VaultApp {
    store: Shared,
    service: Service,
    ipc: Receiver<String>,
    query: String,
    category: String,
    selected: Option<String>,
    pending_activation: Option<String>,
    settings_open: bool,
    settings_tab: usize,
    preview: Option<Item>,
    editor: Option<(Item, String)>,
    confirm_clear: bool,
    confirm_delete: Option<Item>,
    pending_settings: Option<Settings>,
    shot: Option<PathBuf>,
    frames: usize,
    shot_requested: bool,
    hidden: bool,
    minimized: bool,
    quitting: bool,
    status: String,
    error: Option<String>,
    textures: HashMap<String, TextureHandle>,
    logo: TextureHandle,
    focus_search: bool,
    scroll_to_selected: bool,
    shortcut_recording: bool,
    local_recorder: crate::shortcut::Recorder,
    shortcut_pending: bool,
    shortcut_display: String,
    active_shortcut: Option<String>,
    shortcut_feedback: String,
    checker: crate::updates::Checker,
    update_outcome: Option<crate::updates::Outcome>,
    toast: Option<(String, Instant)>,
    last_cleanup: Instant,
    automatic_activation: bool,
    desktop_shortcut: Option<String>,
    restart_updates: Option<Receiver<String>>,
    replacement: Option<String>,
    image_result: Option<Receiver<(String, Result<ColorImage, String>)>>,
    image_loading: Option<String>,
    image_errors: HashMap<String, String>,
    text_cache: Option<(String, Result<String, String>)>,
    list_cache: Option<ListCache>,
    last_geometry: Instant,
    installed_fonts: Vec<crate::fonts::InstalledFont>,
    applied_font: String,
    font_available: bool,
    changelog_open: bool,
    changelog_considered: bool,
}
impl VaultApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        store: Shared,
        service: Service,
        ipc: Receiver<String>,
        shot: Option<PathBuf>,
        hidden: bool,
    ) -> Self {
        let wake = |receiver: Receiver<_>| {
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(message) = receiver.recv() {
                    if tx.send(message).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
            rx
        };
        let service = Service {
            tx: service.tx,
            rx: wake(service.rx),
        };
        let ipc = {
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(message) = ipc.recv() {
                    if tx.send(message).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
            rx
        };
        let mut app = Self::create(&cc.egui_ctx, store, service, ipc, shot, hidden);
        let startup = {
            let s = app.store.lock().unwrap();
            if s.settings.startup {
                Some(s.root.clone())
            } else {
                None
            }
        };
        if app.shot.is_none() {
            app.restart_updates = Some(crate::restart::watch());
            if let Some(root) = startup {
                if let Err(e) = set_startup(true, &root) {
                    app.error = Some(format!("Login startup could not be updated: {e}"));
                }
            }
        }
        if app.shot.is_some() {
            match std::env::var("CLIPVAULT_CAPTURE_PAGE").as_deref() {
                Ok("about") => {
                    app.settings_open = true;
                    app.settings_tab = 4;
                }
                Ok("shortcuts") => {
                    app.settings_open = true;
                    app.settings_tab = 2;
                }
                _ => {}
            }
        }
        app
    }
    fn create(
        ctx: &Context,
        store: Shared,
        service: Service,
        ipc: Receiver<String>,
        shot: Option<PathBuf>,
        hidden: bool,
    ) -> Self {
        let installed_fonts = crate::fonts::installed();
        let applied_font = store.lock().unwrap().settings.app_font.clone();
        let font_available = crate::fonts::apply(ctx, &applied_font, &installed_fonts);
        let mut style = (*ctx.style()).clone();
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(13.));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.));
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.button_padding = vec2(11., 8.);
        style.spacing.item_spacing = vec2(8., 9.);
        ctx.set_style(style);
        let image =
            image::load_from_memory(include_bytes!("../assets/icons/128x128/clipvault.png"))
                .expect("bundled logo")
                .into_rgba8();
        let logo = ctx.load_texture(
            "ClipVault logo",
            ColorImage::from_rgba_unmultiplied([128, 128], image.as_raw()),
            TextureOptions::LINEAR,
        );

        let update_outcome = store
            .lock()
            .unwrap()
            .settings
            .updates
            .cached_outcome
            .clone();
        Self {
            store,
            service,
            ipc,
            query: String::new(),
            category: "All".into(),
            selected: None,
            pending_activation: None,
            settings_open: false,
            settings_tab: 0,
            preview: None,
            editor: None,
            confirm_clear: false,
            confirm_delete: None,
            pending_settings: None,
            shot,
            frames: 0,
            shot_requested: false,
            hidden,
            minimized: false,
            quitting: false,
            status: "Connecting to the desktop clipboard…".into(),
            error: None,
            textures: HashMap::new(),
            logo,
            focus_search: false,
            scroll_to_selected: false,
            shortcut_recording: false,
            local_recorder: Default::default(),
            shortcut_pending: false,
            shortcut_display: String::new(),
            active_shortcut: None,
            shortcut_feedback: if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                "Desktop shortcut required on Wayland. Expand setup help below.".into()
            } else {
                "Checking global shortcut registration…".into()
            },
            checker: Default::default(),
            update_outcome,
            toast: None,
            last_cleanup: Instant::now(),
            automatic_activation: false,
            desktop_shortcut: None,
            restart_updates: None,
            replacement: None,
            image_result: None,
            image_loading: None,
            image_errors: HashMap::new(),
            text_cache: None,
            list_cache: None,
            last_geometry: Instant::now(),
            installed_fonts,
            applied_font,
            font_available,
            changelog_open: false,
            changelog_considered: false,
        }
    }
    fn handle_notice(&mut self, ctx: &Context, notice: Notice) {
        match notice {
            Notice::PasteFeedback(message) => {
                if self.hidden && self.restart_updates.is_some() && message.starts_with("Copied") {
                    let _ = std::process::Command::new("notify-send")
                        .args([
                            "--app-name=ClipVault",
                            "--icon=clipvault",
                            "ClipVault",
                            &message,
                        ])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
                self.toast = Some((message, Instant::now()));
            }
            Notice::ShortcutConfigured(shortcut) => {
                self.active_shortcut = None;
                self.desktop_shortcut = Some(shortcut);
                self.shortcut_recording = false;
                self.shortcut_pending = false;
                self.shortcut_feedback="Desktop binding configured and read back. Press it to verify; desktop registration is not independently confirmed.".into();
            }
            Notice::Open => {
                if !self.shortcut_recording && !self.shortcut_pending {
                    self.toggle(ctx);
                }
            }
            Notice::ShortcutInactive => {
                self.active_shortcut = None;
                self.desktop_shortcut = None;
                self.shortcut_feedback =
                    "No confirmed binding. Choose a shortcut or use desktop Keyboard settings."
                        .into();
            }
            Notice::ShortcutActive(shortcut) => {
                self.desktop_shortcut = None;
                self.active_shortcut = Some(shortcut);
                if self.shortcut_feedback.starts_with("Checking") {
                    self.shortcut_feedback =
                        "Shortcut registered and ready. Click to record a new combination.".into();
                }
            }
            Notice::RecordingReady => {
                self.shortcut_feedback = "Recording · Press your shortcut. Escape cancels.".into();
                if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                    self.local_recorder.begin();
                    self.local_recorder.ready = true;
                }
            }
            Notice::ShortcutCandidate(shortcut) => {
                self.shortcut_display = shortcut;
            }
            Notice::RecordingCancelled => {
                self.shortcut_recording = false;
                self.shortcut_pending = false;
                self.shortcut_feedback = "Cancelled · previous shortcut kept.".into();
            }
            Notice::ShortcutFailed(error) => {
                self.shortcut_recording = false;
                self.shortcut_pending = false;
                // Only a subsequent registration acknowledgement may mark a binding active.
                if self.active_shortcut.is_none() && self.desktop_shortcut.is_none() {
                    self.error = Some(error.clone());
                }
                self.shortcut_feedback = error;
            }
            Notice::ShortcutApplied(shortcut) => {
                self.active_shortcut = Some(shortcut);
                self.shortcut_recording = false;
                self.shortcut_pending = false;
                self.shortcut_feedback = "Saved · shortcut registered and ready.".into();
            }
            Notice::Activated(id) if self.pending_activation.as_deref() == Some(&id) => {
                self.pending_activation = None;
                self.selected = Some(id);
                self.error = None;
                self.toast = Some(("Copied · Paste with Ctrl + V.".into(), Instant::now()));
                let hide = self.store.lock().unwrap().settings.hide_after_copy;
                if self.automatic_activation {
                    self.automatic_activation = false;
                    self.hide(ctx);
                    let _ = self.service.tx.send(Command::Paste);
                } else if hide {
                    self.hide(ctx);
                }
            }
            Notice::Activated(_) | Notice::Changed => {}
            Notice::ActivationFailed(id, error) => {
                if self.pending_activation.as_deref() == Some(&id) {
                    self.pending_activation = None;
                    self.automatic_activation = false;
                    self.toast = None;
                    self.error = Some(error);
                    if self.hidden {
                        self.show(ctx);
                    }
                }
            }
            Notice::Status(s) => self.status = s,
            Notice::Unavailable(e) => {
                self.pending_activation = None;
                self.automatic_activation = false;
                self.active_shortcut = None;
                self.shortcut_recording = false;
                self.shortcut_pending = false;
                self.shortcut_feedback = format!("Keyboard service unavailable. {e}");
                self.error = Some(e);
            }
            Notice::Error(e) => {
                self.automatic_activation = false;
                if self.pending_activation.take().is_some() && self.hidden {
                    self.show(ctx);
                }
                self.error = Some(e);
            }
        }
    }
    fn handle_ipc(&mut self, ctx: &Context, message: &str) {
        if self.shortcut_recording || self.shortcut_pending {
            return;
        }
        if message == "toggle" {
            let _ = self.service.tx.send(Command::Toggle);
        } else if message == "show" {
            self.show(ctx);
        }
    }
    fn toggle(&mut self, ctx: &Context) {
        if self.hidden || self.minimized {
            self.show(ctx);
        } else {
            self.hide(ctx);
        }
    }
    fn cancel_recording(&mut self) {
        if self.shortcut_recording {
            self.shortcut_recording = false;
            self.shortcut_pending = true;
            if self
                .service
                .tx
                .send(Command::RecordShortcut(false))
                .is_err()
            {
                self.shortcut_pending = false;
                self.shortcut_feedback = "Keyboard service unavailable. Restart ClipVault.".into();
                self.active_shortcut = None;
            }
        }
    }
    fn hide(&mut self, ctx: &Context) {
        self.cancel_recording();
        if self.shot.is_none() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.hidden = true;
            self.textures.clear();
            self.text_cache = None;
            self.image_result = None;
            self.image_loading = None;
        }
    }
    fn show(&mut self, ctx: &Context) {
        let _ = self.service.tx.send(Command::RememberTarget);
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.hidden = false;
        self.minimized = false;
        if let Some((message, time)) = &mut self.toast {
            if message.starts_with("Copied") {
                *time = Instant::now();
            }
        }
        self.focus_search = true;
    }
    fn restart_safely(&mut self, ctx: &Context) {
        if self.editor.is_some() || self.pending_activation.is_some() {
            return;
        }
        if self.mutate(|store| store.flush()) {
            crate::restart::request();
            self.quit(ctx);
        } else {
            self.replacement = None;
            self.error=Some("Update is installed, but storage could not be flushed. Restart manually after resolving the storage error.".into());
        }
    }
    fn quit(&mut self, ctx: &Context) {
        self.quitting = true;
        let _ = self.service.tx.send(Command::Stop);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    fn mutate(&mut self, f: impl FnOnce(&mut crate::storage::Store) -> Result<(), String>) -> bool {
        // Scope the database guard before rendering, showing an error, or dispatching.
        let result = {
            let mut store = self.store.lock().unwrap();
            f(&mut store)
        };
        match result {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
    fn activate_and_paste(&mut self, item: &Item) {
        if self.pending_activation.is_none() {
            self.activate(item);
            self.automatic_activation = self.pending_activation.is_some();
        }
    }
    fn activate(&mut self, item: &Item) {
        // One outstanding request keeps delayed acknowledgements from hiding
        // the window after a newer copy has failed. Use saved identity, not row index.
        if self.pending_activation.is_some() {
            return;
        }
        self.automatic_activation = false;
        self.toast = None;
        let result = {
            let store = self.store.lock().unwrap();
            store
                .items
                .iter()
                .find(|saved| saved.id == item.id)
                .ok_or_else(|| "This clip is no longer in history".to_owned())
                .and_then(|saved| {
                    if saved.formats.is_empty() {
                        return Err("This saved clip has no clipboard formats".into());
                    }
                    let folder = std::fs::canonicalize(store.root.join("items"))
                        .map_err(|e| e.to_string())?;
                    for format in &saved.formats {
                        let path = std::fs::canonicalize(store.root.join(&format.file))
                            .map_err(|e| e.to_string())?;
                        if !path.starts_with(&folder) || !path.is_file() {
                            return Err("Saved payload unavailable".into());
                        }
                    }
                    Ok(())
                })
        };
        match result {
            Ok(()) => {
                if self
                    .service
                    .tx
                    .send(Command::RestoreItem(item.id.clone()))
                    .is_err()
                {
                    self.error=Some("Clipboard service is unavailable. Restart ClipVault to reconnect; your history is saved.".into());
                } else {
                    self.selected = Some(item.id.clone());
                    self.pending_activation = Some(item.id.clone());
                    self.error = None;
                }
            }
            Err(e) => {
                self.error = Some(format!(
                    "Could not read the saved clip: {e}. The window will stay open."
                ))
            }
        }
    }
    fn pin(&mut self, item: &Item) {
        self.mutate(|s| s.pin(&item.id));
    }
    fn delete(&mut self, item: &Item) {
        // Detail views can outlive a pin change; use authoritative saved state.
        let current = {
            self.store
                .lock()
                .unwrap()
                .items
                .iter()
                .find(|i| i.id == item.id)
                .cloned()
        };
        let Some(item) = current else {
            self.preview = None;
            return;
        };
        if self.store.lock().unwrap().settings.confirm_before_delete {
            self.confirm_delete = Some(item.clone());
        } else {
            self.confirm_delete = None;
            self.delete_confirmed(&item);
        }
    }
    fn delete_confirmed(&mut self, item: &Item) {
        if self.mutate(|s| s.delete(&item.id)) {
            self.textures.remove(&item.id);
            self.textures.remove(&format!("full-{}", item.id));
            self.preview = None;
            if self.selected.as_deref() == Some(&item.id) {
                self.selected = None;
            }
            self.confirm_delete = None;
        }
    }
    fn edit(&mut self, item: &Item) {
        let result = { self.store.lock().unwrap().full_text(item) };
        match result {
            Ok(text) => {
                self.editor = Some((item.clone(), text));
                self.preview = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn texture(&mut self, ctx: &Context, item: &Item) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(&item.id) {
            return Some(t.clone());
        }
        let path = {
            self.store
                .lock()
                .unwrap()
                .root
                .join(item.thumbnail.as_ref()?)
        };
        let im = image::open(path).ok()?.into_rgba8();
        let tex = ctx.load_texture(
            &item.id,
            ColorImage::from_rgba_unmultiplied(
                [im.width() as usize, im.height() as usize],
                im.as_raw(),
            ),
            TextureOptions::LINEAR,
        );
        if self.textures.len() >= 16 {
            self.textures.clear();
        }
        self.textures.insert(item.id.clone(), tex.clone());
        Some(tex)
    }
    fn full_image(&mut self, ctx: &Context, item: &Item) -> Option<TextureHandle> {
        let key = format!("full-{}", item.id);
        if let Some(t) = self.textures.get(&key) {
            return Some(t.clone());
        }
        let root = self.store.lock().unwrap().root.clone();
        let path = root.join(
            &item
                .formats
                .iter()
                .find(|f| f.mime.starts_with("image/"))?
                .file,
        );
        if let Some(rx) = &self.image_result {
            if let Ok((id, result)) = rx.try_recv() {
                self.image_result = None;
                self.image_loading = None;
                if let Err(error) = &result {
                    self.image_errors.insert(id.clone(), error.clone());
                }
                if let Ok(image) = result {
                    self.textures.retain(|id, _| !id.starts_with("full-"));
                    let texture = ctx.load_texture(&id, image, TextureOptions::LINEAR);
                    self.textures.insert(id, texture);
                }
            }
        }
        if let Some(texture) = self.textures.get(&key) {
            return Some(texture.clone());
        }
        if self.image_errors.contains_key(&key) {
            return None;
        }
        if self.image_loading.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.image_result = Some(rx);
            self.image_loading = Some(key.clone());
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = (|| {
                    let mut reader = image::ImageReader::open(&path)
                        .map_err(|e| e.to_string())?
                        .with_guessed_format()
                        .map_err(|e| e.to_string())?;
                    let mut limits = image::Limits::default();
                    limits.max_alloc = Some(128 * 1024 * 1024);
                    reader.limits(limits);
                    let image = reader
                        .decode()
                        .or_else(|_| crate::storage::native_image(&path))
                        .map_err(|e| e.to_string())?;
                    let image = if image.width() > 1600 || image.height() > 1200 {
                        image.thumbnail(1600, 1200)
                    } else {
                        image
                    }
                    .into_rgba8();
                    Ok(ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ))
                })();
                let _ = tx.send((key, result));
                ctx.request_repaint();
            });
        }
        None
    }
    fn header(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            ui.image((self.logo.id(), vec2(32., 32.)));
            let title = ui.label(RichText::new("ClipVault").strong().size(19.));
            if title.interact(Sense::drag()).drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if ui.available_width() > 420. {
                ui.label(RichText::new("On this device").color(muted(ui)));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.store.lock().unwrap().settings.system_title_bar {
                    if icons::button(ui, "close", "Hide · Escape", 30., muted(ui)).clicked() {
                        self.hide(ctx);
                    }
                    if icons::button(ui, "maximize", "Maximize / Restore", 24., muted(ui)).clicked()
                    {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(
                            !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                        ));
                    }
                    if icons::button(ui, "minimize", "Minimize", 24., muted(ui)).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                }
                let settings_tip = if self.settings_open {
                    "Back to clipboard history"
                } else {
                    "Settings"
                };
                if icons::button(ui, "settings", settings_tip, 30., muted(ui)).clicked() {
                    self.settings_open = !self.settings_open;
                    self.preview = None;
                    self.editor = None;
                }
                let paused = self.store.lock().unwrap().settings.paused;
                if ui.button(if paused { "Resume" } else { "Pause" }).clicked() {
                    self.mutate(|s| {
                        s.settings.paused = !s.settings.paused;
                        s.save_settings()
                    });
                }
                if ui.available_width() > 95. {
                    ui.label(
                        RichText::new(if self.shot.is_some() {
                            "• Preview"
                        } else if paused {
                            "• Paused"
                        } else if self.status.starts_with("Connecting") || self.error.is_some() {
                            "• Check status"
                        } else {
                            "• Monitoring"
                        })
                        .color(if paused {
                            muted(ui)
                        } else {
                            Color32::from_rgb(132, 204, 183)
                        }),
                    );
                }
            });
        });
    }
    fn search(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let clear_width = if self.query.is_empty() { 0. } else { 32. };
            let field = ui.add_sized(
                [ui.available_width() - clear_width, 30.],
                TextEdit::singleline(&mut self.query)
                    .id(Id::new("search"))
                    .vertical_align(Align::Center)
                    .hint_text("Search clips…"),
            );
            if self.focus_search {
                field.request_focus();
                self.focus_search = false;
            }
            if field.changed() {
                self.selected = None;
            }
            if !self.query.is_empty() {
                let clear = ui.button("×").on_hover_text("Clear search");
                clear.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Clear search"));
                if clear.clicked() {
                    self.query.clear();
                    self.focus_search = true;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for label in ["All", "Text", "Images", "Links", "Files", "Pinned"] {
                if ui
                    .selectable_value(&mut self.category, label.to_owned(), label)
                    .clicked()
                {
                    self.selected = None;
                }
            }
        });
    }
    fn row(&mut self, ui: &mut Ui, ctx: &Context, item: &Item, _compact: bool) {
        let height = 52.;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let response = ui.interact(rect, Id::new(("clip-row", &item.id)), Sense::click());
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &item.title));
        if !ui.is_rect_visible(rect) {
            return;
        }
        let selected = self.selected.as_deref() == Some(&item.id);
        let fill = if selected {
            if ui.visuals().dark_mode {
                Color32::from_rgb(47, 40, 62)
            } else {
                Color32::from_rgb(232, 220, 248)
            }
        } else {
            ui.visuals().panel_fill
        };
        ui.painter().rect_filled(rect, 8, fill);

        if selected || response.has_focus() {
            ui.painter()
                .rect_stroke(rect, 8, Stroke::new(1_f32, ACCENT), StrokeKind::Inside);
        }
        let icon = match item.kind.as_str() {
            "Images" => "image",
            "Files" => "file",
            "Links" => "link",
            _ => "text",
        };
        let tile = Rect::from_min_size(rect.min + vec2(10., 11.), vec2(30., 30.));
        ui.painter()
            .rect_filled(tile, 7, ui.visuals().extreme_bg_color);
        if let Some(texture) = self.texture(ctx, item) {
            let size = texture.size_vec2();
            let scale = (tile.width() / size.x).min(tile.height() / size.y);
            ui.painter().image(
                texture.id(),
                Rect::from_center_size(tile.center(), size * scale),
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        } else {
            icons::draw(ui.painter(), tile.shrink(7.), icon, muted(ui));
        }
        let text_rect = Rect::from_min_max(rect.min + vec2(50., 7.), rect.max - vec2(84., 8.));
        let title_width = text_rect.width().max(20.);
        let content = if matches!(item.kind.as_str(), "Text" | "Links") {
            item.preview.chars().take(240).collect::<String>()
        } else if item.kind == "Images" && item.thumbnail.is_some() {
            String::new()
        } else {
            item.title.clone()
        };
        let mut job = egui::text::LayoutJob::simple(
            content.replace(['\r', '\n', '\t'], " "),
            FontId::proportional(13.),
            ui.visuals().text_color(),
            title_width,
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        ui.painter()
            .galley(text_rect.min, galley, ui.visuals().text_color());
        let timestamp = if let Some(time) = item.last_used {
            format!("Last used {}", crate::storage::relative_time(time))
        } else if item.captured_at.is_none() {
            "Capture time unknown".into()
        } else {
            item.copied_label()
        };
        ui.painter().text(
            rect.min + vec2(50., 32.),
            Align2::LEFT_TOP,
            &timestamp,
            FontId::proportional(11.),
            muted(ui),
        );
        response.clone().on_hover_text(item.exact_times());
        let controls_width = 72.;
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(&item.id)
                .max_rect(Rect::from_min_size(
                    pos2(rect.right() - controls_width - 6., rect.top() + 10.),
                    vec2(controls_width, 32.),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        let mut action = 0;
        if icons::button(
            &mut child,
            if item.pinned { "pin-filled" } else { "pin" },
            if item.pinned { "Unpin" } else { "Pin" },
            26.,
            if item.pinned { ACCENT } else { muted(ui) },
        )
        .clicked()
        {
            action = 1;
        }
        let menu = child.menu_button("···", |ui| {
            if ui.button(copy_label(item)).clicked() {
                action = 2;
                ui.close_menu();
            }
            if ui
                .button(if item.pinned { "Unpin" } else { "Pin" })
                .clicked()
            {
                action = 1;
                ui.close_menu();
            }
            if matches!(item.kind.as_str(), "Text" | "Links") && ui.button("Edit").clicked() {
                action = 4;
                ui.close_menu();
            }
            if ui.button("Delete").clicked() {
                action = 5;
                ui.close_menu();
            }
        });
        match action {
            1 => self.pin(item),
            2 => self.activate(item),
            4 => self.edit(item),
            5 => self.delete(item),
            _ => {}
        }
        let on_control = ctx
            .pointer_interact_pos()
            .is_some_and(|p| p.x >= rect.right() - 80.);
        if response.clicked() && !on_control && action == 0 && !menu.response.clicked() {
            self.activate_and_paste(item);
        }
        if response.hovered() && ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
            self.selected = Some(item.id.clone());
        }
        if selected && ctx.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp)) {
            response.scroll_to_me(Some(Align::Center));
        }
    }
    fn detail(&mut self, ui: &mut Ui, ctx: &Context, item: &Item, back: bool) {
        if back && ui.button("Back to history").clicked() {
            self.preview = None;
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(
                    Button::new(
                        RichText::new(copy_label(item)).color(Color32::from_rgb(30, 24, 42)),
                    )
                    .fill(ACCENT),
                )
                .clicked()
            {
                self.activate(item);
            }
            if ui
                .button(if item.pinned { "Unpin" } else { "Pin" })
                .clicked()
            {
                self.pin(item);
            }
            if matches!(item.kind.as_str(), "Text" | "Links") && ui.button("Edit").clicked() {
                self.edit(item);
            }
            if ui
                .button(RichText::new("Delete").color(Color32::from_rgb(220, 132, 142)))
                .clicked()
            {
                self.delete(item);
            }
        });
        ui.label(item.copied_label())
            .on_hover_text(item.exact_times());
        if let Some(time) = item.last_used {
            ui.label(format!("Last used {}", crate::storage::relative_time(time)))
                .on_hover_text(item.exact_times());
        }
        ui.separator();
        ScrollArea::vertical()
            .id_salt("full-preview")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if item.kind == "Images" {
                    if let Some(t) = self.full_image(ctx, item) {
                        ui.add(egui::Image::new(&t).max_width(ui.available_width()));
                    } else {
                        ui.label(
                            "Image preview unavailable. The saved original can still be copied.",
                        );
                    }
                } else {
                    if item.kind == "Files" {
                        ui.label("File references only. The original files are not backed up.");
                    }
                    if self
                        .text_cache
                        .as_ref()
                        .is_none_or(|(id, _)| id != &item.id)
                    {
                        self.text_cache =
                            Some((item.id.clone(), self.store.lock().unwrap().full_text(item)));
                    }
                    let result = self.text_cache.as_ref().unwrap().1.clone();
                    match result {
                        Ok(text) => {
                            ui.add(Label::new(text).wrap().selectable(true));
                        }
                        Err(e) => {
                            ui.colored_label(
                                Color32::LIGHT_RED,
                                format!("Saved content unavailable: {e}"),
                            );
                        }
                    }
                }
            });
    }
    fn editor(&mut self, ui: &mut Ui) {
        let Some((original, mut text)) = self.editor.take() else {
            return;
        };
        let mut cancel = false;
        let mut save = false;
        let mut copy = false;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Cancel / Back").clicked() {
                cancel = true;
            }
            if ui.button("Save as new clip").clicked() {
                save = true;
            }
            if ui.add(Button::new("Save and copy").fill(ACCENT)).clicked() {
                save = true;
                copy = true;
            }
        });
        ui.heading("Edit text as a new clip");
        ui.label("The original content, position, and pin remain unchanged. Edited clips start unpinned.");
        ScrollArea::vertical()
            .id_salt("editor")
            .max_height(280.)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut text)
                        .desired_rows(18)
                        .desired_width(ui.available_width())
                        .font(TextStyle::Monospace),
                );
            });
        if cancel {
            return;
        }
        if save {
            let result = { self.store.lock().unwrap().edit_new(&original, &text) };
            match result {
                Ok(Some(item)) => {
                    self.selected = Some(item.id.clone());
                    self.query.clear();
                    self.category = "All".into();
                    self.toast = Some(("New clip saved.".into(), Instant::now()));
                    if copy {
                        self.activate(&item);
                    }
                    return;
                }
                Ok(None) => {
                    self.toast = Some((
                        "Text is unchanged; no clip was added.".into(),
                        Instant::now(),
                    ));
                    return;
                }
                Err(e) => self.error = Some(e),
            }
        }
        self.editor = Some((original, text));
    }
    fn apply_settings(&mut self, settings: Settings) {
        let result = {
            let mut store = self.store.lock().unwrap();
            let old = store.settings.clone();
            store.settings = settings;
            match store.save_settings() {
                Ok(()) => Ok(()),
                Err(e) => {
                    store.settings = old;
                    Err(e)
                }
            }
        };
        if let Err(e) = result {
            self.error = Some(e);
        } else {
            self.toast = Some(("Saved".into(), Instant::now()));
        }
    }
    fn settings(&mut self, ui: &mut Ui, ctx: &Context) {
        let previous = self.settings_tab;
        let available = ui.available_size();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.;
            ui.allocate_ui_with_layout(
                vec2(96., available.y),
                Layout::top_down(Align::Min),
                |ui| {
                    if ui.button("History").clicked() {
                        self.settings_open = false;
                    }
                    ui.add_space(12.);
                    for (index, label) in ["General", "Appearance", "Shortcuts", "Privacy", "About"]
                        .iter()
                        .enumerate()
                    {
                        ui.selectable_value(&mut self.settings_tab, index, *label);
                    }
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                vec2((available.x - 148.).max(120.), available.y),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_max_width((available.x - 164.).max(120.));
                    self.settings_content(ui, ctx);
                },
            );
        });
        if previous != self.settings_tab || !self.settings_open {
            self.cancel_recording();
        }
    }
    fn settings_content(&mut self, ui: &mut Ui, ctx: &Context) {
        let mut s = self.store.lock().unwrap().settings.clone();
        let before = serde_json::to_string(&s).unwrap();
        let previous_startup = (s.startup, s.start_hidden);
        let mut startup_changed = false;
        ScrollArea::vertical().id_salt("settings-page").auto_shrink([false,false]).show(ui,|ui|{
            ui.set_max_width(ui.available_width());
            match self.settings_tab {
                0=>{
                    heading(ui,"History that stays useful","Choose what to keep and how ClipVault behaves.");
                    startup_changed=ui.checkbox(&mut s.startup,"Start on login").changed();
                    if s.startup {startup_changed |= ui.checkbox(&mut s.start_hidden,"Start hidden").changed();}
                    ui.label(RichText::new("Start hidden applies only at login. Opening ClipVault from the app menu always shows the window.").color(muted(ui)));
                    ui.label(RichText::new("Monitoring continues when the window is hidden. Copies made while ClipVault is not running cannot be captured.").color(muted(ui)));
                    ui.checkbox(&mut s.confirm_before_delete,"Confirm before deleting");
                    ui.checkbox(&mut s.always_on_top,"Always on top");
                    ui.checkbox(&mut s.system_title_bar,"Use system title bar and borders");
                    ui.checkbox(&mut s.skip_consecutive,"Skip consecutive duplicates");
                    ui.label(RichText::new("Identical content shares one record. When enabled, repeated consecutive copies leave its timestamp unchanged; otherwise recency refreshes.").color(muted(ui)));
                    ui.checkbox(&mut s.hide_after_copy,"Hide after successfully copying a clip");
                    ui.label("This preference applies to the explicit Copy action. Selecting a row or pressing Enter always hides history and attempts paste.");
                    ui.separator();
                    ui.checkbox(&mut s.age_enabled,"Enable age-based cleanup");
                    ui.add_enabled_ui(s.age_enabled,|ui|{ui.horizontal_wrapped(|ui|{ui.label("Delete unpinned clips after");ui.add(DragValue::new(&mut s.days).range(1..=3650).suffix(" days"));});});
                    ui.checkbox(&mut s.count_enabled,"Enable maximum-item cleanup");
                    ui.add_enabled_ui(s.count_enabled,|ui|{ui.horizontal_wrapped(|ui|{ui.label("Keep at most");ui.add(DragValue::new(&mut s.limit).range(1..=10000).suffix(" unpinned clips"));});});
                    ui.label(RichText::new("Either rule, both, or neither can be enabled. Age cleanup uses the last-used time, or the saved capture time if never used. Either enabled limit can remove eligible unpinned clips. Pins never expire and do not count toward the maximum.").color(muted(ui)));
                    ui.checkbox(&mut s.clear_on_reboot,"Clear unpinned history on reboot");
                    ui.label("Off by default. Only a changed Linux boot ID triggers cleanup. Restarting ClipVault or updating its package does not. Pins stay safe.");
                    ui.separator();
                    ui.horizontal_wrapped(|ui|{ui.label("Maximum clip size");ui.add(DragValue::new(&mut s.max_mb).range(1..=128).suffix(" MB"));});
                    if ui.button("Clear unpinned history…").clicked(){self.confirm_clear=true;}
                }
                1=>{
                    heading(ui,"Appearance","A comfortable space for your clipboard.");
                    ui.horizontal_wrapped(|ui|{
                        for (value,label) in [("system","System"),("light","Light"),("dark","Dark")] {
                            if ui.selectable_label(if s.theme.is_empty(){(s.light && value=="light")||(!s.light && value=="dark")}else{s.theme==value},label).clicked(){s.theme=value.into();s.light=value=="light";}
                        }
                    });
                    ui.add_space(12.);
                    ui.horizontal(|ui|{ui.selectable_value(&mut s.compact,true,"Compact");ui.selectable_value(&mut s.compact,false,"Comfortable");});
                    ui.label(RichText::new("Compact shows just the clip content. Comfortable adds one full-content preview on the right.").color(muted(ui)));

                    ui.add_space(8.);
                    ui.label("App font");
                    ComboBox::from_id_salt("app-font").selected_text(if s.app_font.is_empty() { "System default" } else { &s.app_font }).width(220.).show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.app_font, String::new(), "System default");
                        for font in &self.installed_fonts { ui.selectable_value(&mut s.app_font, font.family.clone(), &font.family); }
                    });
                    ui.label("The quick brown fox jumps over the lazy dog. Aa 0123456789");
                    if !self.font_available { ui.label("Saved font unavailable. Using System default until it is installed again."); }
                    ui.horizontal(|ui|{ui.label("Font size");ui.add(DragValue::new(&mut s.font_size).range(12.0..=22.0).suffix(" pt"));});
                    ui.label("Resize from the window border. Size and position are saved while running; Wayland controls window placement.");
                }
                2=>{ self.shortcuts_page(ui); }
                3=>{
                    heading(ui,"Privacy","Clipboard history stays on this device.");
                    ui.checkbox(&mut s.paused,"Pause clipboard monitoring");
                    ui.label("Checking for updates contacts GitHub. Clipboard content is never sent.");
                    ui.label("Automatic update checks are optional and disabled by default. No analytics, clips, exclusions, or personal file paths are sent.");
                    ui.label("Exclude content containing (one phrase per line)");
                    ui.add(TextEdit::multiline(&mut s.exclude_content).desired_rows(3).desired_width(ui.available_width()));
                    ui.label("Exclude source applications on X11 (one name per line)");
                    ui.add(TextEdit::multiline(&mut s.exclude_apps).desired_rows(3).desired_width(ui.available_width()));
                    ui.label("X11 checks the active window's application name, which may differ from the clipboard owner. Wayland does not expose source application identity. Application exclusions cannot guarantee protection.");
                    ui.label("Supported password-manager and sensitive-content MIME hints are always respected. Content matching scans textual formats, not image pixels. Pause before copying other sensitive content.");
                    ui.separator();
                    ui.label("Encryption: not encrypted. Private Linux account permissions protect the local files. Deletion is not secure erasure and does not clear your system clipboard or backups.");
                    let root=self.store.lock().unwrap().root.clone();
                    ui.add(Label::new(format!("Local storage: {}",root.display())).wrap());
                    if ui.button("Open storage folder").clicked(){if let Err(e)=std::process::Command::new("xdg-open").arg(&root).spawn(){self.error=Some(e.to_string());}}
                }
                _=>{ self.about_page(ui, ctx, &mut s); }

            }
            ui.add_space(18.);
        });
        if startup_changed {
            let root = self.store.lock().unwrap().root.clone();
            if let Err(e) = set_startup(s.startup, &root) {
                (s.startup, s.start_hidden) = previous_startup;
                self.error = Some(e);
            }
        }
        if before != serde_json::to_string(&s).unwrap() {
            let deletes = { self.store.lock().unwrap().cleanup_ids(&s).len() };
            let old = self.store.lock().unwrap().settings.clone();
            let retention_changed = old.age_enabled != s.age_enabled
                || old.count_enabled != s.count_enabled
                || old.days != s.days
                || old.limit != s.limit;
            if retention_changed && deletes > 0 {
                self.pending_settings = Some(s);
            } else {
                if old.compact != s.compact {
                    let maximized = ctx.input(|i| i.viewport().maximized == Some(true));
                    let mut old_geometry = old.geometry();
                    if !maximized {
                        if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                            old_geometry.size = [rect.width(), rect.height()];
                        }
                    }
                    old_geometry.maximized = maximized;
                    if old.compact {
                        s.compact_geometry = Some(old_geometry);
                    } else {
                        s.comfortable_geometry = Some(old_geometry);
                    }
                    let geometry = s.geometry();
                    ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(vec2(
                        if s.compact { 440. } else { 640. },
                        420.,
                    )));
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(
                        geometry.size[0],
                        geometry.size[1],
                    )));
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(geometry.maximized));
                    self.last_geometry = Instant::now();
                }
                self.apply_settings(s);
            }
        }
    }
    fn begin_shortcut_recording(&mut self) {
        if self.shortcut_recording || self.shortcut_pending {
            return;
        }
        self.shortcut_recording = true;
        self.shortcut_display = "Press your shortcut".into();
        self.shortcut_feedback = "Starting keyboard recorder…".into();
        if self.service.tx.send(Command::RecordShortcut(true)).is_err() {
            self.shortcut_recording = false;
            self.active_shortcut = None;
            self.shortcut_feedback =
                "Keyboard service unavailable; shortcut was not changed. Restart ClipVault.".into();
        }
    }
    fn shortcuts_page(&mut self, ui: &mut Ui) {
        heading(
            ui,
            "Shortcuts",
            "A faster way to get back to what you copied.",
        );
        Frame::group(ui.style()).inner_margin(16).corner_radius(10).show(ui, |ui| {
            ui.set_width((ui.available_width()-24.).max(0.));
            ui.add(Label::new(RichText::new("Open clipboard history").size(17.).strong()).wrap());
            ui.label("Press once to show and focus. Press again to hide.");
            ui.add_space(8.);
            let saved = self.store.lock().unwrap().settings.shortcut.clone();
            let label = if self.shortcut_recording { self.shortcut_display.clone() }
                else if self.shortcut_pending { "Finishing recording…".into() }
                else { self.active_shortcut.as_ref().unwrap_or(&saved).replace('+', " + ") };
            if ui.add_enabled_ui(!self.shortcut_pending, |ui|ui.add_sized([ui.available_width(),64.], Button::new(RichText::new(format!("Record shortcut\n{label}")).strong().size(16.)).wrap())).inner.clicked() && !self.shortcut_recording {
                {
                    self.shortcut_recording = true; self.shortcut_display = "Press your shortcut".into();
                    self.shortcut_feedback = "Starting keyboard recorder…".into();
                    if self.service.tx.send(Command::RecordShortcut(true)).is_err() {
                        self.shortcut_recording = false; self.active_shortcut = None;
                        self.shortcut_feedback = "Keyboard service unavailable; shortcut was not changed. Restart ClipVault.".into();
                    }
                }
            }
            ui.label(RichText::new("Super is the Windows key").color(muted(ui)));
            ui.label("Hold Ctrl, Alt, Shift, or Super and press one letter or number. Release the full combination to save. Escape cancels.");
            ui.label(RichText::new(&self.shortcut_feedback).color(ui.visuals().hyperlink_color));
            if !self.shortcut_recording && !self.shortcut_pending {
                ui.label(RichText::new(match &self.active_shortcut {
                    Some(s) => format!("Active: {}", s.replace('+', " + ")),
                    None => "No confirmed global binding. Use desktop setup below if unavailable.".into(),
                }).color(muted(ui)));
            }
        });
        if self.active_shortcut.is_none()
            && self.desktop_shortcut.is_none()
            && ui.button("Choose shortcut").clicked()
        {
            self.begin_shortcut_recording();
        }
        if ui.button("Open Keyboard settings").clicked() {
            if let Err(e) = crate::desktop::open_keyboard() {
                self.shortcut_feedback = e;
            }
        }
        ui.label(format!(
            "Desktop: {:?} · Session: {}",
            crate::desktop::detect(),
            std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into())
        ));
        ui.add_space(16.);
        ui.label(RichText::new("Within ClipVault").size(17.).strong());
        for (keys, description) in [
            ("↑ / ↓", "Select previous / next clip"),
            ("Enter", "Activate and attempt paste"),
            ("Ctrl + F", "Focus search"),
            ("Ctrl + Shift + P", "Pin / unpin"),
            ("Delete", "Delete selected clip"),
            ("Escape", "Cancel or hide history"),
            ("Tab", "Move between controls"),
        ] {
            ui.vertical(|ui| {
                keycap(ui, keys);
                ui.add(Label::new(description).wrap());
            });
        }
        ui.add_space(12.);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            keycap(ui, "Ctrl");
            keycap(ui, "V");
            ui.label("pastes in the destination application.");
        });
        ui.label(RichText::new("Selecting history restores its saved formats, hides ClipVault, and attempts paste. If unavailable, press Ctrl+V in your app.").color(muted(ui)));
        ui.add_space(10.);
        CollapsingHeader::new("Advanced desktop setup").show(ui, |ui| {
            let root=self.store.lock().unwrap().root.clone();
            ui.label("Exact command for this installation (preserves your data directory):");
            ui.add(Label::new(crate::desktop::toggle_command(&root)).wrap().selectable(true));
            ui.label("Cinnamon, GNOME, MATE and Xfce: ClipVault manages only its own custom action when the desktop settings tools are available. Changes made in Keyboard settings are read back. Configured means saved and read back; it is not independent proof that the desktop activated the shortcut.");
            ui.label("Clipboard data-control does not register shortcuts. Desktop settings tools handle the binding separately. Plasma and unsupported desktops require manual setup.");
            ui.label("GNOME: Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts. Add ClipVault with command clipvault --toggle, then record Super+V there.");
            ui.label("KDE Plasma: System Settings → Keyboard → Shortcuts. Add an application/command for clipvault --toggle and assign your keys.");
            ui.label("Sway: bindsym --no-repeat $mod+v exec /absolute/path/to/clipvault --toggle");
            ui.label("Hyprland: bind = SUPER, V, exec, /absolute/path/to/clipvault --toggle (do not use a repeating bind).");
            ui.label("Use your installed executable’s absolute path if it is not in PATH. For AppImage, use the stable AppImage path. Include the same --data-dir if you use custom storage. Review conflicts in Keyboard settings; ClipVault never removes another action. This also works on X11 when a global grab is unavailable.");
        });
    }
    fn start_update_check(&mut self, ctx: &Context, automatic: bool) {
        if self.checker.running() {
            return;
        }
        let timestamp = now();
        let prefs = self.store.lock().unwrap().settings.updates.clone();
        if timestamp < prefs.retry_after
            || prefs
                .last_attempt
                .is_some_and(|t| timestamp.saturating_sub(t) < 30)
        {
            return;
        }
        // Persist before starting the worker: a crash or offline failure cannot
        // create an automatic retry loop after restart.
        let old = prefs;
        let result = {
            let mut s = self.store.lock().unwrap();
            s.settings.updates.last_attempt = Some(timestamp);
            s.settings.updates.last_status = "Checking for updates…".into();
            let result = s.save_settings();
            if result.is_err() {
                s.settings.updates = old;
            }
            result
        };
        if result.is_err() {
            if !automatic {
                self.error =
                    Some("Could not save the update schedule; check was not started.".into());
            }
            return;
        }
        self.update_outcome = None;
        self.checker.start(automatic, ctx.clone());
    }
    fn poll_updates(&mut self, ctx: &Context) {
        if let Some(result) = self.checker.poll() {
            let mut prefs = self.store.lock().unwrap().settings.updates.clone();
            match result {
                Ok(outcome) => {
                    prefs.last_status = outcome.message();
                    prefs.cached_outcome = Some(outcome.clone());
                    prefs.retry_after = 0;
                    if let crate::updates::Outcome::Available { version, .. } = &outcome {
                        if self.checker.automatic
                            && prefs.automatic
                            && prefs.dismissed_version.as_deref() != Some(version)
                        {
                            self.toast = Some((
                                format!(
                                    "ClipVault {version} is available. See About → App updates."
                                ),
                                Instant::now(),
                            ));
                        }
                    }
                    self.update_outcome = Some(outcome);
                }
                Err(failure) => {
                    prefs.last_status = failure.message;
                    prefs.cached_outcome = None;
                    prefs.retry_after = failure.retry_after;
                    self.update_outcome = None;
                }
            }
            self.mutate(|s| {
                s.settings.updates = prefs;
                s.save_settings()
            });
        }
        let due = self.shot.is_none() && self.store.lock().unwrap().settings.updates.due(now());
        if due {
            self.start_update_check(ctx, true);
        }
    }
    fn about_page(&mut self, ui: &mut Ui, ctx: &Context, settings: &mut Settings) {
        ui.horizontal(|ui| {
            // This bundled raster is generated solely from assets/clipvault.svg.
            ui.image((self.logo.id(), vec2(52., 52.)));
            ui.vertical(|ui| {
                ui.label(RichText::new("ClipVault").size(24.).strong());
                ui.label(RichText::new("A little history. A smoother workflow.").color(muted(ui)));
                ui.label(
                    RichText::new(format!("v{} · Native Rust app", env!("CARGO_PKG_VERSION")))
                        .color(ui.visuals().hyperlink_color),
                );
            });
        });
        debug_assert!(
            ui.min_rect().is_finite(),
            "about section rect {:?}",
            ui.min_rect()
        );
        if ui.button("View changelog").clicked() {
            self.changelog_open = true;
        }
        ui.add_space(14.);
        ui.separator();
        ui.add_space(12.);
        ui.label(
            RichText::new("DESIGNED & BUILT BY")
                .size(11.)
                .strong()
                .color(ui.visuals().hyperlink_color),
        );
        ui.label(
            RichText::new("Md Biplob Hosain")
                .size(if ui.available_width() < 450. {
                    30.
                } else {
                    33.
                })
                .strong(),
        );
        ui.label(RichText::new("Developer & creator of ClipVault").color(muted(ui)));
        ui.add_space(12.);
        let links = [
            (
                "github",
                "GitHub",
                "Explore my projects",
                "https://github.com/Amibiplob",
            ),
            (
                "linkedin",
                "LinkedIn",
                "Connect with me",
                "https://linkedin.com/in/amibiplob",
            ),
            (
                "globe",
                "Portfolio",
                "See my work",
                "https://amibiplob.vercel.app",
            ),
            (
                "code",
                "Source code",
                "Build & contribute",
                "https://github.com/Amibiplob/ClipVault",
            ),
            (
                "bug",
                "Report an issue",
                "Help improve ClipVault",
                "https://github.com/Amibiplob/ClipVault/issues",
            ),
            (
                "download",
                "Releases",
                "Downloads & changes",
                "https://github.com/Amibiplob/ClipVault/releases",
            ),
        ];
        debug_assert!(
            ui.min_rect().is_finite(),
            "about before links {:?}",
            ui.min_rect()
        );
        if ui.available_width() >= 500. {
            for row in links.chunks(2) {
                ui.columns(2, |cols| {
                    for (col, link) in cols.iter_mut().zip(row) {
                        link_card(col, *link);
                    }
                });
            }
        } else {
            for link in links {
                link_card(ui, link);
            }
        }
        ui.add_space(12.);
        debug_assert!(
            ui.min_rect().is_finite(),
            "about before updates {:?}",
            ui.min_rect()
        );
        Frame::group(ui.style()).inner_margin(14).corner_radius(9).show(ui, |ui| {
            ui.set_width(ui.available_width().max(0.));
            ui.label(RichText::new("App updates").size(17.).strong());
            ui.label(format!("Installed version: {}", env!("CARGO_PKG_VERSION")));
            let prefs = &settings.updates;
            let cooldown = now() < prefs.retry_after || prefs.last_attempt.is_some_and(|t| now().saturating_sub(t) < 30);
            if ui.add_enabled(!self.checker.running() && !cooldown, Button::new(if self.checker.running() { "Checking for updates…" } else { "Check for updates" })).clicked() {
                self.start_update_check(ctx, false);
                settings.updates = self.store.lock().unwrap().settings.updates.clone();
            }
            ui.checkbox(&mut settings.updates.automatic, "Automatically check once a day");
            ui.label(RichText::new("Optional and off by default. Updates are installed manually.").color(muted(ui)));
            if let Some(t) = settings.updates.last_attempt {
                let date = chrono::DateTime::from_timestamp(t, 0).map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string()).unwrap_or_else(|| "Unknown time".into());
                ui.label(RichText::new(format!("Last check: {date}")).color(muted(ui)));
                ui.label(if self.checker.running() { "Checking for updates…" } else if settings.updates.last_status == "Checking for updates…" { "Unable to check; try again. The previous check was interrupted." } else { &settings.updates.last_status });
            } else { ui.label(RichText::new("No update check yet.").color(muted(ui))); }
            if cooldown && !self.checker.running() { ui.label(RichText::new("Please wait before checking again.").color(muted(ui))); }
            if let Some(crate::updates::Outcome::Available { version, url }) = self.update_outcome.clone() {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("View release / Download").clicked() && crate::updates::official_release(&url) { ctx.open_url(OpenUrl::new_tab(url)); }
                    if settings.updates.dismissed_version.as_deref() != Some(&version) && ui.button("Remind me later").clicked() {
                        settings.updates.dismissed_version = Some(version.clone()); self.toast = None;
                    }
                });
                if settings.updates.dismissed_version.as_deref() == Some(&version) { ui.label(RichText::new("Reminder dismissed for this release. You can still download it above.").color(muted(ui))); }
                ui.label("Choose the .deb or AppImage on GitHub and install it yourself. ClipVault never downloads or installs updates.");
            }
            ui.add_space(4.);
            ui.label(RichText::new("Clipboard history stays on this device. Checking for updates contacts GitHub. Clipboard content is never sent.").color(muted(ui)));
        });
        debug_assert!(
            ui.min_rect().is_finite(),
            "about section rect {:?}",
            ui.min_rect()
        );
        ui.add_space(14.);
        CollapsingHeader::new("Linux desktop compatibility").show(ui, |ui| {
            ui.label("X11 supports clipboard monitoring and global shortcuts. Wayland needs ext-data-control or wlr-data-control and a desktop shortcut for clipvault --toggle. GNOME Wayland may require an appropriate extension or an X11 session. Support varies by compositor.");
            ui.label(RichText::new(&self.status).color(muted(ui)));
        });
        ui.separator();
        CollapsingHeader::new("License & acknowledgments").show(ui, |ui| {
            ui.label("MIT License · Copyright 2026 Md Biplob Hosain.");
            ui.label("ClipVault’s logo is bundled with the application. GitHub and LinkedIn icons are adapted from Bootstrap Icons, MIT, Copyright 2019–2024 The Bootstrap Authors. Third-party dependency, icon, and font licenses are preserved in THIRD-PARTY-NOTICES.txt in the package documentation.");
            CollapsingHeader::new("MIT license text").show(ui, |ui| { ui.label(include_str!("../LICENSE")); });
            CollapsingHeader::new("Third-party license information").show(ui, |ui| {
                ScrollArea::vertical().id_salt("license-notices").max_height(220.).show(ui, |ui| { ui.add(Label::new(include_str!("../THIRD-PARTY-NOTICES.txt")).wrap()); });
            });
        });
    }
    fn dialogs(&mut self, ctx: &Context) {
        if self.confirm_clear {
            Window::new("Clear unpinned history?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER,Vec2::ZERO).show(ctx,|ui|{
                ui.set_max_width(330.);ui.label("Delete all unpinned saved clips and unused payloads? Pinned clips are protected. This cannot be undone.");
                ui.horizontal(|ui|{if ui.button("Cancel").clicked(){self.confirm_clear=false;}
                    if ui.button("Clear unpinned history").clicked(){self.mutate(|s|s.clear(false));self.confirm_clear=false;self.preview=None;self.textures.clear();}});
            });
        }
        if let Some(item) = self.confirm_delete.clone() {
            Window::new("Delete this clip?")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(330.);
                    ui.label(format!(
                        "Permanently delete “{}”? This cannot be undone.",
                        item.title
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_delete = None;
                        }
                        if ui.button("Delete").clicked() {
                            self.delete_confirmed(&item);
                        }
                    });
                });
        }
        if let Some(s) = self.pending_settings.clone() {
            let n = { self.store.lock().unwrap().cleanup_ids(&s).len() };
            Window::new("Apply automatic cleanup?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER,Vec2::ZERO).show(ctx,|ui|{
                ui.set_max_width(330.);ui.label(format!("These settings immediately delete {n} unpinned clips. Pinned clips stay. Apply?"));
                ui.horizontal(|ui|{if ui.button("Cancel").clicked(){self.pending_settings=None;}
                    if ui.button("Apply and delete").clicked(){self.apply_settings(s);self.pending_settings=None;self.preview=None;}});
            });
        }
    }
    fn consider_changelog(&mut self) {
        if !self.hidden && !self.changelog_considered && self.shot.is_none() {
            self.changelog_considered = true;
            self.changelog_open = self
                .store
                .lock()
                .unwrap()
                .settings
                .changelog_dismissed_version
                != env!("CARGO_PKG_VERSION");
        }
    }
    fn dismiss_changelog(&mut self) {
        if self.mutate(|s| {
            s.settings.changelog_dismissed_version = env!("CARGO_PKG_VERSION").into();
            s.save_settings()
        }) {
            self.changelog_open = false;
        }
    }
    fn changelog(&mut self, ctx: &Context) {
        if !self.changelog_open {
            return;
        }
        egui::Modal::new(Id::new("changelog")).show(ctx, |ui| {
            ui.set_width((ctx.screen_rect().width() - 80.).min(520.));
            ui.heading(format!(
                "What’s New in ClipVault v{}",
                env!("CARGO_PKG_VERSION")
            ));
            ScrollArea::vertical()
                .max_height((ctx.screen_rect().height() - 180.).max(100.))
                .show(ui, |ui| {
                    ui.add(Label::new(include_str!("../CHANGELOG-v2.0.0.txt")).wrap());
                });
            if ui.button("Done").clicked() {
                self.dismiss_changelog();
            }
        });
    }
    fn history_list(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        items: &[Item],
    ) -> scroll_area::ScrollAreaOutput<()> {
        let mut scroll = ScrollArea::vertical().id_salt("history");
        if self.scroll_to_selected {
            self.scroll_to_selected = false;
            if let Some(index) = items
                .iter()
                .position(|item| self.selected.as_deref() == Some(&item.id))
            {
                scroll = scroll.vertical_scroll_offset(
                    (index as f32 * 61. - ui.available_height() / 2.).max(0.),
                );
            }
        }
        scroll
            .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysVisible)
            .auto_shrink([false, false])
            .show_rows(ui, 52., items.len(), |ui, range| {
                for item in &items[range] {
                    self.row(ui, ctx, item, true);
                }
            })
    }
    fn render(&mut self, ctx: &Context) {
        if let Some(item) = &self.preview {
            let current = {
                self.store
                    .lock()
                    .unwrap()
                    .items
                    .iter()
                    .find(|i| i.id == item.id)
                    .cloned()
            };
            self.preview = current;
        }
        self.consider_changelog();
        let mut settings = self.store.lock().unwrap().settings.clone();
        settings.light = match settings.theme.as_str() {
            "light" => true,
            "dark" => false,
            "system" => ctx.input(|i| i.raw.system_theme == Some(egui::Theme::Light)),
            _ => settings.light,
        };
        ctx.set_zoom_factor((settings.font_size / 14.).clamp(0.85, 1.6));
        if settings.app_font != self.applied_font {
            self.font_available =
                crate::fonts::apply(ctx, &settings.app_font, &self.installed_fonts);
            self.applied_font = settings.app_font.clone();
        }
        ctx.send_viewport_cmd(ViewportCommand::Decorations(settings.system_title_bar));
        ctx.style_mut(|s| s.spacing.scroll = egui::style::ScrollStyle::solid());
        if settings.compact {
            self.preview = None;
        }
        let mut visuals = if settings.light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        if !settings.light {
            visuals.panel_fill = Color32::from_rgb(24, 24, 30);
            visuals.window_fill = Color32::from_rgb(31, 30, 38);
            visuals.extreme_bg_color = Color32::from_rgb(35, 34, 42);
            visuals.override_text_color = Some(Color32::from_rgb(232, 229, 241));
        }
        visuals.selection.bg_fill = if settings.light {
            Color32::from_rgb(217, 195, 242)
        } else {
            Color32::from_rgb(69, 54, 91)
        };
        let accent = if settings.light {
            Color32::from_rgb(103, 61, 151)
        } else {
            ACCENT
        };
        visuals.hyperlink_color = accent;
        visuals.selection.stroke = Stroke::new(1_f32, accent);
        visuals.widgets.active.bg_stroke = Stroke::new(2_f32, accent);
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.noninteractive,
        ] {
            widget.corner_radius = CornerRadius::same(6);
        }
        if !settings.light {
            visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(40, 39, 48);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 39, 48);
            visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(58, 50, 72);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 50, 72);
            visuals.widgets.inactive.bg_stroke = Stroke::new(1_f32, Color32::from_rgb(65, 61, 76));
        }
        ctx.set_visuals(visuals);
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if settings.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        let query = self.query.to_lowercase();
        let key = {
            let store = self.store.lock().unwrap();
            (
                store.revision,
                store.items.len(),
                query.clone(),
                self.category.clone(),
            )
        };
        if self.list_cache.as_ref().is_none_or(|(old, _)| old != &key) {
            let store = self.store.lock().unwrap();
            let matches = if query.is_empty() {
                None
            } else {
                Some(store.search_ids(&query))
            };
            let mut search_error = None;
            let matched = match matches {
                Some(Ok(ids)) => Some(ids),
                Some(Err(e)) => {
                    search_error = Some(e);
                    None
                }
                None => None,
            };
            let items = store
                .items
                .iter()
                .filter(|i| {
                    (self.category == "All"
                        || self.category == i.kind
                        || (self.category == "Pinned" && i.pinned))
                        && (query.is_empty()
                            || matched.as_ref().is_some_and(|ids| ids.contains(&i.id))
                            || i.title.to_lowercase().contains(&query)
                            || i.preview.to_lowercase().contains(&query))
                })
                .cloned()
                .collect();
            drop(store);
            if let Some(e) = search_error {
                self.error = Some(format!("Search unavailable: {e}"));
            }
            self.list_cache = Some((key, std::sync::Arc::new(items)));
        }
        let items = self.list_cache.as_ref().unwrap().1.clone();
        let mut selected_index = items
            .iter()
            .position(|item| self.selected.as_deref() == Some(&item.id))
            .unwrap_or(0);
        self.selected = items.get(selected_index).map(|item| item.id.clone());
        let modal = self.confirm_clear
            || self.confirm_delete.is_some()
            || self.pending_settings.is_some()
            || self.changelog_open
            || self.editor.is_some();
        if !self.shortcut_recording
            && !self.shortcut_pending
            && !modal
            && !self.settings_open
            && !ctx.memory(|m| m.any_popup_open())
            && self.editor.is_none()
            && self.preview.is_none()
        {
            if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::F)) {
                self.focus_search = true;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                selected_index = (selected_index + 1).min(items.len().saturating_sub(1));
                self.scroll_to_selected = true;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                selected_index = selected_index.saturating_sub(1);
                self.scroll_to_selected = true;
            }
            self.selected = items.get(selected_index).map(|item| item.id.clone());
            if let Some(item) = items.get(selected_index) {
                let focus = ctx.memory(|m| m.focused());
                let history_focused = focus.is_none()
                    || focus == Some(Id::new("search"))
                    || items
                        .iter()
                        .any(|row| focus == Some(Id::new(("clip-row", &row.id))));
                // Enter on a focused action belongs to that action, not history.
                if history_focused && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                {
                    self.activate_and_paste(item);
                }
                if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::P)) {
                    self.pin(item);
                }
                if !ctx.wants_keyboard_input()
                    && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete))
                {
                    self.delete(item);
                }
            }
        }
        if !self.shortcut_recording
            && !self.shortcut_pending
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            if self.changelog_open {
                self.dismiss_changelog();
            } else if self.editor.is_some() {
                self.editor = None;
            } else if modal {
                self.confirm_clear = false;
                self.confirm_delete = None;
                self.pending_settings = None;
            } else if self.editor.is_some() {
                self.editor = None;
            } else if self.preview.is_some() {
                self.preview = None;
            } else if self.settings_open {
                self.settings_open = false;
            } else {
                self.hide(ctx);
            }
        }
        TopBottomPanel::top("header")
            .frame(
                Frame::NONE
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(18),
            )
            .show(ctx, |ui| self.header(ui, ctx));
        TopBottomPanel::bottom("footer")
            .frame(
                Frame::NONE
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(12),
            )
            .show(ctx, |ui| {
                if self.settings_open && self.settings_tab == 4 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Open source. Made for Linux.").color(muted(ui)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Quit ClipVault").clicked() {
                                self.quit(ctx);
                            }
                        });
                    });
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Up/Down Select   Enter Paste   Ctrl+F Search   Esc Hide")
                            .size(12.)
                            .color(muted(ui)),
                    );
                    if let Some((message, time)) = &self.toast {
                        if time.elapsed() < Duration::from_secs(5) {
                            ui.label(RichText::new(message).color(ACCENT));
                        }
                    }
                });
            });
        CentralPanel::default().frame(Frame::NONE.fill(ctx.style().visuals.panel_fill).inner_margin(18)).show(ctx,|ui|{
            if let Some(e)=self.error.clone(){ui.horizontal_wrapped(|ui|{ui.add(Label::new(RichText::new(&e).color(if settings.light{Color32::DARK_RED}else{Color32::from_rgb(238,176,150)})).wrap());if ui.small_button("Dismiss").clicked(){self.error=None;}});
                if self.active_shortcut.is_none()&&self.desktop_shortcut.is_none(){ui.horizontal_wrapped(|ui|{
                    if ui.button("Choose shortcut").clicked(){self.settings_open=true;self.settings_tab=2;self.begin_shortcut_recording();}
                    if ui.button("Open Keyboard settings").clicked(){if let Err(error)=crate::desktop::open_keyboard(){self.error=Some(error);}}
                });}
                ui.separator();}
            ui.add_enabled_ui(!modal,|ui|{
                if self.settings_open{self.settings(ui,ctx);return;}
                if let Some(item)=self.preview.clone(){self.detail(ui,ctx,&item,true);return;}
                self.search(ui);ui.separator();
                ui.horizontal(|ui|{ui.label(RichText::new(format!("{} clips · Newest first",items.len())).color(muted(ui)));ui.with_layout(Layout::right_to_left(Align::Center),|ui|{if ui.small_button("Clear…").clicked(){self.confirm_clear=true;}});});
                if let Some(version)=self.replacement.clone(){
                    ui.label(format!("An installed update ({version}) is ready. Close the panel to restart safely. Unsaved edits are kept open."));
                    if ui.button("Restart now").clicked() && self.editor.is_none(){self.restart_safely(ctx);}
                }
                if items.is_empty(){
                    ui.add_space(55.);ui.vertical_centered(|ui|{ui.image((self.logo.id(),vec2(66.,66.)));ui.heading(if query.is_empty(){"Your clipboard, remembered."}else{"No matching clips"});
                    if query.is_empty(){ui.label("Copy text, an image, or a file reference while monitoring is enabled.");}
                    else {ui.label("Try a different search or filter.");if ui.button("Clear search").clicked(){self.query.clear();self.category="All".into();self.focus_search=true;}}ui.label(RichText::new(&self.status).color(muted(ui)));});return;
                }
                if !settings.compact {
                    let available=ui.available_size();
                    let min=190.;
                    let width=available.x.max(min*2.+12.);
                    let left=(width*settings.divider).clamp(min,width-min-12.);
                    let (rect,_)=ui.allocate_exact_size(vec2(width,available.y),Sense::hover());
                    let divider=Rect::from_min_size(rect.min+vec2(left,0.),vec2(10.,rect.height()));
                    let response=ui.interact(divider,Id::new("preview-divider"),Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
                    ui.painter().line_segment([divider.center_top(),divider.center_bottom()],Stroke::new(1_f32,ui.visuals().widgets.noninteractive.bg_stroke.color));
                    if response.dragged(){if let Some(p)=response.interact_pointer_pos(){let ratio=((p.x-rect.left())/width).clamp(min/width,(width-min-12.)/width);self.mutate(|s|{s.settings.divider=ratio;s.save_settings()});}}
                    let mut list=ui.new_child(UiBuilder::new().id_salt("history-pane").max_rect(Rect::from_min_size(rect.min,vec2(left,rect.height()))).layout(Layout::top_down(Align::Min)));
                    self.history_list(&mut list,ctx,&items);
                    let mut preview=ui.new_child(UiBuilder::new().id_salt("preview-pane").max_rect(Rect::from_min_max(pos2(divider.right()+2.,rect.top()),rect.max)).layout(Layout::top_down(Align::Min)));
                    if let Some(item)=items.iter().find(|i|self.selected.as_deref()==Some(&i.id)){self.detail(&mut preview,ctx,item,false);}
                } else {
                    self.history_list(ui, ctx, &items);
                }
            });
        });
        self.dialogs(ctx);
        if self.editor.is_some() {
            egui::Modal::new(Id::new("edit-clip")).show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() - 70.).clamp(250., 600.));
                self.editor(ui);
            });
        }
        self.changelog(ctx);
        if !settings.system_title_bar {
            custom_resize(ctx);
        }
    }
}
impl eframe::App for VaultApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        if let Some(rx) = &self.restart_updates {
            while let Ok(version) = rx.try_recv() {
                if let Some(message) = version.strip_prefix("manual:") {
                    self.error = Some(message.into());
                } else if !version.is_empty() {
                    self.replacement = Some(version);
                }
            }
        }
        if self.replacement.is_some()
            && self.hidden
            && self.editor.is_none()
            && self.pending_settings.is_none()
            && self.pending_activation.is_none()
            && !self.shortcut_recording
        {
            self.restart_safely(ctx);
        }
        if self.shortcut_recording && self.local_recorder.recording {
            let result = ctx.input(|i| {
                self.local_recorder.input(
                    &i.events,
                    i.modifiers,
                    !i.keys_down.is_empty(),
                    i.focused,
                )
            });
            self.shortcut_display = self.local_recorder.display();
            if let Some(result) = result {
                match result {
                    crate::shortcut::Result::Saved(next) => {
                        self.shortcut_pending = true;
                        let _ = self.service.tx.send(Command::SetShortcut(next));
                    }
                    _ => self.cancel_recording(),
                }
            }
        }
        self.minimized = ctx.input(|i| i.viewport().minimized == Some(true));
        self.poll_updates(ctx);
        while let Ok(notice) = self.service.rx.try_recv() {
            self.handle_notice(ctx, notice);
        }
        while let Ok(message) = self.ipc.try_recv() {
            self.handle_ipc(ctx, &message);
        }
        if self.last_cleanup.elapsed() > Duration::from_secs(60) {
            self.last_cleanup = Instant::now();
            self.mutate(|s| s.cleanup());
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.shot.is_none() && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide(ctx);
        }
        if self.shot.is_none()
            && !self.hidden
            && !ctx.input(|i| i.viewport().minimized == Some(true))
            && self.last_geometry.elapsed() > Duration::from_secs(2)
        {
            self.last_geometry = Instant::now();
            let (inner, outer, monitor) = ctx.input(|i| {
                (
                    i.viewport().inner_rect,
                    i.viewport().outer_rect,
                    i.viewport().monitor_size,
                )
            });
            if let Some(r) = inner {
                let size = [r.width().max(440.), r.height().max(420.)];
                let position = outer.map(|r| [r.min.x, r.min.y]);
                self.mutate(|s| {
                    let mut geometry = s.settings.geometry();
                    let maximized = ctx.input(|i| i.viewport().maximized == Some(true));
                    if !maximized {
                        geometry.size = size;
                    }
                    geometry.maximized = maximized;
                    if geometry != s.settings.geometry() || s.settings.window_position != position {
                        s.settings.set_geometry(geometry);
                        s.settings.window_position = position;
                        s.save_settings()
                    } else {
                        Ok(())
                    }
                });
                if let (Some(out), Some(screen)) = (outer, monitor) {
                    if out.min.x > screen.x - 40.
                        || out.min.y > screen.y - 40.
                        || out.max.x < 40.
                        || out.max.y < 40.
                    {
                        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(pos2(20., 20.)));
                    }
                }
            }
        }
        if self.shortcut_recording && (!self.settings_open || self.settings_tab != 2) {
            self.cancel_recording();
        }
        self.render(ctx);
        self.frames += 1;
        if self.shot.is_some() {
            ctx.request_repaint();
            if self.frames >= 120 && !self.shot_requested {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
                self.shot_requested = true;
            }
            let captures = ctx.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| {
                        if let Event::Screenshot { image, .. } = e {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            });
            for im in captures {
                if let Some(path) = &self.shot {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let bytes: Vec<u8> = im.pixels.iter().flat_map(|p| p.to_array()).collect();
                    match image::save_buffer(
                        path,
                        &bytes,
                        im.width() as u32,
                        im.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        Ok(()) => {
                            println!(
                                "Screenshot saved: {} ({}x{})",
                                path.display(),
                                im.width(),
                                im.height()
                            );
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        Err(e) => {
                            eprintln!("Screenshot failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
        } else {
            ctx.request_repaint_after(Duration::from_secs(if self.hidden { 30 } else { 1 }));
        }
    }
}
fn keycap(ui: &mut Ui, key: &str) {
    Frame::group(ui.style())
        .corner_radius(4)
        .inner_margin(Margin::symmetric(7, 3))
        .show(ui, |ui| {
            if key == "Up" || key == "Down" {
                let (r, _) = ui.allocate_exact_size(vec2(14., 16.), Sense::hover());
                icons::draw(
                    ui.painter(),
                    r,
                    if key == "Up" { "up" } else { "down" },
                    ui.visuals().text_color(),
                );
            } else {
                ui.label(RichText::new(key).strong().size(12.));
            }
        });
}
fn link_card(ui: &mut Ui, link: (&str, &str, &str, &str)) {
    let (icon, title, description, url) = link;
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 60.), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Link, true, title));
    let fill = if response.hovered() || response.has_focus() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        ui.visuals().faint_bg_color
    };
    ui.painter().rect_filled(r, 8, fill);
    ui.painter().rect_stroke(
        r,
        8,
        ui.visuals().widgets.noninteractive.bg_stroke,
        StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(r, 8, ui.visuals().selection.stroke, StrokeKind::Inside);
    }
    let icon_rect = Rect::from_min_size(r.min + vec2(12., 21.), vec2(18., 18.));
    if icon == "github" || icon == "linkedin" {
        let id = Id::new(("brand-texture", icon));
        let texture = ui
            .ctx()
            .data_mut(|d| d.get_temp::<TextureHandle>(id))
            .unwrap_or_else(|| {
                let bytes: &[u8] = if icon == "github" {
                    include_bytes!("../assets/social/github.png")
                } else {
                    include_bytes!("../assets/social/linkedin.png")
                };
                let image = image::load_from_memory(bytes)
                    .expect("bundled brand icon")
                    .into_rgba8();
                let texture = ui.ctx().load_texture(
                    icon,
                    ColorImage::from_rgba_unmultiplied([48, 48], image.as_raw()),
                    TextureOptions::LINEAR,
                );
                ui.ctx().data_mut(|d| d.insert_temp(id, texture.clone()));
                texture
            });
        ui.painter().image(
            texture.id(),
            icon_rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
            ui.visuals().hyperlink_color,
        );
    } else {
        icons::draw(ui.painter(), icon_rect, icon, ui.visuals().hyperlink_color);
    }
    ui.painter().text(
        r.min + vec2(40., 13.),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(13.),
        ui.visuals().text_color(),
    );
    ui.painter().text(
        r.min + vec2(40., 34.),
        Align2::LEFT_TOP,
        description,
        FontId::proportional(11.),
        muted(ui),
    );
    icons::draw(
        ui.painter(),
        Rect::from_min_size(pos2(r.max.x - 28., r.center().y - 7.), vec2(14., 14.)),
        "external",
        ui.visuals().hyperlink_color,
    );
    if response.clicked() {
        ui.ctx().open_url(OpenUrl::new_tab(url));
    }
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(url);
}
fn heading(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).strong().size(19.));
    ui.label(RichText::new(subtitle).color(muted(ui)));
    ui.add_space(12.);
}
fn copy_label(item: &Item) -> &'static str {
    match item.kind.as_str() {
        "Text" | "Links" => "Copy text",
        "Images" => "Copy image",
        _ => "Copy",
    }
}
fn custom_resize(ctx: &Context) {
    let r = ctx.screen_rect();
    let e = 6.;
    use egui::ResizeDirection::*;
    for (direction, rect, cursor) in [
        (
            NorthWest,
            Rect::from_min_size(r.min, vec2(e, e)),
            CursorIcon::ResizeNwSe,
        ),
        (
            NorthEast,
            Rect::from_min_size(pos2(r.right() - e, r.top()), vec2(e, e)),
            CursorIcon::ResizeNeSw,
        ),
        (
            SouthWest,
            Rect::from_min_size(pos2(r.left(), r.bottom() - e), vec2(e, e)),
            CursorIcon::ResizeNeSw,
        ),
        (
            SouthEast,
            Rect::from_min_size(r.max - vec2(e, e), vec2(e, e)),
            CursorIcon::ResizeNwSe,
        ),
        (
            North,
            Rect::from_min_max(r.min + vec2(e, 0.), pos2(r.right() - e, r.top() + e)),
            CursorIcon::ResizeVertical,
        ),
        (
            South,
            Rect::from_min_max(pos2(r.left() + e, r.bottom() - e), r.max - vec2(e, 0.)),
            CursorIcon::ResizeVertical,
        ),
        (
            West,
            Rect::from_min_max(r.min + vec2(0., e), pos2(r.left() + e, r.bottom() - e)),
            CursorIcon::ResizeHorizontal,
        ),
        (
            East,
            Rect::from_min_max(pos2(r.right() - e, r.top() + e), r.max - vec2(0., e)),
            CursorIcon::ResizeHorizontal,
        ),
    ] {
        Area::new(Id::new(("resize", format!("{direction:?}"))))
            .movable(false)
            .constrain(false)
            .default_size(rect.size())
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());
                let response = response.on_hover_cursor(cursor);
                if response.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                }
            });
    }
}
fn set_startup(enabled: bool, root: &std::path::Path) -> Result<(), String> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let dir = config.join("autostart");
    let file = dir.join("clipvault.desktop");
    if file.exists() {
        let old = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
        if !old.lines().any(|line| line == "Name=ClipVault") {
            return Err(format!(
                "An unrelated autostart entry already exists at {}; it was left unchanged",
                file.display()
            ));
        }
    }
    if !enabled {
        if file.exists() {
            std::fs::remove_file(file).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let exe = crate::desktop::executable()?;
    let escape = |s: &str| {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%")
    };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text=format!("[Desktop Entry]\nType=Application\nName=ClipVault\nComment=Private clipboard history\nExec=\"{}\" --autostart --data-dir \"{}\"\nIcon=clipvault\nStartupWMClass=clipvault\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",escape(&exe.to_string_lossy()),escape(&root.to_string_lossy()));
    let temp = file.with_extension("desktop.tmp");
    std::fs::write(&temp, text)
        .and_then(|_| std::fs::rename(temp, file))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Snapshot, Store};
    use std::sync::{mpsc, Arc, Mutex};
    fn app() -> (VaultApp, Receiver<Command>, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "cv-ui-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let mut s = Store::open(root.clone()).unwrap();
        s.settings.changelog_dismissed_version = env!("CARGO_PKG_VERSION").into();
        s.save_settings().unwrap();
        s.ingest(
            &Snapshot::from([(
                "text/plain".into(),
                b"Saved text\nFull text content".to_vec(),
            )]),
            "test",
        )
        .unwrap();
        drop(s); // Exercise selecting a clip loaded from the existing database.
        let s = Arc::new(Mutex::new(Store::open(root.clone()).unwrap()));
        let (tx, rx) = mpsc::channel();
        let (_, notice) = mpsc::channel();
        let (_, ipc) = mpsc::channel();
        let ctx = Context::default();
        (
            VaultApp::create(&ctx, s, Service { tx, rx: notice }, ipc, None, false),
            rx,
            root,
        )
    }
    fn restore_command(rx: &Receiver<Command>, store: &Shared) -> Command {
        let command = rx.try_recv().expect("activation dispatch");
        let (out, _) = mpsc::channel();
        crate::clipboard::resolve_restore(command, store, &out).expect("readable original payload")
    }
    fn click_row(app: &mut VaultApp, ctx: &Context, item: &Item, pos: Pos2) {
        for pressed in [None, Some(true), Some(false)] {
            let mut input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(440., 420.))),
                ..Default::default()
            };
            if let Some(pressed) = pressed {
                input.events = vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ];
            }
            let _ = ctx.run(input, |ctx| {
                let compact = app.store.lock().unwrap().settings.compact;
                CentralPanel::default().show(ctx, |ui| app.row(ui, ctx, item, compact));
            });
        }
    }
    fn saved_image(app: &mut VaultApp) -> Item {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(23, 17)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut store = app.store.lock().unwrap();
        store
            .ingest(
                &Snapshot::from([("image/png".into(), png.into_inner())]),
                "test",
            )
            .unwrap();
        store.items[0].clone()
    }
    fn text_position(output: &FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing control: {label}"))
    }
    fn click_at(ctx: &Context, pos: Pos2, mut render: impl FnMut(&Context)) {
        for pressed in [true, false] {
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
                    events: vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| render(ctx),
            );
        }
    }
    #[test]
    fn appearance_choices_save_across_restarts_without_changing_clips() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let image = saved_image(&mut app);
        app.pin(&image);
        let before = serde_json::to_value(&app.store.lock().unwrap().items).unwrap();
        let payloads: Vec<_> = {
            let store = app.store.lock().unwrap();
            store
                .items
                .iter()
                .map(|i| (i.id.clone(), store.snapshot(i).unwrap()))
                .collect()
        };
        app.settings_open = true;
        app.settings_tab = 1;
        for (label, compact) in [("Comfortable", false), ("Compact", true)] {
            let mut output = FullOutput::default();
            for _ in 0..3 {
                output = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
                        ..Default::default()
                    },
                    |ctx| app.render(ctx),
                );
            }
            text_position(&output, "Compact shows just the clip content. Comfortable adds one full-content preview on the right.");
            click_at(&ctx, text_position(&output, label), |ctx| app.render(ctx));
            let restored = Store::open(root.clone()).unwrap();
            assert_eq!(restored.settings.compact, compact);
            assert_eq!(serde_json::to_value(&restored.items).unwrap(), before);
            for (id, data) in &payloads {
                let item = restored.items.iter().find(|i| &i.id == id).unwrap();
                assert_eq!(&restored.snapshot(item).unwrap(), data);
            }
        }
        assert!(
            rx.try_recv().is_err(),
            "appearance changes must not touch the clipboard"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn compact_menu_keeps_pin_edit_and_protected_delete_accessible() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        for action in ["Pin", "Edit", "Delete"] {
            let item = app.store.lock().unwrap().items[0].clone();
            let render = |app: &mut VaultApp, ctx: &Context| {
                CentralPanel::default().show(ctx, |ui| app.row(ui, ctx, &item, true));
            };
            let mut output = FullOutput::default();
            for _ in 0..3 {
                output = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
                        ..Default::default()
                    },
                    |ctx| render(&mut app, ctx),
                );
            }
            click_at(&ctx, text_position(&output, "···"), |ctx| {
                render(&mut app, ctx)
            });
            for _ in 0..3 {
                output = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
                        ..Default::default()
                    },
                    |ctx| render(&mut app, ctx),
                );
            }
            assert!(!output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, Shape::Text(t) if t.galley.job.text == "Preview")));
            click_at(&ctx, text_position(&output, action), |ctx| {
                render(&mut app, ctx)
            });
            match action {
                "Pin" => assert!(app.store.lock().unwrap().items[0].pinned),
                "Edit" => {
                    assert_eq!(
                        app.editor.as_ref().unwrap().1,
                        "Saved text\nFull text content"
                    );
                    app.editor = None;
                }
                "Delete" => assert!(app.confirm_delete.is_some()),
                _ => unreachable!(),
            }
            assert!(
                rx.try_recv().is_err(),
                "menu actions must not activate a clip"
            );
            assert!(!app.hidden);
        }
        assert!(app.store.lock().unwrap().items[0].pinned);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn appearance_layouts_keep_search_filters_content_and_thumbnails() {
        let (mut app, rx, root) = app();
        let text = app.store.lock().unwrap().items[0].clone();
        let image = saved_image(&mut app);
        {
            let mut store = app.store.lock().unwrap();
            for value in ["https://example.com/clip", "file:///tmp/report.pdf"] {
                store
                    .ingest(
                        &Snapshot::from([("text/plain".into(), value.as_bytes().to_vec())]),
                        "test",
                    )
                    .unwrap();
            }
        }
        for compact in [true, false] {
            for light in [false, true] {
                for width in [440u32, 820] {
                    let ctx = Context::default();
                    app.store.lock().unwrap().settings.compact = compact;
                    app.store.lock().unwrap().settings.light = light;
                    app.selected = Some(text.id.clone());
                    app.preview = if compact { Some(text.clone()) } else { None };
                    app.textures.clear();
                    let mut textures = HashMap::new();
                    for frame in 0..3 {
                        let output = ctx.run(
                            RawInput {
                                screen_rect: Some(Rect::from_min_size(
                                    Pos2::ZERO,
                                    vec2(width as f32, 620.),
                                )),
                                ..Default::default()
                            },
                            |ctx| {
                                app.render(ctx);
                                assert!(ctx.used_size().x <= width as f32 + 1.);
                            },
                        );
                        let labels: Vec<_> = output
                            .shapes
                            .iter()
                            .filter_map(|s| match &s.shape {
                                Shape::Text(t) => Some(t.galley.job.text.as_str()),
                                _ => None,
                            })
                            .collect();
                        for filter in ["All", "Text", "Images", "Links", "Files", "Pinned"] {
                            assert!(labels.contains(&filter));
                        }
                        assert!(labels.contains(&"Search clips…"));
                        assert!(labels.iter().any(|t| t.contains("example.com/clip")));
                        assert!(labels.iter().any(|t| t.contains("report.pdf")));
                        if compact {
                            assert!(app.preview.is_none());
                            assert!(labels.contains(&"Saved text Full text content"));
                            assert!(!labels.iter().any(|t| t.contains("PREVIEW")
                                || t.contains("Saved formats:")
                                || t.contains("Copied image")));
                        } else {
                            assert!(labels.contains(&"Copy text"));
                            assert!(labels.contains(&"Saved text\nFull text content"));
                            assert!(!labels
                                .iter()
                                .any(|t| t.contains("PREVIEW") || t.contains("Saved formats:")));
                        }
                        assert!(
                            app.textures.contains_key(&image.id),
                            "saved image thumbnail must load"
                        );
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                            "target/layout/appearance-{}-{}-{width}.png",
                            if compact { "compact" } else { "comfortable" },
                            if light { "light" } else { "dark" }
                        ));
                        if frame == 2 {
                            crate::ui_capture::capture(
                                &ctx,
                                output,
                                &mut textures,
                                [width, 620],
                                &path,
                            );
                        } else {
                            crate::ui_capture::capture(
                                &ctx,
                                output,
                                &mut textures,
                                [width, 620],
                                &path.with_file_name("appearance-warmup.png"),
                            );
                        }
                    }
                }
            }
        }
        // Filtering is still available in the content-only mode.
        app.store.lock().unwrap().settings.compact = true;
        for (category, query, expected) in [
            ("Links", "example.com", "https://example.com/clip"),
            ("Files", "report", "report.pdf"),
        ] {
            app.category = category.into();
            app.query = query.into();
            let ctx = Context::default();
            let output = ctx.run(RawInput::default(), |ctx| app.render(ctx));
            assert!(output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, Shape::Text(t) if t.galley.job.text == expected)));
            let item = app
                .store
                .lock()
                .unwrap()
                .items
                .iter()
                .find(|i| i.kind == category)
                .unwrap()
                .clone();
            let expected_data = app.store.lock().unwrap().snapshot(&item).unwrap();
            app.activate(&item);
            let Command::Restore(id, data) = restore_command(&rx, &app.store) else {
                panic!("restore expected")
            };
            assert_eq!(id, item.id);
            assert_eq!(data, expected_data);
            app.pending_activation = None;
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    fn activation_visibility(image: bool, hide: bool) {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let item = if image {
            saved_image(&mut app)
        } else {
            app.store.lock().unwrap().items[0].clone()
        };
        let (data, count) = {
            let mut store = app.store.lock().unwrap();
            store.pin(&item.id).unwrap();
            store
                .ingest(
                    &Snapshot::from([("text/plain".into(), b"newer clip".to_vec())]),
                    "test",
                )
                .unwrap();
            store.settings.hide_after_copy = hide;
            store.save_settings().unwrap();
            (store.snapshot(&item).unwrap(), store.items.len())
        };
        // Read text/image payloads from disk after a database reopen, not a demo cache.
        *app.store.lock().unwrap() = Store::open(root.clone()).unwrap();
        app.activate(&item);
        let Command::Restore(id, restored) = restore_command(&rx, &app.store) else {
            panic!("restore expected")
        };
        assert_eq!(id, item.id);
        assert_eq!(restored, data);
        assert!(!app.hidden, "request alone must never hide");
        assert_eq!(app.pending_activation.as_deref(), Some(item.id.as_str()));
        app.store.lock().unwrap().touch(&id).unwrap(); // Simulated service acknowledgement.
        let output = ctx.run(RawInput::default(), |ctx| {
            app.handle_notice(ctx, Notice::Activated(id.clone()));
            app.render(ctx);
        });
        assert_eq!(app.hidden, hide);
        assert!(!app.quitting);
        assert!(app.pending_activation.is_none());
        assert!(app.error.is_none());
        assert!(app.toast.as_ref().unwrap().0.starts_with("Copied"));
        assert_eq!(app.selected.as_deref(), Some(id.as_str()));
        let commands = &output.viewport_output[&ViewportId::ROOT].commands;
        assert_eq!(
            commands
                .iter()
                .any(|c| matches!(c, ViewportCommand::Visible(false))),
            hide
        );
        assert!(!commands.iter().any(|c| matches!(c, ViewportCommand::Close)));
        assert!(
            rx.try_recv().is_err(),
            "hiding must not stop the clipboard service"
        );
        let store = app.store.lock().unwrap();
        assert_eq!(store.items.len(), count);
        assert_eq!(store.items[0].id, id);
        assert!(store.items[0].pinned);
        assert_eq!(store.snapshot(&store.items[0]).unwrap(), data);
        drop(store);
        drop(app);
        let store = Store::open(root.clone()).unwrap();
        assert_eq!(store.settings.hide_after_copy, hide);
        assert!(store.items[0].pinned);
        assert_eq!(store.items[0].id, id);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn text_copy_hide_off_keeps_window_and_confirms_copied() {
        activation_visibility(false, false);
    }
    #[test]
    fn text_copy_hide_on_waits_for_success() {
        activation_visibility(false, true);
    }
    #[test]
    fn image_copy_hide_off_keeps_window_and_confirms_copied() {
        activation_visibility(true, false);
    }
    #[test]
    fn image_copy_hide_on_waits_for_success() {
        activation_visibility(true, true);
    }
    #[test]
    fn text_and_image_failures_keep_window_open_for_both_preferences() {
        for image in [false, true] {
            for hide in [false, true] {
                let (mut app, rx, root) = app();
                let ctx = Context::default();
                let item = if image {
                    saved_image(&mut app)
                } else {
                    app.store.lock().unwrap().items[0].clone()
                };
                app.store.lock().unwrap().settings.hide_after_copy = hide;
                app.activate(&item);
                assert!(matches!(
                    restore_command(&rx, &app.store),
                    Command::Restore(_, _)
                ));
                // Ownership and database errors take the same explicit failure path.
                for error in ["Could not take clipboard ownership", "Clipboard ready, but recency could not be saved: SQLite could not save clipboard history"] {
                    app.handle_notice(&ctx, Notice::ActivationFailed(item.id.clone(), error.into()));
                    assert!(!app.hidden);
                    assert_eq!(app.error.as_deref(), Some(error));
                    assert!(app.toast.is_none());
                    app.activate(&item);
                    assert!(matches!(restore_command(&rx,&app.store), Command::Restore(_, _)));
                }
                app.handle_notice(
                    &ctx,
                    Notice::ActivationFailed(item.id.clone(), "failed".into()),
                );
                std::fs::remove_file(root.join(&item.formats[0].file)).unwrap();
                app.activate(&item);
                assert!(app
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("Could not read the saved clip"));
                assert!(!app.hidden);
                assert!(rx.try_recv().is_err());
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
    #[test]
    fn stale_acknowledgements_and_deleted_items_do_not_hide_or_copy() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let item = app.store.lock().unwrap().items[0].clone();
        app.store.lock().unwrap().settings.hide_after_copy = true;
        app.activate(&item);
        app.activate(&item);
        assert!(matches!(
            restore_command(&rx, &app.store),
            Command::Restore(_, _)
        ));
        assert!(
            rx.try_recv().is_err(),
            "only one restore can be outstanding"
        );
        app.handle_notice(&ctx, Notice::Activated("unrelated-id".into()));
        assert!(!app.hidden);
        assert!(app.pending_activation.is_some());
        app.handle_notice(
            &ctx,
            Notice::ActivationFailed(item.id.clone(), "failed".into()),
        );
        app.handle_notice(&ctx, Notice::Activated(item.id.clone()));
        assert!(!app.hidden, "a stale success after failure must never hide");
        app.store.lock().unwrap().delete(&item.id).unwrap();
        app.activate(&item);
        assert!(app.error.as_ref().unwrap().contains("no longer in history"));
        assert!(rx.try_recv().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hiding_shortcut_and_repeat_launch_do_not_quit_or_pause() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let output = ctx.run(RawInput::default(), |ctx| app.hide(ctx));
        assert!(app.hidden);
        assert!(output.viewport_output[&ViewportId::ROOT]
            .commands
            .iter()
            .any(|c| matches!(c, ViewportCommand::Visible(false))));
        app.handle_notice(&ctx, Notice::Open);
        assert!(
            !app.hidden,
            "configured shortcut notice reopens the hidden window"
        );
        app.hide(&ctx);
        app.handle_ipc(&ctx, "show");
        assert!(!app.hidden, "another launch shows the same app");
        app.hide(&ctx);
        app.handle_ipc(&ctx, "toggle");
        assert!(matches!(rx.try_iter().last(), Some(Command::Toggle)));
        app.handle_notice(&ctx, Notice::Open);
        assert!(!app.hidden);
        assert!(!app.store.lock().unwrap().settings.paused);
        assert!(!app.quitting);
        assert!(rx.try_iter().all(|c| matches!(c, Command::RememberTarget)));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn header_gear_next_to_close_toggles_settings_and_close_only_hides() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let item = app.store.lock().unwrap().items[0].clone();
        app.store.lock().unwrap().settings.system_title_bar = false;
        app.preview = Some(item.clone());
        app.editor = Some((item, "draft".into()));
        for expected in [true, false] {
            // Gear is immediately to the left of Close, with no menu in between.
            for pressed in [None, Some(true), Some(false)] {
                let mut input = RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(440., 420.))),
                    ..Default::default()
                };
                if let Some(pressed) = pressed {
                    let pos = pos2(315., 24.);
                    input.events = vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ];
                }
                let output = ctx.run(input, |ctx| {
                    CentralPanel::default().show(ctx, |ui| app.header(ui, ctx));
                });
                assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, Shape::Text(text) if text.galley.job.text.contains('☰'))));
            }
            assert_eq!(app.settings_open, expected);
            assert!(app.preview.is_none());
            assert!(app.editor.is_none());
            assert!(!app.hidden);
        }
        for pressed in [true, false] {
            let pos = pos2(417., 24.);
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(440., 420.))),
                events: vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| app.header(ui, ctx));
            });
        }
        assert!(app.hidden);
        assert!(!app.quitting);
        assert!(!app.store.lock().unwrap().settings.paused);
        assert!(rx.try_recv().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn row_pin_menu_preview_edit_and_delete_do_not_copy_or_hide() {
        for image in [false, true] {
            let (mut app, rx, root) = app();
            let ctx = Context::default();
            let item = if image {
                saved_image(&mut app)
            } else {
                app.store.lock().unwrap().items[0].clone()
            };
            app.store.lock().unwrap().settings.hide_after_copy = true;
            app.store.lock().unwrap().settings.compact = false;
            click_row(&mut app, &ctx, &item, pos2(366., 35.));
            assert!(
                app.store
                    .lock()
                    .unwrap()
                    .items
                    .iter()
                    .find(|i| i.id == item.id)
                    .unwrap()
                    .pinned
            );
            assert!(rx.try_recv().is_err(), "row pin must not bubble into copy");
            click_row(&mut app, &ctx, &item, pos2(414., 35.));
            assert!(rx.try_recv().is_err(), "opening the row menu must not copy");
            app.preview = Some(item.clone());
            if !image {
                app.edit(&item);
            }
            app.delete(&item); // Pinned deletion only opens confirmation.
            assert!(app.confirm_delete.is_some());
            assert!(!app.hidden);
            assert!(rx.try_recv().is_err());
            app.pin(&item);
            app.store.lock().unwrap().settings.confirm_before_delete = false;
            app.delete(&item);
            assert!(app
                .store
                .lock()
                .unwrap()
                .items
                .iter()
                .all(|i| i.id != item.id));
            assert!(!app.hidden);
            assert!(rx.try_recv().is_err());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn saved_text_click_regression_dispatches_exact_payload_without_panicking() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let item = app.store.lock().unwrap().items[0].clone();
        for pressed in [None, Some(true), Some(false)] {
            let mut input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(440., 420.))),
                ..Default::default()
            };
            if let Some(pressed) = pressed {
                input.events = vec![
                    Event::PointerMoved(pos2(80., 35.)),
                    Event::PointerButton {
                        pos: pos2(80., 35.),
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ];
            }
            let _ = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| app.row(ui, ctx, &item, true));
            });
        }
        let Command::Restore(id, data) = restore_command(&rx, &app.store) else {
            panic!("wrong command")
        };
        assert_eq!(id, item.id);
        assert_eq!(
            data.get("text/plain").unwrap(),
            b"Saved text\nFull text content"
        );
        assert!(!app.hidden);
        assert!(app.error.is_none()); // Hide only on service acknowledgement.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unreadable_saved_text_keeps_window_open_and_reports_error() {
        let (mut app, rx, root) = app();
        let item = app.store.lock().unwrap().items[0].clone();
        std::fs::remove_file(root.join(&item.formats[0].file)).unwrap();
        app.activate(&item);
        assert!(app.error.as_ref().unwrap().contains("Could not read"));
        assert!(!app.hidden);
        assert!(rx.try_recv().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn first_press_cycles_restore_minimized_and_recording_blocks_actions() {
        let (mut app, _rx, root) = app();
        let ctx = Context::default();
        for _ in 0..30 {
            app.hidden = true;
            app.handle_notice(&ctx, Notice::Open);
            assert!(!app.hidden);
            app.handle_notice(&ctx, Notice::Open);
            assert!(app.hidden);
            app.handle_ipc(&ctx, "toggle");
            app.handle_notice(&ctx, Notice::Open);
            assert!(!app.hidden);
            app.minimized = true;
            app.handle_ipc(&ctx, "toggle");
            app.handle_notice(&ctx, Notice::Open);
            assert!(!app.minimized);
            assert!(!app.hidden);
            app.handle_ipc(&ctx, "toggle");
            app.handle_notice(&ctx, Notice::Open);
            assert!(app.hidden);
        }
        app.shortcut_recording = true;
        app.handle_notice(&ctx, Notice::Open);
        app.handle_ipc(&ctx, "toggle");
        assert!(app.hidden);
        app.handle_notice(&ctx, Notice::RecordingCancelled);
        assert!(!app.shortcut_recording);
        app.handle_ipc(&ctx, "show");
        assert!(!app.hidden);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn registration_feedback_is_confirmed_and_update_preferences_persist() {
        let (mut app, _rx, root) = app();
        let ctx = Context::default();
        assert!(app.active_shortcut.is_none());
        app.handle_notice(&ctx, Notice::ShortcutActive("Super+V".into()));
        app.handle_notice(&ctx, Notice::ShortcutFailed("Conflict".into()));
        assert_eq!(app.active_shortcut.as_deref(), Some("Super+V"));
        app.handle_notice(&ctx, Notice::ShortcutInactive);
        assert!(app.active_shortcut.is_none());
        app.poll_updates(&ctx);
        assert!(!app.checker.running());
        assert!(app
            .store
            .lock()
            .unwrap()
            .settings
            .updates
            .last_attempt
            .is_none());
        app.mutate(|s| {
            s.settings.updates.automatic = true;
            s.settings.updates.last_attempt = Some(now());
            s.settings.updates.dismissed_version = Some("2.0.0".into());
            s.save_settings()
        });
        app.poll_updates(&ctx);
        assert!(!app.checker.running());
        let restored = Store::open(root.clone()).unwrap();
        assert!(restored.settings.updates.automatic);
        assert_eq!(
            restored.settings.updates.dismissed_version.as_deref(),
            Some("2.0.0")
        );
        assert_eq!(restored.items.len(), app.store.lock().unwrap().items.len());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn about_and_shortcuts_themes_and_responsive_layout_artifacts() {
        let (original, _rx, root) = app();
        for light in [false, true] {
            for width in [440u32, 820] {
                for tab in [2, 4] {
                    let ctx = Context::default();
                    let (tx, _) = mpsc::channel();
                    let (_, rx) = mpsc::channel();
                    let (_, ipc) = mpsc::channel();
                    let mut app = VaultApp::create(
                        &ctx,
                        original.store.clone(),
                        Service { tx, rx },
                        ipc,
                        None,
                        false,
                    );
                    app.store.lock().unwrap().settings.light = light;
                    app.settings_open = true;
                    app.settings_tab = tab;
                    let mut textures = HashMap::new();
                    for frame in 0..3 {
                        let mut used = Vec2::ZERO;
                        let output = ctx.run(
                            RawInput {
                                screen_rect: Some(Rect::from_min_size(
                                    Pos2::ZERO,
                                    vec2(width as f32, 1050.),
                                )),
                                ..Default::default()
                            },
                            |ctx| {
                                app.render(ctx);
                                used = ctx.used_size();
                            },
                        );
                        assert!(used.x <= width as f32 + 1., "overflow width={width} tab={tab} light={light} frame={frame} size={:?}",used);
                        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                            "target/layout/{}-{}-{width}.png",
                            if tab == 4 { "about" } else { "shortcuts" },
                            if light { "light" } else { "dark" }
                        ));
                        if frame == 2 {
                            crate::ui_capture::capture(
                                &ctx,
                                output,
                                &mut textures,
                                [width, 1050],
                                &path,
                            );
                        } else {
                            // Keep the font atlas and logo deltas between passes.
                            crate::ui_capture::capture(
                                &ctx,
                                output,
                                &mut textures,
                                [width, 1050],
                                &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                    .join("target/layout/warmup.png"),
                            );
                        }
                    }
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn small_large_layouts_and_all_settings_pages_render() {
        let (mut app, _rx, root) = app();
        let ctx = Context::default();
        for size in [vec2(440., 420.), vec2(820., 620.), vec2(1200., 850.)] {
            for tab in 0_usize..6 {
                app.settings_open = tab > 0;
                app.settings_tab = tab.saturating_sub(1);
                let _ = ctx.run(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| {
                        app.render(ctx);
                        assert!(
                            ctx.used_size().x <= size.x + 1.,
                            "horizontal overflow at {size:?}, tab {tab}: {:?}",
                            ctx.used_size()
                        );
                    },
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pinned_delete_requires_confirmation_and_cancel_changes_nothing() {
        let (mut app, _rx, root) = app();
        // A preview opened before pinning must still protect the saved pin.
        let item = app.store.lock().unwrap().items[0].clone();
        let id = item.id.clone();
        app.mutate(|s| s.pin(&id));
        app.delete(&item);
        assert!(app.confirm_delete.is_some());
        assert_eq!(app.store.lock().unwrap().items.len(), 1);
        app.confirm_delete = None;
        assert!(app.store.lock().unwrap().items[0].pinned);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn enter_on_a_focused_control_does_not_activate_history() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let _ = ctx.run(RawInput::default(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                ui.button("Pin action").request_focus();
            });
        });
        assert!(ctx.memory(|m| m.focused()).is_some());
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
            events: vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.render(ctx));
        assert!(
            rx.try_recv().is_err(),
            "Enter on an action must not restore a clip"
        );
        assert!(!app.hidden);
        std::fs::remove_dir_all(root).unwrap();
    }
    fn frame(app: &mut VaultApp, ctx: &Context) -> FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
                ..Default::default()
            },
            |ctx| app.render(ctx),
        )
    }
    #[test]
    fn v120_delete_confirmation_menu_preview_and_keyboard_on_and_off() {
        for confirm in [true, false] {
            for pinned in [true, false] {
                for source in ["menu", "preview", "keyboard"] {
                    let (mut app, rx, root) = app();
                    let ctx = Context::default();
                    let item = {
                        let mut store = app.store.lock().unwrap();
                        store.settings.confirm_before_delete = confirm;
                        store.settings.compact = false;
                        let id = store.items[0].id.clone();
                        if pinned {
                            store.pin(&id).unwrap();
                        }
                        store.items[0].clone()
                    };
                    frame(&mut app, &ctx);
                    match source {
                        "menu" => {
                            let render = |app: &mut VaultApp, ctx: &Context| {
                                CentralPanel::default()
                                    .show(ctx, |ui| app.row(ui, ctx, &item, true));
                            };
                            let output = ctx.run(RawInput::default(), |ctx| render(&mut app, ctx));
                            click_at(&ctx, text_position(&output, "···"), |ctx| {
                                render(&mut app, ctx)
                            });
                            let output = ctx.run(RawInput::default(), |ctx| render(&mut app, ctx));
                            click_at(&ctx, text_position(&output, "Delete"), |ctx| {
                                render(&mut app, ctx)
                            });
                        }
                        "preview" => {
                            let output = ctx.run(RawInput::default(), |ctx| {
                                CentralPanel::default()
                                    .show(ctx, |ui| app.detail(ui, ctx, &item, false));
                            });
                            click_at(&ctx, text_position(&output, "Delete"), |ctx| {
                                CentralPanel::default()
                                    .show(ctx, |ui| app.detail(ui, ctx, &item, false));
                            });
                        }
                        _ => {
                            ctx.memory_mut(|m| m.surrender_focus(Id::new("search")));
                            let _ = ctx.run(
                                RawInput {
                                    events: vec![Event::Key {
                                        key: Key::Delete,
                                        physical_key: None,
                                        pressed: true,
                                        repeat: false,
                                        modifiers: Modifiers::NONE,
                                    }],
                                    ..Default::default()
                                },
                                |ctx| app.render(ctx),
                            );
                        }
                    }
                    assert_eq!(
                        app.confirm_delete.is_some(),
                        confirm,
                        "source={source} pinned={pinned}"
                    );
                    assert_eq!(app.store.lock().unwrap().items.len(), usize::from(confirm));
                    if confirm {
                        let mut output = frame(&mut app, &ctx);
                        for _ in 0..2 {
                            output = frame(&mut app, &ctx);
                        }
                        text_position(&output, "Delete this clip?");
                        click_at(&ctx, text_position(&output, "Cancel"), |ctx| {
                            app.render(ctx)
                        });
                        assert!(app.confirm_delete.is_none());
                        assert_eq!(app.store.lock().unwrap().items.len(), 1);
                        app.delete(&item);
                        let output = frame(&mut app, &ctx);
                        let delete = output
                            .shapes
                            .iter()
                            .filter_map(|s| match &s.shape {
                                Shape::Text(t) if t.galley.job.text == "Delete" => {
                                    Some(t.pos + t.galley.size() * 0.5)
                                }
                                _ => None,
                            })
                            .next_back()
                            .unwrap();
                        click_at(&ctx, delete, |ctx| app.render(ctx));
                        assert!(app.confirm_delete.is_none());
                        assert!(app.store.lock().unwrap().items.is_empty());
                    }
                    assert!(rx.try_recv().is_err());
                    std::fs::remove_dir_all(root).unwrap();
                }
            }
        }
    }
    #[test]
    fn v120_changelog_waits_for_first_open_dismisses_persistently_and_reopens_in_about() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        app.store
            .lock()
            .unwrap()
            .settings
            .changelog_dismissed_version = "1.1.0".into();
        app.hidden = true;
        frame(&mut app, &ctx);
        assert!(!app.changelog_open && !app.changelog_considered);
        app.handle_notice(&ctx, Notice::Open);
        frame(&mut app, &ctx);
        assert!(app.changelog_open);
        let output = frame(&mut app, &ctx);
        text_position(&output, "What’s New in ClipVault v2.0.0");
        let output = frame(&mut app, &ctx);
        click_at(&ctx, text_position(&output, "Done"), |ctx| app.render(ctx));
        assert!(!app.changelog_open);
        let store = Store::open(root.clone()).unwrap();
        assert_eq!(
            store.settings.changelog_dismissed_version,
            env!("CARGO_PKG_VERSION")
        );
        app.changelog_considered = false;
        frame(&mut app, &ctx);
        assert!(!app.changelog_open);
        app.settings_open = true;
        app.settings_tab = 4;
        let mut output = frame(&mut app, &ctx);
        for _ in 0..2 {
            output = frame(&mut app, &ctx);
        }
        click_at(&ctx, text_position(&output, "View changelog"), |ctx| {
            app.render(ctx)
        });
        assert!(app.changelog_open);
        app.dismiss_changelog();
        app.settings_open = false;
        app.changelog_considered = false;
        app.store
            .lock()
            .unwrap()
            .settings
            .changelog_dismissed_version = "1.0.0".into();
        frame(&mut app, &ctx);
        assert!(app.changelog_open, "another upgrade should show it again");
        assert!(rx.try_iter().all(|c| matches!(c, Command::RememberTarget)));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v120_preview_copy_labels_dispatch_original_text_and_image_and_render_full_image() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1200, 700)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        app.store
            .lock()
            .unwrap()
            .ingest(
                &Snapshot::from([("image/png".into(), png.into_inner())]),
                "test",
            )
            .unwrap();
        let items = app.store.lock().unwrap().items.clone();
        for item in items {
            let render = |app: &mut VaultApp, ctx: &Context| {
                CentralPanel::default().show(ctx, |ui| app.detail(ui, ctx, &item, false));
            };
            let output = ctx.run(RawInput::default(), |ctx| render(&mut app, ctx));
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    Shape::Text(t) => Some(t.galley.job.text.as_str()),
                    _ => None,
                })
                .collect();
            assert!(
                !labels.contains(&item.title.as_str()),
                "no repeated preview title"
            );
            if item.kind == "Images" {
                let deadline = Instant::now() + Duration::from_secs(3);
                while app.full_image(&ctx, &item).is_none() {
                    assert!(
                        Instant::now() < deadline,
                        "async image decoding did not finish"
                    );
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(
                    app.textures[&format!("full-{}", item.id)].size(),
                    [1200, 700]
                );
                assert!(!labels.contains(&"Edit"));
            } else {
                assert_eq!(
                    labels
                        .iter()
                        .filter(|&&s| s == "Saved text\nFull text content")
                        .count(),
                    1
                );
                assert!(labels.contains(&"Edit"));
            }
            click_at(&ctx, text_position(&output, copy_label(&item)), |ctx| {
                render(&mut app, ctx)
            });
            let Command::Restore(id, data) = restore_command(&rx, &app.store) else {
                panic!("copy must restore")
            };
            assert_eq!(id, item.id);
            assert_eq!(data, app.store.lock().unwrap().snapshot(&item).unwrap());
            app.pending_activation = None;
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v120_scrollbar_lane_is_outside_menu_buttons_at_all_widths() {
        let (mut app, _rx, root) = app();
        let mut items = vec![app.store.lock().unwrap().items[0].clone(); 40];
        for (n, item) in items.iter_mut().enumerate() {
            item.id = format!("layout-{n}");
            item.pinned = n % 2 == 0;
        }
        for width in [440., 820., 1200.] {
            for compact in [true, false] {
                let ctx = Context::default();
                ctx.style_mut(|s| {
                    s.spacing.scroll = egui::style::ScrollStyle::solid();
                    s.spacing.button_padding = vec2(11., 8.);
                });
                let mut inner = Rect::NOTHING;
                let mut outer = Rect::NOTHING;
                for _ in 0..3 {
                    let output = ctx.run(
                        RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 420.))),
                            ..Default::default()
                        },
                        |ctx| {
                            CentralPanel::default().show(ctx, |ui| {
                                if !compact {
                                    ui.set_width((width - 36.) / 2. - 8.);
                                }
                                outer = ui.available_rect_before_wrap();
                                let output = app.history_list(ui, ctx, &items);
                                inner = output.inner_rect;
                                assert!(output.content_size.y > output.inner_rect.height());
                            });
                        },
                    );
                    let menus: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|s| match &s.shape {
                            Shape::Text(t) if t.galley.job.text == "···" => {
                                Some((t.pos.x, t.galley.size().x))
                            }
                            _ => None,
                        })
                        .collect();
                    assert!(!menus.is_empty());
                    for (x, w) in menus {
                        assert!(x+w+11.<=inner.right(),"button must fit content rect: width={width} compact={compact} x={x} w={w} inner={inner:?}");
                    }
                    assert!(
                        outer.right() - inner.right() >= ctx.style().spacing.scroll.bar_width,
                        "scrollbar must reserve its own lane"
                    );
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v120_title_bar_commands_and_font_choice_persist() {
        let (mut app, _rx, root) = app();
        let ctx = Context::default();
        for native in [false, true] {
            app.store.lock().unwrap().settings.system_title_bar = native;
            let output = frame(&mut app, &ctx);
            assert!(output.viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::Decorations(native)));
        }
        let font = app
            .installed_fonts
            .first()
            .map(|f| f.family.clone())
            .unwrap_or_default();
        app.mutate(|s| {
            s.settings.app_font = font.clone();
            s.settings.system_title_bar = false;
            s.settings.start_hidden = true;
            s.settings.confirm_before_delete = false;
            s.save_settings()
        });
        frame(&mut app, &ctx);
        assert_eq!(app.applied_font, font);
        let restored = Store::open(root.clone()).unwrap();
        assert_eq!(restored.settings.app_font, font);
        assert!(!restored.settings.system_title_bar);
        assert!(restored.settings.start_hidden);
        assert!(!restored.settings.confirm_before_delete);
        app.store.lock().unwrap().settings.app_font = "Unavailable family".into();
        frame(&mut app, &ctx);
        assert!(!app.font_available);
        app.store.lock().unwrap().settings.app_font.clear();
        frame(&mut app, &ctx);
        assert!(app.font_available);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v120_custom_resize_edges_send_compositor_resize_requests() {
        let ctx = Context::default();
        let input = |events| RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
            events,
            ..Default::default()
        };
        for _ in 0..3 {
            let _ = ctx.run(input(vec![]), custom_resize);
        }
        let start = pos2(3., 150.);
        let press = ctx.run(
            input(vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]),
            custom_resize,
        );
        let output = ctx.run(
            input(vec![Event::PointerMoved(start + vec2(12., 0.))]),
            custom_resize,
        );
        assert!([press, output]
            .iter()
            .any(|output| output.viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::BeginResize(egui::ResizeDirection::West))));
    }
    #[test]
    fn v120_pin_states_differ_in_shape_and_color_and_clear_still_confirms() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let mut item = app.store.lock().unwrap().items[0].clone();
        let draw = |app: &mut VaultApp, ctx: &Context, item: &Item| {
            ctx.run(RawInput::default(), |ctx| {
                CentralPanel::default().show(ctx, |ui| app.row(ui, ctx, item, true));
            })
        };
        let outline = draw(&mut app, &ctx, &item);
        item.pinned = true;
        let filled = draw(&mut app, &ctx, &item);
        let accent_paths = |o: &FullOutput| {
            o.shapes
                .iter()
                .filter(|s| matches!(&s.shape,Shape::Path(p) if p.fill==ACCENT))
                .count()
        };
        assert_eq!(accent_paths(&outline), 0);
        assert_eq!(accent_paths(&filled), 1);
        app.store.lock().unwrap().settings.confirm_before_delete = false;
        let output = frame(&mut app, &ctx);
        click_at(&ctx, text_position(&output, "Clear…"), |ctx| {
            app.render(ctx)
        });
        assert!(app.confirm_clear);
        assert_eq!(app.store.lock().unwrap().items.len(), 1);
        assert!(rx.try_recv().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "explicit reproducible performance workload; no desktop claims"]
    fn performance_large_history() {
        let (mut app, _commands, root) = app();
        let ctx = Context::default();
        let mut data = Snapshot::new();
        data.insert("text/plain".into(), b"Performance fixture".to_vec());
        app.store
            .lock()
            .unwrap()
            .ingest(&data, "benchmark")
            .unwrap();
        let item = app.store.lock().unwrap().items[0].clone();
        {
            let mut store = app.store.lock().unwrap();
            store.settings.compact = true;
            for n in 1..2000 {
                let mut next = item.clone();
                next.id = format!("fixture-{n}");
                next.preview = "Clipboard history performance fixture. ".repeat(28);
                store.items.push(next);
            }
        }
        let start = Instant::now();
        for _ in 0..30 {
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(820., 620.))),
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
        }
        println!(
            "2000 metadata records, 30 frames: {} ms",
            start.elapsed().as_millis()
        );
        println!(
            "{}",
            std::fs::read_to_string("/proc/self/status")
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("VmRSS:") || l.starts_with("VmHWM:"))
                .collect::<Vec<_>>()
                .join("; ")
        );
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn v200_row_activation_pastes_only_after_restore_and_keeps_identity() {
        for image in [false, true] {
            let (mut app, rx, root) = app();
            let ctx = Context::default();
            let item = if image {
                saved_image(&mut app)
            } else {
                app.store.lock().unwrap().items[0].clone()
            };
            app.store.lock().unwrap().pin(&item.id).unwrap();
            click_row(&mut app, &ctx, &item, pos2(80., 35.));
            assert!(
                matches!(restore_command(&rx,&app.store),Command::Restore(id,_) if id==item.id)
            );
            assert!(!app.hidden);
            assert!(rx.try_recv().is_err());
            app.store.lock().unwrap().touch(&item.id).unwrap();
            app.handle_notice(&ctx, Notice::Activated(item.id.clone()));
            assert!(app.hidden);
            assert!(matches!(rx.try_recv().unwrap(), Command::Paste));
            assert_eq!(app.selected.as_deref(), Some(item.id.as_str()));
            assert!(app.store.lock().unwrap().items[0].pinned);
            app.handle_notice(
                &ctx,
                Notice::PasteFeedback("Copied—press Ctrl+V to paste.".into()),
            );
            assert!(app.toast.as_ref().unwrap().0.contains("Ctrl+V"));
            fs_remove(root);
        }
    }
    fn fs_remove(root: PathBuf) {
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v200_desktop_configured_is_never_reported_as_active() {
        let (mut app, _rx, root) = app();
        let ctx = Context::default();
        app.handle_notice(&ctx, Notice::ShortcutConfigured("Super+V".into()));
        assert!(app.active_shortcut.is_none());
        assert_eq!(app.desktop_shortcut.as_deref(), Some("Super+V"));
        assert!(app
            .shortcut_feedback
            .contains("not independently confirmed"));
        fs_remove(root);
    }
}
