use egui::{pos2, vec2, Align2, Color32, CornerRadius, Frame, Key, Margin, Rect, Sense, Stroke, Ui};

use stoll_kalip_editoru::math::{
    auto_detect_line_function_in_context, mm_to_rows, mm_to_stitches, recalc_line_from_mm,
    recalc_line_from_step_edit_for_element, recalc_line_from_stitches_and_rows, rows_to_mm,
    snapshot_endline_distances, stitches_to_mm, sync_all_endline_elements,
    sync_mirrored_right_lines,
};
use stoll_kalip_editoru::model::{
    FormDocument, InputFormat, KanteChildSlot, LineFunction, LineSegment, LineSide, PointEntry,
    PointOffset, UndoStack,
};

use crate::ui::icons::{paint_icon, paint_studio_glyph, IconKind, StudioGlyph};
use crate::ui::theme::{
    apply_pristine_zero_guard, bold_font, bold_italic_font, colors, paint_win32_3d_button,
    regular_font, studio_card_frame, win32_button,
};
use crate::ui::{
    line_function_default_tab, line_function_grid_label_tr, marker_function_label_tr, AppAction,
    SignMode, TableMode, UiState,
};

/// Column definitions for the 14-column Lines Grid (`Sol çizgiler` / `Sağ çizgiler`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineCol {
    No = 0,
    LineEditor = 1,
    Rows = 2,
    Stitches = 3,
    StepY = 4,
    StepX = 5,
    Factor = 6,
    RestY = 7,
    RestX = 8,
    WidthNarrow = 9,
    WidthFadeOut = 10,
    Function = 11,
    Group = 12,
    Comment = 13,
}

// Tuned to fit all 14 columns cleanly inside 1024px window without clipping (`MAIN-07`, total ~776px)
const ALL_LINE_COLS: &[(LineCol, f32)] = &[
    (LineCol::No, 30.0),
    (LineCol::LineEditor, 58.0),
    (LineCol::Rows, 64.0),
    (LineCol::Stitches, 62.0),
    (LineCol::StepY, 50.0),
    (LineCol::StepX, 50.0),
    (LineCol::Factor, 44.0),
    (LineCol::RestY, 56.0),
    (LineCol::RestX, 56.0),
    (LineCol::WidthNarrow, 44.0),
    (LineCol::WidthFadeOut, 44.0),
    (LineCol::Function, 84.0),
    (LineCol::Group, 42.0),
    (LineCol::Comment, 92.0),
];

/// Responsive layout metrics for Lines and Markers grids so rows, headers,
/// column widths, and fonts scale smoothly when the window grows / maximizes.
#[derive(Clone, Copy)]
struct GridMetrics {
    font_scale: f32,
    row_scale: f32,
    header_h: f32,
    row_h: f32,
    footer_h: f32,
    cell_font_sz: f32,
    header_font_sz: f32,
    footer_font_sz: f32,
    cell_pad_x: f32,
}

fn compute_grid_metrics(avail_w: f32, ref_w: f32) -> GridMetrics {
    let ratio = (avail_w / ref_w.max(1.0)).max(1.0);
    let row_scale = (1.0 + (ratio - 1.0) * 0.65).clamp(1.0, 1.545);
    let font_scale = (1.0 + (ratio - 1.0) * 0.50).clamp(1.0, 1.375);
    GridMetrics {
        font_scale,
        row_scale,
        header_h: (28.0 * row_scale).round(),
        row_h: (22.0 * row_scale).round(),
        footer_h: (24.0 * row_scale).round(),
        cell_font_sz: 12.0 * font_scale,
        header_font_sz: 11.0 * font_scale,
        footer_font_sz: 11.5 * font_scale,
        cell_pad_x: 5.0 * font_scale,
    }
}

fn card_header_scale(avail_w: f32) -> f32 {
    (1.0 + ((avail_w - 640.0) / 500.0).max(0.0) * 0.28).clamp(1.0, 1.28)
}

fn line_col_flex_weight(col: LineCol) -> f32 {
    match col {
        LineCol::No => 0.3,
        LineCol::LineEditor => 0.5,
        LineCol::Rows | LineCol::Stitches => 1.2,
        LineCol::StepY | LineCol::StepX | LineCol::RestY | LineCol::RestX => 0.9,
        LineCol::Factor | LineCol::WidthNarrow | LineCol::WidthFadeOut | LineCol::Group => 0.7,
        LineCol::Function => 1.8,
        LineCol::Comment => 3.0,
    }
}

fn marker_col_flex_weight(idx: usize) -> f32 {
    match idx {
        0 => 0.3, // No.
        1 | 2 => 1.2, // Boy, En
        3 => 1.8, // Fonksiyon
        4 => 0.6, // Grup
        _ => 3.0, // Yorum
    }
}

fn distribute_col_widths<T: Copy>(
    cols: &[(T, f32)],
    target_w: f32,
    font_scale: f32,
    weight_fn: impl Fn(usize, T) -> f32,
) -> Vec<(T, f32)> {
    let base_total_w: f32 = cols.iter().map(|(_, w)| *w).sum();
    if cols.is_empty() || target_w <= base_total_w {
        return cols.to_vec();
    }

    let mut widths: Vec<f32> = cols.iter().map(|(_, w)| w * font_scale).collect();
    let scaled_sum: f32 = widths.iter().sum();

    if target_w < scaled_sum {
        let k = target_w / base_total_w.max(1.0);
        for (i, &(_, w)) in cols.iter().enumerate() {
            widths[i] = w * k;
        }
    } else {
        let rem = target_w - scaled_sum;
        let total_weight: f32 = cols
            .iter()
            .enumerate()
            .map(|(i, &(tag, _))| weight_fn(i, tag))
            .sum::<f32>()
            .max(1.0);
        for (i, &(tag, _)) in cols.iter().enumerate() {
            widths[i] += rem * (weight_fn(i, tag) / total_weight);
        }
    }

    let mut rounded_sum = 0.0_f32;
    let last_idx = cols.len() - 1;
    for w in widths.iter_mut().take(last_idx) {
        *w = w.round();
        rounded_sum += *w;
    }
    widths[last_idx] = (target_w - rounded_sum).max(cols[last_idx].1).round();

    cols.iter()
        .zip(widths)
        .map(|(&(tag, _), w)| (tag, w))
        .collect()
}

fn visible_line_cols(state: &UiState) -> Vec<(LineCol, f32)> {
    ALL_LINE_COLS
        .iter()
        .copied()
        .filter(|(c, _)| {
            if !state.show_detail_columns {
                matches!(
                    c,
                    LineCol::No
                        | LineCol::LineEditor
                        | LineCol::Rows
                        | LineCol::Stitches
                        | LineCol::WidthNarrow
                        | LineCol::WidthFadeOut
                        | LineCol::Function
                        | LineCol::Comment
                )
            } else if state.hide_readonly_columns {
                !matches!(c, LineCol::RestY | LineCol::RestX | LineCol::Group)
            } else {
                true
            }
        })
        .collect()
}

/// Render the Stacked Dual Tables (`Sol çizgiler` on top, `Sağ çizgiler` directly below;
/// or `Sol işaretler` + `Sağ işaretler` in Markers mode) matching Modern Light Studio layout.
pub fn show_grid_area(
    ui: &mut Ui,
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
) {
    let elem = doc.elements.get(state.selected_elem);
    let mirrored = elem.map(|e| e.mirrored).unwrap_or(true);
    let avail_h = ui.available_height();
    let gap = 6.0;

    let (top_h, bot_h) = if mirrored {
        if state.right_panel_collapsed {
            let hdr_scale = card_header_scale(ui.available_width());
            let bh = (32.0 * hdr_scale).round();
            let th = (avail_h - gap - bh).max(160.0);
            (th, bh)
        } else {
            let th = ((avail_h - gap) * 0.68).max(180.0);
            let bh = (avail_h - gap - th).max(120.0);
            (th, bh)
        }
    } else {
        let th = ((avail_h - gap) * 0.5).max(140.0);
        let bh = (avail_h - gap - th).max(140.0);
        (th, bh)
    };

    let mut queued_start_edit: Option<(LineSide, usize, usize, String)> = None;

    if state.table_mode.is_lines() {
        // ── Top Card: Sol çizgiler ───────────────────────────────────────────────
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), top_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                studio_card_frame()
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height((top_h - 14.0).max(120.0));
                        show_lines_card_header(ui, doc, undo, state, actions, LineSide::Left, false);
                        ui.add_space(4.0);
                        show_lines_grid_for_side(
                            ui,
                            doc,
                            undo,
                            state,
                            actions,
                            LineSide::Left,
                            false,
                            &mut queued_start_edit,
                        );
                    });
            },
        );

        ui.add_space(gap);

        // ── Bottom Card: Sağ çizgiler ────────────────────────────────────────────
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), bot_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                studio_card_frame()
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height((bot_h - 14.0).max(18.0));
                        show_lines_card_header(
                            ui,
                            doc,
                            undo,
                            state,
                            actions,
                            LineSide::Right,
                            mirrored,
                        );
                        if !(mirrored && state.right_panel_collapsed) {
                            ui.add_space(4.0);
                            show_lines_grid_for_side(
                                ui,
                                doc,
                                undo,
                                state,
                                actions,
                                LineSide::Right,
                                mirrored,
                                &mut queued_start_edit,
                            );
                        }
                    });
            },
        );
    } else {
        // ── Top Card: Sol işaretler ──────────────────────────────────────────────
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), top_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                studio_card_frame()
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height((top_h - 14.0).max(120.0));
                        show_markers_card_header(ui, doc, undo, state, actions, LineSide::Left, false);
                        ui.add_space(4.0);
                        show_markers_grid_for_side(
                            ui,
                            doc,
                            undo,
                            state,
                            actions,
                            LineSide::Left,
                            false,
                            &mut queued_start_edit,
                        );
                    });
            },
        );

        ui.add_space(gap);

        // ── Bottom Card: Sağ işaretler ───────────────────────────────────────────
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), bot_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                studio_card_frame()
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height((bot_h - 14.0).max(18.0));
                        show_markers_card_header(ui, doc, undo, state, actions, LineSide::Right, mirrored);
                        if !(mirrored && state.right_panel_collapsed) {
                            ui.add_space(4.0);
                            show_markers_grid_for_side(
                                ui,
                                doc,
                                undo,
                                state,
                                actions,
                                LineSide::Right,
                                mirrored,
                                &mut queued_start_edit,
                            );
                        }
                    });
            },
        );
    }

    if let Some((eside, er, ec, init)) = queued_start_edit {
        state.editing_side = eside;
        state.editing_cell = Some((er, ec, init));
        state.editing_pristine = true;
    }
}

fn show_unit_toggle_pill(
    ui: &mut Ui,
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    state: &mut UiState,
    scale: f32,
) {
    let base_lbl = if doc.header.input_format == InputFormat::Mm {
        "mm"
    } else {
        "İlmek"
    };
    let h = (19.0 * scale).round();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (idx, (lbl, is_cm)) in [(base_lbl, false), ("cm", true)].into_iter().enumerate() {
            let active = state.grid_unit_cm == is_cm;
            let text_col = if active { colors::WIN_BLUE } else { colors::TEXT_SECONDARY };
            let font = if active {
                bold_font(10.0 * scale)
            } else {
                regular_font(10.0 * scale)
            };
            let galley = ui.painter().layout_no_wrap(lbl.to_owned(), font, text_col);
            let w = galley.size().x + 12.0 * scale;
            let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
            let fill = if active {
                colors::WIN_BLUE_FILL
            } else if resp.hovered() {
                colors::WIN_BLUE_HOVER
            } else {
                colors::INPUT_DISABLED_BG
            };
            let border = if active { colors::WIN_BLUE } else { colors::BORDER_MID };
            let cr = if idx == 0 {
                CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }
            } else {
                CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }
            };
            ui.painter().rect_filled(rect, cr, fill);
            ui.painter().rect_stroke(rect, cr, Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
            ui.painter().galley(
                pos2(rect.center().x - galley.size().x * 0.5, rect.center().y - galley.size().y * 0.5),
                galley,
                text_col,
            );
            if resp.clicked() && state.grid_unit_cm != is_cm {
                flush_active_grid_edit(doc, undo, state);
                state.grid_unit_cm = is_cm;
                state.editing_cell = None;
            }
            resp.on_hover_text(if is_cm {
                "Boy ve En değerlerini doğrudan cm olarak görüntüle / düzenle"
            } else {
                "Boy ve En değerlerini ilmek / sıra (veya dosya birimi) olarak görüntüle"
            });
        }
    });
}

fn show_collapse_toggle_button(ui: &mut Ui, collapsed: &mut bool, scale: f32) {
    let label = if *collapsed { "Göster" } else { "Gizle" };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        regular_font(10.0 * scale),
        colors::TEXT_SECONDARY,
    );
    let ic_sz = (10.0 * scale).round();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(galley.size().x + ic_sz + 12.0 * scale, (18.0 * scale).round()),
        Sense::click(),
    );
    let fill = if resp.hovered() {
        colors::WIN_BLUE_HOVER
    } else {
        colors::CARD_BG
    };
    let border = if resp.hovered() {
        colors::WIN_BLUE
    } else {
        colors::BORDER_MID
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    let glyph = if *collapsed {
        StudioGlyph::ChevronUp
    } else {
        StudioGlyph::ChevronDown
    };
    let ic_r = Rect::from_min_size(
        pos2(rect.left() + 5.0 * scale, rect.center().y - ic_sz * 0.5),
        vec2(ic_sz, ic_sz),
    );
    paint_studio_glyph(ui.painter(), ic_r, glyph, colors::TEXT_SECONDARY);
    ui.painter().galley(
        pos2(ic_r.right() + 3.0 * scale, rect.center().y - galley.size().y * 0.5),
        galley,
        colors::TEXT_SECONDARY,
    );
    if resp.clicked() {
        *collapsed = !*collapsed;
    }
    resp.on_hover_text("Aynalanmış sağ tabloyu daralt / genişlet");
}

fn grid_header_button(
    ui: &mut Ui,
    text: &str,
    tooltip: &str,
    enabled: bool,
    scale: f32,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let text_col = if enabled {
        colors::TEXT_BLACK
    } else {
        Color32::from_rgb(0x83, 0x83, 0x83)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), regular_font(10.5 * scale), text_col);
    let size = vec2(galley.size().x + 10.0 * scale, (19.0 * scale).round());
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let (fill, stroke) = if !enabled {
        (Color32::from_rgb(0xE8, 0xE8, 0xE8), Color32::from_rgb(0xCC, 0xCC, 0xCC))
    } else if resp.is_pointer_button_down_on() {
        (colors::WIN_BLUE_FILL, colors::WIN_BLUE)
    } else if resp.hovered() {
        (colors::WIN_BLUE_HOVER, colors::WIN_BLUE)
    } else {
        (Color32::from_rgb(0xE1, 0xE1, 0xE1), Color32::from_rgb(0xAD, 0xAD, 0xAD))
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        text_col,
    );
    if !tooltip.is_empty() {
        resp.clone().on_hover_text(tooltip);
    }
    resp
}

fn show_lines_card_header(
    ui: &mut Ui,
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
    side: LineSide,
    read_only_mirrored: bool,
) {
    let hdr_scale = card_header_scale(ui.available_width());
    let title = match side {
        LineSide::Left => "Sol çizgiler",
        LineSide::Right => "Sağ çizgiler",
    };
    let is_active = state.active_table_side == side;
    let elem = doc.elements.get(state.selected_elem);
    let elem_mirrored = elem.map(|e| e.mirrored).unwrap_or(true);
    let line_count = elem.map(|e| e.lines(side).len()).unwrap_or(0);
    let has_deletable = is_active
        && !read_only_mirrored
        && !state.selected_rows.is_empty()
        && elem
            .map(|e| {
                state
                    .selected_rows
                    .iter()
                    .any(|&r| r < line_count && !e.is_base_line_row(side, r))
            })
            .unwrap_or(false);
    let has_selected =
        is_active && state.primary_row().map(|r| r < line_count).unwrap_or(false);
    let has_copied = !state.copied_lines.is_empty();

    let activate_lines_side = |st: &mut UiState| {
        st.active_table_side = side;
        st.table_mode = if side == LineSide::Left {
            TableMode::LeftLines
        } else {
            TableMode::RightLines
        };
    };

    let render_action_buttons = |ui: &mut Ui, state: &mut UiState, actions: &mut Vec<AppAction>| {
        ui.spacing_mut().item_spacing.x = 2.5 * hdr_scale;
        if side == LineSide::Left
            && !elem_mirrored
            && swap_sides_button(ui, hdr_scale).clicked()
        {
            actions.push(AppAction::SwapLeftRight);
        }
        if grid_header_button(ui, "Kapanış", "Konturu merkeze (dX=0) kapat", true, hdr_scale)
            .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::CreateClosingLine);
        }
        if grid_header_button(
            ui,
            "Yapıştır",
            "Seçili satırın üzerine yapıştır / değiştir (Ctrl+V)",
            has_copied,
            hdr_scale,
        )
        .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::PasteReplaceLines);
        }
        if grid_header_button(
            ui,
            "Ekle+Yapıştır",
            "Kopyalanan satırları seçili satırdan önce ekle (Ctrl+Shift+V)",
            has_copied,
            hdr_scale,
        )
        .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::PasteInsertLines);
        }
        if grid_header_button(
            ui,
            "Kopyala",
            "Seçili satırları kopyala (Ctrl+C)",
            has_selected,
            hdr_scale,
        )
        .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::CopySelectedLines);
        }
        if grid_header_button(
            ui,
            "Sil",
            "Seçili satırları sil (Delete)",
            has_deletable,
            hdr_scale,
        )
        .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::DeleteSelectedLines);
        }
        if grid_header_button(
            ui,
            "+ Öne",
            "Seçili satırdan önce yeni satır ekle (Insert)",
            true,
            hdr_scale,
        )
        .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::InsertLineBefore);
        }
        if grid_header_button(
            ui,
            "+ Sona",
            "Tablonun sonuna yeni satır ekle",
            true,
            hdr_scale,
        )
        .clicked()
        {
            activate_lines_side(state);
            actions.push(AppAction::AppendLine);
        }
    };

    let avail_w = ui.available_width();
    let wrap_actions = !read_only_mirrored && avail_w < 660.0;

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0 * hdr_scale;
            let title_col = if is_active {
                colors::WIN_BLUE
            } else {
                colors::TEXT_BLACK
            };
            ui.label(
                egui::RichText::new(title)
                    .font(bold_font(12.0 * hdr_scale))
                    .color(title_col),
            );

            if side == LineSide::Left {
                show_unit_toggle_pill(ui, doc, undo, state, hdr_scale);
            }

            if read_only_mirrored {
                let badge_txt = "Aynalanmış (Sol ile senkron)";
                let galley = ui.painter().layout_no_wrap(
                    badge_txt.to_owned(),
                    regular_font(10.0 * hdr_scale),
                    colors::WIN_BLUE,
                );
                let ic_sz = (12.0 * hdr_scale).round();
                let (rect, _) = ui.allocate_exact_size(
                    vec2(galley.size().x + ic_sz + 14.0 * hdr_scale, (18.0 * hdr_scale).round()),
                    Sense::hover(),
                );
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(4), colors::WIN_BLUE_FILL);
                let ic_r = Rect::from_min_size(
                    pos2(rect.left() + 6.0 * hdr_scale, rect.center().y - ic_sz * 0.5),
                    vec2(ic_sz, ic_sz),
                );
                paint_studio_glyph(ui.painter(), ic_r, StudioGlyph::Mirror, colors::WIN_BLUE);
                ui.painter().galley(
                    pos2(ic_r.right() + 4.0 * hdr_scale, rect.center().y - galley.size().y * 0.5),
                    galley,
                    colors::WIN_BLUE,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    show_collapse_toggle_button(ui, &mut state.right_panel_collapsed, hdr_scale);
                });
            } else if !wrap_actions {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_action_buttons(ui, state, actions);
                });
            }
        });

        if wrap_actions {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_action_buttons(ui, state, actions);
                });
            });
        }
    });
}

fn swap_sides_button(ui: &mut Ui, scale: f32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        "Değiştir".to_owned(),
        regular_font(10.5 * scale),
        colors::TEXT_BLACK,
    );
    let ic_sz = (12.0 * scale).round();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(galley.size().x + ic_sz + 12.0 * scale, (19.0 * scale).round()),
        Sense::click(),
    );
    let (fill, stroke) = if resp.is_pointer_button_down_on() {
        (colors::WIN_BLUE_FILL, colors::WIN_BLUE)
    } else if resp.hovered() {
        (colors::WIN_BLUE_HOVER, colors::WIN_BLUE)
    } else {
        (Color32::from_rgb(0xE1, 0xE1, 0xE1), Color32::from_rgb(0xAD, 0xAD, 0xAD))
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );
    let ic_r = Rect::from_min_size(
        pos2(rect.left() + 5.0 * scale, rect.center().y - ic_sz * 0.5),
        vec2(ic_sz, ic_sz),
    );
    paint_studio_glyph(ui.painter(), ic_r, StudioGlyph::Swap, colors::TEXT_BLACK);
    ui.painter().galley(
        pos2(ic_r.right() + 3.0 * scale, rect.center().y - galley.size().y * 0.5),
        galley,
        colors::TEXT_BLACK,
    );
    resp.on_hover_text("Sol ve sağ çizgileri karşılıklı değiştir")
}

fn show_markers_card_header(
    ui: &mut Ui,
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
    side: LineSide,
    read_only_mirrored: bool,
) {
    let hdr_scale = card_header_scale(ui.available_width());
    let title = match side {
        LineSide::Left => "Sol işaretler",
        LineSide::Right => "Sağ işaretler",
    };
    let is_active = state.active_table_side == side;
    let has_copied = !state.copied_markers.is_empty();
    let point_count = doc
        .elements
        .get(state.selected_elem)
        .map(|el| match side {
            LineSide::Left => el.left_points.len(),
            LineSide::Right => el.right_points.len(),
        })
        .unwrap_or(0);
    let has_points = point_count > 0;
    let has_selected = is_active
        && (state
            .selected_rows
            .iter()
            .any(|&r| r < point_count)
            || state.primary_row().is_some_and(|r| r < point_count));

    let activate_markers_side = |st: &mut UiState| {
        st.active_table_side = side;
        st.table_mode = if side == LineSide::Left {
            TableMode::LeftMarkers
        } else {
            TableMode::RightMarkers
        };
    };

    let render_action_buttons = |ui: &mut Ui, state: &mut UiState, actions: &mut Vec<AppAction>| {
        ui.spacing_mut().item_spacing.x = 2.5 * hdr_scale;
        if grid_header_button(
            ui,
            "Yapıştır",
            "Seçili işaretin üzerine yapıştır / değiştir (Ctrl+V)",
            has_copied,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::PasteReplaceMarkers);
        }
        if grid_header_button(
            ui,
            "Ekle+Yapıştır",
            "Kopyalanan işaretleri seçili işaretten önce ekle (Ctrl+Shift+V)",
            has_copied,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::PasteInsertMarkers);
        }
        if grid_header_button(
            ui,
            "Kopyala",
            "Seçili işaretleri kopyala (Ctrl+C)",
            has_selected && has_points,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::CopySelectedMarkers);
        }
        if grid_header_button(
            ui,
            "Çoğalt",
            "İşaretleme noktalarını çoğalt...",
            has_points,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::OpenMultiplyMarkers);
        }
        if grid_header_button(
            ui,
            "Sil",
            "Seçili işaretleri sil (Delete)",
            has_selected && has_points,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::DeleteSelectedMarker);
        }
        if grid_header_button(
            ui,
            "+ Öne",
            "Seçili işaretten önce yeni işaret ekle",
            true,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::InsertMarkerBefore);
        }
        if grid_header_button(
            ui,
            "+ Sona",
            "Tablonun sonuna yeni işaret ekle",
            true,
            hdr_scale,
        )
        .clicked()
        {
            activate_markers_side(state);
            actions.push(AppAction::AppendMarker);
        }
    };

    let avail_w = ui.available_width();
    let wrap_actions = !read_only_mirrored && avail_w < 640.0;

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0 * hdr_scale;
            let title_col = if is_active {
                colors::WIN_BLUE
            } else {
                colors::TEXT_BLACK
            };
            ui.label(
                egui::RichText::new(title)
                    .font(bold_font(12.0 * hdr_scale))
                    .color(title_col),
            );

            if side == LineSide::Left {
                show_unit_toggle_pill(ui, doc, undo, state, hdr_scale);
            }

            if read_only_mirrored {
                ui.label(
                    egui::RichText::new("(Aynalanmış)")
                        .font(regular_font(10.0 * hdr_scale))
                        .color(colors::TEXT_SECONDARY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    show_collapse_toggle_button(ui, &mut state.right_panel_collapsed, hdr_scale);
                });
            } else if !wrap_actions {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_action_buttons(ui, state, actions);
                });
            }
        });

        if wrap_actions {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_action_buttons(ui, state, actions);
                });
            });
        }
    });
}

fn show_lines_grid_for_side(
    ui: &mut Ui,
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
    side: LineSide,
    read_only_mirrored: bool,
    queued_start_edit: &mut Option<(LineSide, usize, usize, String)>,
) {
    let elem_idx = state.selected_elem;
    let is_mm = doc.header.input_format == InputFormat::Mm;
    let unit_cm = state.grid_unit_cm;
    let sign_mode = state.sign_mode;
    let is_active_side = state.active_table_side == side;

    let base_cols = visible_line_cols(state);
    let base_total_w: f32 = base_cols.iter().map(|(_, w)| *w).sum();
    let ref_w = base_total_w.max(520.0);
    let avail_w = ui.available_width();
    let avail_h = ui.available_height();
    let metrics = compute_grid_metrics(avail_w, ref_w);

    let row_count = doc
        .elements
        .get(elem_idx)
        .map(|e| e.lines(side).len())
        .unwrap_or(0);

    // Handle keyboard navigation when this side is active and not typing elsewhere
    let mut scroll_target: Option<(usize, usize)> = if is_active_side {
        handle_lines_grid_keyboard(
            ui,
            doc,
            state,
            actions,
            side,
            read_only_mirrored,
            row_count,
            base_cols.len(),
        )
    } else {
        None
    };

    // Compute closing row index (last row returning to center axis X=0, or `last_line`)
    let closing_row_idx: Option<usize> = doc.elements.get(elem_idx).and_then(|el| {
        let lines = el.lines(side);
        if lines.len() >= 2 {
            let sum_x: i32 = lines.iter().map(|l| l.stitches).sum();
            let last = lines.len() - 1;
            if sum_x == 0 || lines[last].last_line != 0 {
                return Some(last);
            }
        }
        None
    });

    let header_h = metrics.header_h;
    let row_h = metrics.row_h;
    let empty_card_h = if row_count == 0 && !read_only_mirrored {
        (86.0 * metrics.row_scale).round()
    } else {
        0.0
    };
    let footer_h = if read_only_mirrored || row_count == 0 {
        0.0
    } else {
        metrics.footer_h
    };
    let content_h = header_h + (row_count as f32) * row_h + empty_card_h + footer_h + 8.0;
    let scroll_style = &ui.spacing().scroll;
    let vscroll_reserve = if content_h > avail_h {
        scroll_style.bar_width + scroll_style.bar_inner_margin + scroll_style.bar_outer_margin + 2.0
    } else {
        2.0
    };
    let target_w = (avail_w - vscroll_reserve).floor().max(0.0);
    let visible_cols = distribute_col_widths(&base_cols, target_w, metrics.font_scale, |_, col| {
        line_col_flex_weight(col)
    });
    let total_w: f32 = visible_cols.iter().map(|(_, w)| *w).sum();

    Frame::NONE
        .fill(colors::CARD_BG)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt(("lines_grid_scroll", side as u8))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let canvas_h = (header_h + (row_count as f32) * row_h + empty_card_h + footer_h + 8.0)
                        .max(ui.available_height());
                    let canvas_w = total_w.max(ui.available_width());
                    let (grid_rect, _) =
                        ui.allocate_exact_size(vec2(canvas_w, canvas_h), Sense::hover());

                    let painter = ui.painter().clone();

                    // ── Header Row ───────────────────────────────────────────────
                    let mut x = grid_rect.left();
                    for &(col, w) in &visible_cols {
                        let cell_r = Rect::from_min_size(pos2(x, grid_rect.top()), vec2(w, header_h));
                        painter.rect_filled(cell_r, CornerRadius::ZERO, Color32::from_rgb(0xF8, 0xFA, 0xFC));
                        painter.rect_stroke(
                            cell_r,
                            CornerRadius::ZERO,
                            Stroke::new(0.8_f32, colors::BORDER_MID),
                            egui::StrokeKind::Inside,
                        );
                        let title = line_col_header(col, is_mm, unit_cm);
                        painter.text(
                            cell_r.center(),
                            Align2::CENTER_CENTER,
                            title,
                            bold_font(metrics.header_font_sz),
                            colors::TEXT_SECONDARY,
                        );
                        x += w;
                    }

                    // Mouse drag across `No.` column / grid rows for continuous multi-row selection
                    if let Some(drag_row) = update_drag_row_selection(
                        ui,
                        state,
                        side,
                        row_count,
                        grid_rect.top(),
                        grid_rect.left(),
                        header_h,
                        row_h,
                        total_w,
                    ) {
                        scroll_target = Some((drag_row, 0));
                    }

                    // ── Data Rows ────────────────────────────────────────────────
                    let mut pending_commit: Option<(usize, usize, LineCol, String, EditMove)> = None;
                    let mut pending_start_edit: Option<(usize, usize, String)> = None;
                    let mut cancel_edit = false;
                    let mut flush_pending = false;
                    let edit_init_id = ui.id().with(("lines_grid_edit_focused_cell", side as u8));
                    if state.editing_cell.is_none() || state.editing_side != side {
                        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(edit_init_id));
                    }

                    let clip_rect = ui.clip_rect();
                    let rows_top = grid_rect.top() + header_h;
                    let first_vis = (((clip_rect.top() - rows_top) / row_h).floor() as isize - 1)
                        .max(0) as usize;
                    let last_vis = ((((clip_rect.bottom() - rows_top) / row_h).ceil() as isize
                        + 1)
                        .max(0) as usize)
                        .min(row_count);
                    let active_edit_row = if state.editing_side == side && !read_only_mirrored {
                        state
                            .editing_cell
                            .as_ref()
                            .map(|(er, _, _)| *er)
                            .filter(|&er| er < row_count)
                    } else {
                        None
                    };
                    let mut rows_to_render: Vec<usize> = (first_vis..last_vis).collect();
                    if let Some(er) = active_edit_row {
                        if er < first_vis || er >= last_vis {
                            rows_to_render.push(er);
                        }
                    }

                    for r in rows_to_render {
                        let y = grid_rect.top() + header_h + (r as f32) * row_h;
                        let is_closing = closing_row_idx == Some(r);
                        let is_row_selected = is_active_side && state.selected_rows.contains(&r);

                        let Some(line_snap) = doc.elements.get(elem_idx).and_then(|e| e.lines(side).get(r)) else {
                            continue;
                        };
                        let has_curve = line_snap
                            .curve
                            .as_ref()
                            .map(|c| !c.steps.is_empty())
                            .unwrap_or(false);
                        let func_id = line_snap.function.to_id();
                        let is_base_row = doc
                            .elements
                            .get(elem_idx)
                            .map(|e| e.is_base_line_row(side, r))
                            .unwrap_or(r == 0 || func_id == 8);

                        let mut cx = grid_rect.left();
                        for (vis_ci, &(col, w)) in visible_cols.iter().enumerate() {
                            let cell_r = Rect::from_min_size(pos2(cx, y), vec2(w, row_h));
                            let is_focused = is_active_side && state.focused_cell == Some((r, vis_ci));
                            let is_editing = state.editing_side == side
                                && !read_only_mirrored
                                && matches!(
                                    state.editing_cell,
                                    Some((er, ec, _)) if er == r && ec == vis_ci
                                );

                            let editable = !read_only_mirrored
                                && is_line_cell_editable(col, has_curve, is_base_row, func_id);

                            // Exact background color rules (matching Screenshot 222001.png & #12/#28 audit!)
                            let multi_selected = is_row_selected && state.selected_rows.len() > 1;
                            let cell_bg = match col {
                                LineCol::No => {
                                    if multi_selected {
                                        Color32::from_rgb(0xBF, 0xDB, 0xFE)
                                    } else if is_row_selected {
                                        colors::ROW_SELECT_TINT
                                    } else {
                                        colors::CHROME_BG
                                    }
                                }
                                LineCol::LineEditor => colors::CELL_BTN_BG,
                                LineCol::Function => colors::INPUT_BG, // stays white even on closing row!
                                _ => {
                                    if multi_selected && !is_focused {
                                        colors::WIN_BLUE_FILL
                                    } else if is_closing {
                                        colors::CLOSING_ROW_GREEN
                                    } else if !editable {
                                        colors::CELL_READONLY_BG
                                    } else {
                                        colors::INPUT_BG
                                    }
                                }
                            };

                            if col == LineCol::LineEditor {
                                let btn_enabled = !read_only_mirrored && !is_base_row && !is_closing;
                                let resp = ui.interact(
                                    cell_r,
                                    ui.id().with(("line_ed_btn", elem_idx, side as u8, r)),
                                    Sense::click(),
                                );
                                paint_win32_3d_button(
                                    &painter,
                                    cell_r.shrink(1.0),
                                    resp.is_pointer_button_down_on(),
                                    btn_enabled,
                                );
                                // Paint curve glyph if line has `[Kurve]`
                                if let Some(ref curve) = line_snap.curve {
                                    if !curve.steps.is_empty() {
                                        let glyph = match curve.function {
                                            1 => IconKind::CurveLine,
                                            3 => IconKind::CurveJ,
                                            4 => IconKind::CurveS,
                                            _ => IconKind::CurveUser,
                                        };
                                        paint_icon(&painter, cell_r.shrink(2.0), glyph, true);
                                    }
                                }
                                // Outer cell border
                                painter.rect_stroke(
                                    cell_r,
                                    CornerRadius::ZERO,
                                    Stroke::new(0.8_f32, colors::BORDER_MID),
                                    egui::StrokeKind::Inside,
                                );
                                if resp.clicked() && btn_enabled {
                                    state.active_table_side = side;
                                    state.table_mode = if side == LineSide::Left {
                                        TableMode::LeftLines
                                    } else {
                                        TableMode::RightLines
                                    };
                                    state.select_single_row(r);
                                    state.focused_cell = Some((r, vis_ci));
                                    actions.push(AppAction::OpenCurveEditor { line_idx: r });
                                }
                                if !read_only_mirrored {
                                    resp.context_menu(|ui| {
                                        show_line_context_menu(ui, state, actions, r, func_id, is_base_row, is_closing);
                                    });
                                }
                                cx += w;
                                continue;
                            }

                            // Paint cell fill & border (Modern Light Studio + colored Fonksiyon pill)
                            if col == LineCol::Function {
                                let outer_bg = if multi_selected {
                                    colors::WIN_BLUE_FILL
                                } else if is_closing {
                                    colors::CLOSING_ROW_GREEN
                                } else {
                                    colors::CARD_BG
                                };
                                painter.rect_filled(cell_r, CornerRadius::ZERO, outer_bg);
                                painter.rect_stroke(
                                    cell_r,
                                    CornerRadius::ZERO,
                                    Stroke::new(0.8_f32, colors::BORDER_MID),
                                    egui::StrokeKind::Inside,
                                );
                                let (pill_bg, pill_border) = function_pill_colors(func_id);
                                let inner_r = cell_r.shrink(2.0);
                                let pill_cr = CornerRadius::same((3.0 * metrics.font_scale).round() as u8);
                                painter.rect_filled(inner_r, pill_cr, pill_bg);
                                painter.rect_stroke(
                                    inner_r,
                                    pill_cr,
                                    Stroke::new(0.8_f32, pill_border),
                                    egui::StrokeKind::Inside,
                                );
                            } else {
                                painter.rect_filled(cell_r, CornerRadius::ZERO, cell_bg);
                                painter.rect_stroke(
                                    cell_r,
                                    CornerRadius::ZERO,
                                    Stroke::new(0.8_f32, colors::BORDER_MID),
                                    egui::StrokeKind::Inside,
                                );
                                if col == LineCol::No && is_row_selected {
                                    let bar = Rect::from_min_size(
                                        cell_r.min,
                                        vec2((3.0 * metrics.font_scale).round(), cell_r.height()),
                                    );
                                    painter.rect_filled(bar, CornerRadius::ZERO, colors::WIN_BLUE);
                                }
                            }

                            // Render inline editor if this cell is being edited
                            if is_editing {
                                if let Some((_, _, ref mut buf)) = state.editing_cell {
                                    let te_id = ui.id().with(("line_te", elem_idx, side as u8, r, vis_ci));
                                    match show_grid_cell_text_edit(
                                        ui,
                                        cell_r,
                                        te_id,
                                        edit_init_id,
                                        (elem_idx, side as u8, r, vis_ci),
                                        buf,
                                        &mut state.editing_pristine,
                                        col != LineCol::Comment,
                                        metrics.cell_font_sz,
                                    ) {
                                        CellEditOutcome::Cancel => {
                                            cancel_edit = true;
                                        }
                                        CellEditOutcome::Commit(mv) => {
                                            pending_commit = Some((r, vis_ci, col, buf.clone(), mv));
                                        }
                                        CellEditOutcome::Continue => {}
                                    }
                                }
                            } else {
                                // Render static text
                                let text = format_line_cell(
                                    line_snap,
                                    col,
                                    is_mm,
                                    unit_cm,
                                    sign_mode,
                                    side,
                                    has_curve,
                                    is_base_row,
                                );
                                let (align, pos) = match col {
                                    LineCol::No => (
                                        Align2::LEFT_CENTER,
                                        pos2(cell_r.left() + metrics.cell_pad_x, cell_r.center().y),
                                    ),
                                    LineCol::Function => (Align2::CENTER_CENTER, cell_r.center()),
                                    LineCol::Comment => (
                                        Align2::LEFT_CENTER,
                                        pos2(cell_r.left() + metrics.cell_pad_x, cell_r.center().y),
                                    ),
                                    _ => (
                                        Align2::RIGHT_CENTER,
                                        pos2(cell_r.right() - metrics.cell_pad_x, cell_r.center().y),
                                    ),
                                };
                                let font = if col == LineCol::Function {
                                    if line_snap.user_set_function {
                                        bold_italic_font(metrics.cell_font_sz)
                                    } else {
                                        bold_font(metrics.cell_font_sz)
                                    }
                                } else if col == LineCol::No && is_row_selected {
                                    bold_font(metrics.cell_font_sz)
                                } else {
                                    regular_font(metrics.cell_font_sz)
                                };
                                let text_col = if col == LineCol::No && multi_selected {
                                    Color32::from_rgb(0x1E, 0x3A, 0x8A)
                                } else {
                                    colors::TEXT_BLACK
                                };
                                if !text.is_empty() {
                                    painter
                                        .with_clip_rect(cell_r.shrink(1.0))
                                        .text(pos, align, text, font, text_col);
                                }

                                // Interaction (`#48.1` & `#48.2` + Bug #4 multi-row selection):
                                let sense = if col == LineCol::No {
                                    Sense::click_and_drag()
                                } else {
                                    Sense::click()
                                };
                                let resp = ui.interact(
                                    cell_r,
                                    ui.id().with(("line_cell", elem_idx, side as u8, r, vis_ci)),
                                    sense,
                                );

                                if col == LineCol::No {
                                    let pressed = ui.input(|i| {
                                        i.pointer.button_pressed(egui::PointerButton::Primary)
                                    });
                                    if pressed && resp.hovered() {
                                        state.active_table_side = side;
                                        if !read_only_mirrored {
                                            state.table_mode = if side == LineSide::Left {
                                                TableMode::LeftLines
                                            } else {
                                                TableMode::RightLines
                                            };
                                        }
                                        let modifiers = ui.input(|i| i.modifiers);
                                        flush_pending = true;
                                        handle_no_col_press(state, side, r, vis_ci, modifiers);
                                    }
                                } else if resp.clicked() {
                                    state.active_table_side = side;
                                    if !read_only_mirrored {
                                        state.table_mode = if side == LineSide::Left {
                                            TableMode::LeftLines
                                        } else {
                                            TableMode::RightLines
                                        };
                                    }
                                    let modifiers = ui.input(|i| i.modifiers);
                                    if modifiers.shift {
                                        let anchor = state
                                            .row_select_anchor
                                            .or_else(|| state.selected_rows.first().copied())
                                            .unwrap_or(r);
                                        let (lo, hi) = if anchor <= r { (anchor, r) } else { (r, anchor) };
                                        state.selected_rows = (lo..=hi).collect();
                                        state.focused_cell = Some((r, vis_ci));
                                        flush_pending = true;
                                    } else if modifiers.ctrl {
                                        toggle_row_selection(state, r);
                                        state.focused_cell = Some((r, vis_ci));
                                        flush_pending = true;
                                    } else {
                                        state.select_single_row(r);
                                        state.focused_cell = Some((r, vis_ci));
                                        if col == LineCol::Function && !read_only_mirrored {
                                            actions.push(AppAction::OpenLineProps {
                                                line_idx: r,
                                                initial_tab: line_function_default_tab(func_id),
                                            });
                                        } else if editable {
                                            let init = format_line_cell(
                                                line_snap,
                                                col,
                                                is_mm,
                                                unit_cm,
                                                sign_mode,
                                                side,
                                                has_curve,
                                                is_base_row,
                                            );
                                            pending_start_edit = Some((r, vis_ci, init));
                                        }
                                    }
                                }
                                if resp.double_clicked() && !editable && col != LineCol::Function && !read_only_mirrored {
                                    state.active_table_side = side;
                                    state.select_single_row(r);
                                    state.focused_cell = Some((r, vis_ci));
                                    let tab = match col {
                                        LineCol::WidthNarrow => {
                                            if func_id == 2 { 2 } else { 1 }
                                        }
                                        LineCol::WidthFadeOut => 4,
                                        _ => line_function_default_tab(func_id),
                                    };
                                    actions.push(AppAction::OpenLineProps {
                                        line_idx: r,
                                        initial_tab: tab,
                                    });
                                }

                                // Right-click context menu (`MENU 4278`)
                                if !read_only_mirrored {
                                    resp.context_menu(|ui| {
                                        show_line_context_menu(ui, state, actions, r, func_id, is_base_row, is_closing);
                                    });
                                }
                            }

                            // Crisp focus rectangle (Screenshot 222026.png Row 3)
                            if is_focused {
                                painter.rect_stroke(
                                    cell_r.shrink(0.5),
                                    CornerRadius::ZERO,
                                    Stroke::new(2.0_f32, colors::CELL_FOCUS_BLUE),
                                    egui::StrokeKind::Inside,
                                );
                            }

                            cx += w;
                        }
                    }

                    // ── Empty-state Quick Start OR Inline `+ Satır Ekle` footer ──
                    if !read_only_mirrored && row_count == 0 {
                        let card_w = (total_w.min(canvas_w) - 16.0).max(280.0);
                        let card_h = (68.0 * metrics.row_scale).round();
                        let card_r = Rect::from_min_size(
                            pos2(grid_rect.left() + 8.0, grid_rect.top() + header_h + 10.0),
                            vec2(card_w, card_h),
                        );
                        painter.rect_filled(card_r, CornerRadius::same(6), Color32::from_rgb(0xF8, 0xFA, 0xFC));
                        painter.rect_stroke(
                            card_r,
                            CornerRadius::same(6),
                            Stroke::new(1.0_f32, colors::BORDER_MID),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            pos2(card_r.left() + 12.0, card_r.top() + 16.0 * metrics.row_scale),
                            Align2::LEFT_CENTER,
                            "Henüz çizgi yok — hızlı başlangıç için seçin:",
                            regular_font(metrics.cell_font_sz),
                            colors::TEXT_SECONDARY,
                        );
                        let mut child_ui = ui.new_child(
                            egui::UiBuilder::new().max_rect(Rect::from_min_max(
                                pos2(card_r.left() + 12.0, card_r.top() + 30.0 * metrics.row_scale),
                                pos2(card_r.right() - 12.0, card_r.bottom() - 8.0),
                            )),
                        );
                        child_ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0 * metrics.font_scale;
                            let btn_h = (24.0 * metrics.row_scale).round();
                            if win32_button(
                                ui,
                                "+ Temel Çizgi Ekle",
                                vec2(126.0 * metrics.font_scale, btn_h),
                                true,
                            )
                            .clicked()
                            {
                                state.active_table_side = side;
                                state.table_mode = if side == LineSide::Left {
                                    TableMode::LeftLines
                                } else {
                                    TableMode::RightLines
                                };
                                actions.push(AppAction::QuickStartTemel);
                            }
                            if win32_button(
                                ui,
                                "+ Dikdörtgen Kalıp Taslağı",
                                vec2(166.0 * metrics.font_scale, btn_h),
                                true,
                            )
                            .clicked()
                            {
                                state.active_table_side = side;
                                state.table_mode = if side == LineSide::Left {
                                    TableMode::LeftLines
                                } else {
                                    TableMode::RightLines
                                };
                                actions.push(AppAction::QuickStartRectangle);
                            }
                        });
                    } else if !read_only_mirrored {
                        let fy = grid_rect.top() + header_h + (row_count as f32) * row_h;
                        let footer_w = total_w.min(canvas_w);
                        let footer_r = Rect::from_min_size(pos2(grid_rect.left(), fy), vec2(footer_w, footer_h));
                        let f_resp = ui.interact(
                            footer_r,
                            ui.id().with(("lines_add_footer", elem_idx, side as u8)),
                            Sense::click(),
                        );
                        let f_bg = if f_resp.hovered() {
                            colors::WIN_BLUE_HOVER
                        } else {
                            Color32::from_rgb(0xF8, 0xFA, 0xFC)
                        };
                        let f_txt_col = if f_resp.hovered() {
                            colors::WIN_BLUE
                        } else {
                            colors::TEXT_SECONDARY
                        };
                        painter.rect_filled(footer_r, CornerRadius::ZERO, f_bg);
                        painter.rect_stroke(
                            footer_r,
                            CornerRadius::ZERO,
                            Stroke::new(0.8_f32, colors::BORDER_MID),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            pos2(footer_r.left() + 10.0 * metrics.font_scale, footer_r.center().y),
                            Align2::LEFT_CENTER,
                            "+ Satır Ekle",
                            bold_font(metrics.footer_font_sz),
                            f_txt_col,
                        );
                        if f_resp.clicked() {
                            state.active_table_side = side;
                            state.table_mode = if side == LineSide::Left {
                                TableMode::LeftLines
                            } else {
                                TableMode::RightLines
                            };
                            actions.push(AppAction::AppendLine);
                        }
                    }

                    if flush_pending {
                        flush_active_grid_edit(doc, undo, state);
                    }
                    if cancel_edit {
                        state.editing_cell = None;
                        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(edit_init_id));
                    } else if let Some((row_i, col_i, col, new_val, move_kind)) = pending_commit {
                        commit_line_cell_edit(
                            doc,
                            undo,
                            elem_idx,
                            side,
                            row_i,
                            col,
                            &new_val,
                            sign_mode,
                            unit_cm,
                        );
                        state.editing_cell = None;
                        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(edit_init_id));
                        if pending_start_edit.is_none() {
                            let pending_scroll_id = ui.id().with(("lines_pending_scroll", side as u8));
                            match move_kind {
                                EditMove::Stay => {}
                                EditMove::Tab(forward) => {
                                    let (next_r, next_c) = next_editable_line_cell(
                                        doc,
                                        elem_idx,
                                        side,
                                        &visible_cols,
                                        row_i,
                                        col_i,
                                        forward,
                                    );
                                    state.focused_cell = Some((next_r, next_c));
                                    if next_r != row_i {
                                        state.select_single_row(next_r);
                                    }
                                    ui.ctx().data_mut(|d| d.insert_temp(pending_scroll_id, (next_r, next_c)));
                                }
                                EditMove::Delta(dr, dc) => {
                                    let nr = (row_i as i32 + dr)
                                        .clamp(0, row_count.saturating_sub(1) as i32)
                                        as usize;
                                    let nc = (col_i as i32 + dc)
                                        .clamp(0, visible_cols.len().saturating_sub(1) as i32)
                                        as usize;
                                    state.focused_cell = Some((nr, nc));
                                    if nr != row_i {
                                        state.select_single_row(nr);
                                    }
                                    ui.ctx().data_mut(|d| d.insert_temp(pending_scroll_id, (nr, nc)));
                                }
                            }
                        }
                    }
                    if let Some((sr, sc, init)) = pending_start_edit {
                        *queued_start_edit = Some((side, sr, sc, init));
                        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(edit_init_id));
                    }
                    let pending_scroll_id = ui.id().with(("lines_pending_scroll", side as u8));
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
                        let col_x: f32 = visible_cols.iter().take(sc).map(|(_, w)| *w).sum();
                        let col_w = visible_cols.get(sc).map(|(_, w)| *w).unwrap_or(40.0);
                        let target_r = Rect::from_min_size(
                            pos2(grid_rect.left() + col_x, grid_rect.top() + header_h + (sr as f32) * row_h),
                            vec2(col_w, row_h),
                        );
                        ui.scroll_to_rect(target_r, None);
                    }
                });
        });
}

fn show_markers_grid_for_side(
    ui: &mut Ui,
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
    side: LineSide,
    read_only_mirrored: bool,
    queued_start_edit: &mut Option<(LineSide, usize, usize, String)>,
) {
    let elem_idx = state.selected_elem;
    let is_mm = doc.header.input_format == InputFormat::Mm;
    let unit_cm = state.grid_unit_cm;
    let hdr = doc.header.clone();
    let is_active_side = state.active_table_side == side;

    let boy_hdr = if unit_cm {
        "Boy\ncm"
    } else if is_mm {
        "Boy\nmm"
    } else {
        "Boy"
    };
    let en_hdr = if unit_cm {
        "En\ncm"
    } else if is_mm {
        "En\nmm"
    } else {
        "En"
    };
    let base_cols: &[(&str, f32)] = &[
        ("No.", 36.0),
        (boy_hdr, 76.0),
        (en_hdr, 76.0),
        ("Fonksiyon", 160.0),
        ("Grup", 52.0),
        ("Yorum", 140.0),
    ];

    let points_len = doc
        .elements
        .get(elem_idx)
        .map(|el| match side {
            LineSide::Left => el.left_points.len(),
            LineSide::Right => el.right_points.len(),
        })
        .unwrap_or(0);

    // Keyboard navigation & direct typing for Markers Grid
    let mut scroll_target: Option<(usize, usize)> = if is_active_side {
        let pts_slice = doc
            .elements
            .get(elem_idx)
            .map(|el| match side {
                LineSide::Left => el.left_points.as_slice(),
                LineSide::Right => el.right_points.as_slice(),
            })
            .unwrap_or(&[]);
        handle_markers_grid_keyboard(
            ui,
            &hdr,
            pts_slice,
            state,
            side,
            read_only_mirrored,
            base_cols.len(),
        )
    } else {
        None
    };

    let base_total_w: f32 = base_cols.iter().map(|(_, w)| *w).sum();
    let avail_w = ui.available_width();
    let avail_h = ui.available_height();
    let metrics = compute_grid_metrics(avail_w, base_total_w.max(520.0));
    let header_h = metrics.header_h;
    let row_h = metrics.row_h;
    let footer_h = if read_only_mirrored { 0.0 } else { metrics.footer_h };
    let content_h = header_h + (points_len as f32) * row_h + footer_h + 16.0;
    let scroll_style = &ui.spacing().scroll;
    let vscroll_reserve = if content_h > avail_h {
        scroll_style.bar_width + scroll_style.bar_inner_margin + scroll_style.bar_outer_margin + 2.0
    } else {
        2.0
    };
    let target_w = (avail_w - vscroll_reserve).floor().max(0.0);
    let cols = distribute_col_widths(base_cols, target_w, metrics.font_scale, |idx, _| {
        marker_col_flex_weight(idx)
    });
    let total_w: f32 = cols.iter().map(|(_, w)| *w).sum();

    let mut pending_marker_commit: Option<(usize, usize, String, EditMove)> = None;
    let mut pending_marker_start_edit: Option<(usize, usize, String)> = None;
    let mut pending_kind_change: Option<(usize, i32)> = None;
    let mut cancel_edit = false;
    let mut flush_pending = false;
    let marker_edit_init_id = ui.id().with(("markers_grid_edit_focused_cell", side as u8));
    if state.editing_cell.is_none() || state.editing_side != side {
        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(marker_edit_init_id));
    }

    Frame::NONE
        .fill(colors::CARD_BG)
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt(("markers_grid_scroll", side as u8))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let canvas_h = (header_h + (points_len as f32) * row_h + footer_h + 16.0)
                        .max(ui.available_height());
                    let canvas_w = total_w.max(ui.available_width());
                    let (grid_rect, _) =
                        ui.allocate_exact_size(vec2(canvas_w, canvas_h), Sense::hover());
                    let painter = ui.painter().clone();

                    if let Some(drag_row) = update_drag_row_selection(
                        ui,
                        state,
                        side,
                        points_len,
                        grid_rect.top(),
                        grid_rect.left(),
                        header_h,
                        row_h,
                        total_w,
                    ) {
                        scroll_target = Some((drag_row, 0));
                    }

                    // Header row
                    let mut x = grid_rect.left();
                    for &(title, w) in &cols {
                        let cell_r = Rect::from_min_size(pos2(x, grid_rect.top()), vec2(w, header_h));
                        painter.rect_filled(cell_r, CornerRadius::ZERO, colors::CHROME_BG);
                        painter.rect_stroke(
                            cell_r,
                            CornerRadius::ZERO,
                            Stroke::new(1.0_f32, colors::BORDER_BLACK),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            cell_r.center(),
                            Align2::CENTER_CENTER,
                            title,
                            bold_font(metrics.header_font_sz),
                            colors::TEXT_BLACK,
                        );
                        x += w;
                    }

                    // Data rows (viewport-virtualized + active editing row)
                    let pts_slice = doc
                        .elements
                        .get(elem_idx)
                        .map(|el| match side {
                            LineSide::Left => el.left_points.as_slice(),
                            LineSide::Right => el.right_points.as_slice(),
                        })
                        .unwrap_or(&[]);
                    let clip_rect = ui.clip_rect();
                    let rows_top = grid_rect.top() + header_h;
                    let first_vis = (((clip_rect.top() - rows_top) / row_h).floor() as isize - 1)
                        .max(0) as usize;
                    let last_vis = ((((clip_rect.bottom() - rows_top) / row_h).ceil() as isize
                        + 1)
                        .max(0) as usize)
                        .min(points_len);
                    let active_edit_row = if state.editing_side == side && !read_only_mirrored {
                        state
                            .editing_cell
                            .as_ref()
                            .map(|(er, _, _)| *er)
                            .filter(|&er| er < points_len)
                    } else {
                        None
                    };
                    let mut rows_to_render: Vec<usize> = (first_vis..last_vis).collect();
                    if let Some(er) = active_edit_row {
                        if er < first_vis || er >= last_vis {
                            rows_to_render.push(er);
                        }
                    }

                    for r in rows_to_render {
                        let Some(pt) = pts_slice.get(r) else {
                            continue;
                        };
                        let y = grid_rect.top() + header_h + (r as f32) * row_h;
                        let selected = is_active_side && state.selected_rows.contains(&r);
                        let multi_selected = selected && state.selected_rows.len() > 1;
                        let off = pt.offsets.first();
                        let px = off.map(|o| o.x).unwrap_or(0);
                        let py = off.map(|o| o.y).unwrap_or(0);

                        let y_str = if unit_cm {
                            format_cm_val(rows_to_mm(py, &hdr) / 10.0)
                        } else if is_mm {
                            format!("{:.1}", rows_to_mm(py, &hdr))
                        } else {
                            py.to_string()
                        };
                        let x_str = if unit_cm {
                            format_cm_val(stitches_to_mm(px, &hdr) / 10.0)
                        } else if is_mm {
                            format!("{:.1}", stitches_to_mm(px, &hdr))
                        } else {
                            px.to_string()
                        };
                        let info_str = if !pt.modules.is_empty() {
                            pt.modules[0].id.clone()
                        } else {
                            String::new()
                        };

                        let values = [
                            format!("{}", r + 1),
                            y_str,
                            x_str,
                            marker_function_label_tr(pt.punkt_art_tag_id).to_string(),
                            pt.gruppe_tag_id.to_string(),
                            info_str,
                        ];

                        let mut cx = grid_rect.left();
                        for (ci, &(_, w)) in cols.iter().enumerate() {
                            let cell_r = Rect::from_min_size(pos2(cx, y), vec2(w, row_h));
                            let is_focused = is_active_side && state.focused_cell == Some((r, ci));
                            let is_editing = state.editing_side == side
                                && !read_only_mirrored
                                && matches!(
                                    state.editing_cell,
                                    Some((er, ec, _)) if er == r && ec == ci
                                );
                            let editable = !read_only_mirrored && matches!(ci, 1 | 2 | 4);

                            let bg = match ci {
                                0 => {
                                    if multi_selected {
                                        Color32::from_rgb(0xBF, 0xDB, 0xFE)
                                    } else if selected {
                                        colors::ROW_SELECT_TINT
                                    } else {
                                        colors::CHROME_BG
                                    }
                                }
                                5 => {
                                    if multi_selected && !is_focused {
                                        colors::WIN_BLUE_FILL
                                    } else {
                                        colors::CELL_READONLY_BG
                                    }
                                }
                                _ => {
                                    if multi_selected && !is_focused {
                                        colors::WIN_BLUE_FILL
                                    } else {
                                        colors::INPUT_BG
                                    }
                                }
                            };
                            painter.rect_filled(cell_r, CornerRadius::ZERO, bg);
                            let border_col = if ci == 0 {
                                colors::BORDER_BLACK
                            } else {
                                colors::BORDER_MID
                            };
                            painter.rect_stroke(
                                cell_r,
                                CornerRadius::ZERO,
                                Stroke::new(0.8_f32, border_col),
                                egui::StrokeKind::Inside,
                            );
                            if ci == 0 && selected {
                                let bar = Rect::from_min_size(
                                    cell_r.min,
                                    vec2((3.0 * metrics.font_scale).round(), cell_r.height()),
                                );
                                painter.rect_filled(bar, CornerRadius::ZERO, colors::WIN_BLUE);
                            }

                            if ci == 3 && !read_only_mirrored {
                                // Interactive Fonksiyon selector inside the cell
                                let mut kind_id = pt.punkt_art_tag_id;
                                let mut child_ui = ui.new_child(
                                    egui::UiBuilder::new().max_rect(cell_r.shrink(1.0)),
                                );
                                {
                                    let style = child_ui.style_mut();
                                    style
                                        .text_styles
                                        .insert(egui::TextStyle::Button, regular_font(metrics.cell_font_sz));
                                    style
                                        .text_styles
                                        .insert(egui::TextStyle::Body, regular_font(metrics.cell_font_sz));
                                    style.spacing.interact_size.y = (row_h - 3.0).max(18.0);
                                }
                                egui::ComboBox::from_id_salt(("marker_kind", elem_idx, side as u8, r))
                                    .width(w - 4.0)
                                    .selected_text(marker_function_label_tr(kind_id))
                                    .show_ui(&mut child_ui, |ui| {
                                        for id in [0, 1, 2, 3] {
                                            ui.selectable_value(
                                                &mut kind_id,
                                                id,
                                                marker_function_label_tr(id),
                                            );
                                        }
                                    });
                                if kind_id != pt.punkt_art_tag_id {
                                    state.active_table_side = side;
                                    state.table_mode = if side == LineSide::Left {
                                        TableMode::LeftMarkers
                                    } else {
                                        TableMode::RightMarkers
                                    };
                                    pending_kind_change = Some((r, kind_id));
                                }
                            } else if is_editing && editable {
                                if let Some((_, _, ref mut buf)) = state.editing_cell {
                                    let te_id = ui.id().with(("marker_te", elem_idx, side as u8, r, ci));
                                    match show_grid_cell_text_edit(
                                        ui,
                                        cell_r,
                                        te_id,
                                        marker_edit_init_id,
                                        (elem_idx, side as u8, r, ci),
                                        buf,
                                        &mut state.editing_pristine,
                                        true,
                                        metrics.cell_font_sz,
                                    ) {
                                        CellEditOutcome::Cancel => {
                                            cancel_edit = true;
                                        }
                                        CellEditOutcome::Commit(mv) => {
                                            pending_marker_commit = Some((r, ci, buf.clone(), mv));
                                        }
                                        CellEditOutcome::Continue => {}
                                    }
                                }
                            } else {
                                let (align, pos) = match ci {
                                    0 => (
                                        Align2::LEFT_CENTER,
                                        pos2(cell_r.left() + metrics.cell_pad_x, cell_r.center().y),
                                    ),
                                    3 | 5 => (
                                        Align2::LEFT_CENTER,
                                        pos2(cell_r.left() + metrics.cell_pad_x, cell_r.center().y),
                                    ),
                                    _ => (
                                        Align2::RIGHT_CENTER,
                                        pos2(cell_r.right() - metrics.cell_pad_x, cell_r.center().y),
                                    ),
                                };
                                let font = if ci == 0 && selected {
                                    bold_font(metrics.cell_font_sz)
                                } else {
                                    regular_font(metrics.cell_font_sz)
                                };
                                let txt_col = if ci == 0 && multi_selected {
                                    Color32::from_rgb(0x1E, 0x3A, 0x8A)
                                } else {
                                    colors::TEXT_BLACK
                                };
                                painter
                                    .with_clip_rect(cell_r.shrink(1.0))
                                    .text(pos, align, &values[ci], font, txt_col);

                                let sense = if ci == 0 {
                                    Sense::click_and_drag()
                                } else {
                                    Sense::click()
                                };
                                let resp = ui.interact(
                                    cell_r,
                                    ui.id().with(("marker_cell", elem_idx, side as u8, r, ci)),
                                    sense,
                                );
                                if ci == 0 {
                                    let pressed = ui.input(|i| {
                                        i.pointer.button_pressed(egui::PointerButton::Primary)
                                    });
                                    if pressed && resp.hovered() {
                                        state.active_table_side = side;
                                        if !read_only_mirrored {
                                            state.table_mode = if side == LineSide::Left {
                                                TableMode::LeftMarkers
                                            } else {
                                                TableMode::RightMarkers
                                            };
                                        }
                                        let modifiers = ui.input(|i| i.modifiers);
                                        flush_pending = true;
                                        handle_no_col_press(state, side, r, ci, modifiers);
                                    }
                                } else if resp.clicked() {
                                    state.active_table_side = side;
                                    if !read_only_mirrored {
                                        state.table_mode = if side == LineSide::Left {
                                            TableMode::LeftMarkers
                                        } else {
                                            TableMode::RightMarkers
                                        };
                                    }
                                    let modifiers = ui.input(|i| i.modifiers);
                                    if modifiers.shift {
                                        let anchor = state
                                            .row_select_anchor
                                            .or_else(|| state.selected_rows.first().copied())
                                            .unwrap_or(r);
                                        let (lo, hi) = if anchor <= r { (anchor, r) } else { (r, anchor) };
                                        state.selected_rows = (lo..=hi).collect();
                                        state.focused_cell = Some((r, ci));
                                        flush_pending = true;
                                    } else if modifiers.ctrl {
                                        toggle_row_selection(state, r);
                                        state.focused_cell = Some((r, ci));
                                        flush_pending = true;
                                    } else {
                                        state.select_single_row(r);
                                        state.focused_cell = Some((r, ci));
                                        if editable {
                                            pending_marker_start_edit = Some((r, ci, values[ci].clone()));
                                        }
                                    }
                                }
                                if !read_only_mirrored {
                                    resp.context_menu(|ui| {
                                        if ui.button("Yeni işaretlemeyi seçilenden önce ekle").clicked() {
                                            state.active_table_side = side;
                                            state.table_mode = if side == LineSide::Left {
                                                TableMode::LeftMarkers
                                            } else {
                                                TableMode::RightMarkers
                                            };
                                            state.select_single_row(r);
                                            actions.push(AppAction::InsertMarkerBefore);
                                            ui.close_menu();
                                        }
                                        if ui.button("Yeni işaretlemeyi sona ekle").clicked() {
                                            state.active_table_side = side;
                                            state.table_mode = if side == LineSide::Left {
                                                TableMode::LeftMarkers
                                            } else {
                                                TableMode::RightMarkers
                                            };
                                            actions.push(AppAction::AppendMarker);
                                            ui.close_menu();
                                        }
                                        if ui.button("Seçilen işaretlemeyi sil").clicked() {
                                            state.active_table_side = side;
                                            state.table_mode = if side == LineSide::Left {
                                                TableMode::LeftMarkers
                                            } else {
                                                TableMode::RightMarkers
                                            };
                                            if !state.selected_rows.contains(&r) {
                                                state.select_single_row(r);
                                            }
                                            actions.push(AppAction::DeleteSelectedMarker);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Kopyala (Ctrl+C)").clicked() {
                                            if !state.selected_rows.contains(&r) {
                                                state.select_single_row(r);
                                            }
                                            actions.push(AppAction::CopySelectedMarkers);
                                            ui.close_menu();
                                        }
                                        if ui
                                            .add_enabled(!state.copied_markers.is_empty(), egui::Button::new("Ekle+Yapıştır (Ctrl+Shift+V)"))
                                            .clicked()
                                        {
                                            state.select_single_row(r);
                                            actions.push(AppAction::PasteInsertMarkers);
                                            ui.close_menu();
                                        }
                                        if ui
                                            .add_enabled(!state.copied_markers.is_empty(), egui::Button::new("Üzerine Yapıştır (Ctrl+V)"))
                                            .clicked()
                                        {
                                            state.select_single_row(r);
                                            actions.push(AppAction::PasteReplaceMarkers);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Noktaları çoğalt...").clicked() {
                                            state.active_table_side = side;
                                            state.table_mode = if side == LineSide::Left {
                                                TableMode::LeftMarkers
                                            } else {
                                                TableMode::RightMarkers
                                            };
                                            state.select_single_row(r);
                                            actions.push(AppAction::OpenMultiplyMarkers);
                                            ui.close_menu();
                                        }
                                    });
                                }
                            }

                            if is_focused {
                                painter.rect_stroke(
                                    cell_r.shrink(0.5),
                                    CornerRadius::ZERO,
                                    Stroke::new(2.0_f32, colors::CELL_FOCUS_BLUE),
                                    egui::StrokeKind::Inside,
                                );
                            }

                            cx += w;
                        }
                    }

                    // Inline `+ İşaret Ekle` footer row
                    if !read_only_mirrored {
                        let fy = grid_rect.top() + header_h + (points_len as f32) * row_h;
                        let footer_w = total_w.min(canvas_w);
                        let footer_r = Rect::from_min_size(pos2(grid_rect.left(), fy), vec2(footer_w, footer_h));
                        let f_resp = ui.interact(
                            footer_r,
                            ui.id().with(("markers_add_footer", elem_idx, side as u8)),
                            Sense::click(),
                        );
                        let f_bg = if f_resp.hovered() {
                            colors::WIN_BLUE_HOVER
                        } else {
                            Color32::from_rgb(0xF8, 0xFA, 0xFC)
                        };
                        let f_txt_col = if f_resp.hovered() {
                            colors::WIN_BLUE
                        } else {
                            colors::TEXT_SECONDARY
                        };
                        painter.rect_filled(footer_r, CornerRadius::ZERO, f_bg);
                        painter.rect_stroke(
                            footer_r,
                            CornerRadius::ZERO,
                            Stroke::new(0.8_f32, colors::BORDER_MID),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            pos2(footer_r.left() + 10.0 * metrics.font_scale, footer_r.center().y),
                            Align2::LEFT_CENTER,
                            "+ İşaret Ekle",
                            bold_font(metrics.footer_font_sz),
                            f_txt_col,
                        );
                        if f_resp.clicked() {
                            state.active_table_side = side;
                            state.table_mode = if side == LineSide::Left {
                                TableMode::LeftMarkers
                            } else {
                                TableMode::RightMarkers
                            };
                            actions.push(AppAction::AppendMarker);
                        }
                    }
                    let pending_scroll_id = ui.id().with(("markers_pending_scroll", side as u8));
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
                        let col_x: f32 = cols.iter().take(sc).map(|(_, w)| *w).sum();
                        let col_w = cols.get(sc).map(|(_, w)| *w).unwrap_or(40.0);
                        let target_r = Rect::from_min_size(
                            pos2(grid_rect.left() + col_x, grid_rect.top() + header_h + (sr as f32) * row_h),
                            vec2(col_w, row_h),
                        );
                        ui.scroll_to_rect(target_r, None);
                    }
                });
        });

    if let Some((r, new_kind)) = pending_kind_change {
        undo.push("İşaret fonksiyonu", doc);
        let mirrored = doc.elements.get(elem_idx).map(|e| e.mirrored).unwrap_or(false);
        if let Some(el) = doc.elements.get_mut(elem_idx) {
            let pts = match side {
                LineSide::Left => &mut el.left_points,
                LineSide::Right => &mut el.right_points,
            };
            if let Some(pt) = pts.get_mut(r) {
                pt.punkt_art_tag_id = new_kind;
                doc.dirty = true;
            }
        }
        if mirrored && side == LineSide::Left {
            sync_mirrored_right_lines(doc, elem_idx);
        }
    }

    if flush_pending {
        flush_active_grid_edit(doc, undo, state);
    }
    if cancel_edit {
        state.editing_cell = None;
        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(marker_edit_init_id));
    } else if let Some((row_i, col_i, new_val, move_kind)) = pending_marker_commit {
        commit_marker_cell_edit(doc, undo, elem_idx, side, row_i, col_i, &new_val, unit_cm);
        state.editing_cell = None;
        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(marker_edit_init_id));
        if pending_marker_start_edit.is_none() {
            let pending_scroll_id = ui.id().with(("markers_pending_scroll", side as u8));
            match move_kind {
                EditMove::Stay => {}
                EditMove::Tab(forward) => {
                    let (next_r, next_c) = next_editable_marker_cell(points_len, row_i, col_i, forward);
                    state.focused_cell = Some((next_r, next_c));
                    if next_r != row_i {
                        state.select_single_row(next_r);
                    }
                    ui.ctx().data_mut(|d| d.insert_temp(pending_scroll_id, (next_r, next_c)));
                }
                EditMove::Delta(dr, dc) => {
                    let next_r = (row_i as i32 + dr)
                        .clamp(0, points_len.saturating_sub(1) as i32) as usize;
                    let next_c = (col_i as i32 + dc)
                        .clamp(0, cols.len().saturating_sub(1) as i32) as usize;
                    state.focused_cell = Some((next_r, next_c));
                    if next_r != row_i {
                        state.select_single_row(next_r);
                    }
                    ui.ctx().data_mut(|d| d.insert_temp(pending_scroll_id, (next_r, next_c)));
                }
            }
        }
    }
    if let Some((sr, sc, init)) = pending_marker_start_edit {
        *queued_start_edit = Some((side, sr, sc, init));
        ui.ctx().data_mut(|d| d.remove::<(usize, u8, usize, usize)>(marker_edit_init_id));
    }
}

fn handle_markers_grid_keyboard(
    ui: &mut Ui,
    hdr: &stoll_kalip_editoru::model::FormHeader,
    points: &[PointEntry],
    state: &mut UiState,
    side: LineSide,
    read_only_mirrored: bool,
    col_count: usize,
) -> Option<(usize, usize)> {
    if state.editing_cell.is_some()
        || state.any_modal_open()
        || ui.ctx().wants_keyboard_input()
        || points.is_empty()
    {
        return None;
    }
    let Some((r, c)) = state.focused_cell else {
        let init_cell = (0, 1);
        let wants_nav = ui.ctx().input(|i| {
            i.key_pressed(Key::ArrowDown)
                || i.key_pressed(Key::ArrowUp)
                || i.key_pressed(Key::ArrowLeft)
                || i.key_pressed(Key::ArrowRight)
                || i.key_pressed(Key::Tab)
        });
        if wants_nav {
            state.focused_cell = Some(init_cell);
            state.select_single_row(0);
            return Some(init_cell);
        }
        return None;
    };
    let row_count = points.len();
    if r >= row_count || c >= col_count {
        state.focused_cell = Some((r.min(row_count - 1), c.min(col_count - 1)));
        return None;
    }
    let is_mm = hdr.input_format == InputFormat::Mm;
    let unit_cm = state.grid_unit_cm;
    let mut moved_to: Option<(usize, usize)> = None;

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
            let enter_f2_bs = if !read_only_mirrored {
                let f2 = i.consume_key(none, Key::F2);
                let ent = i.consume_key(none, Key::Enter);
                let bs = i.consume_key(none, Key::Backspace);
                if bs {
                    Some(true)
                } else if f2 || ent {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            };
            let typed = if !read_only_mirrored && !i.modifiers.ctrl && !i.modifiers.alt {
                i.events.iter().find_map(|ev| match ev {
                    egui::Event::Text(txt) => {
                        let ch = txt.chars().next()?;
                        if ch.is_ascii_digit() || ch == '-' || ch == '.' || ch == ',' {
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

    if ctrl_a {
        state.selected_rows = (0..row_count).collect();
    } else if down {
        let nr = (r + 1).min(row_count - 1);
        state.focused_cell = Some((nr, c));
        state.select_single_row(nr);
        moved_to = Some((nr, c));
    } else if up {
        let nr = r.saturating_sub(1);
        state.focused_cell = Some((nr, c));
        state.select_single_row(nr);
        moved_to = Some((nr, c));
    } else if right {
        let nc = (c + 1).min(col_count - 1);
        state.focused_cell = Some((r, nc));
        moved_to = Some((r, nc));
    } else if left {
        let nc = c.saturating_sub(1);
        state.focused_cell = Some((r, nc));
        moved_to = Some((r, nc));
    } else if tab_fwd || tab_back {
        let (nr, nc) = next_editable_marker_cell(row_count, r, c, tab_fwd);
        state.focused_cell = Some((nr, nc));
        if nr != r {
            state.select_single_row(nr);
        }
        moved_to = Some((nr, nc));
    } else if let Some(clear_init) = enter_f2_bs {
        if matches!(c, 1 | 2 | 4) {
            let pt = &points[r];
            let off = pt.offsets.first();
            let init = if clear_init {
                String::new()
            } else {
                match c {
                    1 => {
                        let py = off.map(|o| o.y).unwrap_or(0);
                        if unit_cm {
                            format_cm_val(rows_to_mm(py, hdr) / 10.0)
                        } else if is_mm {
                            format!("{:.1}", rows_to_mm(py, hdr))
                        } else {
                            py.to_string()
                        }
                    }
                    2 => {
                        let px = off.map(|o| o.x).unwrap_or(0);
                        if unit_cm {
                            format_cm_val(stitches_to_mm(px, hdr) / 10.0)
                        } else if is_mm {
                            format!("{:.1}", stitches_to_mm(px, hdr))
                        } else {
                            px.to_string()
                        }
                    }
                    4 => pt.gruppe_tag_id.to_string(),
                    _ => String::new(),
                }
            };
            state.editing_side = side;
            state.editing_cell = Some((r, c, init));
            state.editing_pristine = !clear_init;
        }
    } else if let Some(txt) = typed_txt {
        if matches!(c, 1 | 2 | 4) {
            state.editing_side = side;
            state.editing_cell = Some((r, c, txt));
            state.editing_pristine = false;
        }
    }

    moved_to
}

fn commit_marker_cell_edit(
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
    col_idx: usize,
    raw_text: &str,
    unit_cm: bool,
) {
    let hdr = doc.header.clone();
    let is_mm = hdr.input_format == InputFormat::Mm;
    let trimmed = raw_text.trim().replace(',', ".");
    let parsed_f = trimmed.parse::<f64>().ok().filter(|v| v.is_finite());
    let parsed_i = trimmed.parse::<i32>().ok();

    let Some(el) = doc.elements.get(elem_idx) else {
        return;
    };
    let pts = match side {
        LineSide::Left => &el.left_points,
        LineSide::Right => &el.right_points,
    };
    if row_idx >= pts.len() {
        return;
    }

    let mirrored = el.mirrored;
    let old_y = pts[row_idx].offsets.first().map(|o| o.y).unwrap_or(0);
    let old_x = pts[row_idx].offsets.first().map(|o| o.x).unwrap_or(0);
    let old_g = pts[row_idx].gruppe_tag_id;
    let mut changed = false;

    match col_idx {
        1 => {
            let new_y = if unit_cm {
                parsed_f.map(|v| mm_to_rows(v * 10.0, &hdr))
            } else if is_mm {
                parsed_f.map(|v| mm_to_rows(v, &hdr))
            } else {
                parsed_i
            };
            if let Some(y) = new_y {
                let text_unchanged = unit_cm
                    && parsed_f
                        .map(|v| (v - (rows_to_mm(old_y, &hdr) / 10.0)).abs() < 0.005)
                        .unwrap_or(false);
                if y != old_y && !text_unchanged {
                    undo.push("İşaret Boy", doc);
                    let pts = match side {
                        LineSide::Left => &mut doc.elements[elem_idx].left_points,
                        LineSide::Right => &mut doc.elements[elem_idx].right_points,
                    };
                    if pts[row_idx].offsets.is_empty() {
                        pts[row_idx].offsets.push(PointOffset::default());
                    }
                    pts[row_idx].offsets[0].y = y;
                    changed = true;
                }
            }
        }
        2 => {
            let new_x = if unit_cm {
                parsed_f.map(|v| mm_to_stitches(v * 10.0, &hdr))
            } else if is_mm {
                parsed_f.map(|v| mm_to_stitches(v, &hdr))
            } else {
                parsed_i
            };
            if let Some(x) = new_x {
                let text_unchanged = unit_cm
                    && parsed_f
                        .map(|v| (v - (stitches_to_mm(old_x, &hdr) / 10.0)).abs() < 0.005)
                        .unwrap_or(false);
                if x != old_x && !text_unchanged {
                    undo.push("İşaret En", doc);
                    let pts = match side {
                        LineSide::Left => &mut doc.elements[elem_idx].left_points,
                        LineSide::Right => &mut doc.elements[elem_idx].right_points,
                    };
                    if pts[row_idx].offsets.is_empty() {
                        pts[row_idx].offsets.push(PointOffset::default());
                    }
                    pts[row_idx].offsets[0].x = x;
                    changed = true;
                }
            }
        }
        4 => {
            if let Some(g) = parsed_i {
                let clamped = g.max(0);
                if clamped != old_g {
                    undo.push("İşaret Grup", doc);
                    let pts = match side {
                        LineSide::Left => &mut doc.elements[elem_idx].left_points,
                        LineSide::Right => &mut doc.elements[elem_idx].right_points,
                    };
                    pts[row_idx].gruppe_tag_id = clamped;
                    changed = true;
                }
            }
        }
        _ => {}
    }

    if changed {
        if mirrored && side == LineSide::Left {
            sync_mirrored_right_lines(doc, elem_idx);
        }
        doc.dirty = true;
    }
}

fn handle_lines_grid_keyboard(
    ui: &mut Ui,
    doc: &FormDocument,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
    side: LineSide,
    read_only_mirrored: bool,
    row_count: usize,
    col_count: usize,
) -> Option<(usize, usize)> {
    if state.editing_cell.is_some()
        || state.any_modal_open()
        || ui.ctx().wants_keyboard_input()
        || row_count == 0
    {
        return None;
    }
    let Some((r, c)) = state.focused_cell else {
        let init_cell = (0, 2.min(col_count.saturating_sub(1)));
        let wants_nav = ui.ctx().input(|i| {
            i.key_pressed(Key::ArrowDown)
                || i.key_pressed(Key::ArrowUp)
                || i.key_pressed(Key::ArrowLeft)
                || i.key_pressed(Key::ArrowRight)
                || i.key_pressed(Key::Tab)
        });
        if wants_nav {
            state.focused_cell = Some(init_cell);
            state.select_single_row(0);
            return Some(init_cell);
        }
        return None;
    };
    if r >= row_count || c >= col_count {
        state.focused_cell = Some((r.min(row_count - 1), c.min(col_count.saturating_sub(1))));
        return None;
    }

    let (ctrl_a, home, end, pg_dn, pg_up, down, up, right, left, tab_fwd, tab_back, edit_key, typed_text) =
        ui.ctx().input_mut(|i| {
            let none = egui::Modifiers::NONE;
            let shift = egui::Modifiers::SHIFT;
            let ctrl_a = i.consume_key(egui::Modifiers::CTRL, Key::A);
            let home = i.consume_key(none, Key::Home);
            let end = i.consume_key(none, Key::End);
            let pg_dn = i.consume_key(none, Key::PageDown);
            let pg_up = i.consume_key(none, Key::PageUp);
            let down = i.consume_key(none, Key::ArrowDown);
            let up = i.consume_key(none, Key::ArrowUp);
            let right = i.consume_key(none, Key::ArrowRight);
            let left = i.consume_key(none, Key::ArrowLeft);
            let tab_back = i.consume_key(shift, Key::Tab);
            let tab_fwd = !tab_back && i.consume_key(none, Key::Tab);
            let edit_key = if !read_only_mirrored {
                let f2 = i.consume_key(none, Key::F2);
                let ent = i.consume_key(none, Key::Enter);
                let bs = i.consume_key(none, Key::Backspace);
                if bs {
                    Some(true)
                } else if f2 || ent {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            };
            let typed = if !read_only_mirrored && !i.modifiers.ctrl && !i.modifiers.alt {
                i.events.iter().find_map(|ev| match ev {
                    egui::Event::Text(s) if !s.is_empty() => Some(s.clone()),
                    _ => None,
                })
            } else {
                None
            };
            (ctrl_a, home, end, pg_dn, pg_up, down, up, right, left, tab_fwd, tab_back, edit_key, typed)
        });

    let mut moved_to: Option<(usize, usize)> = None;

    if ctrl_a {
        state.selected_rows = (0..row_count).collect();
    } else if home {
        let nc = 2.min(col_count.saturating_sub(1));
        state.focused_cell = Some((r, nc));
        moved_to = Some((r, nc));
    } else if end {
        let nc = col_count.saturating_sub(1);
        state.focused_cell = Some((r, nc));
        moved_to = Some((r, nc));
    } else if pg_dn {
        let nr = (r + 10).min(row_count.saturating_sub(1));
        state.focused_cell = Some((nr, c));
        state.select_single_row(nr);
        moved_to = Some((nr, c));
    } else if pg_up {
        let nr = r.saturating_sub(10);
        state.focused_cell = Some((nr, c));
        state.select_single_row(nr);
        moved_to = Some((nr, c));
    } else if down {
        let nr = (r + 1).min(row_count.saturating_sub(1));
        state.focused_cell = Some((nr, c));
        state.select_single_row(nr);
        moved_to = Some((nr, c));
    } else if up {
        let nr = r.saturating_sub(1);
        state.focused_cell = Some((nr, c));
        state.select_single_row(nr);
        moved_to = Some((nr, c));
    } else if right {
        let nc = (c + 1).min(col_count.saturating_sub(1));
        state.focused_cell = Some((r, nc));
        moved_to = Some((r, nc));
    } else if left {
        let nc = c.saturating_sub(1);
        state.focused_cell = Some((r, nc));
        moved_to = Some((r, nc));
    } else if tab_fwd || tab_back {
        let visible_cols = visible_line_cols(state);
        let (nr, nc) = next_editable_line_cell(
            doc,
            state.selected_elem,
            side,
            &visible_cols,
            r,
            c,
            tab_fwd,
        );
        state.focused_cell = Some((nr, nc));
        if nr != r {
            state.select_single_row(nr);
        }
        moved_to = Some((nr, nc));
    } else if let Some(clear_init) = edit_key {
        if let Some(el) = doc.elements.get(state.selected_elem) {
            if let Some(line) = el.lines(side).get(r) {
                let visible_cols: Vec<LineCol> = visible_line_cols(state)
                    .into_iter()
                    .map(|(col, _)| col)
                    .collect();
                if let Some(&col) = visible_cols.get(c) {
                    let has_curve = line.curve.as_ref().map(|cv| !cv.steps.is_empty()).unwrap_or(false);
                    let func_id = line.function.to_id();
                    let is_base = el.is_base_line_row(side, r);
                    if is_line_cell_editable(col, has_curve, is_base, func_id) {
                        let init = if clear_init {
                            String::new()
                        } else {
                            format_line_cell(
                                line,
                                col,
                                doc.header.input_format == InputFormat::Mm,
                                state.grid_unit_cm,
                                state.sign_mode,
                                side,
                                has_curve,
                                is_base,
                            )
                        };
                        state.editing_side = side;
                        state.editing_cell = Some((r, c, init));
                        state.editing_pristine = !clear_init;
                    } else if col == LineCol::LineEditor && !is_base {
                        actions.push(AppAction::OpenCurveEditor { line_idx: r });
                    } else if col == LineCol::Function {
                        actions.push(AppAction::OpenLineProps {
                            line_idx: r,
                            initial_tab: line_function_default_tab(func_id),
                        });
                    }
                }
            }
        }
    } else if let Some(txt) = typed_text {
        // Direct typing to begin editing a focused editable cell (Stingray Grid parity)
        if let Some(el) = doc.elements.get(state.selected_elem) {
            if let Some(line) = el.lines(side).get(r) {
                let visible_cols: Vec<LineCol> = visible_line_cols(state)
                    .into_iter()
                    .map(|(col, _)| col)
                    .collect();
                if let Some(&col) = visible_cols.get(c) {
                    let has_curve = line.curve.as_ref().map(|cv| !cv.steps.is_empty()).unwrap_or(false);
                    let func_id = line.function.to_id();
                    let is_base = el.is_base_line_row(side, r);
                    if is_line_cell_editable(col, has_curve, is_base, func_id) {
                        let ch = txt.chars().next().unwrap_or('\0');
                        let allow = if col == LineCol::Comment {
                            !ch.is_control()
                        } else {
                            ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' || ch == ','
                        };
                        if allow {
                            state.editing_side = side;
                            state.editing_cell = Some((r, c, txt));
                            state.editing_pristine = false;
                        }
                    }
                }
            }
        }
    }

    moved_to
}

fn line_col_header(col: LineCol, is_mm: bool, unit_cm: bool) -> &'static str {
    match col {
        LineCol::No => "No.",
        LineCol::LineEditor => "Kavis",
        LineCol::Rows => {
            if unit_cm {
                "Boy\ncm"
            } else if is_mm {
                "Boy\nmm"
            } else {
                "Boy"
            }
        }
        LineCol::Stitches => {
            if unit_cm {
                "En\ncm"
            } else if is_mm {
                "En\nmm"
            } else {
                "En"
            }
        }
        LineCol::StepY => "Boy\nBasamak",
        LineCol::StepX => "En\nBasamak",
        LineCol::Factor => "Faktör",
        LineCol::RestY => "Boy\nArta kalan",
        LineCol::RestX => "En\nArta kalan",
        LineCol::WidthNarrow => "En ---",
        LineCol::WidthFadeOut => "En \\\\\\",
        LineCol::Function => "Fonksiyon",
        LineCol::Group => "Grup",
        LineCol::Comment => "Yorum",
    }
}

fn format_cm_val(cm: f64) -> String {
    let s = format!("{cm:.2}");
    if s.ends_with('0') && !s.ends_with(".0") {
        s[..s.len() - 1].to_string()
    } else {
        s
    }
}

fn function_pill_colors(func_id: i32) -> (Color32, Color32) {
    match func_id {
        1 | 9 => (Color32::from_rgb(0xDB, 0xEA, 0xFE), Color32::from_rgb(0x93, 0xC5, 0xFD)), // Reglan: soft blue
        2 => (Color32::from_rgb(0xDC, 0xFC, 0xE7), Color32::from_rgb(0x86, 0xEF, 0xAC)),     // Artırma: soft green
        3 | 6 => (Color32::from_rgb(0xF3, 0xE8, 0xFF), Color32::from_rgb(0xD8, 0xB4, 0xFE)), // Remayözleme: soft purple
        4 => (Color32::from_rgb(0xFE, 0xF3, 0xC7), Color32::from_rgb(0xFC, 0xD3, 0x4D)),     // Kenar düzeltimi: soft amber
        8 => (Color32::from_rgb(0xE2, 0xE8, 0xF0), Color32::from_rgb(0xCB, 0xD5, 0xE1)),     // Temel: soft slate
        _ => (colors::CARD_BG, colors::BORDER_LIGHT),
    }
}

fn next_editable_line_cell(
    doc: &FormDocument,
    elem_idx: usize,
    side: LineSide,
    visible_cols: &[(LineCol, f32)],
    cur_r: usize,
    cur_c: usize,
    forward: bool,
) -> (usize, usize) {
    let Some(el) = doc.elements.get(elem_idx) else {
        return (cur_r, cur_c);
    };
    let lines = el.lines(side);
    if lines.is_empty() || visible_cols.is_empty() {
        return (cur_r, cur_c);
    }
    let row_count = lines.len();
    let col_count = visible_cols.len();
    let mut r = cur_r.min(row_count - 1);
    let mut c = cur_c.min(col_count - 1);
    let total = row_count * col_count;

    for _ in 0..total {
        if forward {
            if c + 1 < col_count {
                c += 1;
            } else if r + 1 < row_count {
                r += 1;
                c = 0;
            } else {
                break;
            }
        } else if c > 0 {
            c -= 1;
        } else if r > 0 {
            r -= 1;
            c = col_count - 1;
        } else {
            break;
        }

        let line = &lines[r];
        let has_curve = line.curve.as_ref().map(|cv| !cv.steps.is_empty()).unwrap_or(false);
        let func_id = line.function.to_id();
        let is_base = el.is_base_line_row(side, r);
        if is_line_cell_editable(visible_cols[c].0, has_curve, is_base, func_id) {
            return (r, c);
        }
    }
    (cur_r, cur_c)
}

fn is_line_cell_editable(col: LineCol, has_curve: bool, is_base_row: bool, func_id: i32) -> bool {
    match col {
        LineCol::No | LineCol::LineEditor | LineCol::RestY | LineCol::RestX | LineCol::Function | LineCol::Group => false,
        LineCol::Rows => !has_curve && !is_base_row,
        LineCol::Stitches => !has_curve,
        LineCol::StepY | LineCol::StepX | LineCol::Factor => !has_curve && !is_base_row,
        LineCol::WidthNarrow => matches!(func_id, 1 | 2 | 9),
        LineCol::WidthFadeOut => !is_base_row,
        LineCol::Comment => true,
    }
}

fn format_line_cell(
    line: &LineSegment,
    col: LineCol,
    is_mm: bool,
    unit_cm: bool,
    sign_mode: SignMode,
    side: LineSide,
    has_curve: bool,
    is_base_row: bool,
) -> String {
    // `MAIN-05`: Row 1 (`Temel` / `is_base_row`) is exempt from Knitting sign inversion.
    let x_sign = if !is_base_row && matches!((sign_mode, side), (SignMode::Knitting, LineSide::Left))
    {
        -1
    } else {
        1
    };
    let func_id = line.function.to_id();

    match col {
        LineCol::No => line.index.to_string(),
        LineCol::LineEditor => String::new(),
        LineCol::Rows => {
            if unit_cm {
                format_cm_val(line.height_mm / 10.0)
            } else if is_mm {
                format!("{:.2}", line.height_mm)
            } else {
                line.rows.to_string()
            }
        }
        LineCol::Stitches => {
            if unit_cm {
                format_cm_val((line.width_mm / 10.0) * (x_sign as f64))
            } else if is_mm {
                format!("{:.2}", line.width_mm * (x_sign as f64))
            } else {
                (line.stitches * x_sign).to_string()
            }
        }
        LineCol::StepY => {
            if has_curve {
                "0".into()
            } else {
                line.step_y.to_string()
            }
        }
        LineCol::StepX => {
            if has_curve {
                "0".into()
            } else {
                (line.step_x * x_sign).to_string()
            }
        }
        LineCol::Factor => {
            if has_curve {
                "0".into()
            } else {
                line.step_factor.to_string()
            }
        }
        LineCol::RestY => {
            if has_curve {
                "0".into()
            } else {
                line.rest_y.to_string()
            }
        }
        LineCol::RestX => {
            if has_curve {
                "0".into()
            } else {
                (line.rest_x * x_sign).to_string()
            }
        }
        LineCol::WidthNarrow => {
            // Blank on non-Reglan/Artırma rows (matches Screenshot 222001.png & #12/#28 audit!)
            if !matches!(func_id, 1 | 2 | 9) {
                return String::new();
            }
            if func_id == 2 {
                line.widening
                    .as_ref()
                    .and_then(|w| {
                        w.modules
                            .iter()
                            .find(|m| m.module_art_id == 0)
                            .or_else(|| w.modules.iter().find(|m| m.module_art_id >= 0 && m.width > 0))
                            .or_else(|| w.modules.iter().find(|m| m.width > 0))
                            .or_else(|| w.modules.first())
                    })
                    .map(|m| m.width)
                    .filter(|&w| w > 0)
                    .or_else(|| {
                        line.curve
                            .as_ref()
                            .and_then(|c| c.steps.first())
                            .map(|s| s.widening_width)
                            .filter(|&w| w > 0)
                    })
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| "1".into())
            } else {
                line.narrowing
                    .as_ref()
                    .and_then(|n| {
                        n.modules
                            .iter()
                            .find(|m| m.module_art_id == 0)
                            .or_else(|| n.modules.iter().find(|m| m.module_art_id >= 0 && m.width > 0))
                            .or_else(|| n.modules.iter().find(|m| m.width > 0))
                            .or_else(|| n.modules.first())
                    })
                    .map(|m| m.width)
                    .filter(|&w| w > 0)
                    .or_else(|| {
                        line.curve
                            .as_ref()
                            .and_then(|c| c.steps.first())
                            .map(|s| s.narrowing_width)
                            .filter(|&w| w > 0)
                    })
                    .map(|w| w.to_string())
                    .unwrap_or_else(|| "0".into())
            }
        }
        LineCol::WidthFadeOut => {
            if is_base_row {
                return String::new();
            }
            line.fade_out
                .as_ref()
                .and_then(|f| {
                    f.modules
                        .iter()
                        .find(|m| m.module_art_id == 0)
                        .or_else(|| f.modules.iter().find(|m| m.module_art_id >= 0))
                        .or_else(|| f.modules.first())
                })
                .map(|m| m.width.to_string())
                .or_else(|| {
                    line.curve
                        .as_ref()
                        .and_then(|c| c.steps.first())
                        .map(|s| s.fade_out_width.to_string())
                })
                .unwrap_or_else(|| "0".into())
        }
        LineCol::Function => line_function_grid_label_tr(func_id).to_string(),
        LineCol::Group => line.group.to_string(),
        LineCol::Comment => line.comment.clone(),
    }
}

#[derive(Clone, Copy)]
enum EditMove {
    Stay,
    Delta(i32, i32),
    Tab(bool),
}

enum EditNavAction {
    None,
    Cancel,
    Move(EditMove),
}

enum CellEditOutcome {
    Continue,
    Cancel,
    Commit(EditMove),
}

/// Shared inline `TextEdit` renderer for Lines & Markers grids with select-all on edit entry
/// and automatic leading-`"0"` replacement while `pristine` is true.
#[allow(clippy::too_many_arguments)]
fn show_grid_cell_text_edit(
    ui: &mut Ui,
    cell_r: Rect,
    te_id: egui::Id,
    edit_init_id: egui::Id,
    cell_key: (usize, u8, usize, usize),
    buf: &mut String,
    pristine: &mut bool,
    is_numeric: bool,
    font_size: f32,
) -> CellEditOutcome {
    let already_inited = ui
        .ctx()
        .data(|d| d.get_temp::<(usize, u8, usize, usize)>(edit_init_id))
        == Some(cell_key);

    let nav = if already_inited {
        consume_text_edit_nav_keys(ui.ctx(), te_id, buf)
    } else {
        EditNavAction::None
    };

    match nav {
        EditNavAction::Cancel => return CellEditOutcome::Cancel,
        EditNavAction::Move(mv) => return CellEditOutcome::Commit(mv),
        EditNavAction::None => {}
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
    let edit_rect = cell_r.shrink2(vec2(3.0, 1.0));
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
    let te = egui::TextEdit::singleline(buf)
        .id(te_id)
        .font(regular_font(font_size))
        .horizontal_align(if is_numeric {
            egui::Align::RIGHT
        } else {
            egui::Align::LEFT
        })
        .vertical_align(egui::Align::Center)
        .frame(false)
        .desired_width(edit_rect.width());
    let output = te.show(&mut child_ui);
    let resp = output.response;
    let mut te_state = output.state;

    if is_numeric {
        if apply_pristine_zero_guard(&prev_buf, buf, pristine) {
            let len = buf.chars().count();
            te_state.cursor.set_char_range(Some(
                egui::text::CCursorRange::one(egui::text::CCursor::new(len)),
            ));
            te_state.clone().store(ui.ctx(), resp.id);
        } else if *buf != prev_buf {
            *pristine = false;
        }
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
        return CellEditOutcome::Commit(EditMove::Stay);
    }

    CellEditOutcome::Continue
}

/// Intercept arrow keys, Enter, Tab/Shift+Tab, and Escape inside an active grid `TextEdit`
/// BEFORE `TextEdit` consumes them, so navigation across rows and columns stays fluid.
fn consume_text_edit_nav_keys(ctx: &egui::Context, te_id: egui::Id, buf: &str) -> EditNavAction {
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
            return EditNavAction::Cancel;
        }
        if i.consume_key(none, Key::Enter) || i.consume_key(shift, Key::Enter) {
            return EditNavAction::Move(EditMove::Delta(1, 0));
        }
        if i.consume_key(none, Key::ArrowUp) {
            return EditNavAction::Move(EditMove::Delta(-1, 0));
        }
        if i.consume_key(none, Key::ArrowDown) {
            return EditNavAction::Move(EditMove::Delta(1, 0));
        }
        if i.consume_key(shift, Key::Tab) {
            return EditNavAction::Move(EditMove::Tab(false));
        }
        if i.consume_key(none, Key::Tab) {
            return EditNavAction::Move(EditMove::Tab(true));
        }
        if at_start && i.consume_key(none, Key::ArrowLeft) {
            return EditNavAction::Move(EditMove::Delta(0, -1));
        }
        if at_end && i.consume_key(none, Key::ArrowRight) {
            return EditNavAction::Move(EditMove::Delta(0, 1));
        }
        EditNavAction::None
    })
}

fn toggle_row_selection(state: &mut UiState, r: usize) {
    if let Some(pos) = state.selected_rows.iter().position(|&x| x == r) {
        if state.selected_rows.len() > 1 {
            state.selected_rows.remove(pos);
        }
    } else {
        state.selected_rows.push(r);
        state.selected_rows.sort_unstable();
    }
    state.row_select_anchor = Some(r);
}

fn handle_no_col_press(
    state: &mut UiState,
    side: LineSide,
    r: usize,
    col_idx: usize,
    modifiers: egui::Modifiers,
) {
    if modifiers.shift {
        let anchor = state
            .row_select_anchor
            .or_else(|| state.selected_rows.first().copied())
            .unwrap_or(r);
        let (lo, hi) = if anchor <= r { (anchor, r) } else { (r, anchor) };
        state.selected_rows = (lo..=hi).collect();
        state.focused_cell = Some((r, col_idx));
        state.row_drag_selecting = Some((side, anchor));
    } else if modifiers.ctrl {
        toggle_row_selection(state, r);
        state.focused_cell = Some((r, col_idx));
    } else {
        state.select_single_row(r);
        state.focused_cell = Some((r, col_idx));
        state.row_drag_selecting = Some((side, r));
    }
}

fn update_drag_row_selection(
    ui: &Ui,
    state: &mut UiState,
    side: LineSide,
    row_count: usize,
    grid_top: f32,
    grid_left: f32,
    header_h: f32,
    row_h: f32,
    total_w: f32,
) -> Option<usize> {
    let Some((drag_side, anchor)) = state.row_drag_selecting else {
        return None;
    };
    if drag_side != side {
        return None;
    }
    if row_count == 0 || !ui.input(|i| i.pointer.primary_down()) {
        state.row_drag_selecting = None;
        return None;
    }
    let pos = ui.input(|i| i.pointer.interact_pos())?;
    if pos.x < grid_left - 24.0 || pos.x > grid_left + total_w + 24.0 {
        return None;
    }
    let hr = ((pos.y - (grid_top + header_h)) / row_h)
        .floor()
        .clamp(0.0, row_count.saturating_sub(1) as f32) as usize;
    let anchor_clamped = anchor.min(row_count.saturating_sub(1));
    let (lo, hi) = if anchor_clamped <= hr {
        (anchor_clamped, hr)
    } else {
        (hr, anchor_clamped)
    };
    state.selected_rows = (lo..=hi).collect();
    state.focused_cell = Some((hr, 0));
    Some(hr)
}

fn next_editable_marker_cell(
    row_count: usize,
    cur_r: usize,
    cur_c: usize,
    forward: bool,
) -> (usize, usize) {
    if row_count == 0 {
        return (cur_r, cur_c);
    }
    const EDIT_COLS: &[usize] = &[1, 2, 4];
    let mut r = cur_r.min(row_count - 1);
    if forward {
        if let Some(&nc) = EDIT_COLS.iter().find(|&&ec| ec > cur_c) {
            return (r, nc);
        }
        if r + 1 < row_count {
            r += 1;
            return (r, EDIT_COLS[0]);
        }
    } else {
        if let Some(&nc) = EDIT_COLS.iter().rev().find(|&&ec| ec < cur_c) {
            return (r, nc);
        }
        if r > 0 {
            r -= 1;
            return (r, *EDIT_COLS.last().unwrap());
        }
    }
    (cur_r, cur_c)
}

fn show_line_context_menu(
    ui: &mut Ui,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
    r: usize,
    func_id: i32,
    is_base_row: bool,
    is_closing: bool,
) {
    if ui.button("Yeni çizgiyi seçilenden önce ekle").clicked() {
        state.select_single_row(r);
        actions.push(AppAction::InsertLineBefore);
        ui.close_menu();
    }
    if ui.button("Yeni çizgiyi sona ekle").clicked() {
        actions.push(AppAction::AppendLine);
        ui.close_menu();
    }
    if ui
        .add_enabled(!is_base_row, egui::Button::new("Seçilen çizgiyi sil"))
        .clicked()
    {
        if !state.selected_rows.contains(&r) {
            state.select_single_row(r);
        }
        actions.push(AppAction::DeleteSelectedLines);
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Kopyala (Ctrl+C)").clicked() {
        if !state.selected_rows.contains(&r) {
            state.select_single_row(r);
        }
        actions.push(AppAction::CopySelectedLines);
        ui.close_menu();
    }
    if ui
        .add_enabled(
            !state.copied_lines.is_empty(),
            egui::Button::new("Ekle+Yapıştır (Ctrl+Shift+V)"),
        )
        .clicked()
    {
        state.select_single_row(r);
        actions.push(AppAction::PasteInsertLines);
        ui.close_menu();
    }
    if ui
        .add_enabled(
            !state.copied_lines.is_empty(),
            egui::Button::new("Üzerine Yapıştır (Ctrl+V)"),
        )
        .clicked()
    {
        state.select_single_row(r);
        actions.push(AppAction::PasteReplaceLines);
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Özellikler...").clicked() {
        state.select_single_row(r);
        actions.push(AppAction::OpenLineProps {
            line_idx: r,
            initial_tab: line_function_default_tab(func_id),
        });
        ui.close_menu();
    }
    if ui.button("Özellikleri başka çizgiden al...").clicked() {
        state.select_single_row(r);
        actions.push(AppAction::OpenRefLineSelect { line_idx: r });
        ui.close_menu();
    }
    if ui
        .add_enabled(
            !is_base_row && !is_closing,
            egui::Button::new("Satır Editörü..."),
        )
        .clicked()
    {
        state.select_single_row(r);
        actions.push(AppAction::OpenCurveEditor { line_idx: r });
        ui.close_menu();
    }
}

/// Commit any active inline grid cell edit (`state.editing_cell`) before switching tables, elements, or saving.
pub fn flush_active_grid_edit(doc: &mut FormDocument, undo: &mut UndoStack, state: &mut UiState) {
    let Some((row_i, col_i, buf)) = state.editing_cell.take() else {
        return;
    };
    let elem_idx = state.selected_elem;
    let side = state.editing_side;
    if state.table_mode.is_lines() {
        let visible_cols = visible_line_cols(state);
        if let Some(&(col, _)) = visible_cols.get(col_i) {
            commit_line_cell_edit(
                doc,
                undo,
                elem_idx,
                side,
                row_i,
                col,
                &buf,
                state.sign_mode,
                state.grid_unit_cm,
            );
        }
    } else {
        commit_marker_cell_edit(doc, undo, elem_idx, side, row_i, col_i, &buf, state.grid_unit_cm);
    }
}

fn commit_line_cell_edit(
    doc: &mut FormDocument,
    undo: &mut UndoStack,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
    col: LineCol,
    raw_text: &str,
    sign_mode: SignMode,
    unit_cm: bool,
) {
    let hdr = doc.header.clone();
    let is_mm = hdr.input_format == InputFormat::Mm;
    let is_left = side == LineSide::Left;

    let is_base_row = match doc.elements.get(elem_idx) {
        Some(el) if row_idx < el.lines(side).len() => el.is_base_line_row(side, row_idx),
        _ => return,
    };

    // `MAIN-05`: Row 1 (`Temel`) is exempt from Knitting sign inversion.
    let x_sign = if !is_base_row && matches!((sign_mode, side), (SignMode::Knitting, LineSide::Left))
    {
        -1
    } else {
        1
    };

    let snapshot = doc.clone();
    let mirrored = doc.elements[elem_idx].mirrored;
    let elem_type = doc.elements[elem_idx].element_type();
    let endline_dists = snapshot_endline_distances(doc);
    let line_count = doc.elements[elem_idx].lines(side).len();
    let sum_x_others: i32 = doc.elements[elem_idx]
        .lines(side)
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != row_idx)
        .map(|(_, l)| l.stitches)
        .sum();
    let is_last_row = row_idx > 0 && row_idx + 1 == line_count;
    let Some(el) = doc.elements.get_mut(elem_idx) else {
        return;
    };
    let Some(line) = el.lines_mut(side).get_mut(row_idx) else {
        return;
    };

    let trimmed = raw_text.trim().replace(',', ".");
    let mut changed = false;

    match col {
        LineCol::Rows => {
            if unit_cm {
                if let Ok(cm) = trimmed.parse::<f64>() {
                    if cm.is_finite() {
                        let mm = (cm * 10.0).max(0.0);
                        let same = if is_mm {
                            (mm - line.height_mm).abs() < 0.05
                                || format_cm_val(line.height_mm / 10.0) == format_cm_val(cm.max(0.0))
                        } else {
                            mm_to_rows(mm, &hdr) == line.rows
                                || format_cm_val(line.height_mm / 10.0) == format_cm_val(cm.max(0.0))
                        };
                        if !same {
                            if is_mm {
                                line.height_mm = mm;
                                recalc_line_from_mm(line, &hdr);
                            } else {
                                line.rows = mm_to_rows(mm, &hdr).max(0);
                                recalc_line_from_stitches_and_rows(line, &hdr);
                            }
                            let is_closing = is_last_row
                                && line.rows == 0
                                && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                            auto_detect_line_function_in_context(
                                line,
                                row_idx == 0,
                                is_closing,
                                is_left,
                                elem_type,
                            );
                            changed = true;
                        }
                    }
                }
            } else if is_mm {
                if let Ok(v) = trimmed.parse::<f64>() {
                    if v.is_finite() {
                        let clamped = v.max(0.0);
                        if (clamped - line.height_mm).abs() >= 0.005 {
                            line.height_mm = clamped;
                            recalc_line_from_mm(line, &hdr);
                            let is_closing = is_last_row
                                && line.rows == 0
                                && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                            auto_detect_line_function_in_context(
                                line,
                                row_idx == 0,
                                is_closing,
                                is_left,
                                elem_type,
                            );
                            changed = true;
                        }
                    }
                }
            } else if let Ok(v) = trimmed.parse::<i32>() {
                let clamped = v.max(0);
                if clamped != line.rows {
                    line.rows = clamped;
                    recalc_line_from_stitches_and_rows(line, &hdr);
                    let is_closing = is_last_row
                        && line.rows == 0
                        && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                    auto_detect_line_function_in_context(
                        line,
                        row_idx == 0,
                        is_closing,
                        is_left,
                        elem_type,
                    );
                    changed = true;
                }
            }
        }
        LineCol::Stitches => {
            if unit_cm {
                if let Ok(cm) = trimmed.parse::<f64>() {
                    if cm.is_finite() {
                        let mm = cm * 10.0 * (x_sign as f64);
                        let same = if is_mm {
                            (mm - line.width_mm).abs() < 0.05
                                || format_cm_val((line.width_mm / 10.0) * (x_sign as f64))
                                    == format_cm_val(cm)
                        } else {
                            mm_to_stitches(mm, &hdr) == line.stitches
                                || format_cm_val((line.width_mm / 10.0) * (x_sign as f64))
                                    == format_cm_val(cm)
                        };
                        if !same {
                            if is_mm {
                                line.width_mm = mm;
                                recalc_line_from_mm(line, &hdr);
                            } else {
                                line.stitches = mm_to_stitches(mm, &hdr);
                                recalc_line_from_stitches_and_rows(line, &hdr);
                            }
                            let is_closing = is_last_row
                                && line.rows == 0
                                && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                            auto_detect_line_function_in_context(
                                line,
                                row_idx == 0,
                                is_closing,
                                is_left,
                                elem_type,
                            );
                            changed = true;
                        }
                    }
                }
            } else if is_mm {
                if let Ok(v) = trimmed.parse::<f64>() {
                    if v.is_finite() {
                        let new_w = v * (x_sign as f64);
                        if (new_w - line.width_mm).abs() >= 0.005 {
                            line.width_mm = new_w;
                            recalc_line_from_mm(line, &hdr);
                            let is_closing = is_last_row
                                && line.rows == 0
                                && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                            auto_detect_line_function_in_context(
                                line,
                                row_idx == 0,
                                is_closing,
                                is_left,
                                elem_type,
                            );
                            changed = true;
                        }
                    }
                }
            } else if let Ok(v) = trimmed.parse::<i32>() {
                let new_st = v * x_sign;
                if new_st != line.stitches {
                    line.stitches = new_st;
                    recalc_line_from_stitches_and_rows(line, &hdr);
                    let is_closing = is_last_row
                        && line.rows == 0
                        && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                    auto_detect_line_function_in_context(
                        line,
                        row_idx == 0,
                        is_closing,
                        is_left,
                        elem_type,
                    );
                    changed = true;
                }
            }
        }
        LineCol::StepY => {
            if let Ok(v) = trimmed.parse::<i32>() {
                if v != line.step_y {
                    line.step_y = v;
                    let is_closing = is_last_row
                        && line.rows == 0
                        && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                    recalc_line_from_step_edit_for_element(
                        line,
                        &hdr,
                        false,
                        row_idx == 0,
                        is_closing,
                        is_left,
                        elem_type,
                    );
                    changed = true;
                }
            }
        }
        LineCol::StepX => {
            if let Ok(v) = trimmed.parse::<i32>() {
                let new_sx = v * x_sign;
                if new_sx != line.step_x {
                    line.step_x = new_sx;
                    let is_closing = is_last_row
                        && line.rows == 0
                        && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                    recalc_line_from_step_edit_for_element(
                        line,
                        &hdr,
                        false,
                        row_idx == 0,
                        is_closing,
                        is_left,
                        elem_type,
                    );
                    changed = true;
                }
            }
        }
        LineCol::Factor => {
            if let Ok(v) = trimmed.parse::<i32>() {
                let clamped = v.max(1);
                if clamped != line.step_factor {
                    line.step_factor = clamped;
                    let is_closing = is_last_row
                        && line.rows == 0
                        && (line.last_line != 0 || sum_x_others + line.stitches == 0);
                    recalc_line_from_step_edit_for_element(
                        line,
                        &hdr,
                        true,
                        row_idx == 0,
                        is_closing,
                        is_left,
                        elem_type,
                    );
                    changed = true;
                }
            }
        }
        LineCol::WidthNarrow => {
            if let Ok(v) = trimmed.parse::<i32>() {
                let val = v.max(0);
                let cur_str = format_line_cell(
                    line,
                    LineCol::WidthNarrow,
                    is_mm,
                    unit_cm,
                    sign_mode,
                    side,
                    line.is_curve(),
                    is_base_row,
                );
                if cur_str.parse::<i32>().ok() != Some(val) {
                    let func_id = line.function.to_id();
                    if func_id == 2 {
                        let w = line.widening.get_or_insert_with(Default::default);
                        set_active_module_width(&mut w.modules, val, true);
                        line.ensure_child_slot(KanteChildSlot::Widening);
                        if let Some(ref mut cv) = line.curve {
                            for st in &mut cv.steps {
                                st.widening_width = val;
                            }
                        }
                    } else {
                        let n = line.narrowing.get_or_insert_with(Default::default);
                        set_active_module_width(&mut n.modules, val, true);
                        line.ensure_child_slot(KanteChildSlot::Narrowing);
                        if let Some(ref mut cv) = line.curve {
                            for st in &mut cv.steps {
                                st.narrowing_width = val;
                            }
                        }
                    }
                    changed = true;
                }
            }
        }
        LineCol::WidthFadeOut => {
            if let Ok(v) = trimmed.parse::<i32>() {
                let val = v.max(0);
                let cur_str = format_line_cell(
                    line,
                    LineCol::WidthFadeOut,
                    is_mm,
                    unit_cm,
                    sign_mode,
                    side,
                    line.is_curve(),
                    is_base_row,
                );
                if cur_str.parse::<i32>().ok() != Some(val) {
                    let f = line.fade_out.get_or_insert_with(Default::default);
                    set_active_module_width(&mut f.modules, val, false);
                    line.ensure_child_slot(KanteChildSlot::FadeOut);
                    if let Some(ref mut cv) = line.curve {
                        for st in &mut cv.steps {
                            st.fade_out_width = val;
                        }
                    }
                    changed = true;
                }
            }
        }
        LineCol::Comment => {
            if line.comment != raw_text {
                line.comment = raw_text.to_string();
                changed = true;
            }
        }
        _ => {}
    }

    if changed {
        if is_last_row && line.rows == 0 && (line.last_line != 0 || sum_x_others + line.stitches == 0)
        {
            line.function = LineFunction::None;
        }
        undo.push("Hücre düzenleme", &snapshot);
        if mirrored && side == LineSide::Left {
            sync_mirrored_right_lines(doc, elem_idx);
        }
        if matches!(col, LineCol::Rows | LineCol::StepY | LineCol::Factor) {
            sync_all_endline_elements(doc, &endline_dists);
        }
        doc.dirty = true;
    }
}

fn set_active_module_width(
    modules: &mut Vec<stoll_kalip_editoru::model::ModuleAssignment>,
    val: i32,
    prefer_positive: bool,
) {
    let idx = modules
        .iter()
        .position(|m| m.module_art_id == 0)
        .or_else(|| {
            if prefer_positive {
                modules
                    .iter()
                    .position(|m| m.module_art_id >= 0 && m.width > 0)
            } else {
                None
            }
        })
        .or_else(|| modules.iter().position(|m| m.module_art_id >= 0))
        .or_else(|| if modules.is_empty() { None } else { Some(0) });

    if let Some(i) = idx {
        modules[i].width = val;
    } else {
        modules.push(stoll_kalip_editoru::model::ModuleAssignment {
            module_art_id: 0,
            width: val,
            ..Default::default()
        });
    }
}
