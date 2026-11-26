use egui::{vec2, Align, Color32, CornerRadius, Frame, Layout, Margin, Sense, Stroke, Ui};

use stoll_kalip_editoru::catalog::StollCatalog;
use stoll_kalip_editoru::math::{rows_to_mm, stitches_to_mm, DocumentGeometry};
use stoll_kalip_editoru::model::{DisplayUnit, FormDocument, LineSide, UndoStack};

use crate::ui::icons::{paint_icon, IconKind};
use crate::ui::theme::{
    bold_font, colors, regular_font, studio_card_frame, win32_button, win32_checkbox,
    win32_combo_scope, win32_group_box_on, win32_input_frame, win32_numeric_text_edit,
    win32_radio,
};
use crate::ui::{
    shape_cut_type_label_tr, show_modal_backdrop, show_studio_modal_window, AppAction, UiState,
    SHAPE_CUT_TYPES,
};

/// Compact `Boy (May)` (LEFT) and `En (İğne)` (RIGHT) summary strip at the top of the Right Pane.
/// Displays single-row start/end/delta or clean `Seçili satırlar (N)` totals when multiple rows are selected.
pub fn show_summary_strip(
    ui: &mut Ui,
    doc: &FormDocument,
    geom: &DocumentGeometry,
    state: &UiState,
) {
    let mirrored = doc
        .elements
        .get(state.selected_elem)
        .map(|e| e.mirrored)
        .unwrap_or(true);
    let side = if mirrored {
        LineSide::Left
    } else {
        state.active_table_side
    };

    enum SummaryData {
        Empty,
        Single {
            no_lbl: String,
            sy_s: String,
            ey_s: String,
            dy_s: String,
            sy_cm: String,
            ey_cm: String,
            dy_cm: String,
            sx_s: String,
            ex_s: String,
            dx_s: String,
            sx_cm: String,
            ex_cm: String,
            dx_cm: String,
        },
        Multi {
            count: usize,
            dy_s: String,
            dy_cm: String,
            dx_s: String,
            dx_cm: String,
        },
    }

    let data = if state.table_mode.is_markers() {
        doc.elements
            .get(state.selected_elem)
            .and_then(|el| {
                let pts = match side {
                    LineSide::Left => &el.left_points,
                    LineSide::Right => &el.right_points,
                };
                if pts.is_empty() {
                    return None;
                }
                let mut rows = state.selected_rows.clone();
                if rows.is_empty() {
                    if let Some((r, _)) = state.focused_cell {
                        rows.push(r);
                    }
                }
                rows.retain(|&r| r < pts.len());
                rows.sort_unstable();
                rows.dedup();
                if rows.is_empty() {
                    return None;
                }
                if rows.len() > 1 {
                    let mut sum_x = 0i32;
                    let mut sum_y = 0i32;
                    for &r in &rows {
                        if let Some(o) = pts[r].offsets.first() {
                            sum_x += o.x;
                            sum_y += o.y;
                        }
                    }
                    let dy_cm = rows_to_mm(sum_y, &doc.header) / 10.0;
                    let dx_cm = stitches_to_mm(sum_x, &doc.header) / 10.0;
                    Some(SummaryData::Multi {
                        count: rows.len(),
                        dy_s: sum_y.to_string(),
                        dy_cm: format!("{dy_cm:.1}"),
                        dx_s: sum_x.to_string(),
                        dx_cm: format!("{dx_cm:.1}"),
                    })
                } else {
                    let r = rows[0];
                    let (x0, y0) = pts[r].offsets.first().map(|o| (o.x, o.y)).unwrap_or((0, 0));
                    let sx = stitches_to_mm(x0, &doc.header) / 10.0;
                    let sy = rows_to_mm(y0, &doc.header) / 10.0;
                    Some(SummaryData::Single {
                        no_lbl: format!("#{}", r + 1),
                        sy_s: "0".to_string(),
                        ey_s: y0.to_string(),
                        dy_s: y0.to_string(),
                        sy_cm: "0.0".to_string(),
                        ey_cm: format!("{sy:.1}"),
                        dy_cm: format!("{sy:.1}"),
                        sx_s: "0".to_string(),
                        ex_s: x0.to_string(),
                        dx_s: x0.to_string(),
                        sx_cm: "0.0".to_string(),
                        ex_cm: format!("{sx:.1}"),
                        dx_cm: format!("{sx:.1}"),
                    })
                }
            })
            .unwrap_or(SummaryData::Empty)
    } else {
        geom.elements
            .get(state.selected_elem)
            .and_then(|eg| {
                let segs = match side {
                    LineSide::Left => &eg.left_segments,
                    LineSide::Right => {
                        if !eg.right_segments.is_empty() || !mirrored {
                            &eg.right_segments
                        } else {
                            &eg.left_segments
                        }
                    }
                };
                if segs.is_empty() {
                    return None;
                }
                let mut rows = state.selected_rows.clone();
                if rows.is_empty() {
                    if let Some((r, _)) = state.focused_cell {
                        rows.push(r);
                    }
                }
                rows.retain(|&r| r < segs.len());
                rows.sort_unstable();
                rows.dedup();
                if rows.is_empty() {
                    return None;
                }
                if rows.len() > 1 {
                    let mut sum_dx = 0i32;
                    let mut sum_dy = 0i32;
                    let mut sum_dx_mm = 0.0_f64;
                    let mut sum_dy_mm = 0.0_f64;
                    for &r in &rows {
                        let s = &segs[r];
                        sum_dx += s.delta_st.0;
                        sum_dy += s.delta_st.1;
                        sum_dx_mm += s.end_mm.x - s.start_mm.x;
                        sum_dy_mm += s.end_mm.y - s.start_mm.y;
                    }
                    Some(SummaryData::Multi {
                        count: rows.len(),
                        dy_s: sum_dy.to_string(),
                        dy_cm: format!("{:.1}", sum_dy_mm / 10.0),
                        dx_s: sum_dx.to_string(),
                        dx_cm: format!("{:.1}", sum_dx_mm / 10.0),
                    })
                } else {
                    let r = rows[0];
                    let seg = &segs[r];
                    let sx = seg.start_mm.x / 10.0;
                    let ex = seg.end_mm.x / 10.0;
                    let sy = seg.start_mm.y / 10.0;
                    let ey = seg.end_mm.y / 10.0;
                    Some(SummaryData::Single {
                        no_lbl: format!("#{}", r + 1),
                        sy_s: seg.start_st.1.to_string(),
                        ey_s: seg.end_st.1.to_string(),
                        dy_s: seg.delta_st.1.to_string(),
                        sy_cm: format!("{sy:.1}"),
                        ey_cm: format!("{ey:.1}"),
                        dy_cm: format!("{:.1}", ey - sy),
                        sx_s: seg.start_st.0.to_string(),
                        ex_s: seg.end_st.0.to_string(),
                        dx_s: seg.delta_st.0.to_string(),
                        sx_cm: format!("{sx:.1}"),
                        ex_cm: format!("{ex:.1}"),
                        dx_cm: format!("{:.1}", ex - sx),
                    })
                }
            })
            .unwrap_or(SummaryData::Empty)
    };

    let avail_w = ui.available_width();
    let scale = (1.0 + ((avail_w - 560.0) / 560.0).max(0.0) * 0.24).clamp(1.0, 1.24);
    let side_lbl = match side {
        LineSide::Left => "Sol",
        LineSide::Right => "Sağ",
    };

    studio_card_frame()
        .inner_margin(Margin::symmetric(
            (10.0 * scale).round() as i8,
            (5.0 * scale).round() as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0 * scale, 4.0);

                match &data {
                    SummaryData::Empty => {
                        ui.label(
                            egui::RichText::new(format!("{side_lbl} —"))
                                .font(bold_font(11.5 * scale))
                                .color(colors::TEXT_SECONDARY),
                        );
                        summary_divider(ui, scale);
                        summary_metric_group(
                            ui,
                            IconKind::VerticalArrow,
                            "Boy (May)",
                            "0 → 0",
                            "Δ 0 may",
                            "0.0 → 0.0 cm (Δ 0.0)",
                            scale,
                        );
                        summary_divider(ui, scale);
                        summary_metric_group(
                            ui,
                            IconKind::HorizontalArrow,
                            "En (İğne)",
                            "0 → 0",
                            "Δ 0 iğne",
                            "0.0 → 0.0 cm (Δ 0.0)",
                            scale,
                        );
                    }
                    SummaryData::Single {
                        no_lbl,
                        sy_s,
                        ey_s,
                        dy_s,
                        sy_cm,
                        ey_cm,
                        dy_cm,
                        sx_s,
                        ex_s,
                        dx_s,
                        sx_cm,
                        ex_cm,
                        dx_cm,
                    } => {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0 * scale;
                            ui.label(
                                egui::RichText::new(format!("{side_lbl} {no_lbl}"))
                                    .font(bold_font(11.5 * scale))
                                    .color(colors::WIN_BLUE),
                            );
                            ui.label(
                                egui::RichText::new("Seçili satır")
                                    .font(regular_font(9.5 * scale))
                                    .color(colors::TEXT_SECONDARY),
                            );
                        });
                        summary_divider(ui, scale);
                        // Boy (May) on the LEFT
                        summary_metric_group(
                            ui,
                            IconKind::VerticalArrow,
                            "Boy (May)",
                            &format!("{sy_s} → {ey_s}"),
                            &format!("Δ {dy_s} may"),
                            &format!("{sy_cm} → {ey_cm} cm (Δ {dy_cm})"),
                            scale,
                        );
                        summary_divider(ui, scale);
                        // En (İğne) on the RIGHT
                        summary_metric_group(
                            ui,
                            IconKind::HorizontalArrow,
                            "En (İğne)",
                            &format!("{sx_s} → {ex_s}"),
                            &format!("Δ {dx_s} iğne"),
                            &format!("{sx_cm} → {ex_cm} cm (Δ {dx_cm})"),
                            scale,
                        );
                    }
                    SummaryData::Multi {
                        count,
                        dy_s,
                        dy_cm,
                        dx_s,
                        dx_cm,
                    } => {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0 * scale;
                            ui.label(
                                egui::RichText::new(format!("Seçili satırlar ({count})"))
                                    .font(bold_font(11.5 * scale))
                                    .color(colors::WIN_BLUE),
                            );
                            ui.label(
                                egui::RichText::new(format!("{side_lbl} toplam"))
                                    .font(regular_font(9.5 * scale))
                                    .color(colors::TEXT_SECONDARY),
                            );
                        });
                        summary_divider(ui, scale);
                        // Boy (May) on the LEFT
                        summary_metric_group(
                            ui,
                            IconKind::VerticalArrow,
                            "Boy (May)",
                            "Toplam:",
                            &format!("Δ {dy_s} may"),
                            &format!("Δ {dy_cm} cm"),
                            scale,
                        );
                        summary_divider(ui, scale);
                        // En (İğne) on the RIGHT
                        summary_metric_group(
                            ui,
                            IconKind::HorizontalArrow,
                            "En (İğne)",
                            "Toplam:",
                            &format!("Δ {dx_s} iğne"),
                            &format!("Δ {dx_cm} cm"),
                            scale,
                        );
                    }
                }
            });
        });
}

fn summary_metric_group(
    ui: &mut Ui,
    icon: IconKind,
    title: &str,
    range_st: &str,
    delta_st: &str,
    cm_line: &str,
    scale: f32,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0 * scale;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0 * scale;
            inline_micro_icon(ui, icon, (13.0 * scale).round());
            ui.label(
                egui::RichText::new(title)
                    .font(bold_font(10.5 * scale))
                    .color(colors::TEXT_SECONDARY),
            );
            ui.label(
                egui::RichText::new(range_st)
                    .font(regular_font(11.0 * scale))
                    .color(colors::TEXT_BLACK),
            );
            ui.label(
                egui::RichText::new(delta_st)
                    .font(bold_font(10.5 * scale))
                    .color(colors::WIN_BLUE),
            );
        });
        ui.label(
            egui::RichText::new(cm_line)
                .font(regular_font(10.0 * scale))
                .color(colors::TEXT_SECONDARY),
        );
    });
}

fn summary_divider(ui: &mut Ui, scale: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(2.0, (28.0 * scale).round()), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top() + 2.0),
            egui::pos2(rect.center().x, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0_f32, colors::BORDER_LIGHT),
    );
}

/// Floating/modal `⚙ Kalıp Ayarları` window containing classic Stoll settings
/// (`Genel kesim verileri`, `Kanvas & Görünüm`, `Varsayılan Şablon & Ayarlar`).
pub fn show_settings_popover(
    ctx: &egui::Context,
    doc: &mut FormDocument,
    catalog: &StollCatalog,
    undo: &mut UndoStack,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
) {
    if !state.settings_popover_open {
        return;
    }

    let mut open = state.settings_popover_open;
    show_modal_backdrop(ctx, "settings_popover_backdrop", false);
    show_studio_modal_window(
        ctx,
        egui::Id::new("studio_settings_popover"),
        "Kalıp Ayarları",
        &mut open,
        vec2(575.0, 520.0),
        true,
        true,
        false,
        |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            egui::ScrollArea::vertical()
                .id_salt("settings_popover_scroll")
                .auto_shrink([false, true])
                .max_height(500.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;

                    ui.columns(2, |cols| {
                        // ── Left column: Genel kesim verileri + Kanvas & Görünüm ──
                        cols[0].vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            win32_group_box_on(ui, "Genel kesim verileri:", colors::CHROME_BG, |ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;

                                ui.label(egui::RichText::new("Tür:").font(regular_font(11.0)));
                                let cut_label = shape_cut_type_label_tr(doc.header.cut_type);
                                let mut next_cut = doc.header.cut_type;
                                win32_combo_scope(ui, |ui| {
                                    egui::ComboBox::from_id_salt("settings_genel_tur")
                                        .width(220.0)
                                        .selected_text(cut_label)
                                        .show_ui(ui, |ui| {
                                            for &(id, lbl) in SHAPE_CUT_TYPES {
                                                ui.selectable_value(&mut next_cut, id, lbl);
                                            }
                                        });
                                });
                                if next_cut != doc.header.cut_type {
                                    undo.push("Kesim türü", doc);
                                    doc.header.cut_type = next_cut;
                                    doc.dirty = true;
                                    actions.push(AppAction::CutTypeChanged);
                                }

                                ui.label(egui::RichText::new("Yapılış tarihi:").font(regular_font(11.0)));
                                let date_str = format_unix_timestamp(doc.header.created_timestamp);
                                win32_input_frame(false).show(ui, |ui| {
                                    ui.set_min_width(214.0);
                                    ui.label(egui::RichText::new(date_str).font(regular_font(10.5)));
                                });

                                ui.label(egui::RichText::new("Yorum:").font(regular_font(11.0)));
                                let cmt_mem_id = ui.make_persistent_id("settings_doc_comment");
                                let mut cmt_buf: String = ui
                                    .ctx()
                                    .data_mut(|d| d.get_temp::<String>(cmt_mem_id))
                                    .unwrap_or_else(|| doc.header.comment.clone());
                                let mut cmt_resp_opt: Option<egui::Response> = None;
                                win32_input_frame(true).show(ui, |ui| {
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut cmt_buf)
                                            .font(regular_font(11.0))
                                            .frame(false)
                                            .desired_width(212.0),
                                    );
                                    cmt_resp_opt = Some(resp);
                                });
                                if let Some(resp) = cmt_resp_opt {
                                    let enter = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let esc = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
                                    if esc {
                                        ui.ctx().data_mut(|d| d.remove::<String>(cmt_mem_id));
                                        resp.surrender_focus();
                                    } else if enter || resp.lost_focus() {
                                        if cmt_buf != doc.header.comment {
                                            undo.push("Kalıp yorumu", doc);
                                            doc.header.comment = cmt_buf;
                                            doc.dirty = true;
                                        }
                                        ui.ctx().data_mut(|d| d.remove::<String>(cmt_mem_id));
                                        if enter {
                                            resp.surrender_focus();
                                        }
                                    } else if resp.has_focus() {
                                        ui.ctx().data_mut(|d| d.insert_temp(cmt_mem_id, cmt_buf));
                                    } else {
                                        ui.ctx().data_mut(|d| d.remove::<String>(cmt_mem_id));
                                    }
                                }

                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new("Giriş formatı:").font(regular_font(10.5)));
                                        ui.add_enabled_ui(false, |ui| {
                                            win32_combo_scope(ui, |ui| {
                                                let in_lbl = doc.header.input_format.label_tr();
                                                egui::ComboBox::from_id_salt("settings_giris_formati")
                                                    .width(102.0)
                                                    .selected_text(in_lbl)
                                                    .show_ui(ui, |_| {});
                                            });
                                        });
                                    });
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new("Görüntü formatı:").font(regular_font(10.5)));
                                        let mut next_unit = doc.header.display_unit;
                                        win32_combo_scope(ui, |ui| {
                                            egui::ComboBox::from_id_salt("settings_goruntu_formati")
                                                .width(102.0)
                                                .selected_text(doc.header.display_unit.label_tr())
                                                .show_ui(ui, |ui| {
                                                    ui.selectable_value(&mut next_unit, DisplayUnit::Mm, "mm");
                                                    ui.selectable_value(&mut next_unit, DisplayUnit::Cm, "cm");
                                                    ui.selectable_value(&mut next_unit, DisplayUnit::Inch, "inç");
                                                });
                                        });
                                        if next_unit != doc.header.display_unit {
                                            undo.push("Görüntü formatı", doc);
                                            doc.header.display_unit = next_unit;
                                            doc.dirty = true;
                                        }
                                    });
                                });

                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.0} mm İlmek Yoğunluğu:",
                                        doc.header.density_base_mm
                                    ))
                                    .font(regular_font(10.5)),
                                );
                                ui.horizontal(|ui| {
                                    inline_micro_icon(ui, IconKind::HorizontalArrow, 14.0);
                                    let mut new_dx: Option<f64> = None;
                                    let mut new_dy: Option<f64> = None;
                                    if let Some(txt) = win32_numeric_text_edit(
                                        ui,
                                        "settings_density_x",
                                        format!("{:.2}", doc.header.density_x),
                                        56.0,
                                        true,
                                    ) {
                                        if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                                            if v > 0.0 && v.is_finite() {
                                                let clamped = v.clamp(1.0, 500.0);
                                                if (clamped - doc.header.density_x).abs() > 1e-6 {
                                                    new_dx = Some(clamped);
                                                }
                                            }
                                        }
                                    }
                                    inline_micro_icon(ui, IconKind::VerticalArrow, 14.0);
                                    if let Some(txt) = win32_numeric_text_edit(
                                        ui,
                                        "settings_density_y",
                                        format!("{:.2}", doc.header.density_y),
                                        56.0,
                                        true,
                                    ) {
                                        if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                                            if v > 0.0 && v.is_finite() {
                                                let clamped = v.clamp(1.0, 500.0);
                                                if (clamped - doc.header.density_y).abs() > 1e-6 {
                                                    new_dy = Some(clamped);
                                                }
                                            }
                                        }
                                    }
                                    if new_dx.is_some() || new_dy.is_some() {
                                        actions.push(AppAction::SetDensity {
                                            density_x: new_dx,
                                            density_y: new_dy,
                                        });
                                    }
                                });

                                let kw_enabled = doc.header.cut_type >= 0;
                                ui.add_enabled_ui(kw_enabled, |ui| {
                                    let mut kaw = doc.header.kaw_all_needles;
                                    if win32_checkbox(ui, &mut kaw, "k&w tüm iğneler").changed() {
                                        undo.push("k&w tüm iğneler", doc);
                                        doc.header.kaw_all_needles = kaw;
                                        doc.dirty = true;
                                    }
                                });
                            });
                        });

                        // ── Right column: Kanvas & Görünüm (no duplicate template box!) ──
                        cols[1].vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;

                            win32_group_box_on(ui, "Kanvas & Görünüm:", colors::CHROME_BG, |ui| {
                                ui.spacing_mut().item_spacing.y = 5.0;
                                if win32_checkbox(ui, &mut state.show_dimensions, "Kanvas ölçü etiketleri").changed() {
                                    state.user_settings.show_dimensions = state.show_dimensions;
                                    actions.push(AppAction::SaveUserSettings);
                                }
                                if win32_checkbox(
                                    ui,
                                    &mut state.user_settings.default_even_courses,
                                    "Varsayılan Çift May",
                                )
                                .changed()
                                {
                                    actions.push(AppAction::SaveUserSettings);
                                }
                                if win32_checkbox(ui, &mut state.show_graphics, "Klasik Grafik penceresi").changed() {
                                    actions.push(AppAction::ToggleGraphics);
                                }
                            });
                        });
                    });

                    // ── Full-width Persistent Settings & Templates (`%APPDATA%\kalipeditor`) ──
                    win32_group_box_on(
                        ui,
                        r"Varsayılan Şablon & Ayarlar (%APPDATA%\kalipeditor):",
                        colors::CHROME_BG,
                        |ui| {
                            ui.spacing_mut().item_spacing.y = 5.0;

                            // Default 1 cm density + Set Current as Default
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    egui::RichText::new("Varsayılan 1 cm Yoğunluk:")
                                        .font(regular_font(10.5)),
                                );
                                inline_micro_icon(ui, IconKind::HorizontalArrow, 13.0);
                                if let Some(txt) = win32_numeric_text_edit(
                                    ui,
                                    "settings_def_dx_1cm",
                                    format!("{:.2}", state.user_settings.default_density_x_per_cm),
                                    52.0,
                                    true,
                                ) {
                                    if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                                        if v.is_finite() && v > 0.0 {
                                            let clamped = v.clamp(0.1, 50.0);
                                            if (clamped - state.user_settings.default_density_x_per_cm).abs() > 1e-6 {
                                                state.user_settings.default_density_x_per_cm = clamped;
                                                actions.push(AppAction::SaveUserSettings);
                                            }
                                        }
                                    }
                                }
                                inline_micro_icon(ui, IconKind::VerticalArrow, 13.0);
                                if let Some(txt) = win32_numeric_text_edit(
                                    ui,
                                    "settings_def_dy_1cm",
                                    format!("{:.2}", state.user_settings.default_density_y_per_cm),
                                    52.0,
                                    true,
                                ) {
                                    if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                                        if v.is_finite() && v > 0.0 {
                                            let clamped = v.clamp(0.1, 50.0);
                                            if (clamped - state.user_settings.default_density_y_per_cm).abs() > 1e-6 {
                                                state.user_settings.default_density_y_per_cm = clamped;
                                                actions.push(AppAction::SaveUserSettings);
                                            }
                                        }
                                    }
                                }
                                if win32_button(
                                    ui,
                                    "Mevcut yoğunluğu varsayılan yap",
                                    vec2(178.0, 20.0),
                                    true,
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::SetCurrentDensityAsDefault);
                                }
                            });

                            // Auto-apply default template on new lines
                            if win32_checkbox(
                                ui,
                                &mut state.user_settings.auto_apply_default_template,
                                "Yeni satırlara varsayılan şablonu otomatik uygula",
                            )
                            .changed()
                            {
                                actions.push(AppAction::SaveUserSettings);
                            }

                            // Active default template selector (synced with Standart özellikler)
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    egui::RichText::new("Aktif şablon:")
                                        .font(regular_font(10.5)),
                                );
                                if win32_radio(ui, &mut state.preset_is_user, false, "Stoll").changed() {
                                    state.selected_preset_idx = 0;
                                    state.user_settings.default_template_is_user = false;
                                    state.user_settings.default_template_index = 0;
                                    actions.push(AppAction::SaveUserSettings);
                                }
                                if win32_radio(ui, &mut state.preset_is_user, true, "Kendinin / Özel").changed() {
                                    state.selected_preset_idx = 0;
                                    state.user_settings.default_template_is_user = true;
                                    state.user_settings.default_template_index = 0;
                                    actions.push(AppAction::SaveUserSettings);
                                }

                                let presets = catalog.presets(state.preset_is_user);
                                if !presets.is_empty() && state.selected_preset_idx >= presets.len() {
                                    state.selected_preset_idx = 0;
                                }
                                let active_lbl = presets
                                    .get(state.selected_preset_idx)
                                    .map(|p| p.display_name.as_str())
                                    .unwrap_or("(Özel tanım yok)");
                                let prev_i = state.selected_preset_idx;
                                win32_combo_scope(ui, |ui| {
                                    egui::ComboBox::from_id_salt("settings_active_default_tpl")
                                        .width(210.0)
                                        .selected_text(active_lbl)
                                        .show_ui(ui, |ui| {
                                            for (i, p) in presets.iter().enumerate() {
                                                ui.selectable_value(
                                                    &mut state.selected_preset_idx,
                                                    i,
                                                    &p.display_name,
                                                );
                                            }
                                        });
                                });
                                if state.selected_preset_idx != prev_i {
                                    state.user_settings.default_template_index = state.selected_preset_idx;
                                    actions.push(AppAction::SaveUserSettings);
                                }
                            });

                            // Apply active template to selected line(s)
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let presets = catalog.presets(state.preset_is_user);
                                let can_apply = !state.selected_rows.is_empty() && !presets.is_empty();
                                if win32_button(
                                    ui,
                                    "Seçili satıra şablon uygula",
                                    vec2(174.0, 22.0),
                                    can_apply,
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::ApplyStandardPreset);
                                }
                            });

                            // Save selected line as custom preset + Delete custom preset
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    egui::RichText::new("Şablon adı:")
                                        .font(regular_font(10.5)),
                                );
                                win32_input_frame(true).show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut state.preset_save_name)
                                            .font(regular_font(10.5))
                                            .frame(false)
                                            .hint_text("Örn: 1x1 Reglanım")
                                            .desired_width(142.0),
                                    );
                                });
                                let can_save = !state.selected_rows.is_empty();
                                if win32_button(
                                    ui,
                                    "Seçili satırı şablon kaydet",
                                    vec2(160.0, 21.0),
                                    can_save,
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::SaveSelectedLineAsUserPreset);
                                }
                                let can_del = state.preset_is_user && !catalog.user_presets.is_empty();
                                if win32_button(
                                    ui,
                                    "Özel şablonu sil",
                                    vec2(114.0, 21.0),
                                    can_del,
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::DeleteSelectedUserPreset);
                                }
                            });

                            // Export / Import .kalipcfg bundle
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                if win32_button(
                                    ui,
                                    "Ayarları Dışa Aktar...",
                                    vec2(148.0, 22.0),
                                    true,
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::ExportSettingsBundle);
                                }
                                if win32_button(
                                    ui,
                                    "Ayarları İçe Aktar...",
                                    vec2(148.0, 22.0),
                                    true,
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::ImportSettingsBundle);
                                }
                            });
                        },
                    );
                });

            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if win32_button(ui, "Kapat", vec2(80.0, 24.0), true).clicked() {
                    state.settings_popover_open = false;
                }
            });
        },
    );

    if !open {
        state.settings_popover_open = false;
    }
    let _ = Frame::NONE;
    let _ = Color32::WHITE;
    let _ = CornerRadius::ZERO;
    let _ = DisplayUnit::Mm;
}

fn inline_micro_icon(ui: &mut Ui, kind: IconKind, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), rect, kind, true);
}

fn format_unix_timestamp(ts: i64) -> String {
    use chrono::{DateTime, Local, Utc};
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| {
            let local: DateTime<Local> = dt.into();
            local.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_else(|| "1970-01-01 00:00".to_string())
}
