use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{vec2, Color32, CornerRadius, Frame, Margin, Key, Modifiers, Stroke};

use stoll_kalip_editoru::catalog::{
    export_settings_bundle, import_settings_bundle, monitor_work_area_for_rect,
    relocate_rect_into_work_area, StollCatalog, UserSettings, WindowPlacementPx,
};
use stoll_kalip_editoru::io::{import_dxf_file, load_file, save_file};
use stoll_kalip_editoru::math::{
    add_element, append_line, apply_standard_preset_to_line, auto_sync_closing_lines,
    convert_input_format, copy_line, copy_markers, delete_element, delete_line,
    element_auto_detects_shaping, element_has_base_line, ensure_all_closing_lines,
    ensure_closing_line_for_side, evaluate_document_geometry, import_external_table_lines,
    insert_line_before, paste_insert_line, paste_insert_markers, paste_line, paste_replace_markers,
    recalc_document_for_density_change, recalc_line_from_stitches_and_rows, renumber_points,
    snapshot_endline_distances, swap_left_right, sync_all_endline_elements,
    sync_all_mirrored_elements, sync_mirrored_right_lines, validate_document, DocumentGeometry,
};
use stoll_kalip_editoru::model::{
    ElementType, FormDocument, FormHeader, InputFormat, LineFunction, LineSide, PointEntry,
    PointOffset, ShapeCutType, ShapeElement, UndoStack,
};
use stoll_kalip_editoru::updater::{
    format_release_notes_summary, open_url_in_browser, spawn_update_check, UpdateStatus,
    CURRENT_VERSION, GITHUB_REPO_URL,
};

use crate::ui::icons::load_app_icon_64;

use crate::ui::cad_canvas::CadCanvasState;
use crate::ui::curve_editor_dialog::CurveEditorDialogState;
use crate::ui::format_convert_dialog::FormatConvertDialogState;
use crate::ui::graphics_window::GraphicsWindowState;
use crate::ui::grid_table::{flush_active_grid_edit, show_grid_area};
use crate::ui::left_panel::show_elements_strip;
use crate::ui::line_props_dialog::{
    LinePropsDialogState, NecklinePropsDialogState, RefLineSelectDialogState,
};
use crate::ui::menu_toolbar::{show_menu_bar, show_toolbar};
use crate::ui::point_multiply_dialog::PointMultiplyDialogState;
use crate::ui::theme::{apply_win32_theme, bold_font, colors, configure_fonts, regular_font, win32_button};
use crate::ui::top_panel::{show_settings_popover, show_summary_strip};
use crate::ui::{
    show_modal_backdrop, show_studio_modal_window, AppAction, CurveEditorRequest,
    FormatConvertRequest, LinePropsRequest, PendingDirtyAction, PointMultiplyRequest, TableMode,
    UiState,
};

/// Automated OpenGL screenshot capture sequence gated by `KALIPEDITOR_CAPTURE_DIR` env var.
/// Zero overhead when unset.
struct CaptureSequence {
    out_dir: PathBuf,
    step: usize,
    wait_frames: u8,
    requested: bool,
    temp_elem2_added: bool,
}

impl CaptureSequence {
    fn from_env() -> Option<Self> {
        let dir = std::env::var_os("KALIPEDITOR_CAPTURE_DIR")?;
        let out_dir = PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&out_dir);
        Some(Self {
            out_dir,
            step: 0,
            wait_frames: 4,
            requested: false,
            temp_elem2_added: false,
        })
    }
}

/// Main eframe application state for `Stoll Kalıp Editörü` (.shp / .shv).
pub struct KalipEditorApp {
    pub doc: FormDocument,
    pub undo: UndoStack,
    pub catalog: StollCatalog,
    pub state: UiState,
    pub cad_canvas: CadCanvasState,
    pub graphics_win: GraphicsWindowState,
    pub line_props_dlg: LinePropsDialogState,
    pub neckline_props_dlg: NecklinePropsDialogState,
    pub ref_line_select_dlg: RefLineSelectDialogState,
    pub curve_editor_dlg: CurveEditorDialogState,
    pub point_multiply_dlg: PointMultiplyDialogState,
    pub format_convert_dlg: FormatConvertDialogState,
    pub update_status: Arc<Mutex<UpdateStatus>>,
    cached_geom: Option<(FormHeader, Vec<ShapeElement>, Arc<DocumentGeometry>)>,
    capture: Option<CaptureSequence>,
    last_title: String,
    allow_close: bool,
    pending_startup_placement: Option<WindowPlacementPx>,
    startup_settle_frames: u8,
    last_normal_window_rect: Option<(i32, i32, i32, i32)>,
    window_save_deadline: Option<Instant>,
}

impl KalipEditorApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_path: Option<PathBuf>,
        startup_placement: Option<WindowPlacementPx>,
    ) -> Self {
        configure_fonts(&cc.egui_ctx);
        apply_win32_theme(&cc.egui_ctx);
        let _keep_theme_fn: fn(&egui::Context, egui::Id, &str, &mut bool, Option<egui::Vec2>, bool, bool, fn(&mut egui::Ui)) =
            crate::ui::theme::win32_window;
        let _ = (
            _keep_theme_fn,
            colors::CLOSING_ROW_TEXT,
            colors::ACCENT_AMBER_BG,
            colors::ACCENT_AMBER_TEXT,
        );

        let settings = UserSettings::load();
        let mut catalog = StollCatalog::load_with_autodetect();
        if settings.custom_stoll_defaultform_path != catalog.custom_stoll_defaultform_path {
            catalog.custom_stoll_defaultform_path = settings.custom_stoll_defaultform_path.clone();
            catalog.reload_for_cut_type(ShapeCutType::Standard);
        }

        let mut state = UiState::default();
        state.recent_files = load_mru_files();
        state.show_dimensions = settings.show_dimensions;
        state.show_detail_columns = settings.show_detail_columns;
        state.always_on_top = settings.always_on_top;
        state.preset_is_user = settings.default_template_is_user;
        state.selected_preset_idx = settings.default_template_index;
        state.user_settings = settings.clone();
        if settings.always_on_top {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        }

        let make_blank_doc = |s: &UserSettings| -> FormDocument {
            let mut d = FormDocument::new_default_template();
            d.header.density_x = s.density_x_per_100mm();
            d.header.density_y = s.density_y_per_100mm();
            d
        };

        let doc = if let Some(path) = initial_path {
            match load_file(&path) {
                Ok(loaded) => {
                    push_mru_file(&mut state.recent_files, path.clone());
                    let cut_type = ShapeCutType::from_id(loaded.header.cut_type);
                    if cut_type != catalog.active_cut_type {
                        catalog.reload_for_cut_type(cut_type);
                    }
                    state.set_status(format!("Yüklendi: {}", path.display()), false);
                    loaded
                }
                Err(e) => {
                    state.set_status(format!("Dosya açılamadı: {e}"), true);
                    make_blank_doc(&settings)
                }
            }
        } else {
            make_blank_doc(&settings)
        };
        state.directive_narrowing = doc.guidelines.std_narrow_w.clamp(1, 20);
        state.directive_widening = doc.guidelines.std_widen_w.clamp(1, 20);
        state.right_panel_collapsed = doc.elements.first().map(|e| e.mirrored).unwrap_or(true);

        let max_p = catalog.presets(state.preset_is_user).len();
        if max_p == 0 {
            state.preset_is_user = false;
            state.selected_preset_idx = state
                .selected_preset_idx
                .min(catalog.presets(false).len().saturating_sub(1));
        } else if state.selected_preset_idx >= max_p {
            state.selected_preset_idx = max_p - 1;
        }

        let update_status = Arc::new(Mutex::new(UpdateStatus::Idle));
        let ctx_upd = cc.egui_ctx.clone();
        spawn_update_check(Arc::clone(&update_status), move || {
            ctx_upd.request_repaint();
        });

        let last_normal_window_rect = startup_placement
            .map(|p| (p.x, p.y, p.w, p.h))
            .or_else(|| {
                Some((
                    settings.window_x?,
                    settings.window_y?,
                    settings.window_w?,
                    settings.window_h?,
                ))
            });

        Self {
            doc,
            undo: UndoStack::default(),
            catalog,
            state,
            cad_canvas: CadCanvasState::default(),
            graphics_win: GraphicsWindowState::default(),
            line_props_dlg: LinePropsDialogState::default(),
            neckline_props_dlg: NecklinePropsDialogState::default(),
            ref_line_select_dlg: RefLineSelectDialogState::default(),
            curve_editor_dlg: CurveEditorDialogState::default(),
            point_multiply_dlg: PointMultiplyDialogState::default(),
            format_convert_dlg: FormatConvertDialogState::default(),
            update_status,
            cached_geom: None,
            capture: CaptureSequence::from_env(),
            last_title: String::new(),
            allow_close: false,
            pending_startup_placement: startup_placement,
            startup_settle_frames: 0,
            last_normal_window_rect,
            window_save_deadline: None,
        }
    }

    fn eval_cached_geometry(&mut self) -> Arc<DocumentGeometry> {
        let hit = self
            .cached_geom
            .as_ref()
            .is_some_and(|(ch, ce, _)| *ch == self.doc.header && *ce == self.doc.elements);
        if !hit {
            let eval = Arc::new(evaluate_document_geometry(&self.doc));
            self.cached_geom = Some((self.doc.header.clone(), self.doc.elements.clone(), eval));
        }
        Arc::clone(&self.cached_geom.as_ref().expect("cached_geom populated").2)
    }

    fn compute_window_title(&self) -> String {
        let file_part = match &self.doc.file_path {
            Some(p) => p
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| p.display().to_string()),
            None => "Yeni Kalıp".to_string(),
        };
        let dirty_mark = if self.doc.dirty { "*" } else { "" };
        format!("{file_part}{dirty_mark} — Stoll Kalıp Editörü {CURRENT_VERSION}")
    }

    fn sync_window_title(&mut self, ctx: &egui::Context) {
        let title = self.compute_window_title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn push_undo(&mut self, label: &str) {
        self.undo.push(label, &self.doc);
    }

    fn reset_element_scoped_modals(&mut self) {
        self.state.editing_cell = None;
        self.state.line_props_req = None;
        self.state.curve_editor_req = None;
        self.state.point_multiply_req = None;
        self.state.neckline_props_open = false;
        self.state.ref_line_select_open = false;
        self.state.pending_mirror_confirm = false;
    }

    fn reset_document_ui_state(&mut self) {
        self.reset_element_scoped_modals();
        self.state.format_convert_req = None;
        self.state.print_preview_open = false;
        self.state.external_import_open = false;
        self.state.settings_popover_open = false;
        self.state.selected_elem = 0;
        self.state.all_elements_selected = true;
        self.state.table_mode = TableMode::LeftLines;
        self.state.active_table_side = LineSide::Left;
        self.state.right_panel_collapsed = self
            .doc
            .elements
            .first()
            .map(|e| e.mirrored)
            .unwrap_or(true);
        self.state.directive_narrowing = self.doc.guidelines.std_narrow_w.clamp(1, 20);
        self.state.directive_widening = self.doc.guidelines.std_widen_w.clamp(1, 20);
        self.clamp_selection_after_doc_change();
        if let Some(el) = self.doc.elements.first() {
            if !el.left_lines.is_empty() {
                self.state.select_single_row(0);
                self.state.focused_cell = Some((0, 2));
            }
        }
    }

    fn open_file_from_path(&mut self, path: PathBuf) {
        match load_file(&path) {
            Ok(loaded) => {
                self.catalog
                    .reload_for_cut_type(ShapeCutType::from_id(loaded.header.cut_type));
                self.doc = loaded;
                self.undo.clear();
                self.cad_canvas.clear_drag();
                self.reset_document_ui_state();
                push_mru_file(&mut self.state.recent_files, path.clone());
                self.state
                    .set_status(format!("Açıldı: {}", path.display()), false);
            }
            Err(e) => {
                self.state
                    .set_status(format!("Dosya açma hatası: {e}"), true);
            }
        }
    }

    fn open_preset_path(&mut self, path: PathBuf, is_user: bool) {
        self.open_file_from_path(path);
        if !is_user {
            self.doc.file_path = None;
            self.doc.header.write_protected = false;
        }
    }

    fn do_save(&mut self, force_save_as: bool) -> bool {
        flush_active_grid_edit(&mut self.doc, &mut self.undo, &mut self.state);
        let save_as = force_save_as || self.doc.header.write_protected;
        if !force_save_as && self.doc.header.write_protected {
            self.state
                .set_status("Salt okunur kalıp — farklı adla kaydedin.", false);
        }

        let target_path = if !save_as {
            self.doc.file_path.clone()
        } else {
            None
        };

        let path = match target_path {
            Some(p) => p,
            None => {
                let is_mm = self.doc.header.input_format == InputFormat::Mm;
                let default_ext = if is_mm { "shv" } else { "shp" };
                let base_name = if self.doc.header.name.is_empty() {
                    "Yeni"
                } else {
                    &self.doc.header.name
                };
                let default_name = format!("{base_name}.{default_ext}");
                let dialog = rfd::FileDialog::new()
                    .set_file_name(&default_name)
                    .add_filter("Stoll İlmek Kalıbı (*.shp)", &["shp"])
                    .add_filter("Stoll mm Kalıbı (*.shv)", &["shv"]);
                match dialog.save_file() {
                    Some(p) => p,
                    None => return false,
                }
            }
        };

        if force_save_as || self.doc.header.name.trim().is_empty() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !stem.is_empty() {
                    self.doc.header.name = stem.to_string();
                }
            }
        }

        // Auto-close any open contour before saving if the user hasn't added a closing line yet
        ensure_all_closing_lines(&mut self.doc);
        let report = validate_document(&self.doc);
        match save_file(&self.doc, &path) {
            Ok(()) => {
                self.doc.file_path = Some(path.clone());
                self.doc.header.write_protected = false;
                self.doc.dirty = false;
                push_mru_file(&mut self.state.recent_files, path.clone());
                if report.has_errors() {
                    let first_err = report
                        .errors()
                        .next()
                        .map(|i| i.message.as_str())
                        .unwrap_or("");
                    self.state.set_status(
                        format!("Kaydedildi (uyarı var): {} — {}", path.display(), first_err),
                        true,
                    );
                } else {
                    self.state
                        .set_status(format!("Kaydedildi: {}", path.display()), false);
                }
                true
            }
            Err(e) => {
                self.state.set_status(format!("Kaydetme hatası: {e}"), true);
                false
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<AppAction>) {
        if self.state.any_modal_open() {
            if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::F1)) {
                actions.push(AppAction::ShowHelp);
            }
            return;
        }
        let typing = ctx.wants_keyboard_input();
        let mut switch_table_mode: Option<TableMode> = None;

        ctx.input(|i| {
            let ctrl = i.modifiers.ctrl;
            let alt = i.modifiers.alt;

            if ctrl && i.key_pressed(Key::N) {
                actions.push(AppAction::NewFile);
            } else if ctrl && i.key_pressed(Key::O) {
                actions.push(AppAction::OpenShv);
            } else if ctrl && i.key_pressed(Key::S) {
                actions.push(AppAction::Save);
            } else if i.key_pressed(Key::F12) {
                actions.push(AppAction::SaveAs);
            } else if alt && i.key_pressed(Key::F4) {
                actions.push(AppAction::Exit);
            } else if ctrl && i.key_pressed(Key::Z) && !typing {
                actions.push(AppAction::Undo);
            } else if ctrl && i.key_pressed(Key::Y) && !typing {
                actions.push(AppAction::Redo);
            } else if i.key_pressed(Key::F7) {
                switch_table_mode = Some(TableMode::LeftLines);
            } else if i.key_pressed(Key::F8) {
                if !self.current_elem_mirrored() {
                    switch_table_mode = Some(TableMode::RightLines);
                }
            } else if i.key_pressed(Key::F9) {
                switch_table_mode = Some(TableMode::LeftMarkers);
            } else if i.key_pressed(Key::F10) {
                if !self.current_elem_mirrored() {
                    switch_table_mode = Some(TableMode::RightMarkers);
                }
            } else if i.key_pressed(Key::F1) {
                actions.push(AppAction::ShowHelp);
            } else if !typing && i.modifiers == Modifiers::NONE {
                if i.key_pressed(Key::Escape) {
                    self.state.selected_rows.clear();
                    self.state.row_select_anchor = None;
                    self.state.focused_cell = None;
                } else if i.key_pressed(Key::Insert) {
                    if self.state.table_mode.is_lines() {
                        actions.push(AppAction::InsertLineBefore);
                    } else {
                        actions.push(AppAction::InsertMarkerBefore);
                    }
                } else if i.key_pressed(Key::Delete) {
                    if self.state.table_mode.is_lines() {
                        actions.push(AppAction::DeleteSelectedLines);
                    } else {
                        actions.push(AppAction::DeleteSelectedMarker);
                    }
                }
            } else if !typing && ctrl {
                if i.key_pressed(Key::C) {
                    if self.state.table_mode.is_lines() {
                        actions.push(AppAction::CopySelectedLines);
                    } else {
                        actions.push(AppAction::CopySelectedMarkers);
                    }
                } else if i.modifiers.shift && i.key_pressed(Key::V) {
                    if self.state.table_mode.is_lines() {
                        actions.push(AppAction::PasteInsertLines);
                    } else {
                        actions.push(AppAction::PasteInsertMarkers);
                    }
                } else if i.key_pressed(Key::V) {
                    if self.state.table_mode.is_lines() {
                        actions.push(AppAction::PasteReplaceLines);
                    } else {
                        actions.push(AppAction::PasteReplaceMarkers);
                    }
                }
            }
        });

        if let Some(tm) = switch_table_mode {
            flush_active_grid_edit(&mut self.doc, &mut self.undo, &mut self.state);
            self.state.table_mode = tm;
            self.state.active_table_side = tm.side();
            self.state.editing_cell = None;
            self.state.select_single_row(0);
        }
    }

    fn current_elem_mirrored(&self) -> bool {
        self.doc
            .elements
            .get(self.state.selected_elem)
            .map(|e| e.mirrored)
            .unwrap_or(true)
    }

    fn sync_current_if_mirrored(&mut self) {
        if self.current_elem_mirrored() {
            sync_mirrored_right_lines(&mut self.doc, self.state.selected_elem);
        }
    }

    fn execute_actions(&mut self, ctx: &egui::Context, actions: Vec<AppAction>) {
        for action in actions {
            match action {
                AppAction::NewFile => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::NewFile);
                    } else {
                        self.do_new_file();
                    }
                }
                AppAction::OpenShv => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::OpenShv);
                    } else {
                        self.prompt_open_dialog("Stoll mm Kalıbı (*.shv)", &["shv", "shp"]);
                    }
                }
                AppAction::OpenShp => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::OpenShp);
                    } else {
                        self.prompt_open_dialog("Stoll İlmek Kalıbı (*.shp)", &["shp", "shv"]);
                    }
                }
                AppAction::OpenShpx => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::OpenShpx);
                    } else {
                        self.prompt_open_dialog("Stoll Kalıp (*.shp, *.shv)", &["shp", "shv"]);
                    }
                }
                AppAction::ImportDxf => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::ImportDxf);
                    } else {
                        self.prompt_import_dxf();
                    }
                }
                AppAction::OpenPath(path) => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::OpenPath(path));
                    } else {
                        self.open_file_from_path(path);
                    }
                }
                AppAction::OpenPresetFile { kind } => {
                    if let Some(ref root) = self.catalog.stoll_data_root {
                        let p = root.join("ConfigData").join(kind);
                        if p.exists() {
                            let is_user = kind.contains("_user");
                            if self.doc.dirty {
                                self.state.pending_dirty_action =
                                    Some(PendingDirtyAction::OpenPresetPath { path: p, is_user });
                            } else {
                                self.open_preset_path(p, is_user);
                            }
                        } else {
                            self.state.set_status(
                                format!("Standart dosya bulunamadı: {}", p.display()),
                                true,
                            );
                        }
                    } else {
                        self.state.set_status(
                            format!("Stoll ConfigData dizini bulunamadı ({kind})."),
                            true,
                        );
                    }
                }
                AppAction::Save => {
                    self.do_save(false);
                }
                AppAction::SaveAs => {
                    self.do_save(true);
                }
                AppAction::RecalcSaveAs => {
                    let target = if self.doc.header.input_format == InputFormat::Mm {
                        InputFormat::Steps
                    } else {
                        InputFormat::Mm
                    };
                    self.state.format_convert_req = Some(FormatConvertRequest {
                        target_format: target,
                        save_as_after: true,
                    });
                }
                AppAction::ConvertFormat(target) => {
                    self.state.editing_cell = None;
                    self.push_undo("Format çevrimi");
                    convert_input_format(&mut self.doc, target);
                    self.state.set_status(
                        format!("Giriş formatı değiştirildi: {}", target.label_tr()),
                        false,
                    );
                }
                AppAction::Exit => {
                    if self.doc.dirty {
                        self.state.pending_dirty_action = Some(PendingDirtyAction::Exit);
                    } else {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                AppAction::Undo => {
                    self.state.editing_cell = None;
                    self.cad_canvas.clear_drag();
                    if self.undo.undo(&mut self.doc) {
                        self.clamp_selection_after_doc_change();
                        self.state.set_status("Geri alındı", false);
                    }
                }
                AppAction::Redo => {
                    self.state.editing_cell = None;
                    self.cad_canvas.clear_drag();
                    if self.undo.redo(&mut self.doc) {
                        self.clamp_selection_after_doc_change();
                        self.state.set_status("İleri alındı", false);
                    }
                }
                AppAction::ToggleAlwaysOnTop => {
                    let level = if self.state.always_on_top {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    };
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                    self.state.user_settings.always_on_top = self.state.always_on_top;
                    if self.capture.is_none() {
                        let _ = self.state.user_settings.save();
                    }
                }
                AppAction::ToggleGraphics => {}
                AppAction::SwapLeftRight => {
                    if !self.current_elem_mirrored() {
                        self.state.editing_cell = None;
                        self.push_undo("Sol-Sağ değiştir");
                        if swap_left_right(&mut self.doc, self.state.selected_elem) {
                            self.state
                                .set_status("Sol ve sağ taraflar değiştirildi.", false);
                        }
                    }
                }
                AppAction::AddElement => {
                    self.reset_element_scoped_modals();
                    self.push_undo("Yeni eleman");
                    let idx = add_element(&mut self.doc, ElementType::Neckline, None);
                    self.state.selected_elem = idx;
                    self.state.table_mode = TableMode::LeftLines;
                    self.state.active_table_side = LineSide::Left;
                    self.state.selected_rows.clear();
                    self.state.focused_cell = None;
                    self.state
                        .set_status("Yeni eleman eklendi (Yaka Kesimi).", false);
                }
                AppAction::DeleteElement => {
                    if self.state.selected_elem > 0 {
                        self.reset_element_scoped_modals();
                        self.push_undo("Elemanı sil");
                        if delete_element(&mut self.doc, self.state.selected_elem).is_some() {
                            self.state.selected_elem =
                                self.state.selected_elem.saturating_sub(1);
                            self.state.active_table_side = LineSide::Left;
                            self.state.table_mode = TableMode::LeftLines;
                            self.state.selected_rows.clear();
                            self.state.focused_cell = None;
                            self.clamp_selection_after_doc_change();
                            if let Some(el) = self.doc.elements.get(self.state.selected_elem) {
                                if !el.left_lines.is_empty() {
                                    self.state.select_single_row(0);
                                    self.state.focused_cell = Some((0, 2));
                                }
                            }
                            self.state.set_status("Eleman silindi.", false);
                        }
                    }
                }
                AppAction::SelectAllElements => {
                    self.state.all_elements_selected = !self.state.all_elements_selected;
                }
                AppAction::OpenNecklineProps => {
                    self.state.neckline_props_open = true;
                }
                AppAction::OpenRefLineSelect { line_idx } => {
                    self.state.select_single_row(line_idx);
                    self.state.ref_line_select_open = true;
                }
                AppAction::DeleteSelectedLines => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let mut rows = self.state.selected_rows.clone();
                    rows.sort_unstable();
                    rows.dedup();
                    if let Some(el) = self.doc.elements.get(self.state.selected_elem) {
                        rows.retain(|&r| r < el.lines(side).len() && !el.is_base_line_row(side, r));
                    } else {
                        rows.clear();
                    }
                    if !rows.is_empty() {
                        self.push_undo("Çizgi sil");
                        for r in rows.into_iter().rev() {
                            delete_line(&mut self.doc, self.state.selected_elem, side, r);
                        }
                        self.sync_current_if_mirrored();
                        auto_sync_closing_lines(&mut self.doc);
                        let new_len = self
                            .doc
                            .elements
                            .get(self.state.selected_elem)
                            .map(|e| e.lines(side).len())
                            .unwrap_or(0);
                        if new_len > 0 {
                            let next_r = self
                                .state
                                .primary_row()
                                .unwrap_or(0)
                                .min(new_len.saturating_sub(1));
                            self.state.select_single_row(next_r);
                            self.state.focused_cell = Some((next_r, 2));
                        } else {
                            self.state.selected_rows.clear();
                            self.state.focused_cell = None;
                        }
                    }
                }
                AppAction::ToggleGroupSelectedLines => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let rows = self.state.selected_rows.clone();
                    if !rows.is_empty() {
                        self.push_undo("Çizgi grupla");
                        if let Some(el) = self.doc.elements.get_mut(self.state.selected_elem) {
                            let lines = el.lines_mut(side);
                            let any_grouped = rows
                                .iter()
                                .filter_map(|&r| lines.get(r))
                                .any(|l| l.group > 0);
                            let new_grp = if any_grouped {
                                0
                            } else {
                                lines.iter().map(|l| l.group).max().unwrap_or(0) + 1
                            };
                            for r in rows {
                                if let Some(l) = lines.get_mut(r) {
                                    l.group = new_grp;
                                }
                            }
                            self.doc.dirty = true;
                        }
                        self.sync_current_if_mirrored();
                    }
                }
                AppAction::AppendLine => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let was_empty = self
                        .doc
                        .elements
                        .get(self.state.selected_elem)
                        .map(|e| e.lines(side).is_empty())
                        .unwrap_or(false);
                    let elem_type = self
                        .doc
                        .elements
                        .get(self.state.selected_elem)
                        .map(|e| e.element_type())
                        .unwrap_or(ElementType::BasicShape);
                    let has_base = element_has_base_line(elem_type);
                    let auto_shaping = element_auto_detects_shaping(elem_type);
                    self.push_undo("Çizgi ekle");
                    if let Some(pos) = append_line(&mut self.doc, self.state.selected_elem, side) {
                        if was_empty && pos == 0 {
                            let hdr = self.doc.header.clone();
                            let is_base_fn = self.doc.elements[self.state.selected_elem]
                                .lines(side)
                                .get(pos)
                                .map(|l| l.function == LineFunction::BaseLine)
                                .unwrap_or(false);
                            if is_base_fn {
                                if let Some(line) = self.doc.elements[self.state.selected_elem]
                                    .lines_mut(side)
                                    .get_mut(pos)
                                {
                                    line.rows = 0;
                                    line.stitches = if side == LineSide::Left { -80 } else { 80 };
                                    recalc_line_from_stitches_and_rows(line, &hdr);
                                }
                            } else if !has_base && auto_shaping {
                                self.maybe_apply_default_template_to_line(side, pos);
                            }
                        } else if pos > 0 && auto_shaping {
                            self.maybe_apply_default_template_to_line(side, pos);
                        }
                        self.sync_current_if_mirrored();
                        auto_sync_closing_lines(&mut self.doc);
                        self.state.select_single_row(pos);
                        self.state.focused_cell = Some((pos, if was_empty && has_base { 3 } else { 2 }));
                    }
                }
                AppAction::InsertLineBefore => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let elem_type = self
                        .doc
                        .elements
                        .get(self.state.selected_elem)
                        .map(|e| e.element_type())
                        .unwrap_or(ElementType::BasicShape);
                    let has_base = element_has_base_line(elem_type);
                    let auto_shaping = element_auto_detects_shaping(elem_type);
                    let was_empty = self
                        .doc
                        .elements
                        .get(self.state.selected_elem)
                        .map(|e| e.lines(side).is_empty())
                        .unwrap_or(false);
                    let row = if has_base {
                        self.state.primary_row().unwrap_or(1).max(1)
                    } else {
                        self.state.primary_row().unwrap_or(0)
                    };
                    self.push_undo("Çizgi araya ekle");
                    if let Some(pos) =
                        insert_line_before(&mut self.doc, self.state.selected_elem, side, row)
                    {
                        if was_empty && pos == 0 {
                            let hdr = self.doc.header.clone();
                            let is_base_fn = self.doc.elements[self.state.selected_elem]
                                .lines(side)
                                .get(pos)
                                .map(|l| l.function == LineFunction::BaseLine)
                                .unwrap_or(false);
                            if is_base_fn {
                                if let Some(line) = self.doc.elements[self.state.selected_elem]
                                    .lines_mut(side)
                                    .get_mut(pos)
                                {
                                    line.rows = 0;
                                    line.stitches = if side == LineSide::Left { -80 } else { 80 };
                                    recalc_line_from_stitches_and_rows(line, &hdr);
                                }
                            } else if !has_base && auto_shaping {
                                self.maybe_apply_default_template_to_line(side, pos);
                            }
                        } else if (pos > 0 || !has_base) && auto_shaping {
                            self.maybe_apply_default_template_to_line(side, pos);
                        }
                        self.sync_current_if_mirrored();
                        auto_sync_closing_lines(&mut self.doc);
                        self.state.select_single_row(pos);
                        self.state.focused_cell = Some((pos, if was_empty && has_base { 3 } else { 2 }));
                    }
                }
                AppAction::QuickStartTemel => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let has_base = self
                        .doc
                        .elements
                        .get(self.state.selected_elem)
                        .map(|e| element_has_base_line(e.element_type()))
                        .unwrap_or(false);
                    self.push_undo(if has_base { "Temel çizgi ekle" } else { "Çizgi ekle" });
                    if let Some(pos) = append_line(&mut self.doc, self.state.selected_elem, side) {
                        let hdr = self.doc.header.clone();
                        if let Some(line) = self.doc.elements[self.state.selected_elem]
                            .lines_mut(side)
                            .get_mut(pos)
                        {
                            line.function = if has_base {
                                LineFunction::BaseLine
                            } else {
                                LineFunction::None
                            };
                            line.rows = 0;
                            line.stitches = if side == LineSide::Left { -80 } else { 80 };
                            recalc_line_from_stitches_and_rows(line, &hdr);
                        }
                        self.sync_current_if_mirrored();
                        auto_sync_closing_lines(&mut self.doc);
                        self.state.select_single_row(pos);
                        self.state.focused_cell = Some((pos, 3));
                        self.state.set_status(
                            if has_base {
                                "Temel çizgi eklendi."
                            } else {
                                "Başlangıç çizgisi eklendi."
                            },
                            false,
                        );
                    }
                }
                AppAction::QuickStartRectangle => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let has_base = self
                        .doc
                        .elements
                        .get(self.state.selected_elem)
                        .map(|e| element_has_base_line(e.element_type()))
                        .unwrap_or(false);
                    self.push_undo("Dikdörtgen kalıp taslağı");
                    let endline_dists = snapshot_endline_distances(&self.doc);
                    let hdr = self.doc.header.clone();
                    let base_dx = if side == LineSide::Left { -80 } else { 80 };
                    let first_fn = if has_base {
                        LineFunction::BaseLine
                    } else {
                        LineFunction::None
                    };
                    let spec = [
                        (first_fn, 0, base_dx, false),
                        (LineFunction::None, 120, 0, false),
                        (LineFunction::None, 0, -base_dx, true),
                    ];
                    for (func, r, s, is_closing) in spec {
                        if let Some(pos) = append_line(&mut self.doc, self.state.selected_elem, side) {
                            if let Some(line) = self.doc.elements[self.state.selected_elem]
                                .lines_mut(side)
                                .get_mut(pos)
                            {
                                line.function = func;
                                line.rows = r;
                                line.stitches = s;
                                line.last_line = 0;
                                if is_closing {
                                    line.default_widening = 1;
                                    line.default_used = 0;
                                    line.comment.clear();
                                    line.narrowing = None;
                                    line.widening = None;
                                    line.binding_off = None;
                                    line.fade_out = None;
                                    line.connection_points = None;
                                    line.child_order.clear();
                                }
                                recalc_line_from_stitches_and_rows(line, &hdr);
                            }
                        }
                    }
                    sync_all_endline_elements(&mut self.doc, &endline_dists);
                    self.sync_current_if_mirrored();
                    auto_sync_closing_lines(&mut self.doc);
                    self.state.select_single_row(0);
                    self.state.focused_cell = Some((0, 3));
                    self.state.set_status("Dikdörtgen kalıp taslağı oluşturuldu.", false);
                }
                AppAction::CopySelectedLines => {
                    let side = self.state.table_mode.side();
                    let mut copied = Vec::new();
                    let mut rows = self.state.selected_rows.clone();
                    rows.sort_unstable();
                    for r in rows {
                        if let Some(l) = copy_line(&self.doc, self.state.selected_elem, side, r) {
                            copied.push(l);
                        }
                    }
                    if !copied.is_empty() {
                        let n = copied.len();
                        self.state.copied_lines = copied;
                        self.state.set_status(format!("{n} çizgi kopyalandı."), false);
                    }
                }
                AppAction::PasteReplaceLines => {
                    if !self.state.copied_lines.is_empty() {
                        self.state.editing_cell = None;
                        let side = self.state.table_mode.side();
                        let min_sel = self
                            .state
                            .selected_rows
                            .iter()
                            .copied()
                            .min()
                            .or_else(|| self.state.primary_row())
                            .unwrap_or(0);
                        let src = self.state.copied_lines.clone();
                        let base_protected = self
                            .doc
                            .elements
                            .get(self.state.selected_elem)
                            .map(|el| {
                                el.is_base_line_row(side, min_sel)
                                    && src
                                        .first()
                                        .map(|l| l.function != LineFunction::BaseLine)
                                        .unwrap_or(true)
                            })
                            .unwrap_or(false);
                        let start_r = if base_protected { min_sel.max(1) } else { min_sel };
                        let snapshot = self.doc.clone();
                        let mut pasted = 0usize;
                        for (i, line) in src.iter().enumerate() {
                            if paste_line(
                                &mut self.doc,
                                self.state.selected_elem,
                                side,
                                start_r + i,
                                line,
                            ) {
                                pasted += 1;
                            }
                        }
                        if pasted > 0 {
                            self.undo.push("Çizgi üzerine yapıştır", &snapshot);
                            self.sync_current_if_mirrored();
                            auto_sync_closing_lines(&mut self.doc);
                            self.state.selected_rows = (start_r..start_r + pasted).collect();
                            self.state.row_select_anchor = Some(start_r);
                            self.state.focused_cell = Some((start_r, 2));
                            self.state
                                .set_status(format!("{pasted} çizgi üzerine yapıştırıldı."), false);
                        }
                    }
                }
                AppAction::PasteInsertLines => {
                    if !self.state.copied_lines.is_empty() {
                        self.state.editing_cell = None;
                        let side = self.state.table_mode.side();
                        let min_sel = self
                            .state
                            .selected_rows
                            .iter()
                            .copied()
                            .min()
                            .or_else(|| self.state.primary_row())
                            .unwrap_or(1);
                        let preserve_temel = self.state.selected_elem == 0
                            && self
                                .doc
                                .elements
                                .get(self.state.selected_elem)
                                .and_then(|e| e.lines(side).first())
                                .map(|l| l.function == LineFunction::BaseLine)
                                .unwrap_or(false);
                        let row = if preserve_temel { min_sel.max(1) } else { min_sel };
                        let src = self.state.copied_lines.clone();
                        self.push_undo("Çizgi ekle ve yapıştır");
                        let mut inserted = 0usize;
                        for (i, line) in src.iter().enumerate() {
                            if paste_insert_line(
                                &mut self.doc,
                                self.state.selected_elem,
                                side,
                                row + i,
                                line,
                            )
                            .is_some()
                            {
                                inserted += 1;
                            }
                        }
                        self.sync_current_if_mirrored();
                        auto_sync_closing_lines(&mut self.doc);
                        if inserted > 0 {
                            self.state.selected_rows = (row..row + inserted).collect();
                            self.state.row_select_anchor = Some(row);
                            self.state.focused_cell = Some((row, 2));
                            self.state
                                .set_status(format!("{inserted} çizgi araya eklendi (yapıştırıldı)."), false);
                        }
                    }
                }
                AppAction::ImportExternalTable => {
                    self.state.external_import_open = true;
                }
                AppAction::DeleteSelectedMarker => {
                    let side = self.state.table_mode.side();
                    let mut rows = self.state.selected_rows.clone();
                    rows.sort_unstable();
                    rows.dedup();
                    if !rows.is_empty() {
                        self.push_undo("İşaret sil");
                        if let Some(el) = self.doc.elements.get_mut(self.state.selected_elem) {
                            let pts = match side {
                                LineSide::Left => &mut el.left_points,
                                LineSide::Right => &mut el.right_points,
                            };
                            for r in rows.into_iter().rev() {
                                if r < pts.len() {
                                    pts.remove(r);
                                }
                            }
                            renumber_points(pts);
                            let new_len = pts.len();
                            if new_len > 0 {
                                let next = self.state.primary_row().unwrap_or(0).min(new_len - 1);
                                self.state.select_single_row(next);
                                self.state.focused_cell = Some((next, 1));
                            } else {
                                self.state.selected_rows.clear();
                                self.state.focused_cell = None;
                            }
                            self.doc.dirty = true;
                        }
                        self.sync_current_if_mirrored();
                    }
                }
                AppAction::ToggleGroupSelectedMarkers => {
                    let side = self.state.table_mode.side();
                    let rows = self.state.selected_rows.clone();
                    if !rows.is_empty() {
                        self.push_undo("İşaret grupla");
                        if let Some(el) = self.doc.elements.get_mut(self.state.selected_elem) {
                            let pts = match side {
                                LineSide::Left => &mut el.left_points,
                                LineSide::Right => &mut el.right_points,
                            };
                            let any_grouped = rows
                                .iter()
                                .filter_map(|&r| pts.get(r))
                                .any(|p| p.gruppe_tag_id > 0);
                            let new_grp = if any_grouped {
                                0
                            } else {
                                pts.iter().map(|p| p.gruppe_tag_id).max().unwrap_or(0) + 1
                            };
                            for r in rows {
                                if let Some(p) = pts.get_mut(r) {
                                    p.gruppe_tag_id = new_grp;
                                }
                            }
                            self.doc.dirty = true;
                        }
                        self.sync_current_if_mirrored();
                    }
                }
                AppAction::AppendMarker | AppAction::InsertMarkerBefore => {
                    let side = self.state.table_mode.side();
                    if self.state.selected_elem < self.doc.elements.len() {
                        self.push_undo("İşaret ekle");
                        let pt_tag_id = self.doc.next_point_tag_id();
                        let el = &mut self.doc.elements[self.state.selected_elem];
                        let pts = match side {
                            LineSide::Left => {
                                el.has_left_points_section = true;
                                &mut el.left_points
                            }
                            LineSide::Right => {
                                el.has_right_points_section = true;
                                &mut el.right_points
                            }
                        };
                        let new_pt = PointEntry {
                            index: (pts.len() + 1) as i32,
                            punkt_tag_id: pt_tag_id,
                            punkt_art_tag_id: 0,
                            ..PointEntry::default()
                        };
                        let pos = if matches!(action, AppAction::InsertMarkerBefore) {
                            self.state.primary_row().unwrap_or(0).min(pts.len())
                        } else {
                            pts.len()
                        };
                        pts.insert(pos, new_pt);
                        renumber_points(pts);
                        self.state.select_single_row(pos);
                        self.state.focused_cell = Some((pos, 1));
                        self.doc.dirty = true;
                        self.sync_current_if_mirrored();
                    }
                }
                AppAction::OpenMultiplyMarkers => {
                    self.state.point_multiply_req = Some(PointMultiplyRequest {
                        elem_idx: self.state.selected_elem,
                        side: self.state.table_mode.side(),
                        marker_idx: self.state.primary_row().unwrap_or(0),
                    });
                }
                AppAction::CopySelectedMarkers => {
                    let side = self.state.table_mode.side();
                    let copied = copy_markers(
                        &self.doc,
                        self.state.selected_elem,
                        side,
                        &self.state.selected_rows,
                    );
                    if !copied.is_empty() {
                        let n = copied.len();
                        self.state.copied_markers = copied;
                        self.state
                            .set_status(format!("{n} işaret kopyalandı."), false);
                    }
                }
                AppAction::PasteReplaceMarkers => {
                    if !self.state.copied_markers.is_empty() {
                        self.state.editing_cell = None;
                        let side = self.state.table_mode.side();
                        let start_r = self
                            .state
                            .selected_rows
                            .iter()
                            .copied()
                            .min()
                            .or_else(|| self.state.primary_row())
                            .unwrap_or(0);
                        let src = self.state.copied_markers.clone();
                        self.push_undo("İşaret yapıştır");
                        let n = paste_replace_markers(
                            &mut self.doc,
                            self.state.selected_elem,
                            side,
                            start_r,
                            &src,
                        );
                        if n > 0 {
                            self.state.selected_rows = (start_r..start_r + n).collect();
                            self.state.row_select_anchor = Some(start_r);
                            self.state.focused_cell = Some((start_r, 1));
                        }
                        self.state
                            .set_status(format!("{n} işaret üzerine yapıştırıldı."), false);
                    }
                }
                AppAction::PasteInsertMarkers => {
                    if !self.state.copied_markers.is_empty() {
                        self.state.editing_cell = None;
                        let side = self.state.table_mode.side();
                        let row = self
                            .state
                            .selected_rows
                            .iter()
                            .copied()
                            .min()
                            .or_else(|| self.state.primary_row())
                            .unwrap_or(0);
                        let src = self.state.copied_markers.clone();
                        self.push_undo("İşaret ekle ve yapıştır");
                        let n = paste_insert_markers(
                            &mut self.doc,
                            self.state.selected_elem,
                            side,
                            row,
                            &src,
                        );
                        if n > 0 {
                            self.state.selected_rows = (row..row + n).collect();
                            self.state.row_select_anchor = Some(row);
                            self.state.focused_cell = Some((row, 1));
                        }
                        self.state
                            .set_status(format!("{n} işaret araya eklendi."), false);
                    }
                }
                AppAction::OpenLineProps {
                    line_idx,
                    initial_tab,
                } => {
                    self.state.line_props_req = Some(LinePropsRequest {
                        elem_idx: self.state.selected_elem,
                        side: self.state.table_mode.side(),
                        line_idx,
                        initial_tab,
                    });
                }
                AppAction::OpenCurveEditor { line_idx } => {
                    self.state.curve_editor_req = Some(CurveEditorRequest {
                        elem_idx: self.state.selected_elem,
                        side: self.state.table_mode.side(),
                        line_idx,
                    });
                }
                AppAction::AddClosingLine => {
                    self.state.editing_cell = None;
                    let side = self.state.table_mode.side();
                    let snap = self.doc.clone();
                    if ensure_closing_line_for_side(&mut self.doc, self.state.selected_elem, side) {
                        self.undo.push("Bitiş çizgisi", &snap);
                        self.sync_current_if_mirrored();
                        if let Some(el) = self.doc.elements.get(self.state.selected_elem) {
                            let last = el.lines(side).len().saturating_sub(1);
                            self.state.select_single_row(last);
                            self.state.focused_cell = Some((last, 3));
                        }
                        self.state.set_status("Bitiş (kapanış) çizgisi güncellendi.", false);
                    } else {
                        self.state.set_status("Kalıp zaten kapalı veya boy henüz 0.", false);
                    }
                }
                AppAction::ApplyStandardPreset => {
                    let presets = self.catalog.presets(self.state.preset_is_user);
                    if let Some(preset) = presets.get(self.state.selected_preset_idx).cloned() {
                        let side = self.state.table_mode.side();
                        let copy_comment = self.state.preset_name_as_comment;
                        let target_rows: Vec<usize> = self
                            .doc
                            .elements
                            .get(self.state.selected_elem)
                            .map(|el| {
                                let closing_idx =
                                    stoll_kalip_editoru::math::closing_line_index(el, side);
                                self.state
                                    .selected_rows
                                    .iter()
                                    .copied()
                                    .filter(|&r| {
                                        r < el.lines(side).len()
                                            && !el.is_base_line_row(side, r)
                                            && closing_idx != Some(r)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if target_rows.is_empty() {
                            self.state.set_status(
                                "Şablon uygulamak için temel/bitiş dışında en az bir satır seçin.",
                                true,
                            );
                        } else {
                            self.push_undo("Standart özellikler devir al");
                            if let Some(el) = self.doc.elements.get_mut(self.state.selected_elem) {
                                let lines = el.lines_mut(side);
                                for r in target_rows {
                                    if let Some(target) = lines.get_mut(r) {
                                        apply_standard_preset_to_line(target, &preset.template_line);
                                        if copy_comment && !preset.template_line.comment.is_empty() {
                                            target.comment = preset.template_line.comment.clone();
                                        }
                                    }
                                }
                                self.doc.dirty = true;
                            }
                            self.sync_current_if_mirrored();
                            auto_sync_closing_lines(&mut self.doc);
                            self.state.set_status(
                                format!("Standart özellik uygulandı: {}", preset.display_name),
                                false,
                            );
                        }
                    }
                }
                AppAction::MirrorToggled(new_val) => {
                    self.state.editing_cell = None;
                    if new_val {
                        let has_right = self
                            .doc
                            .elements
                            .get(self.state.selected_elem)
                            .map(|el| !el.right_lines.is_empty())
                            .unwrap_or(false);
                        if has_right {
                            self.state.pending_mirror_confirm = true;
                        } else {
                            self.apply_mirror_toggle(true);
                        }
                    } else {
                        self.apply_mirror_toggle(false);
                    }
                }
                AppAction::DensityChanged => {
                    self.state.editing_cell = None;
                    self.push_undo("İlmek yoğunluğu");
                    recalc_document_for_density_change(&mut self.doc);
                    sync_all_mirrored_elements(&mut self.doc);
                    auto_sync_closing_lines(&mut self.doc);
                }
                AppAction::SetDensity {
                    density_x,
                    density_y,
                } => {
                    self.state.editing_cell = None;
                    self.push_undo("İlmek yoğunluğu");
                    if let Some(dx) = density_x {
                        self.doc.header.density_x = dx;
                    }
                    if let Some(dy) = density_y {
                        self.doc.header.density_y = dy;
                    }
                    recalc_document_for_density_change(&mut self.doc);
                    sync_all_mirrored_elements(&mut self.doc);
                    auto_sync_closing_lines(&mut self.doc);
                    self.state.set_status("İlmek yoğunluğu güncellendi.", false);
                }
                AppAction::CutTypeChanged => {
                    self.catalog
                        .reload_for_cut_type(ShapeCutType::from_id(self.doc.header.cut_type));
                    let max_p = self.catalog.presets(self.state.preset_is_user).len();
                    if max_p == 0 {
                        self.state.preset_is_user = false;
                    }
                    let cur_len = self.catalog.presets(self.state.preset_is_user).len();
                    self.state.selected_preset_idx =
                        self.state.selected_preset_idx.min(cur_len.saturating_sub(1));
                }
                AppAction::ShowPrintPreview => {
                    self.state.print_preview_open = true;
                }
                AppAction::PrintDocument => {
                    self.export_and_open_printable_html();
                }
                AppAction::ExportCsvTable => {
                    self.export_lines_csv();
                }
                AppAction::ShowHelp => {
                    self.state.help_open = true;
                }
                AppAction::ShowAbout => {
                    self.state.about_open = true;
                }
                AppAction::CheckForUpdates => {
                    let ctx_upd = ctx.clone();
                    spawn_update_check(Arc::clone(&self.update_status), move || {
                        ctx_upd.request_repaint();
                    });
                }
                AppAction::SetCurrentDensityAsDefault => {
                    let factor = (self.doc.header.density_base_mm / 10.0).max(0.1);
                    self.state.user_settings.default_density_x_per_cm =
                        (self.doc.header.density_x / factor).clamp(0.1, 50.0);
                    self.state.user_settings.default_density_y_per_cm =
                        (self.doc.header.density_y / factor).clamp(0.1, 50.0);
                    match self.state.user_settings.save() {
                        Ok(()) => self.state.set_status(
                            format!(
                                "Varsayılan 1 cm yoğunluk kaydedildi ({:.2} x {:.2}).",
                                self.state.user_settings.default_density_x_per_cm,
                                self.state.user_settings.default_density_y_per_cm
                            ),
                            false,
                        ),
                        Err(e) => self.state.set_status(e, true),
                    }
                }
                AppAction::SaveUserSettings => {
                    self.state.user_settings.show_dimensions = self.state.show_dimensions;
                    self.state.user_settings.show_detail_columns = self.state.show_detail_columns;
                    self.state.user_settings.default_template_is_user = self.state.preset_is_user;
                    self.state.user_settings.default_template_index = self.state.selected_preset_idx;
                    if let Err(e) = self.state.user_settings.save() {
                        self.state.set_status(e, true);
                    }
                }
                AppAction::SaveSelectedLineAsUserPreset => {
                    let side = self.state.table_mode.side();
                    let row_opt = self.state.primary_row();
                    let line_opt = row_opt.and_then(|r| {
                        self.doc
                            .elements
                            .get(self.state.selected_elem)
                            .and_then(|e| e.lines(side).get(r))
                            .cloned()
                    });
                    if let Some(line) = line_opt {
                        match self
                            .catalog
                            .save_line_as_user_preset(&self.state.preset_save_name, &line)
                        {
                            Ok(new_idx) => {
                                self.state.preset_is_user = true;
                                self.state.selected_preset_idx = new_idx;
                                self.state.user_settings.default_template_is_user = true;
                                self.state.user_settings.default_template_index = new_idx;
                                self.state.preset_save_name.clear();
                                let _ = self.state.user_settings.save();
                                self.state.set_status(
                                    r"Özel şablon %APPDATA%\kalipeditor\defaultform_user.shp içine kaydedildi.",
                                    false,
                                );
                            }
                            Err(e) => self.state.set_status(e, true),
                        }
                    } else {
                        self.state
                            .set_status("Önce şablon olarak kaydedilecek bir satır seçin.", true);
                    }
                }
                AppAction::DeleteSelectedUserPreset => {
                    if self.state.preset_is_user {
                        let idx = self.state.selected_preset_idx;
                        match self.catalog.delete_user_preset(idx) {
                            Ok(()) => {
                                if self.catalog.user_presets.is_empty() {
                                    self.state.preset_is_user = false;
                                    self.state.selected_preset_idx = 0;
                                } else {
                                    self.state.selected_preset_idx =
                                        idx.min(self.catalog.user_presets.len() - 1);
                                }
                                self.state.user_settings.default_template_is_user =
                                    self.state.preset_is_user;
                                self.state.user_settings.default_template_index =
                                    self.state.selected_preset_idx;
                                let _ = self.state.user_settings.save();
                                self.state.set_status("Özel şablon silindi.", false);
                            }
                            Err(e) => self.state.set_status(e, true),
                        }
                    }
                }
                AppAction::ExportSettingsBundle => {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("kalipeditor_ayarlar.kalipcfg")
                        .add_filter("Kalıp Editörü Ayarları (*.kalipcfg)", &["kalipcfg"])
                        .save_file()
                    {
                        match export_settings_bundle(&self.state.user_settings, &path) {
                            Ok(()) => self.state.set_status(
                                format!("Ayarlar dışa aktarıldı: {}", path.display()),
                                false,
                            ),
                            Err(e) => self.state.set_status(e, true),
                        }
                    }
                }
                AppAction::ImportSettingsBundle => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Kalıp Editörü Ayarları (*.kalipcfg)", &["kalipcfg"])
                        .pick_file()
                    {
                        match import_settings_bundle(&path) {
                            Ok((loaded, tpl_imported)) => {
                                self.state.show_dimensions = loaded.show_dimensions;
                                self.state.show_detail_columns = loaded.show_detail_columns;
                                self.state.preset_is_user = loaded.default_template_is_user;
                                self.state.selected_preset_idx = loaded.default_template_index;
                                self.state.user_settings = loaded.clone();
                                self.catalog.custom_stoll_defaultform_path =
                                    loaded.custom_stoll_defaultform_path;
                                self.catalog.reload_for_cut_type(ShapeCutType::from_id(
                                    self.doc.header.cut_type,
                                ));
                                let max_p = self.catalog.presets(self.state.preset_is_user).len();
                                if max_p == 0 {
                                    self.state.preset_is_user = false;
                                    self.state.selected_preset_idx = self
                                        .state
                                        .selected_preset_idx
                                        .min(self.catalog.presets(false).len().saturating_sub(1));
                                } else if self.state.selected_preset_idx >= max_p {
                                    self.state.selected_preset_idx = max_p - 1;
                                }
                                let msg = if tpl_imported {
                                    format!("Ayarlar ve özel şablonlar içe aktarıldı: {}", path.display())
                                } else {
                                    format!("Ayarlar içe aktarıldı: {}", path.display())
                                };
                                self.state.set_status(msg, false);
                            }
                            Err(e) => self.state.set_status(e, true),
                        }
                    }
                }
            }
        }
    }

    fn maybe_apply_default_template_to_line(&mut self, side: LineSide, pos: usize) {
        if !self.state.user_settings.auto_apply_default_template {
            return;
        }
        let presets = self.catalog.presets(self.state.preset_is_user);
        let Some(preset) = presets.get(self.state.selected_preset_idx).cloned() else {
            return;
        };
        let copy_comment = self.state.preset_name_as_comment;
        if let Some(el) = self.doc.elements.get_mut(self.state.selected_elem) {
            if !element_auto_detects_shaping(el.element_type()) {
                return;
            }
            if let Some(target) = el.lines_mut(side).get_mut(pos) {
                let is_closing = target.rows == 0 && target.default_widening == 1;
                if target.function != LineFunction::BaseLine && !is_closing {
                    apply_standard_preset_to_line(target, &preset.template_line);
                    if copy_comment && !preset.template_line.comment.is_empty() {
                        target.comment = preset.template_line.comment.clone();
                    }
                }
            }
        }
    }

    fn apply_mirror_toggle(&mut self, new_val: bool) {
        self.push_undo("Aynalama");
        let idx = self.state.selected_elem;
        if !new_val {
            // Initialize right side from mirrored left before turning mirroring off (`MAIN-13`)
            let need_init = self
                .doc
                .elements
                .get(idx)
                .map(|el| el.right_lines.is_empty() && !el.left_lines.is_empty())
                .unwrap_or(false);
            if need_init {
                if let Some(el) = self.doc.elements.get_mut(idx) {
                    el.mirrored = true;
                }
                sync_mirrored_right_lines(&mut self.doc, idx);
            }
            if let Some(el) = self.doc.elements.get_mut(idx) {
                el.mirrored = false;
            }
            self.state.right_panel_collapsed = false;
        } else {
            if let Some(el) = self.doc.elements.get_mut(idx) {
                el.mirrored = true;
            }
            sync_mirrored_right_lines(&mut self.doc, idx);
            self.state.active_table_side = LineSide::Left;
            self.state.right_panel_collapsed = true;
            if self.state.table_mode == TableMode::RightLines {
                self.state.table_mode = TableMode::LeftLines;
            } else if self.state.table_mode == TableMode::RightMarkers {
                self.state.table_mode = TableMode::LeftMarkers;
            }
        }
        self.doc.dirty = true;
    }

    fn clamp_selection_after_doc_change(&mut self) {
        if self.doc.elements.is_empty() {
            self.state.selected_elem = 0;
            self.state.selected_rows.clear();
            self.state.focused_cell = None;
            return;
        }
        if self.state.selected_elem >= self.doc.elements.len() {
            self.state.selected_elem = self.doc.elements.len() - 1;
        }
        let el = &self.doc.elements[self.state.selected_elem];
        if el.mirrored && self.state.active_table_side == LineSide::Right {
            self.state.active_table_side = LineSide::Left;
            self.state.table_mode = if self.state.table_mode.is_markers() {
                TableMode::LeftMarkers
            } else {
                TableMode::LeftLines
            };
        }
        let side = self.state.table_mode.side();
        let len = if self.state.table_mode.is_markers() {
            match side {
                LineSide::Left => el.left_points.len(),
                LineSide::Right => el.right_points.len(),
            }
        } else {
            el.lines(side).len()
        };
        self.state.selected_rows.retain(|&r| r < len);
        if self.state.selected_rows.is_empty() && len > 0 {
            self.state.select_single_row(0);
        }
        if let Some((r, c)) = self.state.focused_cell {
            if len == 0 {
                self.state.focused_cell = None;
            } else if r >= len {
                self.state.focused_cell = Some((len - 1, c));
            }
        }
    }

    fn do_new_file(&mut self) {
        self.doc = FormDocument::new_default_template();
        self.doc.header.density_x = self.state.user_settings.density_x_per_100mm();
        self.doc.header.density_y = self.state.user_settings.density_y_per_100mm();
        self.catalog
            .reload_for_cut_type(ShapeCutType::from_id(self.doc.header.cut_type));
        self.undo.clear();
        self.cad_canvas.clear_drag();
        self.reset_document_ui_state();
        self.state.directive_narrowing = self.doc.guidelines.std_narrow_w.clamp(1, 20);
        self.state.directive_widening = self.doc.guidelines.std_widen_w.clamp(1, 20);
        self.state.selected_rows.clear();
        self.state.focused_cell = None;
        self.state.set_status("Yeni kalıp oluşturuldu.", false);
    }

    fn prompt_import_dxf(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("DXF 2D Kontur (*.dxf)", &["dxf"])
            .pick_file()
        {
            match import_dxf_file(&p) {
                Ok(loaded) => {
                    self.doc = loaded;
                    self.undo.clear();
                    self.reset_document_ui_state();
                    self.state.set_status(
                        format!("DXF içe aktarıldı: {}", p.display()),
                        false,
                    );
                }
                Err(e) => {
                    self.state.set_status(format!("DXF hatası: {e}"), true);
                }
            }
        }
    }

    fn prompt_open_dialog(&mut self, filter_label: &str, exts: &[&str]) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(filter_label, exts)
            .add_filter("Tüm Stoll Kalıpları (*.shp, *.shv)", &["shp", "shv"])
            .pick_file()
        {
            self.open_file_from_path(path);
        }
    }

    fn export_lines_csv(&mut self) {
        let base_name = if self.doc.header.name.is_empty() {
            "Yeni"
        } else {
            &self.doc.header.name
        };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&format!("{base_name}.csv"))
            .add_filter("CSV Tablo (*.csv)", &["csv"])
            .save_file()
        else {
            return;
        };
        let mut out = String::from(
            "No;Boy;En;BoyBasamak;EnBasamak;Faktor;BoyArtaKalan;EnArtaKalan;Fonksiyon;Grup;Yorum\n",
        );
        if let Some(el) = self.doc.elements.get(self.state.selected_elem) {
            for l in el.lines(self.state.table_mode.side()) {
                out.push_str(&format!(
                    "{};{};{};{};{};{};{};{};{};{};{}\n",
                    l.index,
                    l.rows,
                    l.stitches,
                    l.step_y,
                    l.step_x,
                    l.step_factor,
                    l.rest_y,
                    l.rest_x,
                    crate::ui::line_function_grid_label_tr(l.function.to_id()),
                    l.group,
                    l.comment
                        .replace(';', ",")
                        .replace(['\r', '\n'], " ")
                ));
            }
        }
        match std::fs::write(&path, out) {
            Ok(()) => self
                .state
                .set_status(format!("CSV dışa aktarıldı: {}", path.display()), false),
            Err(e) => self.state.set_status(format!("CSV hatası: {e}"), true),
        }
    }

    fn export_and_open_printable_html(&mut self) {
        let raw_name = if self.doc.header.name.is_empty() {
            "Yeni"
        } else {
            &self.doc.header.name
        };
        let name = html_escape(raw_name);
        let mut html = format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{name} - Stoll Kalıp Editörü</title>\
             <style>body{{font-family:Segoe UI,sans-serif;font-size:12px;margin:20px;}}\
             table{{border-collapse:collapse;width:100%;margin-top:12px;}}\
             th,td{{border:1px solid #444;padding:4px 6px;text-align:right;}}\
             th{{background:#e8e8e8;text-align:center;}}</style></head><body>\
             <h2>{name}</h2><p>İlmek Yoğunluğu ({:.0} mm): &harr; {:.2} | &varr; {:.2}</p>\
             <table><tr><th>No.</th><th>Boy</th><th>En</th><th>Boy Bas.</th><th>En Bas.</th><th>Faktör</th><th>Fonksiyon</th><th>Yorum</th></tr>",
            self.doc.header.density_base_mm,
            self.doc.header.density_x,
            self.doc.header.density_y
        );
        if let Some(el) = self.doc.elements.get(self.state.selected_elem) {
            for l in el.lines(self.state.table_mode.side()) {
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td style=\"text-align:center\">{}</td><td style=\"text-align:left\">{}</td></tr>",
                    l.index,
                    l.rows,
                    l.stitches,
                    l.step_y,
                    l.step_x,
                    l.step_factor,
                    html_escape(crate::ui::line_function_grid_label_tr(l.function.to_id())),
                    html_escape(&l.comment)
                ));
            }
        }
        html.push_str("</table><script>window.print();</script></body></html>");
        let tmp_path = std::env::temp_dir().join("kalipeditor_print.html");
        match std::fs::write(&tmp_path, html) {
            Ok(()) => {
                stoll_kalip_editoru::updater::open_local_file(&tmp_path);
                self.state
                    .set_status(format!("Yazdırma raporu açıldı: {}", tmp_path.display()), false);
            }
            Err(e) => self
                .state
                .set_status(format!("Yazdırma raporu oluşturulamadı: {e}"), true),
        }
    }

    /// Delegate floating Graphics window and all modal dialogs to `ui-dialogs` modules.
    fn show_dialogs_and_floating_windows(
        &mut self,
        ctx: &egui::Context,
        geom: &DocumentGeometry,
    ) {
        // 1. Floating `Kesim:` Graphics Window
        self.graphics_win.show(ctx, &self.doc, geom, &mut self.state);

        // 2. Curve / Line Editor Modal (drawn BEFORE LinePropsDialog so both stay open simultaneously with LineProps on top)
        self.curve_editor_dlg
            .show(ctx, &mut self.doc, &mut self.undo, &mut self.state);

        // 3. 8-Tab Line Properties Modal
        self.line_props_dlg
            .show(ctx, &mut self.doc, &mut self.undo, &mut self.state, &self.catalog);

        // 3b. Neckline Element Properties Modal (`Yaka kesim özellikleri`, `50563`)
        let mut neck_open = self.state.neckline_props_open;
        if neck_open {
            self.neckline_props_dlg.show(
                ctx,
                &mut neck_open,
                &mut self.doc,
                &mut self.undo,
                &mut self.state,
                &self.catalog,
            );
            self.state.neckline_props_open = neck_open;
        }

        // 3c. Reference Line Select Modal (`Özellikleri başka çizgiden al...`, `DIALOG 4274`)
        let mut ref_open = self.state.ref_line_select_open;
        if ref_open {
            self.ref_line_select_dlg.show(
                ctx,
                &mut ref_open,
                &mut self.doc,
                &mut self.undo,
                &mut self.state,
            );
            self.state.ref_line_select_open = ref_open;
        }

        // 4. Marker Multiply Modal
        self.point_multiply_dlg
            .show(ctx, &mut self.doc, &mut self.undo, &mut self.state);

        // 5. Format Conversion / Recalculate Save As Modal
        self.format_convert_dlg
            .show(ctx, &mut self.doc, &mut self.undo, &mut self.state);

        // 6. Dirty save confirmation ("Kesim kaydedilsin mi?")
        if let Some(pending) = self.state.pending_dirty_action.clone() {
            let mut open = true;
            let mut choice: Option<u8> = None;
            show_modal_backdrop(ctx, "dirty_confirm_backdrop", true);
            show_studio_modal_window(
                ctx,
                egui::Id::new("dirty_confirm_modal"),
                "Stoll Kalıp Editörü — Kaydedilsin mi?",
                &mut open,
                vec2(360.0, 124.0),
                false,
                true,
                true,
                |ui| {
                    ui.add_space(6.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Kesim değiştirildi. Değişiklikler kaydedilsin mi?")
                                .font(bold_font(12.0)),
                        );
                    });
                    ui.add_space(14.0);
                    let row_w = 3.0 * 74.0 + 2.0 * 8.0;
                    let pad = ((ui.available_width() - row_w) * 0.5).max(0.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.add_space(pad);
                        if win32_button(ui, "Evet", vec2(74.0, 24.0), true).clicked() {
                            choice = Some(1);
                        }
                        if win32_button(ui, "Hayır", vec2(74.0, 24.0), true).clicked() {
                            choice = Some(2);
                        }
                        if win32_button(ui, "İptal", vec2(74.0, 24.0), true).clicked() {
                            choice = Some(0);
                        }
                    });
                },
            );
            if !open {
                choice = Some(0);
            }
            if let Some(c) = choice {
                let proceed = match c {
                    1 => self.do_save(false),
                    2 => true,
                    _ => false,
                };
                if c != 1 || proceed {
                    self.state.pending_dirty_action = None;
                }
                if proceed {
                    match pending {
                        PendingDirtyAction::NewFile => self.do_new_file(),
                        PendingDirtyAction::OpenShv => {
                            self.prompt_open_dialog("Stoll mm Kalıbı (*.shv)", &["shv", "shp"]);
                        }
                        PendingDirtyAction::OpenShp => {
                            self.prompt_open_dialog("Stoll İlmek Kalıbı (*.shp)", &["shp", "shv"]);
                        }
                        PendingDirtyAction::OpenShpx => {
                            self.prompt_open_dialog("Stoll Kalıp (*.shp, *.shv)", &["shp", "shv"]);
                        }
                        PendingDirtyAction::ImportDxf => self.prompt_import_dxf(),
                        PendingDirtyAction::OpenPath(p) => self.open_file_from_path(p),
                        PendingDirtyAction::OpenPresetPath { path, is_user } => {
                            self.open_preset_path(path, is_user);
                        }
                        PendingDirtyAction::Exit => {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }
        }

        // 7. Mirror confirmation dialog (`Aynalama` ON overwrites right side, `13624`)
        if self.state.pending_mirror_confirm {
            let mut open = true;
            let mut confirm: Option<bool> = None;
            show_modal_backdrop(ctx, "mirror_confirm_backdrop", true);
            show_studio_modal_window(
                ctx,
                egui::Id::new("mirror_confirm_modal"),
                "Stoll Kalıp Editörü — Aynalama",
                &mut open,
                vec2(360.0, 124.0),
                false,
                true,
                true,
                |ui| {
                    ui.add_space(6.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Sağ taraf sol tarafın aynası ile değiştirilsin mi?")
                                .font(bold_font(11.5)),
                        );
                    });
                    ui.add_space(14.0);
                    let row_w = 2.0 * 74.0 + 8.0;
                    let pad = ((ui.available_width() - row_w) * 0.5).max(0.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.add_space(pad);
                        if win32_button(ui, "Evet", vec2(74.0, 24.0), true).clicked() {
                            confirm = Some(true);
                        }
                        if win32_button(ui, "Hayır", vec2(74.0, 24.0), true).clicked() {
                            confirm = Some(false);
                        }
                    });
                },
            );
            if !open {
                confirm = Some(false);
            }
            if let Some(yes) = confirm {
                self.state.pending_mirror_confirm = false;
                if yes {
                    self.apply_mirror_toggle(true);
                }
            }
        }

        // 8. External Table Import modal (`Harici tablodan ekle`)
        if self.state.external_import_open {
            let mut open = self.state.external_import_open;
            let mut do_import = false;
            show_modal_backdrop(ctx, "external_import_backdrop", false);
            show_studio_modal_window(
                ctx,
                egui::Id::new("external_import_modal"),
                "Harici tablodan ekle (CSV / TSV)",
                &mut open,
                vec2(430.0, 250.0),
                true,
                false,
                false,
                |ui| {
                    ui.label("Her satır: Boy;En [;BoyBas;EnBas;Faktör;Yorum]");
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(155.0).show(ui, |ui| {
                        ui.add_sized(
                            vec2(400.0, 145.0),
                            egui::TextEdit::multiline(&mut self.state.external_import_text)
                                .font(egui::FontId::monospace(11.0)),
                        );
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if win32_button(ui, "Dosyadan Yükle...", vec2(115.0, 22.0), true).clicked()
                        {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("Tablo (*.csv, *.tsv, *.txt)", &["csv", "tsv", "txt"])
                                .pick_file()
                            {
                                if let Ok(txt) = std::fs::read_to_string(&p) {
                                    self.state.external_import_text = txt;
                                }
                            }
                        }
                        if win32_button(ui, "Çizgileri Ekle", vec2(96.0, 22.0), true).clicked() {
                            do_import = true;
                        }
                        if win32_button(ui, "İptal", vec2(68.0, 22.0), true).clicked() {
                            self.state.external_import_open = false;
                        }
                    });
                },
            );
            if do_import {
                let text = self.state.external_import_text.clone();
                let side = self.state.table_mode.side();
                let snapshot = self.doc.clone();
                let added = import_external_table_lines(
                    &mut self.doc,
                    self.state.selected_elem,
                    side,
                    &text,
                    false,
                );
                if added > 0 {
                    self.undo.push("Harici tablo içe aktar", &snapshot);
                    auto_sync_closing_lines(&mut self.doc);
                    self.clamp_selection_after_doc_change();
                    self.state
                        .set_status(format!("{added} çizgi harici tablodan eklendi."), false);
                } else {
                    self.state
                        .set_status("Geçerli sayısal satır bulunamadı (Boy;En).", true);
                }
                self.state.external_import_open = false;
            }
            if !open {
                self.state.external_import_open = false;
            }
        }

        // 9. Print Preview modal (`Sayfa görünümü`)
        if self.state.print_preview_open {
            let mut open = self.state.print_preview_open;
            let mut do_print = false;
            show_modal_backdrop(ctx, "print_preview_backdrop", false);
            show_studio_modal_window(
                ctx,
                egui::Id::new("print_preview_modal"),
                "Sayfa görünümü",
                &mut open,
                vec2(550.0, 370.0),
                true,
                false,
                false,
                |ui| {
                    let name = if self.doc.header.name.is_empty() {
                        "Yeni"
                    } else {
                        &self.doc.header.name
                    };
                    ui.label(egui::RichText::new(format!("Kalıp: {name}")).font(bold_font(12.5)));
                    ui.label(format!(
                        "İlmek Yoğunluğu ({:.0} mm): En {:.2}  |  Boy {:.2}   •   Eleman sayısı: {}",
                        self.doc.header.density_base_mm,
                        self.doc.header.density_x,
                        self.doc.header.density_y,
                        self.doc.elements.len()
                    ));
                    ui.separator();
                    if let Some(el) = self.doc.elements.get(self.state.selected_elem) {
                        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                            egui::Grid::new("print_preview_table")
                                .striped(true)
                                .min_col_width(48.0)
                                .show(ui, |ui| {
                                    for h in [
                                        "No.",
                                        "Boy",
                                        "En",
                                        "Bas.Y",
                                        "Bas.X",
                                        "Faktör",
                                        "Fonksiyon",
                                        "Yorum",
                                    ] {
                                        ui.label(egui::RichText::new(h).font(bold_font(10.5)));
                                    }
                                    ui.end_row();
                                    for l in el.lines(self.state.table_mode.side()) {
                                        ui.label(l.index.to_string());
                                        ui.label(l.rows.to_string());
                                        ui.label(l.stitches.to_string());
                                        ui.label(l.step_y.to_string());
                                        ui.label(l.step_x.to_string());
                                        ui.label(l.step_factor.to_string());
                                        ui.label(crate::ui::line_function_grid_label_tr(
                                            l.function.to_id(),
                                        ));
                                        ui.label(&l.comment);
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if win32_button(ui, "Yazdır...", vec2(84.0, 22.0), true).clicked() {
                            do_print = true;
                        }
                        if win32_button(ui, "Kapat", vec2(68.0, 22.0), true).clicked() {
                            self.state.print_preview_open = false;
                        }
                    });
                },
            );
            if do_print {
                self.export_and_open_printable_html();
            }
            if !open {
                self.state.print_preview_open = false;
            }
        }

        // 10. About & Updates dialog (`Hakkında & Güncellemeler`)
        if self.state.about_open {
            let mut open = self.state.about_open;
            let mut trigger_check = false;
            let status_snapshot = self
                .update_status
                .lock()
                .map(|g| g.clone())
                .unwrap_or_else(|p| p.into_inner().clone());
            let icon_tex = load_app_icon_64(ctx);
            let modal_size = if matches!(status_snapshot, UpdateStatus::UpdateAvailable(_)) {
                vec2(520.0, 410.0)
            } else {
                vec2(490.0, 320.0)
            };

            show_modal_backdrop(ctx, "about_modal_backdrop", false);
            show_studio_modal_window(
                ctx,
                egui::Id::new("about_modal"),
                "Stoll Kalıp Editörü — Hakkında & Güncellemeler",
                &mut open,
                modal_size,
                false,
                true,
                false,
                |ui| {
                    // Header with custom 64x64 icon + title + version
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Image::new((icon_tex.id(), vec2(64.0, 64.0)))
                                .corner_radius(CornerRadius::same(10)),
                        );
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Stoll Kalıp Editörü - {CURRENT_VERSION}"
                                ))
                                .font(bold_font(15.0))
                                .color(colors::TEXT_BLACK),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Stoll .shp / .shv Kalıp Editörü • Açık Kaynak (MIT)",
                                )
                                .font(regular_font(11.5))
                                .color(colors::TEXT_SECONDARY),
                            );
                            ui.label(
                                egui::RichText::new("Geliştirici: Gökhan Yücel")
                                    .font(regular_font(11.0))
                                    .color(colors::TEXT_SECONDARY),
                            );
                        });
                    });

                    ui.add_space(10.0);

                    // Description card
                    Frame::NONE
                        .fill(Color32::from_rgb(0xF0, 0xF9, 0xFF))
                        .corner_radius(CornerRadius::same(6))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0xBA, 0xE6, 0xFD)))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                egui::RichText::new(
                                    "UTF-16LE OLE2 CFB (.shp / .shv) • Bresenham / J-Eğrisi / S-Eğrisi • Tam kayıpsız M1plus uyumlu kayıt.",
                                )
                                .font(regular_font(11.0))
                                .color(Color32::from_rgb(0x0C, 0x4A, 0x6E)),
                            );
                        });

                    ui.add_space(8.0);

                    // Live GitHub Update Status card
                    Frame::NONE
                        .fill(colors::CARD_BG)
                        .corner_radius(CornerRadius::same(6))
                        .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                        .inner_margin(Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let (title_text, desc_text, status_col) = match &status_snapshot {
                                UpdateStatus::Idle | UpdateStatus::Checking => (
                                    "Güncelleme Denetleniyor...".to_string(),
                                    "GitHub üzerinden en güncel sürüm kontrol ediliyor...".to_string(),
                                    colors::TEXT_SECONDARY,
                                ),
                                UpdateStatus::UpToDate => (
                                    "Sürümünüz Güncel".to_string(),
                                    format!(
                                        "Harika! En güncel sürümü kullanıyorsunuz ({CURRENT_VERSION})."
                                    ),
                                    colors::STATUS_OK_GREEN,
                                ),
                                UpdateStatus::UpdateAvailable(info) => (
                                    format!("Yeni Sürüm Mevcut: {}!", info.tag_name),
                                    "Yeni bir güncelleme bulundu. İndir butonuna tıklayarak en son sürüme geçebilirsiniz.".to_string(),
                                    Color32::from_rgb(0x02, 0x84, 0xC7),
                                ),
                                UpdateStatus::Unavailable(err) => (
                                    "Güncelleme Durumu".to_string(),
                                    format!("Mevcut sürümünüz: {CURRENT_VERSION}. ({err})"),
                                    colors::TEXT_SECONDARY,
                                ),
                            };

                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_max_width(275.0);
                                    ui.label(
                                        egui::RichText::new(title_text)
                                            .font(bold_font(12.0))
                                            .color(status_col),
                                    );
                                    ui.label(
                                        egui::RichText::new(desc_text)
                                            .font(regular_font(11.0))
                                            .color(colors::TEXT_BLACK),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if let UpdateStatus::UpdateAvailable(info) = &status_snapshot {
                                            if win32_button(ui, "İndir", vec2(66.0, 24.0), true)
                                                .clicked()
                                            {
                                                open_url_in_browser(&info.download_url);
                                            }
                                        }
                                        let can_check =
                                            !matches!(status_snapshot, UpdateStatus::Checking);
                                        if win32_button(
                                            ui,
                                            "Güncellemeleri Denetle",
                                            vec2(142.0, 24.0),
                                            can_check,
                                        )
                                        .clicked()
                                        {
                                            trigger_check = true;
                                        }
                                    },
                                );
                            });

                            if let UpdateStatus::UpdateAvailable(info) = &status_snapshot {
                                let summary =
                                    format_release_notes_summary(&info.body_notes, 10, 650);
                                if !summary.is_empty() {
                                    ui.add_space(6.0);
                                    Frame::NONE
                                        .fill(colors::CHROME_BG)
                                        .corner_radius(CornerRadius::same(4))
                                        .stroke(Stroke::new(1.0_f32, colors::BORDER_LIGHT))
                                        .inner_margin(Margin::symmetric(8, 6))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.label(
                                                egui::RichText::new("Sürüm Notları:")
                                                    .font(bold_font(10.5))
                                                    .color(colors::TEXT_SECONDARY),
                                            );
                                            egui::ScrollArea::vertical()
                                                .max_height(76.0)
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        egui::RichText::new(summary)
                                                            .font(regular_font(10.5))
                                                            .color(colors::TEXT_BLACK),
                                                    );
                                                });
                                        });
                                }

                                ui.add_space(6.0);
                                let is_ignored = self
                                    .state
                                    .user_settings
                                    .ignored_update_version
                                    .as_deref()
                                    == Some(info.tag_name.as_str());
                                ui.horizontal(|ui| {
                                    if !is_ignored {
                                        if win32_button(
                                            ui,
                                            "Bu sürümü şimdilik gizle",
                                            vec2(168.0, 22.0),
                                            true,
                                        )
                                        .clicked()
                                        {
                                            self.state.user_settings.ignored_update_version =
                                                Some(info.tag_name.clone());
                                            if self.capture.is_none() {
                                                let _ = self.state.user_settings.save();
                                            }
                                        }
                                    } else {
                                        ui.label(
                                            egui::RichText::new(
                                                "Araç çubuğu rozeti bu sürüm için gizlendi.",
                                            )
                                            .font(regular_font(10.5))
                                            .color(colors::TEXT_SECONDARY),
                                        );
                                        if win32_button(
                                            ui,
                                            "Rozeti Göster",
                                            vec2(96.0, 22.0),
                                            true,
                                        )
                                        .clicked()
                                        {
                                            self.state.user_settings.ignored_update_version = None;
                                            if self.capture.is_none() {
                                                let _ = self.state.user_settings.save();
                                            }
                                        }
                                    }
                                });
                            }
                        });

                    ui.add_space(10.0);

                    // Bottom bar: clickable GitHub link + Tamam
                    ui.horizontal(|ui| {
                        let link = ui.add(
                            egui::Label::new(
                                egui::RichText::new(GITHUB_REPO_URL)
                                    .font(regular_font(11.0))
                                    .color(colors::WIN_BLUE)
                                    .underline(),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if link.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if link.clicked() {
                            open_url_in_browser(GITHUB_REPO_URL);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if win32_button(ui, "Tamam", vec2(78.0, 24.0), true).clicked() {
                                self.state.about_open = false;
                            }
                        });
                    });
                },
            );
            if trigger_check {
                self.execute_actions(ctx, vec![AppAction::CheckForUpdates]);
            }
            if !open {
                self.state.about_open = false;
            }
        }

        // 11. Help dialog
        if self.state.help_open {
            let mut open = self.state.help_open;
            show_modal_backdrop(ctx, "help_modal_backdrop", false);
            show_studio_modal_window(
                ctx,
                egui::Id::new("help_modal"),
                "Yardım konuları",
                &mut open,
                vec2(420.0, 160.0),
                false,
                false,
                false,
                |ui| {
                    ui.label(egui::RichText::new("Kısayollar:").font(bold_font(11.5)));
                    ui.label(
                        "Ctrl+N: Yeni   |   Ctrl+O: Aç   |   Ctrl+S: Kaydet   |   F12: Farklı Kaydet",
                    );
                    ui.label("F7 / F8: Sol / Sağ çizgiler   |   F9 / F10: Sol / Sağ işaretler");
                    ui.label(
                        "Insert: Araya çizgi ekle   |   Delete: Çizgi sil   |   Ctrl+C / Ctrl+Shift+V / Ctrl+V: Kopyala / Ekle+Yapıştır / Yapıştır",
                    );
                    ui.label(
                        "Ctrl+Z / Ctrl+Y: Geri al / İleri al   |   Tek tık: Hücre düzenle / Fonksiyon özellikleri",
                    );
                    ui.add_space(6.0);
                    if win32_button(ui, "Tamam", vec2(74.0, 22.0), true).clicked() {
                        self.state.help_open = false;
                    }
                },
            );
            if !open {
                self.state.help_open = false;
            }
        }
        // 12. Modern Studio Settings Popover (`Kalıp Ayarları`)
        let mut settings_actions: Vec<AppAction> = Vec::new();
        show_settings_popover(
            ctx,
            &mut self.doc,
            &self.catalog,
            &mut self.undo,
            &mut self.state,
            &mut settings_actions,
        );
        if !settings_actions.is_empty() {
            self.execute_actions(ctx, settings_actions);
        }
    }
}

impl KalipEditorApp {
    fn apply_startup_window_placement(&mut self, ctx: &egui::Context) -> bool {
        let Some(p) = self.pending_startup_placement else {
            return false;
        };
        let ppp = ctx.pixels_per_point().max(0.5);
        let outer_rect = ctx.input(|i| i.viewport().outer_rect);
        let pos_matches = outer_rect
            .map(|r| {
                let cur_x = (r.min.x * ppp).round() as i32;
                let cur_y = (r.min.y * ppp).round() as i32;
                (cur_x - p.x).abs() <= 4 && (cur_y - p.y).abs() <= 4
            })
            .unwrap_or(false);

        if !pos_matches && self.startup_settle_frames < 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                p.x as f32 / ppp,
                p.y as f32 / ppp,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                p.w as f32 / ppp,
                p.h as f32 / ppp,
            )));
            self.startup_settle_frames += 1;
            ctx.request_repaint();
            return true;
        }

        if p.maximized {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        self.pending_startup_placement = None;
        true
    }

    fn track_window_geometry(&mut self, ctx: &egui::Context) {
        let (outer_rect, inner_rect, maximized, minimized) = ctx.input(|i| {
            let vp = i.viewport();
            (vp.outer_rect, vp.inner_rect, vp.maximized, vp.minimized)
        });
        if minimized == Some(true) {
            return;
        }

        let is_max = maximized.unwrap_or(false);
        let ppp = ctx.pixels_per_point().max(0.5);

        if !is_max {
            if let (Some(out_r), Some(in_r)) = (outer_rect, inner_rect) {
                let x = (out_r.min.x * ppp).round() as i32;
                let y = (out_r.min.y * ppp).round() as i32;
                let w = (in_r.width() * ppp).round() as i32;
                let h = (in_r.height() * ppp).round() as i32;
                if w >= 400 && h >= 300 && (-20000..=20000).contains(&x) && (-20000..=20000).contains(&y) {
                    self.last_normal_window_rect = Some((x, y, w, h));
                }
            }
        } else if let Some(out_r) = outer_rect {
            // If maximized window moved to a different monitor (e.g. Win+Shift+Arrow),
            // relocate the saved normal restore rect onto that same monitor so next startup
            // restores and maximizes on that monitor.
            let cx = (out_r.center().x * ppp).round() as i32;
            let cy = (out_r.center().y * ppp).round() as i32;
            if let Some((cur_hmon, cur_work)) = monitor_work_area_for_rect(cx - 8, cy - 8, 16, 16) {
                let normal_same_mon = self
                    .last_normal_window_rect
                    .and_then(|(nx, ny, nw, nh)| monitor_work_area_for_rect(nx, ny, nw, nh))
                    .is_some_and(|(h, _)| h == cur_hmon);
                if !normal_same_mon {
                    let (nw, nh) = self
                        .last_normal_window_rect
                        .map(|(_, _, w, h)| (w, h))
                        .unwrap_or((1360, 860));
                    self.last_normal_window_rect =
                        Some(relocate_rect_into_work_area(nw, nh, cur_work));
                }
            }
        }

        let desired_x = self.last_normal_window_rect.map(|r| r.0);
        let desired_y = self.last_normal_window_rect.map(|r| r.1);
        let desired_w = self.last_normal_window_rect.map(|r| r.2);
        let desired_h = self.last_normal_window_rect.map(|r| r.3);

        let changed = self.state.user_settings.window_x != desired_x
            || self.state.user_settings.window_y != desired_y
            || self.state.user_settings.window_w != desired_w
            || self.state.user_settings.window_h != desired_h
            || self.state.user_settings.window_maximized != is_max;

        if changed {
            self.state.user_settings.window_x = desired_x;
            self.state.user_settings.window_y = desired_y;
            self.state.user_settings.window_w = desired_w;
            self.state.user_settings.window_h = desired_h;
            self.state.user_settings.window_maximized = is_max;
            self.window_save_deadline = Some(Instant::now() + Duration::from_millis(500));
        }

        if self
            .window_save_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.window_save_deadline = None;
            let _ = self.state.user_settings.save();
        }
    }

    fn persist_window_settings_now(&mut self) {
        if self.capture.is_some() {
            return;
        }
        self.window_save_deadline = None;
        let _ = self.state.user_settings.save();
    }
}

impl eframe::App for KalipEditorApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_window_settings_now();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_window_title(ctx);

        if self.capture.is_none() && !self.apply_startup_window_placement(ctx) {
            self.track_window_geometry(ctx);
        }

        // Intercept OS window close when document has unsaved edits (flush active grid edit first!)
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && !self.allow_close {
            crate::ui::grid_table::flush_active_grid_edit(
                &mut self.doc,
                &mut self.undo,
                &mut self.state,
            );
        }
        if close_requested && self.doc.dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.state.pending_dirty_action = Some(PendingDirtyAction::Exit);
        } else if close_requested {
            self.persist_window_settings_now();
        }

        // Handle drag-and-drop `.shp` / `.shv` files
        let dropped: Option<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .find_map(|f| f.path.clone())
        });
        let mut actions: Vec<AppAction> = Vec::new();
        if let Some(p) = dropped {
            actions.push(AppAction::OpenPath(p));
        }

        // Global shortcuts
        self.handle_shortcuts(ctx, &mut actions);

        // Evaluate 2D geometry (cached when doc.header & doc.elements are unchanged)
        let geom = self.eval_cached_geometry();

        // ── Top Chrome: Menu Bar + Streamlined Studio Toolbar ──────────────────
        egui::TopBottomPanel::top("menu_and_toolbar_panel")
            .frame(Frame::NONE.fill(colors::CARD_BG))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                show_menu_bar(
                    ui,
                    &mut self.doc,
                    &mut self.undo,
                    &mut self.state,
                    &mut actions,
                );
                let available_update_tag = self
                    .update_status
                    .lock()
                    .ok()
                    .and_then(|g| match &*g {
                        UpdateStatus::UpdateAvailable(info)
                            if self.state.user_settings.ignored_update_version.as_deref()
                                != Some(info.tag_name.as_str()) =>
                        {
                            Some(info.tag_name.clone())
                        }
                        _ => None,
                    });
                show_toolbar(
                    ui,
                    &mut self.doc,
                    &mut self.undo,
                    &mut self.state,
                    available_update_tag.as_deref(),
                    &mut actions,
                );
            });

        // ── Bottom Status Bar (`Görünüm -> Statü çubuğu`) ────────────────────────
        if self.state.show_status_bar {
            egui::TopBottomPanel::bottom("status_bar_panel")
                .exact_height(22.0)
                .frame(
                    Frame::NONE
                        .fill(colors::CARD_BG)
                        .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                        .inner_margin(Margin::symmetric(10, 3)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (msg, col) = match &self.state.status_message {
                            Some((m, true)) => (m.as_str(), colors::STATUS_ERR_RED),
                            Some((m, false)) => (m.as_str(), colors::TEXT_BLACK),
                            None => ("Hazır", colors::TEXT_SECONDARY),
                        };
                        ui.label(egui::RichText::new(msg).font(regular_font(10.5)).color(col));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(eg) = geom.elements.get(self.state.selected_elem) {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Eleman {}/{}  |  En: {} iğne ({:.1} cm)  |  Boy: {} may ({:.1} cm)",
                                        self.state.selected_elem + 1,
                                        self.doc.elements.len(),
                                        eg.max_width_stitches,
                                        eg.max_width_mm / 10.0,
                                        eg.total_height_rows,
                                        eg.total_height_mm / 10.0,
                                    ))
                                    .font(regular_font(10.5))
                                    .color(colors::TEXT_SECONDARY),
                                );
                            }
                        });
                    });
                });
        }

        // ── Left Pane (resizable splitter): Live Interactive CAD Canvas + Elemanlar ──
        ctx.style_mut(|s| {
            s.interaction.resize_grab_radius_side = 6.0;
        });
        let screen_w = ctx.screen_rect().width();
        let max_left_w = (screen_w - 420.0).max(320.0);
        let default_left_w = self
            .state
            .user_settings
            .splitter_width
            .unwrap_or_else(|| (screen_w * 0.48).clamp(320.0, 780.0))
            .clamp(280.0, max_left_w);
        let left_panel_resp = egui::SidePanel::left("studio_left_canvas_panel")
            .default_width(default_left_w)
            .min_width(280.0)
            .max_width(max_left_w)
            .resizable(true)
            .show_separator_line(true)
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(8)),
            )
            .show(ctx, |ui| {
                let avail_h = ui.available_height();
                let elem_strip_h = if ui.available_width() < 430.0 {
                    if self.doc.elements.len() > 1 { 250.0 } else { 196.0 }
                } else if self.doc.elements.len() > 1 {
                    184.0
                } else {
                    136.0
                };
                let canvas_h = (avail_h - elem_strip_h - 8.0).max(180.0);

                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), canvas_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let mutated = self.cad_canvas.show(
                            ui,
                            &mut self.doc,
                            &geom,
                            &mut self.undo,
                            &mut self.state,
                        );
                        if mutated {
                            sync_mirrored_right_lines(&mut self.doc, self.state.selected_elem);
                            auto_sync_closing_lines(&mut self.doc);
                            self.state.editing_cell = None;
                            self.doc.dirty = true;
                        }
                    },
                );

                ui.add_space(6.0);

                show_elements_strip(
                    ui,
                    &mut self.doc,
                    &geom,
                    &mut self.undo,
                    &mut self.state,
                    &mut actions,
                );
            });
        // Persist user-dragged splitter width (debounced) & draw subtle splitter grip dots
        let splitter_rect = left_panel_resp.response.rect;
        let actual_left_w = splitter_rect.width();
        if self.capture.is_none() && actual_left_w.is_finite() && actual_left_w >= 280.0 {
            let prev = self.state.user_settings.splitter_width;
            if prev.is_none_or(|p| (p - actual_left_w).abs() > 1.0) {
                self.state.user_settings.splitter_width = Some(actual_left_w);
                self.window_save_deadline = Some(Instant::now() + Duration::from_millis(500));
            }
        }
        {
            let grip_painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("studio_splitter_grip"),
            ));
            let gx = splitter_rect.right();
            let gy = splitter_rect.center().y;
            for dy in [-6.0_f32, 0.0, 6.0] {
                grip_painter.circle_filled(
                    egui::pos2(gx, gy + dy),
                    1.5,
                    colors::TEXT_DISABLED,
                );
            }
        }

        // ── Right Pane: Summary Strip + Standart Özellikler + Stacked Dual Tables ──
        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin {
                        left: 4,
                        right: 8,
                        top: 8,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                show_summary_strip(ui, &self.doc, &geom, &self.state);
                show_grid_area(
                    ui,
                    &mut self.doc,
                    &mut self.undo,
                    &mut self.state,
                    &mut actions,
                );
            });

        // Execute emitted actions
        if !actions.is_empty() {
            self.execute_actions(ctx, actions);
        }

        // Auto-persist view preferences if changed from toolbar, menu, or CAD HUD (never in capture mode)
        if self.capture.is_none()
            && (self.state.show_dimensions != self.state.user_settings.show_dimensions
                || self.state.show_detail_columns != self.state.user_settings.show_detail_columns)
        {
            self.state.user_settings.show_dimensions = self.state.show_dimensions;
            self.state.user_settings.show_detail_columns = self.state.show_detail_columns;
            let _ = self.state.user_settings.save();
        }

        // Show floating Graphics window & modal dialogs
        self.show_dialogs_and_floating_windows(ctx, &geom);

        // Automated screenshot capture hook (only active when KALIPEDITOR_CAPTURE_DIR is set)
        if self.capture.is_some() {
            self.step_capture_sequence(ctx);
        }
    }
}

impl KalipEditorApp {
    fn step_capture_sequence(&mut self, ctx: &egui::Context) {
        const BLANK_FILES: &[&str] = &["00_blank.bmp"];
        const STUDIO_FILES: &[&str] = &[
            "01_studio_main.bmp",
            "02_element2.bmp",
            "02b_studio_detail_cols.bmp",
            "03_settings_popover.bmp",
            "04_props_reglan.bmp",
            "05_curve_editor_curved.bmp",
            "05b_curve_editor_straight.bmp",
            "06_markers.bmp",
            "08_about_modal.bmp",
        ];

        let is_blank = self
            .doc
            .elements
            .first()
            .map(|e| e.left_lines.is_empty())
            .unwrap_or(true);
        let capture_files = if is_blank && self.doc.file_path.is_none() {
            BLANK_FILES
        } else {
            STUDIO_FILES
        };

        let screenshot_img = ctx.input(|i| {
            i.events.iter().find_map(|ev| match ev {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });

        let Some(mut cap) = self.capture.take() else {
            return;
        };

        if let Some(img) = screenshot_img {
            if cap.requested && cap.step < capture_files.len() {
                let path = cap.out_dir.join(capture_files[cap.step]);
                let _ = write_bmp_rgb24(&path, &img);
                cap.step += 1;
                cap.requested = false;
                cap.wait_frames = 4;
                self.configure_capture_step(cap.step, &mut cap.temp_elem2_added);
            }
        }

        if cap.step >= capture_files.len() {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if !cap.requested {
            if cap.wait_frames > 0 {
                cap.wait_frames -= 1;
                ctx.request_repaint();
            } else {
                cap.requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::UserData::default(),
                ));
                ctx.request_repaint();
            }
        } else {
            ctx.request_repaint();
        }

        self.capture = Some(cap);
    }

    fn configure_capture_step(&mut self, step: usize, temp_elem2_added: &mut bool) {
        if *temp_elem2_added && step != 1 {
            if self.doc.elements.len() > 1 {
                let last_idx = self.doc.elements.len() - 1;
                delete_element(&mut self.doc, last_idx);
            }
            *temp_elem2_added = false;
        }

        let line_count = self
            .doc
            .elements
            .first()
            .map(|e| e.left_lines.len())
            .unwrap_or(1);
        let row = |one_based: usize| one_based.saturating_sub(1).min(line_count.saturating_sub(1));

        self.state.show_graphics = false;
        self.state.settings_popover_open = false;
        self.state.about_open = false;
        self.state.show_detail_columns = false;
        self.state.line_props_req = None;
        self.state.curve_editor_req = None;
        self.state.point_multiply_req = None;
        self.state.format_convert_req = None;

        match step {
            0 => {
                self.state.selected_elem = 0;
                self.state.active_table_side = LineSide::Left;
                self.state.table_mode = TableMode::LeftLines;
                self.state.select_single_row(0);
            }
            1 => {
                if self.doc.elements.len() <= 1 {
                    add_element(&mut self.doc, ElementType::Neckline, None);
                    *temp_elem2_added = true;
                }
                self.state.selected_elem = 1;
                self.state.active_table_side = LineSide::Left;
                self.state.table_mode = TableMode::LeftLines;
                self.state.selected_rows.clear();
            }
            2 => {
                self.state.selected_elem = 0;
                self.state.show_detail_columns = true;
                self.state.select_single_row(row(3));
            }
            3 => {
                self.state.selected_elem = 0;
                self.state.settings_popover_open = true;
                self.state.select_single_row(row(3));
            }
            4 => {
                self.state.selected_elem = 0;
                let r = row(6);
                self.state.select_single_row(r);
                self.state.line_props_req = Some(LinePropsRequest {
                    elem_idx: 0,
                    side: LineSide::Left,
                    line_idx: r,
                    initial_tab: 1,
                });
            }
            5 => {
                self.state.selected_elem = 0;
                let r = row(6);
                self.state.select_single_row(r);
                self.state.curve_editor_req = Some(CurveEditorRequest {
                    elem_idx: 0,
                    side: LineSide::Left,
                    line_idx: r,
                });
            }
            6 => {
                self.state.selected_elem = 0;
                let r = row(3);
                self.state.select_single_row(r);
                self.state.curve_editor_req = Some(CurveEditorRequest {
                    elem_idx: 0,
                    side: LineSide::Left,
                    line_idx: r,
                });
            }
            7 => {
                self.state.selected_elem = 0;
                self.state.active_table_side = LineSide::Left;
                self.state.table_mode = TableMode::LeftMarkers;
                if let Some(el) = self.doc.elements.get_mut(0) {
                    if el.left_points.is_empty() {
                        el.has_left_points_section = true;
                        el.left_points.push(PointEntry {
                            index: 1,
                            punkt_art_tag_id: 0,
                            offsets: vec![PointOffset {
                                index: 0,
                                x: -40,
                                y: 120,
                                ..Default::default()
                            }],
                            ..Default::default()
                        });
                    }
                }
                self.state.select_single_row(0);
                self.state.focused_cell = Some((0, 1));
            }
            8 => {
                self.state.selected_elem = 0;
                self.state.about_open = true;
            }
            _ => {}
        }
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn mru_file_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("kalipeditor").join("mru.txt"))
}

fn load_mru_files() -> Vec<PathBuf> {
    let Some(p) = mru_file_path() else {
        return Vec::new();
    };
    let Ok(txt) = std::fs::read_to_string(p) else {
        return Vec::new();
    };
    txt.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .take(8)
        .collect()
}

fn push_mru_file(list: &mut Vec<PathBuf>, path: PathBuf) {
    list.retain(|p| p != &path);
    list.insert(0, path);
    list.truncate(8);
    if let Some(mru_p) = mru_file_path() {
        if let Some(parent) = mru_p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let body: String = list
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(mru_p, body);
    }
}

/// Write an `egui::ColorImage` as a standard uncompressed 24-bit Windows `.bmp` (zero external deps).
fn write_bmp_rgb24(path: &std::path::Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let w = image.width();
    let h = image.height();
    let row_stride = (w * 3 + 3) & !3;
    let pixel_bytes = row_stride * h;
    let file_size = 54 + pixel_bytes;

    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    let pad = row_stride - w * 3;
    for y in (0..h).rev() {
        let row_start = y * w;
        for x in 0..w {
            let c = image.pixels[row_start + x];
            out.push(c.b());
            out.push(c.g());
            out.push(c.r());
        }
        for _ in 0..pad {
            out.push(0);
        }
    }
    std::fs::write(path, out)
}
