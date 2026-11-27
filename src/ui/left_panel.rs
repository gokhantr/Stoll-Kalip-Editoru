use egui::{vec2, Align, Color32, CornerRadius, Layout, Margin, Rect, Sense, Stroke, StrokeKind, Ui};

use stoll_kalip_editoru::math::{
    element_distance_from_top, mm_to_stitches, recalc_all_auto_line_functions, round2,
    set_element_distance_from_bottom, set_element_distance_from_top, stitches_to_mm,
    sync_mirrored_right_lines, DocumentGeometry,
};
use stoll_kalip_editoru::model::{
    ElementType, FormDocument, InputFormat, LineSide, UndoStack, YReferenceLine,
};

use crate::ui::menu_toolbar::paint_shape_thumbnail;
use crate::ui::theme::{
    bold_font, colors, regular_font, studio_card_frame, win32_button, win32_checkbox,
    win32_combo_scope, win32_input_frame, win32_numeric_text_edit, win32_radio,
};
use crate::ui::{
    element_art_label_tr, AppAction, TableMode, UiState, SECONDARY_ELEMENT_TYPES,
};

fn element_short_tag_tr(art_tag_id: i32) -> &'static str {
    match art_tag_id {
        1 => "Ana",
        2 => "Yaka",
        3 => "Kol",
        4 => "Delik",
        5 => "Tik",
        6 => "Açık",
        7 => "Yaka-İ",
        8 => "Del-İ",
        9 => "Açık-İ",
        10 => "Pens",
        _ => "Ana",
    }
}

/// Clean Light Studio `Elemanlar` card placed directly beneath the Left `CadCanvas`.
/// Houses a crisp header bar (`Elemanlar (i/n)` | actions | `Aynalama`), a scrollable
/// `48×54` element thumbnail strip on the left, and a structured 2-row form grid on the right.
pub fn show_elements_strip(
    ui: &mut Ui,
    doc: &mut FormDocument,
    geom: &DocumentGeometry,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
) {
    let elem_idx = state.selected_elem.min(doc.elements.len().saturating_sub(1));
    let elem_count = doc.elements.len();
    let is_secondary = elem_idx > 0;
    let is_neckline = doc
        .elements
        .get(elem_idx)
        .map(|e| e.art_tag_id == 2)
        .unwrap_or(false);
    let cur_mirrored = doc.elements.get(elem_idx).map(|e| e.mirrored).unwrap_or(true);

    studio_card_frame()
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;

            // ── Header Bar: `Elemanlar (1/3)` | Actions | `Aynalama` (wraps cleanly on narrow panes) ──
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                ui.label(
                    egui::RichText::new(format!("Elemanlar ({}/{})", elem_idx + 1, elem_count))
                        .font(bold_font(11.5))
                        .color(colors::TEXT_BLACK),
                );
                if win32_button(ui, "+ Ekle", vec2(52.0, 20.0), true).clicked() {
                    actions.push(AppAction::AddElement);
                }
                if win32_button(ui, "Sil", vec2(36.0, 20.0), is_secondary).clicked() {
                    actions.push(AppAction::DeleteElement);
                }
                if is_neckline && win32_button(ui, "Yaka Öz.", vec2(58.0, 20.0), true).clicked() {
                    actions.push(AppAction::OpenNecklineProps);
                }
                let mut m = cur_mirrored;
                if win32_checkbox(ui, &mut m, "Aynalama").changed() {
                    actions.push(AppAction::MirrorToggled(m));
                }
            });

            // Thin header divider
            let div_rect = ui.available_rect_before_wrap();
            ui.painter().line_segment(
                [
                    egui::pos2(div_rect.left(), div_rect.top()),
                    egui::pos2(div_rect.right(), div_rect.top()),
                ],
                Stroke::new(1.0_f32, colors::BORDER_LIGHT),
            );
            ui.add_space(2.0);

            // ── Responsive Split: 3×2 Thumbnail Grid | Form Grid (stacks cleanly on narrow splitter) ──
            let panel_w = ui.available_width();
            let narrow_stack = panel_w < 430.0;
            let strip_max_w = if narrow_stack {
                panel_w
            } else if elem_count > 6 {
                168.0
            } else {
                156.0
            };
            let show_thumbs = |ui: &mut Ui, doc: &mut FormDocument, undo: &mut UndoStack, state: &mut UiState| {
                egui::ScrollArea::vertical()
                    .id_salt("elem_thumbs_scroll_studio")
                    .max_width(strip_max_w)
                    .max_height(114.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let cols = 3usize;
                        let total = doc.elements.len();
                        let rows = total.div_ceil(cols).max(1);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 5.0;
                            for row in 0..rows {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    for col in 0..cols {
                                        let i = row * cols + col;
                                        if i >= total {
                                            break;
                                        }
                                        let is_active = state.selected_elem == i;
                                        let is_visible = state.all_elements_selected || is_active;
                                        let (rect, resp) =
                                            ui.allocate_exact_size(vec2(48.0, 52.0), Sense::click());

                                        let thumb_rect = Rect::from_min_max(
                                            rect.min,
                                            egui::pos2(rect.right(), rect.bottom() - 15.0),
                                        );
                                        let badge_rect = Rect::from_min_max(
                                            egui::pos2(rect.left(), rect.bottom() - 15.0),
                                            rect.max,
                                        );

                                        if let Some(eg) = geom.elements.get(i) {
                                            let mut lines: Vec<(&[stoll_kalip_editoru::math::Vec2], Color32, f32)> =
                                                Vec::new();
                                            if !eg.left_polyline.is_empty() {
                                                lines.push((
                                                    &eg.left_polyline,
                                                    colors::CONTOUR_GREEN,
                                                    1.0,
                                                ));
                                            }
                                            if !eg.right_polyline.is_empty() {
                                                lines.push((
                                                    &eg.right_polyline,
                                                    colors::CONTOUR_GREEN,
                                                    1.0,
                                                ));
                                            }
                                            let b = if eg.bounds.0.is_finite() {
                                                eg.bounds
                                            } else {
                                                (-100.0, 100.0, 0.0, 200.0)
                                            };
                                            paint_shape_thumbnail(
                                                ui.painter(),
                                                thumb_rect,
                                                &lines,
                                                b,
                                                true,
                                                None,
                                            );
                                        } else {
                                            paint_shape_thumbnail(
                                                ui.painter(),
                                                thumb_rect,
                                                &[],
                                                (-100.0, 100.0, 0.0, 200.0),
                                                true,
                                                None,
                                            );
                                        }

                                        let badge_bg = if is_active {
                                            colors::WIN_BLUE_FILL
                                        } else if is_visible {
                                            Color32::from_rgb(0xEE, 0xF5, 0xFF)
                                        } else {
                                            colors::INPUT_DISABLED_BG
                                        };
                                        let badge_fg = if is_active {
                                            colors::WIN_BLUE
                                        } else if is_visible {
                                            Color32::from_rgb(0x1E, 0x40, 0xAF)
                                        } else {
                                            colors::TEXT_BLACK
                                        };
                                        ui.painter().rect_filled(badge_rect, CornerRadius::ZERO, badge_bg);
                                        ui.painter().text(
                                            badge_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            format!("{}·{}", i + 1, element_short_tag_tr(doc.elements[i].art_tag_id)),
                                            if is_active { bold_font(9.5) } else { regular_font(9.5) },
                                            badge_fg,
                                        );

                                        let (border_col, border_w) = if is_active {
                                            (colors::WIN_BLUE, 2.0_f32)
                                        } else if resp.hovered() {
                                            (colors::WIN_BLUE, 1.3_f32)
                                        } else if is_visible {
                                            (Color32::from_rgb(0x93, 0xC5, 0xFD), 1.2_f32)
                                        } else {
                                            (colors::BORDER_DARK, 1.0_f32)
                                        };
                                        ui.painter().rect_stroke(
                                            rect,
                                            CornerRadius::same(4),
                                            Stroke::new(border_w, border_col),
                                            StrokeKind::Inside,
                                        );

                                        if resp.clicked() {
                                            if state.selected_elem == i && elem_count > 1 {
                                                // Toggle between single selected element and all elements visible
                                                state.all_elements_selected = !state.all_elements_selected;
                                            } else {
                                                if state.selected_elem != i {
                                                    crate::ui::grid_table::flush_active_grid_edit(doc, undo, state);
                                                    let old_idx = state.selected_elem;
                                                    ui.ctx().data_mut(|d| {
                                                        d.remove::<String>(ui.make_persistent_id(("studio_elem_name", old_idx)));
                                                    });
                                                }
                                                state.selected_elem = i;
                                                state.all_elements_selected = false;
                                                state.editing_cell = None;
                                                if doc.elements[i].mirrored {
                                                    state.active_table_side = LineSide::Left;
                                                    state.right_panel_collapsed = true;
                                                    if matches!(
                                                        state.table_mode,
                                                        TableMode::RightLines | TableMode::RightMarkers
                                                    ) {
                                                        state.table_mode = TableMode::LeftLines;
                                                    }
                                                } else {
                                                    state.right_panel_collapsed = false;
                                                }
                                                let active_len = if state.table_mode.is_markers() {
                                                    match state.table_mode.side() {
                                                        LineSide::Left => doc.elements[i].left_points.len(),
                                                        LineSide::Right => doc.elements[i].right_points.len(),
                                                    }
                                                } else {
                                                    doc.elements[i].lines(state.table_mode.side()).len()
                                                };
                                                if active_len > 0 {
                                                    state.select_single_row(0);
                                                    state.focused_cell = Some((0, 2));
                                                } else {
                                                    state.selected_rows.clear();
                                                    state.focused_cell = None;
                                                }
                                            }
                                        }
                                        let mut tip = if doc.elements[i].name.trim().is_empty() {
                                            format!(
                                                "{}: {}",
                                                i + 1,
                                                element_art_label_tr(doc.elements[i].art_tag_id)
                                            )
                                        } else {
                                            format!(
                                                "{}: {} ({})",
                                                i + 1,
                                                element_art_label_tr(doc.elements[i].art_tag_id),
                                                doc.elements[i].name.trim()
                                            )
                                        };
                                        if elem_count > 1 {
                                            tip.push_str("\n(Seçili iken tekrar tıklayarak tek/tüm eleman görünümü değiştir)");
                                        }
                                        resp.on_hover_text(tip);
                                    }
                                });
                            }
                        });
                    });
            };

            let show_form = |ui: &mut Ui, doc: &mut FormDocument, undo: &mut UndoStack, state: &mut UiState| {
                // Right: Structured 2-Row Form Grid
                let is_mm_mode = doc.header.input_format == InputFormat::Mm;
                let hdr = doc.header.clone();
                let (dist_from_top_rows, dist_from_top_mm) =
                    element_distance_from_top(doc, elem_idx);
                let mut pending_start_width: Option<f64> = None;
                let mut pending_offset_x: Option<f64> = None;
                let mut pending_offset_x_base: Option<f64> = None;
                let mut pending_y_bottom: Option<f64> = None;
                let mut pending_y_top: Option<f64> = None;
                let mut pending_art_tag: Option<i32> = None;
                let mut pending_y_ref: Option<YReferenceLine> = None;
                let mut pending_dist_halves: Option<i32> = None;
                let mut pending_name: Option<String> = None;

                if let Some(el) = doc.elements.get(elem_idx) {
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 8.0;

                        // ── Row A: Tür | Ad | Başlama eni | (Primary: Yarım şekil mesafesi) ──
                        let row_avail = ui.available_width();
                        let compact_row = !is_secondary && row_avail < 525.0;
                        let tur_w = if compact_row { 96.0 } else { 110.0 };
                        let name_w = if is_secondary {
                            (row_avail - 290.0).clamp(72.0, 150.0)
                        } else if compact_row {
                            62.0
                        } else {
                            82.0
                        };
                        let sw_w = if compact_row { 40.0 } else { 46.0 };

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(if compact_row { 4.0 } else { 6.0 }, 4.0);
                            ui.label(
                                egui::RichText::new("Tür:")
                                    .font(regular_font(10.5))
                                    .color(colors::TEXT_SECONDARY),
                            );
                            let cur_lbl = element_art_label_tr(el.art_tag_id);
                            if !is_secondary {
                                ui.add_enabled_ui(false, |ui| {
                                    win32_combo_scope(ui, |ui| {
                                        egui::ComboBox::from_id_salt("studio_elem_tur_primary")
                                            .width(tur_w)
                                            .selected_text("Ana Kesim")
                                            .show_ui(ui, |_| {});
                                    });
                                });
                            } else {
                                let mut art_id = el.art_tag_id;
                                win32_combo_scope(ui, |ui| {
                                    egui::ComboBox::from_id_salt("studio_elem_tur_secondary")
                                        .width(tur_w)
                                        .selected_text(cur_lbl)
                                        .show_ui(ui, |ui| {
                                            for &(id, lbl) in SECONDARY_ELEMENT_TYPES {
                                                ui.selectable_value(&mut art_id, id, lbl);
                                            }
                                        });
                                });
                                if art_id != el.art_tag_id {
                                    pending_art_tag = Some(art_id);
                                }
                            }

                            ui.label(
                                egui::RichText::new("Ad:")
                                    .font(regular_font(10.5))
                                    .color(colors::TEXT_SECONDARY),
                            );
                            let name_mem_id = ui.make_persistent_id(("studio_elem_name", elem_idx));
                            let mut name_buf: String = ui
                                .ctx()
                                .data_mut(|d| d.get_temp::<String>(name_mem_id))
                                .unwrap_or_else(|| el.name.clone());
                            let mut name_resp_opt: Option<egui::Response> = None;
                            win32_input_frame(true).show(ui, |ui| {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut name_buf)
                                        .font(regular_font(10.5))
                                        .frame(false)
                                        .desired_width(name_w),
                                );
                                name_resp_opt = Some(resp);
                            });
                            if let Some(resp) = name_resp_opt {
                                let enter =
                                    resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let esc = resp.has_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Escape));
                                if esc {
                                    ui.ctx().data_mut(|d| d.remove::<String>(name_mem_id));
                                    resp.surrender_focus();
                                } else if enter || resp.lost_focus() {
                                    if name_buf != el.name {
                                        pending_name = Some(name_buf);
                                    }
                                    ui.ctx().data_mut(|d| d.remove::<String>(name_mem_id));
                                    if enter {
                                        resp.surrender_focus();
                                    }
                                } else if resp.has_focus() {
                                    ui.ctx().data_mut(|d| d.insert_temp(name_mem_id, name_buf));
                                } else {
                                    ui.ctx().data_mut(|d| d.remove::<String>(name_mem_id));
                                }
                            }

                            ui.label(
                                egui::RichText::new("Başlama eni:")
                                    .font(regular_font(10.5))
                                    .color(colors::TEXT_SECONDARY),
                            );
                            let sw_disp = if is_mm_mode {
                                format!("{:.1}", el.start_width)
                            } else {
                                format!("{:.0}", el.start_width)
                            };
                            let sw_enabled = !matches!(
                                el.element_type(),
                                ElementType::Neckline
                                    | ElementType::Armhole
                                    | ElementType::Opening
                                    | ElementType::Hole
                            );
                            if let Some(txt) = win32_numeric_text_edit(
                                ui,
                                ("studio_start_width", elem_idx),
                                sw_disp,
                                sw_w,
                                sw_enabled,
                            ) {
                                if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                                    if v.is_finite() {
                                        let clamped = v.max(0.0);
                                        if (clamped - el.start_width).abs() > 1e-6 {
                                            pending_start_width = Some(clamped);
                                        }
                                    }
                                }
                            }

                            if !is_secondary {
                                ui.label(
                                    egui::RichText::new("Yarım şekil mesafesi:")
                                        .font(regular_font(10.5))
                                        .color(colors::TEXT_SECONDARY),
                                );
                                let mut dh = el.distance_halves;
                                win32_combo_scope(ui, |ui| {
                                    egui::ComboBox::from_id_salt("studio_yarim_sekil_mesafe")
                                        .width(42.0)
                                        .selected_text(dh.to_string())
                                        .show_ui(ui, |ui| {
                                            for v in [0, 1, 2, 4, 6, 8, 10, 12, 16, 20] {
                                                ui.selectable_value(&mut dh, v, v.to_string());
                                            }
                                        });
                                });
                                if dh != el.distance_halves {
                                    pending_dist_halves = Some(dh);
                                }
                            }
                        });

                        // ── Row B: X & Y Position Grid ───────────────────────────────────
                        let disp_x = if is_mm_mode {
                            format!("{:.1}", el.offset_x_mm)
                        } else {
                            format!("{:.0}", el.offset_x_stitches)
                        };
                        let disp_x_base = if is_mm_mode {
                            format!("{:.1}", el.offset_x_mm_base)
                        } else {
                            format!("{:.0}", el.offset_x_stitches_base)
                        };
                        let x_unit = if is_mm_mode { "mm" } else { "iğne" };
                        let y_unit = if is_mm_mode { "mm" } else { "may" };

                        if is_secondary {
                            ui.columns(2, |cols| {
                                // Left sub-card: x mesafesi
                                egui::Frame::NONE
                                    .fill(colors::INPUT_DISABLED_BG)
                                    .corner_radius(CornerRadius::same(4))
                                    .stroke(Stroke::new(1.0_f32, colors::BORDER_LIGHT))
                                    .inner_margin(Margin::symmetric(8, 6))
                                    .show(&mut cols[0], |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.spacing_mut().item_spacing.y = 5.0;
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new("x mesafesi")
                                                    .font(bold_font(10.5))
                                                    .color(colors::TEXT_BLACK),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    unit_badge(ui, x_unit);
                                                },
                                            );
                                        });
                                        egui::Grid::new(("studio_x_pos_grid", elem_idx))
                                            .num_columns(2)
                                            .spacing([8.0, 5.0])
                                            .show(ui, |ui| {
                                                ui.label(
                                                    egui::RichText::new("Orta eksen:")
                                                        .font(regular_font(10.5))
                                                        .color(colors::TEXT_SECONDARY),
                                                );
                                                if let Some(txt) = win32_numeric_text_edit(
                                                    ui,
                                                    ("studio_off_x", elem_idx),
                                                    disp_x.clone(),
                                                    54.0,
                                                    true,
                                                ) {
                                                    if let Ok(v) =
                                                        txt.trim().replace(',', ".").parse::<f64>()
                                                    {
                                                        if v.is_finite() {
                                                            pending_offset_x = Some(v);
                                                        }
                                                    }
                                                }
                                                ui.end_row();

                                                ui.label(
                                                    egui::RichText::new("Ana kesimin orta ekseni:")
                                                        .font(regular_font(10.5))
                                                        .color(colors::TEXT_SECONDARY),
                                                );
                                                if let Some(txt) = win32_numeric_text_edit(
                                                    ui,
                                                    ("studio_off_x_base", elem_idx),
                                                    disp_x_base,
                                                    54.0,
                                                    true,
                                                ) {
                                                    if let Ok(v) =
                                                        txt.trim().replace(',', ".").parse::<f64>()
                                                    {
                                                        if v.is_finite() {
                                                            pending_offset_x_base = Some(v);
                                                        }
                                                    }
                                                }
                                                ui.end_row();
                                            });
                                    });

                                // Right sub-card: y mesafesi
                                egui::Frame::NONE
                                    .fill(colors::INPUT_DISABLED_BG)
                                    .corner_radius(CornerRadius::same(4))
                                    .stroke(Stroke::new(1.0_f32, colors::BORDER_LIGHT))
                                    .inner_margin(Margin::symmetric(8, 6))
                                    .show(&mut cols[1], |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.spacing_mut().item_spacing.y = 5.0;
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new("y mesafesi")
                                                    .font(bold_font(10.5))
                                                    .color(colors::TEXT_BLACK),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    unit_badge(ui, y_unit);
                                                },
                                            );
                                        });
                                        let is_base = el.y_ref_line == YReferenceLine::BaseLine;
                                        let mut y_ref = el.y_ref_line;
                                        egui::Grid::new(("studio_y_pos_grid", elem_idx))
                                            .num_columns(2)
                                            .spacing([8.0, 5.0])
                                            .show(ui, |ui| {
                                                if win32_radio(
                                                    ui,
                                                    &mut y_ref,
                                                    YReferenceLine::BaseLine,
                                                    "Zemin çizgi",
                                                )
                                                .changed()
                                                {
                                                    pending_y_ref = Some(y_ref);
                                                }
                                                let disp_yb = if is_mm_mode {
                                                    format!("{:.1}", el.offset_y_mm)
                                                } else {
                                                    format!("{:.0}", el.offset_y_rows)
                                                };
                                                if let Some(txt) = win32_numeric_text_edit(
                                                    ui,
                                                    ("studio_off_y_base", elem_idx),
                                                    disp_yb,
                                                    54.0,
                                                    is_base,
                                                ) {
                                                    if let Ok(v) =
                                                        txt.trim().replace(',', ".").parse::<f64>()
                                                    {
                                                        if v.is_finite() {
                                                            pending_y_bottom = Some(v);
                                                        }
                                                    }
                                                }
                                                ui.end_row();

                                                if win32_radio(
                                                    ui,
                                                    &mut y_ref,
                                                    YReferenceLine::EndLine,
                                                    "Bitiş çizgisi",
                                                )
                                                .changed()
                                                {
                                                    pending_y_ref = Some(y_ref);
                                                }
                                                let top_disp = if is_mm_mode {
                                                    format!("{:.1}", dist_from_top_mm)
                                                } else {
                                                    format!("{:.0}", dist_from_top_rows)
                                                };
                                                if let Some(txt) = win32_numeric_text_edit(
                                                    ui,
                                                    ("studio_off_y_end", elem_idx),
                                                    top_disp,
                                                    54.0,
                                                    !is_base,
                                                ) {
                                                    if let Ok(v) =
                                                        txt.trim().replace(',', ".").parse::<f64>()
                                                    {
                                                        if v.is_finite() {
                                                            pending_y_top = Some(v);
                                                        }
                                                    }
                                                }
                                                ui.end_row();
                                            });
                                    });
                            });
                        } else {
                            // Primary Element Row B: clean aligned `x mesafesi` bar
                            egui::Frame::NONE
                                .fill(colors::INPUT_DISABLED_BG)
                                .corner_radius(CornerRadius::same(4))
                                .stroke(Stroke::new(1.0_f32, colors::BORDER_LIGHT))
                                .inner_margin(Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        ui.label(
                                            egui::RichText::new("x mesafesi:")
                                                .font(bold_font(10.5))
                                                .color(colors::TEXT_BLACK),
                                        );
                                        ui.label(
                                            egui::RichText::new("Orta eksen:")
                                                .font(regular_font(10.5))
                                                .color(colors::TEXT_SECONDARY),
                                        );
                                        if let Some(txt) = win32_numeric_text_edit(
                                            ui,
                                            ("studio_off_x", elem_idx),
                                            disp_x,
                                            56.0,
                                            true,
                                        ) {
                                            if let Ok(v) =
                                                txt.trim().replace(',', ".").parse::<f64>()
                                            {
                                                if v.is_finite() {
                                                    pending_offset_x = Some(v);
                                                }
                                            }
                                        }
                                        unit_badge(ui, x_unit);
                                    });
                                });
                        }
                    });
                }

                if let Some(art_id) = pending_art_tag {
                    undo.push("Eleman türü", doc);
                    let default_basic_sw = doc
                        .basic_element()
                        .map(|e| e.start_width)
                        .filter(|&w| w > 0.0)
                        .unwrap_or(500.0);
                    let mut sync_mirror = false;
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        el.art_tag_id = art_id;
                        let new_t = el.element_type();
                        if new_t == ElementType::Gore && el.start_width <= 0.0 {
                            el.start_width = default_basic_sw;
                        } else if matches!(
                            new_t,
                            ElementType::Neckline
                                | ElementType::Armhole
                                | ElementType::Opening
                                | ElementType::Hole
                        ) {
                            el.start_width = 0.0;
                        }
                        recalc_all_auto_line_functions(el);
                        sync_mirror = el.mirrored;
                        doc.dirty = true;
                    }
                    if sync_mirror {
                        sync_mirrored_right_lines(doc, elem_idx);
                    }
                }
                if let Some(new_name) = pending_name {
                    undo.push("Eleman adı", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        el.name = new_name;
                        doc.dirty = true;
                    }
                }
                if let Some(y_ref) = pending_y_ref {
                    undo.push("Referans çizgisi", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        el.y_ref_line = y_ref;
                        doc.dirty = true;
                    }
                }
                if let Some(dh) = pending_dist_halves {
                    undo.push("Yarım şekillerin mesafesi", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        el.distance_halves = dh;
                        doc.dirty = true;
                    }
                }

                if let Some(v) = pending_start_width {
                    undo.push("Başlama eni", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        el.start_width = v.max(0.0);
                        doc.dirty = true;
                    }
                }
                if let Some(v) = pending_offset_x {
                    undo.push("Orta eksen", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        if is_mm_mode {
                            el.offset_x_mm = round2(v);
                            el.offset_x_stitches = mm_to_stitches(v, &hdr) as f64;
                        } else {
                            let st = v.round();
                            el.offset_x_stitches = st;
                            el.offset_x_mm = round2(stitches_to_mm(st as i32, &hdr));
                        }
                        doc.dirty = true;
                    }
                }
                if let Some(v) = pending_offset_x_base {
                    undo.push("Ana kesimin orta ekseni", doc);
                    if let Some(el) = doc.elements.get_mut(elem_idx) {
                        if is_mm_mode {
                            el.offset_x_mm_base = round2(v);
                            el.offset_x_stitches_base = mm_to_stitches(v, &hdr) as f64;
                        } else {
                            let st = v.round();
                            el.offset_x_stitches_base = st;
                            el.offset_x_mm_base = round2(stitches_to_mm(st as i32, &hdr));
                        }
                        doc.dirty = true;
                    }
                }
                if let Some(v) = pending_y_bottom {
                    let snap = doc.clone();
                    match set_element_distance_from_bottom(doc, elem_idx, v, is_mm_mode) {
                        Ok(()) => undo.push("y mesafesi", &snap),
                        Err(e) => state.set_status(e, true),
                    }
                }
                if let Some(v) = pending_y_top {
                    let snap = doc.clone();
                    match set_element_distance_from_top(doc, elem_idx, v, is_mm_mode) {
                        Ok(()) => undo.push("y mesafesi", &snap),
                        Err(e) => state.set_status(e, true),
                    }
                }
            };

            if narrow_stack {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    show_thumbs(ui, doc, undo, state);
                    show_form(ui, doc, undo, state);
                });
            } else {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    show_thumbs(ui, doc, undo, state);
                    let sep_top = ui.cursor().min;
                    let sep_h = if is_secondary { 118.0 } else { 112.0 };
                    let (_, _) = ui.allocate_exact_size(vec2(1.0, sep_h), Sense::hover());
                    ui.painter().line_segment(
                        [sep_top, egui::pos2(sep_top.x, sep_top.y + sep_h)],
                        Stroke::new(1.0_f32, colors::BORDER_LIGHT),
                    );
                    show_form(ui, doc, undo, state);
                });
            }
        });
}

fn unit_badge(ui: &mut Ui, unit: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(unit.to_owned(), regular_font(9.5), colors::TEXT_SECONDARY);
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 10.0, 16.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(3), colors::CARD_BG);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, colors::BORDER_LIGHT),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        colors::TEXT_SECONDARY,
    );
}
