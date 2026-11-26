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
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_w: Option<i32>,
    pub window_h: Option<i32>,
    pub window_maximized: bool,
    pub ignored_update_version: Option<String>,
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
            window_x: None,
            window_y: None,
            window_w: None,
            window_h: None,
            window_maximized: false,
            ignored_update_version: None,
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
        let mut out = format!(
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
        );
        if let Some(v) = self.window_x {
            out.push_str(&format!("WindowX={v}\r\n"));
        }
        if let Some(v) = self.window_y {
            out.push_str(&format!("WindowY={v}\r\n"));
        }
        if let Some(v) = self.window_w {
            out.push_str(&format!("WindowW={v}\r\n"));
        }
        if let Some(v) = self.window_h {
            out.push_str(&format!("WindowH={v}\r\n"));
        }
        out.push_str(&format!("WindowMaximized={}\r\n", self.window_maximized));
        if let Some(ver) = &self.ignored_update_version {
            let trimmed = ver.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("IgnoredUpdateVersion={trimmed}\r\n"));
            }
        }
        out
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
                "WindowX" | "window_x" => {
                    if let Ok(val) = v.parse::<i32>() {
                        if (-20000..=20000).contains(&val) {
                            s.window_x = Some(val);
                        }
                    }
                }
                "WindowY" | "window_y" => {
                    if let Ok(val) = v.parse::<i32>() {
                        if (-20000..=20000).contains(&val) {
                            s.window_y = Some(val);
                        }
                    }
                }
                "WindowW" | "window_w" => {
                    if let Ok(val) = v.parse::<i32>() {
                        if (400..=16000).contains(&val) {
                            s.window_w = Some(val);
                        }
                    }
                }
                "WindowH" | "window_h" => {
                    if let Ok(val) = v.parse::<i32>() {
                        if (300..=16000).contains(&val) {
                            s.window_h = Some(val);
                        }
                    }
                }
                "WindowMaximized" | "window_maximized" => {
                    if let Some(b) = parse_bool(v) {
                        s.window_maximized = b;
                    }
                }
                "IgnoredUpdateVersion" | "ignored_update_version" => {
                    if v.is_empty() {
                        s.ignored_update_version = None;
                    } else {
                        s.ignored_update_version = Some(v.to_string());
                    }
                }
                _ => {}
            }
        }
        s
    }
}

/// Validated startup window placement in physical screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPlacementPx {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub maximized: bool,
}

/// Monitor work area rectangle (`rcWork` in Win32 `MONITORINFO`, physical pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorWorkRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Clamp a saved window rectangle `(x, y, w, h)` cleanly into a monitor's work area `work`.
/// Returns `None` if the rectangle has non-positive size or does not meaningfully intersect `work`.
pub fn clamp_rect_to_work_area(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    work: MonitorWorkRect,
) -> Option<(i32, i32, i32, i32)> {
    let work_w = work.right - work.left;
    let work_h = work.bottom - work.top;
    if w <= 0 || h <= 0 || work_w <= 0 || work_h <= 0 {
        return None;
    }

    let inter_left = x.max(work.left);
    let inter_top = y.max(work.top);
    let inter_right = x.saturating_add(w).min(work.right);
    let inter_bottom = y.saturating_add(h).min(work.bottom);
    let inter_w = inter_right - inter_left;
    let inter_h = inter_bottom - inter_top;
    if inter_w < 32.min(work_w) || inter_h < 32.min(work_h) {
        return None;
    }

    let clamped_w = w.clamp(960.min(work_w), work_w);
    let clamped_h = h.clamp(640.min(work_h), work_h);
    let max_x = (work.right - clamped_w).max(work.left);
    let max_y = (work.bottom - clamped_h).max(work.top);
    let clamped_x = x.clamp(work.left, max_x);
    let clamped_y = y.clamp(work.top, max_y);
    Some((clamped_x, clamped_y, clamped_w, clamped_h))
}

/// Center `(w, h)` inside `work` while clamping dimensions so it never exceeds `work`.
pub fn relocate_rect_into_work_area(w: i32, h: i32, work: MonitorWorkRect) -> (i32, i32, i32, i32) {
    let work_w = (work.right - work.left).max(1);
    let work_h = (work.bottom - work.top).max(1);
    let clamped_w = w.clamp(960.min(work_w), work_w);
    let clamped_h = h.clamp(640.min(work_h), work_h);
    let x = work.left + (work_w - clamped_w) / 2;
    let y = work.top + (work_h - clamped_h) / 2;
    (x, y, clamped_w, clamped_h)
}

#[cfg(windows)]
mod win32_monitor {
    use super::MonitorWorkRect;
    use core::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        rc_monitor: WinRect,
        rc_work: WinRect,
        dw_flags: u32,
    }

    const MONITOR_DEFAULTTONULL: u32 = 0;

    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromRect(lprc: *const WinRect, dw_flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(h_monitor: *mut c_void, lpmi: *mut MonitorInfo) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleA(lp_module_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    }

    pub fn enable_per_monitor_dpi_awareness() {
        unsafe {
            let user32 = GetModuleHandleA(b"user32.dll\0".as_ptr());
            if user32.is_null() {
                return;
            }
            let proc = GetProcAddress(user32, b"SetProcessDpiAwarenessContext\0".as_ptr());
            if !proc.is_null() {
                let func: extern "system" fn(isize) -> i32 = core::mem::transmute(proc);
                // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
                let _ = func(-4isize);
            }
        }
    }

    pub fn primary_monitor_scale_factor() -> f32 {
        unsafe {
            let user32 = GetModuleHandleA(b"user32.dll\0".as_ptr());
            if !user32.is_null() {
                let proc = GetProcAddress(user32, b"GetDpiForSystem\0".as_ptr());
                if !proc.is_null() {
                    let func: extern "system" fn() -> u32 = core::mem::transmute(proc);
                    let dpi = func();
                    if dpi >= 48 && dpi <= 960 {
                        return (dpi as f32 / 96.0).max(0.5);
                    }
                }
            }
        }
        1.0
    }

    pub fn monitor_work_area_for_rect(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> Option<(usize, MonitorWorkRect)> {
        if w <= 0 || h <= 0 {
            return None;
        }
        let rect = WinRect {
            left: x,
            top: y,
            right: x.saturating_add(w),
            bottom: y.saturating_add(h),
        };
        unsafe {
            let hmon = MonitorFromRect(&rect, MONITOR_DEFAULTTONULL);
            if hmon.is_null() {
                return None;
            }
            let mut info = MonitorInfo {
                cb_size: core::mem::size_of::<MonitorInfo>() as u32,
                rc_monitor: WinRect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                rc_work: WinRect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                dw_flags: 0,
            };
            if GetMonitorInfoW(hmon, &mut info) == 0 {
                return None;
            }
            Some((
                hmon as usize,
                MonitorWorkRect {
                    left: info.rc_work.left,
                    top: info.rc_work.top,
                    right: info.rc_work.right,
                    bottom: info.rc_work.bottom,
                },
            ))
        }
    }
}

/// Ensure Windows Per-Monitor DPI awareness is active before querying monitor coordinates.
pub fn enable_per_monitor_dpi_awareness() {
    #[cfg(windows)]
    win32_monitor::enable_per_monitor_dpi_awareness();
}

/// Primary monitor DPI scale factor (`1.0` for 96 DPI).
pub fn primary_monitor_scale_factor() -> f32 {
    #[cfg(windows)]
    {
        win32_monitor::primary_monitor_scale_factor()
    }
    #[cfg(not(windows))]
    {
        1.0
    }
}

/// Query the connected monitor handle (`usize`) and work area (`rcWork`) intersecting `(x, y, w, h)`.
/// Returns `None` if `(x, y, w, h)` does not intersect any currently connected monitor.
pub fn monitor_work_area_for_rect(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Option<(usize, MonitorWorkRect)> {
    #[cfg(windows)]
    {
        win32_monitor::monitor_work_area_for_rect(x, y, w, h)
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y, w, h);
        None
    }
}

/// Validate saved window geometry against currently connected monitors (`MonitorFromRect` + `GetMonitorInfoW`).
/// Returns `None` if no geometry is saved or the saved monitor is disconnected/off-screen.
pub fn validate_saved_window_placement(settings: &UserSettings) -> Option<WindowPlacementPx> {
    let (x, y, w, h) = (
        settings.window_x?,
        settings.window_y?,
        settings.window_w?,
        settings.window_h?,
    );
    let (_hmon, work) = monitor_work_area_for_rect(x, y, w, h)?;
    let (cx, cy, cw, ch) = clamp_rect_to_work_area(x, y, w, h, work)?;
    Some(WindowPlacementPx {
        x: cx,
        y: cy,
        w: cw,
        h: ch,
        maximized: settings.window_maximized,
    })
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

    // Parse & validate `[UserTemplatesForm]` before modifying `settings.ini` on disk.
    let parsed_template_doc = if let Some(form_text) = templates_part {
        let trimmed = form_text.trim();
        if !trimmed.is_empty() {
            Some(
                parse_form_text(trimmed, OlePreservedStreams::default())
                    .map_err(|e| format!("Şablon verisi ayrıştırılamadı: {e}"))?,
            )
        } else {
            None
        }
    } else {
        None
    };

    settings.save_to_path(&target_dir.join("settings.ini"))?;

    let mut templates_imported = false;
    if let Some(doc) = parsed_template_doc {
        let dst_shp = target_dir.join("defaultform_user.shp");
        save_file(&doc, &dst_shp)
            .map_err(|e| format!("Özel şablon dosyası kaydedilemedi: {e}"))?;
        templates_imported = true;
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
