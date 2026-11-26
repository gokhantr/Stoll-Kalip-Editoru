use serde::{Deserialize, Serialize};
use crate::model::enums::PointKind;

/// Lossless key-value store preserving original key order and raw formatting for any section.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RawSectionMeta {
    /// Exact header text as read (e.g. `[Kante] = 1` or `[Mindern]`), if parsed from file.
    pub original_header: Option<String>,
    /// Ordered list of `(key, raw_value)` pairs as read from disk.
    /// Known keys are updated from typed fields upon serialization; unknown keys are preserved verbatim.
    pub raw_entries: Vec<(String, String)>,
}

impl RawSectionMeta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_raw(&mut self, key: String, value: String) {
        self.raw_entries.push((key, value));
    }

    pub fn get_raw(&self, key: &str) -> Option<&str> {
        self.raw_entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Set or insert a raw `(key, value)` entry, preserving position if already present.
    pub fn set_raw(&mut self, key: &str, value: String) {
        if let Some((_, existing)) = self.raw_entries.iter_mut().find(|(k, _)| k == key) {
            *existing = value;
        } else {
            self.raw_entries.push((key.to_string(), value));
        }
    }

    /// Remove a raw `(key, value)` entry if present.
    pub fn remove_raw(&mut self, key: &str) {
        self.raw_entries.retain(|(k, _)| k != key);
    }

    /// Detect whether a boolean key originally used `ja`/`nein` vs `1`/`0`.
    pub fn uses_ja_nein(&self, key: &str, default_ja_nein: bool) -> bool {
        match self.get_raw(key) {
            Some(v) => {
                let vl = v.trim().to_ascii_lowercase();
                if vl == "ja" || vl == "nein" {
                    true
                } else if vl == "0" || vl == "1" {
                    false
                } else {
                    default_ja_nein
                }
            }
            None => default_ja_nein,
        }
    }
}

/// One `[ModuleArt] = <id>` entry inside `[Ausblenden]`, `[Mindern]`, `[Zunehmen]`, `[Abketteln]`, or `[Punkt]`.
/// `module_art_id` maps to Stoll's knitting structure index:
/// - `-1`: Tüm örgü türleri / Default
/// - `0`: Strüktür tekplaka (Structure single jersey)
/// - `1`: Strüktür dolu iğne (Structure full needle)
/// - `2`: Jac-Dolu (Jacquard full)
/// - `3`: Jac-Pike (Jacquard pique)
/// - `4`: Jac-File (Jacquard net/birdseye)
/// - `5`: Jac-Atlama (Jacquard float)
/// - `6`: 1x1-MG ...
/// - `7..12`: Other rib / interlock / fair-isle / knit&wear structures
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleAssignment {
    pub module_art_id: i32,
    /// `ID` — Module GUID (`{00000000-0000-0000-0000-000000000000}` when empty)
    pub id: String,
    /// `Breite` — Transfer/narrowing/fade-out width in stitches
    pub width: i32,
    /// `AbkettelGrenze` — Bind-off threshold (default 999)
    pub bind_off_limit: i32,
    /// `MinderPosition` — Narrowing position (default 4)
    pub narrowing_position: i32,
    /// `FairIsleStatus` — Fair-Isle status (0, 1, 2)
    pub fair_isle_status: i32,
    /// `FillGuid` — Fill module GUID
    pub fill_guid: String,
    /// `BorderOptGuid` — Border option module GUID
    pub border_opt_guid: String,
    pub border_width_left: i32,
    pub border_width_right: i32,
    pub border_width_up: i32,
    pub border_width_down: i32,
    pub raw: RawSectionMeta,
}

impl Default for ModuleAssignment {
    fn default() -> Self {
        Self {
            module_art_id: 0,
            id: "{00000000-0000-0000-0000-000000000000}".to_string(),
            width: 0,
            bind_off_limit: 999,
            narrowing_position: 4,
            fair_isle_status: 0,
            fill_guid: "{00000000-0000-0000-0000-000000000000}".to_string(),
            border_opt_guid: "{00000000-0000-0000-0000-000000000000}".to_string(),
            border_width_left: 0,
            border_width_right: 0,
            border_width_up: 0,
            border_width_down: 0,
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Ausblenden]` — Remayözleme / Fade-out properties (Tab 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FadeOutProps {
    /// `ModulFarbeVerwenden` (ja/nein)
    pub use_module_color: bool,
    /// `FuerAlleStrickarten` (bitmask or 2147483647 for all structures)
    pub for_all_structures: i64,
    /// `KeinAutomatAusblenden` (0/1) — No automatic fade-out
    pub no_auto_fade_out: bool,
    /// `OffsetX`
    pub offset_x: i32,
    /// `FillOffsetX`
    pub fill_offset_x: i32,
    /// `FillOffsetY`
    pub fill_offset_y: i32,
    pub modules: Vec<ModuleAssignment>,
    pub raw: RawSectionMeta,
}

impl Default for FadeOutProps {
    fn default() -> Self {
        Self {
            use_module_color: false,
            for_all_structures: 2_147_483_647,
            no_auto_fade_out: false,
            offset_x: 0,
            fill_offset_x: 0,
            fill_offset_y: 0,
            modules: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Mindern]` — Reglan / Narrowing properties (Tab 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NarrowingProps {
    /// `FuerAlleStrickarten`
    pub for_all_structures: i64,
    /// `SofortAusfuehren` (0/1 or ja/nein) — Execute immediately
    pub execute_immediately: bool,
    /// `AnzahlMaschen` — Multi-step narrowing stitch count (default 1)
    pub stitch_count: i32,
    /// `ModulFarbeVerwenden` (ja/nein)
    pub use_module_color: bool,
    pub modules: Vec<ModuleAssignment>,
    pub raw: RawSectionMeta,
}

impl Default for NarrowingProps {
    fn default() -> Self {
        Self {
            for_all_structures: 2_147_483_647,
            execute_immediately: false,
            stitch_count: 1,
            use_module_color: false,
            modules: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Zunehmen]` — Artırma / Widening properties (Tab 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WideningProps {
    /// `FuerAlleStrickarten`
    pub for_all_structures: i64,
    /// `SofortAusfuehren` (ja/nein or 0/1)
    pub execute_immediately: bool,
    /// `ModulFarbeVerwenden` (ja/nein)
    pub use_module_color: bool,
    pub modules: Vec<ModuleAssignment>,
    pub raw: RawSectionMeta,
}

impl Default for WideningProps {
    fn default() -> Self {
        Self {
            for_all_structures: 0,
            execute_immediately: false,
            use_module_color: false,
            modules: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Abketteln]` — Remayözleme / Binding-off properties (Tab 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingOffProps {
    /// `FuerAlleStrickarten`
    pub for_all_structures: i64,
    /// `ModulFarbeVerwenden` (ja/nein)
    pub use_module_color: bool,
    /// UI-only legacy hint (`AbkettelTyp` is never written to Stoll `.shp` files).
    pub bind_off_type: Option<i32>,
    pub modules: Vec<ModuleAssignment>,
    pub raw: RawSectionMeta,
}

impl Default for BindingOffProps {
    fn default() -> Self {
        Self {
            for_all_structures: 0,
            use_module_color: false,
            bind_off_type: None,
            modules: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Offset] = <idx>` child of `[Punkt]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointOffset {
    pub index: i32,
    pub x: i32,
    pub y: i32,
    pub raw: RawSectionMeta,
}

impl Default for PointOffset {
    fn default() -> Self {
        Self {
            index: 0,
            x: 0,
            y: 0,
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Punkt] = <idx>` — Covers Start (`Başlama`, Tab 6), End (`Bitiş`, Tab 7), and Marking points (`İşaretler`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointEntry {
    pub index: i32,
    pub punkt_tag_id: i32,
    pub punkt_art_tag_id: i32,
    pub gruppe_tag_id: i32,
    pub for_all_structures: i64,
    pub use_module_color: bool,
    pub mark_ke_alignment: Option<i32>,
    pub number_of_marks: Option<i32>,
    pub offsets: Vec<PointOffset>,
    pub modules: Vec<ModuleAssignment>,
    pub raw: RawSectionMeta,
}

impl PointEntry {
    pub fn kind(&self) -> PointKind {
        PointKind::from_id(self.punkt_art_tag_id)
    }
}

impl Default for PointEntry {
    fn default() -> Self {
        Self {
            index: 0,
            punkt_tag_id: 0,
            punkt_art_tag_id: PointKind::Normal.to_id(),
            gruppe_tag_id: 0,
            for_all_structures: 2_147_483_647,
            use_module_color: false,
            mark_ke_alignment: Some(-1),
            number_of_marks: Some(1),
            offsets: vec![PointOffset::default()],
            modules: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[KurvenLinie] = <idx>` — One step row inside a curve (`[Kurve]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveStep {
    pub index: i32,
    pub tag_id: i32,
    /// `GruppenFaktor`
    pub group_factor: i32,
    /// `GruppenTagId`
    pub group_tag_id: i32,
    /// `StufungBreite` (dx in stitches)
    pub step_width: i32,
    /// `StufungHoehe` (dy in rows)
    pub step_height: i32,
    /// `Faktor` (repeat count, MUST be >= 1)
    pub factor: i32,
    /// `IsHandKnittingSuppressFirstY`
    pub suppress_first_y: Option<i32>,
    /// `MindernBreite` (En --- narrowing module width)
    pub narrowing_width: i32,
    /// `ZunehmenBreite` (En +++ widening module width)
    pub widening_width: i32,
    /// `AusblendenBreite` (En \\\ fade-out width)
    pub fade_out_width: i32,
    pub raw: RawSectionMeta,
}

impl Default for CurveStep {
    fn default() -> Self {
        Self {
            index: 1,
            tag_id: 1,
            group_factor: 1,
            group_tag_id: 0,
            step_width: 0,
            step_height: 2,
            factor: 1,
            suppress_first_y: Some(0),
            narrowing_width: 0,
            widening_width: 0,
            fade_out_width: 0,
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Kurve]` — Curve container inside `[Kante]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveData {
    /// `Funktion` inside `[Kurve]` (typically 1=Narrowing, 2=Widening, 3=Curve)
    pub function: i32,
    pub steps: Vec<CurveStep>,
    pub raw: RawSectionMeta,
}

impl Default for CurveData {
    fn default() -> Self {
        Self {
            function: 3,
            steps: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[ConnectPoint]` — One connection point inside `[ConnectionPoints]` (Tab 8: Bağlantılar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectPoint {
    pub connect_id: i32,
    pub connect_type: Option<i32>,
    /// `Direction` (ja/nein)
    pub direction: bool,
    pub raw: RawSectionMeta,
}

impl Default for ConnectPoint {
    fn default() -> Self {
        Self {
            connect_id: 1,
            connect_type: None,
            direction: true,
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[ConnectionPoints]` container inside `[Kante]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConnectionPoints {
    pub points: Vec<ConnectPoint>,
    pub raw: RawSectionMeta,
}

/// Order marker for child sub-sections of `[Kante]` so round-trip serialization preserves exact order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KanteChildSlot {
    FadeOut,
    Narrowing,
    Widening,
    BindingOff,
    Point(usize),
    Curve,
    ConnectionPoints,
}

/// External local module reference in `[Form]` header (`GuidExterneLokaleModule` / `NameExterneLokaleModule`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalLocalModule {
    pub guid: String,
    pub name: String,
}
