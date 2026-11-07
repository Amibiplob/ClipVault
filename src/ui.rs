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
const ACCENT: Color32 = Color32::from_rgb(184, 172, 250);
const MUTED: Color32 = Color32::from_rgb(125, 126, 138);
pub struct VaultApp {
    store: Shared,
    service: Service,
    ipc: Receiver<String>,
    query: String,
    category: String,
    selected: usize,
    settings_open: bool,
    settings_tab: usize,
    preview: Option<Item>,
    confirm_clear: bool,
    include_pinned: bool,
    shot: Option<PathBuf>,
    frames: usize,
    shot_requested: bool,
    hidden: bool,
    quitting: bool,
    status: String,
    error: Option<String>,
    textures: HashMap<String, TextureHandle>,
    focus_search: bool,
    shortcut_draft: String,
    toast: Option<(String, Instant)>,
    last_cleanup: Instant,
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
        cc.egui_ctx.set_fonts(fonts);
        let mut style = (*cc.egui_ctx.style()).clone();
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(13.));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.));
        style.spacing.button_padding = vec2(12., 7.);
        style.spacing.item_spacing = vec2(8., 8.);
        style.visuals.window_corner_radius = CornerRadius::same(14);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
        cc.egui_ctx.set_style(style);
        let shortcut_draft = store.lock().unwrap().settings.shortcut.clone();
        Self {
            store,
            service,
            ipc,
            query: String::new(),
            category: "All".into(),
            selected: 0,
            settings_open: false,
            settings_tab: 0,
            preview: None,
            confirm_clear: false,
            include_pinned: false,
            shot,
            frames: 0,
            shot_requested: false,
            hidden,
            quitting: false,
            status: "Connecting to your clipboard…".into(),
            error: None,
            textures: HashMap::new(),
            focus_search: false,
            shortcut_draft,
            toast: None,
            last_cleanup: Instant::now(),
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
    fn activate(&mut self, item: &Item) {
        let data = self.store.lock().unwrap().snapshot(item);
        match data {
            Ok(data) => {
                if self.service.tx.send(Command::Restore(data)).is_err() {
                    self.error = Some(
                        "Clipboard service is unavailable. Restart ClipVault to reconnect.".into(),
                    );
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn mutate(&mut self, f: impl FnOnce(&mut crate::storage::Store) -> Result<(), String>) {
        if let Err(e) = f(&mut self.store.lock().unwrap()) {
            self.error = Some(e)
        }
    }
    fn pin(&mut self, id: &str) {
        self.mutate(|s| s.pin(id));
    }
    fn delete(&mut self, id: &str) {
        self.mutate(|s| s.delete(id));
        self.textures.remove(id);
    }
    fn texture(&mut self, ctx: &Context, item: &Item) -> Option<TextureHandle> {
        if let Some(t) = self.textures.get(&item.id) {
            return Some(t.clone());
        }
        let path = item.thumbnail.as_ref()?;
        let path = self.store.lock().unwrap().root.join(path);
        let im = image::open(path).ok()?.into_rgba8();
        let size = [im.width() as usize, im.height() as usize];
        let tex = ctx.load_texture(
            &item.id,
            ColorImage::from_rgba_unmultiplied(size, im.as_raw()),
            TextureOptions::LINEAR,
        );
        if self.textures.len() >= 24 {
            if let Some(oldest) = self.textures.keys().next().cloned() {
                self.textures.remove(&oldest);
            }
        }
        self.textures.insert(item.id.clone(), tex.clone());
        Some(tex)
    }
    fn keycap(ui: &mut Ui, text: &str) {
        Frame::NONE
            .fill(ui.visuals().extreme_bg_color)
            .stroke(Stroke::new(
                1_f32,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .corner_radius(4)
            .inner_margin(Margin::symmetric(5, 2))
            .show(ui, |ui| {
                if text == "↑ ↓" || text == "↵" {
                    let (r, _) = ui.allocate_exact_size(
                        vec2(if text == "↑ ↓" { 24. } else { 12. }, 14.),
                        Sense::hover(),
                    );
                    if text == "↑ ↓" {
                        icons::draw(
                            ui.painter(),
                            Rect::from_min_size(r.min, vec2(12., 14.)),
                            "up",
                            MUTED,
                        );
                        icons::draw(
                            ui.painter(),
                            Rect::from_min_size(r.min + vec2(12., 0.), vec2(12., 14.)),
                            "down",
                            MUTED,
                        );
                    } else {
                        icons::draw(ui.painter(), r, "enter", MUTED);
                    }
                } else {
                    ui.label(RichText::new(text).size(10.).color(MUTED));
                }
            });
    }
    fn header(&mut self, ui: &mut Ui, ctx: &Context, fg: Color32) {
        let rect = ui
            .allocate_exact_size(vec2(ui.available_width(), 76.), Sense::hover())
            .0;
        let drag = ui.interact(rect, Id::new("drag-title"), Sense::drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        let logo = Rect::from_min_size(rect.min + vec2(25., 22.), vec2(33., 33.));
        ui.painter()
            .rect_filled(logo, 9, Color32::from_rgb(48, 42, 67));
        icons::draw(ui.painter(), logo.shrink(6.), "clipboard", ACCENT);
        ui.painter().text(
            rect.min + vec2(70., 25.),
            Align2::LEFT_TOP,
            "ClipVault",
            FontId::proportional(19.),
            fg,
        );
        let badge = Rect::from_min_size(rect.min + vec2(171., 29.), vec2(43., 18.));
        ui.painter().rect_stroke(
            badge,
            4,
            Stroke::new(1_f32, Color32::from_gray(58)),
            StrokeKind::Inside,
        );
        ui.painter().text(
            badge.center(),
            Align2::CENTER_CENTER,
            "LOCAL",
            FontId::proportional(8.),
            MUTED,
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(
                    pos2(rect.right() - 150., rect.top() + 21.),
                    vec2(124., 36.),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        let paused = self.store.lock().unwrap().settings.paused;
        if icons::button(
            &mut child,
            if paused { "play" } else { "pause" },
            if paused {
                "Resume clipboard monitoring"
            } else {
                "Pause clipboard monitoring"
            },
            32.,
            MUTED,
        )
        .clicked()
        {
            self.mutate(|s| {
                s.settings.paused = !s.settings.paused;
                s.save_settings()
            });
        }
        if icons::button(&mut child, "settings", "Settings", 32., MUTED).clicked() {
            self.settings_open = true;
        }
        if icons::button(&mut child, "close", "Hide ClipVault · Esc", 32., MUTED).clicked() {
            self.hide(ctx);
        }
    }
    fn search(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            ui.add_space(24.);
            let width = ui.available_width() - 24.;
            Frame::NONE
                .fill(ui.visuals().extreme_bg_color)
                .stroke(Stroke::new(
                    1_f32,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .corner_radius(9)
                .inner_margin(Margin::symmetric(13, 12))
                .show(ui, |ui| {
                    ui.set_width(width - 26.);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(19., 22.), Sense::hover());
                        icons::draw(ui.painter(), r.shrink(1.), "search", MUTED);
                        ui.add_space(5.);
                        let field = ui.add_sized(
                            vec2(ui.available_width() - 52., 24.),
                            TextEdit::singleline(&mut self.query)
                                .id(Id::new("search"))
                                .frame(false)
                                .hint_text(RichText::new("Search clipboard...").color(MUTED))
                                .font(FontId::proportional(14.)),
                        );
                        if self.focus_search {
                            field.request_focus();
                            self.focus_search = false;
                        }
                        if field.changed() {
                            self.selected = 0;
                        }
                        if !self.query.is_empty() {
                            if icons::button(ui, "close", "Clear search", 22., MUTED).clicked() {
                                self.query.clear();
                                self.selected = 0;
                            }
                        } else {
                            Self::keycap(ui, "Ctrl F");
                        }
                    });
                });
        });
        ui.add_space(13.);
        ui.horizontal(|ui| {
            ui.add_space(24.);
            for (cat, icon) in [
                ("All", "grid"),
                ("Text", "text"),
                ("Images", "image"),
                ("Files", "folder"),
                ("Links", "link"),
                ("Pinned", "pin"),
            ] {
                let selected = self.category == cat;
                let width = if cat == "Images" {
                    91.
                } else if cat == "Pinned" {
                    88.
                } else {
                    77.
                };
                let (r, res) = ui.allocate_exact_size(vec2(width, 32.), Sense::click());
                if selected {
                    ui.painter().rect_filled(
                        r,
                        6,
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(43, 39, 57)
                        } else {
                            Color32::from_rgb(232, 226, 247)
                        },
                    );
                } else if res.hovered() {
                    ui.painter().rect_filled(r, 6, Color32::from_white_alpha(8));
                }
                let color = if selected { ACCENT } else { MUTED };
                icons::draw(
                    ui.painter(),
                    Rect::from_min_size(r.min + vec2(10., 9.), vec2(14., 14.)),
                    icon,
                    color,
                );
                ui.painter().text(
                    r.min + vec2(31., 8.),
                    Align2::LEFT_TOP,
                    cat,
                    FontId::proportional(12.),
                    color,
                );
                if res.clicked() {
                    self.category = cat.into();
                    self.selected = 0;
                }
            }
        });
        ui.add_space(17.);
        let _ = ctx;
    }
    fn empty(&mut self, ui: &mut Ui, filtered: bool) {
        let w = ui.available_width();
        let h = ui.available_height();
        let (r, _) = ui.allocate_exact_size(vec2(w, h.max(150.)), Sense::hover());
        let center = pos2(r.center().x, r.top() + h.min(450.) * 0.37);
        let p = ui.painter();
        let border = if ui.visuals().dark_mode {
            Color32::from_rgb(53, 51, 65)
        } else {
            Color32::from_gray(213)
        };
        let fill = if ui.visuals().dark_mode {
            Color32::from_rgb(31, 30, 39)
        } else {
            Color32::from_gray(246)
        };
        p.rect_filled(
            Rect::from_center_size(center + vec2(-23., -5.), vec2(76., 80.)),
            12,
            fill,
        );
        p.rect_stroke(
            Rect::from_center_size(center + vec2(-23., -5.), vec2(76., 80.)),
            12,
            Stroke::new(1_f32, border),
            StrokeKind::Inside,
        );
        p.rect_filled(
            Rect::from_center_size(center + vec2(23., 5.), vec2(76., 80.)),
            12,
            fill,
        );
        p.rect_stroke(
            Rect::from_center_size(center + vec2(23., 5.), vec2(76., 80.)),
            12,
            Stroke::new(1_f32, border),
            StrokeKind::Inside,
        );
        let front = Rect::from_center_size(center, vec2(77., 88.));
        p.rect_filled(
            front,
            13,
            if ui.visuals().dark_mode {
                Color32::from_rgb(41, 36, 56)
            } else {
                Color32::from_rgb(234, 229, 246)
            },
        );
        p.rect_stroke(
            front,
            13,
            Stroke::new(1_f32, Color32::from_rgb(76, 64, 100)),
            StrokeKind::Inside,
        );
        icons::draw(
            p,
            Rect::from_center_size(center, vec2(32., 36.)),
            if filtered { "search" } else { "clipboard" },
            ACCENT,
        );
        let fg = ui.visuals().text_color();
        p.text(
            center + vec2(0., 76.),
            Align2::CENTER_TOP,
            if filtered {
                "Nothing found. Keep looking."
            } else {
                "Good things are worth keeping."
            },
            FontId::proportional(21.),
            fg,
        );
        p.text(
            center + vec2(0., 111.),
            Align2::CENTER_TOP,
            if filtered {
                "Try a different search or another category."
            } else {
                "Copy some text, an image, or a file. We’ll take it from here."
            },
            FontId::proportional(12.),
            MUTED,
        );
        if !filtered {
            let keyrect = Rect::from_center_size(center + vec2(0., 164.), vec2(188., 32.));
            p.rect_filled(keyrect, 7, fill);
            p.rect_stroke(keyrect, 7, Stroke::new(1_f32, border), StrokeKind::Inside);
            let shortcut = self.store.lock().unwrap().settings.shortcut.clone();
            p.text(
                keyrect.center(),
                Align2::CENTER_CENTER,
                format!("{shortcut}   ·   Always within reach"),
                FontId::proportional(10.),
                MUTED,
            );
            if h > 380. {
                let y = r.bottom() - 70.;
                let line_y = y - 23.;
                p.line_segment(
                    [pos2(r.left() + 40., line_y), pos2(r.right() - 40., line_y)],
                    Stroke::new(1_f32, border),
                );
                for (index, (icon, title, desc)) in [
                    ("clipboard", "Copy anything", "Your clipboard, remembered"),
                    ("search", "Find it instantly", "Search. Select. Ready."),
                    ("enter", "Paste anywhere", "With your usual Ctrl + V"),
                ]
                .iter()
                .enumerate()
                {
                    let x = r.left() + w * (index as f32 + 0.5) / 3.;
                    icons::draw(
                        p,
                        Rect::from_center_size(pos2(x, y - 2.), vec2(18., 18.)),
                        icon,
                        MUTED,
                    );
                    p.text(
                        pos2(x, y + 16.),
                        Align2::CENTER_TOP,
                        *title,
                        FontId::proportional(11.),
                        fg,
                    );
                    p.text(
                        pos2(x, y + 35.),
                        Align2::CENTER_TOP,
                        *desc,
                        FontId::proportional(9.),
                        MUTED,
                    );
                }
            }
        }
    }
    fn row(&mut self, ui: &mut Ui, ctx: &Context, item: &Item, index: usize) {
        let selected = index == self.selected;
        let height = if item.thumbnail.is_some() { 128. } else { 96. };
        let (r, res) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        if !ui.is_rect_visible(r) {
            return;
        }
        let thumb = self.texture(ctx, item);
        let hovered = res.hovered();
        let p = ui.painter();
        let bg = if selected {
            if ui.visuals().dark_mode {
                Color32::from_rgb(39, 36, 50)
            } else {
                Color32::from_rgb(236, 230, 248)
            }
        } else if hovered {
            ui.visuals().faint_bg_color
        } else {
            ui.visuals().panel_fill
        };
        p.rect_filled(r, 9, bg);
        if selected {
            p.rect_stroke(
                r,
                9,
                Stroke::new(1_f32, Color32::from_rgb(74, 64, 99)),
                StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_min_size(r.min + vec2(0., 22.), vec2(2., height - 44.)),
                2,
                ACCENT,
            );
        }
        let icon = match item.kind.as_str() {
            "Images" => "image",
            "Files" => "file",
            "Links" => "link",
            _ => {
                if item.preview.contains("fn ")
                    || item.preview.contains("const ")
                    || item.preview.contains("import ")
                    || item.preview.contains("{\n")
                {
                    "code"
                } else {
                    "text"
                }
            }
        };
        let tile = Rect::from_min_size(r.min + vec2(16., 17.), vec2(34., 34.));
        p.rect_filled(
            tile,
            8,
            if ui.visuals().dark_mode {
                Color32::from_rgb(43, 42, 51)
            } else {
                Color32::from_gray(232)
            },
        );
        icons::draw(
            p,
            tile.shrink(8.),
            icon,
            if selected { ACCENT } else { MUTED },
        );
        let content_x = r.left() + 65.;
        let right_reserved = if thumb.is_some() { 168. } else { 90. };
        let width = r.width() - 65. - right_reserved;
        let mut title_job = egui::text::LayoutJob::simple(
            item.title.replace(['\n', '\r'], " "),
            FontId::proportional(13.),
            ui.visuals().text_color(),
            width,
        );
        title_job.wrap.max_rows = 1;
        title_job.wrap.break_anywhere = true;
        let title = p.layout_job(title_job);
        p.galley(
            pos2(content_x, r.top() + 16.),
            title,
            ui.visuals().text_color(),
        );
        let preview = if item.kind == "Images" {
            format!(
                "Original image · {}",
                size_label(item.formats.iter().map(|f| f.size).max().unwrap_or(0))
            )
        } else {
            item.preview
                .lines()
                .skip(1)
                .find(|l| !l.trim().is_empty())
                .unwrap_or(if item.kind == "Files" {
                    "File references · ready to paste"
                } else if item.kind == "Links" {
                    "Link · ready to paste"
                } else {
                    "Plain text"
                })
                .chars()
                .take(100)
                .collect()
        };
        let galley = p.layout(preview, FontId::proportional(11.), MUTED, width);
        p.galley(pos2(content_x, r.top() + 40.), galley, MUTED);
        let meta = format!(
            "{}   ·   {}{}",
            item.kind.trim_end_matches('s'),
            relative_time(item.time),
            if item.pinned { "   ·   Pinned" } else { "" }
        );
        p.text(
            pos2(content_x, r.bottom() - 22.),
            Align2::LEFT_TOP,
            meta,
            FontId::proportional(9.),
            MUTED,
        );
        if let Some(t) = thumb {
            let boxr = Rect::from_min_size(
                pos2(r.right() - 157., r.top() + 13.),
                vec2(100., height - 26.),
            );
            let size = t.size_vec2();
            let scale = (boxr.width() / size.x).min(boxr.height() / size.y);
            let actual = Rect::from_center_size(boxr.center(), size * scale);
            p.rect_filled(boxr, 5, Color32::from_black_alpha(30));
            p.image(
                t.id(),
                actual,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::WHITE,
            );
        }
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(&item.id)
                .max_rect(Rect::from_min_size(
                    pos2(r.right() - 45., r.top() + 8.),
                    vec2(32., height - 12.),
                ))
                .layout(Layout::top_down(Align::Center)),
        );
        let mut action = false;
        if icons::button(
            &mut child,
            "pin",
            if item.pinned {
                "Unpin item"
            } else {
                "Pin item"
            },
            28.,
            if item.pinned { ACCENT } else { MUTED },
        )
        .clicked()
        {
            self.pin(&item.id);
            action = true;
        }
        let mut menu_action = 0;
        child.menu_button(RichText::new("···").size(17.).color(MUTED), |ui| {
            if ui.button("Expand preview").clicked() {
                menu_action = 1;
                ui.close_menu();
            }
            if ui
                .button(if item.pinned { "Unpin" } else { "Pin" })
                .clicked()
            {
                menu_action = 2;
                ui.close_menu();
            }
            if ui.button("Delete item").clicked() {
                menu_action = 3;
                ui.close_menu();
            }
        });
        res.context_menu(|ui| {
            if ui.button("Use this item").clicked() {
                menu_action = 4;
                ui.close_menu();
            }
            if ui.button("Expand preview").clicked() {
                menu_action = 1;
                ui.close_menu();
            }
            if ui.button("Delete item").clicked() {
                menu_action = 3;
                ui.close_menu();
            }
        });
        match menu_action {
            4 => self.activate(item),
            2 => self.pin(&item.id),
            1 => self.preview = Some(item.clone()),
            3 => self.delete(&item.id),
            _ => {}
        }
        if res.clicked() && !action && menu_action == 0 {
            self.selected = index;
            self.activate(item);
        }
        if selected && ctx.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp)) {
            res.scroll_to_me(Some(Align::Center));
        }
    }
    fn footer(&mut self, ui: &mut Ui) {
        let r = ui.max_rect();
        let border = ui.visuals().widgets.noninteractive.bg_stroke.color;
        ui.painter()
            .line_segment([r.left_top(), r.right_top()], Stroke::new(1_f32, border));
        ui.add_space(14.);
        ui.horizontal(|ui| {
            ui.add_space(24.);
            Self::keycap(ui, "↑ ↓");
            ui.label(RichText::new("Navigate").size(10.).color(MUTED));
            ui.add_space(8.);
            Self::keycap(ui, "↵");
            ui.label(RichText::new("Use clip").size(10.).color(MUTED));
            ui.add_space(8.);
            Self::keycap(ui, "esc");
            ui.label(RichText::new("Close").size(10.).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(24.);
                let paused = self.store.lock().unwrap().settings.paused;
                let good = self.error.is_none() && !paused;
                let label = if self.error.is_some() {
                    "Needs attention"
                } else if paused {
                    "Paused"
                } else {
                    "Listening"
                };
                let (r, res) = ui.allocate_exact_size(vec2(70., 20.), Sense::hover());
                ui.painter().circle_filled(
                    pos2(r.left() + 4., r.center().y),
                    2.5,
                    if good {
                        Color32::from_rgb(131, 181, 146)
                    } else {
                        Color32::from_rgb(192, 156, 108)
                    },
                );
                ui.painter().text(
                    pos2(r.left() + 14., r.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(10.),
                    MUTED,
                );
                res.on_hover_text(self.error.as_ref().unwrap_or(&self.status));
                icons::draw(
                    ui.painter(),
                    Rect::from_min_size(ui.cursor().min, vec2(15., 15.)),
                    "shield",
                    MUTED,
                );
                ui.add_space(20.);
                ui.label(RichText::new("Only on this device").size(10.).color(MUTED));
            });
        });
    }
    fn save_settings(&mut self, settings: Settings) {
        self.mutate(|s| {
            s.settings = settings;
            s.save_settings()
        });
    }
    fn settings(&mut self, ctx: &Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut s = self.store.lock().unwrap().settings.clone();
        let before = serde_json::to_string(&s).unwrap();
        let root = self.store.lock().unwrap().root.clone();
        let mut startup_changed = false;
        Window::new("Settings").open(&mut open).collapsible(false).resizable(false).default_width(520.).anchor(Align2::CENTER_CENTER,Vec2::ZERO).show(ctx,|ui|{ui.add_space(6.);ui.horizontal(|ui|{for(i,label)in["General","Appearance","Shortcuts","Privacy","About"].iter().enumerate(){ui.selectable_value(&mut self.settings_tab,i,*label);}});ui.add_space(14.);ui.separator();ui.add_space(8.);
  match self.settings_tab{
   0=>{heading(ui,"Make room for what matters","Pinned clips stay until you delete them.");ui.checkbox(&mut s.paused,"Pause clipboard monitoring");ui.add_space(10.);ui.horizontal(|ui|{ui.label("History limit");ui.add(DragValue::new(&mut s.limit).range(10..=10000).suffix(" items"));});ui.horizontal(|ui|{ui.label("Delete unpinned clips after");ui.add(DragValue::new(&mut s.days).range(0..=3650).suffix(" days"));});ui.label(RichText::new("Set 0 days to keep history indefinitely.").small().color(MUTED));ui.horizontal(|ui|{ui.label("Maximum clipboard size");ui.add(DragValue::new(&mut s.max_mb).range(1..=128).suffix(" MB"));});ui.add_space(12.);if ui.checkbox(&mut s.startup,"Start ClipVault when I log in").changed(){startup_changed=true;}ui.add_space(15.);if ui.button("Clear clipboard history…").clicked(){self.confirm_clear=true;}ui.add_space(10.);ui.label(RichText::new(format!("{} saved clips · stored locally",self.store.lock().unwrap().items.len())).small().color(MUTED));},
   1=>{heading(ui,"A quieter workspace","Choose the appearance that feels right.");ui.horizontal(|ui|{ui.selectable_value(&mut s.light,false,"Dark");ui.selectable_value(&mut s.light,true,"Light");});ui.add_space(25.);ui.label("Compact, always on top, and out of your way.");ui.label(RichText::new("Selecting a clip hides the window and keeps the clipboard available.").small().color(MUTED));},
   2=>{heading(ui,"Keep your hands on the keyboard","Your desktop, one shortcut away.");ui.horizontal(|ui|{ui.label("Open clipboard history");ui.add_sized([150.,28.],TextEdit::singleline(&mut self.shortcut_draft));if ui.button("Apply").clicked() && self.service.tx.send(Command::Shortcut(self.shortcut_draft.clone())).is_err(){self.error=Some("Clipboard service is unavailable. Restart ClipVault to reconnect.".into());}});ui.label(RichText::new("Example: Super+V, Ctrl+Alt+V, or Super+Shift+V").small().color(MUTED));ui.add_space(12.);Grid::new("shortcut-list").num_columns(2).spacing([80.,10.]).show(ui,|ui|{for(key,action)in[("↑ / ↓","Navigate history"),("Enter","Activate clip and hide"),("Escape","Dismiss without changing clipboard"),("Ctrl + F","Search history"),("Delete","Delete highlighted clip"),("Ctrl + Shift + P","Pin or unpin highlighted clip"),("Ctrl + V","Paste in your target application")]{ui.label(RichText::new(key).color(ACCENT));ui.label(action);ui.end_row();}});ui.add_space(14.);ui.label(RichText::new("Wayland: add a desktop shortcut for clipvault --toggle. The compositor owns global shortcuts.").small().color(MUTED));},
   3=>{heading(ui,"Your clipboard. Your business.","No account. No network requests. No cloud storage.");ui.label("Ignore clips containing these words (one per line)");ui.add_sized([ui.available_width(),65.],TextEdit::multiline(&mut s.exclude_content).hint_text("A private email, token prefix, or sensitive phrase"));ui.add_space(8.);ui.label("Ignore these applications on X11 (one per line)");ui.add_sized([ui.available_width(),65.],TextEdit::multiline(&mut s.exclude_apps));ui.label(RichText::new("Application identity is not exposed by Wayland data-control. Use pause or content exclusions there. Password-manager clipboard hints are always ignored.").small().color(MUTED));ui.add_space(10.);ui.label(RichText::new("History is private to your Linux account, but is not encrypted. Clearing history removes saved data; it does not clear the active system clipboard.").small().color(MUTED));ui.add_space(10.);ui.label(RichText::new(format!("Local storage: {}",root.display())).small().color(MUTED));},
   _=>{heading(ui,"ClipVault","A little history. A smoother workflow.");ui.label("Version 0.1.0 · Native Rust edition");ui.add_space(10.);ui.label("Setup & desktop support");ui.label(RichText::new("X11: background monitoring and global shortcuts work directly.\nWayland: supports ext-data-control and wlr-data-control, including compatible KDE and wlroots desktops. Assign your desktop shortcut to clipvault --toggle.\nGNOME Wayland: requires an extension exposing data-control, or choose an X11 session. ClipVault reports unavailable access instead of pretending to capture.").size(12.).color(MUTED));ui.add_space(10.);ui.label(RichText::new(&self.status).small().color(ACCENT));ui.add_space(10.);ui.label(RichText::new("Supported content: text, HTML, rich text, PNG, JPEG, GIF, WebP, file URIs, file-manager references, links, and code. Original MIME data is retained within the size limit. Files remain references; moving the original can break pasting.").small().color(MUTED));ui.add_space(16.);if ui.button("Quit ClipVault").clicked(){self.quitting=true;let _=self.service.tx.send(Command::Stop);ctx.send_viewport_cmd(ViewportCommand::Close);}}
  }
  ui.add_space(12.);
 });
        self.settings_open = open;
        if startup_changed {
            if let Err(e) = set_startup(s.startup) {
                s.startup = !s.startup;
                self.error = Some(e);
            }
        }
        if before != serde_json::to_string(&s).unwrap() {
            self.save_settings(s);
        }
    }
    fn dialogs(&mut self, ctx: &Context) {
        if self.confirm_clear {
            Window::new("Clear history?")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Saved clips will be permanently removed from this device.");
                    ui.checkbox(&mut self.include_pinned, "Also delete pinned items");
                    ui.add_space(12.);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_clear = false;
                        }
                        if ui
                            .button(
                                RichText::new("Delete history")
                                    .color(Color32::from_rgb(237, 151, 151)),
                            )
                            .clicked()
                        {
                            let include = self.include_pinned;
                            self.mutate(|s| s.clear(include));
                            self.textures.clear();
                            self.confirm_clear = false;
                        }
                    });
                });
        }
        if let Some(item) = self.preview.clone() {
            let mut open = true;
            Window::new("Clipboard preview")
                .open(&mut open)
                .default_width(520.)
                .default_height(400.)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new(&item.title).size(17.));
                    ui.label(
                        RichText::new(format!("{} · {}", item.kind, relative_time(item.time)))
                            .small()
                            .color(MUTED),
                    );
                    ui.separator();
                    ScrollArea::vertical().max_height(360.).show(ui, |ui| {
                        if let Some(t) = self.texture(ctx, &item) {
                            ui.add(
                                egui::Image::new(&t)
                                    .max_size(vec2(540., 300.))
                                    .maintain_aspect_ratio(true),
                            );
                        }
                        let mut text = item.preview.clone();
                        ui.add(
                            TextEdit::multiline(&mut text)
                                .desired_width(f32::INFINITY)
                                .font(TextStyle::Monospace)
                                .interactive(true),
                        );
                    });
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "Original formats: {}",
                            item.formats
                                .iter()
                                .map(|f| f.mime.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                        .small()
                        .color(MUTED),
                    );
                    if ui.button("Use this clip  ↵").clicked() {
                        self.activate(&item);
                        self.preview = None;
                    }
                });
            if !open {
                self.preview = None;
            }
        }
    }
}
impl eframe::App for VaultApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        while let Ok(n) = self.service.rx.try_recv() {
            match n {
                Notice::Open => {
                    if self.hidden {
                        self.show(ctx)
                    } else {
                        self.hide(ctx)
                    }
                }
                Notice::ShortcutApplied(shortcut) => {
                    self.error = None;
                    self.mutate(|s| {
                        s.settings.shortcut = shortcut.clone();
                        s.save_settings()
                    });
                    self.status = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                        format!("Bind {shortcut} in desktop settings to clipvault --toggle")
                    } else {
                        format!("{shortcut} · shortcut ready")
                    };
                }
                Notice::Activated => {
                    self.hide(ctx);
                    self.toast = Some((
                        "Clipboard ready. Paste with Ctrl + V.".into(),
                        Instant::now(),
                    ));
                }
                Notice::Status(s) => {
                    self.status = s;
                    self.error = None
                }
                Notice::Error(e) => self.error = Some(e),
                Notice::Changed => {
                    self.selected = 0;
                }
            }
        }
        while let Ok(s) = self.ipc.try_recv() {
            if s == "toggle" && !self.hidden {
                self.hide(ctx)
            } else {
                self.show(ctx)
            }
        }
        if self.last_cleanup.elapsed() > Duration::from_secs(60) {
            self.last_cleanup = Instant::now();
            self.mutate(|s| s.cleanup());
        }
        let light = self.store.lock().unwrap().settings.light;
        let mut visuals = if light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        visuals.override_text_color = Some(if light {
            Color32::from_rgb(40, 38, 49)
        } else {
            Color32::from_rgb(228, 226, 235)
        });
        visuals.panel_fill = if light {
            Color32::from_rgb(250, 249, 252)
        } else {
            Color32::from_rgb(25, 25, 30)
        };
        visuals.window_fill = if light {
            Color32::from_rgb(250, 249, 252)
        } else {
            Color32::from_rgb(30, 29, 36)
        };
        visuals.extreme_bg_color = if light {
            Color32::from_rgb(242, 240, 246)
        } else {
            Color32::from_rgb(33, 32, 39)
        };
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(
            1_f32,
            if light {
                Color32::from_gray(221)
            } else {
                Color32::from_rgb(50, 49, 59)
            },
        );
        visuals.selection.bg_fill = Color32::from_rgb(72, 59, 105);
        visuals.selection.stroke = Stroke::new(1_f32, ACCENT);
        ctx.set_visuals(visuals);
        let query = self.query.to_lowercase();
        let items: Vec<_> = self
            .store
            .lock()
            .unwrap()
            .items
            .iter()
            .filter(|i| {
                (self.category == "All"
                    || self.category == i.kind
                    || (self.category == "Pinned" && i.pinned))
                    && (query.is_empty()
                        || i.preview.to_lowercase().contains(&query)
                        || i.title.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();
        self.selected = self.selected.min(items.len().saturating_sub(1));
        if !self.settings_open && !self.confirm_clear && self.preview.is_none() {
            ctx.input_mut(|i| {
                if i.consume_key(Modifiers::CTRL, Key::F) {
                    self.focus_search = true;
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !items.is_empty() {
                    self.selected = (self.selected + 1).min(items.len() - 1);
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    self.selected = self.selected.saturating_sub(1);
                }
            });
            if let Some(item) = items.get(self.selected) {
                let item = item.clone();
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                    self.activate(&item)
                }
                if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::P)) {
                    self.pin(&item.id)
                }
                if (self.query.is_empty() || !ctx.wants_keyboard_input())
                    && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete))
                {
                    self.delete(&item.id)
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.confirm_clear {
                self.confirm_clear = false
            } else if self.preview.is_some() {
                self.preview = None
            } else if self.settings_open {
                self.settings_open = false
            } else {
                self.hide(ctx)
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.shot.is_none() && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide(ctx);
        }
        TopBottomPanel::bottom("footer")
            .exact_height(55.)
            .frame(Frame::NONE.fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| self.footer(ui));
        CentralPanel::default()
            .frame(Frame::NONE.fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                let bounds = ui.max_rect();
                ui.painter().rect_stroke(
                    bounds,
                    0,
                    Stroke::new(1_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    StrokeKind::Inside,
                );
                self.header(ui, ctx, ui.visuals().text_color());
                self.search(ui, ctx);
                let y = ui.cursor().top();
                ui.painter().line_segment(
                    [pos2(24., y), pos2(bounds.right() - 24., y)],
                    Stroke::new(1_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
                ui.add_space(14.);
                ui.horizontal(|ui| {
                    ui.add_space(26.);
                    ui.label(
                        RichText::new(if self.category == "Pinned" {
                            "PINNED CLIPS"
                        } else {
                            "CLIPBOARD HISTORY"
                        })
                        .size(9.)
                        .color(MUTED)
                        .extra_letter_spacing(1.),
                    );
                    Frame::NONE
                        .fill(ui.visuals().extreme_bg_color)
                        .corner_radius(4)
                        .inner_margin(Margin::symmetric(5, 1))
                        .show(ui, |ui| {
                            ui.label(RichText::new(items.len().to_string()).size(9.).color(MUTED));
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(25.);
                        if icons::button(ui, "trash", "Clear history", 24., MUTED).clicked() {
                            self.confirm_clear = true;
                        }
                        ui.label(RichText::new("Newest first").size(10.).color(MUTED));
                    });
                });
                ui.add_space(10.);
                if let Some(error) = &self.error {
                    ui.horizontal(|ui| {
                        ui.add_space(25.);
                        Frame::NONE
                            .fill(if light {
                                Color32::from_rgb(250, 238, 219)
                            } else {
                                Color32::from_rgb(51, 41, 32)
                            })
                            .corner_radius(6)
                            .inner_margin(9)
                            .show(ui, |ui| {
                                ui.set_max_width(bounds.width() - 70.);
                                ui.label(RichText::new(error).size(10.).color(if light {
                                    Color32::from_rgb(127, 91, 40)
                                } else {
                                    Color32::from_rgb(207, 178, 130)
                                }));
                            });
                    });
                    ui.add_space(7.);
                }
                if items.is_empty() {
                    self.empty(ui, !self.query.is_empty() || self.category != "All");
                } else {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(24.);
                                ui.vertical(|ui| {
                                    ui.set_width(bounds.width() - 48.);
                                    for (index, item) in items.iter().enumerate() {
                                        self.row(ui, ctx, item, index);
                                        ui.add_space(5.);
                                    }
                                });
                            });
                        });
                }
            });
        self.settings(ctx);
        self.dialogs(ctx);
        if let Some((message, start)) = &self.toast {
            if start.elapsed() < Duration::from_secs(3) && !self.hidden {
                Area::new(Id::new("toast"))
                    .anchor(Align2::CENTER_BOTTOM, vec2(0., -70.))
                    .order(Order::Foreground)
                    .show(ctx, |ui| {
                        Frame::NONE
                            .fill(Color32::from_rgb(55, 47, 74))
                            .corner_radius(8)
                            .inner_margin(12)
                            .show(ui, |ui| {
                                ui.label(RichText::new(message).color(ACCENT));
                            });
                    });
            }
        }
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
                        if let egui::Event::Screenshot { image, .. } = e {
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
                                "Screenshot saved: {} ({}x{}, {} frames)",
                                path.display(),
                                im.width(),
                                im.height(),
                                self.frames
                            );
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        Err(e) => {
                            eprintln!("Screenshot failed: {e}");
                            std::process::exit(1)
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
    ui.label(RichText::new(title).size(19.));
    ui.label(RichText::new(subtitle).size(12.).color(MUTED));
    ui.add_space(14.);
}
fn size_label(n: usize) -> String {
    if n >= 1024 * 1024 {
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
        format!("{}m ago", d / 60)
    } else if d < 86400 {
        format!("{}h ago", d / 3600)
    } else {
        format!("{}d ago", d / 86400)
    }
}
fn set_startup(enabled: bool) -> Result<(), String> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    let dir = config.join("autostart");
    let file = dir.join("clipvault.desktop");
    if enabled {
        // AppImages mount at temporary paths; login startup needs the stable image.
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        // A launcher such as Craft Anything can itself pass APPIMAGE to native
        // children. Use it only when this executable belongs to that AppDir.
        let image_root = std::env::var_os("APPDIR").and_then(|dir| std::fs::canonicalize(dir).ok());
        let executable = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|path| {
                path.is_absolute()
                    && path.is_file()
                    && image_root
                        .as_ref()
                        .is_some_and(|dir| executable.starts_with(dir))
            })
            .unwrap_or(executable);
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = executable
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
            .replace('%', "%%");
        std::fs::write(file,format!("[Desktop Entry]\nType=Application\nName=ClipVault\nComment=Private clipboard history\nExec=\"{path}\" --background\nIcon=edit-paste\nTerminal=false\nX-GNOME-Autostart-enabled=true\n")).map_err(|e|e.to_string())
    } else if file.exists() {
        std::fs::remove_file(file).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
