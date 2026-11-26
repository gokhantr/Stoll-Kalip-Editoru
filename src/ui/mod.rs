pub mod cad_canvas;
pub mod curve_editor_dialog;
pub mod format_convert_dialog;
pub mod graphics_window;
pub mod grid_table;
pub mod icons;
pub mod left_panel;
pub mod line_props_dialog;
pub mod menu_toolbar;
pub mod point_multiply_dialog;
pub mod theme;
pub mod top_panel;

use stoll_kalip_editoru::catalog::UserSettings;
use stoll_kalip_editoru::model::{InputFormat, LineSegment, LineSide, PointEntry};

/// Active grid table tab (`Sol çizgiler`, `Sağ çizgiler`, `Sol işaretler`, `Sağ işaretler`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableMode {
    LeftLines,
    RightLines,
    LeftMarkers,
    RightMarkers,
}

impl TableMode {
    pub fn side(self) -> LineSide {
        match self {
            Self::LeftLines | Self::LeftMarkers => LineSide::Left,
            Self::RightLines | Self::RightMarkers => LineSide::Right,
        }
    }

    pub fn is_lines(self) -> bool {
        matches!(self, Self::LeftLines | Self::RightLines)
    }

    pub fn is_markers(self) -> bool {
        matches!(self, Self::LeftMarkers | Self::RightMarkers)
    }

    #[allow(dead_code)]
    pub fn heading_tr(self) -> &'static str {
        match self {
            Self::LeftLines => "Sol çizgiler",
            Self::RightLines => "Sağ çizgiler",
            Self::LeftMarkers => "Sol işaretler",
            Self::RightMarkers => "Sağ işaretler",
        }
    }
}

/// Sign convention (`Çizgiler-Giriş` menu & Giriş toolbar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignMode {
    /// `+: Sağa doğru, -: Sola doğru` (Cartesian default)
    Cartesian,
    /// `+: Arttır, -: Reglan` (Knitting widen/narrow sign mode)
    Knitting,
}

/// Which toolbars are visible (`Görünüm -> Sembol çubukları`).
#[derive(Debug, Clone, Copy)]
pub struct ToolbarVisibility {
    pub standard: bool,
    pub input: bool,
    pub elements: bool,
    pub lines: bool,
    pub markers: bool,
}

impl Default for ToolbarVisibility {
    fn default() -> Self {
        Self {
            standard: true,
            input: true,
            elements: true,
            lines: true,
            markers: true,
        }
    }
}

/// Request payload to open the Line Properties modal dialog (`M3_LinePropSheet` — 8 tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinePropsRequest {
    pub elem_idx: usize,
    pub side: LineSide,
    pub line_idx: usize,
    /// Initial active tab `0..=7`:
    /// 0=Genel, 1=Reglan, 2=Artırma, 3=Remayözleme, 4=Kenar düzeltme, 5=Başlama, 6=Bitiş, 7=Bağlantılar
    pub initial_tab: usize,
}

/// Request payload to open the Curve / Line Editor (`Çizgi editörü`, `DIALOG 4265`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveEditorRequest {
    pub elem_idx: usize,
    pub side: LineSide,
    pub line_idx: usize,
}

/// Request payload to open the Marker Multiply dialog (`İşaretleme noktalarını çoğalt`, `DIALOG 4277`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointMultiplyRequest {
    pub elem_idx: usize,
    pub side: LineSide,
    pub marker_idx: usize,
}

/// Request payload to open Format Conversion / Recalculate Save As dialog (`50505`, `50521`, `50522`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatConvertRequest {
    pub target_format: InputFormat,
    pub save_as_after: bool,
}

/// Pending file action awaiting dirty-save confirmation ("Kesim kaydedilsin mi?").
#[derive(Debug, Clone)]
pub enum PendingDirtyAction {
    NewFile,
    OpenShv,
    OpenShp,
    OpenPath(std::path::PathBuf),
    OpenPresetPath {
        path: std::path::PathBuf,
        is_user: bool,
    },
    Exit,
}

/// Shared UI state owned by `app.rs` and passed cleanly to panels, grid, and `ui-dialogs`.
#[derive(Debug, Clone)]
pub struct UiState {
    pub selected_elem: usize,
    pub all_elements_selected: bool,
    pub table_mode: TableMode,
    pub sign_mode: SignMode,

    /// Selected row indices (0-based) in the active table.
    pub selected_rows: Vec<usize>,
    /// Anchor row index for Shift+Click and mouse drag multi-row selection.
    pub row_select_anchor: Option<usize>,
    /// Active mouse click-drag multi-row selection `(side, anchor_row)`.
    pub row_drag_selecting: Option<(LineSide, usize)>,
    /// Focused `(row, col)` cell in the active grid.
    pub focused_cell: Option<(usize, usize)>,
    /// Currently editing cell `(row, col, text_buffer)`.
    pub editing_cell: Option<(usize, usize, String)>,
    /// True while editing cell buffer has not yet received user keystrokes (select-all / `"0"` replacement).
    pub editing_pristine: bool,
    /// Which table side owns `editing_cell` so cross-table clicks commit cleanly.
    pub editing_side: LineSide,

    /// Top panel `Yönergeler` step directives (Reglan, Artırma, Boy).
    pub directive_narrowing: i32,
    pub directive_widening: i32,
    pub directive_height: i32,

    /// Top panel `Standart özellikler` selection.
    pub preset_is_user: bool,
    pub selected_preset_idx: usize,

    /// View toggles (`Görünüm` menu & Modern Studio toolbar).
    pub always_on_top: bool,
    pub show_graphics: bool,
    pub hide_readonly_columns: bool,
    pub preset_name_as_comment: bool,
    pub show_status_bar: bool,
    pub toolbars: ToolbarVisibility,

    /// Modern Light Studio state.
    pub show_detail_columns: bool,
    pub snap_even_rows: bool,
    pub show_dimensions: bool,
    pub settings_popover_open: bool,
    pub active_table_side: LineSide,
    /// Inline grid unit toggle: `true` displays/edits Boy & En in cm directly.
    pub grid_unit_cm: bool,
    /// Collapse mirrored `Sağ çizgiler` / `Sağ işaretler` card to give `Sol` full height.
    pub right_panel_collapsed: bool,

    /// Clipboard for lines & markers (`Ctrl+C` / `Ctrl+V`).
    pub copied_lines: Vec<LineSegment>,
    pub copied_markers: Vec<PointEntry>,

    /// Dialog requests (consumed by `ui-dialogs` / `app.rs`).
    pub line_props_req: Option<LinePropsRequest>,
    pub curve_editor_req: Option<CurveEditorRequest>,
    pub point_multiply_req: Option<PointMultiplyRequest>,
    pub format_convert_req: Option<FormatConvertRequest>,
    pub about_open: bool,
    pub help_open: bool,
    pub neckline_props_open: bool,
    pub ref_line_select_open: bool,
    pub print_preview_open: bool,
    pub external_import_open: bool,
    pub external_import_text: String,
    pub pending_mirror_confirm: bool,
    pub pending_dirty_action: Option<PendingDirtyAction>,

    /// Transient status bar message `(text, is_error)`.
    pub status_message: Option<(String, bool)>,
    /// Recent files list (MRU).
    pub recent_files: Vec<std::path::PathBuf>,

    /// Persistent `%APPDATA%\kalipeditor\settings.ini` state & custom template name input.
    pub user_settings: UserSettings,
    pub preset_save_name: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_elem: 0,
            all_elements_selected: false,
            table_mode: TableMode::LeftLines,
            sign_mode: SignMode::Cartesian,
            selected_rows: vec![0],
            row_select_anchor: Some(0),
            row_drag_selecting: None,
            focused_cell: Some((0, 2)),
            editing_cell: None,
            editing_pristine: false,
            editing_side: LineSide::Left,
            directive_narrowing: 1,
            directive_widening: 1,
            directive_height: 1,
            preset_is_user: false,
            selected_preset_idx: 1, // "2 : CMS  >6< / <1>" default as in screenshot 222001.png
            always_on_top: false,
            show_graphics: false,
            hide_readonly_columns: false,
            preset_name_as_comment: true,
            show_status_bar: true,
            toolbars: ToolbarVisibility::default(),
            show_detail_columns: false,
            snap_even_rows: false,
            show_dimensions: true,
            settings_popover_open: false,
            active_table_side: LineSide::Left,
            grid_unit_cm: false,
            right_panel_collapsed: false,
            copied_lines: Vec::new(),
            copied_markers: Vec::new(),
            line_props_req: None,
            curve_editor_req: None,
            point_multiply_req: None,
            format_convert_req: None,
            about_open: false,
            help_open: false,
            neckline_props_open: false,
            ref_line_select_open: false,
            print_preview_open: false,
            external_import_open: false,
            external_import_text: String::new(),
            pending_mirror_confirm: false,
            pending_dirty_action: None,
            status_message: None,
            recent_files: Vec::new(),
            user_settings: UserSettings::default(),
            preset_save_name: String::new(),
        }
    }
}

impl UiState {
    pub fn primary_row(&self) -> Option<usize> {
        self.focused_cell
            .map(|(r, _)| r)
            .or_else(|| self.selected_rows.first().copied())
    }

    pub fn select_single_row(&mut self, row: usize) {
        self.selected_rows.clear();
        self.selected_rows.push(row);
        self.row_select_anchor = Some(row);
    }

    pub fn any_modal_open(&self) -> bool {
        self.line_props_req.is_some()
            || self.curve_editor_req.is_some()
            || self.point_multiply_req.is_some()
            || self.format_convert_req.is_some()
            || self.about_open
            || self.help_open
            || self.neckline_props_open
            || self.ref_line_select_open
            || self.print_preview_open
            || self.external_import_open
            || self.pending_mirror_confirm
            || self.pending_dirty_action.is_some()
            || self.settings_popover_open
    }

    pub fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status_message = Some((msg.into(), is_error));
    }
}

/// Actions emitted by Menus, Toolbars, Left Panel, Top Panel, and Grid to be executed by `app.rs`.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppAction {
    // File menu
    NewFile,
    OpenShv,
    OpenShp,
    OpenShpx,
    ImportDxf,
    OpenPath(std::path::PathBuf),
    OpenPresetFile { kind: &'static str },
    Save,
    SaveAs,
    RecalcSaveAs,
    ConvertFormat(InputFormat),
    Exit,

    // Undo / Redo
    Undo,
    Redo,

    // View menu
    ToggleAlwaysOnTop,
    ToggleGraphics,
    SwapLeftRight,

    // Elements toolbar
    AddElement,
    DeleteElement,
    SelectAllElements,
    OpenNecklineProps,

    // Lines toolbar & grid
    DeleteSelectedLines,
    ToggleGroupSelectedLines,
    CreateClosingLine,
    AppendLine,
    InsertLineBefore,
    QuickStartTemel,
    QuickStartRectangle,
    CopySelectedLines,
    PasteReplaceLines,
    PasteInsertLines,
    ImportExternalTable,

    // Markers toolbar & grid
    DeleteSelectedMarker,
    ToggleGroupSelectedMarkers,
    AppendMarker,
    InsertMarkerBefore,
    OpenMultiplyMarkers,
    CopySelectedMarkers,
    PasteReplaceMarkers,
    PasteInsertMarkers,

    // Dialog triggers
    OpenLineProps {
        line_idx: usize,
        initial_tab: usize,
    },
    OpenCurveEditor {
        line_idx: usize,
    },

    // Top panel actions
    ApplyDirectives,
    ApplyStandardPreset,

    // Left panel actions
    MirrorToggled(bool),
    DensityChanged,
    SetDensity {
        density_x: Option<f64>,
        density_y: Option<f64>,
    },
    CutTypeChanged,
    OpenRefLineSelect {
        line_idx: usize,
    },

    // Help / Print / Export
    ShowPrintPreview,
    PrintDocument,
    ExportCsvTable,
    ShowHelp,
    ShowAbout,
    CheckForUpdates,

    // Persistent settings & custom templates (%APPDATA%\kalipeditor)
    SetCurrentDensityAsDefault,
    SaveUserSettings,
    SaveSelectedLineAsUserPreset,
    DeleteSelectedUserPreset,
    ExportSettingsBundle,
    ImportSettingsBundle,
}

/// Exact Stoll Turkish `ArtTagId` labels (matches `SHP_FORMAT_SPEC.md` & Screenshot `222157.png`).
pub fn element_art_label_tr(art_tag_id: i32) -> &'static str {
    match art_tag_id {
        1 => "Ana Kesim",
        2 => "Yaka Kesimi",
        3 => "Kol Kesimi",
        4 => "Delik",
        5 => "Tiktak",
        6 => "Açıklık",
        7 => "Kesim Yaka açıklığı - İşaretleme",
        8 => "Delik - İşaretleme",
        9 => "Açıklık - İşaretleme",
        10 => "Pens",
        _ => "Ana Kesim",
    }
}

/// Exact 8 secondary element choices shown in Screenshot `222157.png`.
pub const SECONDARY_ELEMENT_TYPES: &[(i32, &str)] = &[
    (2, "Yaka Kesimi"),
    (4, "Delik"),
    (5, "Tiktak"),
    (6, "Açıklık"),
    (7, "Kesim Yaka açıklığı - İşaretleme"),
    (8, "Delik - İşaretleme"),
    (9, "Açıklık - İşaretleme"),
    (10, "Pens"),
];

/// Exact Stoll Turkish `SchnittArt` labels (`Genel kesim verileri -> Tür`).
pub const SHAPE_CUT_TYPES: &[(i32, &str)] = &[
    (-1, "Standart"),
    (0, "KW beden, ön"),
    (1, "KW beden, arka"),
    (2, "KW kollar"),
    (4, "KW yaka"),
];

pub fn shape_cut_type_label_tr(cut_type: i32) -> &'static str {
    SHAPE_CUT_TYPES
        .iter()
        .find(|&&(id, _)| id == cut_type)
        .map(|&(_, s)| s)
        .unwrap_or("Standart")
}

/// Exact Stoll Turkish `Funktion` labels in the Lines Grid (`SHP_FORMAT_SPEC.md` §5.2 & `222001.png`).
pub fn line_function_grid_label_tr(func_id: i32) -> &'static str {
    match func_id {
        0 => "",
        1 => "Reglan",
        2 => "Artırma",
        3 => "Remayözleme",
        4 => "Kenar düzeltimi",
        5 => "Tiktak",
        6 => "Remayözleme",
        8 => "Temel",
        9 => "Reglan M",
        _ => "",
    }
}

/// Map `LineSegment` function to the corresponding initial tab index in Line Properties (`0..=7`).
pub fn line_function_default_tab(func_id: i32) -> usize {
    match func_id {
        1 | 9 => 1, // Reglan
        2 => 2,     // Artırma
        3 | 6 => 3, // Remayözleme
        4 => 4,     // Kenar düzeltme
        _ => 0,     // Genel
    }
}

/// Exact Stoll Turkish `PunktArtTagId` labels (`Sol işaretler` / `Sağ işaretler`).
pub fn marker_function_label_tr(punkt_art_tag_id: i32) -> &'static str {
    match punkt_art_tag_id {
        0 => "Normal",
        1 => "üst sol yaka kesimi",
        2 => "Alt kol kesimi",
        3 => "Delik",
        _ => "Normal",
    }
}
