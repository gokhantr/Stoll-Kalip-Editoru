use egui::{pos2, vec2, Align2, Color32, CornerRadius, Frame, Margin, Rect, Sense, Stroke, Ui};

use stoll_kalip_editoru::io::save_file;
use stoll_kalip_editoru::math::{
    convert_input_format, mm_to_rows, mm_to_stitches, recalc_document_for_density_change,
    rows_to_mm, stitches_to_mm, sync_mirrored_right_lines,
};
use stoll_kalip_editoru::model::{DisplayUnit, FormDocument, FormHeader, InputFormat, LineSide, UndoStack};

use crate::ui::icons::{paint_icon, IconKind};
use crate::ui::theme::{
    bold_font, colors, regular_font, win32_checkbox, win32_group_box, win32_input_frame,
};
use crate::ui::{
    centered_modal_pos, modal_shadow, show_modal_backdrop, FormatConvertRequest, UiState,
};

/// Modal dialog state handling:
/// - `Başka formatta kaydet` / `Yeniden hesapla ve adıyla kayıt et...` (`50505`)
/// - `İlmekler => Basamaklar` (`50521`)
/// - `Basamaklar => İlmekler` (`50522`)
/// - `mm <=> İlmekler` conversion with live density/rounding preview.
#[derive(Debug, Clone)]
pub struct FormatConvertDialogState {
    active_req: Option<FormatConvertRequest>,
    pub target_format: InputFormat,
    pub display_unit: DisplayUnit,
    pub density_base_text: String,
    pub density_x_text: String,
    pub density_y_text: String,
    pub save_as_after: bool,
}

impl Default for FormatConvertDialogState {
    fn default() -> Self {
        Self {
            active_req: None,
            target_format: InputFormat::Steps,
            display_unit: DisplayUnit::Mm,
            density_base_text: "100.00".to_string(),
            density_x_text: "30.00".to_string(),
            density_y_text: "30.00".to_string(),
            save_as_after: false,
        }
    }
}

impl FormatConvertDialogState {
    /// Render the Format Conversion & Recalculate modal when `state.format_convert_req` is `Some`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
    ) {
        let Some(req) = state.format_convert_req else {
            self.active_req = None;
            return;
        };

        if self.active_req != Some(req) {
            self.active_req = Some(req);
            self.target_format = req.target_format;
            self.display_unit = doc.header.display_unit;
            self.density_base_text = format!("{:.2}", doc.header.density_base_mm);
            self.density_x_text = format!("{:.2}", doc.header.density_x);
            self.density_y_text = format!("{:.2}", doc.header.density_y);
            self.save_as_after = req.save_as_after;
        }

        let title = if self.save_as_after {
            "Yeniden hesapla ve adıyla kayıt et"
        } else {
            "Çevrim ve giriş formatı"
        };

        let preview_header = self.build_preview_header(&doc.header);

        let mut close =
            !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut commit = false;

        show_modal_backdrop(ctx, "format_convert_backdrop", false);
        let def_pos = centered_modal_pos(ctx, vec2(520.0, 420.0));

        egui::Window::new(title)
            .id(egui::Id::new("format_convert_modal"))
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
                ui.set_min_width(516.0);
                if paint_win32_fc_titlebar(ui, title) {
                    close = true;
                }
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.style_mut().visuals.widgets.inactive.bg_fill = colors::INPUT_BG;
                        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = colors::INPUT_BG;
                        ui.style_mut().visuals.widgets.inactive.bg_stroke =
                            Stroke::new(1.0_f32, colors::BORDER_MID);

                        // Format selection
                        win32_group_box(ui, "Giriş formatı çevrimi", |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Mevcut format:").font(bold_font(11.0)),
                                );
                                ui.label(doc.header.input_format.label_tr());
                                ui.add_space(16.0);
                                ui.label(egui::RichText::new("=>").font(bold_font(12.0)));
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new("Hedef format:").font(bold_font(11.0)),
                                );
                                egui::ComboBox::from_id_salt("fc_target_fmt")
                                    .width(150.0)
                                    .selected_text(format_option_label(self.target_format))
                                    .show_ui(ui, |ui| {
                                        for fmt in [
                                            InputFormat::Steps,
                                            InputFormat::Stitches,
                                            InputFormat::Mm,
                                        ] {
                                            ui.selectable_value(
                                                &mut self.target_format,
                                                fmt,
                                                format_option_label(fmt),
                                            );
                                        }
                                    });
                            });

                            ui.add_space(4.0);
                            // Quick one-click Stoll conversion presets (`50521`, `50522`, `mm <=> İlmekler`)
                            ui.horizontal(|ui| {
                                let mk_btn = |lbl: &str| {
                                    egui::Button::new(lbl)
                                        .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                        .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                                        .corner_radius(CornerRadius::ZERO)
                                };
                                if ui.add(mk_btn("İlmekler => Basamaklar")).clicked() {
                                    self.target_format = InputFormat::Steps;
                                }
                                if ui.add(mk_btn("Basamaklar => İlmekler")).clicked() {
                                    self.target_format = InputFormat::Stitches;
                                }
                                if ui.add(mk_btn("İlmekler => mm (.shv)")).clicked() {
                                    self.target_format = InputFormat::Mm;
                                }
                                if ui.add(mk_btn("mm => İlmekler (.shp)")).clicked() {
                                    self.target_format = InputFormat::Steps;
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // Density / Gauge recalculation settings
                        win32_group_box(ui, "İlmek yoğunluğu ve birim", |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Referans (mm):");
                                win32_input_frame(true).show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.density_base_text)
                                            .desired_width(58.0)
                                            .frame(false),
                                    );
                                });
                                ui.add_space(8.0);
                                let (irx, _) =
                                    ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                paint_icon(ui.painter(), irx, IconKind::HorizontalArrow, true);
                                ui.label("En:");
                                win32_input_frame(true).show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.density_x_text)
                                            .desired_width(58.0)
                                            .frame(false),
                                    );
                                });
                                ui.add_space(8.0);
                                let (iry, _) =
                                    ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                paint_icon(ui.painter(), iry, IconKind::VerticalArrow, true);
                                ui.label("Boy:");
                                win32_input_frame(true).show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.density_y_text)
                                            .desired_width(58.0)
                                            .frame(false),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label("Görüntü:");
                                egui::ComboBox::from_id_salt("fc_disp_unit")
                                    .width(62.0)
                                    .selected_text(self.display_unit.label_tr())
                                    .show_ui(ui, |ui| {
                                        for u in
                                            [DisplayUnit::Mm, DisplayUnit::Cm, DisplayUnit::Inch]
                                        {
                                            ui.selectable_value(
                                                &mut self.display_unit,
                                                u,
                                                u.label_tr(),
                                            );
                                        }
                                    });
                            });

                            ui.add_space(3.0);
                            win32_checkbox(
                                ui,
                                &mut self.save_as_after,
                                "Çevrim sonrası yeni dosya adıyla kaydet (.shp / .shv)",
                            );
                        });

                        ui.add_space(4.0);

                        // Live rounding & recalculation preview table
                        let elem_idx = state.selected_elem.min(doc.elements.len().saturating_sub(1));
                        let side = if doc
                            .elements
                            .get(elem_idx)
                            .map(|e| e.mirrored)
                            .unwrap_or(true)
                        {
                            LineSide::Left
                        } else {
                            state.table_mode.side()
                        };

                        win32_group_box(ui, "Canlı yuvarlama ve ölçü önizlemesi", |ui| {
                            Frame::NONE
                                .fill(colors::INPUT_BG)
                                .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                                .inner_margin(Margin::same(3))
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt("fc_preview_scroll")
                                        .max_height(150.0)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            egui::Grid::new("fc_preview_grid")
                                                .num_columns(7)
                                                .striped(true)
                                                .spacing(vec2(10.0, 2.0))
                                                .show(ui, |ui| {
                                                    for h in [
                                                        "No.",
                                                        "Mevcut (R, N)",
                                                        "Mevcut (mm)",
                                                        "Yeni (R, N)",
                                                        "Yeni (mm)",
                                                        "Yuvarlama Fark",
                                                        "Durum",
                                                    ] {
                                                        ui.label(
                                                            egui::RichText::new(h)
                                                                .font(bold_font(10.5)),
                                                        );
                                                    }
                                                    ui.end_row();

                                                    if let Some(el) = doc.elements.get(elem_idx) {
                                                        for line in el.lines(side) {
                                                            let (
                                                                new_st,
                                                                new_rows,
                                                                new_w_mm,
                                                                new_h_mm,
                                                            ) = preview_line_conversion(
                                                                line,
                                                                doc.header.input_format,
                                                                self.target_format,
                                                                &preview_header,
                                                            );
                                                            let dx_diff = new_st - line.stitches;
                                                            let dy_diff = new_rows - line.rows;
                                                            let changed = dx_diff != 0
                                                                || dy_diff != 0
                                                                || (new_w_mm - line.width_mm).abs()
                                                                    > 0.05
                                                                || (new_h_mm - line.height_mm)
                                                                    .abs()
                                                                    > 0.05;

                                                            ui.label(format!("{}", line.index));
                                                            ui.label(format!(
                                                                "{}, {}",
                                                                line.rows, line.stitches
                                                            ));
                                                            ui.label(format!(
                                                                "{:.1} × {:.1}",
                                                                line.height_mm, line.width_mm
                                                            ));
                                                            ui.label(format!("{new_rows}, {new_st}"));
                                                            ui.label(format!(
                                                                "{new_h_mm:.1} × {new_w_mm:.1}"
                                                            ));
                                                            let diff_col =
                                                                if dx_diff != 0 || dy_diff != 0 {
                                                                    colors::STATUS_ERR_RED
                                                                } else {
                                                                    colors::STATUS_OK_GREEN
                                                                };
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "{dy_diff:+}, {dx_diff:+}"
                                                                ))
                                                                .color(diff_col),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(if changed {
                                                                    "Güncellenir"
                                                                } else {
                                                                    "Tam eşleşme"
                                                                })
                                                                .font(regular_font(10.5))
                                                                .color(if changed {
                                                                    colors::WIN_BLUE
                                                                } else {
                                                                    colors::STATUS_OK_GREEN
                                                                }),
                                                            );
                                                            ui.end_row();
                                                        }
                                                    }
                                                });
                                        });
                                });
                        });

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cancel_btn = egui::Button::new("İptal")
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(78.0, 23.0), cancel_btn).clicked() {
                                close = true;
                            }
                            let ok_lbl = if self.save_as_after {
                                "Çevir ve Kaydet..."
                            } else {
                                "Tamam"
                            };
                            let ok_btn = egui::Button::new(ok_lbl)
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.5_f32, colors::WIN_BLUE))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(124.0, 23.0), ok_btn).clicked() {
                                commit = true;
                                close = true;
                            }
                        });
                    });
            });

        if commit {
            self.apply_conversion(doc, undo, state, &preview_header);
        }

        if close {
            state.format_convert_req = None;
            self.active_req = None;
        }
    }

    fn build_preview_header(&self, orig: &FormHeader) -> FormHeader {
        let mut h = orig.clone();
        h.display_unit = self.display_unit;
        if let Ok(v) = self.density_base_text.trim().replace(',', ".").parse::<f64>() {
            if v.is_finite() && (1.0..=1000.0).contains(&v) {
                h.density_base_mm = v;
            }
        }
        if let Ok(v) = self.density_x_text.trim().replace(',', ".").parse::<f64>() {
            if v.is_finite() && (1.0..=500.0).contains(&v) {
                h.density_x = v;
            }
        }
        if let Ok(v) = self.density_y_text.trim().replace(',', ".").parse::<f64>() {
            if v.is_finite() && (1.0..=500.0).contains(&v) {
                h.density_y = v;
            }
        }
        h
    }

    fn apply_conversion(
        &self,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
        new_hdr: &FormHeader,
    ) {
        undo.push("Format çevrimi", doc);

        let density_changed = (doc.header.density_base_mm - new_hdr.density_base_mm).abs() > 1e-6
            || (doc.header.density_x - new_hdr.density_x).abs() > 1e-6
            || (doc.header.density_y - new_hdr.density_y).abs() > 1e-6;

        doc.header.density_base_mm = new_hdr.density_base_mm;
        doc.header.density_x = new_hdr.density_x;
        doc.header.density_y = new_hdr.density_y;
        doc.header.display_unit = new_hdr.display_unit;

        if density_changed {
            recalc_document_for_density_change(doc);
        }

        convert_input_format(doc, self.target_format);

        for ei in 0..doc.elements.len() {
            sync_mirrored_right_lines(doc, ei);
        }

        if self.save_as_after {
            let is_mm = self.target_format == InputFormat::Mm;
            let default_ext = if is_mm { "shv" } else { "shp" };
            let base_name = if doc.header.name.trim().is_empty() {
                "Yeni"
            } else {
                doc.header.name.trim()
            };
            let default_name = format!("{base_name}.{default_ext}");
            let dialog = rfd::FileDialog::new()
                .set_file_name(&default_name)
                .add_filter(
                    if is_mm {
                        "Stoll mm Kalıbı (*.shv)"
                    } else {
                        "Stoll İlmek Kalıbı (*.shp)"
                    },
                    &[default_ext],
                )
                .add_filter("Tüm Stoll Kalıpları (*.shp, *.shv)", &["shp", "shv"]);

            if let Some(path) = dialog.save_file() {
                match save_file(doc, &path) {
                    Ok(()) => {
                        doc.file_path = Some(path.clone());
                        doc.dirty = false;
                        state.recent_files.retain(|p| p != &path);
                        state.recent_files.insert(0, path.clone());
                        state.recent_files.truncate(8);
                        if let Some(appdata) = std::env::var_os("APPDATA") {
                            let mru_p = std::path::PathBuf::from(appdata)
                                .join("kalipeditor")
                                .join("mru.txt");
                            if let Some(parent) = mru_p.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let body: String = state
                                .recent_files
                                .iter()
                                .map(|p| p.to_string_lossy())
                                .collect::<Vec<_>>()
                                .join("\n");
                            let _ = std::fs::write(mru_p, body);
                        }
                        state.set_status(
                            format!(
                                "Dönüştürüldü ({}) ve kaydedildi: {}",
                                self.target_format.label_tr(),
                                path.display()
                            ),
                            false,
                        );
                    }
                    Err(e) => {
                        state.set_status(format!("Kaydetme hatası: {e}"), true);
                    }
                }
            } else {
                state.set_status(
                    format!(
                        "Giriş formatı dönüştürüldü: {} (kayıt iptal edildi)",
                        self.target_format.label_tr()
                    ),
                    false,
                );
            }
        } else {
            state.set_status(
                format!(
                    "Giriş formatı dönüştürüldü: {}",
                    self.target_format.label_tr()
                ),
                false,
            );
        }
    }
}

fn format_option_label(fmt: InputFormat) -> &'static str {
    match fmt {
        InputFormat::Steps => "Basamaklar (.shp)",
        InputFormat::Stufung => "Basamaklar / Stufung (.shp)",
        InputFormat::Stitches => "İlmekler (.shp)",
        InputFormat::Mm => "mm / Vektör (.shv)",
    }
}

fn preview_line_conversion(
    line: &stoll_kalip_editoru::model::LineSegment,
    old_fmt: InputFormat,
    new_fmt: InputFormat,
    hdr: &FormHeader,
) -> (i32, i32, f64, f64) {
    match (old_fmt, new_fmt) {
        (InputFormat::Mm, InputFormat::Mm) => {
            let st = mm_to_stitches(line.width_mm, hdr);
            let rw = mm_to_rows(line.height_mm, hdr);
            (st, rw, line.width_mm, line.height_mm)
        }
        (InputFormat::Mm, InputFormat::Steps | InputFormat::Stufung | InputFormat::Stitches) => {
            let st = mm_to_stitches(line.width_mm, hdr);
            let rw = mm_to_rows(line.height_mm, hdr);
            (st, rw, stitches_to_mm(st, hdr), rows_to_mm(rw, hdr))
        }
        (InputFormat::Steps | InputFormat::Stufung | InputFormat::Stitches, InputFormat::Mm) => {
            let wm = stitches_to_mm(line.stitches, hdr);
            let hm = rows_to_mm(line.rows, hdr);
            (line.stitches, line.rows, wm, hm)
        }
        (InputFormat::Steps | InputFormat::Stufung, InputFormat::Stitches) => {
            let (st, rw) = if line.is_curve() {
                (line.stitches, line.rows)
            } else {
                (
                    line.step_x * line.step_factor.max(1) + line.rest_x,
                    line.step_y * line.step_factor.max(1) + line.rest_y,
                )
            };
            (st, rw, stitches_to_mm(st, hdr), rows_to_mm(rw, hdr))
        }
        _ => {
            let wm = stitches_to_mm(line.stitches, hdr);
            let hm = rows_to_mm(line.rows, hdr);
            (line.stitches, line.rows, wm, hm)
        }
    }
}

fn paint_win32_fc_titlebar(ui: &mut Ui, title: &str) -> bool {
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
    let close_resp = ui.interact(close_rect, ui.id().with("fc_close_x"), Sense::click());
    let hov = close_resp.hovered();
    if hov {
        painter.rect_filled(close_rect, CornerRadius::ZERO, Color32::from_rgb(0xE8, 0x11, 0x23));
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
