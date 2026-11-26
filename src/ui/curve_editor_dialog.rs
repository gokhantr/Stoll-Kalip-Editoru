use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, Frame, Key, Margin, PointerButton, Pos2, Rect,
    Sense, Stroke, StrokeKind, Ui,
};

use stoll_kalip_editoru::io::{load_form_curve, save_form_curve};
use stoll_kalip_editoru::math::{
    apply_curve_to_line, auto_detect_line_function_in_context, auto_sync_closing_line_for_side,
    check_curve_feasibility, curve_steps_sum_slice, detect_even_courses_from_steps,
    element_distance_from_top, generate_curve_steps, normalize_curve_groups,
    recalc_line_from_stitches_and_rows, resample_user_curve, resolve_curve_module_widths,
    sync_endline_element_y, sync_mirrored_right_lines, CourseParityMode, CurveGenParams,
    CurveStepDrive,
};
use stoll_kalip_editoru::model::{
    CurveData, CurveForm, CurveStep, FormDocument, KanteChildSlot, LineFunction, LineSide,
    UndoStack, YReferenceLine,
};

use crate::ui::icons::{paint_icon, IconKind};
use crate::ui::theme::{
    apply_pristine_zero_guard, bold_font, colors, regular_font, win32_button, win32_checkbox,
    win32_combo_scope, win32_numeric_text_edit, win32_radio,
};
use crate::ui::{CurveEditorRequest, LinePropsRequest, UiState};

/// Active step-driving axis in Curve Editor (`En basamak` vs `Boy basamak`, mutually exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepAxisMode {
    Width,
    Height,
}

/// Modal `Çizgi editörü` (`DIALOG 4265`) for editing `[Kurve]` / `[KurvenLinie]` curve steps
/// with exact Stoll formulas (`Line`, `J-Curve`, `S-Curve`, `Reverse J-Curve`, `User Curve`),
/// `.FormCurve` load/save, multi-row grouping, and live 2D staircase plot.
#[derive(Debug, Clone)]
pub struct CurveEditorDialogState {
    active_req: Option<CurveEditorRequest>,
    orig_form: CurveForm,
    orig_steps: Vec<CurveStep>,
    pub curve_form: CurveForm,
    /// Active UI preset state for `Ters J eğrisi` (stored as `UserCurve` for Stoll `.shp` parity).
    pub reverse_j_active: bool,
    /// False when opened on a line that has NO `[Kurve]` yet until user clicks a formula or edits steps.
    pub has_chosen_formula: bool,
    /// True when the line already had a non-empty `[Kurve]` when the dialog opened (or after committing).
    had_initial_curve: bool,
    pub steps: Vec<CurveStep>,
    /// Target `(NadelZahl, Reihen)` of the parent `[Kante]`.
    pub target_dx: i32,
    pub target_dy: i32,
    /// Whether the parent line is a widening line (controls whether `En ---` edits `widening_width`).
    pub is_widening: bool,
    /// Mutually exclusive step-axis selector (`En basamak` vs `Boy basamak`).
    step_axis_mode: StepAxisMode,
    /// Horizontal step magnitude (`1..=99`) used when `step_axis_mode == StepAxisMode::Width`.
    pub step_x_mag: i32,
    /// Vertical step magnitude (`1..=99`) used when `step_axis_mode == StepAxisMode::Height`.
    pub step_y_mag: i32,
    /// `☑ Çift May` — even-course curve stepping (default from `UserSettings.default_even_courses`).
    pub even_courses: bool,
    /// `☑ Tek Sistem` — right side even courses, left side odd courses (1 course offset).
    pub single_system: bool,
    /// `Min May` (`0` = `-` disabled, `1..=9` = minimum vertical step height).
    pub min_step_height: i32,
    orig_single_system: bool,
    /// Preserved step-level module widths `(MindernBreite, ZunehmenBreite, AusblendenBreite)`.
    pub default_widths: (i32, i32, i32),
    /// Multi-row selection (`0`-based, sorted & deduplicated) for Delete / Copy / Group (`DLG-10`).
    pub selected_steps: Vec<usize>,
    /// Anchor row for `Shift+Click` and click-drag range selection.
    anchor_step: Option<usize>,
    /// Active click-drag selection anchor on the leftmost row-selector gutter.
    row_drag_selecting: Option<usize>,
    /// Focused `(row, col)` cell in the step matrix (`col` `0..=7`).
    focused_cell: Option<(usize, usize)>,
    /// Active inline cell editor `(row, col, buffer)`.
    editing_cell: Option<(usize, usize, String)>,
    /// True while active cell edit buffer has not yet received modifying input.
    editing_pristine: bool,
    /// Internal clipboard for multi-row copy/paste steps.
    pub copied_steps: Vec<CurveStep>,
    /// Live canvas zoom & pan.
    pub plot_zoom: f32,
    pub plot_pan: egui::Vec2,
    /// Hovered step index on the live 2D plot.
    hovered_plot_step: Option<usize>,
    /// Transient inline notice inside Curve Editor (e.g. `.crv` loaded/saved).
    pub notice: Option<(String, bool)>,
    /// Incremented whenever the dialog opens for a new request (resets toolbar text edit state).
    open_gen: u64,
    /// Incremented whenever `steps` are replaced/reordered outside cell editing (prevents stale egui IDs).
    table_gen: u64,
    /// Tracks whether `LinePropsDialog` was open on the previous frame so closing it refreshes `default_widths`.
    prev_line_props_open: bool,
}

impl Default for CurveEditorDialogState {
    fn default() -> Self {
        Self {
            active_req: None,
            orig_form: CurveForm::Line,
            orig_steps: Vec::new(),
            curve_form: CurveForm::Line,
            reverse_j_active: false,
            has_chosen_formula: false,
            had_initial_curve: false,
            steps: Vec::new(),
            target_dx: 0,
            target_dy: 0,
            is_widening: false,
            step_axis_mode: StepAxisMode::Width,
            step_x_mag: 1,
            step_y_mag: 2,
            even_courses: false,
            single_system: false,
            min_step_height: 0,
            orig_single_system: false,
            default_widths: (0, 0, 0),
            selected_steps: Vec::new(),
            anchor_step: None,
            row_drag_selecting: None,
            focused_cell: None,
            editing_cell: None,
            editing_pristine: false,
            copied_steps: Vec::new(),
            plot_zoom: 1.0,
            plot_pan: egui::Vec2::ZERO,
            hovered_plot_step: None,
            notice: None,
            open_gen: 0,
            table_gen: 0,
            prev_line_props_open: false,
        }
    }
}

impl CurveEditorDialogState {
    fn primary_step(&self) -> Option<usize> {
        self.selected_steps.first().copied()
    }

    fn select_single(&mut self, idx: usize) {
        self.selected_steps.clear();
        self.selected_steps.push(idx);
        self.anchor_step = Some(idx);
    }

    fn toggle_step_selection(&mut self, idx: usize) {
        if let Some(pos) = self.selected_steps.iter().position(|&x| x == idx) {
            if self.selected_steps.len() > 1 {
                self.selected_steps.remove(pos);
            }
        } else {
            self.selected_steps.push(idx);
            self.selected_steps.sort_unstable();
        }
        self.anchor_step = Some(idx);
    }

    fn active_curve_label(&self) -> &'static str {
        if !self.has_chosen_formula {
            "Seçilmedi"
        } else if self.reverse_j_active {
            "Ters J-eğrisi"
        } else {
            self.curve_form.label_tr()
        }
    }

    fn effective_stoll_curve_form(&self) -> CurveForm {
        let uses_custom_stepping = self.reverse_j_active
            || self.even_courses
            || self.single_system
            || self.min_step_height > 0
            || self.step_axis_mode == StepAxisMode::Height
            || self.step_x_mag != 1;
        if uses_custom_stepping {
            CurveForm::UserCurve
        } else {
            self.curve_form
        }
    }

    fn has_pending_changes(&self) -> bool {
        if !self.had_initial_curve {
            self.has_chosen_formula
        } else {
            self.effective_stoll_curve_form() != self.orig_form
                || self.steps != self.orig_steps
                || self.single_system != self.orig_single_system
        }
    }

    /// Commit any in-progress inline cell edit before toolbar actions or dialog apply.
    fn flush_active_edit(&mut self) {
        if let Some((row_i, col_i, buf)) = self.editing_cell.take() {
            self.apply_cell_edit(row_i, col_i, &buf);
            self.editing_pristine = false;
        }
    }

    fn apply_cell_edit(&mut self, row_i: usize, col_i: usize, raw_text: &str) {
        let Some(parsed) = raw_text.trim().parse::<i32>().ok() else {
            return;
        };
        if row_i >= self.steps.len() {
            return;
        }
        let mut changed = false;
        match col_i {
            1 => {
                // Faktör Gruplandırılmış (`group_factor` stored on first row of group; `0` on continuation rows)
                let v = parsed.clamp(1, 99);
                let gid = self.steps[row_i].group_tag_id;
                if gid > 0 {
                    let cur_gf = self
                        .steps
                        .iter()
                        .filter(|s| s.group_tag_id == gid)
                        .map(|s| s.group_factor)
                        .max()
                        .unwrap_or(1);
                    if cur_gf != v {
                        let mut first = true;
                        for s in &mut self.steps {
                            if s.group_tag_id == gid {
                                s.group_factor = if first { v } else { 0 };
                                first = false;
                            }
                        }
                        changed = true;
                    }
                } else if self.steps[row_i].group_factor != v {
                    self.steps[row_i].group_factor = v;
                    changed = true;
                }
            }
            2 => {
                // Grup (`group_tag_id`)
                let v = parsed.clamp(0, 99);
                if self.steps[row_i].group_tag_id != v {
                    self.steps[row_i].group_tag_id = v;
                    if v == 0 {
                        self.steps[row_i].group_factor = 1;
                    } else {
                        let existing_gf = self
                            .steps
                            .iter()
                            .enumerate()
                            .filter(|&(idx, s)| idx != row_i && s.group_tag_id == v)
                            .map(|(_, s)| s.group_factor)
                            .max();
                        if let Some(gf) = existing_gf {
                            self.steps[row_i].group_factor = gf.max(2);
                        } else {
                            self.steps[row_i].group_factor = self.steps[row_i].group_factor.max(2);
                        }
                        normalize_curve_groups(&mut self.steps);
                    }
                    changed = true;
                }
            }
            3 => {
                // Boy Basamak (`step_height`)
                let v = parsed.clamp(-999, 999);
                if self.steps[row_i].step_height != v {
                    self.steps[row_i].step_height = v;
                    changed = true;
                }
            }
            4 => {
                // En Basamak (`step_width`)
                let v = parsed.clamp(-999, 999);
                if self.steps[row_i].step_width != v {
                    self.steps[row_i].step_width = v;
                    changed = true;
                }
            }
            5 => {
                // Faktör (`factor >= 1`)
                let v = parsed.clamp(1, 999);
                if self.steps[row_i].factor != v {
                    self.steps[row_i].factor = v;
                    changed = true;
                }
            }
            6 => {
                // En --- / En +++
                let v = parsed.clamp(0, 999);
                let width_ref = if self.is_widening {
                    &mut self.steps[row_i].widening_width
                } else {
                    &mut self.steps[row_i].narrowing_width
                };
                if *width_ref != v {
                    *width_ref = v;
                    changed = true;
                }
            }
            7 => {
                // En \\\ (`fade_out_width`)
                let v = parsed.clamp(0, 999);
                if self.steps[row_i].fade_out_width != v {
                    self.steps[row_i].fade_out_width = v;
                    changed = true;
                }
            }
            _ => {}
        }
        if changed {
            self.has_chosen_formula = true;
            self.curve_form = CurveForm::UserCurve;
            self.reverse_j_active = false;
        }
    }

    fn format_step_cell(&self, row_i: usize, col_i: usize) -> String {
        let Some(st) = self.steps.get(row_i) else {
            return String::new();
        };
        match col_i {
            0 => format!("{}", row_i + 1),
            1 => {
                if st.group_tag_id > 0 && st.group_factor == 0 {
                    String::new()
                } else {
                    st.group_factor.to_string()
                }
            }
            2 => st.group_tag_id.to_string(),
            3 => st.step_height.to_string(),
            4 => st.step_width.to_string(),
            5 => st.factor.to_string(),
            6 => {
                if self.is_widening {
                    st.widening_width.to_string()
                } else {
                    st.narrowing_width.to_string()
                }
            }
            7 => st.fade_out_width.to_string(),
            _ => String::new(),
        }
    }

    /// Render `Çizgi editörü` (`DIALOG 4265`) when `state.curve_editor_req` is `Some`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
    ) {
        let Some(req) = state.curve_editor_req else {
            self.active_req = None;
            return;
        };

        if self.active_req != Some(req) {
            if let Some(line) = doc
                .elements
                .get(req.elem_idx)
                .and_then(|e| e.lines(req.side).get(req.line_idx))
            {
                self.active_req = Some(req);
                self.open_gen = self.open_gen.wrapping_add(1);
                self.table_gen = self.table_gen.wrapping_add(1);
                self.target_dx = line.stitches;
                self.target_dy = line.rows;
                self.is_widening = line.function == LineFunction::Widening;
                self.default_widths = resolve_curve_module_widths(
                    line,
                    doc.guidelines.std_narrow_w,
                    doc.guidelines.std_widen_w,
                    doc.guidelines.std_fadeout_w,
                );
                self.even_courses = state.user_settings.default_even_courses;
                self.single_system = line.single_system_curve;
                self.min_step_height = 0;
                self.orig_single_system = self.single_system;
                self.step_axis_mode = StepAxisMode::Width;
                self.step_y_mag = if self.even_courses { 2 } else { 1 };
                self.reverse_j_active = false;
                self.editing_cell = None;
                self.editing_pristine = false;
                self.row_drag_selecting = None;
                self.plot_zoom = 1.0;
                self.plot_pan = egui::Vec2::ZERO;
                self.hovered_plot_step = None;
                self.notice = None;

                if let Some(ref c) = line.curve.as_ref().filter(|c| !c.steps.is_empty()) {
                    self.had_initial_curve = true;
                    self.has_chosen_formula = true;
                    self.curve_form = CurveForm::from_id(c.function);
                    self.steps = c.steps.clone();
                    normalize_curve_groups(&mut self.steps);
                    // Auto-detect `Çift May` from existing step heights (zero non-Stoll `.shp` fields)
                    self.even_courses =
                        detect_even_courses_from_steps(&self.steps, self.single_system);
                    self.step_y_mag = if self.even_courses { 2 } else { 1 };
                    self.step_x_mag = c
                        .steps
                        .iter()
                        .find(|s| s.step_width != 0)
                        .map(|s| s.step_width.abs())
                        .unwrap_or(1)
                        .clamp(1, 99);
                    if self.curve_form == CurveForm::UserCurve {
                        let sx_signed = if line.stitches < 0 {
                            -self.step_x_mag
                        } else {
                            self.step_x_mag
                        };
                        let (nw, ww, fw) = self.default_widths;
                        let parity_candidates = [self.parity_mode(req.side), CourseParityMode::Any];
                        for pm in parity_candidates {
                            let rj = generate_curve_steps(&CurveGenParams {
                                curve_form: CurveForm::JCurve,
                                total_dx: line.stitches,
                                total_dy: line.rows,
                                step_x: sx_signed,
                                step_y: 0,
                                narrowing_width: nw,
                                widening_width: ww,
                                fade_out_width: fw,
                                parity_mode: pm,
                                step_drive: CurveStepDrive::ByWidth,
                                reverse_j: true,
                                min_step_height: 0,
                            });
                            if !rj.is_empty()
                                && rj.len() == c.steps.len()
                                && rj.iter().zip(c.steps.iter()).all(|(a, b)| {
                                    a.step_width == b.step_width
                                        && a.step_height == b.step_height
                                        && a.factor == b.factor
                                })
                            {
                                self.reverse_j_active = true;
                                break;
                            }
                        }
                    }
                } else {
                    self.had_initial_curve = false;
                    self.has_chosen_formula = false;
                    self.curve_form = line
                        .curve
                        .as_ref()
                        .map(|c| CurveForm::from_id(c.function))
                        .unwrap_or(CurveForm::Line);
                    // Only reuse `line.step_x` when the straight line was actually stepped (`step_factor > 1`);
                    // unstepped single-segment lines store `step_x == stitches`, which would collapse curves to 1 step.
                    self.step_x_mag = if line.step_factor > 1 && line.step_x != 0 {
                        line.step_x.abs().clamp(1, 99)
                    } else {
                        1
                    };
                    let (nw, ww, fw) = self.default_widths;
                    self.steps = generate_curve_steps(&CurveGenParams {
                        curve_form: self.curve_form,
                        total_dx: self.target_dx,
                        total_dy: self.target_dy,
                        step_x: if self.target_dx < 0 {
                            -self.step_x_mag
                        } else {
                            self.step_x_mag
                        },
                        step_y: if self.target_dy < 0 {
                            -self.step_y_mag
                        } else {
                            self.step_y_mag
                        },
                        narrowing_width: nw,
                        widening_width: ww,
                        fade_out_width: fw,
                        parity_mode: self.parity_mode(req.side),
                        step_drive: self.step_drive(),
                        reverse_j: false,
                        min_step_height: self.min_step_height,
                    });
                }
                self.orig_form = self.curve_form;
                self.orig_steps = self.steps.clone();
                self.selected_steps.clear();
                if !self.steps.is_empty() {
                    self.selected_steps.push(0);
                    self.anchor_step = Some(0);
                    self.focused_cell = Some((0, 1));
                } else {
                    self.anchor_step = None;
                    self.focused_cell = None;
                }
            } else {
                state.curve_editor_req = None;
                self.active_req = None;
                return;
            }
        }

        let line_props_open_now = state.line_props_req.is_some();
        if self.prev_line_props_open && !line_props_open_now {
            if let Some(line) = doc
                .elements
                .get(req.elem_idx)
                .and_then(|e| e.lines(req.side).get(req.line_idx))
            {
                self.is_widening = line.function == LineFunction::Widening;
                self.default_widths = resolve_curve_module_widths(
                    line,
                    doc.guidelines.std_narrow_w,
                    doc.guidelines.std_widen_w,
                    doc.guidelines.std_fadeout_w,
                );
            }
        }
        self.prev_line_props_open = line_props_open_now;

        let side_lbl = match req.side {
            LineSide::Left => "Sol",
            LineSide::Right => "Sağ",
        };
        let title = format!(
            "Çizgi editörü - {} No.: {} ({} - dX: {}, dY: {})",
            side_lbl,
            req.line_idx + 1,
            self.active_curve_label(),
            self.target_dx,
            self.target_dy
        );

        let was_editing_cell = self.editing_cell.is_some();
        let mut do_apply = false;
        let mut do_close = !was_editing_cell
            && !line_props_open_now
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(Key::Escape));
        let mut open_props_after = false;

        let screen_sz = ctx.screen_rect().size();
        let def_w = 880.0_f32.min((screen_sz.x - 24.0).max(680.0));
        let def_h = 560.0_f32.min((screen_sz.y - 24.0).max(440.0));
        let min_w = 760.0_f32.min(def_w);
        let min_h = 460.0_f32.min(def_h);
        let center = ctx.screen_rect().center();
        let def_pos = pos2(
            (center.x - def_w * 0.5).max(8.0),
            (center.y - def_h * 0.5).max(8.0),
        );

        egui::Window::new(&title)
            .id(egui::Id::new("curve_editor_modal"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_pos(def_pos)
            .default_size(vec2(def_w, def_h))
            .min_size(vec2(min_w, min_h))
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                // Classic Win32 white `#FFFFFF` titlebar (`DLG-01`)
                if paint_win32_curve_titlebar(ui, &title) {
                    do_close = true;
                }

                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);

                        // ── 1. Top Toolbar (`TOOLBAR 12105` + Load/Save `.FormCurve` + Presets) ──
                        self.show_toolbar(ui, req.side, &mut open_props_after);

                        ui.add_space(4.0);

                        // ── 2. Split View: Left Step Matrix Grid | Right Live Curve Plot ─
                        // Exact non-growing geometry: `left_w + gap + right_w <= total_w` and
                        // `body_h + item_spacing + footer_box_h <= available_height` so egui::Window
                        // never auto-expands frame-over-frame.
                        let total_w = ui.available_width().floor().max(560.0);
                        let gap = 8.0_f32;
                        let max_left = (total_w - gap - 200.0).max(360.0);
                        let min_left = 468.0_f32.min(max_left);
                        let left_w = (total_w * 0.62).clamp(min_left, max_left).floor();
                        let right_w = (total_w - left_w - gap).max(190.0).floor();

                        let footer_box_h = 34.0_f32;
                        let body_h = (ui.available_height() - footer_box_h - 4.0)
                            .floor()
                            .max(180.0);

                        ui.allocate_ui_with_layout(
                            vec2(total_w, body_h),
                            egui::Layout::left_to_right(egui::Align::Min),
                            |ui| {
                                ui.spacing_mut().item_spacing = vec2(gap, 0.0);

                                // Left Pane: Step Grid + Live Closure Badge
                                ui.allocate_ui_with_layout(
                                    vec2(left_w, body_h),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
                                        let closure_h = 28.0_f32;
                                        let table_h = (body_h - closure_h - 4.0).max(120.0);
                                        self.show_step_table(ui, left_w, table_h);
                                        self.show_closure_bar(ui, left_w, closure_h);
                                    },
                                );

                                // Right Pane: Live 2D Staircase Curve Canvas (`#F4F6FA`)
                                self.show_curve_plot(ui, vec2(right_w, body_h));
                            },
                        );

                        // ── 3. Footer: Notice + Tamam | İptal et | Uygula ────────────────
                        ui.allocate_ui_with_layout(
                            vec2(total_w, footer_box_h),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
                                ui.separator();
                                ui.horizontal(|ui| {
                                    if let Some((ref msg, is_err)) = self.notice {
                                        ui.label(
                                            egui::RichText::new(msg)
                                                .font(regular_font(10.5))
                                                .color(if is_err {
                                                    colors::STATUS_ERR_RED
                                                } else {
                                                    colors::STATUS_OK_GREEN
                                                }),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            if win32_button(ui, "Uygula", vec2(74.0, 23.0), true)
                                                .clicked()
                                            {
                                                do_apply = true;
                                            }
                                            if win32_button(ui, "İptal et", vec2(74.0, 23.0), true)
                                                .clicked()
                                            {
                                                do_close = true;
                                            }
                                            let ok_btn = egui::Button::new("Tamam")
                                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                                .stroke(Stroke::new(1.5_f32, colors::WIN_BLUE))
                                                .corner_radius(CornerRadius::ZERO);
                                            if ui.add_sized(vec2(74.0, 23.0), ok_btn).clicked() {
                                                do_apply = true;
                                                do_close = true;
                                            }
                                        },
                                    );
                                });
                            },
                        );
                    });
            });

        if open_props_after {
            self.flush_active_edit();
            if self.has_pending_changes() {
                self.commit_to_doc(doc, undo, state, req);
            }
            let init_tab = doc
                .elements
                .get(req.elem_idx)
                .and_then(|e| e.lines(req.side).get(req.line_idx))
                .map(|l| crate::ui::line_function_default_tab(l.function.to_id()))
                .unwrap_or(0);
            state.line_props_req = Some(LinePropsRequest {
                elem_idx: req.elem_idx,
                side: req.side,
                line_idx: req.line_idx,
                initial_tab: init_tab,
            });
            // Keep Curve Editor open simultaneously while Line Properties is open
        } else if do_apply {
            self.flush_active_edit();
            self.commit_to_doc(doc, undo, state, req);
        }

        if do_close {
            state.curve_editor_req = None;
            self.active_req = None;
            self.editing_cell = None;
            self.row_drag_selecting = None;
        }
    }

    fn show_toolbar(&mut self, ui: &mut Ui, side: LineSide, open_props_after: &mut bool) {
        Frame::NONE
            .fill(colors::CHROME_BG)
            .stroke(Stroke::new(1.0_f32, colors::BORDER_LIGHT))
            .inner_margin(Margin::symmetric(4, 3))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    // Slots 1..2 (`TOOLBAR 12105`): Load / Save `.FormCurve`
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::LoadCurve,
                        "Eğriyi/Bükümü yükle (.FormCurve)",
                        true,
                        false,
                    ) {
                        self.editing_cell = None;
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Stoll Eğri Dosyası (*.FormCurve)", &["FormCurve", "formcurve"])
                            .add_filter("Tüm Dosyalar (*.*)", &["*"])
                            .pick_file()
                        {
                            match load_form_curve(&path) {
                                Ok(loaded) => {
                                    self.has_chosen_formula = true;
                                    self.curve_form = CurveForm::from_id(loaded.function);
                                    self.reverse_j_active = false;
                                    self.steps = loaded.steps;
                                    normalize_curve_groups(&mut self.steps);
                                    self.renumber_local_steps();
                                    self.table_gen = self.table_gen.wrapping_add(1);
                                    self.selected_steps.clear();
                                    if !self.steps.is_empty() {
                                        self.selected_steps.push(0);
                                        self.anchor_step = Some(0);
                                        self.focused_cell = Some((0, 1));
                                    } else {
                                        self.anchor_step = None;
                                        self.focused_cell = None;
                                    }
                                    self.notice = Some((
                                        format!("Eğri yüklendi ({} basamak).", self.steps.len()),
                                        false,
                                    ));
                                }
                                Err(e) => {
                                    self.notice = Some((format!("Yükleme hatası: {e}"), true));
                                }
                            }
                        }
                    }

                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::SaveCurve,
                        "Eğriyi/Bükümü farklı kaydet (.FormCurve)",
                        !self.steps.is_empty(),
                        false,
                    ) {
                        self.flush_active_edit();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("egri.FormCurve")
                            .add_filter("Stoll Eğri Dosyası (*.FormCurve)", &["FormCurve", "formcurve"])
                            .save_file()
                        {
                            let mut steps_out = self.steps.clone();
                            normalize_curve_groups(&mut steps_out);
                            let data = CurveData {
                                function: self.effective_stoll_curve_form().to_id(),
                                steps: steps_out,
                                raw: Default::default(),
                            };
                            match save_form_curve(&path, &data) {
                                Ok(()) => {
                                    self.notice =
                                        Some((format!("Kaydedildi: {}", path.display()), false));
                                }
                                Err(e) => {
                                    self.notice = Some((format!("Kaydetme hatası: {e}"), true));
                                }
                            }
                        }
                    }

                    ui.separator();

                    // Curve Formula Presets: Doğru, J, S, Ters J, Kullanıcı (`Düz çizgi üst` removed)
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::Line,
                        "Doğru oluştur (Bresenham / CalcFormula_Line)",
                        true,
                        self.has_chosen_formula
                            && !self.reverse_j_active
                            && self.curve_form == CurveForm::Line,
                    ) {
                        self.editing_cell = None;
                        self.has_chosen_formula = true;
                        self.reverse_j_active = false;
                        self.curve_form = CurveForm::Line;
                        self.regenerate_formula(side);
                    }
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::JCurve,
                        "J-eğrisi oluştur (CalcFormula_J)",
                        true,
                        self.has_chosen_formula
                            && !self.reverse_j_active
                            && self.curve_form == CurveForm::JCurve,
                    ) {
                        self.editing_cell = None;
                        self.has_chosen_formula = true;
                        self.reverse_j_active = false;
                        self.curve_form = CurveForm::JCurve;
                        self.regenerate_formula(side);
                    }
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::SCurve,
                        "S-eğrisi oluştur (CalcFormula_S)",
                        true,
                        self.has_chosen_formula
                            && !self.reverse_j_active
                            && self.curve_form == CurveForm::SCurve,
                    ) {
                        self.editing_cell = None;
                        self.has_chosen_formula = true;
                        self.reverse_j_active = false;
                        self.curve_form = CurveForm::SCurve;
                        self.regenerate_formula(side);
                    }
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::ReverseJCurve,
                        "Ters J eğrisi oluştur (Simetrik J-eğrisi)",
                        true,
                        self.has_chosen_formula && self.reverse_j_active,
                    ) {
                        self.editing_cell = None;
                        self.has_chosen_formula = true;
                        self.reverse_j_active = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.regenerate_formula(side);
                    }
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::UserCurve,
                        "Kullanıcı eğrisi / Serbest mod",
                        true,
                        self.has_chosen_formula
                            && !self.reverse_j_active
                            && self.curve_form == CurveForm::UserCurve,
                    ) {
                        self.flush_active_edit();
                        self.has_chosen_formula = true;
                        self.reverse_j_active = false;
                        self.curve_form = CurveForm::UserCurve;
                    }

                    ui.separator();

                    // Vertical Flip (`Boyuna`): reverses step order while keeping total (dX, dY) intact
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::FlipVertical,
                        "Eğriyi Ters Çevir (Boyuna)",
                        !self.steps.is_empty(),
                        false,
                    ) {
                        self.flush_active_edit();
                        let mut gf_map = std::collections::HashMap::new();
                        for s in &self.steps {
                            if s.group_tag_id > 0 && s.group_factor > 1 {
                                gf_map.insert(s.group_tag_id, s.group_factor);
                            }
                        }
                        self.steps.reverse();
                        for s in &mut self.steps {
                            if s.group_tag_id > 0 {
                                if let Some(&gf) = gf_map.get(&s.group_tag_id) {
                                    s.group_factor = gf;
                                }
                            }
                        }
                        normalize_curve_groups(&mut self.steps);
                        self.renumber_local_steps();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    // Horizontal Flip (`Enine`): negates `step_width` signs (mirrors horizontal profile)
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::FlipHorizontal,
                        "Eğriyi Ters Çevir (Enine)",
                        !self.steps.is_empty(),
                        false,
                    ) {
                        self.flush_active_edit();
                        for s in &mut self.steps {
                            s.step_width = -s.step_width;
                        }
                        self.renumber_local_steps();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    ui.separator();

                    // Insert step before
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::InsertBefore,
                        "Seçilmişin önüne yeni satır ekle",
                        true,
                        false,
                    ) {
                        self.flush_active_edit();
                        let pos = self.primary_step().unwrap_or(0).min(self.steps.len());
                        let mut st = self.new_default_step();
                        st.index = (pos + 1) as i32;
                        self.steps.insert(pos, st);
                        self.renumber_local_steps();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        self.select_single(pos);
                        self.focused_cell = Some((pos, 1));
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    // Append step
                    if tb_icon_btn(ui, CurveTbIcon::Append, "Sona yeni satır ekle", true, false) {
                        self.flush_active_edit();
                        let st = self.new_default_step();
                        self.steps.push(st);
                        self.renumber_local_steps();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        let new_idx = self.steps.len() - 1;
                        self.select_single(new_idx);
                        self.focused_cell = Some((new_idx, 1));
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    // Delete selected step(s) (multi-row aware!)
                    let has_sel = self.selected_steps.iter().any(|&i| i < self.steps.len());
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::DeleteSelected,
                        "Seçilmiş satırları sil",
                        has_sel,
                        false,
                    ) {
                        self.editing_cell = None;
                        let mut to_del: Vec<usize> = self
                            .selected_steps
                            .iter()
                            .copied()
                            .filter(|&i| i < self.steps.len())
                            .collect();
                        to_del.sort_unstable();
                        to_del.dedup();
                        let first_deleted = to_del.first().copied().unwrap_or(0);
                        for idx in to_del.into_iter().rev() {
                            self.steps.remove(idx);
                        }
                        normalize_curve_groups(&mut self.steps);
                        self.renumber_local_steps();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        self.selected_steps.clear();
                        if !self.steps.is_empty() {
                            let next_r = first_deleted.min(self.steps.len() - 1);
                            self.select_single(next_r);
                            self.focused_cell = Some((next_r, 1));
                        } else {
                            self.anchor_step = None;
                            self.focused_cell = None;
                        }
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    // Clear all steps
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::ClearAll,
                        "Bütün satırları sil",
                        !self.steps.is_empty(),
                        false,
                    ) {
                        self.editing_cell = None;
                        self.steps.clear();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        self.selected_steps.clear();
                        self.anchor_step = None;
                        self.focused_cell = None;
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    ui.separator();

                    // Copy selected step(s)
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::Copy,
                        "Seçili basamakları kopyala",
                        has_sel,
                        false,
                    ) {
                        self.flush_active_edit();
                        let mut idxs: Vec<usize> = self
                            .selected_steps
                            .iter()
                            .copied()
                            .filter(|&i| i < self.steps.len())
                            .collect();
                        idxs.sort_unstable();
                        idxs.dedup();
                        self.copied_steps = idxs.into_iter().map(|i| self.steps[i].clone()).collect();
                    }

                    // Paste step(s)
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::Paste,
                        "Kopyalanan basamakları yapıştır",
                        !self.copied_steps.is_empty(),
                        false,
                    ) {
                        self.flush_active_edit();
                        let pos = self
                            .selected_steps
                            .iter()
                            .max()
                            .map(|&i| (i + 1).min(self.steps.len()))
                            .unwrap_or(self.steps.len());
                        let count = self.copied_steps.len();
                        for (k, cp) in self.copied_steps.clone().into_iter().enumerate() {
                            self.steps.insert(pos + k, cp);
                        }
                        self.renumber_local_steps();
                        self.table_gen = self.table_gen.wrapping_add(1);
                        self.selected_steps = (pos..pos + count).collect();
                        self.anchor_step = Some(pos);
                        self.focused_cell = Some((pos, 1));
                        self.has_chosen_formula = true;
                        self.curve_form = CurveForm::UserCurve;
                        self.reverse_j_active = false;
                    }

                    // Multi-row Group / Ungroup (`DLG-10`)
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::Group,
                        "Seçilmiş satırları gruplandır veya gruplandırmayı kaldır (Shift/Ctrl/Sürükleme ile çoklu seçim)",
                        has_sel,
                        false,
                    ) {
                        self.flush_active_edit();
                        let valid: Vec<usize> = self
                            .selected_steps
                            .iter()
                            .copied()
                            .filter(|&i| i < self.steps.len())
                            .collect();
                        if !valid.is_empty() {
                            let all_grouped = valid.iter().all(|&i| self.steps[i].group_tag_id > 0);
                            if all_grouped {
                                for &i in &valid {
                                    self.steps[i].group_tag_id = 0;
                                    self.steps[i].group_factor = 1;
                                }
                            } else {
                                let next_g = self
                                    .steps
                                    .iter()
                                    .map(|s| s.group_tag_id)
                                    .max()
                                    .unwrap_or(0)
                                    + 1;
                                let shared_gf = valid
                                    .iter()
                                    .map(|&i| self.steps[i].group_factor)
                                    .max()
                                    .unwrap_or(2)
                                    .max(2);
                                for (k, &i) in valid.iter().enumerate() {
                                    self.steps[i].group_tag_id = next_g;
                                    self.steps[i].group_factor = if k == 0 { shared_gf } else { 0 };
                                }
                                normalize_curve_groups(&mut self.steps);
                            }
                            self.has_chosen_formula = true;
                            self.curve_form = CurveForm::UserCurve;
                            self.reverse_j_active = false;
                        }
                    }

                    // Open line properties / Zoom fit
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::Props,
                        "Çizgi özelliklerini aç",
                        true,
                        false,
                    ) {
                        *open_props_after = true;
                    }
                    if tb_icon_btn(
                        ui,
                        CurveTbIcon::ZoomFit,
                        "Grafiği sığdır (Yakınlaştırmayı sıfırla)",
                        true,
                        false,
                    ) {
                        self.plot_zoom = 1.0;
                        self.plot_pan = egui::Vec2::ZERO;
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(3.0);

                // Row 2: Mutually exclusive `En basamak (İğne)` & `Boy basamak (May)` spinners + `Çift May` & `Tek Sistem`
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let mut regen = false;

                    if curve_step_axis_spinner(
                        ui,
                        &mut self.step_axis_mode,
                        StepAxisMode::Width,
                        "En basamak (İğne):",
                        ("curve_tb_step_x", self.open_gen),
                        &mut self.step_x_mag,
                    ) {
                        regen = true;
                    }

                    ui.separator();

                    if curve_step_axis_spinner(
                        ui,
                        &mut self.step_axis_mode,
                        StepAxisMode::Height,
                        "Boy basamak (May):",
                        ("curve_tb_step_y", self.open_gen),
                        &mut self.step_y_mag,
                    ) {
                        regen = true;
                    }

                    ui.separator();

                    if win32_checkbox(ui, &mut self.even_courses, "Çift May")
                        .on_hover_text("Üretilen eğri basamaklarında çift may (2, 4, 6...) kullan")
                        .changed()
                    {
                        regen = true;
                    }

                    if win32_checkbox(ui, &mut self.single_system, "Tek Sistem")
                        .on_hover_text("Sağ çift may, sol tek may (1 may kayık)")
                        .changed()
                    {
                        regen = true;
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(
                            egui::RichText::new("Min May:")
                                .font(regular_font(11.0))
                                .color(colors::TEXT_BLACK),
                        )
                        .on_hover_text("Formüldeki en küçük boy (may) basamağını sınırla ('-' = serbest, 1..9 = en az o kadar may)");
                        let prev_min = self.min_step_height;
                        let sel_txt = if self.min_step_height <= 0 {
                            "-".to_string()
                        } else {
                            self.min_step_height.to_string()
                        };
                        win32_combo_scope(ui, |ui| {
                            egui::ComboBox::from_id_salt(("curve_min_may", self.open_gen))
                                .width(42.0)
                                .selected_text(sel_txt)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.min_step_height, 0, "-");
                                    for m in 1..=9 {
                                        ui.selectable_value(
                                            &mut self.min_step_height,
                                            m,
                                            m.to_string(),
                                        );
                                    }
                                });
                        });
                        if self.min_step_height != prev_min {
                            regen = true;
                        }
                    });

                    if regen && (self.reverse_j_active || self.curve_form != CurveForm::UserCurve) {
                        self.regenerate_formula(side);
                    }
                });
            });
    }

    fn show_step_table(&mut self, ui: &mut Ui, width: f32, height: f32) {
        let is_widening = self.is_widening;
        let open_gen = self.open_gen;
        let table_gen = self.table_gen;
        let row_count = self.steps.len();

        let w_hdr = if is_widening { "En\n+++" } else { "En\n---" };
        let col_specs: [(&str, f32); 8] = [
            ("", 30.0),
            ("Faktör", 62.0),
            ("Grup", 48.0),
            ("Boy", 62.0),
            ("En", 62.0),
            ("Faktör", 58.0),
            (w_hdr, 52.0),
            ("En\n\\\\\\", 52.0),
        ];
        let base_total_w: f32 = col_specs.iter().map(|(_, w)| *w).sum();

        // Responsive row & font scaling matching the main Lines Grid
        let scale = (width / 460.0).clamp(1.0, 1.45);
        let font_scale = (1.0 + (scale - 1.0) * 0.78).clamp(1.0, 1.35);
        let header_h = (32.0 * scale).round();
        let summary_h = (25.0 * scale).round();
        let row_h = (25.0 * scale).round();
        let header_font_sz = 11.5 * font_scale;
        let cell_font_sz = 13.0 * font_scale;
        let gutter_font_sz = 11.5 * font_scale;

        // Compute totals for the pinned summary row directly beneath column headers
        let (sum_dx, sum_dy) = curve_steps_sum_slice(&self.steps);

        // Keyboard navigation when no cell TextEdit is active and toolbar input doesn't have focus
        let mut scroll_target = self.handle_step_table_keyboard(ui);

        let mut pending_commit: Option<(usize, usize, String, CurveEditMove)> = None;
        let mut pending_start_edit: Option<(usize, usize, String)> = None;
        let mut cancel_edit = false;
        let edit_init_id = ui.id().with(("curve_step_edit_init", open_gen));
        if self.editing_cell.is_none() {
            ui.ctx().data_mut(|d| d.remove::<(u64, usize, usize)>(edit_init_id));
        }

        ui.allocate_ui_with_layout(
            vec2(width, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Frame::NONE
                    .fill(colors::INPUT_BG)
                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                    .inner_margin(Margin::same(2))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        let inner_w = ui.available_width().floor().max(320.0);
                        let inner_h = ui.available_height().floor().max(100.0);
                        ui.set_width(inner_w);
                        ui.set_height(inner_h);

                        let scrollbar_w = (ui.spacing().scroll.bar_width + 2.0).max(12.0);
                        let target_table_w = (inner_w - scrollbar_w).max(base_total_w);
                        let rest_base = base_total_w - col_specs[0].1;
                        let rest_target = target_table_w - col_specs[0].1;
                        let col_widths: [f32; 8] = std::array::from_fn(|i| {
                            if i == 0 {
                                col_specs[0].1
                            } else {
                                (col_specs[i].1 * (rest_target / rest_base)).floor()
                            }
                        });
                        let table_w: f32 = col_widths.iter().sum();

                        // ── Pinned Column Header + Totals Summary Row ──────────
                        let (top_rect, _) =
                            ui.allocate_exact_size(vec2(table_w, header_h + summary_h), Sense::hover());
                        let top_painter = ui.painter();

                        let mut hx = top_rect.left();
                        for (ci, &(hdr_txt, _)) in col_specs.iter().enumerate() {
                            let cw = col_widths[ci];
                            let cell_r = Rect::from_min_size(pos2(hx, top_rect.top()), vec2(cw, header_h));
                            top_painter.rect_filled(
                                cell_r,
                                CornerRadius::ZERO,
                                Color32::from_rgb(0xF1, 0xF5, 0xF9),
                            );
                            top_painter.rect_stroke(
                                cell_r,
                                CornerRadius::ZERO,
                                Stroke::new(0.8_f32, colors::BORDER_DARK),
                                StrokeKind::Inside,
                            );
                            if !hdr_txt.is_empty() {
                                top_painter.text(
                                    cell_r.center(),
                                    Align2::CENTER_CENTER,
                                    hdr_txt,
                                    bold_font(header_font_sz),
                                    colors::TEXT_BLACK,
                                );
                            }
                            hx += cw;
                        }

                        // Totals Summary Row directly under headers (matches Stoll Screenshot 2)
                        let sum_y = top_rect.top() + header_h;
                        let mut sx = top_rect.left();
                        for ci in 0..8 {
                            let cw = col_widths[ci];
                            let cell_r = Rect::from_min_size(pos2(sx, sum_y), vec2(cw, summary_h));
                            let fill = if ci == 0 {
                                colors::CHROME_BG
                            } else {
                                colors::CARD_BG
                            };
                            top_painter.rect_filled(cell_r, CornerRadius::ZERO, fill);
                            top_painter.rect_stroke(
                                cell_r,
                                CornerRadius::ZERO,
                                Stroke::new(0.8_f32, colors::BORDER_DARK),
                                StrokeKind::Inside,
                            );
                            if ci == 3 {
                                top_painter.text(
                                    pos2(cell_r.right() - 6.0, cell_r.center().y),
                                    Align2::RIGHT_CENTER,
                                    sum_dy.to_string(),
                                    bold_font(cell_font_sz),
                                    colors::TEXT_BLACK,
                                );
                            } else if ci == 4 {
                                top_painter.text(
                                    pos2(cell_r.right() - 6.0, cell_r.center().y),
                                    Align2::RIGHT_CENTER,
                                    sum_dx.to_string(),
                                    bold_font(cell_font_sz),
                                    colors::TEXT_BLACK,
                                );
                            }
                            sx += cw;
                        }
                        // Crisp divider line under totals row
                        top_painter.line_segment(
                            [
                                pos2(top_rect.left(), sum_y + summary_h),
                                pos2(top_rect.left() + table_w, sum_y + summary_h),
                            ],
                            Stroke::new(1.2_f32, colors::BORDER_DARK),
                        );

                        // ── Scrollable Data Rows ───────────────────────────────
                        let scroll_h = (inner_h - header_h - summary_h).max(40.0);
                        egui::ScrollArea::vertical()
                            .id_salt(("curve_steps_table_scroll", open_gen))
                            .max_height(scroll_h)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let canvas_h = ((row_count as f32) * row_h).max(scroll_h);
                                let (rows_rect, _) =
                                    ui.allocate_exact_size(vec2(table_w, canvas_h), Sense::hover());

                                // Click-drag across leftmost row-selector column
                                if let Some(drag_row) = self.update_gutter_drag_selection(
                                    ui,
                                    rows_rect.top(),
                                    rows_rect.left(),
                                    row_h,
                                    table_w,
                                ) {
                                    scroll_target = Some((drag_row, 0));
                                }

                                let (shift_down, ctrl_down, primary_pressed) = ui.input(|i| {
                                    (
                                        i.modifiers.shift,
                                        i.modifiers.ctrl,
                                        i.pointer.button_pressed(PointerButton::Primary),
                                    )
                                });

                                let painter = ui.painter().clone();
                                let clip_rect = ui.clip_rect();
                                let rows_top = rows_rect.top();
                                let first_vis = (((clip_rect.top() - rows_top) / row_h).floor()
                                    as isize
                                    - 1)
                                .max(0) as usize;
                                let last_vis = ((((clip_rect.bottom() - rows_top) / row_h).ceil()
                                    as isize
                                    + 1)
                                .max(0) as usize)
                                    .min(row_count);
                                let active_edit_row = self
                                    .editing_cell
                                    .as_ref()
                                    .map(|(er, _, _)| *er)
                                    .filter(|&er| er < row_count);
                                let mut rows_to_render: Vec<usize> = (first_vis..last_vis).collect();
                                if let Some(er) = active_edit_row {
                                    if er < first_vis || er >= last_vis {
                                        rows_to_render.push(er);
                                    }
                                }

                                for r in rows_to_render {
                                    let ry = rows_rect.top() + (r as f32) * row_h;
                                    let is_row_sel = self.selected_steps.contains(&r);
                                    let multi_sel = is_row_sel && self.selected_steps.len() > 1;
                                    let is_plot_hov = self.hovered_plot_step == Some(r);
                                    let step_group_zero = self.steps[r].group_tag_id == 0;

                                    let mut cx = rows_rect.left();
                                    for (ci, &cw) in col_widths.iter().enumerate() {
                                        let cell_r = Rect::from_min_size(pos2(cx, ry), vec2(cw, row_h));
                                        let is_focused = self.focused_cell == Some((r, ci));
                                        let is_editing = matches!(
                                            self.editing_cell,
                                            Some((er, ec, _)) if er == r && ec == ci
                                        );

                                        let bg = if ci == 0 {
                                            if multi_sel {
                                                Color32::from_rgb(0xBF, 0xDB, 0xFE)
                                            } else if is_row_sel {
                                                colors::ROW_SELECT_TINT
                                            } else if is_plot_hov {
                                                colors::WIN_BLUE_HOVER
                                            } else {
                                                colors::CHROME_BG
                                            }
                                        } else if multi_sel && !is_focused {
                                            colors::WIN_BLUE_FILL
                                        } else if ci == 2 && step_group_zero {
                                            // Grey shaded `Grup` cell when 0 (matches Screenshot 2)
                                            Color32::from_rgb(0xD6, 0xDC, 0xE5)
                                        } else if is_plot_hov {
                                            colors::WIN_BLUE_HOVER
                                        } else {
                                            colors::INPUT_BG
                                        };

                                        painter.rect_filled(cell_r, CornerRadius::ZERO, bg);
                                        painter.rect_stroke(
                                            cell_r,
                                            CornerRadius::ZERO,
                                            Stroke::new(0.8_f32, colors::BORDER_MID),
                                            StrokeKind::Inside,
                                        );

                                        if ci == 0 && is_row_sel {
                                            let bar = Rect::from_min_size(
                                                cell_r.min,
                                                vec2(3.0, cell_r.height()),
                                            );
                                            painter.rect_filled(bar, CornerRadius::ZERO, colors::WIN_BLUE);
                                        }

                                        if is_editing && ci > 0 {
                                            if let Some((_, _, ref mut buf)) = self.editing_cell {
                                                let te_id =
                                                    ui.id().with(("ce_cell_te", open_gen, table_gen, r, ci));
                                                match show_curve_cell_text_edit(
                                                    ui,
                                                    cell_r,
                                                    te_id,
                                                    edit_init_id,
                                                    (table_gen, r, ci),
                                                    buf,
                                                    &mut self.editing_pristine,
                                                    cell_font_sz,
                                                ) {
                                                    CurveCellEditOutcome::Cancel => {
                                                        cancel_edit = true;
                                                    }
                                                    CurveCellEditOutcome::Commit(mv) => {
                                                        pending_commit =
                                                            Some((r, ci, buf.clone(), mv));
                                                    }
                                                    CurveCellEditOutcome::Continue => {}
                                                }
                                            }
                                        } else {
                                            let txt = self.format_step_cell(r, ci);
                                            let (align, pos, font, txt_col) = if ci == 0 {
                                                (
                                                    Align2::CENTER_CENTER,
                                                    cell_r.center(),
                                                    if is_row_sel {
                                                        bold_font(gutter_font_sz)
                                                    } else {
                                                        regular_font(gutter_font_sz)
                                                    },
                                                    if multi_sel {
                                                        Color32::from_rgb(0x1E, 0x3A, 0x8A)
                                                    } else {
                                                        colors::TEXT_SECONDARY
                                                    },
                                                )
                                            } else {
                                                (
                                                    Align2::RIGHT_CENTER,
                                                    pos2(cell_r.right() - 6.0, cell_r.center().y),
                                                    regular_font(cell_font_sz),
                                                    colors::TEXT_BLACK,
                                                )
                                            };
                                            if !txt.is_empty() {
                                                painter.text(pos, align, txt, font, txt_col);
                                            }

                                            let sense = if ci == 0 {
                                                Sense::click_and_drag()
                                            } else {
                                                Sense::click()
                                            };
                                            let resp = ui.interact(
                                                cell_r,
                                                ui.id().with(("ce_cell", open_gen, table_gen, r, ci)),
                                                sense,
                                            );

                                            if ci == 0 {
                                                if primary_pressed && resp.hovered() {
                                                    self.flush_active_edit();
                                                    if shift_down {
                                                        let anchor = self
                                                            .anchor_step
                                                            .or_else(|| self.selected_steps.first().copied())
                                                            .unwrap_or(r);
                                                        let (a, b) = if anchor <= r {
                                                            (anchor, r)
                                                        } else {
                                                            (r, anchor)
                                                        };
                                                        self.selected_steps = (a..=b).collect();
                                                        self.focused_cell = Some((r, 0));
                                                        self.row_drag_selecting = Some(anchor);
                                                    } else if ctrl_down {
                                                        self.toggle_step_selection(r);
                                                        self.focused_cell = Some((r, 0));
                                                    } else {
                                                        self.select_single(r);
                                                        self.focused_cell = Some((r, 0));
                                                        self.row_drag_selecting = Some(r);
                                                    }
                                                }
                                            } else if resp.clicked() {
                                                // Commit any previous edited cell before switching
                                                if let Some((er, ec, prev_buf)) = self.editing_cell.take() {
                                                    if (er, ec) != (r, ci) {
                                                        self.apply_cell_edit(er, ec, &prev_buf);
                                                    }
                                                }
                                                if shift_down {
                                                    let anchor = self
                                                        .anchor_step
                                                        .or_else(|| self.selected_steps.first().copied())
                                                        .unwrap_or(r);
                                                    let (a, b) = if anchor <= r {
                                                        (anchor, r)
                                                    } else {
                                                        (r, anchor)
                                                    };
                                                    self.selected_steps = (a..=b).collect();
                                                    self.focused_cell = Some((r, ci));
                                                } else if ctrl_down {
                                                    self.toggle_step_selection(r);
                                                    self.focused_cell = Some((r, ci));
                                                } else {
                                                    self.select_single(r);
                                                    self.focused_cell = Some((r, ci));
                                                    pending_start_edit =
                                                        Some((r, ci, self.format_step_cell(r, ci)));
                                                }
                                            }
                                        }

                                        if is_focused {
                                            painter.rect_stroke(
                                                cell_r.shrink(0.5),
                                                CornerRadius::ZERO,
                                                Stroke::new(2.0_f32, colors::CELL_FOCUS_BLUE),
                                                StrokeKind::Inside,
                                            );
                                        }

                                        cx += cw;
                                    }
                                }

                                let pending_scroll_id =
                                    ui.id().with(("curve_pending_scroll", open_gen));
                                if scroll_target.is_none() {
                                    scroll_target = ui.ctx().data_mut(|d| {
                                        let val = d.get_temp::<(usize, usize)>(pending_scroll_id);
                                        if val.is_some() {
                                            d.remove::<(usize, usize)>(pending_scroll_id);
                                        }
                                        val
                                    });
                                }
                                if let Some((sr, sc)) = scroll_target {
                                    let col_x: f32 = col_widths.iter().take(sc).sum();
                                    let col_w = col_widths.get(sc).copied().unwrap_or(44.0);
                                    let target_r = Rect::from_min_size(
                                        pos2(rows_rect.left() + col_x, rows_rect.top() + (sr as f32) * row_h),
                                        vec2(col_w, row_h),
                                    );
                                    ui.scroll_to_rect(target_r, None);
                                }
                            });
                    });
            },
        );

        if cancel_edit {
            self.editing_cell = None;
            self.editing_pristine = false;
            ui.ctx().data_mut(|d| d.remove::<(u64, usize, usize)>(edit_init_id));
        } else if let Some((row_i, col_i, new_val, move_kind)) = pending_commit {
            self.apply_cell_edit(row_i, col_i, &new_val);
            self.editing_cell = None;
            self.editing_pristine = false;
            ui.ctx().data_mut(|d| d.remove::<(u64, usize, usize)>(edit_init_id));
            if pending_start_edit.is_none() && row_count > 0 {
                let pending_scroll_id = ui.id().with(("curve_pending_scroll", open_gen));
                match move_kind {
                    CurveEditMove::Stay => {}
                    CurveEditMove::Tab(forward) => {
                        let (nr, nc) = next_curve_editable_cell(row_count, row_i, col_i, forward);
                        self.focused_cell = Some((nr, nc));
                        if nr != row_i {
                            self.select_single(nr);
                        }
                        ui.ctx().data_mut(|d| d.insert_temp(pending_scroll_id, (nr, nc)));
                    }
                    CurveEditMove::Delta(dr, dc) => {
                        let nr = (row_i as i32 + dr).clamp(0, (row_count - 1) as i32) as usize;
                        let nc = (col_i as i32 + dc).clamp(1, 7) as usize;
                        self.focused_cell = Some((nr, nc));
                        if nr != row_i {
                            self.select_single(nr);
                        }
                        ui.ctx().data_mut(|d| d.insert_temp(pending_scroll_id, (nr, nc)));
                    }
                }
            }
        }

        if let Some((sr, sc, init)) = pending_start_edit {
            self.editing_cell = Some((sr, sc, init));
            self.editing_pristine = true;
            ui.ctx().data_mut(|d| d.remove::<(u64, usize, usize)>(edit_init_id));
        }
    }

    fn handle_step_table_keyboard(&mut self, ui: &Ui) -> Option<(usize, usize)> {
        if self.editing_cell.is_some() || ui.ctx().wants_keyboard_input() || self.steps.is_empty() {
            return None;
        }
        let row_count = self.steps.len();
        let (r, c) = self.focused_cell.unwrap_or((0, 1));
        if r >= row_count || c > 7 {
            self.focused_cell = Some((r.min(row_count - 1), c.min(7)));
            return None;
        }

        let (ctrl_a, down, up, right, left, tab_fwd, tab_back, enter_f2_bs, typed_txt) =
            ui.ctx().input_mut(|i| {
                let none = egui::Modifiers::NONE;
                let shift = egui::Modifiers::SHIFT;
                let ctrl_a = i.consume_key(egui::Modifiers::CTRL, Key::A);
                let down = i.consume_key(none, Key::ArrowDown);
                let up = i.consume_key(none, Key::ArrowUp);
                let right = i.consume_key(none, Key::ArrowRight);
                let left = i.consume_key(none, Key::ArrowLeft);
                let tab_back = i.consume_key(shift, Key::Tab);
                let tab_fwd = !tab_back && i.consume_key(none, Key::Tab);
                let f2 = i.consume_key(none, Key::F2);
                let ent = i.consume_key(none, Key::Enter);
                let bs = i.consume_key(none, Key::Backspace);
                let enter_f2_bs = if bs {
                    Some(true)
                } else if f2 || ent {
                    Some(false)
                } else {
                    None
                };
                let typed = if !i.modifiers.ctrl && !i.modifiers.alt {
                    i.events.iter().find_map(|ev| match ev {
                        egui::Event::Text(txt) => {
                            let ch = txt.chars().next()?;
                            if ch.is_ascii_digit() || ch == '-' || ch == '+' {
                                Some(txt.clone())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                } else {
                    None
                };
                (ctrl_a, down, up, right, left, tab_fwd, tab_back, enter_f2_bs, typed)
            });

        let mut moved_to: Option<(usize, usize)> = None;
        if ctrl_a {
            self.selected_steps = (0..row_count).collect();
        } else if down {
            let nr = (r + 1).min(row_count - 1);
            self.focused_cell = Some((nr, c));
            self.select_single(nr);
            moved_to = Some((nr, c));
        } else if up {
            let nr = r.saturating_sub(1);
            self.focused_cell = Some((nr, c));
            self.select_single(nr);
            moved_to = Some((nr, c));
        } else if right {
            let nc = (c + 1).min(7);
            self.focused_cell = Some((r, nc));
            moved_to = Some((r, nc));
        } else if left {
            let nc = c.saturating_sub(1);
            self.focused_cell = Some((r, nc));
            moved_to = Some((r, nc));
        } else if tab_fwd || tab_back {
            let (nr, nc) = next_curve_editable_cell(row_count, r, c, tab_fwd);
            self.focused_cell = Some((nr, nc));
            if nr != r {
                self.select_single(nr);
            }
            moved_to = Some((nr, nc));
        } else if let Some(clear_init) = enter_f2_bs {
            let edit_col = if c == 0 { 1 } else { c };
            let init = if clear_init {
                String::new()
            } else {
                self.format_step_cell(r, edit_col)
            };
            self.focused_cell = Some((r, edit_col));
            self.editing_cell = Some((r, edit_col, init));
            self.editing_pristine = !clear_init;
        } else if let Some(txt) = typed_txt {
            let edit_col = if c == 0 { 1 } else { c };
            self.focused_cell = Some((r, edit_col));
            self.editing_cell = Some((r, edit_col, txt));
            self.editing_pristine = false;
        }
        moved_to
    }

    fn update_gutter_drag_selection(
        &mut self,
        ui: &Ui,
        rows_top: f32,
        rows_left: f32,
        row_h: f32,
        table_w: f32,
    ) -> Option<usize> {
        let anchor = self.row_drag_selecting?;
        let row_count = self.steps.len();
        if row_count == 0 || !ui.input(|i| i.pointer.primary_down()) {
            self.row_drag_selecting = None;
            return None;
        }
        let pos = ui.input(|i| i.pointer.interact_pos())?;
        if pos.x < rows_left - 24.0 || pos.x > rows_left + table_w + 24.0 {
            return None;
        }
        let hr = ((pos.y - rows_top) / row_h)
            .floor()
            .clamp(0.0, (row_count - 1) as f32) as usize;
        let anchor_clamped = anchor.min(row_count - 1);
        let (lo, hi) = if anchor_clamped <= hr {
            (anchor_clamped, hr)
        } else {
            (hr, anchor_clamped)
        };
        self.selected_steps = (lo..=hi).collect();
        self.focused_cell = Some((hr, 0));
        Some(hr)
    }

    fn show_closure_bar(&mut self, ui: &mut Ui, max_width: f32, bar_height: f32) {
        let (sum_dx, sum_dy) = curve_steps_sum_slice(&self.steps);
        let exact = sum_dx == self.target_dx && sum_dy == self.target_dy;

        let (bg, border, fg) = if exact {
            (
                Color32::from_rgb(0xE4, 0xF5, 0xE6),
                colors::STATUS_OK_GREEN,
                colors::STATUS_OK_GREEN,
            )
        } else {
            (
                Color32::from_rgb(0xFC, 0xE8, 0xE8),
                colors::STATUS_ERR_RED,
                colors::STATUS_ERR_RED,
            )
        };

        ui.allocate_ui_with_layout(
            vec2(max_width, bar_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Frame::NONE
                    .fill(bg)
                    .stroke(Stroke::new(1.0_f32, border))
                    .inner_margin(Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        let inner_w = ui.available_width().floor().max(200.0);
                        ui.set_width(inner_w);
                        ui.horizontal(|ui| {
                            // Procedurally paint status icon (green checkmark or red `!` triangle — zero missing font glyphs)
                            let (icon_r, _) =
                                ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            let p = ui.painter();
                            let c = icon_r.center();
                            if exact {
                                let st = Stroke::new(2.0_f32, fg);
                                p.line_segment([pos2(c.x - 4.5, c.y), pos2(c.x - 1.2, c.y + 3.5)], st);
                                p.line_segment([pos2(c.x - 1.2, c.y + 3.5), pos2(c.x + 5.0, c.y - 4.0)], st);
                            } else {
                                let st = Stroke::new(1.5_f32, fg);
                                p.line_segment([pos2(c.x, c.y - 5.0), pos2(c.x + 5.0, c.y + 4.5)], st);
                                p.line_segment([pos2(c.x + 5.0, c.y + 4.5), pos2(c.x - 5.0, c.y + 4.5)], st);
                                p.line_segment([pos2(c.x - 5.0, c.y + 4.5), pos2(c.x, c.y - 5.0)], st);
                                p.line_segment([pos2(c.x, c.y - 1.8), pos2(c.x, c.y + 1.4)], st);
                                p.circle_filled(pos2(c.x, c.y + 3.0), 0.9, fg);
                            }

                            let badge = if exact {
                                format!(
                                    "Tam kapanış: Toplam En = {sum_dx} (Hedef {})  |  Toplam Boy = {sum_dy} (Hedef {})",
                                    self.target_dx, self.target_dy
                                )
                            } else {
                                format!(
                                    "Fark: En {sum_dx}/{} ({:+}) | Boy {sum_dy}/{} ({:+})",
                                    self.target_dx,
                                    sum_dx - self.target_dx,
                                    self.target_dy,
                                    sum_dy - self.target_dy
                                )
                            };
                            if !exact {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("Hedefe uyarla")
                                            .on_hover_text(
                                                "Mevcut eğri adımlarını orantılı olarak hedef (N, R) değerine ölçekle",
                                            )
                                            .clicked()
                                        {
                                            let tmp = CurveData {
                                                function: self.curve_form.to_id(),
                                                steps: self.steps.clone(),
                                                raw: Default::default(),
                                            };
                                            let resampled = resample_user_curve(
                                                &tmp,
                                                self.target_dx,
                                                self.target_dy,
                                            );
                                            self.steps = resampled;
                                            self.renumber_local_steps();
                                            self.table_gen = self.table_gen.wrapping_add(1);
                                            self.has_chosen_formula = true;
                                            self.reverse_j_active = false;
                                        }
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(badge)
                                                            .font(bold_font(10.5))
                                                            .color(fg),
                                                    )
                                                    .truncate(),
                                                );
                                            },
                                        );
                                    },
                                );
                            } else {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(badge)
                                            .font(bold_font(10.5))
                                            .color(fg),
                                    )
                                    .truncate(),
                                );
                            }
                        });
                    });
            },
        );
    }

    fn show_curve_plot(&mut self, ui: &mut Ui, size: egui::Vec2) {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter().with_clip_rect(rect);

        // Light Studio drafting canvas `#F4F6FA`
        painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(0xF4, 0xF6, 0xFA));
        painter.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0_f32, colors::BORDER_DARK),
            StrokeKind::Inside,
        );

        // Zoom & Pan on plot (cursor-centered zoom + drag pan)
        let inner_probe = rect.shrink(28.0);
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let factor = (1.0 + scroll * 0.0025).clamp(0.75, 1.35);
                let old_zoom = self.plot_zoom;
                let new_zoom = (self.plot_zoom * factor).clamp(0.4, 12.0);
                if let Some(hp) = resp.hover_pos() {
                    let rel = hp - (inner_probe.center() + self.plot_pan);
                    let ratio = new_zoom / old_zoom;
                    self.plot_pan -= rel * (ratio - 1.0);
                }
                self.plot_zoom = new_zoom;
            }
        }
        if resp.dragged_by(PointerButton::Middle)
            || resp.dragged_by(PointerButton::Secondary)
            || resp.dragged_by(PointerButton::Primary)
        {
            self.plot_pan += resp.drag_delta();
        }
        if resp.double_clicked() {
            self.plot_zoom = 1.0;
            self.plot_pan = egui::Vec2::ZERO;
        }

        // Build per-step staircase polylines from `(0, 0)` (expanding Stoll `GruppenFaktor` blocks)
        let mut step_paths: Vec<(usize, Vec<(f64, f64)>)> = Vec::new();
        let mut cx = 0i32;
        let mut cy = 0i32;
        let mut min_x = 0.0_f64.min(self.target_dx as f64);
        let mut max_x = 0.0_f64.max(self.target_dx as f64);
        let mut min_y = 0.0_f64.min(self.target_dy as f64);
        let mut max_y = 0.0_f64.max(self.target_dy as f64);

        let mut i = 0usize;
        while i < self.steps.len() {
            let gid = self.steps[i].group_tag_id;
            let (run_end, group_reps) = if gid > 0 {
                let mut end = i + 1;
                while end < self.steps.len() && self.steps[end].group_tag_id == gid {
                    end += 1;
                }
                let gf = self.steps[i..end]
                    .iter()
                    .map(|s| s.group_factor)
                    .max()
                    .unwrap_or(1)
                    .max(1);
                (end, gf)
            } else {
                (i + 1, 1)
            };

            let draw_group_reps = group_reps.min(200);
            for _ in 0..draw_group_reps {
                for row_idx in i..run_end {
                    let s = &self.steps[row_idx];
                    let mut pts = vec![(cx as f64, cy as f64)];
                    let reps = s.factor.max(1);
                    if s.step_height == 0 || s.step_width == 0 {
                        cx += s.step_width * reps;
                        cy += s.step_height * reps;
                        pts.push((cx as f64, cy as f64));
                    } else {
                        let draw_reps = reps.min(500);
                        for _ in 0..draw_reps {
                            cy += s.step_height;
                            pts.push((cx as f64, cy as f64));
                            cx += s.step_width;
                            pts.push((cx as f64, cy as f64));
                        }
                        if reps > draw_reps {
                            let rem = reps - draw_reps;
                            cx += s.step_width * rem;
                            cy += s.step_height * rem;
                            pts.push((cx as f64, cy as f64));
                        }
                    }
                    for &(px, py) in &pts {
                        min_x = min_x.min(px);
                        max_x = max_x.max(px);
                        min_y = min_y.min(py);
                        max_y = max_y.max(py);
                    }
                    step_paths.push((row_idx, pts));
                }
            }
            if group_reps > draw_group_reps {
                let rem_g = group_reps - draw_group_reps;
                for row_idx in i..run_end {
                    let s = &self.steps[row_idx];
                    let pts_start = (cx as f64, cy as f64);
                    let reps = s.factor.max(1) * rem_g;
                    cx += s.step_width * reps;
                    cy += s.step_height * reps;
                    let pts_end = (cx as f64, cy as f64);
                    min_x = min_x.min(pts_end.0);
                    max_x = max_x.max(pts_end.0);
                    min_y = min_y.min(pts_end.1);
                    max_y = max_y.max(pts_end.1);
                    step_paths.push((row_idx, vec![pts_start, pts_end]));
                }
            }
            i = run_end;
        }

        let pad = 28.0;
        let inner = rect.shrink(pad);
        let span_x = (max_x - min_x).max(4.0);
        let span_y = (max_y - min_y).max(4.0);
        let base_scale = (inner.width() as f64 / span_x)
            .min(inner.height() as f64 / span_y)
            .max(1e-3);
        let scale = base_scale * (self.plot_zoom as f64);
        let wcx = (min_x + max_x) * 0.5;
        let wcy = (min_y + max_y) * 0.5;
        let scx = inner.center().x + self.plot_pan.x;
        let scy = inner.center().y + self.plot_pan.y;

        let to_screen = |wx: f64, wy: f64| -> Pos2 {
            pos2(
                scx + ((wx - wcx) * scale) as f32,
                scy - ((wy - wcy) * scale) as f32,
            )
        };

        // Subtle gridlines
        let g_step = ((span_x.max(span_y)) / 6.0).ceil().max(1.0);
        let gx0 = (min_x / g_step).floor() as i32;
        let gx1 = (max_x / g_step).ceil() as i32;
        let gy0 = (min_y / g_step).floor() as i32;
        let gy1 = (max_y / g_step).ceil() as i32;
        let grid_col = Color32::from_rgb(0xE4, 0xE9, 0xF2);
        for gx in gx0..=gx1 {
            let sx = to_screen(gx as f64 * g_step, 0.0).x;
            if sx >= rect.left() && sx <= rect.right() {
                painter.line_segment(
                    [pos2(sx, rect.top()), pos2(sx, rect.bottom())],
                    Stroke::new(1.0_f32, grid_col),
                );
            }
        }
        for gy in gy0..=gy1 {
            let sy = to_screen(0.0, gy as f64 * g_step).y;
            if sy >= rect.top() && sy <= rect.bottom() {
                painter.line_segment(
                    [pos2(rect.left(), sy), pos2(rect.right(), sy)],
                    Stroke::new(1.0_f32, grid_col),
                );
            }
        }

        // Target chord dashed line `(0,0) -> (target_dx, target_dy)`
        let p0 = to_screen(0.0, 0.0);
        let p1 = to_screen(self.target_dx as f64, self.target_dy as f64);
        draw_dashed_line(&painter, p0, p1, Stroke::new(1.0_f32, Color32::from_rgb(0x94, 0xA3, 0xB8)));

        // Origin & target markers
        painter.circle_filled(p0, 3.5, Color32::from_rgb(0x0F, 0x76, 0x6E));
        painter.circle_filled(p1, 4.0, colors::WIN_BLUE);

        // Hit-test & draw staircase steps
        let hover_pos = resp.hover_pos();
        let mut closest_step: Option<(f32, usize)> = None;

        for (step_idx, pts) in &step_paths {
            let spts: Vec<Pos2> = pts.iter().map(|&(x, y)| to_screen(x, y)).collect();
            if let Some(hp) = hover_pos {
                let d = dist_to_poly(hp, &spts);
                if d < 9.0 && closest_step.map(|(b, _)| d < b).unwrap_or(true) {
                    closest_step = Some((d, *step_idx));
                }
            }

            let is_sel = self.selected_steps.contains(step_idx);
            let is_hov = self.hovered_plot_step == Some(*step_idx);
            let (col, width): (Color32, f32) = if is_sel {
                (colors::WIN_BLUE, 3.0_f32)
            } else if is_hov {
                (colors::SELECTED_LINE_RED, 2.5_f32)
            } else {
                (Color32::from_rgb(0x0F, 0x76, 0x6E), 2.0_f32)
            };

            if spts.len() >= 2 {
                painter.add(egui::Shape::line(spts, Stroke::new(width, col)));
            }
        }

        self.hovered_plot_step = closest_step.map(|(_, idx)| idx);
        if resp.clicked() {
            if let Some((_, idx)) = closest_step {
                self.select_single(idx);
            }
        }

        // Top-left caption badge
        let title_txt = format!("{} ({} basamak)", self.active_curve_label(), self.steps.len());
        painter.text(
            pos2(rect.left() + 6.0, rect.top() + 5.0),
            Align2::LEFT_TOP,
            title_txt,
            bold_font(10.5),
            colors::TEXT_BLACK,
        );

        // Bottom-left endpoint readout
        painter.text(
            pos2(rect.left() + 6.0, rect.bottom() - 5.0),
            Align2::LEFT_BOTTOM,
            format!("(0, 0) -> ({}, {})", self.target_dx, self.target_dy),
            regular_font(10.0),
            colors::TEXT_SECONDARY,
        );
    }

    fn parity_mode(&self, side: LineSide) -> CourseParityMode {
        if self.single_system {
            match side {
                LineSide::Left => CourseParityMode::SingleSystemLeftOdd,
                LineSide::Right => CourseParityMode::SingleSystemRightEven,
            }
        } else if self.even_courses {
            CourseParityMode::EvenCourses
        } else {
            CourseParityMode::Any
        }
    }

    fn step_drive(&self) -> CurveStepDrive {
        match self.step_axis_mode {
            StepAxisMode::Width => CurveStepDrive::ByWidth,
            StepAxisMode::Height => CurveStepDrive::ByHeight,
        }
    }

    fn regenerate_formula(&mut self, side: LineSide) {
        let (nw, ww, fw) = self.default_widths;
        let sx = if self.target_dx < 0 {
            -self.step_x_mag
        } else {
            self.step_x_mag
        };
        let sy = if self.target_dy < 0 {
            -self.step_y_mag
        } else {
            self.step_y_mag
        };
        let params = CurveGenParams {
            curve_form: if self.reverse_j_active {
                CurveForm::JCurve
            } else {
                self.curve_form
            },
            total_dx: self.target_dx,
            total_dy: self.target_dy,
            step_x: sx,
            step_y: sy,
            narrowing_width: nw,
            widening_width: ww,
            fade_out_width: fw,
            parity_mode: self.parity_mode(side),
            step_drive: self.step_drive(),
            reverse_j: self.reverse_j_active,
            min_step_height: self.min_step_height,
        };
        let feas = check_curve_feasibility(&params);
        if !feas.feasible {
            self.notice = feas.warning_tr.map(|w| (w, true));
        } else {
            self.notice = None;
        }
        self.steps = generate_curve_steps(&params);
        self.table_gen = self.table_gen.wrapping_add(1);
        self.selected_steps.clear();
        if !self.steps.is_empty() {
            self.selected_steps.push(0);
            self.anchor_step = Some(0);
            self.focused_cell = Some((0, 1));
        } else {
            self.anchor_step = None;
            self.focused_cell = None;
        }
    }

    fn new_default_step(&self) -> CurveStep {
        let (nw, ww, fw) = self.default_widths;
        CurveStep {
            step_width: if self.target_dx < 0 {
                -self.step_x_mag
            } else {
                self.step_x_mag
            },
            step_height: 2,
            factor: 1,
            narrowing_width: nw,
            widening_width: ww,
            fade_out_width: fw,
            ..CurveStep::default()
        }
    }

    fn renumber_local_steps(&mut self) {
        for (i, s) in self.steps.iter_mut().enumerate() {
            s.index = (i + 1) as i32;
            s.tag_id = s.index;
            if s.factor < 1 {
                s.factor = 1;
            }
            s.raw.original_header = Some(format!("[KurvenLinie] = {}", s.index));
        }
    }

    fn commit_to_doc(
        &mut self,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
        req: CurveEditorRequest,
    ) {
        if !self.has_pending_changes() {
            return;
        }
        undo.push("Çizgi editörü", doc);
        let hdr = doc.header.clone();
        let endline_dists: Vec<(usize, f64)> = (0..doc.elements.len())
            .filter(|&ei| doc.elements[ei].y_ref_line == YReferenceLine::EndLine)
            .map(|ei| (ei, element_distance_from_top(doc, ei).0))
            .collect();
        if let Some(el) = doc.elements.get_mut(req.elem_idx) {
            let elem_type = el.element_type();
            let line_count = el.lines(req.side).len();
            let is_last_row = req.line_idx > 0 && req.line_idx + 1 == line_count;
            let sum_x_others: i32 = el
                .lines(req.side)
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != req.line_idx)
                .map(|(_, l)| l.stitches)
                .sum();
            if let Some(line) = el.lines_mut(req.side).get_mut(req.line_idx) {
                line.single_system_curve = self.single_system;
                if self.steps.is_empty() {
                    line.curve = None;
                    line.kurven_form = 0;
                    line.child_order
                        .retain(|s| !matches!(s, KanteChildSlot::Curve));
                    line.stitches = self.target_dx;
                    line.rows = self.target_dy;
                    recalc_line_from_stitches_and_rows(line, &hdr);
                } else {
                    normalize_curve_groups(&mut self.steps);
                    let save_form = self.effective_stoll_curve_form();
                    apply_curve_to_line(line, &hdr, save_form, self.steps.clone());
                    let is_closing = is_last_row
                        && line.rows == 0
                        && (line.default_widening == 1 || sum_x_others + line.stitches == 0);
                    auto_detect_line_function_in_context(
                        line,
                        req.line_idx == 0,
                        is_closing,
                        req.side == LineSide::Left,
                        elem_type,
                    );
                    self.target_dx = line.stitches;
                    self.target_dy = line.rows;
                }
                doc.dirty = true;
            }
            auto_sync_closing_line_for_side(doc, req.elem_idx, req.side);
            if req.side == LineSide::Left && doc.elements[req.elem_idx].mirrored {
                sync_mirrored_right_lines(doc, req.elem_idx);
            }
        }
        for (ei, dist_top) in endline_dists {
            sync_endline_element_y(doc, ei, dist_top);
        }
        self.orig_form = self.effective_stoll_curve_form();
        self.orig_steps = self.steps.clone();
        self.orig_single_system = self.single_system;
        self.had_initial_curve = !self.steps.is_empty();
        state.set_status(
            format!(
                "Eğri güncellendi (Satır {}, {} - {} basamak).",
                req.line_idx + 1,
                self.active_curve_label(),
                self.steps.len()
            ),
            false,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurveTbIcon {
    LoadCurve,
    SaveCurve,
    Line,
    JCurve,
    SCurve,
    ReverseJCurve,
    UserCurve,
    FlipVertical,
    FlipHorizontal,
    InsertBefore,
    Append,
    DeleteSelected,
    ClearAll,
    Copy,
    Paste,
    Group,
    Props,
    ZoomFit,
}

fn tb_icon_btn(ui: &mut Ui, icon: CurveTbIcon, tooltip: &str, enabled: bool, active: bool) -> bool {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), sense);
    let bg = if active {
        colors::WIN_BLUE_FILL
    } else if enabled && resp.hovered() {
        colors::WIN_BLUE_HOVER
    } else if enabled {
        Color32::from_rgb(0xE1, 0xE1, 0xE1)
    } else {
        Color32::from_rgb(0xE8, 0xE8, 0xE8)
    };
    let border = if active || (enabled && resp.hovered()) {
        colors::WIN_BLUE
    } else {
        colors::BORDER_MID
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, bg);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    // Use exact original Stoll `formtable.dll` PE bitmap icons wherever available (`#48.6`)
    if let Some(stoll_icon) = match icon {
        CurveTbIcon::LoadCurve => Some(IconKind::CurveLoad),
        CurveTbIcon::SaveCurve => Some(IconKind::CurveSave),
        CurveTbIcon::Line => Some(IconKind::CurveLine),
        CurveTbIcon::JCurve => Some(IconKind::CurveJ),
        CurveTbIcon::ReverseJCurve => Some(IconKind::CurveReverseJ),
        CurveTbIcon::SCurve => Some(IconKind::CurveS),
        CurveTbIcon::UserCurve => Some(IconKind::CurveUser),
        CurveTbIcon::InsertBefore => Some(IconKind::CurveInsert),
        CurveTbIcon::Append => Some(IconKind::CurveAppend),
        CurveTbIcon::DeleteSelected => Some(IconKind::CurveDelete),
        CurveTbIcon::Copy => Some(IconKind::CurveCopy),
        CurveTbIcon::Paste => Some(IconKind::CurvePaste),
        CurveTbIcon::Group => Some(IconKind::CurveGroup),
        _ => None,
    } {
        paint_icon(painter, rect.shrink(2.0), stoll_icon, enabled);
    } else {
        paint_curve_tb_icon(painter, rect.shrink(4.0), icon, enabled, active);
    }
    let resp = resp.on_hover_text(tooltip);
    enabled && resp.clicked()
}

fn paint_curve_tb_icon(
    painter: &egui::Painter,
    r: Rect,
    icon: CurveTbIcon,
    enabled: bool,
    active: bool,
) {
    let dim = |c: Color32| -> Color32 {
        if enabled {
            c
        } else {
            Color32::from_rgb(0x98, 0x98, 0x98)
        }
    };
    let blk = dim(Color32::from_rgb(0x1E, 0x1E, 0x1E));
    let blu = dim(if active {
        colors::CURVE_GLYPH_BLUE
    } else {
        Color32::from_rgb(0x00, 0x45, 0xC8)
    });
    let red = dim(Color32::from_rgb(0xC8, 0x18, 0x18));
    let grn = dim(Color32::from_rgb(0x10, 0x82, 0x24));
    let c = r.center();

    match icon {
        CurveTbIcon::FlipVertical => {
            // Vertical flip `↕` (up & down arrows)
            let st = Stroke::new(1.4_f32, blk);
            let lx = c.x - 3.2;
            painter.line_segment([pos2(lx, r.bottom() - 1.0), pos2(lx, r.top() + 1.5)], st);
            painter.line_segment([pos2(lx, r.top() + 1.5), pos2(lx - 2.3, r.top() + 4.2)], st);
            painter.line_segment([pos2(lx, r.top() + 1.5), pos2(lx + 2.3, r.top() + 4.2)], st);
            let rx = c.x + 3.2;
            painter.line_segment([pos2(rx, r.top() + 1.0), pos2(rx, r.bottom() - 1.5)], st);
            painter.line_segment([pos2(rx, r.bottom() - 1.5), pos2(rx - 2.3, r.bottom() - 4.2)], st);
            painter.line_segment([pos2(rx, r.bottom() - 1.5), pos2(rx + 2.3, r.bottom() - 4.2)], st);
        }
        CurveTbIcon::FlipHorizontal => {
            // Horizontal flip `↔` (left & right arrows)
            let st = Stroke::new(1.4_f32, blk);
            let ty = c.y - 3.2;
            painter.line_segment([pos2(r.right() - 1.0, ty), pos2(r.left() + 1.5, ty)], st);
            painter.line_segment([pos2(r.left() + 1.5, ty), pos2(r.left() + 4.2, ty - 2.3)], st);
            painter.line_segment([pos2(r.left() + 1.5, ty), pos2(r.left() + 4.2, ty + 2.3)], st);
            let by = c.y + 3.2;
            painter.line_segment([pos2(r.left() + 1.0, by), pos2(r.right() - 1.5, by)], st);
            painter.line_segment([pos2(r.right() - 1.5, by), pos2(r.right() - 4.2, by - 2.3)], st);
            painter.line_segment([pos2(r.right() - 1.5, by), pos2(r.right() - 4.2, by + 2.3)], st);
        }
        CurveTbIcon::ClearAll => {
            // Stacked rows with red strike-through X
            for dy in [-3.5_f32, 0.0, 3.5] {
                painter.line_segment(
                    [pos2(r.left() + 1.0, c.y + dy), pos2(r.right() - 1.0, c.y + dy)],
                    Stroke::new(1.0_f32, blk),
                );
            }
            let st = Stroke::new(1.6_f32, red);
            painter.line_segment([pos2(r.left() + 2.0, r.top() + 1.5), pos2(r.right() - 2.0, r.bottom() - 1.5)], st);
            painter.line_segment([pos2(r.right() - 2.0, r.top() + 1.5), pos2(r.left() + 2.0, r.bottom() - 1.5)], st);
        }
        CurveTbIcon::Props => {
            // Classic sliders / properties card
            painter.rect_filled(r.shrink(1.0), CornerRadius::ZERO, Color32::WHITE);
            painter.rect_stroke(r.shrink(1.0), CornerRadius::ZERO, Stroke::new(1.0_f32, blk), StrokeKind::Inside);
            painter.line_segment([pos2(r.left() + 3.0, c.y - 2.5), pos2(r.right() - 3.0, c.y - 2.5)], Stroke::new(1.2_f32, blu));
            painter.line_segment([pos2(r.left() + 3.0, c.y + 1.5), pos2(r.right() - 3.0, c.y + 1.5)], Stroke::new(1.2_f32, blk));
            painter.circle_filled(pos2(c.x + 1.5, c.y - 2.5), 1.6, blu);
            painter.circle_filled(pos2(c.x - 1.5, c.y + 1.5), 1.6, blk);
        }
        CurveTbIcon::ZoomFit => {
            let st = Stroke::new(1.4_f32, blk);
            // Four outward corner brackets
            let d = 3.5;
            painter.line_segment([r.left_top(), pos2(r.left() + d, r.top())], st);
            painter.line_segment([r.left_top(), pos2(r.left(), r.top() + d)], st);
            painter.line_segment([r.right_top(), pos2(r.right() - d, r.top())], st);
            painter.line_segment([r.right_top(), pos2(r.right(), r.top() + d)], st);
            painter.line_segment([r.left_bottom(), pos2(r.left() + d, r.bottom())], st);
            painter.line_segment([r.left_bottom(), pos2(r.left(), r.bottom() - d)], st);
            painter.line_segment([r.right_bottom(), pos2(r.right() - d, r.bottom())], st);
            painter.line_segment([r.right_bottom(), pos2(r.right(), r.bottom() - d)], st);
            painter.circle_stroke(c, 2.2, Stroke::new(1.2_f32, blu));
        }
        CurveTbIcon::LoadCurve
        | CurveTbIcon::SaveCurve
        | CurveTbIcon::Line
        | CurveTbIcon::JCurve
        | CurveTbIcon::ReverseJCurve
        | CurveTbIcon::SCurve
        | CurveTbIcon::UserCurve
        | CurveTbIcon::InsertBefore
        | CurveTbIcon::Append
        | CurveTbIcon::DeleteSelected
        | CurveTbIcon::Copy
        | CurveTbIcon::Paste
        | CurveTbIcon::Group => {
            // Handled via `paint_icon` above; fallback keeps match exhaustive without dead code.
            let _ = grn;
        }
    }
}

/// Mutually exclusive step-axis control: radio toggle + `[-]` + editable numeric input + `[+]`.
fn curve_step_axis_spinner(
    ui: &mut Ui,
    active_mode: &mut StepAxisMode,
    my_mode: StepAxisMode,
    label: &str,
    edit_id: (&'static str, u64),
    val: &mut i32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        if win32_radio(ui, active_mode, my_mode, label).changed() {
            changed = true;
        }
        let enabled = *active_mode == my_mode;
        if win32_button(ui, "-", vec2(18.0, 20.0), enabled && *val > 1).clicked() {
            *val = (*val - 1).clamp(1, 99);
            changed = true;
        }
        if let Some(txt) = win32_numeric_text_edit(ui, edit_id, val.to_string(), 34.0, enabled) {
            if let Ok(parsed) = txt.trim().parse::<i32>() {
                let clamped = parsed.clamp(1, 99);
                if clamped != *val {
                    *val = clamped;
                    changed = true;
                }
            }
        }
        if win32_button(ui, "+", vec2(18.0, 20.0), enabled && *val < 99).clicked() {
            *val = (*val + 1).clamp(1, 99);
            changed = true;
        }
    });
    changed
}

#[derive(Clone, Copy)]
enum CurveEditMove {
    Stay,
    Delta(i32, i32),
    Tab(bool),
}

enum CurveEditNavAction {
    None,
    Cancel,
    Move(CurveEditMove),
}

enum CurveCellEditOutcome {
    Continue,
    Cancel,
    Commit(CurveEditMove),
}

fn next_curve_editable_cell(
    row_count: usize,
    cur_r: usize,
    cur_c: usize,
    forward: bool,
) -> (usize, usize) {
    if row_count == 0 {
        return (cur_r, cur_c);
    }
    let mut r = cur_r.min(row_count - 1);
    let mut c = cur_c.clamp(0, 7);
    if forward {
        if c < 7 {
            c = (c + 1).max(1);
            return (r, c);
        }
        if r + 1 < row_count {
            r += 1;
            return (r, 1);
        }
    } else {
        if c > 1 {
            return (r, c - 1);
        }
        if r > 0 {
            r -= 1;
            return (r, 7);
        }
    }
    (cur_r, cur_c)
}

fn show_curve_cell_text_edit(
    ui: &mut Ui,
    cell_r: Rect,
    te_id: egui::Id,
    edit_init_id: egui::Id,
    cell_key: (u64, usize, usize),
    buf: &mut String,
    pristine: &mut bool,
    font_sz: f32,
) -> CurveCellEditOutcome {
    let already_inited = ui
        .ctx()
        .data(|d| d.get_temp::<(u64, usize, usize)>(edit_init_id))
        == Some(cell_key);

    let nav = if already_inited {
        consume_curve_text_edit_nav_keys(ui.ctx(), te_id, buf)
    } else {
        CurveEditNavAction::None
    };

    match nav {
        CurveEditNavAction::Cancel => return CurveCellEditOutcome::Cancel,
        CurveEditNavAction::Move(mv) => return CurveCellEditOutcome::Commit(mv),
        CurveEditNavAction::None => {}
    }

    if !already_inited {
        ui.ctx().memory_mut(|m| m.request_focus(te_id));
        let mut pre_state = egui::TextEdit::load_state(ui.ctx(), te_id).unwrap_or_default();
        let len = buf.chars().count();
        pre_state.cursor.set_char_range(Some(
            egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(len),
            ),
        ));
        pre_state.store(ui.ctx(), te_id);
    } else if ui.input(|i| {
        i.key_pressed(Key::ArrowLeft)
            || i.key_pressed(Key::ArrowRight)
            || i.key_pressed(Key::Home)
            || i.key_pressed(Key::End)
            || i.key_pressed(Key::Backspace)
            || i.key_pressed(Key::Delete)
    }) {
        *pristine = false;
    }

    let prev_buf = buf.clone();
    let edit_rect = cell_r.shrink(1.0);
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
    let te = egui::TextEdit::singleline(buf)
        .id(te_id)
        .font(regular_font(font_sz))
        .horizontal_align(egui::Align::RIGHT)
        .frame(false)
        .desired_width(edit_rect.width());
    let output = te.show(&mut child_ui);
    let resp = output.response;
    let mut te_state = output.state;

    if apply_pristine_zero_guard(&prev_buf, buf, pristine) {
        let len = buf.chars().count();
        te_state.cursor.set_char_range(Some(
            egui::text::CCursorRange::one(egui::text::CCursor::new(len)),
        ));
        te_state.clone().store(ui.ctx(), resp.id);
    } else if *buf != prev_buf {
        *pristine = false;
    }

    if !already_inited {
        resp.request_focus();
        if *pristine {
            let len = buf.chars().count();
            te_state.cursor.set_char_range(Some(
                egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                ),
            ));
            te_state.store(ui.ctx(), resp.id);
        }
        ui.ctx().data_mut(|d| d.insert_temp(edit_init_id, cell_key));
    } else if resp.lost_focus() {
        return CurveCellEditOutcome::Commit(CurveEditMove::Stay);
    }

    CurveCellEditOutcome::Continue
}

fn consume_curve_text_edit_nav_keys(
    ctx: &egui::Context,
    te_id: egui::Id,
    buf: &str,
) -> CurveEditNavAction {
    let char_len = buf.chars().count();
    let (at_start, at_end) = if let Some(te_state) = egui::TextEdit::load_state(ctx, te_id) {
        if let Some(range) = te_state.cursor.char_range() {
            let p = range.primary.index;
            let s = range.secondary.index;
            (p == 0 && s == 0, p == char_len && s == char_len)
        } else {
            (true, true)
        }
    } else {
        (true, true)
    };

    ctx.input_mut(|i| {
        let none = egui::Modifiers::NONE;
        let shift = egui::Modifiers::SHIFT;
        if i.consume_key(none, Key::Escape) {
            return CurveEditNavAction::Cancel;
        }
        if i.consume_key(none, Key::Enter) || i.consume_key(shift, Key::Enter) {
            return CurveEditNavAction::Move(CurveEditMove::Delta(1, 0));
        }
        if i.consume_key(none, Key::ArrowUp) {
            return CurveEditNavAction::Move(CurveEditMove::Delta(-1, 0));
        }
        if i.consume_key(none, Key::ArrowDown) {
            return CurveEditNavAction::Move(CurveEditMove::Delta(1, 0));
        }
        if i.consume_key(shift, Key::Tab) {
            return CurveEditNavAction::Move(CurveEditMove::Tab(false));
        }
        if i.consume_key(none, Key::Tab) {
            return CurveEditNavAction::Move(CurveEditMove::Tab(true));
        }
        if at_start && i.consume_key(none, Key::ArrowLeft) {
            return CurveEditNavAction::Move(CurveEditMove::Delta(0, -1));
        }
        if at_end && i.consume_key(none, Key::ArrowRight) {
            return CurveEditNavAction::Move(CurveEditMove::Delta(0, 1));
        }
        CurveEditNavAction::None
    })
}

fn paint_win32_curve_titlebar(ui: &mut Ui, title: &str) -> bool {
    let w = ui.available_width().floor().max(240.0);
    let (bar_rect, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(bar_rect, CornerRadius::ZERO, Color32::WHITE);
    painter.line_segment(
        [bar_rect.left_bottom(), bar_rect.right_bottom()],
        Stroke::new(1.0_f32, Color32::from_rgb(0xDC, 0xDC, 0xDC)),
    );

    let close_rect = Rect::from_min_max(
        pos2(bar_rect.right() - 34.0, bar_rect.top()),
        pos2(bar_rect.right(), bar_rect.bottom() - 1.0),
    );
    let close_resp = ui.interact(close_rect, ui.id().with("curve_close_x"), Sense::click());
    let hov = close_resp.hovered();
    if hov {
        painter.rect_filled(close_rect, CornerRadius::ZERO, Color32::from_rgb(0xE8, 0x11, 0x23));
    }
    let xc = close_rect.center();
    let x_st = Stroke::new(1.1_f32, if hov { Color32::WHITE } else { colors::TEXT_BLACK });
    painter.line_segment([pos2(xc.x - 4.5, xc.y - 4.5), pos2(xc.x + 4.5, xc.y + 4.5)], x_st);
    painter.line_segment([pos2(xc.x - 4.5, xc.y + 4.5), pos2(xc.x + 4.5, xc.y - 4.5)], x_st);

    let text_clip = Rect::from_min_max(
        pos2(bar_rect.left() + 8.0, bar_rect.top()),
        pos2(close_rect.left() - 6.0, bar_rect.bottom()),
    );
    painter.with_clip_rect(text_clip).text(
        pos2(text_clip.left(), bar_rect.center().y),
        Align2::LEFT_CENTER,
        title,
        regular_font(11.5),
        colors::TEXT_BLACK,
    );
    close_resp.clicked()
}

fn draw_dashed_line(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    let delta = b - a;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let dir = delta / len;
    let dash = 5.0;
    let gap = 4.0;
    let mut t = 0.0;
    while t < len {
        let t2 = (t + dash).min(len);
        painter.line_segment([a + dir * t, a + dir * t2], stroke);
        t += dash + gap;
    }
}

fn dist_to_poly(p: Pos2, pts: &[Pos2]) -> f32 {
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        let ab = w[1] - w[0];
        let lsq = ab.length_sq();
        let d = if lsq < 1e-6 {
            p.distance(w[0])
        } else {
            let t = ((p - w[0]).dot(ab) / lsq).clamp(0.0, 1.0);
            p.distance(w[0] + ab * t)
        };
        if d < best {
            best = d;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_j_curve_closes_exactly_and_is_steep_then_gentle() {
        for (dx, dy, sx) in [(20, 60, 2), (-20, 60, -2), (17, 45, 3), (8, 40, 1)] {
            let steps = generate_curve_steps(&CurveGenParams {
                curve_form: CurveForm::JCurve,
                total_dx: dx,
                total_dy: dy,
                step_x: sx,
                narrowing_width: 6,
                reverse_j: true,
                ..Default::default()
            });
            let data = CurveData {
                function: CurveForm::UserCurve.to_id(),
                steps: steps.clone(),
                raw: Default::default(),
            };
            assert_eq!(curve_steps_sum_slice(&data.steps), (dx, dy), "closure failed for ({dx},{dy},{sx})");
            assert!(
                steps.first().unwrap().step_height >= steps.last().unwrap().step_height,
                "reverse J must start steeper than it ends for ({dx},{dy},{sx}): {steps:?}"
            );
        }
    }

    #[test]
    fn curve_editor_window_does_not_grow_across_frames() {
        let ctx = egui::Context::default();
        crate::ui::theme::configure_fonts(&ctx);
        crate::ui::theme::apply_win32_theme(&ctx);
        let mut doc = FormDocument::new_default_template();
        // Add a basic shape with a curved narrowing line
        stoll_kalip_editoru::math::add_element(&mut doc, stoll_kalip_editoru::model::ElementType::BasicShape, None);
        let steps = generate_curve_steps(&CurveGenParams {
            curve_form: CurveForm::JCurve,
            total_dx: 20,
            total_dy: 60,
            step_x: 2,
            ..Default::default()
        });
        if let Some(el) = doc.elements.get_mut(0) {
            let mut seg = stoll_kalip_editoru::model::LineSegment::default();
            seg.stitches = 20;
            seg.rows = 60;
            stoll_kalip_editoru::math::apply_curve_to_line(&mut seg, &doc.header, CurveForm::JCurve, steps);
            el.left_lines.push(seg);
        }
        let mut undo = UndoStack::default();
        let mut state = UiState::default();
        state.curve_editor_req = Some(CurveEditorRequest {
            elem_idx: 0,
            side: LineSide::Left,
            line_idx: doc.elements[0].left_lines.len() - 1,
        });
        let mut dlg = CurveEditorDialogState::default();

        let mut sizes: Vec<egui::Vec2> = Vec::new();
        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 860.0))),
            ..Default::default()
        };
        for _ in 0..25 {
            let _ = ctx.run(raw_input.clone(), |ctx| {
                dlg.show(ctx, &mut doc, &mut undo, &mut state);
            });
            if let Some(area) = ctx.memory(|m| m.area_rect(egui::Id::new("curve_editor_modal"))) {
                sizes.push(area.size());
            }
        }
        assert!(sizes.len() >= 20);
        let first_stable = sizes[2];
        for (frame_idx, sz) in sizes.iter().enumerate().skip(2) {
            assert!(
                (sz.x - first_stable.x).abs() < 0.5 && (sz.y - first_stable.y).abs() < 0.5,
                "window grew on frame {frame_idx}: first_stable={first_stable:?}, got={sz:?}, all={sizes:?}"
            );
        }
    }

    #[test]
    fn unselected_initial_preset_slash_commit_and_cift_may_reopen() {
        let ctx = egui::Context::default();
        crate::ui::theme::configure_fonts(&ctx);
        crate::ui::theme::apply_win32_theme(&ctx);

        let mut doc = FormDocument::new_default_template();
        let hdr = doc.header.clone();
        let mut seg = stoll_kalip_editoru::model::LineSegment {
            index: 1,
            tag_id: 1,
            stitches: 10,
            rows: 30,
            function: LineFunction::Narrowing,
            ..Default::default()
        };
        recalc_line_from_stitches_and_rows(&mut seg, &hdr);
        doc.elements[0].left_lines.push(seg);

        let mut undo = UndoStack::default();
        let mut state = UiState::default();
        state.user_settings.default_even_courses = false;
        let req = CurveEditorRequest {
            elem_idx: 0,
            side: LineSide::Left,
            line_idx: 0,
        };
        state.curve_editor_req = Some(req);

        let mut dlg = CurveEditorDialogState::default();
        let raw_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 860.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw_input.clone(), |ctx| {
            dlg.show(ctx, &mut doc, &mut undo, &mut state);
        });

        // 1. Opening on a line with NO [Kurve]: preset is unselected
        assert!(!dlg.has_chosen_formula);
        assert!(!dlg.had_initial_curve);
        // Committing without choosing a formula must NOT attach a curve
        dlg.commit_to_doc(&mut doc, &mut undo, &mut state, req);
        assert!(doc.elements[0].left_lines[0].curve.is_none());

        // 2a. User chooses plain `/` (`CurveForm::Line`, even_courses = false, step_x_mag = 1) -> commit saves Line (1)
        dlg.has_chosen_formula = true;
        dlg.curve_form = CurveForm::Line;
        dlg.even_courses = false;
        dlg.step_x_mag = 1;
        dlg.regenerate_formula(LineSide::Left);
        dlg.commit_to_doc(&mut doc, &mut undo, &mut state, req);

        let plain_curve = doc.elements[0].left_lines[0]
            .curve
            .as_ref()
            .expect("clicking '/' and Tamam must commit [Kurve]");
        assert_eq!(plain_curve.function, CurveForm::Line.to_id());

        // 2b. User chooses `J` with `Çift May` and `step_x_mag = 2` -> commit saves UserCurve (5) so Stoll vecformeditor preserves exact steps
        dlg.curve_form = CurveForm::JCurve;
        dlg.even_courses = true;
        dlg.step_x_mag = 2;
        dlg.regenerate_formula(LineSide::Left);
        dlg.commit_to_doc(&mut doc, &mut undo, &mut state, req);

        let saved_curve = doc.elements[0].left_lines[0]
            .curve
            .as_ref()
            .expect("custom-stepped curve must be committed");
        assert_eq!(saved_curve.function, CurveForm::UserCurve.to_id());
        assert!(saved_curve.steps.iter().all(|s| s.step_height % 2 == 0));
        assert!(saved_curve.steps.iter().all(|s| s.step_width.abs() == 2));

        // 3. Reopening a fresh dialog with default_even_courses = false auto-detects `Çift May` = true and UserCurve
        let mut dlg2 = CurveEditorDialogState::default();
        state.curve_editor_req = Some(req);
        let _ = ctx.run(raw_input, |ctx| {
            dlg2.show(ctx, &mut doc, &mut undo, &mut state);
        });
        assert!(dlg2.has_chosen_formula);
        assert!(dlg2.had_initial_curve);
        assert!(dlg2.even_courses, "Çift May must auto-detect on reopen from even step heights");
        assert_eq!(dlg2.curve_form, CurveForm::UserCurve);
    }
}
