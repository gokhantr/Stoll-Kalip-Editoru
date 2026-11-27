pub mod settings;

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::io::bitmap::PreviewImage;
use crate::io::{load_file, save_file};
use crate::model::{
    BindingOffProps, FadeOutProps, FormDocument, KanteChildSlot, LineFunction, LineSegment,
    LineSide, ModuleAssignment, NarrowingProps, RawSectionMeta, ShapeCutType, WideningProps,
};
pub use settings::{
    clamp_rect_to_work_area, enable_per_monitor_dpi_awareness, export_settings_bundle,
    export_settings_bundle_with_template_path, import_settings_bundle,
    import_settings_bundle_to_dir, monitor_work_area_for_rect, primary_monitor_scale_factor,
    relocate_rect_into_work_area, validate_saved_window_placement, MonitorWorkRect, UserSettings,
    WindowPlacementPx,
};

/// Stoll knitting structure definition (`[ModuleArt] = id`), 1:1 with `messagetextresources_7.dll`.
#[derive(Debug, Clone, Copy)]
pub struct KnittingStructure {
    pub id: i32,
    pub name_tr: &'static str,
    pub name_en: &'static str,
}

/// Complete Stoll knitting structure list (`0..=12`, plus `-1` sentinel).
pub const KNITTING_STRUCTURES: &[KnittingStructure] = &[
    KnittingStructure { id: 0, name_tr: "Strüktür tekplaka", name_en: "Structure single jersey" },
    KnittingStructure { id: 1, name_tr: "Strüktür dolu iğne", name_en: "Structure full needle" },
    KnittingStructure { id: 2, name_tr: "Jac-Dolu", name_en: "Jacquard full" },
    KnittingStructure { id: 3, name_tr: "Jac-Pike", name_en: "Jacquard pique" },
    KnittingStructure { id: 4, name_tr: "Jac-File", name_en: "Jacquard net" },
    KnittingStructure { id: 5, name_tr: "Jac-Atlama", name_en: "Jacquard float" },
    KnittingStructure { id: 6, name_tr: "1x1-MG Strüktür tekplaka", name_en: "1x1-MG Structure single jersey" },
    KnittingStructure { id: 7, name_tr: "1x1-MG Strüktür çiftplaka", name_en: "1x1-MG Structure double jersey" },
    KnittingStructure { id: 8, name_tr: "1x1-MG Jac-Dolu", name_en: "1x1-MG Jacquard full" },
    KnittingStructure { id: 9, name_tr: "1x1-MG Jac-Pike", name_en: "1x1-MG Jacquard pique" },
    KnittingStructure { id: 10, name_tr: "1x1-MG Jac-File", name_en: "1x1-MG Jacquard net" },
    KnittingStructure { id: 11, name_tr: "1x1-MG Jac-Atlama", name_en: "1x1-MG Jacquard float" },
    KnittingStructure { id: 12, name_tr: "Pelüş", name_en: "Plush" },
];

pub fn knitting_structure_name_tr(id: i32) -> &'static str {
    if id == -1 {
        return "Tüm örgü türleri";
    }
    KNITTING_STRUCTURES
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name_tr)
        .unwrap_or("Diğer örgü türü")
}

/// Standard step pattern preset from `ConfigData\linesteps.dat`.
#[derive(Debug, Clone, Copy)]
pub struct LineStepPreset {
    pub label: &'static str,
    pub step_x: i32,
    pub step_y: i32,
}

pub const STANDARD_LINE_STEP_PRESETS: &[LineStepPreset] = &[
    LineStepPreset { label: "1:1 ---", step_x: 1, step_y: 1 },
    LineStepPreset { label: "1:1 |", step_x: 0, step_y: 1 },
    LineStepPreset { label: "1:1 X", step_x: 1, step_y: 1 },
    LineStepPreset { label: "1:2 ---", step_x: 1, step_y: 2 },
    LineStepPreset { label: "1:2 |", step_x: 0, step_y: 2 },
    LineStepPreset { label: "1:3 ---", step_x: 1, step_y: 3 },
    LineStepPreset { label: "1:3 |", step_x: 0, step_y: 3 },
    LineStepPreset { label: "1:4 ---", step_x: 1, step_y: 4 },
    LineStepPreset { label: "1:4 |", step_x: 0, step_y: 4 },
    LineStepPreset { label: "1x2 X", step_x: 1, step_y: 2 },
    LineStepPreset { label: "2x1 X", step_x: 2, step_y: 1 },
    LineStepPreset { label: "2x2 /", step_x: 2, step_y: 2 },
    LineStepPreset { label: "2x2 \\", step_x: -2, step_y: 2 },
    LineStepPreset { label: "2x3 /", step_x: 2, step_y: 3 },
    LineStepPreset { label: "2x3 \\", step_x: -2, step_y: 3 },
    LineStepPreset { label: "2x4 /", step_x: 2, step_y: 4 },
    LineStepPreset { label: "2x4 \\", step_x: -2, step_y: 4 },
];

/// Functional category for Stoll knitting module dropdowns (`[ENG-01]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleCategory {
    /// Reglan (`[Mindern]`)
    Narrowing,
    /// Çok kademeli reglan (`[Narrowing multistage]`)
    NarrowingMultistage,
    /// Artırma (`[Zunahme]`)
    Widening,
    /// Remayözleme — Modül (`[Abketteln]`)
    BindOff,
    /// Remayözleme — RA (`[Bind-off-CA]`)
    BindOffCa,
    /// Kenar düzeltme (`[Ausblenden]`)
    FadeOut,
    /// Başlama (`[Shape edge Start]`)
    Start,
    /// V-Yaka Başlama (`[Start V-neck]`)
    StartVneck,
    /// Bitiş (`[Shape edge End]`)
    End,
    /// İşaretleme (`[Shape-Marking]`)
    Point,
}

impl ModuleCategory {
    pub fn label_tr(self) -> &'static str {
        match self {
            Self::Narrowing => "Reglan",
            Self::NarrowingMultistage => "Çok kademeli reglan",
            Self::Widening => "Artırma",
            Self::BindOff => "Remayözleme (Modül)",
            Self::BindOffCa => "Remayözleme (RA)",
            Self::FadeOut => "Kenar düzeltme",
            Self::Start => "Başlama",
            Self::StartVneck => "V-Yaka Başlama",
            Self::End => "Bitiş",
            Self::Point => "İşaretleme",
        }
    }

    /// Categories whose Stoll DB root is flat (not subdivided by the 12 knitting structure slots).
    pub fn is_flat_category(self) -> bool {
        matches!(self, Self::NarrowingMultistage | Self::Point)
    }

    pub fn from_category_str(s: &str) -> Option<Self> {
        match s {
            "Mindern" | "Narrowing" | "Reglan" => Some(Self::Narrowing),
            "NarrowingMultistage" | "Narrowing multistage" | "Çok kademeli reglan" => {
                Some(Self::NarrowingMultistage)
            }
            "Zunehmen" | "Zunahme" | "Widening" | "Artırma" => Some(Self::Widening),
            "Abketteln" | "BindOff" | "Remayözleme" | "Remayözleme (Modül)" => Some(Self::BindOff),
            "Bind-off-CA" | "BindOffCa" | "RA" | "Remayözleme (RA)" => Some(Self::BindOffCa),
            "Ausblenden" | "FadeOut" | "Kenar düzeltme" => Some(Self::FadeOut),
            "Start" | "Shape edge Start" | "Başlama" => Some(Self::Start),
            "StartVneck" | "Start V-neck" | "V-Yaka Başlama" => Some(Self::StartVneck),
            "End" | "Shape edge End" | "Bitiş" => Some(Self::End),
            "Punkt" | "Point" | "Shape-Marking" | "İşaretleme" => Some(Self::Point),
            _ => None,
        }
    }
}

/// A named knitting module from the Stoll module catalog (GUID -> name, category, optional 128x128 preview).
#[derive(Debug, Clone)]
pub struct KnittingModuleInfo {
    pub guid: String,
    pub name: String,
    pub category: String,
    pub thumbnail: Option<PreviewImage>,
}

/// A Standard Property preset ("Standart özellikler": `Stoll` vs `Kendinin`).
#[derive(Debug, Clone)]
pub struct StandardPreset {
    pub id: i32,
    pub display_name: String,
    pub is_user_custom: bool,
    pub template_line: LineSegment,
}

/// Runtime + built-in catalog of Stoll standard presets & knitting modules.
#[derive(Debug, Clone)]
pub struct StollCatalog {
    pub active_cut_type: ShapeCutType,
    pub stoll_presets: Vec<StandardPreset>,
    pub user_presets: Vec<StandardPreset>,
    pub modules_by_guid: HashMap<String, KnittingModuleInfo>,
    pub category_guids: HashMap<ModuleCategory, Vec<String>>,
    pub category_struct_guids: HashMap<(ModuleCategory, i32), Vec<String>>,
    pub stoll_data_root: Option<PathBuf>,
    pub custom_stoll_defaultform_path: Option<PathBuf>,
}

impl Default for StollCatalog {
    fn default() -> Self {
        Self::load_with_autodetect()
    }
}

impl StollCatalog {
    /// Auto-detect local Stoll M1plus data directory (`ConfigData` + `Database\Module\new`),
    /// always falling back cleanly to embedded built-in presets/modules for standalone single-`.exe` portability.
    pub fn load_with_autodetect() -> Self {
        let candidates = [
            PathBuf::from(r"D:\Stoll\M1plus\8.0.010"),
            PathBuf::from(r"C:\Stoll\M1plus\8.0.010"),
            PathBuf::from(r"D:\Stoll\M1plus"),
            PathBuf::from(r"C:\Stoll\M1plus"),
        ];
        let found = candidates
            .into_iter()
            .find(|p| p.join("ConfigData").join("defaultform.shp").exists());
        Self::load_from_root(found.as_deref(), ShapeCutType::Standard)
    }

    pub fn load_from_root(stoll_root: Option<&Path>, cut_type: ShapeCutType) -> Self {
        let settings = UserSettings::load();
        let mut catalog = Self {
            active_cut_type: cut_type,
            stoll_presets: Vec::new(),
            user_presets: Vec::new(),
            modules_by_guid: builtin_modules_map(),
            category_guids: HashMap::new(),
            category_struct_guids: HashMap::new(),
            stoll_data_root: stoll_root.map(|p| p.to_path_buf()),
            custom_stoll_defaultform_path: settings.custom_stoll_defaultform_path,
        };

        catalog.reload_for_cut_type(cut_type);

        if let Some(root) = stoll_root {
            // Index strictly from Stoll DB trees in exact `M3_MOD_DirChildren` display order.
            // Harvest `defaultform*.shp` only into the metadata cache so deleted/legacy GUIDs
            // resolve cleanly without polluting any structure dropdown or changing DB order.
            catalog.index_stoll_db_categories(root);
            catalog.harvest_defaultform_modules(root);
        } else {
            // Offline fallback when no Stoll M1plus installation is present on disk
            for &(guid, _name, cat, struct_id) in builtin_module_entries() {
                catalog.register_category_guid(cat, struct_id, guid);
            }
        }

        catalog
    }

    /// Reload `Standart özellikler` presets when `SchnittArt` (`Tür`) changes (`[ENG-02]`),
    /// preserving all already-indexed modules and cached thumbnails.
    /// User presets load from `%APPDATA%\kalipeditor\defaultform_user.shp` first, falling back
    /// to Stoll `ConfigData\defaultform_user.shp` if the `%APPDATA%` file does not exist yet.
    pub fn reload_for_cut_type(&mut self, cut_type: ShapeCutType) {
        self.active_cut_type = cut_type;
        let mut stoll_presets = Vec::new();
        let mut user_presets = Vec::new();

        if cut_type == ShapeCutType::Standard {
            if let Some(ref custom_path) = self.custom_stoll_defaultform_path {
                if custom_path.exists() {
                    load_presets_from_shp(custom_path, false, &mut stoll_presets);
                }
            }
        }

        let appdata_user_shp = UserSettings::user_template_shp_path();
        let appdata_user_exists = appdata_user_shp.exists();
        if appdata_user_exists {
            load_presets_from_shp(&appdata_user_shp, true, &mut user_presets);
        }

        if let Some(ref root) = self.stoll_data_root {
            let config_dir = root.join("ConfigData");
            let (stoll_file, user_file) = match cut_type {
                ShapeCutType::Standard => ("defaultform.shp", "defaultform_user.shp"),
                ShapeCutType::KwBodyFront | ShapeCutType::KwBodyBack => {
                    ("defaultformk&wbody.shp", "defaultformk&wbody_user.shp")
                }
                ShapeCutType::KwSleeves => {
                    ("defaultformk&wsleeve.shp", "defaultformk&wsleeve_user.shp")
                }
                ShapeCutType::KwNeck => ("defaultformk&wneck.shp", "defaultformk&wneck_user.shp"),
            };

            if stoll_presets.is_empty() {
                load_presets_from_shp(&config_dir.join(stoll_file), false, &mut stoll_presets);
            }
            if !appdata_user_exists {
                load_presets_from_shp(&config_dir.join(user_file), true, &mut user_presets);
            }
        }

        if stoll_presets.is_empty() {
            stoll_presets = builtin_stoll_presets();
        }

        self.stoll_presets = stoll_presets;
        self.user_presets = user_presets;
    }

    /// Reload `user_presets` from an explicit `defaultform_user.shp` path.
    pub fn reload_user_presets_from(&mut self, path: &Path) {
        let mut user_presets = Vec::new();
        if path.exists() {
            load_presets_from_shp(path, true, &mut user_presets);
        }
        self.user_presets = user_presets;
    }

    /// Save a line's knitting properties as a new custom user preset inside
    /// `%APPDATA%\kalipeditor\defaultform_user.shp` and reload `user_presets`.
    pub fn save_line_as_user_preset(
        &mut self,
        name: &str,
        line: &LineSegment,
    ) -> Result<usize, String> {
        let path = UserSettings::user_template_shp_path();
        self.save_line_as_user_preset_to(&path, name, line)
    }

    /// Save a line's knitting properties as a custom user preset to an explicit `.shp` path.
    pub fn save_line_as_user_preset_to(
        &mut self,
        user_shp_path: &Path,
        name: &str,
        line: &LineSegment,
    ) -> Result<usize, String> {
        if let Some(parent) = user_shp_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Şablon klasörü oluşturulamadı ({}): {e}", parent.display())
                })?;
            }
        }

        let mut doc = if user_shp_path.exists() {
            load_file(user_shp_path)
                .map_err(|e| format!("Mevcut şablon dosyası okunamadı: {e}"))?
        } else {
            let mut d = FormDocument::new_default_template();
            d.header.name = "Default Form".to_string();
            if let Some(el) = d.elements.first_mut() {
                el.name = "Grundform".to_string();
                el.mirrored = false;
                for preset in &self.user_presets {
                    el.left_lines.push(preset.template_line.clone());
                }
                el.renumber_lines(LineSide::Left);
            }
            d
        };

        if doc.elements.is_empty() {
            doc = FormDocument::new_default_template();
            doc.header.name = "Default Form".to_string();
        }
        if doc.header.name.trim().is_empty() {
            doc.header.name = "Default Form".to_string();
        }

        let clean_name = {
            let t = name.trim();
            if !t.is_empty() {
                t.to_string()
            } else if !line.comment.trim().is_empty() {
                line.comment.trim().to_string()
            } else {
                "Özel Şablon".to_string()
            }
        };

        let next_tag = doc.next_line_tag_id();
        let mut tpl = line.clone();
        tpl.raw = RawSectionMeta::default();
        tpl.guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
        tpl.param_guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
        tpl.tag_id = next_tag;
        tpl.width_mm = 0.0;
        tpl.height_mm = 0.0;
        tpl.stitches = 0;
        tpl.rows = 0;
        tpl.step_x = 0;
        tpl.step_y = 0;
        tpl.step_factor = 1;
        tpl.rest_x = 0;
        tpl.rest_y = 0;
        tpl.kurven_form = 0;
        tpl.curve = None;
        tpl.child_order
            .retain(|s| !matches!(s, KanteChildSlot::Curve));
        tpl.function = LineFunction::None;
        tpl.user_set_function = false;
        tpl.last_line = 0;
        tpl.comment = clean_name;

        if tpl.fade_out.is_some() {
            tpl.ensure_child_slot(KanteChildSlot::FadeOut);
        }
        if tpl.narrowing.is_some() {
            tpl.ensure_child_slot(KanteChildSlot::Narrowing);
        }
        if tpl.widening.is_some() {
            tpl.ensure_child_slot(KanteChildSlot::Widening);
        }
        if tpl.binding_off.is_some() {
            tpl.ensure_child_slot(KanteChildSlot::BindingOff);
        }
        for i in 0..tpl.points.len() {
            tpl.ensure_child_slot(KanteChildSlot::Point(i));
        }

        let el = &mut doc.elements[0];
        el.mirrored = false;
        el.has_left_lines_section = true;
        el.left_lines.push(tpl);
        el.renumber_lines(LineSide::Left);

        save_file(&doc, user_shp_path)
            .map_err(|e| format!("Özel şablon kaydedilemedi: {e}"))?;
        self.reload_user_presets_from(user_shp_path);
        Ok(self.user_presets.len().saturating_sub(1))
    }

    /// Delete a custom user preset at `idx` from `%APPDATA%\kalipeditor\defaultform_user.shp`
    /// and reload `user_presets`.
    pub fn delete_user_preset(&mut self, idx: usize) -> Result<(), String> {
        let path = UserSettings::user_template_shp_path();
        self.delete_user_preset_from(&path, idx)
    }

    /// Delete a custom user preset at `idx` from an explicit `.shp` path.
    pub fn delete_user_preset_from(
        &mut self,
        user_shp_path: &Path,
        idx: usize,
    ) -> Result<(), String> {
        if idx >= self.user_presets.len() {
            return Err("Geçersiz özel şablon indeksi.".to_string());
        }
        if let Some(parent) = user_shp_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Şablon klasörü oluşturulamadı ({}): {e}", parent.display())
                })?;
            }
        }

        let mut doc = if user_shp_path.exists() {
            load_file(user_shp_path)
                .map_err(|e| format!("Şablon dosyası okunamadı: {e}"))?
        } else {
            let mut d = FormDocument::new_default_template();
            d.header.name = "Default Form".to_string();
            if let Some(el) = d.elements.first_mut() {
                el.name = "Grundform".to_string();
                el.mirrored = false;
                for preset in &self.user_presets {
                    el.left_lines.push(preset.template_line.clone());
                }
                el.renumber_lines(LineSide::Left);
            }
            d
        };

        let Some(el) = doc.elements.first_mut() else {
            return Err("Şablon dosyasında eleman bulunamadı.".to_string());
        };
        if idx >= el.left_lines.len() {
            return Err("Şablon satırı bulunamadı.".to_string());
        }
        el.left_lines.remove(idx);
        el.renumber_lines(LineSide::Left);

        save_file(&doc, user_shp_path)
            .map_err(|e| format!("Özel şablon güncellenemedi: {e}"))?;
        self.reload_user_presets_from(user_shp_path);
        Ok(())
    }

    pub fn presets(&self, is_user_custom: bool) -> &[StandardPreset] {
        if is_user_custom {
            &self.user_presets
        } else {
            &self.stoll_presets
        }
    }

    /// Return all indexed Stoll modules for a functional category (`[ENG-01]`)
    /// in exact Stoll database display order (`M3_MOD_DirChildren`).
    pub fn modules_for_category(&self, category: ModuleCategory) -> Vec<&KnittingModuleInfo> {
        self.category_guids
            .get(&category)
            .map(|guids| {
                guids
                    .iter()
                    .filter_map(|g| self.modules_by_guid.get(g))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return Stoll modules for a specific `(category, structure_id)` slot (`0..=11`)
    /// in exact Stoll `M3_MOD_DirChildren` order.
    ///
    /// Matches Stoll M1plus `vecformeditor.exe` / `formtable.dll`:
    /// - Flat categories (`NarrowingMultistage`, `Point`) and `structure_id < 0` return
    ///   `modules_for_category(category)`.
    /// - Slotted categories (`0..=11`) return ONLY modules belonging to that knitting
    ///   structure; an empty slot returns an empty `Vec` (never leaking other structures).
    pub fn modules_for_category_and_structure(
        &self,
        category: ModuleCategory,
        structure_id: i32,
    ) -> Vec<&KnittingModuleInfo> {
        if category.is_flat_category() || structure_id < 0 {
            return self.modules_for_category(category);
        }
        self.category_struct_guids
            .get(&(category, structure_id))
            .map(|guids| {
                guids
                    .iter()
                    .filter_map(|g| self.modules_by_guid.get(g))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Infer Stoll `Remayözleme-tipi` (`0 = Modül`, `1 = RA`) from a module GUID's root membership.
    pub fn bind_off_type_for_guid(&self, guid: &str) -> usize {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == "{00000000-0000-0000-0000-000000000000}" {
            return 0;
        }
        let in_mod = self
            .category_guids
            .get(&ModuleCategory::BindOff)
            .is_some_and(|v| v.contains(&norm));
        let in_ca = self
            .category_guids
            .get(&ModuleCategory::BindOffCa)
            .is_some_and(|v| v.contains(&norm));
        if in_ca && !in_mod {
            1
        } else {
            0
        }
    }

    /// Returns `true` if `guid` belongs to the flat `NarrowingMultistage` (`{34FFFFF6-...}`) DB tree.
    pub fn is_narrowing_multistage_guid(&self, guid: &str) -> bool {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == "{00000000-0000-0000-0000-000000000000}" {
            return false;
        }
        self.category_guids
            .get(&ModuleCategory::NarrowingMultistage)
            .is_some_and(|v| v.contains(&norm))
    }

    fn register_category_guid(&mut self, cat: ModuleCategory, struct_id: Option<i32>, guid: &str) {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == "{00000000-0000-0000-0000-000000000000}" {
            return;
        }
        let entry = self.category_guids.entry(cat).or_default();
        if !entry.contains(&norm) {
            entry.push(norm.clone());
        }
        if let Some(sid) = struct_id {
            let s_entry = self.category_struct_guids.entry((cat, sid)).or_default();
            if !s_entry.contains(&norm) {
                s_entry.push(norm);
            }
        }
    }

    fn harvest_defaultform_modules(&mut self, root: &Path) {
        let config_dir = root.join("ConfigData");
        let files = [
            "defaultform.shp",
            "defaultform_user.shp",
            "defaultformk&wbody.shp",
            "defaultformk&wneck.shp",
            "defaultformk&wsleeve.shp",
        ];
        for fname in files {
            let p = config_dir.join(fname);
            let Ok(doc) = load_file(&p) else {
                continue;
            };
            for el in &doc.elements {
                for line in el.left_lines.iter().chain(el.right_lines.iter()) {
                    if let Some(ref n) = line.narrowing {
                        for m in &n.modules {
                            self.register_harvested_module(root, ModuleCategory::Narrowing, m.module_art_id, &m.id);
                        }
                    }
                    if let Some(ref w) = line.widening {
                        for m in &w.modules {
                            self.register_harvested_module(root, ModuleCategory::Widening, m.module_art_id, &m.id);
                        }
                    }
                    if let Some(ref b) = line.binding_off {
                        for m in &b.modules {
                            self.register_harvested_module(root, ModuleCategory::BindOff, m.module_art_id, &m.id);
                        }
                    }
                    if let Some(ref f) = line.fade_out {
                        for m in &f.modules {
                            self.register_harvested_module(root, ModuleCategory::FadeOut, m.module_art_id, &m.id);
                        }
                    }
                    for pt in &line.points {
                        for m in &pt.modules {
                            self.register_harvested_module(root, ModuleCategory::Point, m.module_art_id, &m.id);
                        }
                    }
                }
            }
        }
    }

    fn register_harvested_module(
        &mut self,
        root: &Path,
        cat: ModuleCategory,
        _struct_id: i32,
        guid: &str,
    ) {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == "{00000000-0000-0000-0000-000000000000}" {
            return;
        }
        // Only populate metadata cache (`modules_by_guid`), never category dropdown lists,
        // so `defaultformk&w*.shp` or preset ordering cannot pollute structure dropdowns.
        if !self.modules_by_guid.contains_key(&norm) {
            if let Some(mut info) = load_stoll_module_metadata_only(root, &norm) {
                info.category = cat.label_tr().to_string();
                self.modules_by_guid.insert(norm, info);
            }
        }
    }

    /// Traverse Stoll's exact `Technik` category root GUIDs in `Database\Module\new\` via `M3_MOD_DirChildren`,
    /// populating `category_guids`, `category_struct_guids` (0..=11), and `modules_by_guid` (metadata only;
    /// bitmaps are loaded lazily in `resolve_module`).
    fn index_stoll_db_categories(&mut self, root: &Path) {
        struct CategoryRootSpec {
            cat: ModuleCategory,
            root_guid: &'static str,
            has_structure_slots: bool,
            include_dirs: bool,
        }

        // Exact Stoll M1plus 8.0 Technik category roots & directory traversal flags verified from
        // `formtable.dll` (`UpdateModuleDBList` / `FillModuleListFromRootDirectory`) and `Database\Module\new`.
        let roots: &[CategoryRootSpec] = &[
            CategoryRootSpec {
                cat: ModuleCategory::Narrowing,
                root_guid: "{4D55434B-0000-0000-000D-4B41524C4900}", // Mindern (12 slots)
                has_structure_slots: true,
                include_dirs: true,
            },
            CategoryRootSpec {
                cat: ModuleCategory::NarrowingMultistage,
                root_guid: "{34FFFFF6-ECF6-4A53-AD06-347585B2E6C8}", // Narrowing multistage (flat)
                has_structure_slots: false,
                include_dirs: true,
            },
            CategoryRootSpec {
                cat: ModuleCategory::Widening,
                root_guid: "{4D55434B-0000-0000-000F-4B41524C4900}", // Zunahme (12 slots)
                has_structure_slots: true,
                include_dirs: true,
            },
            CategoryRootSpec {
                cat: ModuleCategory::BindOff,
                root_guid: "{4D55434B-0000-0000-000E-4B41524C4900}", // Abketteln ("Modül", 12 slots)
                has_structure_slots: true,
                include_dirs: true,
            },
            CategoryRootSpec {
                cat: ModuleCategory::BindOffCa,
                root_guid: "{F39139E7-520E-4104-8BBA-A4D8A034FA80}", // Bind-off-CA ("RA", 12 slots)
                has_structure_slots: true,
                include_dirs: true,
            },
            CategoryRootSpec {
                cat: ModuleCategory::FadeOut,
                root_guid: "{4471D73D-1517-4989-A1AC-81239CFC4F9C}", // Ausblenden (12 slots, skip 'Yeni modül grubu' folders)
                has_structure_slots: true,
                include_dirs: false,
            },
            CategoryRootSpec {
                cat: ModuleCategory::Start,
                root_guid: "{68F75A9D-F84B-4DBF-B46F-DA9CE3032D2D}", // Shape edge Start (12 slots, skip `element`)
                has_structure_slots: true,
                include_dirs: false,
            },
            CategoryRootSpec {
                cat: ModuleCategory::StartVneck,
                root_guid: "{FB05F2AF-9D7E-11D3-BE1C-00A0C9ECEF97}", // Start V-neck (12 slots, skip `Elements`)
                has_structure_slots: true,
                include_dirs: false,
            },
            CategoryRootSpec {
                cat: ModuleCategory::End,
                root_guid: "{018C72B8-1407-4735-A596-ABB5B90A1C96}", // Shape edge End (12 slots, skip `ELEMENT`)
                has_structure_slots: true,
                include_dirs: false,
            },
            CategoryRootSpec {
                cat: ModuleCategory::Point,
                root_guid: "{D28C962A-D85C-49CE-8015-212A4E79B40A}", // Shape-Marking (flat)
                has_structure_slots: false,
                include_dirs: true,
            },
        ];

        let mut known_dir_guids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for spec in roots {
            if spec.has_structure_slots {
                let slot_dirs = read_dir_children_for_guid(root, spec.root_guid);
                for (slot_idx, slot_dir_guid) in slot_dirs
                    .into_iter()
                    .take(KNITTING_STRUCTURES.len())
                    .enumerate()
                {
                    let struct_id = Some(slot_idx as i32);
                    let items = read_dir_children_for_guid(root, &slot_dir_guid);
                    for item_guid in items {
                        self.index_leaf_module(
                            root,
                            spec.cat,
                            struct_id,
                            &item_guid,
                            !spec.include_dirs,
                            &mut known_dir_guids,
                        );
                    }
                }
            } else {
                let items = read_dir_children_for_guid(root, spec.root_guid);
                for item_guid in items {
                    self.index_leaf_module(
                        root,
                        spec.cat,
                        None,
                        &item_guid,
                        !spec.include_dirs,
                        &mut known_dir_guids,
                    );
                }
            }
        }
    }

    fn index_leaf_module(
        &mut self,
        root: &Path,
        cat: ModuleCategory,
        struct_id: Option<i32>,
        guid: &str,
        skip_dirs: bool,
        known_dir_guids: &mut std::collections::HashSet<String>,
    ) {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == "{00000000-0000-0000-0000-000000000000}" {
            return;
        }
        if skip_dirs && known_dir_guids.contains(&norm) {
            return;
        }
        if !self.modules_by_guid.contains_key(&norm) {
            match load_stoll_module_metadata_checked(root, &norm, skip_dirs) {
                ModuleLoadOutcome::Loaded(mut info) => {
                    info.category = cat.label_tr().to_string();
                    self.modules_by_guid.insert(norm.clone(), info);
                }
                ModuleLoadOutcome::Directory => {
                    known_dir_guids.insert(norm);
                    return;
                }
                ModuleLoadOutcome::Missing => return,
            }
        }
        self.register_category_guid(cat, struct_id, &norm);
    }

    /// Resolve a module GUID to a human-readable name & thumbnail, checking built-in cache first and then
    /// querying `<StollData>\Database\Module\new\<dir1>\<dir2>\{<guid>}.M1_Module` on demand.
    /// Prefers crisp `128x128` active `.M1_Module` diagrams over `24x24` `.M1_Module_Deleted` icons (`[ENG-01]`).
    pub fn resolve_module(&mut self, guid: &str) -> Option<&KnittingModuleInfo> {
        let key = normalize_guid(guid);
        if key == "{00000000-0000-0000-0000-000000000000}" || key.is_empty() {
            return None;
        }
        let needs_db_lookup = match self.modules_by_guid.get(&key) {
            None => true,
            Some(existing) => existing.thumbnail.is_none() && self.stoll_data_root.is_some(),
        };
        if needs_db_lookup {
            if let Some(ref root) = self.stoll_data_root.clone() {
                if let Some((mut info, was_deleted)) = load_stoll_module_from_db_detailed(root, &key)
                {
                    // If a module only has a small 24x24 folder/deleted icon, upgrade its preview thumbnail
                    // from a matching active 128x128 diagram while preserving the original GUID!
                    let small_thumb = info
                        .thumbnail
                        .as_ref()
                        .map(|t| t.width < 64 || t.height < 64)
                        .unwrap_or(true);
                    if small_thumb {
                        if let Some(upgraded) =
                            self.find_active_diagram_for_module(root, &key, &info.name, was_deleted)
                        {
                            info.thumbnail = Some(upgraded);
                        }
                    }
                    // Preserve existing category tag if already indexed
                    if let Some(existing) = self.modules_by_guid.get(&key) {
                        if existing.category != "Stoll DB" {
                            info.category = existing.category.clone();
                        }
                    }
                    self.modules_by_guid.insert(key.clone(), info);
                }
            }
        }
        self.modules_by_guid.get(&key)
    }

    fn find_active_diagram_for_module(
        &self,
        root: &Path,
        guid: &str,
        name: &str,
        _was_deleted: bool,
    ) -> Option<PreviewImage> {
        let norm_name = name.trim().to_ascii_lowercase();
        let mut candidate_guids: Vec<String> = Vec::new();
        // Known active 128x128 Widening "Standard (Structure single jersey)" diagram
        if guid == "{A0EB0C74-54C4-4295-9B98-63BC5E75BECF}"
            || guid == "{AAB4DE9D-30AF-4E62-8E19-301760376ADF}"
            || norm_name == "standard (structure single jersey)"
            || norm_name.contains("arttırma")
            || norm_name.contains("artırma")
        {
            candidate_guids.push("{42D1A932-627A-42D1-9B24-4199959897D1}".to_string());
        }
        for (g, m) in &self.modules_by_guid {
            if g == guid {
                continue;
            }
            let mn = m.name.trim().to_ascii_lowercase();
            if mn == norm_name || mn.starts_with(&norm_name) || norm_name.starts_with(&mn) {
                candidate_guids.push(g.clone());
            }
        }
        for cg in candidate_guids {
            if let Some((info, was_del)) = load_stoll_module_from_db_detailed(root, &cg) {
                if !was_del {
                    if let Some(thumb) = info.thumbnail {
                        if thumb.width >= 64 && thumb.height >= 64 {
                            return Some(thumb);
                        }
                    }
                }
            }
        }
        None
    }

    /// Immutable display name lookup (uses cached/built-in map; call `resolve_module` to populate from disk).
    pub fn module_name(&self, guid: &str) -> String {
        let key = normalize_guid(guid);
        if key == "{00000000-0000-0000-0000-000000000000}" || key.is_empty() {
            return "(Modül yok)".to_string();
        }
        if let Some(info) = self.modules_by_guid.get(&key) {
            return info.name.clone();
        }
        if let Some(ref root) = self.stoll_data_root {
            if let Some(info) = load_stoll_module_metadata_only(root, &key) {
                return info.name;
            }
        }
        guid.to_string()
    }
}

fn load_presets_from_shp(path: &Path, is_user_custom: bool, out: &mut Vec<StandardPreset>) {
    if let Ok(doc) = load_file(path) {
        if let Some(el) = doc.elements.first() {
            for line in &el.left_lines {
                let label = if line.comment.is_empty() {
                    format!(
                        "{} : {}",
                        line.index,
                        if is_user_custom { "Özel" } else { "Standart" }
                    )
                } else {
                    format!("{} : {}", line.index, line.comment)
                };
                out.push(StandardPreset {
                    id: line.index,
                    display_name: label,
                    is_user_custom,
                    template_line: line.clone(),
                });
            }
        }
    }
}

fn normalize_guid(guid: &str) -> String {
    let trimmed = guid.trim().to_ascii_uppercase();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed
    } else {
        format!("{{{trimmed}}}")
    }
}

/// Compute `<StollData>\Database\Module\new\<dir1>\<dir2>\{<guid>}.M1_Module` path using exact Stoll hash rule:
/// Given `{D1D2D3D4D5D6D7D8-...}`: `dir1 = D8`, `dir2 = D7`.
fn module_db_path(stoll_root: &Path, norm_guid: &str) -> Option<PathBuf> {
    let hex: Vec<char> = norm_guid.trim_matches(|c| c == '{' || c == '}').chars().collect();
    if hex.len() < 8 {
        return None;
    }
    let dir1 = hex[7].to_ascii_uppercase().to_string();
    let dir2 = hex[6].to_ascii_uppercase().to_string();
    let folder = stoll_root
        .join("Database")
        .join("Module")
        .join("new")
        .join(&dir1)
        .join(&dir2);

    // Direct lookup first (Windows NTFS is case-insensitive), falling back to `.M1_Module_Deleted`
    let lower_guid = norm_guid.to_ascii_lowercase();
    for filename in [
        format!("{norm_guid}.M1_Module"),
        format!("{lower_guid}.M1_Module"),
        format!("{norm_guid}.M1_Module_Deleted"),
        format!("{lower_guid}.M1_Module_Deleted"),
    ] {
        let candidate = folder.join(filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Load module name (`M3_MOD_Attribs`) and optional 128x128 thumbnail (`M3_UTL_Bitmap`) from Stoll Module DB.
/// Follows `M3_MOD_VersionList` indirection automatically when present, and upgrades `< 64x64`
/// folder/deleted icons on known Widening Standard GUIDs to the active `128x128` diagram (`{42D1A932-...}`).
pub fn load_stoll_module_from_db(stoll_root: &Path, guid: &str) -> Option<KnittingModuleInfo> {
    let norm = normalize_guid(guid);
    let (mut info, _was_deleted) = load_stoll_module_from_db_detailed(stoll_root, &norm)?;
    let small_thumb = info
        .thumbnail
        .as_ref()
        .map(|t| t.width < 64 || t.height < 64)
        .unwrap_or(true);
    if small_thumb {
        let norm_name = info.name.trim().to_ascii_lowercase();
        if norm == "{A0EB0C74-54C4-4295-9B98-63BC5E75BECF}"
            || norm == "{AAB4DE9D-30AF-4E62-8E19-301760376ADF}"
            || norm_name == "standard (structure single jersey)"
            || norm_name.contains("arttırma")
            || norm_name.contains("artırma")
        {
            if let Some((active, false)) = load_stoll_module_from_db_detailed(
                stoll_root,
                "{42D1A932-627A-42D1-9B24-4199959897D1}",
            ) {
                if let Some(thumb) = active.thumbnail {
                    if thumb.width >= 64 && thumb.height >= 64 {
                        info.thumbnail = Some(thumb);
                    }
                }
            }
        }
    }
    Some(info)
}

enum ModuleLoadOutcome {
    Loaded(KnittingModuleInfo),
    Directory,
    Missing,
}

fn load_stoll_module_from_db_detailed(
    stoll_root: &Path,
    guid: &str,
) -> Option<(KnittingModuleInfo, bool)> {
    let norm = normalize_guid(guid);
    let path = module_db_path(stoll_root, &norm)?;
    let is_deleted = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("M1_Module_Deleted"))
        .unwrap_or(false);
    match load_module_file_inner(stoll_root, &norm, &path, 0, true, false) {
        ModuleLoadOutcome::Loaded(info) => Some((info, is_deleted)),
        _ => None,
    }
}

fn load_stoll_module_metadata_only(stoll_root: &Path, guid: &str) -> Option<KnittingModuleInfo> {
    match load_stoll_module_metadata_checked(stoll_root, guid, false) {
        ModuleLoadOutcome::Loaded(info) => Some(info),
        _ => None,
    }
}

fn load_stoll_module_metadata_checked(
    stoll_root: &Path,
    guid: &str,
    skip_dirs: bool,
) -> ModuleLoadOutcome {
    let norm = normalize_guid(guid);
    let Some(path) = module_db_path(stoll_root, &norm) else {
        return ModuleLoadOutcome::Missing;
    };
    load_module_file_inner(stoll_root, &norm, &path, 0, false, skip_dirs)
}

/// Read `M3_MOD_DirChildren` GUID list from a Stoll DB directory node (48-byte header + N*36-byte entries).
fn read_dir_children_for_guid(stoll_root: &Path, guid: &str) -> Vec<String> {
    let norm = normalize_guid(guid);
    let Some(path) = module_db_path(stoll_root, &norm) else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let Ok(mut comp) = cfb::CompoundFile::open(std::io::Cursor::new(bytes)) else {
        return Vec::new();
    };
    if !comp.exists("/M3_MOD_DirChildren") {
        return Vec::new();
    }
    let mut data = Vec::new();
    if comp
        .open_stream("/M3_MOD_DirChildren")
        .and_then(|mut s| s.read_to_end(&mut data))
        .is_err()
        || data.len() < 48
    {
        return Vec::new();
    }
    let count = u32::from_le_bytes(data[44..48].try_into().unwrap_or([0; 4])) as usize;
    let mut out = Vec::with_capacity(count.min(256));
    for i in 0..count {
        let off = 48 + i * 36;
        if off + 36 > data.len() {
            break;
        }
        if let Some(g) = decode_win_guid(&data[off + 16..off + 32]) {
            out.push(g);
        }
    }
    out
}

fn load_module_file_inner(
    stoll_root: &Path,
    orig_guid: &str,
    path: &Path,
    depth: u8,
    load_bitmap: bool,
    skip_dirs: bool,
) -> ModuleLoadOutcome {
    if depth > 3 {
        return ModuleLoadOutcome::Missing;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return ModuleLoadOutcome::Missing;
    };
    let Ok(mut comp) = cfb::CompoundFile::open(std::io::Cursor::new(bytes)) else {
        return ModuleLoadOutcome::Missing;
    };

    if skip_dirs && comp.exists("/M3_MOD_DirChildren") {
        return ModuleLoadOutcome::Directory;
    }

    // Check for M3_MOD_Attribs directly
    if comp.exists("/M3_MOD_Attribs") {
        let mut attribs = Vec::new();
        {
            let Ok(mut stream) = comp.open_stream("/M3_MOD_Attribs") else {
                return ModuleLoadOutcome::Missing;
            };
            if stream.read_to_end(&mut attribs).is_err() {
                return ModuleLoadOutcome::Missing;
            }
        }
        let Some(name) = parse_m3_mod_attribs_name(&attribs) else {
            return ModuleLoadOutcome::Missing;
        };
        let thumbnail = if load_bitmap && comp.exists("/M3_UTL_Bitmap") {
            let mut bmp = Vec::new();
            if comp
                .open_stream("/M3_UTL_Bitmap")
                .and_then(|mut s| s.read_to_end(&mut bmp))
                .is_ok()
            {
                decode_m3_utl_bitmap(&bmp)
            } else {
                None
            }
        } else {
            None
        };
        return ModuleLoadOutcome::Loaded(KnittingModuleInfo {
            guid: orig_guid.to_string(),
            name,
            category: "Stoll DB".to_string(),
            thumbnail,
        });
    }

    // Follow M3_MOD_VersionList pointer -> last version entry
    if comp.exists("/M3_MOD_VersionList") {
        let mut vlist = Vec::new();
        {
            let Ok(mut stream) = comp.open_stream("/M3_MOD_VersionList") else {
                return ModuleLoadOutcome::Missing;
            };
            if stream.read_to_end(&mut vlist).is_err() {
                return ModuleLoadOutcome::Missing;
            }
        }
        if vlist.len() >= 44 {
            let Ok(cnt_bytes) = vlist[40..44].try_into() else {
                return ModuleLoadOutcome::Missing;
            };
            let count = u32::from_le_bytes(cnt_bytes) as usize;
            if count > 0 {
                let entry_off = 44 + (count - 1) * 36;
                if vlist.len() >= entry_off + 36 {
                    let Some(target_guid) = decode_win_guid(&vlist[entry_off + 16..entry_off + 32]) else {
                        return ModuleLoadOutcome::Missing;
                    };
                    let Some(target_path) = module_db_path(stoll_root, &target_guid) else {
                        return ModuleLoadOutcome::Missing;
                    };
                    return load_module_file_inner(
                        stoll_root,
                        orig_guid,
                        &target_path,
                        depth + 1,
                        load_bitmap,
                        skip_dirs,
                    );
                }
            }
        }
    }

    ModuleLoadOutcome::Missing
}

fn decode_win_guid(b: &[u8]) -> Option<String> {
    if b.len() < 16 {
        return None;
    }
    let d1 = u32::from_le_bytes(b[0..4].try_into().ok()?);
    let d2 = u16::from_le_bytes(b[4..6].try_into().ok()?);
    let d3 = u16::from_le_bytes(b[6..8].try_into().ok()?);
    Some(format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        d1, d2, d3, b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    ))
}

fn parse_m3_mod_attribs_name(data: &[u8]) -> Option<String> {
    if data.len() < 20 {
        return None;
    }
    let name_len = u32::from_le_bytes(data[16..20].try_into().ok()?) as usize;
    let end = 20usize.checked_add(name_len.checked_mul(2)?)?;
    if end > data.len() {
        return None;
    }
    let short_name = read_utf16le_slice(&data[20..end])?;
    let mut best_tr: Option<String> = None;
    let mut best_en: Option<String> = None;

    let mut pos = end;
    while pos + 8 <= data.len() {
        let lang_id = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?);
        let slen = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        let next = match pos.checked_add(slen.saturating_mul(2)) {
            Some(n) if n <= data.len() && slen < 512 => n,
            _ => break,
        };
        if let Some(s) = read_utf16le_slice(&data[pos..next]) {
            if !s.is_empty() {
                if lang_id == 0x001F {
                    best_tr = Some(s);
                } else if lang_id == 0x0009 {
                    best_en = Some(s);
                }
            }
        }
        pos = next;
    }

    Some(
        best_tr
            .or(best_en)
            .unwrap_or(short_name)
            .trim()
            .to_string(),
    )
}

fn read_utf16le_slice(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}

/// Decode `M3_UTL_Bitmap` in either Stoll format into **canonical top-down RGBA8** (`[ENG-01]` / #48):
/// 1. 8-bit indexed bottom-up DIB (`[0xFFFF, 1, W, H]` + `W*H` indices + `256*4` BGRX palette, e.g. 128x128)
/// 2. 32-bit bottom-up BGRX DIB (`[W, H]` + `W*H*4` BGRX pixels, e.g. 24x24)
pub fn decode_m3_utl_bitmap(data: &[u8]) -> Option<PreviewImage> {
    if data.len() < 8 {
        return None;
    }
    let first_u32 = u32::from_le_bytes(data[0..4].try_into().ok()?);
    if first_u32 == 0xFFFF && data.len() >= 16 {
        let width = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
        let height = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
        let pix_len = width.checked_mul(height)?;
        let pal_off = 16usize.checked_add(pix_len)?;
        if data.len() < pal_off + 256 * 4 || width == 0 || height == 0 || width > 512 || height > 512
        {
            return None;
        }
        let pixels = &data[16..pal_off];
        let palette = &data[pal_off..pal_off + 1024];
        let mut rgba = Vec::with_capacity(pix_len * 4);
        // Flip bottom-up DIB scanlines to canonical top-down RGBA8 (matching decode_muster_bitmap)
        for dst_y in 0..height {
            let src_y = height - 1 - dst_y;
            let row = &pixels[src_y * width..(src_y + 1) * width];
            for &idx in row {
                let pi = (idx as usize) * 4;
                rgba.push(palette[pi + 2]); // R
                rgba.push(palette[pi + 1]); // G
                rgba.push(palette[pi]);     // B
                rgba.push(255);             // A
            }
        }
        return Some(PreviewImage {
            width: width as u32,
            height: height as u32,
            rgba,
        });
    }

    // Format 2: `[u32 W][u32 H][W*H*4 bottom-up BGRX]` -> decoded top-down RGBA8
    crate::io::bitmap::decode_muster_bitmap(data)
}

fn builtin_stoll_presets() -> Vec<StandardPreset> {
    let specs: &[(i32, &str, i32, i32)] = &[
        (1, "1 : Basis", 0, 999),
        (2, "2 : CMS  >6< / <1>", 6, 4),
        (3, "3 : CMS  >6< / <6>", 6, 6),
        (4, "4 : CMS TC4 >6< / <0>", 6, 4),
        (5, "5 : CMS 1x1 >4< / <0>", 4, 4),
        (6, "6 : CMSTC-R >6<", 6, 4),
        (7, "7 : CMSTC-R  V", 4, 4),
        (8, "8 : Fair Isle U", 6, 4),
        (9, "9 : Fair Isle ^", 6, 4),
    ];

    specs
        .iter()
        .map(|&(id, label, nar_w, bo_lim)| {
            let mut line = LineSegment {
                index: id,
                tag_id: id,
                comment: label
                    .split_once(':')
                    .map(|x| x.1.trim())
                    .unwrap_or(label)
                    .to_string(),
                ..LineSegment::default()
            };
            line.fade_out = Some(FadeOutProps {
                modules: vec![ModuleAssignment {
                    module_art_id: 0,
                    id: "{8BA020E3-AFBA-46F4-99A5-9DD47B54703E}".to_string(),
                    width: 1,
                    ..ModuleAssignment::default()
                }],
                ..FadeOutProps::default()
            });
            line.narrowing = Some(NarrowingProps {
                modules: vec![ModuleAssignment {
                    module_art_id: 0,
                    id: "{D52E8466-70AD-11D3-BE0D-00A0C9ECEF97}".to_string(),
                    width: nar_w,
                    bind_off_limit: bo_lim,
                    narrowing_position: 4,
                    ..ModuleAssignment::default()
                }],
                ..NarrowingProps::default()
            });
            line.widening = Some(WideningProps {
                modules: vec![ModuleAssignment {
                    module_art_id: 0,
                    id: "{AAB4DE9D-30AF-4E62-8E19-301760376ADF}".to_string(),
                    width: 1,
                    bind_off_limit: 999,
                    narrowing_position: 4,
                    ..ModuleAssignment::default()
                }],
                ..WideningProps::default()
            });
            line.binding_off = Some(BindingOffProps {
                modules: vec![ModuleAssignment {
                    module_art_id: 0,
                    id: "{A372A90F-50DD-11D4-BE63-00A0C9ECEF97}".to_string(),
                    width: 1,
                    bind_off_limit: bo_lim,
                    narrowing_position: 4,
                    ..ModuleAssignment::default()
                }],
                ..BindingOffProps::default()
            });
            StandardPreset {
                id,
                display_name: label.to_string(),
                is_user_custom: false,
                template_line: line,
            }
        })
        .collect()
}

fn builtin_module_entries() -> &'static [(&'static str, &'static str, ModuleCategory, Option<i32>)] {
    &[
        ("{D52E8466-70AD-11D3-BE0D-00A0C9ECEF97}", "L-R separate transfer (Structure single jersey)", ModuleCategory::Narrowing, Some(0)),
        ("{70037774-DA09-11D3-BE14-00A0C9D4B941}", "L-R combined transfer (Structure single jersey)", ModuleCategory::Narrowing, Some(0)),
        ("{D52E8467-70AD-11D3-BE0D-00A0C9ECEF97}", "L-R separate transfer (Structure full needle)", ModuleCategory::Narrowing, Some(1)),
        ("{8BA020E3-AFBA-46F4-99A5-9DD47B54703E}", "Front stitch with transfer", ModuleCategory::FadeOut, Some(0)),
        ("{31BB46CF-D6F2-11D2-AE4D-0000F8081442}", "MG_Masche v mit Umhängen", ModuleCategory::FadeOut, Some(6)),
        ("{AAB4DE9D-30AF-4E62-8E19-301760376ADF}", "Standard (Structure single jersey)", ModuleCategory::Widening, Some(0)),
        ("{A0EB0C74-54C4-4295-9B98-63BC5E75BECF}", "Standard (Structure single jersey)", ModuleCategory::Widening, Some(0)),
        ("{A372A90F-50DD-11D4-BE63-00A0C9ECEF97}", "Bind-off SJ-01", ModuleCategory::BindOff, Some(0)),
        ("{0A4C87EC-24EB-4C71-90AC-F3FB9793E0A4}", "Bind-Off RL with fixing 01", ModuleCategory::BindOffCa, Some(0)),
        ("{524134AE-CA09-4F92-8B75-1A33DF7A06BA}", "1-ISARET", ModuleCategory::Start, Some(0)),
        ("{CDF170A8-D448-458E-BB77-2DF1B3C29586}", "2-ISARET", ModuleCategory::Start, Some(0)),
        ("{BC07B9DE-16B6-405E-9ADA-07ED9F49F55E}", "3-ISARET", ModuleCategory::Start, Some(0)),
        ("{8ACE0A1D-A137-4781-937A-CFFE8153E445}", "4-ISARET", ModuleCategory::Start, Some(0)),
        ("{14F1754D-92EE-4EA7-B0CF-7411E1014AE8}", "5-ISARET", ModuleCategory::Start, Some(0)),
        ("{926B162D-9ACB-4B61-A46C-E6BE3F0F6A2B}", "6-ISARET", ModuleCategory::Start, Some(0)),
        ("{11F9019C-CB57-4A3F-B703-5CB6FFAE944E}", "7-ISARET", ModuleCategory::Start, Some(0)),
        ("{91B00030-8FAB-4A98-9B49-6B78208F324A}", "8-ISARET", ModuleCategory::Start, Some(0)),
        ("{1029632E-AA11-4AE3-B41A-ECADCFDB2349}", "9-ISARET", ModuleCategory::Start, Some(0)),
    ]
}

fn builtin_modules_map() -> HashMap<String, KnittingModuleInfo> {
    let mut map = HashMap::new();
    for &(guid, name, cat, _struct_id) in builtin_module_entries() {
        map.insert(
            guid.to_ascii_uppercase(),
            KnittingModuleInfo {
                guid: guid.to_string(),
                name: name.to_string(),
                category: cat.label_tr().to_string(),
                thumbnail: None,
            },
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deleted_module_fallback_and_bgrx_thumbnail() {
        let mut cat = StollCatalog::load_with_autodetect();
        if cat.stoll_data_root.is_some() {
            let info = cat
                .resolve_module("{A0EB0C74-54C4-4295-9B98-63BC5E75BECF}")
                .expect("should resolve .M1_Module_Deleted");
            assert!(!info.name.is_empty() && !info.name.starts_with('{'));
            let thumb = info
                .thumbnail
                .as_ref()
                .expect("should upgrade deleted 24x24 icon to active 128x128 stitch diagram");
            assert_eq!(thumb.width, 128);
            assert_eq!(thumb.height, 128);
            assert_eq!(thumb.rgba.len(), 128 * 128 * 4);
            // Also verify direct load_stoll_module_from_db() path used by LinePropsDialog
            let direct = load_stoll_module_from_db(
                cat.stoll_data_root.as_deref().unwrap(),
                "{A0EB0C74-54C4-4295-9B98-63BC5E75BECF}",
            )
            .expect("direct load_stoll_module_from_db");
            let dthumb = direct.thumbnail.as_ref().expect("direct 128x128 thumbnail");
            assert_eq!(dthumb.width, 128);
            assert_eq!(dthumb.height, 128);
        }
    }

    #[test]
    fn test_stoll_structure_isolation_and_exact_db_order() {
        let cat = StollCatalog::load_with_autodetect();
        let Some(root) = cat.stoll_data_root.as_deref() else {
            // Offline CI fallback: verify no cross-structure leakage even with builtin entries
            let s0 = cat.modules_for_category_and_structure(ModuleCategory::Narrowing, 0);
            assert_eq!(s0.len(), 2);
            assert!(cat.modules_for_category_and_structure(ModuleCategory::Widening, 2).is_empty());
            return;
        };

        // 1. Reglan (Mindern) for Strüktür tekplaka (0): exact 30 modules in M3_MOD_DirChildren order,
        // zero Full-needle / Jac-* / 1x1 / Narrowing-multistage modules leaked.
        let nar0 = cat.modules_for_category_and_structure(ModuleCategory::Narrowing, 0);
        assert_eq!(nar0.len(), 30, "Mindern[0] must contain exactly 30 single-jersey modules");
        assert_eq!(nar0[0].guid.to_ascii_uppercase(), "{D52E8466-70AD-11D3-BE0D-00A0C9ECEF97}");
        assert_eq!(nar0[1].guid.to_ascii_uppercase(), "{70037774-DA09-11D3-BE14-00A0C9D4B941}");
        for m in &nar0 {
            let low = m.name.to_ascii_lowercase();
            assert!(!low.contains("full needle"), "Full needle leaked into slot 0: {}", m.name);
            assert!(!low.contains("jac"), "Jacquard leaked into slot 0: {}", m.name);
            assert!(!low.contains("mg_"), "1x1-MG leaked into slot 0: {}", m.name);
            assert!(!low.contains("fair isle"), "Multistage leaked into Mindern[0]: {}", m.name);
        }
        // Check exact 1:1 GUID order against Stoll DB directory node for Mindern slot 0
        let mindern_slots = read_dir_children_for_guid(root, "{4D55434B-0000-0000-000D-4B41524C4900}");
        let expected_nar0 = read_dir_children_for_guid(root, &mindern_slots[0]);
        let actual_nar0_guids: Vec<String> = nar0.iter().map(|m| m.guid.to_ascii_uppercase()).collect();
        assert_eq!(actual_nar0_guids, expected_nar0, "Reglan slot 0 must match exact Stoll M3_MOD_DirChildren order");
        // Empty Mindern slot 7 (1x1-MG Strüktür çiftplaka) must stay empty, never falling back!
        assert!(cat.modules_for_category_and_structure(ModuleCategory::Narrowing, 7).is_empty());

        // 2. Çok kademeli reglan (NarrowingMultistage): flat list of 6 modules
        let multi = cat.modules_for_category_and_structure(ModuleCategory::NarrowingMultistage, 0);
        assert_eq!(multi.len(), 6);
        assert_eq!(
            cat.modules_for_category_and_structure(ModuleCategory::NarrowingMultistage, 5).len(),
            6
        );

        // 3. Artırma (Zunahme): slot 0 = 16 items starting with Standard in exact Stoll order; slot 2 (Jac-Dolu) = empty!
        let wid0 = cat.modules_for_category_and_structure(ModuleCategory::Widening, 0);
        assert_eq!(wid0.len(), 16);
        assert!(wid0[0].name.starts_with("Standard"));
        let wid_slots = read_dir_children_for_guid(root, "{4D55434B-0000-0000-000F-4B41524C4900}");
        let expected_wid0 = read_dir_children_for_guid(root, &wid_slots[0]);
        let actual_wid0_guids: Vec<String> = wid0.iter().map(|m| m.guid.to_ascii_uppercase()).collect();
        assert_eq!(actual_wid0_guids, expected_wid0, "Widening slot 0 must match exact Stoll M3_MOD_DirChildren order");
        assert!(cat.modules_for_category_and_structure(ModuleCategory::Widening, 2).is_empty());

        // 4. Remayözleme split 1:1 into Modül (`Abketteln`) and RA (`Bind-off-CA`):
        //    - Modül slot 0 = 16 items starting with Bind-off SJ-01
        //    - RA slot 0 = 61 items including CA modules; slot 8 (1x1-MG Jac-Dolu) = empty!
        let bo_mod0 = cat.modules_for_category_and_structure(ModuleCategory::BindOff, 0);
        assert_eq!(bo_mod0.len(), 16);
        assert_eq!(bo_mod0[0].name, "Bind-off SJ-01");
        let bo_ab_slots = read_dir_children_for_guid(root, "{4D55434B-0000-0000-000E-4B41524C4900}");
        let expected_bo_mod0 = read_dir_children_for_guid(root, &bo_ab_slots[0]);
        let actual_bo_mod0: Vec<String> = bo_mod0.iter().map(|m| m.guid.to_ascii_uppercase()).collect();
        assert_eq!(actual_bo_mod0, expected_bo_mod0, "BindOff (Modül) slot 0 must match Abketteln[0]");
        assert_eq!(cat.bind_off_type_for_guid(&bo_mod0[0].guid), 0);

        let bo_ca0 = cat.modules_for_category_and_structure(ModuleCategory::BindOffCa, 0);
        assert_eq!(bo_ca0.len(), 61);
        assert!(bo_ca0[0].name.contains("_CA"), "Bind-off-CA[0] first entry: {}", bo_ca0[0].name);
        assert!(bo_ca0.iter().any(|m| m.name.contains("4no Kolevi Bind-off 1-igne_CA")));
        let bo_ca_slots = read_dir_children_for_guid(root, "{F39139E7-520E-4104-8BBA-A4D8A034FA80}");
        let expected_bo_ca0 = read_dir_children_for_guid(root, &bo_ca_slots[0]);
        let actual_bo_ca0: Vec<String> = bo_ca0.iter().map(|m| m.guid.to_ascii_uppercase()).collect();
        assert_eq!(actual_bo_ca0, expected_bo_ca0, "BindOffCa (RA) slot 0 must match Bind-off-CA[0]");
        assert_eq!(cat.bind_off_type_for_guid(&bo_ca0[0].guid), 1);
        for m in bo_mod0.iter().chain(bo_ca0.iter()) {
            let low = m.name.to_ascii_lowercase();
            assert!(!low.contains("mg_"), "1x1-MG leaked into BindOff[0]: {}", m.name);
        }
        assert!(cat.modules_for_category_and_structure(ModuleCategory::BindOff, 8).is_empty());
        assert!(cat.modules_for_category_and_structure(ModuleCategory::BindOffCa, 8).is_empty());

        // 5. Kenar düzeltme (Ausblenden): slot 0 = 39 leaf items starting with Front stitch with transfer
        //    ('Yeni modül grubu' organizational folder {363924DE-...} excluded); slot 2 (Jac-Dolu) = empty!
        let fo0 = cat.modules_for_category_and_structure(ModuleCategory::FadeOut, 0);
        assert_eq!(fo0.len(), 39);
        assert_eq!(fo0[0].name, "Front stitch with transfer");
        assert!(fo0.iter().all(|m| !m.name.eq_ignore_ascii_case("Yeni modül grubu") && !m.name.eq_ignore_ascii_case("Neue Modulgruppe")));
        assert!(cat.modules_for_category_and_structure(ModuleCategory::FadeOut, 2).is_empty());

        // 6. Başlama / Bitiş / İşaretleme cleanly separated and `element`/`ELEMENT` directory nodes excluded
        let start0 = cat.modules_for_category_and_structure(ModuleCategory::Start, 0);
        assert_eq!(start0.len(), 10);
        assert_eq!(start0[0].name, "1-ISARET");
        assert!(start0.iter().all(|m| !m.name.eq_ignore_ascii_case("element")));
        assert!(cat.modules_for_category_and_structure(ModuleCategory::Start, 1).is_empty());

        let end0 = cat.modules_for_category_and_structure(ModuleCategory::End, 0);
        assert_eq!(end0.len(), 2);
        assert_eq!(end0[0].name, "ORTA- ISARET");
        assert_eq!(end0[1].name, "ORTA- ISARET_4MAY");
        assert!(cat.modules_for_category_and_structure(ModuleCategory::End, 1).is_empty());

        let vneck0 = cat.modules_for_category_and_structure(ModuleCategory::StartVneck, 0);
        assert_eq!(vneck0.len(), 2);
        assert!(vneck0.iter().all(|m| !m.name.eq_ignore_ascii_case("Elements")));

        let mark = cat.modules_for_category_and_structure(ModuleCategory::Point, 0);
        assert_eq!(mark.len(), 2);
    }
}

