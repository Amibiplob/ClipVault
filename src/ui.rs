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
    quitting: bool,
    status: String,
    error: Option<String>,
    textures: HashMap<String, TextureHandle>,
    logo: TextureHandle,
    focus_search: bool,
    shortcut_draft: String,
    toast: Option<(String, Instant)>,
    last_cleanup: Instant,
    last_geometry: Instant,
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
            if let Some(root) = startup {
                if let Err(e) = set_startup(true, &root) {
                    app.error = Some(format!("Login startup could not be updated: {e}"));
                }
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
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "interface".into(),
            FontData::from_static(include_bytes!("../assets/Interface.ttf")).into(),
        );
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "interface".into());
        ctx.set_fonts(fonts);
        let mut style = (*ctx.style()).clone();
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(13.));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.));
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
        let shortcut_draft = store.lock().unwrap().settings.shortcut.clone();
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
            quitting: false,
            status: "Connecting to the desktop clipboard…".into(),
            error: None,
            textures: HashMap::new(),
            logo,
            focus_search: false,
            shortcut_draft,
            toast: None,
            last_cleanup: Instant::now(),
            last_geometry: Instant::now(),
        }
    }
    fn handle_notice(&mut self, ctx: &Context, notice: Notice) {
        match notice {
            Notice::Open => {
                if self.hidden {
                    self.show(ctx)
                } else {
                    self.hide(ctx)
                }
            }
            Notice::ShortcutApplied(shortcut) => {
                self.mutate(|s| {
                    s.settings.shortcut = shortcut;
                    s.save_settings()
                });
                self.toast = Some((
                    "Shortcut saved. On Wayland, configure the desktop binding too.".into(),
                    Instant::now(),
                ));
            }
            Notice::Activated(id) if self.pending_activation.as_deref() == Some(&id) => {
                self.pending_activation = None;
                self.selected = Some(id);
                self.error = None;
                self.toast = Some(("Copied · Paste with Ctrl + V.".into(), Instant::now()));
                let hide = self.store.lock().unwrap().settings.hide_after_copy;
                if hide {
                    self.hide(ctx);
                }
            }
            Notice::Activated(_) | Notice::Changed => {}
            Notice::ActivationFailed(id, error) => {
                if self.pending_activation.as_deref() == Some(&id) {
                    self.pending_activation = None;
                    self.toast = None;
                    self.error = Some(error);
                    if self.hidden {
                        self.show(ctx);
                    }
                }
            }
            Notice::Status(s) => self.status = s,
            Notice::Error(e) => {
                if self.pending_activation.take().is_some() && self.hidden {
                    self.show(ctx);
                }
                self.error = Some(e);
            }
        }
    }
    fn handle_ipc(&mut self, ctx: &Context, message: &str) {
        if message == "toggle" && !self.hidden {
            self.hide(ctx);
        } else {
            self.show(ctx);
        }
    }
    fn hide(&mut self, ctx: &Context) {
        if self.shot.is_none() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.hidden = true;
        }
    }
    fn show(&mut self, ctx: &Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.hidden = false;
        self.focus_search = true;
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
    fn activate(&mut self, item: &Item) {
        // One outstanding request keeps delayed acknowledgements from hiding
        // the window after a newer copy has failed. Use saved identity, not row index.
        if self.pending_activation.is_some() {
            return;
        }
        self.toast = None;
        let result = {
            let store = self.store.lock().unwrap();
            store
                .items
                .iter()
                .find(|saved| saved.id == item.id)
                .ok_or_else(|| "This clip is no longer in history".to_owned())
                .and_then(|saved| store.snapshot(saved))
        };
        match result {
            Ok(data) if !data.is_empty() => {
                if self
                    .service
                    .tx
                    .send(Command::Restore(item.id.clone(), data))
                    .is_err()
                {
                    self.error=Some("Clipboard service is unavailable. Restart ClipVault to reconnect; your history is saved.".into());
                } else {
                    self.selected = Some(item.id.clone());
                    self.pending_activation = Some(item.id.clone());
                    self.error = None;
                }
            }
            Ok(_) => {
                self.error =
                    Some("This saved clip has no clipboard data. It was not copied.".into())
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
        if item.pinned {
            self.confirm_delete = Some(item.clone());
        } else {
            self.mutate(|s| s.delete(&item.id));
            self.textures.remove(&item.id);
            self.preview = None;
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
        if self.textures.len() > 32 {
            self.textures.clear();
        }
        self.textures.insert(item.id.clone(), tex.clone());
        Some(tex)
    }
    fn header(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            ui.image((self.logo.id(), vec2(32., 32.)));
            let title = ui.label(RichText::new("ClipVault").strong().size(19.));
            if title.interact(Sense::drag()).drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if ui.available_width() > 420. {
                ui.label(RichText::new("On this device").small().color(muted(ui)));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icons::button(ui, "close", "Hide · Escape", 30., muted(ui)).clicked() {
                    self.hide(ctx);
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
                        RichText::new(if paused {
                            "• Paused"
                        } else if self.status.starts_with("Connecting") || self.error.is_some() {
                            "• Check status"
                        } else {
                            "• Monitoring"
                        })
                        .small()
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
        let field = ui.add_sized(
            [ui.available_width(), 36.],
            TextEdit::singleline(&mut self.query)
                .id(Id::new("search"))
                .hint_text("Search text, links, and filenames…"),
        );
        if self.focus_search {
            field.request_focus();
            self.focus_search = false;
        }
        if field.changed() {
            self.selected = None;
        }
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
    fn row(&mut self, ui: &mut Ui, ctx: &Context, item: &Item, compact: bool) {
        let height = if compact { 68. } else { 94. };
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
        let tile = Rect::from_min_size(rect.min + vec2(10., 14.), vec2(30., 30.));
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
        let text_rect = Rect::from_min_max(rect.min + vec2(50., 11.), rect.max - vec2(70., 8.));
        let title_width = text_rect.width().max(20.);
        let mut job = egui::text::LayoutJob::simple(
            item.title.replace(['\r', '\n'], " "),
            FontId::proportional(13.),
            ui.visuals().text_color(),
            title_width,
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        ui.painter()
            .galley(text_rect.min, galley, ui.visuals().text_color());
        let meta = format!(
            "{} · {}{}",
            item.kind,
            relative_time(item.time),
            if item.pinned { " · Pinned" } else { "" }
        );
        let mut job =
            egui::text::LayoutJob::simple(meta, FontId::proportional(10.), muted(ui), title_width);
        job.wrap.max_rows = 1;
        ui.painter().galley(
            text_rect.min + vec2(0., 24.),
            ui.painter().layout_job(job),
            muted(ui),
        );
        if !compact {
            let text = item
                .preview
                .lines()
                .nth(1)
                .unwrap_or("Preview the full content →");
            let mut job = egui::text::LayoutJob::simple(
                text.into(),
                FontId::proportional(11.),
                muted(ui),
                title_width,
            );
            job.wrap.max_rows = 1;
            ui.painter().galley(
                text_rect.min + vec2(0., 48.),
                ui.painter().layout_job(job),
                muted(ui),
            );
        }
        // Child controls own their pointer area; they never bubble into activation.
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(&item.id)
                .max_rect(Rect::from_min_size(
                    pos2(rect.right() - 64., rect.top() + 10.),
                    vec2(58., 45.),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        let pin = icons::button(
            &mut child,
            "pin",
            if item.pinned { "Unpin" } else { "Pin" },
            24.,
            if item.pinned { ACCENT } else { muted(ui) },
        );
        let mut action = 0;
        if pin.clicked() {
            action = 1;
        }
        let menu = child.menu_button("···", |ui| {
            if ui.button("Copy clip").clicked() {
                action = 2;
                ui.close_menu();
            }
            if ui.button("Preview").clicked() {
                action = 3;
                ui.close_menu();
            }
            if matches!(item.kind.as_str(), "Text" | "Links")
                && ui.button("Edit text as new clip").clicked()
            {
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
            3 => self.preview = Some(item.clone()),
            4 => self.edit(item),
            5 => self.delete(item),
            _ => {}
        }
        let on_control = ctx
            .pointer_interact_pos()
            .is_some_and(|p| p.x >= rect.right() - 68.);
        if response.clicked() && !on_control && action == 0 && !menu.response.clicked() {
            self.activate(item);
        }
        if response.has_focus() {
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
        ui.label(
            RichText::new(format!("PREVIEW · {}", item.kind.to_uppercase()))
                .small()
                .color(muted(ui)),
        );
        ui.add(Label::new(RichText::new(&item.title).strong().size(16.)).wrap());
        ui.label(
            RichText::new(format!(
                "{} · {}",
                relative_time(item.time),
                size_label(item.formats.iter().map(|f| f.size).sum())
            ))
            .small()
            .color(muted(ui)),
        );
        ui.add_space(6.);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(
                    Button::new(RichText::new("Copy clip").color(Color32::from_rgb(30, 24, 42)))
                        .fill(ACCENT),
                )
                .clicked()
            {
                self.activate(item);
            }
            if ui
                .button(if item.pinned { "Unpin" } else { "Pin clip" })
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
        ui.label(
            RichText::new("Copy, then press Ctrl + V in your destination app.")
                .small()
                .color(muted(ui)),
        );
        ui.separator();
        ScrollArea::vertical()
            .id_salt("full-preview")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(t) = self.texture(ctx, item) {
                    ui.add(
                        egui::Image::new(&t)
                            .max_width(ui.available_width())
                            .max_height(300.),
                    );
                }
                if item.kind == "Files" {
                    ui.label("File references only. The original files are not backed up.");
                }
                let result = { self.store.lock().unwrap().full_text(item) };
                match result {
                    Ok(text) => {
                        ui.add(
                            Label::new(RichText::new(text).monospace())
                                .wrap()
                                .selectable(true),
                        );
                    }
                    Err(e) => {
                        ui.colored_label(
                            Color32::LIGHT_RED,
                            format!("Saved content unavailable: {e}"),
                        );
                    }
                }
                ui.add_space(12.);
                ui.label(
                    RichText::new(format!(
                        "Saved formats: {}",
                        item.formats
                            .iter()
                            .map(|f| f.mime.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .small()
                    .color(muted(ui)),
                );
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
        }
    }
    fn settings(&mut self, ui: &mut Ui, ctx: &Context) {
        if ui.button("Back to history").clicked() {
            self.settings_open = false;
        }
        ui.horizontal_wrapped(|ui| {
            for (i, label) in ["General", "Appearance", "Shortcuts", "Privacy", "About"]
                .iter()
                .enumerate()
            {
                ui.selectable_value(&mut self.settings_tab, i, *label);
            }
        });
        ui.separator();
        let mut s = self.store.lock().unwrap().settings.clone();
        let before = serde_json::to_string(&s).unwrap();
        let mut startup_changed = false;
        ScrollArea::vertical().id_salt("settings-page").auto_shrink([false,false]).show(ui,|ui|{
            ui.set_max_width(ui.available_width());
            match self.settings_tab {
                0=>{
                    heading(ui,"History that stays useful","Choose what to keep and how ClipVault behaves.");
                    startup_changed=ui.checkbox(&mut s.startup,"Start at login").changed();
                    ui.label(RichText::new("Monitoring continues when the window is hidden. Copies made while ClipVault is not running cannot be captured.").small().color(muted(ui)));
                    ui.checkbox(&mut s.paused,"Pause clipboard monitoring");
                    ui.checkbox(&mut s.skip_consecutive,"Skip consecutive duplicates");
                    ui.label(RichText::new("Identical content shares one record. When enabled, repeated consecutive copies leave its timestamp unchanged; otherwise recency refreshes.").small().color(muted(ui)));
                    ui.checkbox(&mut s.hide_after_copy,"Hide after successfully copying a clip");
                    ui.label(RichText::new("Off by default. When off, the window stays open and confirms Copied. Close hides the window while monitoring continues.").small().color(muted(ui)));
                    ui.separator();
                    ui.checkbox(&mut s.age_enabled,"Enable age-based cleanup");
                    ui.add_enabled_ui(s.age_enabled,|ui|{ui.horizontal_wrapped(|ui|{ui.label("Delete unpinned clips after");ui.add(DragValue::new(&mut s.days).range(1..=3650).suffix(" days"));});});
                    ui.checkbox(&mut s.count_enabled,"Enable maximum-item cleanup");
                    ui.add_enabled_ui(s.count_enabled,|ui|{ui.horizontal_wrapped(|ui|{ui.label("Keep at most");ui.add(DragValue::new(&mut s.limit).range(1..=10000).suffix(" unpinned clips"));});});
                    ui.label(RichText::new("Either rule, both, or neither can be enabled. Age and ordering use the last captured or reused time. Reusing refreshes it. Pins never expire and do not count toward the maximum.").small().color(muted(ui)));
                    ui.separator();
                    ui.horizontal_wrapped(|ui|{ui.label("Maximum clip size");ui.add(DragValue::new(&mut s.max_mb).range(1..=128).suffix(" MB"));});
                    if ui.button("Clear unpinned history…").clicked(){self.confirm_clear=true;}
                }
                1=>{
                    heading(ui,"Appearance","A comfortable space for your clipboard.");
                    ui.horizontal(|ui|{ui.selectable_value(&mut s.light,false,"Dark");ui.selectable_value(&mut s.light,true,"Light");});
                    ui.add_space(12.);
                    ui.horizontal(|ui|{ui.selectable_value(&mut s.compact,true,"Compact");ui.selectable_value(&mut s.compact,false,"Comfortable");});
                    ui.checkbox(&mut s.always_on_top,"Always on top");
                    ui.label("Resize from the window border. Size and position are saved while running; Wayland controls window placement.");
                }
                2=>{
                    heading(ui,"Shortcuts","Only the history shortcut is configurable.");
                    ui.label("Open clipboard history");
                    ui.horizontal_wrapped(|ui|{ui.add_sized([170.,30.],TextEdit::singleline(&mut self.shortcut_draft));
                        if ui.button("Apply shortcut").clicked()&&self.service.tx.send(Command::Shortcut(self.shortcut_draft.clone())).is_err(){self.error=Some("Clipboard service is unavailable; shortcut was not changed.".into());}});
                    ui.label(RichText::new("Examples: Super+V, Ctrl+Alt+V. Conflicts preserve the previous binding.").small().color(muted(ui)));
                    ui.separator();
                    for(key,action)in[("↑ / ↓","Select history item"),("Enter","Copy selected clip"),("Ctrl + F","Focus search"),("Ctrl + Shift + P","Pin / unpin"),("Delete","Delete selected clip (pins require confirmation)"),("Escape","Back or hide window"),("Ctrl + V","Paste in the destination application")]{ui.horizontal_wrapped(|ui|{ui.label(RichText::new(key).color(ACCENT));ui.label(action);});}
                    ui.separator();
                    ui.label("Wayland: configure your desktop to run the installed executable with --toggle. ClipVault cannot register a global shortcut through data-control. Use --data-dir consistently if you override storage.");
                }
                3=>{
                    heading(ui,"Privacy","Stored locally. No account or network requests.");
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
                }
                _=>{
                    ui.image((self.logo.id(),vec2(64.,64.)));
                    heading(ui,"ClipVault","A little history. A smoother workflow.");
                    ui.label(format!("Version {} · Native Rust edition",env!("CARGO_PKG_VERSION")));
                    ui.label("Developer: Md Biplob Hosain");
                    for(label,url)in[("GitHub","https://github.com/Amibiplob"),("Project","https://github.com/Amibiplob/ClipVault"),("Portfolio","https://amibiplob.vercel.app"),("LinkedIn","https://linkedin.com/in/amibiplob"),("Report an issue","https://github.com/Amibiplob/ClipVault/issues")]{ui.hyperlink_to(label,url);}
                    ui.separator();
                    ui.label("MIT License · Copyright 2026 Md Biplob Hosain. Third-party copyright and license notices are included with the application packages.");
                    ui.label("Desktop support: X11 clipboard monitoring, restoration, and global shortcuts. Wayland requires ext-data-control or wlr-data-control; availability depends on compositor configuration. GNOME Wayland commonly lacks these protocols; use an appropriate extension or X11. Compatibility is not guaranteed across all desktops.");
                    ui.label(RichText::new(&self.status).small().color(muted(ui)));
                    if ui.button("Quit ClipVault").clicked(){self.quit(ctx);}
                }
            }
            ui.add_space(18.);
        });
        if startup_changed {
            let root = self.store.lock().unwrap().root.clone();
            if let Err(e) = set_startup(s.startup, &root) {
                s.startup = !s.startup;
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
                self.apply_settings(s);
            }
        }
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
            Window::new("Delete pinned clip?")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(330.);
                    ui.label(format!(
                        "Permanently delete the pinned clip “{}”?",
                        item.title
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_delete = None;
                        }
                        if ui.button("Delete pinned clip").clicked() {
                            self.mutate(|s| s.delete(&item.id));
                            self.confirm_delete = None;
                            self.preview = None;
                            self.textures.remove(&item.id);
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
        let settings = self.store.lock().unwrap().settings.clone();
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
        visuals.selection.stroke = Stroke::new(1_f32, ACCENT);
        visuals.widgets.active.bg_stroke = Stroke::new(2_f32, ACCENT);
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
        let items: Vec<_> = {
            self.store
                .lock()
                .unwrap()
                .items
                .iter()
                .filter(|i| {
                    (self.category == "All"
                        || self.category == i.kind
                        || (self.category == "Pinned" && i.pinned))
                        && (query.is_empty()
                            || i.title.to_lowercase().contains(&query)
                            || i.preview.to_lowercase().contains(&query))
                })
                .cloned()
                .collect()
        };
        let mut selected_index = items
            .iter()
            .position(|item| self.selected.as_deref() == Some(&item.id))
            .unwrap_or(0);
        self.selected = items.get(selected_index).map(|item| item.id.clone());
        let modal =
            self.confirm_clear || self.confirm_delete.is_some() || self.pending_settings.is_some();
        if !modal && !self.settings_open && self.editor.is_none() && self.preview.is_none() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::F)) {
                self.focus_search = true;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                selected_index = (selected_index + 1).min(items.len().saturating_sub(1));
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                selected_index = selected_index.saturating_sub(1);
            }
            self.selected = items.get(selected_index).map(|item| item.id.clone());
            if let Some(item) = items.get(selected_index) {
                let focus = ctx.memory(|m| m.focused());
                let history_focused = focus.is_none()
                    || focus == Some(Id::new("search"))
                    || focus == Some(Id::new(("clip-row", &item.id)));
                // Enter on a focused action belongs to that action, not history.
                if history_focused && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
                {
                    self.activate(item);
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
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if modal {
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
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Up/Down Select   Enter Copy   Ctrl+F Search   Esc Back")
                            .size(10.)
                            .color(muted(ui)),
                    );
                    if let Some((message, time)) = &self.toast {
                        if time.elapsed() < Duration::from_secs(5) {
                            ui.label(RichText::new(message).small().color(ACCENT));
                        }
                    }
                });
            });
        CentralPanel::default().frame(Frame::NONE.fill(ctx.style().visuals.panel_fill).inner_margin(18)).show(ctx,|ui|{
            if let Some(e)=self.error.clone(){ui.horizontal_wrapped(|ui|{ui.colored_label(if settings.light{Color32::DARK_RED}else{Color32::from_rgb(238,176,150)},&e);if ui.small_button("Dismiss").clicked(){self.error=None;}});ui.separator();}
            ui.add_enabled_ui(!modal,|ui|{
                if self.settings_open{self.settings(ui,ctx);return;}
                if self.editor.is_some(){self.editor(ui);return;}
                if let Some(item)=self.preview.clone(){self.detail(ui,ctx,&item,true);return;}
                self.search(ui);ui.separator();
                ui.horizontal(|ui|{ui.label(RichText::new(format!("{} clips · Newest first",items.len())).small().color(muted(ui)));ui.with_layout(Layout::right_to_left(Align::Center),|ui|{if ui.small_button("Clear…").clicked(){self.confirm_clear=true;}});});
                if items.is_empty(){
                    ui.add_space(55.);ui.vertical_centered(|ui|{ui.image((self.logo.id(),vec2(66.,66.)));ui.heading(if query.is_empty(){"Your clipboard, remembered."}else{"No matching clips"});ui.label("Copy text, an image, or a file reference while monitoring is enabled.");ui.label(RichText::new(&self.status).small().color(muted(ui)));});return;
                }
                let wide=ui.available_width()>=670.;
                if wide{
                    let width=ui.available_width();
                    ui.columns(2,|cols|{
                        cols[0].set_width(width*0.5-8.);
                        ScrollArea::vertical().id_salt("history").auto_shrink([false,false]).show(&mut cols[0],|ui|{for item in &items{self.row(ui,ctx,item,settings.compact);ui.add_space(4.);}});
                        if let Some(item)=items.get(selected_index){self.detail(&mut cols[1],ctx,item,false);}
                    });
                }else{
                    if let Some(item)=items.get(selected_index){ui.horizontal_wrapped(|ui|{
                        if ui.button("Preview").clicked(){self.preview=Some(item.clone());}
                        if matches!(item.kind.as_str(),"Text"|"Links")&&ui.button("Edit").clicked(){self.edit(item);}
                        if ui.button("Copy").clicked(){self.activate(item);}
                    });}
                    ScrollArea::vertical().id_salt("history").auto_shrink([false,false]).show(ui,|ui|{for item in &items{self.row(ui,ctx,item,settings.compact);ui.add_space(4.);}});
                }
            });
        });
        self.dialogs(ctx);
    }
}
impl eframe::App for VaultApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
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
                    s.settings.window_size = size;
                    s.settings.window_position = position;
                    s.save_settings()
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
            ctx.request_repaint_after(Duration::from_millis(if self.hidden { 250 } else { 100 }));
        }
    }
}
fn heading(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).strong().size(19.));
    ui.label(RichText::new(subtitle).small().color(muted(ui)));
    ui.add_space(12.);
}
fn size_label(n: usize) -> String {
    if n >= 1048576 {
        format!("{:.1} MB", n as f64 / 1048576.)
    } else {
        format!("{} KB", n.div_ceil(1024))
    }
}
fn relative_time(t: i64) -> String {
    let d = (now() - t).max(0);
    if d < 60 {
        "Just now".into()
    } else if d < 3600 {
        format!("{} min ago", d / 60)
    } else if d < 86400 {
        format!("{} h ago", d / 3600)
    } else {
        format!("{} d ago", d / 86400)
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
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let image_root = std::env::var_os("APPDIR").and_then(|d| std::fs::canonicalize(d).ok());
    let exe = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| {
            p.is_absolute()
                && p.is_file()
                && image_root.as_ref().is_some_and(|d| exe.starts_with(d))
        })
        .unwrap_or(exe);
    let escape = |s: &str| {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%")
    };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text=format!("[Desktop Entry]\nType=Application\nName=ClipVault\nComment=Private clipboard history\nExec=\"{}\" --background --data-dir \"{}\"\nIcon=clipvault\nStartupWMClass=clipvault\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",escape(&exe.to_string_lossy()),escape(&root.to_string_lossy()));
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
                CentralPanel::default().show(ctx, |ui| app.row(ui, ctx, item, true));
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
        click_row(&mut app, &ctx, &item, pos2(80., 35.));
        let Command::Restore(id, restored) = rx.try_recv().unwrap() else {
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
                assert!(matches!(rx.try_recv(), Ok(Command::Restore(_, _))));
                // Ownership and database errors take the same explicit failure path.
                for error in ["Could not take clipboard ownership", "Clipboard ready, but recency could not be saved: SQLite could not save clipboard history"] {
                    app.handle_notice(&ctx, Notice::ActivationFailed(item.id.clone(), error.into()));
                    assert!(!app.hidden);
                    assert_eq!(app.error.as_deref(), Some(error));
                    assert!(app.toast.is_none());
                    app.activate(&item);
                    assert!(matches!(rx.try_recv(), Ok(Command::Restore(_, _))));
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
        assert!(matches!(rx.try_recv(), Ok(Command::Restore(_, _))));
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
        assert!(!app.hidden);
        assert!(!app.store.lock().unwrap().settings.paused);
        assert!(!app.quitting);
        assert!(rx.try_recv().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn header_gear_next_to_close_toggles_settings_and_close_only_hides() {
        let (mut app, rx, root) = app();
        let ctx = Context::default();
        let item = app.store.lock().unwrap().items[0].clone();
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
                    let pos = pos2(379., 24.);
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
            click_row(&mut app, &ctx, &item, pos2(380., 35.));
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
        let Command::Restore(id, data) = rx
            .try_recv()
            .expect("click must request actual clipboard restore")
        else {
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
}
