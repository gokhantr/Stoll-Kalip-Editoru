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

    /// Preserved narrowing/widening defaults.
    #[allow(dead_code)]
    pub directive_narrowing: i32,
    #[allow(dead_code)]
    pub directive_widening: i32,

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
            all_elements_selected: true,
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
            preset_is_user: false,
            selected_preset_idx: 1, // "2 : CMS  >6< / <1>" default as in screenshot 222001.png
            always_on_top: false,
            show_graphics: false,
            hide_readonly_columns: false,
            preset_name_as_comment: true,
            show_status_bar: true,
            toolbars: ToolbarVisibility::default(),
            show_detail_columns: false,
            show_dimensions: true,
            settings_popover_open: false,
            active_table_side: LineSide::Left,
            grid_unit_cm: false,
            right_panel_collapsed: true,
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
    AppendLine,
    InsertLineBefore,
    AddClosingLine,
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
        9 => "Tiktak",
        _ => "",
    }
}

/// Map `LineSegment` function to the corresponding initial tab index in Line Properties (`0..=7`).
pub fn line_function_default_tab(func_id: i32) -> usize {
    match func_id {
        1 => 1,     // Reglan
        2 => 2,     // Artırma
        3 | 6 | 9 => 3, // Remayözleme / K&W Tiktak
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

/// Standard crisp drop shadow for prominent modal dialogs.
pub fn modal_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 2,
        color: egui::Color32::from_black_alpha(72),
    }
}

/// Compute a centered top-left window position inside `ctx.screen_rect()` for a modal of `approx_size`.
pub fn centered_modal_pos(ctx: &egui::Context, approx_size: egui::Vec2) -> egui::Pos2 {
    let sr = ctx.screen_rect();
    let c = sr.center();
    egui::pos2(
        (c.x - approx_size.x * 0.5).max(sr.left() + 8.0),
        (c.y - approx_size.y * 0.5).max(sr.top() + 8.0),
    )
}

/// Paint a subtle dark backdrop overlay behind prominent modals so users never miss confirmations or alerts.
/// When `order` is `Order::Middle` and `block_clicks` is false, it only dims visually; when `block_clicks`
/// is true (e.g. behind an `Order::Foreground` alert), it also blocks background clicks.
pub fn show_modal_backdrop(ctx: &egui::Context, id_salt: &str, block_clicks: bool) {
    let sr = ctx.screen_rect();
    let area = egui::Area::new(egui::Id::new(id_salt))
        .order(egui::Order::Middle)
        .fixed_pos(sr.min)
        .interactable(block_clicks)
        .constrain(false);
    area.show(ctx, |ui| {
        let sense = if block_clicks {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let (rect, _) = ui.allocate_exact_size(sr.size(), sense);
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::ZERO,
            egui::Color32::from_black_alpha(68),
        );
    });
}

/// Render a prominent, centered-on-open, movable Win32/Light-Studio modal dialog with crisp border & shadow.
#[allow(clippy::too_many_arguments)]
pub fn show_studio_modal_window(
    ctx: &egui::Context,
    id: egui::Id,
    title: &str,
    open: &mut bool,
    approx_size: egui::Vec2,
    resizable: bool,
    show_icon: bool,
    foreground: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    if !*open {
        return;
    }

    let order = if foreground {
        egui::Order::Foreground
    } else {
        egui::Order::Middle
    };
    let def_pos = centered_modal_pos(ctx, approx_size);

    let mut win = egui::Window::new(title)
        .id(id)
        .order(order)
        .title_bar(false)
        .collapsible(false)
        .movable(true)
        .resizable(resizable)
        .default_pos(def_pos)
        .default_size(approx_size)
        .frame(
            egui::Frame::NONE
                .fill(theme::colors::CARD_BG)
                .corner_radius(egui::CornerRadius::same(6))
                .stroke(egui::Stroke::new(1.5_f32, theme::colors::BORDER_DARK))
                .shadow(modal_shadow()),
        );
    if !resizable {
        win = win.fixed_size(approx_size);
    }

    let mut close_clicked = false;
    win.show(ctx, |ui| {
        let title_h = 28.0_f32;
        let avail_w = if resizable {
            ui.available_width().max(approx_size.x)
        } else {
            approx_size.x
        };
        if !resizable {
            ui.set_width(approx_size.x);
            ui.set_max_width(approx_size.x);
        }
        // Use Sense::hover() so dragging anywhere on the titlebar moves the egui::Window smoothly.
        let (bar_rect, _) =
            ui.allocate_exact_size(egui::vec2(avail_w, title_h), egui::Sense::hover());

        let painter = ui.painter();
        painter.rect_filled(
            bar_rect,
            egui::CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            },
            egui::Color32::WHITE,
        );
        painter.line_segment(
            [bar_rect.left_bottom(), bar_rect.right_bottom()],
            egui::Stroke::new(1.0_f32, theme::colors::BORDER_MID),
        );

        let mut text_left = bar_rect.left() + 10.0;
        if show_icon {
            let icon_r = egui::Rect::from_min_size(
                egui::pos2(bar_rect.left() + 8.0, bar_rect.center().y - 7.0),
                egui::Vec2::splat(14.0),
            );
            painter.rect_filled(icon_r, egui::CornerRadius::same(2), theme::colors::CANVAS_BLUE);
            painter.rect_stroke(
                icon_r,
                egui::CornerRadius::same(2),
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0x30, 0x50, 0x78)),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                icon_r.shrink(3.0),
                egui::CornerRadius::ZERO,
                egui::Stroke::new(1.0_f32, theme::colors::CONTOUR_GREEN),
                egui::StrokeKind::Inside,
            );
            text_left = icon_r.right() + 7.0;
        }

        let close_w = 34.0_f32;
        let close_rect = egui::Rect::from_min_max(
            egui::pos2(bar_rect.right() - close_w, bar_rect.top()),
            bar_rect.max,
        );
        let close_resp = ui.interact(close_rect, id.with("studio_modal_close"), egui::Sense::click());
        if close_resp.hovered() {
            painter.rect_filled(
                close_rect,
                egui::CornerRadius {
                    nw: 0,
                    ne: 6,
                    sw: 0,
                    se: 0,
                },
                egui::Color32::from_rgb(0xE8, 0x11, 0x23),
            );
        }
        let x_col = if close_resp.hovered() {
            egui::Color32::WHITE
        } else {
            theme::colors::TEXT_BLACK
        };
        let xc = close_rect.center();
        painter.line_segment(
            [egui::pos2(xc.x - 4.0, xc.y - 4.0), egui::pos2(xc.x + 4.0, xc.y + 4.0)],
            egui::Stroke::new(1.2_f32, x_col),
        );
        painter.line_segment(
            [egui::pos2(xc.x + 4.0, xc.y - 4.0), egui::pos2(xc.x - 4.0, xc.y + 4.0)],
            egui::Stroke::new(1.2_f32, x_col),
        );
        if close_resp.clicked() {
            close_clicked = true;
        }

        let title_clip = egui::Rect::from_min_max(
            egui::pos2(text_left, bar_rect.top()),
            egui::pos2(close_rect.left() - 4.0, bar_rect.bottom()),
        );
        painter.with_clip_rect(title_clip).text(
            egui::pos2(text_left, bar_rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            theme::bold_font(11.5),
            theme::colors::TEXT_BLACK,
        );

        let inner_w = (avail_w - 20.0).max(1.0);
        egui::Frame::NONE
            .fill(theme::colors::CHROME_BG)
            .corner_radius(egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 6,
                se: 6,
            })
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_width(inner_w);
                if !resizable {
                    ui.set_width(inner_w);
                }
                add_contents(ui);
            });
    });

    if close_clicked {
        *open = false;
    }
}
