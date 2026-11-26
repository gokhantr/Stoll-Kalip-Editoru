use egui::{pos2, vec2, Align2, Color32, CornerRadius, Frame, Margin, Rect, Sense, Stroke, Ui};

use stoll_kalip_editoru::math::{
    mm_to_rows, mm_to_stitches, rows_to_mm, stitches_to_mm, sync_mirrored_right_lines,
};
use stoll_kalip_editoru::model::{FormDocument, InputFormat, LineSide, PointEntry, PointOffset, UndoStack};

use crate::ui::icons::{paint_icon, IconKind};
use crate::ui::theme::{
    bold_font, colors, regular_font, win32_checkbox, win32_group_box, win32_input_frame,
    win32_numeric_text_edit,
};
use crate::ui::{marker_function_label_tr, PointMultiplyRequest, UiState};

/// Direction option when multiplying marking points (`DIALOG 4277`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiplyDirection {
    /// Positive stepping: `base + i * (dx, dy)`
    Forward,
    /// Negative stepping: `base - i * (dx, dy)`
    Backward,
    /// Symmetric around source: `base ± k * (dx, dy)`
    Symmetric,
}

impl MultiplyDirection {
    fn label_tr(self) -> &'static str {
        match self {
            Self::Forward => "+ İleri (Artan)",
            Self::Backward => "- Geri (Azalan)",
            Self::Symmetric => "± Simetrik",
        }
    }
}

/// Modal dialog state for `İşaretleme noktalarını çoğalt` (`DIALOG 4277`).
#[derive(Debug, Clone)]
pub struct PointMultiplyDialogState {
    active_req: Option<PointMultiplyRequest>,
    /// Total count of markers to generate (`2..=99`).
    pub count: usize,
    /// Horizontal spacing (stitches in `.shp`, mm in `.shv`).
    pub delta_x_text: String,
    /// Vertical spacing (rows in `.shp`, mm in `.shv`).
    pub delta_y_text: String,
    /// Stepping direction.
    pub direction: MultiplyDirection,
    /// Keep original source marker in place (`true`) vs replace it.
    pub keep_source: bool,
    /// Assign a common `gruppe_tag_id` to all generated markers.
    pub group_generated: bool,
}

impl Default for PointMultiplyDialogState {
    fn default() -> Self {
        Self {
            active_req: None,
            count: 3,
            delta_x_text: "0".to_string(),
            delta_y_text: "10".to_string(),
            direction: MultiplyDirection::Forward,
            keep_source: true,
            group_generated: false,
        }
    }
}

impl PointMultiplyDialogState {
    /// Render `İşaretleme noktalarını çoğalt` (`DIALOG 4277`) when `state.point_multiply_req` is `Some`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
    ) {
        let Some(req) = state.point_multiply_req else {
            self.active_req = None;
            return;
        };

        if self.active_req != Some(req) {
            self.active_req = Some(req);
            self.count = 3;
            if doc.header.input_format == InputFormat::Mm {
                self.delta_x_text = "0.00".to_string();
                self.delta_y_text = "30.00".to_string();
            } else {
                self.delta_x_text = "0".to_string();
                self.delta_y_text = "10".to_string();
            }
            self.direction = MultiplyDirection::Forward;
            self.keep_source = true;
            self.group_generated = false;
        }

        let use_mm = doc.header.input_format == InputFormat::Mm;
        let unit_x = if use_mm { "mm" } else { "ilmek" };
        let unit_y = if use_mm { "mm" } else { "sıra" };

        // Read source marker snapshot
        let source_entry = doc
            .elements
            .get(req.elem_idx)
            .and_then(|el| match req.side {
                LineSide::Left => el.left_points.get(req.marker_idx),
                LineSide::Right => el.right_points.get(req.marker_idx),
            })
            .cloned();

        let (base_x_st, base_y_st) = source_entry
            .as_ref()
            .and_then(|p| p.offsets.first())
            .map(|o| (o.x, o.y))
            .unwrap_or((0, 0));

        let side_label = match req.side {
            LineSide::Left => "Sol işaretler",
            LineSide::Right => "Sağ işaretler",
        };
        let elem_name = doc
            .elements
            .get(req.elem_idx)
            .map(|e| e.name.as_str())
            .unwrap_or("Eleman");

        // Parse spacing in stitches/rows
        let (dx_st, dy_st) = self.parse_deltas_in_stitches(doc, use_mm);
        let preview_coords = self.compute_preview_coords(base_x_st, base_y_st, dx_st, dy_st);

        let mut close =
            !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut commit = false;

        egui::Window::new("İşaretleme noktalarını çoğalt")
            .id(egui::Id::new("point_multiply_modal"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .frame(
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                if paint_win32_modal_titlebar(ui, "İşaretleme noktalarını çoğalt", "pm_close_x") {
                    close = true;
                }
                Frame::NONE
                    .fill(colors::CHROME_BG)
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        // White Win32 ComboBox styling inside modal (`DLG-13`)
                        ui.style_mut().visuals.widgets.inactive.bg_fill = colors::INPUT_BG;
                        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = colors::INPUT_BG;
                        ui.style_mut().visuals.widgets.inactive.bg_stroke =
                            Stroke::new(1.0_f32, colors::BORDER_MID);

                        // Source info box
                        win32_group_box(ui, "Kaynak işaret bilgisi", |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Eleman:").font(bold_font(11.0)));
                                ui.label(format!("{} ({})", elem_name, side_label));
                            });
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Kaynak No.:").font(bold_font(11.0)));
                                let func_lbl = source_entry
                                    .as_ref()
                                    .map(|p| marker_function_label_tr(p.punkt_art_tag_id))
                                    .unwrap_or("Normal");
                                ui.label(format!(
                                    "{}  |  X: {} ({:.1} mm)  Y: {} ({:.1} mm)  [{}]",
                                    req.marker_idx + 1,
                                    base_x_st,
                                    stitches_to_mm(base_x_st, &doc.header),
                                    base_y_st,
                                    rows_to_mm(base_y_st, &doc.header),
                                    func_lbl,
                                ));
                            });
                        });

                        ui.add_space(4.0);

                        // Parameters box (`DIALOG 4277`: Mesafe ↔, Mesafe ↕, Toplam sayı)
                        win32_group_box(ui, "Çoğaltma parametreleri", |ui| {
                            egui::Grid::new("point_multiply_params_grid")
                                .num_columns(3)
                                .spacing(vec2(8.0, 5.0))
                                .show(ui, |ui| {
                                    // Toplam sayı / Adet (`15744`)
                                    ui.label("Toplam sayı (Adet):");
                                    egui::ComboBox::from_id_salt("pm_count_combo")
                                        .width(84.0)
                                        .selected_text(format!("{}", self.count))
                                        .show_ui(ui, |ui| {
                                            for n in [2, 3, 4, 5, 6, 8, 10, 12, 15, 20, 25, 30, 50] {
                                                ui.selectable_value(&mut self.count, n, format!("{n}"));
                                            }
                                        });
                                    // White sunken numeric edit box for custom count `2..=99` (commits on Enter / focus-loss so 2-digit numbers type cleanly!)
                                    if let Some(txt) = win32_numeric_text_edit(
                                        ui,
                                        "pm_count_custom",
                                        self.count.to_string(),
                                        48.0,
                                        true,
                                    ) {
                                        if let Ok(v) = txt.trim().parse::<usize>() {
                                            self.count = v.clamp(2, 99);
                                        }
                                    }
                                    ui.end_row();

                                    // Mesafe Horizontal (`15742`)
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        let (ir, _) =
                                            ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                        paint_icon(ui.painter(), ir, IconKind::HorizontalArrow, true);
                                        ui.label(format!("Mesafe ({unit_x}):"));
                                    });
                                    win32_input_frame(true).show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.delta_x_text)
                                                .desired_width(78.0)
                                                .frame(false),
                                        );
                                    });
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "~ {} ilmek ({:.1} mm)",
                                            dx_st,
                                            stitches_to_mm(dx_st, &doc.header)
                                        ))
                                        .font(regular_font(10.5))
                                        .color(colors::TEXT_DISABLED),
                                    );
                                    ui.end_row();

                                    // Mesafe Vertical (`15743`)
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        let (ir, _) =
                                            ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                        paint_icon(ui.painter(), ir, IconKind::VerticalArrow, true);
                                        ui.label(format!("Mesafe ({unit_y}):"));
                                    });
                                    win32_input_frame(true).show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.delta_y_text)
                                                .desired_width(78.0)
                                                .frame(false),
                                        );
                                    });
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "~ {} sıra ({:.1} mm)",
                                            dy_st,
                                            rows_to_mm(dy_st, &doc.header)
                                        ))
                                        .font(regular_font(10.5))
                                        .color(colors::TEXT_DISABLED),
                                    );
                                    ui.end_row();

                                    // Yön
                                    ui.label("Yön / Dağılım:");
                                    egui::ComboBox::from_id_salt("pm_dir_combo")
                                        .width(136.0)
                                        .selected_text(self.direction.label_tr())
                                        .show_ui(ui, |ui| {
                                            for d in [
                                                MultiplyDirection::Forward,
                                                MultiplyDirection::Backward,
                                                MultiplyDirection::Symmetric,
                                            ] {
                                                ui.selectable_value(&mut self.direction, d, d.label_tr());
                                            }
                                        });
                                    ui.label("");
                                    ui.end_row();
                                });

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                win32_checkbox(ui, &mut self.keep_source, "Kaynak noktayı koru");
                                ui.add_space(12.0);
                                win32_checkbox(ui, &mut self.group_generated, "Aynı gruba ata");
                            });
                        });

                        ui.add_space(4.0);

                        // Live Preview List of generated [Punkt] coordinates
                        win32_group_box(
                            ui,
                            &format!("Önizleme ({} nokta oluşturulacak)", preview_coords.len()),
                            |ui| {
                                Frame::NONE
                                    .fill(colors::INPUT_BG)
                                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                                    .inner_margin(Margin::same(3))
                                    .show(ui, |ui| {
                                        egui::ScrollArea::vertical()
                                            .id_salt("point_multiply_preview_scroll")
                                            .max_height(135.0)
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                egui::Grid::new("pm_preview_tbl")
                                                    .num_columns(5)
                                                    .striped(true)
                                                    .spacing(vec2(12.0, 2.0))
                                                    .show(ui, |ui| {
                                                        for hdr in [
                                                            "No.",
                                                            "En (ilmek)",
                                                            "Boy (sıra)",
                                                            "En (mm)",
                                                            "Boy (mm)",
                                                        ] {
                                                            ui.label(
                                                                egui::RichText::new(hdr)
                                                                    .font(bold_font(10.5)),
                                                            );
                                                        }
                                                        ui.end_row();

                                                        for (i, &(px, py)) in
                                                            preview_coords.iter().enumerate()
                                                        {
                                                            let is_orig = self.keep_source
                                                                && match self.direction {
                                                                    MultiplyDirection::Symmetric => {
                                                                        i == preview_coords.len() / 2
                                                                    }
                                                                    _ => i == 0,
                                                                };
                                                            let col = if is_orig {
                                                                colors::WIN_BLUE
                                                            } else {
                                                                colors::TEXT_BLACK
                                                            };
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "{}",
                                                                    i + 1
                                                                ))
                                                                .color(col),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!("{px}"))
                                                                    .color(col),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!("{py}"))
                                                                    .color(col),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "{:.2}",
                                                                    stitches_to_mm(px, &doc.header)
                                                                ))
                                                                .color(col),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "{:.2}",
                                                                    rows_to_mm(py, &doc.header)
                                                                ))
                                                                .color(col),
                                                            );
                                                            ui.end_row();
                                                        }
                                                    });
                                            });
                                    });
                            },
                        );

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Footer buttons (`Tamam`, `İptal et`)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cancel_btn = egui::Button::new("İptal et")
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(78.0, 23.0), cancel_btn).clicked() {
                                close = true;
                            }
                            let ok_btn = egui::Button::new("Tamam")
                                .fill(Color32::from_rgb(0xE1, 0xE1, 0xE1))
                                .stroke(Stroke::new(1.5_f32, colors::WIN_BLUE))
                                .corner_radius(CornerRadius::ZERO);
                            if ui.add_sized(vec2(78.0, 23.0), ok_btn).clicked() {
                                commit = true;
                                close = true;
                            }
                        });
                    });
            });

        if commit {
            self.apply_multiply(doc, undo, state, req, &preview_coords, source_entry);
        }

        if close {
            state.point_multiply_req = None;
            self.active_req = None;
        }
    }

    fn parse_deltas_in_stitches(&self, doc: &FormDocument, use_mm: bool) -> (i32, i32) {
        let px = self
            .delta_x_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);
        let py = self
            .delta_y_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);

        if use_mm {
            (
                mm_to_stitches(px, &doc.header),
                mm_to_rows(py, &doc.header),
            )
        } else {
            (px.round() as i32, py.round() as i32)
        }
    }

    fn compute_preview_coords(
        &self,
        base_x: i32,
        base_y: i32,
        dx: i32,
        dy: i32,
    ) -> Vec<(i32, i32)> {
        let n = self.count.clamp(1, 99);
        let mut out = Vec::with_capacity(n);
        match self.direction {
            MultiplyDirection::Forward => {
                for i in 0..n {
                    let step = if self.keep_source { i as i32 } else { (i + 1) as i32 };
                    out.push((base_x + dx * step, base_y + dy * step));
                }
            }
            MultiplyDirection::Backward => {
                for i in 0..n {
                    let step = if self.keep_source { i as i32 } else { (i + 1) as i32 };
                    out.push((base_x - dx * step, base_y - dy * step));
                }
            }
            MultiplyDirection::Symmetric => {
                let half = (n as i32) / 2;
                for i in 0..n {
                    let offset_k = (i as i32) - half;
                    out.push((base_x + dx * offset_k, base_y + dy * offset_k));
                }
            }
        }
        out
    }

    fn apply_multiply(
        &self,
        doc: &mut FormDocument,
        undo: &mut UndoStack,
        state: &mut UiState,
        req: PointMultiplyRequest,
        coords: &[(i32, i32)],
        template: Option<PointEntry>,
    ) {
        if coords.is_empty() {
            return;
        }
        undo.push("İşaretleme noktalarını çoğalt", doc);

        let Some(el) = doc.elements.get_mut(req.elem_idx) else {
            return;
        };

        let pts = match req.side {
            LineSide::Left => {
                el.has_left_points_section = true;
                &mut el.left_points
            }
            LineSide::Right => {
                el.has_right_points_section = true;
                &mut el.right_points
            }
        };

        let next_group_id = if self.group_generated {
            pts.iter().map(|p| p.gruppe_tag_id).max().unwrap_or(0) + 1
        } else {
            0
        };

        let base_template = template.unwrap_or_default();
        let insert_at = if req.marker_idx < pts.len() {
            if self.keep_source && self.direction != MultiplyDirection::Symmetric {
                // Update source group if requested, insert generated copies right after
                if self.group_generated {
                    pts[req.marker_idx].gruppe_tag_id = next_group_id;
                }
                req.marker_idx + 1
            } else {
                pts.remove(req.marker_idx);
                req.marker_idx
            }
        } else {
            pts.len()
        };

        let skip_first = self.keep_source
            && req.marker_idx < pts.len()
            && self.direction != MultiplyDirection::Symmetric;
        let slice = if skip_first && coords.len() > 1 {
            &coords[1..]
        } else {
            coords
        };

        let mut new_entries = Vec::with_capacity(slice.len());
        for &(x, y) in slice {
            let mut entry = base_template.clone();
            entry.punkt_tag_id = doc.next_point_tag_id();
            if self.group_generated {
                entry.gruppe_tag_id = next_group_id;
            }
            entry.offsets = vec![PointOffset {
                index: 0,
                x,
                y,
                raw: Default::default(),
            }];
            entry.number_of_marks = Some(1);
            entry.raw.original_header = None;
            new_entries.push(entry);
        }

        let Some(el) = doc.elements.get_mut(req.elem_idx) else {
            return;
        };
        let pts = match req.side {
            LineSide::Left => &mut el.left_points,
            LineSide::Right => &mut el.right_points,
        };
        let inserted_count = new_entries.len();
        for (i, entry) in new_entries.into_iter().enumerate() {
            pts.insert((insert_at + i).min(pts.len()), entry);
        }

        // Renumber `[Punkt] = 1..=P`
        for (idx, p) in pts.iter_mut().enumerate() {
            p.index = (idx + 1) as i32;
            p.raw.original_header = Some(format!("[Punkt] = {}", p.index));
        }

        let sel_start = insert_at.min(pts.len().saturating_sub(1));
        let sel_end = (insert_at + inserted_count).min(pts.len());
        if sel_start < sel_end {
            state.selected_rows = (sel_start..sel_end).collect();
            state.row_select_anchor = Some(sel_start);
            state.focused_cell = Some((sel_start, 1));
        } else if !pts.is_empty() {
            let r = req.marker_idx.min(pts.len() - 1);
            state.select_single_row(r);
            state.focused_cell = Some((r, 1));
        } else {
            state.selected_rows.clear();
            state.focused_cell = None;
        }

        // Mirror points to right side if element is mirrored and edited on left
        if el.mirrored && req.side == LineSide::Left {
            sync_mirrored_right_lines(doc, req.elem_idx);
        }

        doc.dirty = true;
        state.set_status(
            format!("{} işaretleme noktası oluşturuldu.", coords.len()),
            false,
        );
    }
}

fn paint_win32_modal_titlebar(ui: &mut Ui, title: &str, close_id: &str) -> bool {
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
