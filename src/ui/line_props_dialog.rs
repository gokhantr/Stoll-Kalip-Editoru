use std::collections::{HashMap, HashSet};

use egui::{
    pos2, vec2, Align2, Color32, ColorImage, CornerRadius, Frame, Margin, Rect, Sense, Stroke,
    StrokeKind, TextureHandle, TextureOptions, Ui,
};

use stoll_kalip_editoru::catalog::{
    load_stoll_module_from_db, KnittingModuleInfo, ModuleCategory, StollCatalog,
    KNITTING_STRUCTURES,
};
use stoll_kalip_editoru::math::{
    apply_standard_preset_to_line, auto_sync_closing_lines, sync_mirrored_right_lines,
};
use stoll_kalip_editoru::model::{
    ConnectPoint, FormDocument, KanteChildSlot, LineFunction, LineSegment, LineSide,
    ModuleAssignment, NarrowingProps, PointEntry, PointKind, PointOffset, ShapeCutType, UndoStack,
};

use crate::ui::icons::{paint_icon, IconKind};
use crate::ui::theme::{
    bold_font, colors, regular_font, win32_button, win32_checkbox, win32_combo_scope,
    win32_group_box_on, win32_numeric_text_edit, win32_radio,
};
use crate::ui::{
    centered_modal_pos, modal_shadow, show_modal_backdrop, LinePropsRequest, UiState,
};

const TAB_PANE_BG: Color32 = Color32::from_rgb(0xF9, 0xF9, 0xF9);

const ZERO_GUID: &str = "{00000000-0000-0000-0000-000000000000}";
/// In Stoll `.shp`/`.shv`, `FuerAlleStrickarten = 2147483647` (`INT_MAX`) means "per-structure" (checkbox unchecked),
/// while `0..12` is the index of the single structure applied to all knitting structures (checkbox checked).
const ALL_STRUCTURES_NONE: i64 = 2_147_483_647;

#[inline]
fn is_for_all_structures(val: i64) -> bool {
    (0..12).contains(&val)
}

#[inline]
fn encode_for_all_structures(enabled: bool, struct_idx: usize) -> i64 {
    if enabled {
        (struct_idx.min(11)) as i64
    } else {
        ALL_STRUCTURES_NONE
    }
}

const TAB_LABELS: [&str; 8] = [
    "Genel",
    "Reglan",
    "Artırma",
    "Remayözleme",
    "Kenar düzeltme",
    "Başlama",
    "Bitiş",
    "Bağlantılar",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewCategory {
    Narrowing,
    NarrowingMultistage,
    Widening,
    BindOff,
    BindOffCa,
    FadeOut,
    PointStart,
    PointStartVneck,
    PointEnd,
}

impl PreviewCategory {
    fn to_module_category(self) -> ModuleCategory {
        match self {
            Self::Narrowing => ModuleCategory::Narrowing,
            Self::NarrowingMultistage => ModuleCategory::NarrowingMultistage,
            Self::Widening => ModuleCategory::Widening,
            Self::BindOff => ModuleCategory::BindOff,
            Self::BindOffCa => ModuleCategory::BindOffCa,
            Self::FadeOut => ModuleCategory::FadeOut,
            Self::PointStart => ModuleCategory::Start,
            Self::PointStartVneck => ModuleCategory::StartVneck,
            Self::PointEnd => ModuleCategory::End,
        }
    }
}

/// Modal 8-tab Line Properties Dialog (`M3_LinePropSheet`, `DIALOG 4266`–`4273`,
/// Screenshots `222026.png`–`222125.png`).
pub struct LinePropsDialogState {
    active_req: Option<LinePropsRequest>,
    /// Snapshot of original `LineSegment` to track dirty state for `Uygula` enablement (`DLG-09`).
    original: LineSegment,
    /// Working clone of the target `LineSegment` so `İptal` discards all edits cleanly.
    working: LineSegment,
    /// Active tab `0..=7`.
    active_tab: usize,
    /// Selected knitting-structure row (`0..12`) for each of the 6 module-bearing tabs
    /// (`0=Reglan, 1=Artırma, 2=Remayözleme, 3=Kenar düzeltme, 4=Başlama, 5=Bitiş`).
    selected_struct: [usize; 6],
    /// Optional delayed-narrowing flag (`Reglanı gecikmeli uygula` — unchecked by default matching `222038.png`).
    reglan_delayed: bool,
    /// Multi-stage narrowing mode (`çok kademeli`, `15677`) — switches Tab 1 to `Eksiltme M`.
    reglan_multistage: bool,
    /// Show all narrowing models across all structures (`Tüm modelleri göster`, `15681`).
    show_all_models: bool,
    /// K&W sleeve checkbox (`15675`) that disables `Genişlik` when checked.
    kw_sleeve_no_width: bool,
    /// Selected `Remayözleme-tipi` index (`0 = Modül`, `1 = RA`).
    bind_off_type_idx: usize,
    /// Selected connection point row on Tab 7 (`Bağlantılar`).
    selected_conn_idx: Option<usize>,
    /// Contextual help popup toggle (`Yardım` button).
    help_popup_open: bool,
    /// Cached resolved Stoll DB modules (so `&StollCatalog` suffices without `&mut`).
    resolved_cache: HashMap<String, Option<KnittingModuleInfo>>,
    /// Cached GPU textures for decoded `M3_UTL_Bitmap` 128×128 thumbnails keyed by uppercase GUID.
    texture_cache: HashMap<String, Option<TextureHandle>>,
    /// Cached dropdown choices keyed by `(category, structure_id, norm_guid, ext_count)`.
    cached_cat_choices: Option<((PreviewCategory, i32, String, usize), Vec<(String, String)>)>,
}

impl Default for LinePropsDialogState {
    fn default() -> Self {
        Self {
            active_req: None,
            original: LineSegment::default(),
            working: LineSegment::default(),
            active_tab: 0,
            selected_struct: [0; 6],
            reglan_delayed: false,
            reglan_multistage: false,
            show_all_models: false,
            kw_sleeve_no_width: false,
            bind_off_type_idx: 0,
            selected_conn_idx: None,
            help_popup_open: false,
            resolved_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            cached_cat_choices: None,
        }
    }
}

impl LinePropsDialogState {
    /// Render the 8-tab modal dialog (`Sol çizgiler No.: N` / `Sağ çizgiler No.: N`)
    /// when `state.line_props_req` is `Some`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
        catalog: &StollCatalog,
    ) {
        let Some(req) = state.line_props_req else {
            self.active_req = None;
            self.help_popup_open = false;
            return;
        };

        let is_kw = ShapeCutType::from_id(doc.header.cut_type) != ShapeCutType::Standard;
        let elem_art_id = doc
            .elements
            .get(req.elem_idx)
            .map(|e| e.art_tag_id)
            .unwrap_or(1);
        let is_base_line = doc
            .elements
            .get(req.elem_idx)
            .map(|e| e.is_base_line_row(req.side, req.line_idx))
            .unwrap_or(false);

        // Initialize working clone whenever a new request opens (preserving active tab if switching rows while open!)
        if self.active_req != Some(req) {
            let was_already_open = self.active_req.is_some();
            let prev_tab = self.active_tab;
            if let Some(line) = doc
                .elements
                .get(req.elem_idx)
                .and_then(|e| e.lines(req.side).get(req.line_idx))
            {
                self.original = line.clone();
                self.working = line.clone();
                self.active_req = Some(req);
                let prev_tab_enabled = match prev_tab {
                    0 | 4 | 5 | 6 => true,
                    1 | 2 | 3 => !is_base_line,
                    7 => is_kw,
                    _ => false,
                };
                self.active_tab = if was_already_open && prev_tab_enabled {
                    prev_tab
                } else if req.initial_tab == 7 && !is_kw {
                    0
                } else {
                    req.initial_tab.min(7)
                };
                self.selected_struct = [0; 6];
                self.reglan_delayed = line
                    .narrowing
                    .as_ref()
                    .and_then(|n| n.raw.get_raw("VerzoegertAusfuehren"))
                    .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "ja" | "true"))
                    .unwrap_or(false);
                self.reglan_multistage = line.narrowing.as_ref().is_some_and(|n| {
                    n.modules.iter().any(|m| {
                        m.fair_isle_status != 0 || catalog.is_narrowing_multistage_guid(&m.id)
                    })
                });
                self.show_all_models = false;
                self.kw_sleeve_no_width = false;

                let cur_bo_guid = line
                    .binding_off
                    .as_ref()
                    .map(|b| {
                        let fa = is_for_all_structures(b.for_all_structures);
                        let sid = if fa { b.for_all_structures as i32 } else { 0 };
                        get_or_default_module(&b.modules, sid, fa).id
                    })
                    .unwrap_or_default();
                self.bind_off_type_idx = catalog.bind_off_type_for_guid(&cur_bo_guid);

                self.selected_conn_idx = self
                    .working
                    .connection_points
                    .as_ref()
                    .and_then(|cp| if cp.points.is_empty() { None } else { Some(0) });
                self.help_popup_open = false;
            } else {
                state.line_props_req = None;
                self.active_req = None;
                return;
            }
        }

        let side_lbl = match req.side {
            LineSide::Left => "Sol çizgiler",
            LineSide::Right => "Sağ çizgiler",
        };
        let title = format!("{side_lbl} No.: {}", req.line_idx + 1);

        // External local modules embedded in this document's `[Form]` header
        let doc_ext_modules: Vec<(String, String)> = doc
            .header
            .external_local_modules
            .iter()
            .filter_map(|ext| {
                let g = normalize_guid(&ext.guid);
                if !g.is_empty() && g != ZERO_GUID {
                    Some((g, ext.name.clone()))
                } else {
                    None
                }
            })
            .collect();

        let mut do_apply = false;
        let mut do_close = false;
        let mut open_window = true;

        // Keyboard shortcuts (`DLG-09`): Esc = İptal, Enter (when not typing in a TextEdit) = Tamam
        if !self.help_popup_open && !ctx.wants_keyboard_input() {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    do_close = true;
                } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                    do_apply = true;
                    do_close = true;
                }
            });
        }

        // Keep table rows bright & clickable while LinePropsDialog is open so user can switch rows live!
        let def_pos = if state.curve_editor_req.is_some() {
            let sr = ctx.screen_rect();
            let base = centered_modal_pos(ctx, vec2(560.0, 480.0));
            pos2((base.x + 36.0).min(sr.right() - 570.0).max(sr.left() + 8.0), (base.y + 20.0).min(sr.bottom() - 490.0).max(sr.top() + 8.0))
        } else {
            centered_modal_pos(ctx, vec2(560.0, 480.0))
        };

        egui::Window::new(&title)
            .id(egui::Id::new("line_props_modal"))
            .title_bar(false)
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .default_pos(def_pos)
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .stroke(Stroke::new(1.5_f32, colors::BORDER_DARK))
                    .shadow(modal_shadow())
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_min_width(552.0);
                ui.set_max_width(566.0);

                // Classic Win32 white `#FFFFFF` titlebar (`222026.png`–`222125.png`, `DLG-01`)
                if paint_win32_dialog_titlebar(ui, &title, "lp_close_x") {
                    open_window = false;
                }

                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        // ── 1. Classic Win32 8-Tab Strip (`222026.png` – `222125.png`) ──
                        self.show_tab_strip(ui, is_kw, is_base_line);

                        // ── 2. Active Tab Body inside classic raised white/light tab pane ─
                        Frame::NONE
                            .fill(TAB_PANE_BG)
                            .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                            .inner_margin(Margin {
                                left: 10,
                                right: 10,
                                top: 10,
                                bottom: 10,
                            })
                            .show(ui, |ui| {
                                ui.set_min_width(518.0);
                                ui.set_min_height(378.0);

                                match self.active_tab {
                                    0 => self.show_tab_genel(ui, is_kw, elem_art_id, is_base_line),
                                    1 => self.show_tab_reglan(
                                        ui,
                                        ctx,
                                        catalog,
                                        &doc_ext_modules,
                                        is_kw,
                                        elem_art_id,
                                    ),
                                    2 => self.show_tab_artirma(ui, ctx, catalog, &doc_ext_modules, is_kw),
                                    3 => self.show_tab_remayozleme(ui, ctx, catalog, &doc_ext_modules, is_kw),
                                    4 => self.show_tab_kenar_duzeltme(ui, ctx, catalog, &doc_ext_modules),
                                    5 => self.show_tab_point(
                                        ui,
                                        ctx,
                                        catalog,
                                        &doc_ext_modules,
                                        0,
                                        PreviewCategory::PointStart,
                                        elem_art_id,
                                    ),
                                    6 => self.show_tab_point(
                                        ui,
                                        ctx,
                                        catalog,
                                        &doc_ext_modules,
                                        1,
                                        PreviewCategory::PointEnd,
                                        elem_art_id,
                                    ),
                                    7 => self.show_tab_baglantilar(ui),
                                    _ => {}
                                }
                            });

                        ui.add_space(8.0);

                        // Compute dirty AFTER tab body so focus-loss numeric commits enable `Uygula` immediately!
                        let is_dirty = self.working != self.original;

                        // ── 3. Footer Buttons: Tamam | İptal | Uygula | Yardım ──────────
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;

                            if win32_button(ui, "Yardım", vec2(72.0, 23.0), true).clicked() {
                                self.help_popup_open = !self.help_popup_open;
                            }
                            ui.add_space(10.0);
                            if win32_button(ui, "Uygula", vec2(72.0, 23.0), is_dirty).clicked() {
                                do_apply = true;
                            }
                            if win32_button(ui, "İptal", vec2(72.0, 23.0), true).clicked() {
                                do_close = true;
                            }
                            let ok_btn = egui::Button::new("Tamam")
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.5_f32, colors::WIN_BLUE))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(72.0, 23.0), ok_btn).clicked() {
                                do_apply = true;
                                do_close = true;
                            }
                        });
                    });
            });

        if !open_window {
            do_close = true;
        }

        // Contextual Help Popup
        if self.help_popup_open {
            let mut h_open = self.help_popup_open;
            let help_pos = centered_modal_pos(ctx, vec2(360.0, 180.0));
            egui::Window::new(format!("Yardım — {}", TAB_LABELS[self.active_tab]))
                .id(egui::Id::new("line_props_help_popup"))
                .open(&mut h_open)
                .collapsible(false)
                .movable(true)
                .resizable(false)
                .default_pos(pos2(help_pos.x + 30.0, help_pos.y + 30.0))
                .frame(
                    Frame::NONE
                        .fill(colors::CARD_BG)
                        .stroke(Stroke::new(1.5_f32, colors::BORDER_DARK))
                        .shadow(modal_shadow())
                        .inner_margin(Margin::same(10)),
                )
                .show(ctx, |ui| {
                    ui.set_min_width(340.0);
                    ui.label(egui::RichText::new(TAB_LABELS[self.active_tab]).font(bold_font(12.0)));
                    ui.add_space(4.0);
                    ui.label(tab_help_text_tr(self.active_tab));
                    ui.add_space(8.0);
                    if ui.button("Kapat").clicked() {
                        self.help_popup_open = false;
                    }
                });
            if !h_open {
                self.help_popup_open = false;
            }
        }

        if do_apply {
            if self.working != self.original {
                undo.push("Çizgi özellikleri", doc);
                // Ensure any newly created line-attached `[Punkt]` gets a unique `PunktTagId`
                for pt in &mut self.working.points {
                    if pt.punkt_tag_id <= 0 {
                        pt.punkt_tag_id = doc.next_point_tag_id();
                    }
                }
                if let Some(el) = doc.elements.get_mut(req.elem_idx) {
                    if let Some(target) = el.lines_mut(req.side).get_mut(req.line_idx) {
                        *target = self.working.clone();
                        doc.dirty = true;
                    }
                    if req.side == LineSide::Left && el.mirrored {
                        sync_mirrored_right_lines(doc, req.elem_idx);
                    }
                }
                auto_sync_closing_lines(doc);
                self.original = self.working.clone();
                state.set_status(
                    format!("{side_lbl} No.: {} güncellendi.", req.line_idx + 1),
                    false,
                );
            }
        }

        if do_close {
            state.line_props_req = None;
            self.active_req = None;
            self.help_popup_open = false;
        }
    }

    /// Determine whether a tab is enabled/active for the current `working.function`,
    /// and whether it shows the Stoll active-function checkmark `✓` (`formtable.dll` `0x1007dc38..0x1007dd11`, `0x1007ce60`).
    fn tab_visual_state(&self, tab_idx: usize, is_kw: bool, is_base_line: bool) -> (bool, bool) {
        let f = self.working.function;
        match tab_idx {
            0 | 4 | 5 | 6 => (true, false),
            1 => (
                !is_base_line,
                matches!(f, LineFunction::Narrowing | LineFunction::NarrowingMulti),
            ),
            2 => (!is_base_line, f == LineFunction::Widening),
            3 => (!is_base_line, f == LineFunction::BindOff),
            7 => (is_kw, false),
            _ => (true, false),
        }
    }

    fn switch_to_tab(&mut self, idx: usize, is_base_line: bool) {
        self.active_tab = idx;
        if is_base_line {
            return;
        }
        match idx {
            1 => {
                if !matches!(
                    self.working.function,
                    LineFunction::Narrowing | LineFunction::NarrowingMulti
                ) {
                    self.working.function = LineFunction::Narrowing;
                    self.working.user_set_function = true;
                }
                self.working.narrowing.get_or_insert_with(Default::default);
                self.working.ensure_child_slot(KanteChildSlot::Narrowing);
            }
            2 => {
                if self.working.function != LineFunction::Widening {
                    self.working.function = LineFunction::Widening;
                    self.working.user_set_function = true;
                }
                self.working.widening.get_or_insert_with(Default::default);
                self.working.ensure_child_slot(KanteChildSlot::Widening);
            }
            3 => {
                let keep_narrowing_bindoff = matches!(
                    self.working.function,
                    LineFunction::Narrowing | LineFunction::NarrowingMulti
                ) && self.working.with_bind_off == Some(1);
                if !keep_narrowing_bindoff && self.working.function != LineFunction::BindOff {
                    self.working.function = LineFunction::BindOff;
                    self.working.user_set_function = true;
                }
                self.working.binding_off.get_or_insert_with(Default::default);
                self.working.ensure_child_slot(KanteChildSlot::BindingOff);
            }
            _ => {}
        }
    }

    fn show_function_selector(
        &mut self,
        ui: &mut Ui,
        id_salt: &str,
        width: f32,
        is_kw: bool,
        is_base_line: bool,
    ) {
        let cur_lbl = self.working.function.label_tr();
        ui.add_enabled_ui(!is_base_line, |ui| {
            win32_combo_scope(ui, |ui| {
                egui::ComboBox::from_id_salt(id_salt)
                    .width(width)
                    .selected_text(if cur_lbl.is_empty() { "(Yok)" } else { cur_lbl })
                    .show_ui(ui, |ui| {
                        let mut funcs: Vec<(LineFunction, &str)> = vec![
                            (LineFunction::None, "(Yok)"),
                            (LineFunction::Narrowing, "Reglan"),
                            (LineFunction::Widening, "Artırma"),
                            (LineFunction::BindOff, "Remayözleme"),
                            (LineFunction::FadeOut, "Kenar düzeltimi"),
                        ];
                        if is_kw {
                            funcs.push((LineFunction::NarrowingMulti, "Reglan M"));
                        }
                        for (func, lbl) in funcs {
                            if ui
                                .selectable_value(&mut self.working.function, func, lbl)
                                .changed()
                            {
                                self.working.user_set_function = true;
                                match func {
                                    LineFunction::Narrowing | LineFunction::NarrowingMulti => {
                                        self.working.narrowing.get_or_insert_with(Default::default);
                                        self.working.ensure_child_slot(KanteChildSlot::Narrowing);
                                    }
                                    LineFunction::Widening => {
                                        self.working.widening.get_or_insert_with(Default::default);
                                        self.working.ensure_child_slot(KanteChildSlot::Widening);
                                    }
                                    LineFunction::BindOff => {
                                        self.working.binding_off.get_or_insert_with(Default::default);
                                        self.working.ensure_child_slot(KanteChildSlot::BindingOff);
                                    }
                                    LineFunction::FadeOut => {
                                        self.working.fade_out.get_or_insert_with(Default::default);
                                        self.working.ensure_child_slot(KanteChildSlot::FadeOut);
                                    }
                                    _ => {}
                                }
                                if (1..=3).contains(&self.active_tab) {
                                    self.active_tab = match func {
                                        LineFunction::Narrowing | LineFunction::NarrowingMulti => 1,
                                        LineFunction::Widening => 2,
                                        LineFunction::BindOff => 3,
                                        LineFunction::FadeOut => 4,
                                        _ => 0,
                                    };
                                }
                            }
                        }
                    });
            });
        });
    }

    fn show_tab_strip(&mut self, ui: &mut Ui, is_kw: bool, is_base_line: bool) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            for (idx, &default_label) in TAB_LABELS.iter().enumerate() {
                let label = if idx == 1 && self.reglan_multistage {
                    "Eksiltme M"
                } else {
                    default_label
                };
                let is_selected = self.active_tab == idx;
                let (is_enabled_visual, has_check) =
                    self.tab_visual_state(idx, is_kw, is_base_line);

                let text_galley = ui.painter().layout_no_wrap(
                    label.to_owned(),
                    regular_font(11.0),
                    colors::TEXT_BLACK,
                );
                let check_w = if has_check { 14.0 } else { 0.0 };
                let tab_w = (text_galley.size().x + 14.0 + check_w).max(40.0);
                let tab_h = if is_selected { 23.0 } else { 21.0 };

                let sense = if is_enabled_visual {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (rect, resp) = ui.allocate_exact_size(vec2(tab_w, tab_h), sense);
                let draw_rect = if is_selected {
                    rect
                } else {
                    Rect::from_min_max(pos2(rect.left(), rect.top() + 2.0), rect.right_bottom())
                };

                let bg = if is_selected {
                    Color32::from_rgb(0xF9, 0xF9, 0xF9)
                } else if resp.hovered() && is_enabled_visual {
                    colors::WIN_BLUE_HOVER
                } else {
                    Color32::from_rgb(0xEC, 0xEC, 0xEC)
                };

                let painter = ui.painter();
                painter.rect_filled(draw_rect, CornerRadius::ZERO, bg);
                // Top, left, right border
                let stroke = Stroke::new(1.0_f32, colors::BORDER_MID);
                painter.line_segment([draw_rect.left_bottom(), draw_rect.left_top()], stroke);
                painter.line_segment([draw_rect.left_top(), draw_rect.right_top()], stroke);
                painter.line_segment([draw_rect.right_top(), draw_rect.right_bottom()], stroke);
                if !is_selected {
                    painter.line_segment(
                        [draw_rect.left_bottom(), draw_rect.right_bottom()],
                        stroke,
                    );
                }

                // Checkmark icon for active function tab (`222038`, `222054`, `222102`)
                let mut text_x = draw_rect.left() + 7.0;
                if has_check {
                    let cy = draw_rect.center().y;
                    let cx = draw_rect.left() + 5.0;
                    let chk_stroke = Stroke::new(1.6_f32, colors::TEXT_BLACK);
                    painter.line_segment(
                        [pos2(cx, cy), pos2(cx + 2.8, cy + 3.0)],
                        chk_stroke,
                    );
                    painter.line_segment(
                        [pos2(cx + 2.8, cy + 3.0), pos2(cx + 7.5, cy - 3.5)],
                        chk_stroke,
                    );
                    text_x += 12.0;
                }

                let text_col = if is_selected {
                    Color32::from_rgb(0x00, 0x4E, 0xB8)
                } else if is_enabled_visual {
                    colors::TEXT_BLACK
                } else {
                    colors::TEXT_DISABLED
                };

                painter.text(
                    pos2(text_x, draw_rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    regular_font(11.0),
                    text_col,
                );

                if resp.clicked() && is_enabled_visual {
                    self.switch_to_tab(idx, is_base_line);
                }
            }
        });
    }

    // ── Tab 0: Genel (`DIALOG 4266`, Screenshot `222026.png`) ────────────────────
    fn show_tab_genel(&mut self, ui: &mut Ui, is_kw: bool, elem_art_id: i32, is_base_line: bool) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Fonksiyon:");
            ui.add_space(34.0);
            self.show_function_selector(ui, "lp_genel_func", 136.0, is_kw, is_base_line);
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(14.0);

        ui.horizontal_top(|ui| {
            // Left group: `kalıb kenarında ilmek uzunluğu` (`222026.png`)
            ui.vertical(|ui| {
                ui.set_max_width(248.0);
                win32_group_box_on(ui, "kalıb kenarında ilmek uzunluğu", TAB_PANE_BG, |ui| {
                    egui::Grid::new("lp_genel_np_grid")
                        .num_columns(3)
                        .spacing(vec2(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.label("");
                            ui.label("Endeks");
                            ui.label("Değer");
                            ui.end_row();

                            // NP ön (0 -> >0 auto-defaults to 12.0 = 1200 per Stoll 0x1006cce0)
                            ui.label("NP ön:");
                            let prev_front_idx = self.working.ml_np_front_index;
                            np_index_combo(ui, "lp_np_front_idx", &mut self.working.ml_np_front_index);
                            if prev_front_idx <= 0
                                && self.working.ml_np_front_index > 0
                                && self.working.ml_np_front == 0
                            {
                                self.working.ml_np_front = 1200;
                            }
                            np_tenths_edit(ui, "lp_np_front_val", &mut self.working.ml_np_front);
                            ui.end_row();

                            // NP arka (0 -> >0 auto-defaults to 12.0 = 1200 per Stoll 0x1006cd60)
                            ui.label("NP arka:");
                            let prev_back_idx = self.working.ml_np_back_index;
                            np_index_combo(ui, "lp_np_back_idx", &mut self.working.ml_np_back_index);
                            if prev_back_idx <= 0
                                && self.working.ml_np_back_index > 0
                                && self.working.ml_np_back == 0
                            {
                                self.working.ml_np_back = 1200;
                            }
                            np_tenths_edit(ui, "lp_np_back_val", &mut self.working.ml_np_back);
                            ui.end_row();

                            ui.label("");
                            ui.label("");
                            ui.label("");
                            ui.end_row();

                            // Genişlik
                            ui.label("Genişlik:");
                            win32_int_edit(
                                ui,
                                "lp_genel_ml_width",
                                &mut self.working.ml_width,
                                50.0,
                                0..=999,
                            );
                            ui.label("");
                            ui.end_row();
                        });
                });
            });

            // Optional `İkinci ayar (NPS)` group (`15655..15658`)
            if self.working.second_stitch_length
                || self.working.ml_npz_front != 0
                || self.working.ml_npz_back != 0
            {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.set_max_width(235.0);
                    win32_group_box_on(ui, "İkinci ayar (NPS)", TAB_PANE_BG, |ui| {
                        ui.add_enabled_ui(self.working.second_stitch_length, |ui| {
                            egui::Grid::new("lp_genel_nps_grid")
                                .num_columns(2)
                                .spacing(vec2(8.0, 6.0))
                                .show(ui, |ui| {
                                    ui.label("Ön:");
                                    np_tenths_edit(
                                        ui,
                                        "lp_npz_front_val",
                                        &mut self.working.ml_npz_front,
                                    );
                                    ui.end_row();
                                    ui.label("Arka:");
                                    np_tenths_edit(
                                        ui,
                                        "lp_npz_back_val",
                                        &mut self.working.ml_npz_back,
                                    );
                                    ui.end_row();
                                });
                        });
                    });
                });
            }
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            let mut dont_knit = self.working.change_attrib != 0;
            let can_dont_knit = is_kw && elem_art_id == 1;
            ui.add_enabled_ui(can_dont_knit, |ui| {
                if win32_checkbox(ui, &mut dont_knit, "Sakın Örme").changed() {
                    self.working.change_attrib = if dont_knit { 1 } else { 0 };
                }
            });
            ui.add_space(48.0);
            win32_checkbox(
                ui,
                &mut self.working.second_stitch_length,
                "İkinci ayar (NPS)",
            );
        });
    }

    // ── Tab 1: Reglan / Eksiltme M (`DIALOG 4267`, Screenshot `222038.png`) ──────
    fn show_tab_reglan(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        choices: &[(String, String)],
        is_kw: bool,
        elem_art_id: i32,
    ) {
        let mut props = self.working.narrowing.clone().unwrap_or_default();
        let before_props = props.clone();

        let mut is_multi = self.reglan_multistage;
        let prev_multi = is_multi;
        let for_all = if is_multi {
            false
        } else {
            is_for_all_structures(props.for_all_structures)
        };
        let struct_id = if is_multi {
            0
        } else if for_all {
            props.for_all_structures as i32
        } else {
            self.selected_struct[0] as i32
        };

        let mut width = get_or_default_module(&props.modules, struct_id, for_all).width;
        let mut bo_limit =
            get_or_default_module(&props.modules, struct_id, for_all).bind_off_limit;
        let mut width_changed = false;
        let mut jump_to_bindoff = false;

        // Top row (hidden in `çok kademeli` mode per Stoll 0x10070590)
        if !is_multi {
            let is_kw_sleeve = is_kw && elem_art_id == 3;
            let width_enabled = !(is_kw_sleeve && self.kw_sleeve_no_width);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(width_enabled, |ui| {
                    ui.label("Genişlik:");
                    ui.add_space(6.0);
                    if win32_int_edit(ui, "lp_reglan_width", &mut width, 40.0, 0..=999) {
                        let m = get_or_insert_module(&mut props.modules, struct_id);
                        m.width = width;
                        width_changed = true;
                    }
                });
                ui.add_space(16.0);
                ui.label("Basamaktan sonra Remayözleme:");
                if win32_int_edit(ui, "lp_reglan_bo_limit", &mut bo_limit, 40.0, 0..=999) {
                    let m = get_or_insert_module(&mut props.modules, struct_id);
                    m.bind_off_limit = bo_limit;
                    self.working.with_bind_off = Some(if bo_limit > 0 { 1 } else { 0 });
                }
                ui.add_space(6.0);
                if win32_button(ui, "Remayözleme >>", vec2(96.0, 21.0), true).clicked() {
                    jump_to_bindoff = true;
                }
            });
        }

        // Sync narrowing width to curve steps if a curve exists (`DLG-04` / Kural 45)
        if width_changed {
            if let Some(ref mut curve) = self.working.curve {
                for st in &mut curve.steps {
                    st.narrowing_width = width;
                }
            }
        }

        ui.add_space(4.0);

        // GroupBox `Genel` (`222038.png`) with crisp vertical & horizontal dividers (`DLG-04`)
        let mut delayed_flag = self.reglan_delayed;
        let mut delayed_toggled = false;
        win32_group_box_on(ui, "Genel", TAB_PANE_BG, |ui| {
            let group_top = ui.cursor().top();
            let avail_w = ui.available_width();
            let left_w = (avail_w * 0.56).max(250.0);
            let right_w = (avail_w - left_w - 14.0).max(160.0);
            let mut div_x = ui.cursor().left() + left_w + 5.0;
            let resp_h = ui.horizontal_top(|ui| {
                // Left column (hidden in `çok kademeli` mode)
                ui.vertical(|ui| {
                    ui.set_width(left_w);
                    if !is_multi {
                        if win32_checkbox(ui, &mut delayed_flag, "Reglanı gecikmeli uygula")
                            .changed()
                        {
                            delayed_toggled = true;
                            if delayed_flag {
                                props.execute_immediately = false;
                            }
                        }
                        if win32_checkbox(
                            ui,
                            &mut props.execute_immediately,
                            "Reglanı hemen uygula",
                        )
                        .changed()
                        {
                            if props.execute_immediately {
                                if delayed_flag {
                                    delayed_toggled = true;
                                }
                                delayed_flag = false;
                            }
                        }
                        if is_kw && elem_art_id == 3 {
                            win32_checkbox(
                                ui,
                                &mut self.kw_sleeve_no_width,
                                "Kol reglan genişliği yok",
                            );
                        }
                        ui.add_space(3.0);
                        ui.separator();
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            win32_int_edit(
                                ui,
                                "lp_reglan_stitch_cnt",
                                &mut props.stitch_count,
                                36.0,
                                0..=99,
                            );
                            ui.label(
                                egui::RichText::new("İlmek sayısına, omuz Remayözleme uygula")
                                    .font(regular_font(10.5)),
                            );
                        });
                    } else {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Çok kademeli reglan modu etkin.")
                                .font(regular_font(10.5))
                                .color(colors::TEXT_SECONDARY),
                        );
                    }
                });

                div_x = ui.cursor().left() + 4.0;
                ui.add_space(10.0);

                // Right column
                ui.vertical(|ui| {
                    ui.set_width(right_w);
                    win32_checkbox(ui, &mut is_multi, "çok kademeli");
                    ui.add_space(if is_multi { 4.0 } else { 21.0 });
                    ui.separator();
                    ui.add_space(3.0);
                    win32_checkbox(ui, &mut props.use_module_color, "Modül rengi kullan");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Fonksiyon:");
                        self.show_function_selector(ui, "lp_reglan_fn", 110.0, is_kw, false);
                    });
                });
            });
            // Crisp vertical divider matching `222038.png`
            let group_bot = resp_h.response.rect.bottom();
            ui.painter().line_segment(
                [pos2(div_x, group_top), pos2(div_x, group_bot)],
                Stroke::new(1.0_f32, Color32::from_rgb(0xDC, 0xDC, 0xDC)),
            );
        });
        self.reglan_delayed = delayed_flag;
        if delayed_toggled {
            set_narrowing_delayed_raw(&mut props, delayed_flag);
        }
        self.reglan_multistage = is_multi;
        let multi_toggled = is_multi != prev_multi;

        ui.add_space(4.0);

        // GroupBox `Modül düzeni` (`222038.png`)
        let mut for_all_flag = if is_multi {
            false
        } else {
            is_for_all_structures(props.for_all_structures)
        };
        let mut cur_struct = if is_multi {
            0
        } else if for_all_flag {
            props.for_all_structures as usize
        } else {
            self.selected_struct[0]
        };
        let active_mod = get_or_default_module(&props.modules, cur_struct as i32, for_all_flag);
        let mut nar_pos = active_mod.narrowing_position;
        let mut fair_isle = active_mod.fair_isle_status;
        let mut cur_guid = active_mod.id;
        let mut nar_pos_changed = false;
        let mut fair_isle_changed = false;
        let mut guid_changed = false;

        if multi_toggled {
            if is_multi {
                if fair_isle != 1 && fair_isle != 2 {
                    fair_isle = 1;
                    fair_isle_changed = true;
                }
                if !catalog.is_narrowing_multistage_guid(&cur_guid) {
                    if let Some(first) = catalog
                        .modules_for_category_and_structure(ModuleCategory::NarrowingMultistage, 0)
                        .first()
                    {
                        cur_guid = first.guid.clone();
                        guid_changed = true;
                    }
                }
            } else {
                if fair_isle != 0 {
                    fair_isle = 0;
                    fair_isle_changed = true;
                }
                if catalog.is_narrowing_multistage_guid(&cur_guid) {
                    if let Some(first) = catalog
                        .modules_for_category_and_structure(ModuleCategory::Narrowing, cur_struct as i32)
                        .first()
                    {
                        cur_guid = first.guid.clone();
                        guid_changed = true;
                    }
                }
            }
        }

        let reglan_cat = if is_multi {
            PreviewCategory::NarrowingMultistage
        } else {
            PreviewCategory::Narrowing
        };
        win32_group_box_on(ui, "Modül düzeni", TAB_PANE_BG, |ui| {
            if !is_multi {
                // Stoll `DIALOG 4267`: radios map (before, with, after) -> (1, 4, 2)
                let mut pos_mode = match nar_pos {
                    1 => 1,
                    2 => 2,
                    _ => 4,
                };
                let p_changed = win32_radio(
                    ui,
                    &mut pos_mode,
                    1,
                    "Reglanı mevcut aktarmadan önce uygula",
                )
                .changed()
                    | win32_radio(
                        ui,
                        &mut pos_mode,
                        4,
                        "Reglanı mevcut aktarmayla beraber uygula",
                    )
                    .changed()
                    | win32_radio(
                        ui,
                        &mut pos_mode,
                        2,
                        "Reglanı mevcut aktarmadan sonra uygula",
                    )
                    .changed();
                if p_changed {
                    nar_pos = pos_mode;
                    nar_pos_changed = true;
                }
            } else {
                // `çok kademeli` (`DIALOG 4267` 15678 / 15679): Altta (1) / Üstte (2)
                let mut fi_mode = if fair_isle == 2 { 2 } else { 1 };
                ui.horizontal(|ui| {
                    let fi_changed = win32_radio(ui, &mut fi_mode, 1, "Altta").changed()
                        | win32_radio(ui, &mut fi_mode, 2, "Üstte").changed();
                    if fi_changed || fair_isle != fi_mode {
                        fair_isle = fi_mode;
                        fair_isle_changed = true;
                    }
                });
            }

            ui.add_space(6.0);

            let mut show_all_tmp = self.show_all_models;
            let show_all_opt = if !is_multi && !for_all_flag {
                Some(&mut show_all_tmp)
            } else {
                None
            };
            guid_changed |= self.show_module_picker_body(
                ui,
                ctx,
                catalog,
                choices,
                "reglan",
                &mut for_all_flag,
                &mut cur_struct,
                &mut cur_guid,
                reglan_cat,
                None,
                None,
                is_multi,
                show_all_opt,
            );
            self.show_all_models = show_all_tmp;
        });

        self.selected_struct[0] = cur_struct;
        props.for_all_structures = if is_multi {
            0
        } else {
            encode_for_all_structures(for_all_flag, cur_struct)
        };
        let final_id = if is_multi { 0 } else { cur_struct as i32 };
        if nar_pos_changed
            || fair_isle_changed
            || guid_changed
            || ((for_all_flag || is_multi) && !props.modules.iter().any(|m| m.module_art_id == final_id))
        {
            let m = get_or_insert_module(&mut props.modules, final_id);
            if nar_pos_changed {
                m.narrowing_position = nar_pos;
            }
            if fair_isle_changed {
                m.fair_isle_status = fair_isle;
            }
            if guid_changed {
                m.id = cur_guid;
            }
        }

        if props != before_props {
            self.working.narrowing = Some(props);
            self.working.ensure_child_slot(KanteChildSlot::Narrowing);
        }

        if jump_to_bindoff {
            self.active_tab = 3;
        }
    }

    // ── Tab 2: Artırma (`DIALOG 4270`, Screenshot `222054.png`) ──────────────────
    fn show_tab_artirma(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        choices: &[(String, String)],
        is_kw: bool,
    ) {
        let mut props = self.working.widening.clone().unwrap_or_default();
        let before_props = props.clone();

        let mut for_all_flag = is_for_all_structures(props.for_all_structures);
        let mut cur_struct = if for_all_flag {
            props.for_all_structures as usize
        } else {
            self.selected_struct[1]
        };
        let active_id = cur_struct as i32;
        let mut width = get_or_default_module(&props.modules, active_id, for_all_flag).width;
        let mut width_changed = false;

        ui.horizontal(|ui| {
            ui.label("Genişlik:");
            ui.add_space(12.0);
            if win32_int_edit(ui, "lp_artirma_width", &mut width, 42.0, 0..=999) {
                get_or_insert_module(&mut props.modules, active_id).width = width;
                width_changed = true;
            }
        });

        // Sync widening width to curve steps if present (`DLG-05`)
        if width_changed {
            if let Some(ref mut curve) = self.working.curve {
                for st in &mut curve.steps {
                    st.widening_width = width;
                }
            }
        }

        ui.add_space(4.0);

        win32_group_box_on(ui, "Genel", TAB_PANE_BG, |ui| {
            let top_y = ui.cursor().top();
            let avail_w = ui.available_width();
            let half_w = (avail_w * 0.50).max(200.0);
            let mut div_x = ui.cursor().left() + half_w;
            let resp_h = ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(half_w - 6.0);
                    win32_checkbox(ui, &mut props.execute_immediately, "Artırmayı hemen uygula");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Fonksiyon:");
                        self.show_function_selector(ui, "lp_artirma_fn", 110.0, is_kw, false);
                    });
                });
                div_x = ui.cursor().left() + 2.0;
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    win32_checkbox(ui, &mut props.use_module_color, "Modül rengi kullan");
                });
            });
            ui.painter().line_segment(
                [pos2(div_x, top_y), pos2(div_x, resp_h.response.rect.bottom())],
                Stroke::new(1.0_f32, Color32::from_rgb(0xDC, 0xDC, 0xDC)),
            );
        });

        ui.add_space(4.0);

        let (mut nar_pos, mut cur_guid) = {
            let m = get_or_default_module(&props.modules, active_id, for_all_flag);
            (m.narrowing_position, m.id)
        };
        let mut nar_pos_changed = false;

        let mut guid_changed = false;
        win32_group_box_on(ui, "Modül düzeni", TAB_PANE_BG, |ui| {
            // Stoll `DIALOG 4270`: radios map (before, with, after) -> (1, 4, 2)
            let mut pos_mode = match nar_pos {
                1 => 1,
                2 => 2,
                _ => 4,
            };
            let p_changed = win32_radio(ui, &mut pos_mode, 1, "mevcut aktarmalardan önce artır")
                .changed()
                | win32_radio(ui, &mut pos_mode, 4, "mevcut aktarmalar ile artır").changed()
                | win32_radio(ui, &mut pos_mode, 2, "mevcut aktarmalardan sonra artır").changed();
            if p_changed {
                nar_pos = pos_mode;
                nar_pos_changed = true;
            }

            ui.add_space(6.0);

            guid_changed = self.show_module_picker_body(
                ui,
                ctx,
                catalog,
                choices,
                "artirma",
                &mut for_all_flag,
                &mut cur_struct,
                &mut cur_guid,
                PreviewCategory::Widening,
                None,
                None,
                false,
                None,
            );
        });

        self.selected_struct[1] = cur_struct;
        props.for_all_structures = encode_for_all_structures(for_all_flag, cur_struct);
        let final_id = cur_struct as i32;
        if nar_pos_changed
            || guid_changed
            || (for_all_flag && !props.modules.iter().any(|m| m.module_art_id == final_id))
        {
            let m = get_or_insert_module(&mut props.modules, final_id);
            if nar_pos_changed {
                m.narrowing_position = nar_pos;
            }
            if guid_changed {
                m.id = cur_guid;
            }
        }

        if props != before_props {
            self.working.widening = Some(props);
            self.working.ensure_child_slot(KanteChildSlot::Widening);
        }
    }

    // ── Tab 3: Remayözleme (`DIALOG 4268`, Screenshot `222102.png`) ──────────────
    fn show_tab_remayozleme(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        choices: &[(String, String)],
        is_kw: bool,
    ) {
        let mut props = self.working.binding_off.clone().unwrap_or_default();
        let before_props = props.clone();

        win32_group_box_on(ui, "Genel", TAB_PANE_BG, |ui| {
            ui.horizontal(|ui| {
                win32_checkbox(ui, &mut props.use_module_color, "Modül rengi kullan");
                ui.add_space(28.0);
                ui.label("Fonksiyon:");
                self.show_function_selector(ui, "lp_remayoz_fn", 110.0, is_kw, false);
            });
        });

        ui.add_space(6.0);

        let mut for_all_flag = is_for_all_structures(props.for_all_structures);
        let mut cur_struct = if for_all_flag {
            props.for_all_structures as usize
        } else {
            self.selected_struct[2]
        };
        let active_id = cur_struct as i32;
        let mut cur_guid = get_or_default_module(&props.modules, active_id, for_all_flag).id;
        let mut bo_type = self.bind_off_type_idx.min(1);
        let bo_cat = if bo_type == 1 {
            PreviewCategory::BindOffCa
        } else {
            PreviewCategory::BindOff
        };

        let mut guid_changed = false;
        win32_group_box_on(ui, "Modül düzeni", TAB_PANE_BG, |ui| {
            guid_changed = self.show_module_picker_body(
                ui,
                ctx,
                catalog,
                choices,
                "remayoz",
                &mut for_all_flag,
                &mut cur_struct,
                &mut cur_guid,
                bo_cat,
                Some(&mut bo_type),
                None,
                false,
                None,
            );
        });

        self.bind_off_type_idx = bo_type;
        self.selected_struct[2] = cur_struct;
        props.for_all_structures = encode_for_all_structures(for_all_flag, cur_struct);
        let final_id = cur_struct as i32;
        if guid_changed
            || (for_all_flag && !props.modules.iter().any(|m| m.module_art_id == final_id))
        {
            let m = get_or_insert_module(&mut props.modules, final_id);
            if guid_changed {
                m.id = cur_guid;
            }
        }

        if props != before_props {
            self.working.binding_off = Some(props);
            self.working.ensure_child_slot(KanteChildSlot::BindingOff);
        }
    }

    // ── Tab 4: Kenar düzeltme (`DIALOG 4269`, Screenshot `222110.png`) ───────────
    fn show_tab_kenar_duzeltme(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        choices: &[(String, String)],
    ) {
        let mut props = self.working.fade_out.clone().unwrap_or_default();
        let before_props = props.clone();

        let mut for_all_flag = is_for_all_structures(props.for_all_structures);
        let mut cur_struct = if for_all_flag {
            props.for_all_structures as usize
        } else {
            self.selected_struct[3]
        };
        let active_id = cur_struct as i32;
        let active_mod = get_or_default_module(&props.modules, active_id, for_all_flag);
        let mut width = active_mod.width;
        let mut cur_guid = active_mod.id.clone();
        let mut width_changed = false;
        let has_valid_module = !cur_guid.is_empty() && !cur_guid.eq_ignore_ascii_case(ZERO_GUID);
        let fade_width_enabled = has_valid_module;

        ui.horizontal(|ui| {
            ui.add_enabled_ui(fade_width_enabled, |ui| {
                ui.label("Kenar düzeltme eni:");
                ui.add_space(12.0);
                if win32_int_edit(ui, "lp_kenar_width", &mut width, 42.0, 0..=999) {
                    get_or_insert_module(&mut props.modules, active_id).width = width;
                    width_changed = true;
                }
            });
        });

        // Propagate fade-out width to curve steps if present (`DLG-07`)
        if width_changed {
            if let Some(ref mut curve) = self.working.curve {
                for st in &mut curve.steps {
                    st.fade_out_width = width;
                }
            }
        }

        ui.add_space(4.0);

        win32_group_box_on(ui, "Genel", TAB_PANE_BG, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(275.0);
                    win32_checkbox(ui, &mut props.use_module_color, "Modül rengi kullan");
                    win32_checkbox(
                        ui,
                        &mut props.no_auto_fade_out,
                        "Jakarda otomatik kenar düzleme yok",
                    );
                });
                ui.vertical(|ui| {
                    ui.set_max_width(130.0);
                    win32_group_box_on(ui, "Modül-Offset", TAB_PANE_BG, |ui| {
                        ui.horizontal(|ui| {
                            let (ir, _) =
                                ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            paint_icon(ui.painter(), ir, IconKind::HorizontalArrow, true);
                            win32_int_edit(
                                ui,
                                "lp_kenar_off_x",
                                &mut props.offset_x,
                                46.0,
                                -99..=99,
                            );
                        });
                    });
                });
            });
        });

        ui.add_space(4.0);

        let mut guid_changed = false;
        win32_group_box_on(ui, "Modül düzeni", TAB_PANE_BG, |ui| {
            guid_changed = self.show_module_picker_body(
                ui,
                ctx,
                catalog,
                choices,
                "kenar",
                &mut for_all_flag,
                &mut cur_struct,
                &mut cur_guid,
                PreviewCategory::FadeOut,
                None,
                None,
                false,
                None,
            );
        });

        self.selected_struct[3] = cur_struct;
        props.for_all_structures = encode_for_all_structures(for_all_flag, cur_struct);
        let final_id = cur_struct as i32;
        if guid_changed
            || (for_all_flag && !props.modules.iter().any(|m| m.module_art_id == final_id))
        {
            let m = get_or_insert_module(&mut props.modules, final_id);
            if guid_changed {
                m.id = cur_guid;
            }
        }

        if props != before_props {
            self.working.fade_out = Some(props);
            self.working.ensure_child_slot(KanteChildSlot::FadeOut);
        }
    }

    // ── Tab 5 (Başlama `222119.png`) & Tab 6 (Bitiş `222125.png`) ───────────────
    // Lazy materialization (`DLG-08`): never insert a new `[Punkt]` until a field is actually edited!
    fn show_tab_point(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        choices: &[(String, String)],
        point_slot: usize,
        category: PreviewCategory,
        elem_art_id: i32,
    ) {
        let slot_i32 = point_slot as i32;
        let existing_pos = self.working.points.iter().position(|p| p.index == slot_i32);
        let mut pt_copy = existing_pos
            .map(|i| self.working.points[i].clone())
            .unwrap_or_else(|| PointEntry {
                index: slot_i32,
                for_all_structures: ALL_STRUCTURES_NONE,
                ..PointEntry::default()
            });
        let before_copy = pt_copy.clone();

        let struct_slot = 4 + point_slot;
        let mut cur_struct = self.selected_struct[struct_slot];

        win32_group_box_on(ui, "Genel", TAB_PANE_BG, |ui| {
            win32_checkbox(ui, &mut pt_copy.use_module_color, "Modül rengi kullan");
        });

        ui.add_space(4.0);

        // Stoll `DIALOG 4271` / `4272`: only Normal (0), üst sol yaka kesimi (1), Delik (3) — never Alt kol kesimi (2).
        // On Tab 6 (`Bitiş`, 4272), disable `15727` when element is `Yaka Kesimi` (2) or `Kesim Yaka açıklığı` (7).
        let pt_func_enabled = !(point_slot == 1 && (elem_art_id == 2 || elem_art_id == 7));
        let is_neckline_start = point_slot == 0 && (elem_art_id == 2 || elem_art_id == 7);
        win32_group_box_on(ui, "Fonksiyon", TAB_PANE_BG, |ui| {
            let cur_kind = PointKind::from_id(pt_copy.punkt_art_tag_id);
            let cur_label = if is_neckline_start && pt_copy.punkt_art_tag_id == 3 {
                "Yaka kesimi alt orta"
            } else {
                cur_kind.label_tr()
            };
            ui.add_enabled_ui(pt_func_enabled, |ui| {
                win32_combo_scope(ui, |ui| {
                    egui::ComboBox::from_id_salt(format!("lp_pt_func_{point_slot}"))
                        .width(210.0)
                        .selected_text(cur_label)
                        .show_ui(ui, |ui| {
                            for k in [PointKind::Normal, PointKind::TopLeftNeck, PointKind::Hole] {
                                let lbl = if is_neckline_start && k == PointKind::Hole {
                                    "Yaka kesimi alt orta"
                                } else {
                                    k.label_tr()
                                };
                                ui.selectable_value(
                                    &mut pt_copy.punkt_art_tag_id,
                                    k.to_id(),
                                    lbl,
                                );
                            }
                        });
                });
            });
        });

        ui.add_space(4.0);

        let mut for_all_flag = is_for_all_structures(pt_copy.for_all_structures);
        if for_all_flag {
            cur_struct = pt_copy.for_all_structures as usize;
        }
        let active_id = cur_struct as i32;
        let mut cur_guid = get_or_default_module(&pt_copy.modules, active_id, for_all_flag).id;
        let mut off_xy = pt_copy
            .offsets
            .first()
            .map(|o| (o.x, o.y))
            .unwrap_or((0, 0));

        let id_tag = if point_slot == 0 { "baslama" } else { "bitis" };
        let effective_category = if point_slot == 0
            && (pt_copy.punkt_art_tag_id == PointKind::TopLeftNeck.to_id()
                || (is_neckline_start && pt_copy.punkt_art_tag_id == 3))
        {
            PreviewCategory::PointStartVneck
        } else {
            category
        };

        let mut guid_changed = false;
        win32_group_box_on(ui, "Modül düzeni", TAB_PANE_BG, |ui| {
            guid_changed = self.show_module_picker_body(
                ui,
                ctx,
                catalog,
                choices,
                id_tag,
                &mut for_all_flag,
                &mut cur_struct,
                &mut cur_guid,
                effective_category,
                None,
                Some(&mut off_xy),
                false,
                None,
            );
        });

        self.selected_struct[struct_slot] = cur_struct;
        pt_copy.for_all_structures = encode_for_all_structures(for_all_flag, cur_struct);
        if !pt_copy.offsets.is_empty() || off_xy != (0, 0) {
            if pt_copy.offsets.is_empty() {
                pt_copy.offsets.push(PointOffset::default());
            }
            pt_copy.offsets[0].x = off_xy.0;
            pt_copy.offsets[0].y = off_xy.1;
        }

        let final_id = cur_struct as i32;
        if guid_changed
            || (for_all_flag && !pt_copy.modules.iter().any(|m| m.module_art_id == final_id))
        {
            let m = get_or_insert_module(&mut pt_copy.modules, final_id);
            if guid_changed {
                m.id = cur_guid;
            }
        }

        // Only write back / materialize into `self.working.points` if user actually changed something!
        if pt_copy != before_copy {
            if pt_copy.offsets.is_empty() {
                pt_copy.offsets.push(PointOffset::default());
            }
            if let Some(pos) = existing_pos {
                self.working.points[pos] = pt_copy;
            } else {
                let new_idx = self.working.points.len();
                self.working.points.push(pt_copy);
                self.working.ensure_child_slot(KanteChildSlot::Point(new_idx));
            }
        }
    }

    // ── Tab 7: Bağlantılar (`DIALOG 4273` `15739` CCheckListBox, K&W only) ──────
    fn show_tab_baglantilar(&mut self, ui: &mut Ui) {
        let mut cp = self.working.connection_points.clone().unwrap_or_default();
        let before_cp = cp.clone();

        win32_group_box_on(
            ui,
            "Bağlantı noktaları ([ConnectionPoints])",
            TAB_PANE_BG,
            |ui| {
                ui.label(
                    egui::RichText::new(
                        "K&W elemanları arası bağlantı noktası kimliklerini işaretleyin:",
                    )
                    .font(regular_font(10.5))
                    .color(colors::TEXT_SECONDARY),
                );
                ui.add_space(6.0);

                // Collect standard Stoll bit IDs (1, 2, 4, ..., 8192) + any custom IDs present
                let mut candidate_ids: Vec<i32> = (0..14).map(|shift| 1i32 << shift).collect();
                for pt in &cp.points {
                    if !candidate_ids.contains(&pt.connect_id) {
                        candidate_ids.push(pt.connect_id);
                    }
                }

                Frame::NONE
                    .fill(colors::INPUT_BG)
                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                    .inner_margin(Margin::same(4))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("lp_conn_list_scroll")
                            .min_scrolled_height(260.0)
                            .max_height(260.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for &cid in &candidate_ids {
                                    let pos = cp.points.iter().position(|p| p.connect_id == cid);
                                    let mut checked = pos.is_some();
                                    let lbl = format!("Bağlantı {cid}");
                                    ui.horizontal(|ui| {
                                        if win32_checkbox(ui, &mut checked, &lbl).changed() {
                                            if checked {
                                                cp.points.push(ConnectPoint {
                                                    connect_id: cid,
                                                    connect_type: None,
                                                    direction: true,
                                                    raw: Default::default(),
                                                });
                                            } else if let Some(idx) = pos {
                                                cp.points.remove(idx);
                                            }
                                        }
                                        if let Some(idx) = pos {
                                            if checked {
                                                if let Some(pt) = cp.points.get_mut(idx) {
                                                    ui.add_space(16.0);
                                                    win32_checkbox(
                                                        ui,
                                                        &mut pt.direction,
                                                        "İleri (ja)",
                                                    );
                                                }
                                            }
                                        }
                                    });
                                }
                            });
                    });
            },
        );

        if cp != before_cp {
            self.working.connection_points = Some(cp);
            self.working
                .ensure_child_slot(KanteChildSlot::ConnectionPoints);
        }
    }

    // ── Shared `Modül düzeni` 12-structure ListBox + Module Combo + 128x128 Preview ─
    #[allow(clippy::too_many_arguments)]
    fn show_module_picker_body(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        choices: &[(String, String)],
        id_tag: &str,
        for_all_flag: &mut bool,
        selected_struct: &mut usize,
        cur_guid: &mut String,
        category: PreviewCategory,
        mut bind_off_type: Option<&mut usize>,
        mut offset_xy: Option<&mut (i32, i32)>,
        multistage_mode: bool,
        mut show_all_models: Option<&mut bool>,
    ) -> bool {
        let mut guid_changed = false;
        ui.horizontal_top(|ui| {
            // Left column: 12-item Knitting Structure ListBox (or single `'Tüm örgü türleri'` row in `çok kademeli`)
            ui.vertical(|ui| {
                ui.set_width(188.0);

                // Small knitting structure header icon centered above listbox (like `222038.png`)
                ui.horizontal(|ui| {
                    ui.add_space(76.0);
                    let (icon_r, _) = ui.allocate_exact_size(vec2(24.0, 14.0), Sense::hover());
                    paint_structure_badge_icon(ui.painter(), icon_r);
                });

                // Sunken ListBox frame
                let list_h = 192.0;
                Frame::NONE
                    .fill(if *for_all_flag && !multistage_mode {
                        Color32::BLACK
                    } else {
                        colors::INPUT_BG
                    })
                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                    .inner_margin(Margin::same(1))
                    .show(ui, |ui| {
                        ui.set_width(184.0);
                        ui.set_min_height(list_h);
                        let row_h = list_h / 12.0;
                        if multistage_mode {
                            let (row_rect, _) =
                                ui.allocate_exact_size(vec2(182.0, row_h), Sense::hover());
                            let painter = ui.painter();
                            painter.rect_filled(row_rect, CornerRadius::ZERO, colors::WIN_BLUE);
                            painter.text(
                                pos2(row_rect.left() + 4.0, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                "Tüm örgü türleri",
                                regular_font(10.5),
                                Color32::WHITE,
                            );
                        } else {
                            for (idx, ks) in KNITTING_STRUCTURES.iter().take(12).enumerate() {
                                let (row_rect, resp) =
                                    ui.allocate_exact_size(vec2(182.0, row_h), Sense::click());
                                let painter = ui.painter();

                                if *for_all_flag {
                                    if idx == *selected_struct {
                                        // Active structure row: white icon + green `✓` + blue selection bar (`222038.png`!)
                                        let icon_rect = Rect::from_min_size(
                                            row_rect.min,
                                            vec2(16.0, row_rect.height()),
                                        );
                                        let label_rect = Rect::from_min_max(
                                            pos2(row_rect.left() + 16.0, row_rect.top()),
                                            row_rect.max,
                                        );
                                        painter.rect_filled(
                                            icon_rect,
                                            CornerRadius::ZERO,
                                            Color32::WHITE,
                                        );
                                        painter.rect_filled(
                                            label_rect,
                                            CornerRadius::ZERO,
                                            colors::WIN_BLUE,
                                        );
                                        // Crisp green checkmark
                                        let c = icon_rect.center();
                                        let g_stroke =
                                            Stroke::new(1.8_f32, Color32::from_rgb(0x00, 0xA8, 0x00));
                                        painter.line_segment(
                                            [pos2(c.x - 4.0, c.y), pos2(c.x - 1.0, c.y + 3.5)],
                                            g_stroke,
                                        );
                                        painter.line_segment(
                                            [pos2(c.x - 1.0, c.y + 3.5), pos2(c.x + 4.5, c.y - 4.0)],
                                            g_stroke,
                                        );
                                        painter.text(
                                            pos2(label_rect.left() + 3.0, label_rect.center().y),
                                            Align2::LEFT_CENTER,
                                            ks.name_tr,
                                            regular_font(10.5),
                                            Color32::WHITE,
                                        );
                                    } else {
                                        // Rows 1..11: solid black background + crisp red `X` (`222038.png`, `222102.png`, `222110.png`)
                                        painter.rect_filled(
                                            row_rect,
                                            CornerRadius::ZERO,
                                            Color32::BLACK,
                                        );
                                        let cx = row_rect.left() + 8.0;
                                        let cy = row_rect.center().y;
                                        let r_stroke =
                                            Stroke::new(1.7_f32, Color32::from_rgb(0xFF, 0x18, 0x18));
                                        painter.line_segment(
                                            [pos2(cx - 4.0, cy - 4.0), pos2(cx + 4.0, cy + 4.0)],
                                            r_stroke,
                                        );
                                        painter.line_segment(
                                            [pos2(cx - 4.0, cy + 4.0), pos2(cx + 4.0, cy - 4.0)],
                                            r_stroke,
                                        );
                                    }
                                } else {
                                    // Individual per-structure mode (`222054.png`, `222119.png`, `222125.png`)
                                    let is_sel = *selected_struct == idx;
                                    let bg = if is_sel {
                                        colors::WIN_BLUE
                                    } else if resp.hovered() {
                                        colors::WIN_BLUE_HOVER
                                    } else {
                                        colors::INPUT_BG
                                    };
                                    let fg = if is_sel {
                                        Color32::WHITE
                                    } else {
                                        colors::TEXT_BLACK
                                    };
                                    painter.rect_filled(row_rect, CornerRadius::ZERO, bg);
                                    painter.text(
                                        pos2(row_rect.left() + 4.0, row_rect.center().y),
                                        Align2::LEFT_CENTER,
                                        ks.name_tr,
                                        regular_font(10.5),
                                        fg,
                                    );
                                    if resp.clicked() && *selected_struct != idx {
                                        *selected_struct = idx;
                                        ctx.request_repaint();
                                    }
                                }
                            }
                        }
                    });

                if !multistage_mode {
                    ui.add_space(5.0);
                    win32_checkbox(ui, for_all_flag, "Tüm örgü türleri için geçerlidir");
                }
            });

            ui.add_space(14.0);

            // Right column: Optional `Remayözleme-tipi`, `Modül:` dropdown, 128x128 preview, optional `Ofset`
            ui.vertical(|ui| {
                ui.set_min_width(250.0);

                let mut active_category = category;
                if let Some(ref mut bo_idx) = bind_off_type {
                    ui.label("Remayözleme-tipi:");
                    let bo_types = ["Modül", "RA"];
                    let prev_bo = **bo_idx;
                    let cur_bo = bo_types.get(prev_bo).copied().unwrap_or("Modül");
                    win32_combo_scope(ui, |ui| {
                        egui::ComboBox::from_id_salt(format!("lp_bo_type_{id_tag}"))
                            .width(210.0)
                            .selected_text(cur_bo)
                            .show_ui(ui, |ui| {
                                for (i, &lbl) in bo_types.iter().enumerate() {
                                    ui.selectable_value(*bo_idx, i, lbl);
                                }
                            });
                    });
                    if **bo_idx != prev_bo {
                        active_category = if **bo_idx == 1 {
                            PreviewCategory::BindOffCa
                        } else {
                            PreviewCategory::BindOff
                        };
                        if let Some(first) = catalog
                            .modules_for_category_and_structure(
                                active_category.to_module_category(),
                                *selected_struct as i32,
                            )
                            .first()
                        {
                            *cur_guid = first.guid.clone();
                            guid_changed = true;
                        }
                    }
                    ui.add_space(8.0);
                    ui.label("Modül:");
                } else {
                    if let Some(ref mut show_all) = show_all_models {
                        ui.horizontal(|ui| {
                            ui.label("Modül:");
                            ui.add_space(18.0);
                            win32_checkbox(ui, show_all, "Tüm modelleri göster");
                        });
                    } else {
                        ui.label("Modül:");
                    }
                }

                // Resolve display name of current GUID first (populates `self.resolved_cache`)
                let selected_label = self.resolve_display_name(catalog, cur_guid);
                let show_all_active = show_all_models.as_deref().copied().unwrap_or(false);
                let effective_struct_id: i32 = if multistage_mode || show_all_active {
                    -1
                } else {
                    *selected_struct as i32
                };

                // Cached category- & structure-specific Stoll DB module dropdown (`ENG-01` / `#48.3`)
                let cache_key = (
                    active_category,
                    effective_struct_id,
                    normalize_guid(cur_guid),
                    choices.len(),
                );
                let cat_choices = if let Some((ref k, ref list)) = self.cached_cat_choices {
                    if *k == cache_key {
                        list.clone()
                    } else {
                        let built = collect_category_module_choices(
                            catalog,
                            active_category.to_module_category(),
                            effective_struct_id,
                            choices,
                            cur_guid,
                            &self.resolved_cache,
                        );
                        self.cached_cat_choices = Some((cache_key, built.clone()));
                        built
                    }
                } else {
                    let built = collect_category_module_choices(
                        catalog,
                        active_category.to_module_category(),
                        effective_struct_id,
                        choices,
                        cur_guid,
                        &self.resolved_cache,
                    );
                    self.cached_cat_choices = Some((cache_key, built.clone()));
                    built
                };
                win32_combo_scope(ui, |ui| {
                    egui::ComboBox::from_id_salt(format!(
                        "lp_mod_combo_{id_tag}_{}_{}_{}",
                        effective_struct_id,
                        if active_category == PreviewCategory::BindOffCa { 1 } else { 0 },
                        if show_all_active { 1 } else { 0 }
                    ))
                    .width(216.0)
                    .selected_text(&selected_label)
                    .show_ui(ui, |ui| {
                        // `(Modül yok)` option
                        if ui
                            .selectable_label(
                                cur_guid.is_empty() || cur_guid.eq_ignore_ascii_case(ZERO_GUID),
                                "(Modül yok)",
                            )
                            .clicked()
                        {
                            *cur_guid = ZERO_GUID.to_string();
                            guid_changed = true;
                        }
                        for (guid, name) in &cat_choices {
                            let is_sel = cur_guid.eq_ignore_ascii_case(guid);
                            if ui.selectable_label(is_sel, name).clicked() {
                                *cur_guid = guid.clone();
                                guid_changed = true;
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // 128×128 Stitch Module Preview Canvas
                let preview_size = if offset_xy.is_some() { 112.0 } else { 128.0 };
                let tex = self.get_or_load_module_texture(ctx, catalog, cur_guid);
                let (prev_rect, _) =
                    ui.allocate_exact_size(vec2(preview_size, preview_size), Sense::hover());

                let painter = ui.painter();
                painter.rect_filled(prev_rect, CornerRadius::ZERO, Color32::WHITE);
                if let Some(handle) = tex {
                    painter.image(
                        handle.id(),
                        prev_rect.shrink(1.0),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else if !cur_guid.is_empty() && !cur_guid.eq_ignore_ascii_case(ZERO_GUID) {
                    paint_schematic_module_preview(painter, prev_rect.shrink(1.0), category);
                }
                painter.rect_stroke(
                    prev_rect,
                    CornerRadius::ZERO,
                    Stroke::new(1.0_f32, colors::BORDER_MID),
                    StrokeKind::Inside,
                );

                // Optional `Ofset` group on Başlama / Bitiş tabs (`222119.png`, `222125.png`)
                if let Some(ref mut oxy) = offset_xy {
                    ui.add_space(6.0);
                    win32_group_box_on(ui, "Ofset", TAB_PANE_BG, |ui| {
                        ui.horizontal(|ui| {
                            let (irx, _) =
                                ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            paint_icon(ui.painter(), irx, IconKind::HorizontalArrow, true);
                            win32_int_edit(
                                ui,
                                format!("lp_pt_off_x_{id_tag}"),
                                &mut oxy.0,
                                42.0,
                                -999..=999,
                            );
                            ui.add_space(16.0);
                            let (iry, _) =
                                ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            paint_icon(ui.painter(), iry, IconKind::VerticalArrow, true);
                            win32_int_edit(
                                ui,
                                format!("lp_pt_off_y_{id_tag}"),
                                &mut oxy.1,
                                42.0,
                                -999..=999,
                            );
                        });
                    });
                }
            });
        });
        guid_changed
    }

    fn resolve_display_name(&mut self, catalog: &StollCatalog, guid: &str) -> String {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == ZERO_GUID {
            return String::new();
        }
        if let Some(info) = catalog.modules_by_guid.get(&norm) {
            return info.name.clone();
        }
        if !self.resolved_cache.contains_key(&norm) {
            let loaded = catalog
                .stoll_data_root
                .as_deref()
                .and_then(|root| load_stoll_module_from_db(root, &norm));
            self.resolved_cache.insert(norm.clone(), loaded);
        }
        if let Some(Some(info)) = self.resolved_cache.get(&norm) {
            return info.name.clone();
        }
        guid.to_string()
    }

    fn get_or_load_module_texture(
        &mut self,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        guid: &str,
    ) -> Option<TextureHandle> {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == ZERO_GUID {
            return None;
        }
        if let Some(cached) = self.texture_cache.get(&norm) {
            return cached.clone();
        }

        // First look in catalog's built-in/loaded map, then load directly from Stoll Module DB
        let preview_img = catalog
            .modules_by_guid
            .get(&norm)
            .and_then(|m| m.thumbnail.clone())
            .or_else(|| {
                if !self.resolved_cache.contains_key(&norm) {
                    let loaded = catalog
                        .stoll_data_root
                        .as_deref()
                        .and_then(|root| load_stoll_module_from_db(root, &norm));
                    self.resolved_cache.insert(norm.clone(), loaded);
                }
                self.resolved_cache
                    .get(&norm)
                    .and_then(|opt| opt.as_ref())
                    .and_then(|m| m.thumbnail.clone())
            });

        let handle = preview_img.map(|img| {
            // `decode_m3_utl_bitmap` normalizes ALL Stoll bitmap formats to canonical top-down RGBA (#48 Item 4).
            let w = img.width as usize;
            let h = img.height as usize;
            let color_img = ColorImage::from_rgba_unmultiplied([w, h], &img.rgba);
            ctx.load_texture(format!("mod_bmp_{norm}"), color_img, TextureOptions::NEAREST)
        });
        self.texture_cache.insert(norm, handle.clone());
        handle
    }
}

fn normalize_guid(guid: &str) -> String {
    let t = guid.trim().to_ascii_uppercase();
    if t.is_empty() {
        return String::new();
    }
    if t.starts_with('{') && t.ends_with('}') {
        t
    } else {
        format!("{{{t}}}")
    }
}

fn collect_category_module_choices(
    catalog: &StollCatalog,
    category: ModuleCategory,
    structure_id: i32,
    doc_ext_modules: &[(String, String)],
    current_guid: &str,
    resolved_cache: &HashMap<String, Option<KnittingModuleInfo>>,
) -> Vec<(String, String)> {
    let mut list: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1. Strictly populate from Stoll DB modules for this exact category & knitting structure,
    // preserving Stoll's exact `M3_MOD_DirChildren` order (no alphabetical re-sort, no cross-structure mixing).
    for info in catalog.modules_for_category_and_structure(category, structure_id) {
        let g = normalize_guid(&info.guid);
        if !g.is_empty() && g != ZERO_GUID && seen.insert(g.clone()) {
            list.push((g, info.name.clone()));
        }
    }

    // 2. Ensure currently assigned GUID is visible if not already in this structure's list
    // (so existing custom/deleted `.shp` assignments display cleanly without polluting other structures).
    let cur_norm = normalize_guid(current_guid);
    if !cur_norm.is_empty() && cur_norm != ZERO_GUID && !seen.contains(&cur_norm) {
        let name = resolved_cache
            .get(&cur_norm)
            .and_then(|o| o.as_ref())
            .map(|m| m.name.clone())
            .or_else(|| {
                doc_ext_modules
                    .iter()
                    .find(|(g, _)| g.eq_ignore_ascii_case(&cur_norm))
                    .map(|(_, n)| n.clone())
            })
            .unwrap_or_else(|| catalog.module_name(&cur_norm));
        list.push((cur_norm, name));
    }

    list
}

fn set_narrowing_delayed_raw(props: &mut NarrowingProps, delayed: bool) {
    // Ensure standard keys are present before adding a raw key so serializer.rs still emits all defaults
    if props.raw.raw_entries.is_empty() && delayed {
        props.raw.set_raw(
            "FuerAlleStrickarten",
            props.for_all_structures.to_string(),
        );
        props.raw.set_raw(
            "SofortAusfuehren",
            if props.execute_immediately { "1" } else { "0" }.to_string(),
        );
        props
            .raw
            .set_raw("AnzahlMaschen", props.stitch_count.to_string());
        props.raw.set_raw(
            "ModulFarbeVerwenden",
            if props.use_module_color { "ja" } else { "nein" }.to_string(),
        );
    }
    if delayed {
        props.raw.set_raw("VerzoegertAusfuehren", "1".to_string());
    } else if props.raw.get_raw("VerzoegertAusfuehren").is_some() {
        props.raw.set_raw("VerzoegertAusfuehren", "0".to_string());
    }
}

fn get_or_default_module(
    list: &[ModuleAssignment],
    struct_id: i32,
    for_all: bool,
) -> ModuleAssignment {
    if for_all {
        list.iter()
            .find(|m| m.module_art_id == struct_id)
            .or_else(|| list.iter().find(|m| m.module_art_id == 0))
            .or_else(|| list.iter().find(|m| m.module_art_id == -1))
            .or_else(|| list.first())
            .cloned()
    } else {
        list.iter()
            .find(|m| m.module_art_id == struct_id)
            .or_else(|| list.iter().find(|m| m.module_art_id == -1))
            .cloned()
    }
    .unwrap_or(ModuleAssignment {
        module_art_id: struct_id,
        ..ModuleAssignment::default()
    })
}

fn get_or_insert_module(list: &mut Vec<ModuleAssignment>, struct_id: i32) -> &mut ModuleAssignment {
    if let Some(pos) = list.iter().position(|m| m.module_art_id == struct_id) {
        return &mut list[pos];
    }
    let mut base = ModuleAssignment::default();
    if let Some(fallback) = list
        .iter()
        .find(|m| m.module_art_id == 0)
        .or_else(|| list.iter().find(|m| m.module_art_id == -1))
        .or_else(|| list.first())
    {
        if !fallback.id.is_empty() {
            base.id = fallback.id.clone();
        }
        base.width = fallback.width;
        base.bind_off_limit = fallback.bind_off_limit;
        base.narrowing_position = fallback.narrowing_position;
    }
    base.module_art_id = struct_id;
    base.raw.original_header = Some(format!("[ModuleArt] = {struct_id}"));
    list.push(base);
    list.last_mut().expect("just pushed")
}

fn win32_int_edit(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash,
    val: &mut i32,
    width: f32,
    range: std::ops::RangeInclusive<i32>,
) -> bool {
    if let Some(txt) = win32_numeric_text_edit(ui, id_salt, val.to_string(), width, true) {
        if let Ok(parsed) = txt.trim().parse::<i32>() {
            let clamped = parsed.clamp(*range.start(), *range.end());
            if clamped != *val {
                *val = clamped;
                return true;
            }
        }
    }
    false
}

fn np_index_combo(ui: &mut Ui, id_salt: &str, idx: &mut i32) {
    let label = if *idx <= 0 {
        "-".to_string()
    } else {
        format!("{}", *idx)
    };
    win32_combo_scope(ui, |ui| {
        egui::ComboBox::from_id_salt(id_salt)
            .width(44.0)
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(idx, 0, "-");
                for n in 1..=20 {
                    ui.selectable_value(idx, n, format!("{n}"));
                }
            });
    });
}

fn np_tenths_edit(ui: &mut Ui, id_salt: &str, val_x100: &mut i32) {
    let display = format!("{:.1}", (*val_x100 as f64) / 100.0);
    if let Some(txt) = win32_numeric_text_edit(ui, id_salt, display, 52.0, ui.is_enabled()) {
        if let Ok(parsed) = txt.trim().replace(',', ".").parse::<f64>() {
            *val_x100 = (parsed.clamp(0.0, 25.0) * 100.0).round() as i32;
        }
    }
}

fn paint_win32_dialog_titlebar(ui: &mut Ui, title: &str, close_id: &str) -> bool {
    let w = ui.available_width().max(240.0);
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
    let close_resp = ui.interact(close_rect, ui.id().with(close_id), Sense::click());
    let hov = close_resp.hovered();
    if hov {
        painter.rect_filled(
            close_rect,
            CornerRadius::ZERO,
            Color32::from_rgb(0xE8, 0x11, 0x23),
        );
    }
    let xc = close_rect.center();
    let x_st = Stroke::new(1.1_f32, if hov { Color32::WHITE } else { colors::TEXT_BLACK });
    painter.line_segment([pos2(xc.x - 4.5, xc.y - 4.5), pos2(xc.x + 4.5, xc.y + 4.5)], x_st);
    painter.line_segment([pos2(xc.x - 4.5, xc.y + 4.5), pos2(xc.x + 4.5, xc.y - 4.5)], x_st);

    painter.text(
        pos2(bar_rect.left() + 8.0, bar_rect.center().y),
        Align2::LEFT_CENTER,
        title,
        regular_font(11.5),
        colors::TEXT_BLACK,
    );
    close_resp.clicked()
}

/// Tiny Stoll knitting-structure puzzle badge icon centered above the 12-item ListBox.
fn paint_structure_badge_icon(painter: &egui::Painter, rect: Rect) {
    let c = rect.center();
    let box_r = Rect::from_center_size(pos2(c.x + 2.0, c.y), vec2(11.0, 9.0));
    painter.rect_filled(box_r, CornerRadius::ZERO, Color32::from_rgb(0xB8, 0xA8, 0x38));
    painter.rect_stroke(
        box_r,
        CornerRadius::ZERO,
        Stroke::new(1.0_f32, colors::BORDER_BLACK),
        StrokeKind::Inside,
    );
    // Small curved arrow on left
    painter.line_segment(
        [pos2(c.x - 9.0, c.y - 2.0), pos2(c.x - 4.0, c.y - 2.0)],
        Stroke::new(1.0_f32, colors::BORDER_BLACK),
    );
    painter.line_segment(
        [pos2(c.x - 9.0, c.y - 2.0), pos2(c.x - 6.0, c.y + 1.0)],
        Stroke::new(1.0_f32, colors::BORDER_BLACK),
    );
}

/// Crisp schematic stitch preview rendered when a Stoll `M3_UTL_Bitmap` thumbnail is not installed on disk.
fn paint_schematic_module_preview(
    painter: &egui::Painter,
    rect: Rect,
    category: PreviewCategory,
) {
    let bg = Color32::from_rgb(0xD8, 0xD8, 0xD8);
    painter.rect_filled(rect, CornerRadius::ZERO, bg);

    // Subtle stitch grid
    let cols = 8;
    let rows = 8;
    let cw = rect.width() / cols as f32;
    let rh = rect.height() / rows as f32;
    let grid_col = Color32::from_rgb(0xB4, 0xB4, 0xB4);
    for c in 1..cols {
        let x = rect.left() + c as f32 * cw;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0_f32, grid_col));
    }
    for r in 1..rows {
        let y = rect.top() + r as f32 * rh;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0_f32, grid_col));
    }

    // Center divider (like Stoll left/right split preview in 222038/222054)
    let cx = rect.center().x;
    painter.line_segment(
        [pos2(cx, rect.top()), pos2(cx, rect.bottom())],
        Stroke::new(2.0_f32, Color32::from_rgb(0x60, 0x60, 0x60)),
    );

    match category {
        PreviewCategory::Narrowing | PreviewCategory::NarrowingMultistage => {
            // Red front transfer arrows + green rear transfer arrows + blue knit loops at bottom
            for i in 0..3 {
                let x = rect.left() + (i as f32 + 0.8) * cw;
                let y = rect.top() + 2.5 * rh;
                painter.line_segment(
                    [pos2(x, y), pos2(x + cw, y)],
                    Stroke::new(1.5_f32, Color32::from_rgb(0xC8, 0x10, 0x10)),
                );
                let y2 = rect.top() + 5.2 * rh;
                painter.line_segment(
                    [pos2(x, y2), pos2(x + cw * 0.7, y2 - rh * 0.6)],
                    Stroke::new(1.5_f32, Color32::from_rgb(0x10, 0x80, 0x10)),
                );
            }
            for c in 0..cols {
                let lx = rect.left() + (c as f32 + 0.5) * cw;
                let ly = rect.bottom() - 0.6 * rh;
                painter.circle_stroke(
                    pos2(lx, ly),
                    cw * 0.28,
                    Stroke::new(1.2_f32, Color32::from_rgb(0x30, 0x50, 0xC8)),
                );
            }
        }
        PreviewCategory::Widening => {
            // Yellow split/widen markers + blue stitch loops
            for r in 1..6 {
                let lx = rect.left() + (1.0 + (r % 2) as f32 * 0.5) * cw;
                let rx = rect.right() - (2.0 + (r % 2) as f32 * 0.5) * cw;
                let y = rect.top() + r as f32 * rh;
                painter.rect_filled(
                    Rect::from_min_size(pos2(lx, y), vec2(cw, rh)),
                    CornerRadius::ZERO,
                    Color32::from_rgb(0xF0, 0xE8, 0x30),
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(rx, y), vec2(cw, rh)),
                    CornerRadius::ZERO,
                    Color32::from_rgb(0xF0, 0xE8, 0x30),
                );
            }
        }
        PreviewCategory::BindOff | PreviewCategory::BindOffCa => {
            // Diagonal bind-off cascade ladder
            for k in 0..6 {
                let x = rect.left() + (k as f32 + 1.0) * cw;
                let y = rect.bottom() - (k as f32 + 1.5) * rh;
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, y), vec2(cw, rh)),
                    CornerRadius::ZERO,
                    Color32::from_rgb(0x50, 0x90, 0xD8),
                );
            }
        }
        PreviewCategory::FadeOut => {
            // Single knitted stitch loop icon (`222110.png`)
            painter.rect_filled(rect, CornerRadius::ZERO, Color32::WHITE);
            let c = rect.center();
            let r = rect.width() * 0.28;
            painter.circle_stroke(
                pos2(c.x, c.y - r * 0.35),
                r,
                Stroke::new(4.5_f32, Color32::from_rgb(0xC8, 0xB8, 0x60)),
            );
            painter.line_segment(
                [pos2(c.x - r * 0.7, c.y + r * 0.4), pos2(c.x - r * 0.9, rect.bottom() - 8.0)],
                Stroke::new(4.5_f32, Color32::from_rgb(0xC8, 0xB8, 0x60)),
            );
            painter.line_segment(
                [pos2(c.x + r * 0.7, c.y + r * 0.4), pos2(c.x + r * 0.9, rect.bottom() - 8.0)],
                Stroke::new(4.5_f32, Color32::from_rgb(0xC8, 0xB8, 0x60)),
            );
        }
        PreviewCategory::PointStart
        | PreviewCategory::PointStartVneck
        | PreviewCategory::PointEnd => {
            let c = rect.center();
            let d = 18.0;
            painter.line_segment(
                [pos2(c.x - d, c.y), pos2(c.x + d, c.y)],
                Stroke::new(2.0_f32, colors::WIN_BLUE),
            );
            painter.line_segment(
                [pos2(c.x, c.y - d), pos2(c.x, c.y + d)],
                Stroke::new(2.0_f32, colors::WIN_BLUE),
            );
            painter.circle_stroke(c, 8.0, Stroke::new(1.5_f32, colors::STATUS_ERR_RED));
        }
    }
}

fn tab_help_text_tr(tab: usize) -> &'static str {
    match tab {
        0 => "Genel sekmesi: Çizginin temel fonksiyonunu (Reglan, Artırma, Remayözleme, Temel vb.), kenar ilmek uzunluğunu (NP ön/arka, NPS) ve 'Sakın Örme' seçeneğini belirler.",
        1 => "Reglan sekmesi: Eksiltme genişliği, remayözleme basamak sınırı, aktarma sırası (önce/beraber/sonra) ve örgü türlerine göre reglan modülü atamasını yönetir.",
        2 => "Artırma sekmesi: Artırma genişliği, hemen uygulama ve örgü türlerine göre artırma modülü atamasını yönetir.",
        3 => "Remayözleme sekmesi: Kapatma / kesme (bind-off) tipi ve modül atamalarını yapılandırır.",
        4 => "Kenar düzeltme sekmesi: Kenar düzeltme enini, jakar otomatik düzeltme seçeneğini, modül X ofsetini ve modül atamasını belirler.",
        5 => "Başlama sekmesi: Çizgi başlangıç noktasındaki işaret fonksiyonu, ofseti ve modülünü belirler.",
        6 => "Bitiş sekmesi: Çizgi bitiş noktasındaki işaret fonksiyonu, ofseti ve modülünü belirler.",
        7 => "Bağlantılar sekmesi: Elemanlar arasındaki (ör. kol-beden) bağlantı noktalarını ekler, düzenler veya siler.",
        _ => "Çizgi özellikleri penceresi.",
    }
}

/// Dedicated 1:1 original Stoll `Yaka kesim özellikleri Çizgi: 1, sol` modal dialog (`50563`, `DLG-11`).
/// Edits the neckline element's Line 0 `[Start]` (`PointEntry` with `index = 0`, `punkt_art_tag_id = 1`).
#[derive(Clone)]
pub struct NecklinePropsDialogState {
    initialized_elem: Option<usize>,
    side: LineSide,
    working_point: PointEntry,
    orig_point: PointEntry,
    selected_struct: usize,
    show_help: bool,
    resolved_cache: HashMap<String, Option<KnittingModuleInfo>>,
    texture_cache: HashMap<String, Option<TextureHandle>>,
    cached_cat_choices: Option<(i32, String, usize, Vec<(String, String)>)>,
}

impl Default for NecklinePropsDialogState {
    fn default() -> Self {
        Self {
            initialized_elem: None,
            side: LineSide::Left,
            working_point: PointEntry {
                index: 0,
                punkt_art_tag_id: 3, // Stoll `Yaka kesimi alt orta` (verified in real Stoll V-neck .shv)
                for_all_structures: ALL_STRUCTURES_NONE,
                ..PointEntry::default()
            },
            orig_point: PointEntry::default(),
            selected_struct: 0,
            show_help: false,
            resolved_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            cached_cat_choices: None,
        }
    }
}

impl NecklinePropsDialogState {
    fn resolve_display_name(&mut self, catalog: &StollCatalog, guid: &str) -> String {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == ZERO_GUID {
            return String::new();
        }
        if let Some(info) = catalog.modules_by_guid.get(&norm) {
            return info.name.clone();
        }
        if !self.resolved_cache.contains_key(&norm) {
            let loaded = catalog
                .stoll_data_root
                .as_deref()
                .and_then(|root| load_stoll_module_from_db(root, &norm));
            self.resolved_cache.insert(norm.clone(), loaded);
        }
        if let Some(Some(info)) = self.resolved_cache.get(&norm) {
            return info.name.clone();
        }
        guid.to_string()
    }

    fn get_or_load_module_texture(
        &mut self,
        ctx: &egui::Context,
        catalog: &StollCatalog,
        guid: &str,
    ) -> Option<TextureHandle> {
        let norm = normalize_guid(guid);
        if norm.is_empty() || norm == ZERO_GUID {
            return None;
        }
        if let Some(cached) = self.texture_cache.get(&norm) {
            return cached.clone();
        }
        let preview_img = catalog
            .modules_by_guid
            .get(&norm)
            .and_then(|m| m.thumbnail.clone())
            .or_else(|| {
                if !self.resolved_cache.contains_key(&norm) {
                    let loaded = catalog
                        .stoll_data_root
                        .as_deref()
                        .and_then(|root| load_stoll_module_from_db(root, &norm));
                    self.resolved_cache.insert(norm.clone(), loaded);
                }
                self.resolved_cache
                    .get(&norm)
                    .and_then(|opt| opt.as_ref())
                    .and_then(|m| m.thumbnail.clone())
            });
        let handle = preview_img.map(|img| {
            let w = img.width as usize;
            let h = img.height as usize;
            let color_img = ColorImage::from_rgba_unmultiplied([w, h], &img.rgba);
            ctx.load_texture(format!("neck_mod_bmp_{norm}"), color_img, TextureOptions::NEAREST)
        });
        self.texture_cache.insert(norm, handle.clone());
        handle
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
        catalog: &StollCatalog,
    ) {
        if !*open {
            self.initialized_elem = None;
            self.show_help = false;
            return;
        }
        let elem_idx = state.selected_elem;
        let Some(el) = doc.elements.get(elem_idx) else {
            *open = false;
            self.initialized_elem = None;
            return;
        };

        if self.initialized_elem != Some(elem_idx) {
            self.initialized_elem = Some(elem_idx);
            self.side = if el.mirrored {
                LineSide::Left
            } else {
                state.active_table_side
            };
            self.show_help = false;
            let existing = el
                .lines(self.side)
                .first()
                .and_then(|l| l.points.iter().find(|p| p.index == 0))
                .cloned();
            let pt = existing.unwrap_or_else(|| PointEntry {
                index: 0,
                punkt_art_tag_id: 3, // Stoll `Yaka kesimi alt orta`
                for_all_structures: ALL_STRUCTURES_NONE,
                ..PointEntry::default()
            });
            self.selected_struct = if is_for_all_structures(pt.for_all_structures) {
                (pt.for_all_structures as usize).min(11)
            } else {
                0
            };
            self.orig_point = pt.clone();
            self.working_point = pt;
            self.cached_cat_choices = None;
        }

        let side_tr = match self.side {
            LineSide::Left => "sol",
            LineSide::Right => "sağ",
        };
        let title = format!("Yaka kesim özellikleri Çizgi: 1, {side_tr}");
        let choices: Vec<(String, String)> = doc
            .header
            .external_local_modules
            .iter()
            .filter_map(|ext| {
                let g = normalize_guid(&ext.guid);
                if !g.is_empty() && g != ZERO_GUID {
                    Some((g, ext.name.clone()))
                } else {
                    None
                }
            })
            .collect();

        let mut close =
            !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut commit = false;

        show_modal_backdrop(ctx, "neckline_props_backdrop", false);
        let def_pos = centered_modal_pos(ctx, vec2(494.0, 504.0));

        egui::Window::new(&title)
            .id(egui::Id::new("neckline_props_modal"))
            .title_bar(false)
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .default_pos(def_pos)
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .stroke(Stroke::new(1.5_f32, colors::BORDER_DARK))
                    .shadow(modal_shadow())
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_min_width(488.0);
                if paint_win32_dialog_titlebar(ui, &title, "neck_close_x") {
                    close = true;
                }

                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        // Single Win32 tab header: `Başlama`
                        let tab_h = 22.0_f32;
                        let tab_w = 62.0_f32;
                        let (tab_bar_r, _) =
                            ui.allocate_exact_size(vec2(ui.available_width(), tab_h), Sense::hover());
                        let tab_r = Rect::from_min_size(tab_bar_r.min, vec2(tab_w, tab_h));
                        let painter = ui.painter();
                        // Bottom baseline to the right of active tab
                        painter.line_segment(
                            [
                                pos2(tab_r.right(), tab_bar_r.bottom() - 0.5),
                                pos2(tab_bar_r.right(), tab_bar_r.bottom() - 0.5),
                            ],
                            Stroke::new(1.0_f32, colors::BORDER_MID),
                        );
                        painter.rect_filled(tab_r, CornerRadius::ZERO, TAB_PANE_BG);
                        painter.line_segment(
                            [tab_r.left_top(), tab_r.right_top()],
                            Stroke::new(1.0_f32, colors::BORDER_MID),
                        );
                        painter.line_segment(
                            [tab_r.left_top(), tab_r.left_bottom()],
                            Stroke::new(1.0_f32, colors::BORDER_MID),
                        );
                        painter.line_segment(
                            [tab_r.right_top(), tab_r.right_bottom()],
                            Stroke::new(1.0_f32, colors::BORDER_MID),
                        );
                        painter.text(
                            tab_r.center(),
                            Align2::CENTER_CENTER,
                            "Başlama",
                            regular_font(11.0),
                            colors::WIN_BLUE,
                        );

                        // White Win32 tab body pane
                        Frame::NONE
                            .fill(TAB_PANE_BG)
                            .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.set_min_width(450.0);

                                // 1. Genel
                                win32_group_box_on(ui, "Genel", TAB_PANE_BG, |ui| {
                                    win32_checkbox(
                                        ui,
                                        &mut self.working_point.use_module_color,
                                        "Modül rengi kullan",
                                    );
                                });

                                ui.add_space(6.0);

                                // 2. Fonksiyon: disabled `Yaka kesimi alt orta` + V neckline badge
                                win32_group_box_on(ui, "Fonksiyon", TAB_PANE_BG, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_enabled_ui(false, |ui| {
                                            win32_combo_scope(ui, |ui| {
                                                egui::ComboBox::from_id_salt("neck_func_combo")
                                                    .width(214.0)
                                                    .selected_text("Yaka kesimi alt orta")
                                                    .show_ui(ui, |_| {});
                                            });
                                        });
                                        ui.add_space(28.0);
                                        let (badge_r, _) = ui.allocate_exact_size(
                                            vec2(64.0, 36.0),
                                            Sense::hover(),
                                        );
                                        paint_vneck_graphic_badge(ui.painter(), badge_r);
                                    });
                                });

                                ui.add_space(6.0);

                                // 3. Modül düzeni (12-structure listbox + StartVneck module combo + 128x128 preview + Ofset)
                                let mut for_all_flag =
                                    is_for_all_structures(self.working_point.for_all_structures);
                                if for_all_flag {
                                    self.selected_struct =
                                        (self.working_point.for_all_structures as usize).min(11);
                                }
                                let active_id = self.selected_struct as i32;
                                let mut cur_guid = get_or_default_module(
                                    &self.working_point.modules,
                                    active_id,
                                    for_all_flag,
                                )
                                .id;
                                let mut off_xy = self
                                    .working_point
                                    .offsets
                                    .first()
                                    .map(|o| (o.x, o.y))
                                    .unwrap_or((0, 0));
                                let mut guid_changed = false;

                                win32_group_box_on(ui, "Modül düzeni", TAB_PANE_BG, |ui| {
                                    ui.horizontal_top(|ui| {
                                        // Left column: structure icon + 12-item listbox + checkbox
                                        ui.vertical(|ui| {
                                            ui.set_width(188.0);
                                            ui.horizontal(|ui| {
                                                ui.add_space(76.0);
                                                let (icon_r, _) = ui.allocate_exact_size(
                                                    vec2(24.0, 14.0),
                                                    Sense::hover(),
                                                );
                                                paint_structure_badge_icon(ui.painter(), icon_r);
                                            });

                                            let list_h = 192.0;
                                            Frame::NONE
                                                .fill(if for_all_flag {
                                                    Color32::BLACK
                                                } else {
                                                    colors::INPUT_BG
                                                })
                                                .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                                                .inner_margin(Margin::same(1))
                                                .show(ui, |ui| {
                                                    ui.set_width(184.0);
                                                    ui.set_min_height(list_h);
                                                    let row_h = list_h / 12.0;
                                                    for (idx, ks) in
                                                        KNITTING_STRUCTURES.iter().take(12).enumerate()
                                                    {
                                                        let (row_rect, resp) = ui
                                                            .allocate_exact_size(
                                                                vec2(182.0, row_h),
                                                                Sense::click(),
                                                            );
                                                        let painter = ui.painter();
                                                        if for_all_flag {
                                                            if idx == self.selected_struct {
                                                                let icon_rect = Rect::from_min_size(
                                                                    row_rect.min,
                                                                    vec2(16.0, row_rect.height()),
                                                                );
                                                                let label_rect = Rect::from_min_max(
                                                                    pos2(
                                                                        row_rect.left() + 16.0,
                                                                        row_rect.top(),
                                                                    ),
                                                                    row_rect.max,
                                                                );
                                                                painter.rect_filled(
                                                                    icon_rect,
                                                                    CornerRadius::ZERO,
                                                                    Color32::WHITE,
                                                                );
                                                                painter.rect_filled(
                                                                    label_rect,
                                                                    CornerRadius::ZERO,
                                                                    colors::WIN_BLUE,
                                                                );
                                                                let c = icon_rect.center();
                                                                let g_stroke = Stroke::new(
                                                                    1.8_f32,
                                                                    Color32::from_rgb(0x00, 0xA8, 0x00),
                                                                );
                                                                painter.line_segment(
                                                                    [
                                                                        pos2(c.x - 4.0, c.y),
                                                                        pos2(c.x - 1.0, c.y + 3.5),
                                                                    ],
                                                                    g_stroke,
                                                                );
                                                                painter.line_segment(
                                                                    [
                                                                        pos2(c.x - 1.0, c.y + 3.5),
                                                                        pos2(c.x + 4.5, c.y - 4.0),
                                                                    ],
                                                                    g_stroke,
                                                                );
                                                                painter.text(
                                                                    pos2(
                                                                        label_rect.left() + 3.0,
                                                                        label_rect.center().y,
                                                                    ),
                                                                    Align2::LEFT_CENTER,
                                                                    ks.name_tr,
                                                                    regular_font(10.5),
                                                                    Color32::WHITE,
                                                                );
                                                            } else {
                                                                painter.rect_filled(
                                                                    row_rect,
                                                                    CornerRadius::ZERO,
                                                                    Color32::BLACK,
                                                                );
                                                                let cx = row_rect.left() + 8.0;
                                                                let cy = row_rect.center().y;
                                                                let r_stroke = Stroke::new(
                                                                    1.7_f32,
                                                                    Color32::from_rgb(0xFF, 0x18, 0x18),
                                                                );
                                                                painter.line_segment(
                                                                    [
                                                                        pos2(cx - 4.0, cy - 4.0),
                                                                        pos2(cx + 4.0, cy + 4.0),
                                                                    ],
                                                                    r_stroke,
                                                                );
                                                                painter.line_segment(
                                                                    [
                                                                        pos2(cx - 4.0, cy + 4.0),
                                                                        pos2(cx + 4.0, cy - 4.0),
                                                                    ],
                                                                    r_stroke,
                                                                );
                                                            }
                                                        } else {
                                                            let is_sel =
                                                                self.selected_struct == idx;
                                                            let bg = if is_sel {
                                                                colors::WIN_BLUE
                                                            } else if resp.hovered() {
                                                                colors::WIN_BLUE_HOVER
                                                            } else {
                                                                colors::INPUT_BG
                                                            };
                                                            let fg = if is_sel {
                                                                Color32::WHITE
                                                            } else {
                                                                colors::TEXT_BLACK
                                                            };
                                                            painter.rect_filled(
                                                                row_rect,
                                                                CornerRadius::ZERO,
                                                                bg,
                                                            );
                                                            painter.text(
                                                                pos2(
                                                                    row_rect.left() + 4.0,
                                                                    row_rect.center().y,
                                                                ),
                                                                Align2::LEFT_CENTER,
                                                                ks.name_tr,
                                                                regular_font(10.5),
                                                                fg,
                                                            );
                                                            if resp.clicked()
                                                                && self.selected_struct != idx
                                                            {
                                                                self.selected_struct = idx;
                                                                ctx.request_repaint();
                                                            }
                                                        }
                                                    }
                                                });

                                            ui.add_space(5.0);
                                            win32_checkbox(
                                                ui,
                                                &mut for_all_flag,
                                                "Tüm örgü türleri için geçerlidir",
                                            );
                                        });

                                        ui.add_space(14.0);

                                        // Right column: Modül combo (StartVneck) + 128x128 preview + Ofset
                                        ui.vertical(|ui| {
                                            ui.set_min_width(230.0);
                                            ui.label("Modül:");

                                            let selected_label =
                                                self.resolve_display_name(catalog, &cur_guid);
                                            let eff_struct = self.selected_struct as i32;
                                            let norm_cur = normalize_guid(&cur_guid);
                                            let cat_choices = if let Some((
                                                s_id,
                                                ref g_key,
                                                ch_len,
                                                ref list,
                                            )) = self.cached_cat_choices
                                            {
                                                if s_id == eff_struct
                                                    && *g_key == norm_cur
                                                    && ch_len == choices.len()
                                                {
                                                    list.clone()
                                                } else {
                                                    let built = collect_category_module_choices(
                                                        catalog,
                                                        ModuleCategory::StartVneck,
                                                        eff_struct,
                                                        &choices,
                                                        &cur_guid,
                                                        &self.resolved_cache,
                                                    );
                                                    self.cached_cat_choices = Some((
                                                        eff_struct,
                                                        norm_cur,
                                                        choices.len(),
                                                        built.clone(),
                                                    ));
                                                    built
                                                }
                                            } else {
                                                let built = collect_category_module_choices(
                                                    catalog,
                                                    ModuleCategory::StartVneck,
                                                    eff_struct,
                                                    &choices,
                                                    &cur_guid,
                                                    &self.resolved_cache,
                                                );
                                                self.cached_cat_choices = Some((
                                                    eff_struct,
                                                    norm_cur,
                                                    choices.len(),
                                                    built.clone(),
                                                ));
                                                built
                                            };

                                            win32_combo_scope(ui, |ui| {
                                                egui::ComboBox::from_id_salt(format!(
                                                    "neck_mod_combo_{eff_struct}"
                                                ))
                                                .width(214.0)
                                                .selected_text(&selected_label)
                                                .show_ui(ui, |ui| {
                                                    if ui
                                                        .selectable_label(
                                                            cur_guid.is_empty()
                                                                || cur_guid
                                                                    .eq_ignore_ascii_case(ZERO_GUID),
                                                            "(Modül yok)",
                                                        )
                                                        .clicked()
                                                    {
                                                        cur_guid = ZERO_GUID.to_string();
                                                        guid_changed = true;
                                                    }
                                                    for (guid, name) in &cat_choices {
                                                        let is_sel =
                                                            cur_guid.eq_ignore_ascii_case(guid);
                                                        if ui.selectable_label(is_sel, name).clicked()
                                                        {
                                                            cur_guid = guid.clone();
                                                            guid_changed = true;
                                                        }
                                                    }
                                                });
                                            });

                                            ui.add_space(8.0);

                                            // 128×128 bitmap preview
                                            let preview_size = 114.0;
                                            let tex = self.get_or_load_module_texture(
                                                ctx, catalog, &cur_guid,
                                            );
                                            let (prev_rect, _) = ui.allocate_exact_size(
                                                vec2(preview_size, preview_size),
                                                Sense::hover(),
                                            );
                                            let painter = ui.painter();
                                            painter.rect_filled(
                                                prev_rect,
                                                CornerRadius::ZERO,
                                                Color32::WHITE,
                                            );
                                            if let Some(handle) = tex {
                                                painter.image(
                                                    handle.id(),
                                                    prev_rect.shrink(1.0),
                                                    Rect::from_min_max(
                                                        pos2(0.0, 0.0),
                                                        pos2(1.0, 1.0),
                                                    ),
                                                    Color32::WHITE,
                                                );
                                            } else if !cur_guid.is_empty()
                                                && !cur_guid.eq_ignore_ascii_case(ZERO_GUID)
                                            {
                                                paint_schematic_module_preview(
                                                    painter,
                                                    prev_rect.shrink(1.0),
                                                    PreviewCategory::PointStartVneck,
                                                );
                                            }
                                            painter.rect_stroke(
                                                prev_rect,
                                                CornerRadius::ZERO,
                                                Stroke::new(1.0_f32, colors::BORDER_MID),
                                                StrokeKind::Inside,
                                            );

                                            // Ofset groupbox (`↔ 0` / `↕ 0`)
                                            ui.add_space(6.0);
                                            win32_group_box_on(ui, "Ofset", TAB_PANE_BG, |ui| {
                                                ui.horizontal(|ui| {
                                                    let (irx, _) = ui.allocate_exact_size(
                                                        vec2(14.0, 14.0),
                                                        Sense::hover(),
                                                    );
                                                    paint_icon(
                                                        ui.painter(),
                                                        irx,
                                                        IconKind::HorizontalArrow,
                                                        true,
                                                    );
                                                    win32_int_edit(
                                                        ui,
                                                        "neck_pt_off_x",
                                                        &mut off_xy.0,
                                                        42.0,
                                                        -999..=999,
                                                    );
                                                    ui.add_space(16.0);
                                                    let (iry, _) = ui.allocate_exact_size(
                                                        vec2(14.0, 14.0),
                                                        Sense::hover(),
                                                    );
                                                    paint_icon(
                                                        ui.painter(),
                                                        iry,
                                                        IconKind::VerticalArrow,
                                                        true,
                                                    );
                                                    win32_int_edit(
                                                        ui,
                                                        "neck_pt_off_y",
                                                        &mut off_xy.1,
                                                        42.0,
                                                        -999..=999,
                                                    );
                                                });
                                            });
                                        });
                                    });
                                });

                                self.working_point.for_all_structures =
                                    encode_for_all_structures(for_all_flag, self.selected_struct);
                                if !self.working_point.offsets.is_empty() || off_xy != (0, 0) {
                                    if self.working_point.offsets.is_empty() {
                                        self.working_point.offsets.push(PointOffset::default());
                                    }
                                    self.working_point.offsets[0].x = off_xy.0;
                                    self.working_point.offsets[0].y = off_xy.1;
                                }
                                let final_id = self.selected_struct as i32;
                                if guid_changed
                                    || (for_all_flag
                                        && !self
                                            .working_point
                                            .modules
                                            .iter()
                                            .any(|m| m.module_art_id == final_id))
                                {
                                    let m = get_or_insert_module(
                                        &mut self.working_point.modules,
                                        final_id,
                                    );
                                    if guid_changed {
                                        m.id = cur_guid;
                                    }
                                }
                            });

                        if self.show_help {
                            ui.add_space(6.0);
                            Frame::NONE
                                .fill(Color32::from_rgb(0xFF, 0xFF, 0xE6))
                                .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                                .inner_margin(Margin::symmetric(8, 5))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        egui::RichText::new(
                                            "Yaka kesimi alt orta başlangıç noktası (Çizgi 1) için örgü türüne göre başlama modülü ve X/Y ofset değerlerini ayarlar.",
                                        )
                                        .font(regular_font(10.5))
                                        .color(colors::TEXT_BLACK),
                                    );
                                });
                        }

                        ui.add_space(8.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if win32_button(ui, "Yardım", vec2(74.0, 23.0), true).clicked() {
                                self.show_help = !self.show_help;
                            }
                            if win32_button(ui, "İptal", vec2(74.0, 23.0), true).clicked() {
                                close = true;
                            }
                            let ok_btn = egui::Button::new("Tamam")
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.5_f32, colors::WIN_BLUE))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(74.0, 23.0), ok_btn).clicked() {
                                commit = true;
                                close = true;
                            }
                        });
                    });
            });

        if commit {
            self.working_point.index = 0;
            self.working_point.punkt_art_tag_id = 3; // Stoll `Yaka kesimi alt orta`
            let has_line0 = doc
                .elements
                .get(elem_idx)
                .and_then(|el| el.lines(self.side).first())
                .is_some();
            if !has_line0 {
                state.set_status("Yaka kesiminde henüz başlangıç çizgisi yok.", true);
            } else if self.working_point != self.orig_point {
                if self.working_point.offsets.is_empty() {
                    self.working_point.offsets.push(PointOffset::default());
                }
                undo.push("Yaka kesim özellikleri", doc);
                if self.working_point.punkt_tag_id <= 0 {
                    self.working_point.punkt_tag_id = doc.next_point_tag_id();
                }
                let mut mirrored = false;
                if let Some(el) = doc.elements.get_mut(elem_idx) {
                    mirrored = el.mirrored;
                    if let Some(line0) = el.lines_mut(self.side).first_mut() {
                        if let Some(pos) = line0.points.iter().position(|p| p.index == 0) {
                            line0.points[pos] = self.working_point.clone();
                        } else {
                            let new_idx = line0.points.len();
                            line0.points.push(self.working_point.clone());
                            line0.ensure_child_slot(KanteChildSlot::Point(new_idx));
                        }
                        doc.dirty = true;
                    }
                }
                if mirrored {
                    sync_mirrored_right_lines(doc, elem_idx);
                }
                state.set_status("Yaka kesim özellikleri güncellendi.", false);
            }
        }

        if close {
            *open = false;
            self.initialized_elem = None;
            self.show_help = false;
        }
    }
}

/// Clean metallic oval + `V` neckline graphic badge matching original Stoll `Yaka kesim özellikleri`.
fn paint_vneck_graphic_badge(painter: &egui::Painter, rect: Rect) {
    let c = rect.center();
    let rx = rect.width() * 0.46;
    let ry = rect.height() * 0.44;
    // Soft gray backdrop rectangle
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(0xD4, 0xD4, 0xD4));
    // Layered elliptical shading to suggest Stoll's 3D silver oval
    let steps = 28usize;
    for (scale_f, col) in [
        (1.0_f32, Color32::from_rgb(0x88, 0x88, 0x88)),
        (0.92_f32, Color32::from_rgb(0xBC, 0xBC, 0xBC)),
        (0.76_f32, Color32::from_rgb(0xE8, 0xE8, 0xE8)),
        (0.55_f32, Color32::from_rgb(0xFA, 0xFA, 0xFA)),
    ] {
        let mut pts = Vec::with_capacity(steps);
        for i in 0..steps {
            let ang = (i as f32 / steps as f32) * std::f32::consts::TAU;
            pts.push(pos2(
                c.x + ang.cos() * rx * scale_f,
                c.y + ang.sin() * ry * scale_f,
            ));
        }
        painter.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
    }
    // Crisp dark gray `V` neckline notch with horizontal shoulder arms
    let v_stroke = Stroke::new(2.4_f32, Color32::from_rgb(0x52, 0x52, 0x52));
    let y_top = c.y - ry * 0.42;
    let y_bot = c.y + ry * 0.56;
    let arm_out = rx * 0.70;
    let arm_in = rx * 0.34;
    let pts = vec![
        pos2(c.x - arm_out, y_top),
        pos2(c.x - arm_in, y_top),
        pos2(c.x, y_bot),
        pos2(c.x + arm_in, y_top),
        pos2(c.x + arm_out, y_top),
    ];
    painter.add(egui::Shape::line(pts, v_stroke));
}

/// Dedicated `Referans çizgisi seçimi` modal dialog (`DIALOG 4274`, `DLG-12`):
/// "Özellikler hangi çizgiden alınsın?" — copies module/style attributes from another line in the active element.
#[derive(Debug, Clone, Default)]
pub struct RefLineSelectDialogState {
    selected_source_idx: usize,
    initialized: bool,
}

impl RefLineSelectDialogState {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
    ) {
        if !*open {
            self.initialized = false;
            return;
        }
        let elem_idx = state.selected_elem;
        let side = state.table_mode.side();
        let Some(el) = doc.elements.get(elem_idx) else {
            *open = false;
            self.initialized = false;
            return;
        };
        let line_count = el.lines(side).len();
        if line_count == 0 {
            *open = false;
            self.initialized = false;
            return;
        }
        if !self.initialized {
            self.initialized = true;
            self.selected_source_idx = state.primary_row().unwrap_or(0).min(line_count - 1);
        } else if self.selected_source_idx >= line_count {
            self.selected_source_idx = line_count - 1;
        }

        let mut close =
            !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut commit = false;

        show_modal_backdrop(ctx, "ref_line_select_backdrop", false);
        let def_pos = centered_modal_pos(ctx, vec2(360.0, 290.0));

        egui::Window::new("Referans çizgisi seçimi")
            .id(egui::Id::new("ref_line_select_modal"))
            .title_bar(false)
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .default_pos(def_pos)
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .stroke(Stroke::new(1.5_f32, colors::BORDER_DARK))
                    .shadow(modal_shadow())
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                if paint_win32_dialog_titlebar(ui, "Referans çizgisi seçimi", "ref_line_close_x") {
                    close = true;
                }
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.label("Özellikler hangi çizgiden alınsın?");
                        ui.add_space(6.0);

                        Frame::NONE
                            .fill(colors::INPUT_BG)
                            .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                            .inner_margin(Margin::same(2))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("ref_line_list_scroll")
                                    .max_height(180.0)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        for (i, line) in el.lines(side).iter().enumerate() {
                                            let is_sel = self.selected_source_idx == i;
                                            let fn_lbl = line.function.label_tr();
                                            let row_txt = format!(
                                                "No. {:>2}  |  Boy: {:>4}, En: {:>4}  |  {:<12}  {}",
                                                i + 1,
                                                line.rows,
                                                line.stitches,
                                                if fn_lbl.is_empty() { "-" } else { fn_lbl },
                                                line.comment,
                                            );
                                            let resp = ui.selectable_label(is_sel, row_txt);
                                            if resp.clicked() {
                                                self.selected_source_idx = i;
                                            }
                                            if resp.double_clicked() {
                                                self.selected_source_idx = i;
                                                commit = true;
                                                close = true;
                                            }
                                        }
                                    });
                            });

                        ui.add_space(8.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if win32_button(ui, "İptal et", vec2(74.0, 23.0), true).clicked() {
                                close = true;
                            }
                            let ok_btn = egui::Button::new("Tamam")
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.5_f32, colors::WIN_BLUE))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(74.0, 23.0), ok_btn).clicked() {
                                commit = true;
                                close = true;
                            }
                        });
                    });
            });

        if commit {
            let source_opt = doc
                .elements
                .get(elem_idx)
                .and_then(|e| e.lines(side).get(self.selected_source_idx))
                .cloned();
            if let Some(src_line) = source_opt {
                let target_rows = if state.selected_rows.is_empty() {
                    state.primary_row().into_iter().collect::<Vec<_>>()
                } else {
                    state.selected_rows.clone()
                };
                let effective_targets: Vec<usize> = doc
                    .elements
                    .get(elem_idx)
                    .map(|el| {
                        let closing_idx = stoll_kalip_editoru::math::closing_line_index(el, side);
                        let max_len = el.lines(side).len();
                        target_rows
                            .into_iter()
                            .filter(|&r| {
                                r != self.selected_source_idx
                                    && r < max_len
                                    && !el.is_base_line_row(side, r)
                                    && closing_idx != Some(r)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !effective_targets.is_empty() {
                    undo.push("Referans çizgi özelliklerini al", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        let lines = el.lines_mut(side);
                        for r in effective_targets {
                            if let Some(dst) = lines.get_mut(r) {
                                apply_standard_preset_to_line(dst, &src_line);
                                if state.preset_name_as_comment && !src_line.comment.is_empty() {
                                    dst.comment = src_line.comment.clone();
                                }
                            }
                        }
                        if side == LineSide::Left && el.mirrored {
                            sync_mirrored_right_lines(doc, elem_idx);
                        }
                        doc.dirty = true;
                    }
                    state.set_status(
                        format!(
                            "Çizgi {} özellikleri seçili satırlara aktarıldı.",
                            self.selected_source_idx + 1
                        ),
                        false,
                    );
                }
            }
        }

        if close {
            *open = false;
            self.initialized = false;
        }
    }
}
