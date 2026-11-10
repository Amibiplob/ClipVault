//! Use Fontconfig's installed outline fonts without copying them into packages.
use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};
use std::{collections::BTreeMap, path::PathBuf, process::Command};
#[derive(Clone, Debug)]
pub struct InstalledFont {
    pub family: String,
    path: PathBuf,
    index: u32,
}
fn parse(line: &str) -> Option<InstalledFont> {
    let mut fields = line.split('\t');
    let family = fields.next()?.trim().to_owned();
    let path = PathBuf::from(fields.next()?);
    let index = fields.next()?.parse().ok()?;
    if family.is_empty() || !matches!(path.extension()?.to_str()?, "ttf" | "otf" | "ttc" | "otc") {
        return None;
    }
    Some(InstalledFont {
        family,
        path,
        index,
    })
}
pub fn installed() -> Vec<InstalledFont> {
    let Ok(output) = Command::new("fc-list")
        .args([
            ":scalable=true",
            "-f",
            "%{family[0]}\t%{file}\t%{index}\t%{style[0]}\n",
        ])
        .output()
    else {
        return Vec::new();
    };
    // Prefer the regular face of each family. Enumeration stays deterministic.
    let lines = String::from_utf8_lossy(&output.stdout);
    let mut lines: Vec<_> = lines.lines().collect();
    lines.sort_by_key(|line| (!line.ends_with("Regular") && !line.ends_with("Book"), *line));
    let mut fonts = BTreeMap::new();
    for line in lines {
        if let Some(font) = parse(line) {
            fonts.entry(font.family.clone()).or_insert(font);
        }
    }
    fonts.into_values().collect()
}
fn system_default() -> Option<InstalledFont> {
    let output = Command::new("fc-match")
        .args(["sans-serif", "-f", "%{family[0]}\t%{file}\t%{index}\n"])
        .output()
        .ok()?;
    parse(String::from_utf8_lossy(&output.stdout).lines().next()?)
}
fn read(font: &InstalledFont) -> Option<FontData> {
    if std::fs::metadata(&font.path).ok()?.len() > 32 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(&font.path).ok()?;
    ab_glyph::FontRef::try_from_slice_and_index(&bytes, font.index).ok()?;
    let mut data = FontData::from_owned(bytes);
    data.index = font.index;
    Some(data)
}
/// Returns false if a saved choice is unavailable. System and bundled fallbacks remain usable.
pub fn apply(ctx: &Context, choice: &str, installed: &[InstalledFont]) -> bool {
    let mut definitions = FontDefinitions::default();
    definitions.font_data.insert(
        "interface".into(),
        FontData::from_static(include_bytes!("../assets/Interface.ttf")).into(),
    );
    let mut priority = vec!["interface".to_owned()];
    if let Some(data) = system_default().as_ref().and_then(read) {
        definitions.font_data.insert("system".into(), data.into());
        priority.insert(0, "system".into());
    }
    let selected = installed.iter().find(|f| f.family == choice).and_then(read);
    let available = choice.is_empty() || selected.is_some();
    if let Some(data) = selected {
        definitions.font_data.insert("selected".into(), data.into());
        priority.insert(0, "selected".into());
    }
    priority.extend(definitions.families[&FontFamily::Proportional].clone());
    definitions
        .families
        .insert(FontFamily::Proportional, priority);
    ctx.set_fonts(definitions);
    available
}
#[cfg(test)]
mod tests {
    #[test]
    fn missing_font_falls_back_and_installed_font_renders() {
        let ctx = eframe::egui::Context::default();
        let fonts = super::installed();
        assert!(!super::apply(&ctx, "ClipVault nonexistent font", &fonts));
        assert!(super::apply(&ctx, "", &fonts));
        if let Some(font) = fonts.iter().find(|f| super::read(f).is_some()) {
            assert!(super::apply(&ctx, &font.family, &fonts));
        }
        let _ = ctx.run(Default::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Sample: Aa 0123 বাংলা");
            });
        });
    }
}
