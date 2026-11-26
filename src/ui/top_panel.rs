use egui::{vec2, Align, Color32, CornerRadius, Frame, Layout, Margin, Sense, Stroke, Ui};

use stoll_kalip_editoru::catalog::StollCatalog;
use stoll_kalip_editoru::math::{rows_to_mm, stitches_to_mm, DocumentGeometry, SegmentSpan};
use stoll_kalip_editoru::model::{DisplayUnit, FormDocument, InputFormat, LineSide, UndoStack};

use crate::ui::icons::{paint_icon, IconKind};
use crate::ui::theme::{
    bold_font, colors, regular_font, studio_card_frame, win32_button, win32_checkbox,
    win32_combo_scope, win32_group_box_on, win32_input_frame, win32_numeric_text_edit,
    win32_radio, win32_window,
};
use crate::ui::{shape_cut_type_label_tr, AppAction, UiState, SHAPE_CUT_TYPES};

/// Compact `Başlama / Bitiş / Fark` summary strip at the top of the Right Pane.
/// Displays selected segment(s) start, end, and delta in BOTH `ilmek/sıra` and `cm`.
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

    let span: Option<(usize, usize, &SegmentSpan, &SegmentSpan)> = geom
        .elements
        .get(state.selected_elem)
        .and_then(|eg| {
            let segs = match side {
                LineSide::Left => &eg.left_segments,
                LineSide::Right => {
                    if !eg.right_segments.is_empty() {
                        &eg.right_segments
                    } else if mirrored {
                        &eg.left_segments
                    } else {
                        &eg.right_segments
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
            if rows.is_empty() {
                return None;
            }
            rows.sort_unstable();
            let first = rows[0];
            let last = *rows.last().unwrap();
            Some((first + 1, last + 1, &segs[first], &segs[last]))
        });

    let (no_str, sx_s, ex_s, dx_s, sy_s, ey_s, dy_s, sx_cm, ex_cm, dx_cm, sy_cm, ey_cm, dy_cm) =
        if state.table_mode.is_markers() {
            let marker_span = doc.elements.get(state.selected_elem).and_then(|el| {
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
                if rows.is_empty() {
                    return None;
                }
                rows.sort_unstable();
                let first = rows[0];
                let last = *rows.last().unwrap();
                let p0 = pts[first].offsets.first().map(|o| (o.x, o.y)).unwrap_or((0, 0));
                let p1 = pts[last].offsets.first().map(|o| (o.x, o.y)).unwrap_or((0, 0));
                Some((first + 1, last + 1, p0, p1))
            });
            if let Some((n1, n2, (x0, y0), (x1, y1))) = marker_span {
                let sx = stitches_to_mm(x0, &doc.header) / 10.0;
                let ex = stitches_to_mm(x1, &doc.header) / 10.0;
                let sy = rows_to_mm(y0, &doc.header) / 10.0;
                let ey = rows_to_mm(y1, &doc.header) / 10.0;
                let no_lbl = if n1 == n2 {
                    format!("#{n1}")
                } else {
                    format!("#{n1}–{n2}")
                };
                (
                    no_lbl,
                    x0.to_string(),
                    x1.to_string(),
                    (x1 - x0).to_string(),
                    y0.to_string(),
                    y1.to_string(),
                    (y1 - y0).to_string(),
                    format!("{sx:.1}"),
                    format!("{ex:.1}"),
                    format!("{:.1}", ex - sx),
                    format!("{sy:.1}"),
                    format!("{ey:.1}"),
                    format!("{:.1}", ey - sy),
                )
            } else {
                let z = "0".to_string();
                let zc = "0.0".to_string();
                (
                    "-".to_string(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z.clone(),
                    z,
                    zc.clone(),
                    zc.clone(),
                    zc.clone(),
                    zc.clone(),
                    zc.clone(),
                    zc,
                )
            }
        } else if let Some((n1, n2, first, last)) = span {
            let sx = first.start_mm.x / 10.0;
            let ex = last.end_mm.x / 10.0;
            let sy = first.start_mm.y / 10.0;
            let ey = last.end_mm.y / 10.0;
            let no_lbl = if n1 == n2 {
                format!("#{n1}")
            } else {
                format!("#{n1}–{n2}")
            };
            (
                no_lbl,
                first.start_st.0.to_string(),
                last.end_st.0.to_string(),
                (last.end_st.0 - first.start_st.0).to_string(),
                first.start_st.1.to_string(),
                last.end_st.1.to_string(),
                (last.end_st.1 - first.start_st.1).to_string(),
                format!("{sx:.1}"),
                format!("{ex:.1}"),
                format!("{:.1}", ex - sx),
                format!("{sy:.1}"),
                format!("{ey:.1}"),
                format!("{:.1}", ey - sy),
            )
        } else {
            let z = "0".to_string();
            let zc = "0.0".to_string();
            (
                "-".to_string(),
                z.clone(),
                z.clone(),
                z.clone(),
                z.clone(),
                z.clone(),
                z,
                zc.clone(),
                zc.clone(),
                zc.clone(),
                zc.clone(),
                zc.clone(),
                zc,
            )
        };

    let avail_w = ui.available_width();
    let scale = (1.0 + ((avail_w - 560.0) / 560.0).max(0.0) * 0.28).clamp(1.0, 1.28);

    studio_card_frame()
        .inner_margin(Margin::symmetric(
            (10.0 * scale).round() as i8,
            (6.0 * scale).round() as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0 * scale;

                // Side & Selection badge
                let side_lbl = match side {
                    LineSide::Left => "Sol",
                    LineSide::Right => "Sağ",
                };
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0 * scale;
                    ui.label(
                        egui::RichText::new(format!("{side_lbl} {no_str}"))
                            .font(bold_font(11.5 * scale))
                            .color(colors::WIN_BLUE),
                    );
                    ui.label(
                        egui::RichText::new("Seçili aralık")
                            .font(regular_font(9.5 * scale))
                            .color(colors::TEXT_SECONDARY),
                    );
                });

                summary_divider(ui, scale);

                // Width (En / X)
                summary_metric_group(
                    ui,
                    IconKind::HorizontalArrow,
                    "En (X)",
                    &format!("{sx_s} → {ex_s}"),
                    &format!("Δ {dx_s} ilmek"),
                    &format!("{sx_cm} → {ex_cm} cm (Δ {dx_cm})"),
                    scale,
                );

                summary_divider(ui, scale);

                // Height (Boy / Y)
                summary_metric_group(
                    ui,
                    IconKind::VerticalArrow,
                    "Boy (Y)",
                    &format!("{sy_s} → {ey_s}"),
                    &format!("Δ {dy_s} sıra"),
                    &format!("{sy_cm} → {ey_cm} cm (Δ {dy_cm})"),
                    scale,
                );
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

/// Floating/modal `⚙ Kalıp Ayarları` window containing all classic Stoll settings
/// tucked away from the main view (`Genel kesim verileri`, `Standart özellikler`, `Yönergeler`).
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
    win32_window(
        ctx,
        egui::Id::new("studio_settings_popover"),
        "Kalıp Ayarları (Genel Veriler, Standart Özellikler, Yönergeler)",
        &mut open,
        Some(vec2(575.0, 560.0)),
        true,
        true,
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
                                                let in_lbl = match doc.header.input_format {
                                                    InputFormat::Steps | InputFormat::Stitches => "İlmekler",
                                                    InputFormat::Stufung => "Basamaklar",
                                                    InputFormat::Mm => "mm",
                                                };
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

                            win32_group_box_on(ui, "Kanvas & Görünüm:", colors::CHROME_BG, |ui| {
                                if win32_checkbox(ui, &mut state.show_dimensions, "Kanvas ölçü etiketleri").changed() {
                                    state.user_settings.show_dimensions = state.show_dimensions;
                                    actions.push(AppAction::SaveUserSettings);
                                }
                                if win32_checkbox(ui, &mut state.snap_even_rows, "Çift sıra yakalama (2'şer)").changed() {
                                    state.user_settings.snap_even_rows = state.snap_even_rows;
                                    actions.push(AppAction::SaveUserSettings);
                                }
                                if win32_checkbox(ui, &mut state.show_graphics, "Klasik Grafik penceresi").changed() {
                                    actions.push(AppAction::ToggleGraphics);
                                }
                            });
                        });

                        // ── Right column: Standart özellikler + Yönergeler ────
                        cols[1].vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;

                            win32_group_box_on(ui, "Standart özellikler:", colors::CHROME_BG, |ui| {
                                let can_apply_lines = !state.selected_rows.is_empty();
                                ui.horizontal(|ui| {
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
                                });

                                let presets = catalog.presets(state.preset_is_user);
                                if !presets.is_empty() && state.selected_preset_idx >= presets.len() {
                                    state.selected_preset_idx = 0;
                                }
                                let selected_label = presets
                                    .get(state.selected_preset_idx)
                                    .map(|p| p.display_name.as_str())
                                    .unwrap_or("(Özel tanım yok)");

                                let prev_idx = state.selected_preset_idx;
                                win32_combo_scope(ui, |ui| {
                                    egui::ComboBox::from_id_salt("settings_std_props_combo")
                                        .width(220.0)
                                        .selected_text(selected_label)
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
                                if state.selected_preset_idx != prev_idx {
                                    state.user_settings.default_template_index = state.selected_preset_idx;
                                    actions.push(AppAction::SaveUserSettings);
                                }

                                if win32_button(
                                    ui,
                                    "Seçili çizgiye devir al",
                                    vec2(148.0, 22.0),
                                    can_apply_lines && !presets.is_empty(),
                                )
                                .clicked()
                                {
                                    actions.push(AppAction::ApplyStandardPreset);
                                }
                            });

                            win32_group_box_on(ui, "Yönergeler:", colors::CHROME_BG, |ui| {
                                let can_apply = !state.selected_rows.is_empty();
                                directive_combo_row(ui, "Reglan:", "s_dir_reglan", &mut state.directive_narrowing);
                                directive_combo_row(ui, "Artırma:", "s_dir_artirma", &mut state.directive_widening);
                                directive_combo_row(ui, "Boy:", "s_dir_boy", &mut state.directive_height);
                                if win32_button(ui, "devir al", vec2(105.0, 22.0), can_apply).clicked() {
                                    actions.push(AppAction::ApplyDirectives);
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

fn directive_combo_row(ui: &mut Ui, label: &str, id_salt: &str, val: &mut i32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.allocate_ui_with_layout(
            vec2(54.0, 18.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(egui::RichText::new(label).font(regular_font(11.0)));
            },
        );
        win32_combo_scope(ui, |ui| {
            egui::ComboBox::from_id_salt(id_salt)
                .width(50.0)
                .selected_text(val.to_string())
                .show_ui(ui, |ui| {
                    for n in 1..=20 {
                        ui.selectable_value(val, n, n.to_string());
                    }
                });
        });
        inline_micro_icon(ui, IconKind::StitchLoop, 14.0);
    });
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
