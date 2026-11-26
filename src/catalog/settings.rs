use std::io::Read;
use std::path::{Path, PathBuf};

use crate::io::parser::{decode_form_utf16le, parse_form_text};
use crate::io::save_file;
use crate::model::OlePreservedStreams;

/// Persistent user preferences stored in `%APPDATA%\kalipeditor\settings.ini`.
#[derive(Debug, Clone, PartialEq)]
pub struct UserSettings {
    pub default_density_x_per_cm: f64,
    pub default_density_y_per_cm: f64,
    pub default_template_is_user: bool,
    pub default_template_index: usize,
    pub auto_apply_default_template: bool,
    pub snap_even_rows: bool,
    pub show_dimensions: bool,
    pub show_detail_columns: bool,
    pub custom_stoll_defaultform_path: Option<PathBuf>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            default_density_x_per_cm: 3.0,
            default_density_y_per_cm: 3.0,
            default_template_is_user: false,
            default_template_index: 1, // "2 : CMS  >6< / <1>"
            auto_apply_default_template: true,
            snap_even_rows: false,
            show_dimensions: true,
            show_detail_columns: false,
            custom_stoll_defaultform_path: None,
        }
    }
}

impl UserSettings {
    /// Resolve `%APPDATA%\kalipeditor` (with safe fallback on non-Windows environments).
    pub fn appdata_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os("APPDATA") {
            return PathBuf::from(dir).join("kalipeditor");
        }
        if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(dir).join("kalipeditor");
        }
        if let Some(dir) = std::env::var_os("HOME") {
            return PathBuf::from(dir).join(".kalipeditor");
        }
        std::env::temp_dir().join("kalipeditor")
    }

    pub fn settings_ini_path() -> PathBuf {
        Self::appdata_dir().join("settings.ini")
    }

    pub fn user_template_shp_path() -> PathBuf {
        Self::appdata_dir().join("defaultform_user.shp")
    }

    /// Load settings from `%APPDATA%\kalipeditor\settings.ini` (returns `Default` if missing/unreadable).
    pub fn load() -> Self {
        Self::load_from_path(&Self::settings_ini_path())
    }

    /// Load settings from an explicit `.ini` file path.
    pub fn load_from_path(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => Self::from_ini_str(&content),
            Err(_) => Self::default(),
        }
    }

    /// Save settings to `%APPDATA%\kalipeditor\settings.ini`.
    pub fn save(&self) -> Result<(), String> {
        self.save_to_path(&Self::settings_ini_path())
    }

    /// Save settings to an explicit `.ini` file path.
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Ayar klasörü oluşturulamadı ({}): {e}", parent.display()))?;
        }
        std::fs::write(path, self.to_ini_string())
            .map_err(|e| format!("Ayarlar yazılamadı ({}): {e}", path.display()))
    }

    pub fn density_x_per_100mm(&self) -> f64 {
        (self.default_density_x_per_cm * 10.0).clamp(1.0, 500.0)
    }

    pub fn density_y_per_100mm(&self) -> f64 {
        (self.default_density_y_per_cm * 10.0).clamp(1.0, 500.0)
    }

    pub fn to_ini_string(&self) -> String {
        let custom_path = self
            .custom_stoll_defaultform_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!(
            "[Settings]\r\n\
             default_density_x_per_cm={:.4}\r\n\
             default_density_y_per_cm={:.4}\r\n\
             default_template_is_user={}\r\n\
             default_template_index={}\r\n\
             auto_apply_default_template={}\r\n\
             snap_even_rows={}\r\n\
             show_dimensions={}\r\n\
             show_detail_columns={}\r\n\
             custom_stoll_defaultform_path={}\r\n",
            self.default_density_x_per_cm,
            self.default_density_y_per_cm,
            self.default_template_is_user,
            self.default_template_index,
            self.auto_apply_default_template,
            self.snap_even_rows,
            self.show_dimensions,
            self.show_detail_columns,
            custom_path,
        )
    }

    pub fn from_ini_str(text: &str) -> Self {
        let mut s = Self::default();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with(';')
                || line.starts_with('[')
            {
                continue;
            }
            let Some((k_raw, v_raw)) = line.split_once('=') else {
                continue;
            };
            let k = k_raw.trim();
            let v = v_raw.trim();
            match k {
                "default_density_x_per_cm" => {
                    if let Ok(val) = v.replace(',', ".").parse::<f64>() {
                        if val.is_finite() && val > 0.0 {
                            s.default_density_x_per_cm = val.clamp(0.1, 50.0);
                        }
                    }
                }
                "default_density_y_per_cm" => {
                    if let Ok(val) = v.replace(',', ".").parse::<f64>() {
                        if val.is_finite() && val > 0.0 {
                            s.default_density_y_per_cm = val.clamp(0.1, 50.0);
                        }
                    }
                }
                "default_template_is_user" => {
                    if let Some(b) = parse_bool(v) {
                        s.default_template_is_user = b;
                    }
                }
                "default_template_index" => {
                    if let Ok(idx) = v.parse::<usize>() {
                        s.default_template_index = idx;
                    }
                }
                "auto_apply_default_template" => {
                    if let Some(b) = parse_bool(v) {
                        s.auto_apply_default_template = b;
                    }
                }
                "snap_even_rows" => {
                    if let Some(b) = parse_bool(v) {
                        s.snap_even_rows = b;
                    }
                }
                "show_dimensions" => {
                    if let Some(b) = parse_bool(v) {
                        s.show_dimensions = b;
                    }
                }
                "show_detail_columns" => {
                    if let Some(b) = parse_bool(v) {
                        s.show_detail_columns = b;
                    }
                }
                "custom_stoll_defaultform_path" => {
                    if v.is_empty() {
                        s.custom_stoll_defaultform_path = None;
                    } else {
                        s.custom_stoll_defaultform_path = Some(PathBuf::from(v));
                    }
                }
                _ => {}
            }
        }
        s
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "ja" => Some(true),
        "false" | "0" | "no" | "nein" => Some(false),
        _ => None,
    }
}

const BUNDLE_TEMPLATE_HEADER: &str = "[UserTemplatesForm]";

/// Export `settings` and optional `%APPDATA%\kalipeditor\defaultform_user.shp` into a single
/// portable UTF-8 `.kalipcfg` bundle file.
pub fn export_settings_bundle(settings: &UserSettings, dest_path: &Path) -> Result<(), String> {
    export_settings_bundle_with_template_path(
        settings,
        &UserSettings::user_template_shp_path(),
        dest_path,
    )
}

/// Export `settings` and an explicit `defaultform_user.shp` path into a portable `.kalipcfg` bundle.
pub fn export_settings_bundle_with_template_path(
    settings: &UserSettings,
    user_shp_path: &Path,
    dest_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("Hedef klasör oluşturulamadı ({}): {e}", parent.display())
            })?;
        }
    }

    let mut out = settings.to_ini_string();
    if user_shp_path.exists() {
        let form_text = read_shp_form_text(user_shp_path)?;
        if !form_text.trim().is_empty() {
            out.push_str("\r\n");
            out.push_str(BUNDLE_TEMPLATE_HEADER);
            out.push_str("\r\n");
            out.push_str(&form_text);
            if !out.ends_with('\n') {
                out.push_str("\r\n");
            }
        }
    }

    std::fs::write(dest_path, out)
        .map_err(|e| format!("Ayar paketi yazılamadı ({}): {e}", dest_path.display()))
}

/// Import a `.kalipcfg` bundle into `%APPDATA%\kalipeditor\`, restoring both `settings.ini` and
/// `defaultform_user.shp` (when `[UserTemplatesForm]` is present). Returns `(settings, templates_imported)`.
pub fn import_settings_bundle(src_path: &Path) -> Result<(UserSettings, bool), String> {
    import_settings_bundle_to_dir(src_path, &UserSettings::appdata_dir())
}

/// Import a `.kalipcfg` bundle into an explicit target directory (useful for isolated tests).
pub fn import_settings_bundle_to_dir(
    src_path: &Path,
    target_dir: &Path,
) -> Result<(UserSettings, bool), String> {
    let raw = std::fs::read_to_string(src_path)
        .map_err(|e| format!("Ayar paketi okunamadı ({}): {e}", src_path.display()))?;

    let (settings_part, templates_part) = split_bundle_sections(&raw);
    let settings = UserSettings::from_ini_str(settings_part);
    settings.save_to_path(&target_dir.join("settings.ini"))?;

    let mut templates_imported = false;
    if let Some(form_text) = templates_part {
        let trimmed = form_text.trim();
        if !trimmed.is_empty() {
            let doc = parse_form_text(trimmed, OlePreservedStreams::default())
                .map_err(|e| format!("Şablon verisi ayrıştırılamadı: {e}"))?;
            let dst_shp = target_dir.join("defaultform_user.shp");
            save_file(&doc, &dst_shp)
                .map_err(|e| format!("Özel şablon dosyası kaydedilemedi: {e}"))?;
            templates_imported = true;
        }
    }

    Ok((settings, templates_imported))
}

fn split_bundle_sections(raw: &str) -> (&str, Option<&str>) {
    let mut offset = 0usize;
    for line in raw.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(BUNDLE_TEMPLATE_HEADER) {
            let before = &raw[..offset];
            let after = &raw[offset + line.len()..];
            return (before, Some(after));
        }
        offset += line.len();
    }
    (raw, None)
}

fn read_shp_form_text(shp_path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(shp_path)
        .map_err(|e| format!("Şablon dosyası okunamadı ({}): {e}", shp_path.display()))?;
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bytes))
        .map_err(|e| format!("OLE2 şablon dosyası geçersiz ({}): {e}", shp_path.display()))?;
    let mut form_bytes = Vec::new();
    comp.open_stream("/Form")
        .and_then(|mut s| s.read_to_end(&mut form_bytes))
        .map_err(|e| format!("Form akışı okunamadı ({}): {e}", shp_path.display()))?;
    let (text, _bom, _trailing) = decode_form_utf16le(&form_bytes)?;
    Ok(text)
}
