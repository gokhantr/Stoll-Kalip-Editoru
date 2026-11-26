use egui::{
    vec2, Color32, CornerRadius, Frame, Margin, Rect, Sense, Stroke, Ui,
};

use stoll_kalip_editoru::model::{FormDocument, InputFormat, LineSide};

use crate::ui::icons::{paint_icon, paint_studio_glyph, IconKind, StudioGlyph};
use crate::ui::theme::{bold_font, colors, regular_font, win32_numeric_text_edit};
use crate::ui::{AppAction, SignMode, TableMode, UiState};

/// Render the top Menu Bar (`Dosya`, `Görünüm`, `Çizgiler-Giriş`, `Tablolar`, `?`)
/// matching `MENU 10800` in `bin\tr\vecformeditor.exe` 1:1.
pub fn show_menu_bar(
    ui: &mut Ui,
    doc: &mut FormDocument,
    can_undo: bool,
    can_redo: bool,
    state: &mut UiState,
    actions: &mut Vec<AppAction>,
) {
    let elem = doc.elements.get(state.selected_elem);
    let mirrored = elem.map(|e| e.mirrored).unwrap_or(true);

    Frame::NONE
        .fill(colors::MENU_BG)
        .inner_margin(Margin {
            left: 2,
            right: 4,
            top: 2,
            bottom: 2,
        })
        .show(ui, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // ── Dosya ───────────────────────────────────────────────────────
                ui.menu_button("Dosya", |ui| {
                    ui.set_min_width(240.0);
                    if menu_item(ui, "Yeni", "Ctrl+N", true) {
                        actions.push(AppAction::NewFile);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Aç  .shv Kalıbı [mm] ...", "Ctrl+O", true) {
                        actions.push(AppAction::OpenShv);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Aç  .shp Kalıbı  [İlmek] ...", "", true) {
                        actions.push(AppAction::OpenShp);
                        ui.close_menu();
                    }
                    ui.separator();

                    ui.menu_button("Veri al ...", |ui| {
                        if ui.button(".dxf-kesimi ...").clicked() {
                            actions.push(AppAction::ImportDxf);
                            ui.close_menu();
                        }
                        if ui.button("Harici tablo (.csv / .tsv) ...").clicked() {
                            actions.push(AppAction::ImportExternalTable);
                            ui.close_menu();
                        }
                        let _ = menu_item(ui, ".shpx-kesimi ...", "", false);
                        let _ = menu_item(ui, ".GKS ölçülendirme kalıbı ...", "", false);
                    });

                    ui.menu_button("Veri aktar ...", |ui| {
                        if ui.button(".shp/.shv M1-Formatında").clicked() {
                            actions.push(AppAction::SaveAs);
                            ui.close_menu();
                        }
                        if ui.button("Çizgi tablosu (.csv) ...").clicked() {
                            actions.push(AppAction::ExportCsvTable);
                            ui.close_menu();
                        }
                        let _ = menu_item(ui, ".shpx-kesimi ...", "", false);
                        let _ = menu_item(ui, ".ssf ...", "", false);
                    });
                    ui.separator();

                    ui.menu_button("Aç  standart özellikler ...", |ui| {
                        ui.menu_button("Stoll", |ui| {
                            for (label, kind) in [
                                ("Standart", "defaultform.shp"),
                                ("k&w beden", "defaultformk&wbody.shp"),
                                ("k&w kol", "defaultformk&wsleeve.shp"),
                                ("k&w yaka kesimi", "defaultformk&wneck.shp"),
                            ] {
                                if ui.button(label).clicked() {
                                    actions.push(AppAction::OpenPresetFile { kind });
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Kendinin / Özel", |ui| {
                            for (label, kind) in [
                                ("Standart", "defaultform_user.shp"),
                                ("k&w beden", "defaultformk&wbody_user.shp"),
                                ("k&w kol", "defaultformk&wsleeve_user.shp"),
                                ("k&w yaka kesimi", "defaultformk&wneck_user.shp"),
                            ] {
                                if ui.button(label).clicked() {
                                    actions.push(AppAction::OpenPresetFile { kind });
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    ui.separator();

                    ui.menu_button("çevrim ve giriş formatı ...", |ui| {
                        let is_stitches = doc.header.input_format == InputFormat::Stitches;
                        let is_steps = doc.header.input_format == InputFormat::Steps;
                        if ui
                            .add_enabled(is_stitches || doc.header.input_format == InputFormat::Mm, egui::Button::new("İlmekler => Basamaklar"))
                            .clicked()
                        {
                            actions.push(AppAction::ConvertFormat(InputFormat::Steps));
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(is_steps || doc.header.input_format == InputFormat::Mm, egui::Button::new("Basamaklar => İlmekler"))
                            .clicked()
                        {
                            actions.push(AppAction::ConvertFormat(InputFormat::Stitches));
                            ui.close_menu();
                        }
                    });
                    ui.separator();

                    if menu_item(ui, "Kaydet", "Ctrl+S", true) {
                        actions.push(AppAction::Save);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Adıyla kaydet...", "F12", true) {
                        actions.push(AppAction::SaveAs);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Yeniden hesapla ve adıyla kayıt et...", "", true) {
                        actions.push(AppAction::RecalcSaveAs);
                        ui.close_menu();
                    }
                    ui.separator();

                    if menu_item(ui, "DXF-Çizim aracı...", "", false) {
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Sayfa görünümü", "", true) {
                        actions.push(AppAction::ShowPrintPreview);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Yazdır...", "", true) {
                        actions.push(AppAction::PrintDocument);
                        ui.close_menu();
                    }
                    ui.separator();

                    if menu_item(ui, "Ayarları Dışa Aktar...", "", true) {
                        actions.push(AppAction::ExportSettingsBundle);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Ayarları İçe Aktar...", "", true) {
                        actions.push(AppAction::ImportSettingsBundle);
                        ui.close_menu();
                    }
                    ui.separator();

                    if state.recent_files.is_empty() {
                        menu_item(ui, "Son dosya", "", false);
                    } else {
                        let recent = state.recent_files.clone();
                        for (idx, path) in recent.iter().take(6).enumerate() {
                            let lbl = format!("{}  {}", idx + 1, path.display());
                            if ui.button(lbl).clicked() {
                                actions.push(AppAction::OpenPath(path.clone()));
                                ui.close_menu();
                            }
                        }
                    }
                    ui.separator();

                    if menu_item(ui, "Bitir", "Alt+F4", true) {
                        actions.push(AppAction::Exit);
                        ui.close_menu();
                    }
                });

                // ── Düzenle ─────────────────────────────────────────────────────
                ui.menu_button("Düzenle", |ui| {
                    ui.set_min_width(240.0);
                    let in_markers = state.table_mode.is_markers();
                    let has_copied = if in_markers {
                        !state.copied_markers.is_empty()
                    } else {
                        !state.copied_lines.is_empty()
                    };
                    let has_sel = !state.selected_rows.is_empty();
                    let can_del = if in_markers {
                        has_sel
                    } else {
                        doc.elements
                            .get(state.selected_elem)
                            .map(|el| {
                                let side = state.table_mode.side();
                                state
                                    .selected_rows
                                    .iter()
                                    .any(|&r| r < el.lines(side).len() && !el.is_base_line_row(side, r))
                            })
                            .unwrap_or(false)
                    };

                    if menu_item(ui, "Geri Al", "Ctrl+Z", can_undo) {
                        actions.push(AppAction::Undo);
                        ui.close_menu();
                    }
                    if menu_item(ui, "İleri Al", "Ctrl+Y", can_redo) {
                        actions.push(AppAction::Redo);
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Kopyala", "Ctrl+C", has_sel) {
                        actions.push(if in_markers {
                            AppAction::CopySelectedMarkers
                        } else {
                            AppAction::CopySelectedLines
                        });
                        ui.close_menu();
                    }
                    if menu_item(ui, "Ekle+Yapıştır", "Ctrl+Shift+V", has_copied) {
                        actions.push(if in_markers {
                            AppAction::PasteInsertMarkers
                        } else {
                            AppAction::PasteInsertLines
                        });
                        ui.close_menu();
                    }
                    if menu_item(ui, "Üzerine Yapıştır", "Ctrl+V", has_copied) {
                        actions.push(if in_markers {
                            AppAction::PasteReplaceMarkers
                        } else {
                            AppAction::PasteReplaceLines
                        });
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Seçili Satırı Sil", "Delete", can_del) {
                        actions.push(if in_markers {
                            AppAction::DeleteSelectedMarker
                        } else {
                            AppAction::DeleteSelectedLines
                        });
                        ui.close_menu();
                    }
                });

                // ── Görünüm ─────────────────────────────────────────────────────
                ui.menu_button("Görünüm", |ui| {
                    ui.set_min_width(230.0);
                    if ui
                        .checkbox(&mut state.always_on_top, "Her zaman ön planda")
                        .clicked()
                    {
                        actions.push(AppAction::ToggleAlwaysOnTop);
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(&mut state.show_graphics, "Grafiği göster")
                        .clicked()
                    {
                        actions.push(AppAction::ToggleGraphics);
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(
                            &mut state.hide_readonly_columns,
                            "Düzenlenemeyen sütunları gizle",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(
                            &mut state.preset_name_as_comment,
                            "Yorum olarak standart nitelikler",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut state.show_detail_columns, "Detay sütunları göster (14 sütun)");
                    ui.checkbox(&mut state.show_dimensions, "Kanvas ölçü etiketleri");
                    ui.checkbox(&mut state.snap_even_rows, "Çift sıra yakalama (2'şer)");
                    ui.separator();
                    if ui
                        .checkbox(&mut state.show_status_bar, "Statü çubuğu")
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    ui.menu_button("Sembol çubukları", |ui| {
                        ui.checkbox(&mut state.toolbars.standard, "Standart");
                        ui.checkbox(&mut state.toolbars.input, "Giriş");
                        ui.checkbox(&mut state.toolbars.elements, "Elemanlar");
                        ui.checkbox(&mut state.toolbars.lines, "Çizgiler");
                        ui.checkbox(&mut state.toolbars.markers, "İşaretler");
                    });
                });

                // ── Çizgiler-Giriş ──────────────────────────────────────────────
                ui.menu_button("Çizgiler-Giriş", |ui| {
                    ui.set_min_width(220.0);
                    if ui
                        .radio_value(
                            &mut state.sign_mode,
                            SignMode::Cartesian,
                            "+: Sağa doğru, -: Sola doğru",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut state.sign_mode,
                            SignMode::Knitting,
                            "+: Arttır, -: Reglan",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .checkbox(
                            &mut doc.header.hand_knitting_method,
                            "Kalıb örnekleri için tip A",
                        )
                        .changed()
                    {
                        doc.dirty = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Başlama", |ui| {
                        if ui
                            .radio_value(
                                &mut doc.header.form_start_type,
                                1,
                                "Önce arttır sonra ör",
                            )
                            .changed()
                        {
                            doc.dirty = true;
                            ui.close_menu();
                        }
                        if ui
                            .radio_value(
                                &mut doc.header.form_start_type,
                                2,
                                "Önce ör sonra arttır",
                            )
                            .changed()
                        {
                            doc.dirty = true;
                            ui.close_menu();
                        }
                    });
                });

                // ── Tablolar ────────────────────────────────────────────────────
                ui.menu_button("Tablolar", |ui| {
                    ui.set_min_width(200.0);
                    if menu_radio_item(
                        ui,
                        "Sol çizgiler",
                        "F7",
                        state.table_mode == TableMode::LeftLines,
                        true,
                    ) {
                        state.table_mode = TableMode::LeftLines;
                        state.active_table_side = LineSide::Left;
                        state.select_single_row(0);
                        ui.close_menu();
                    }
                    if menu_radio_item(
                        ui,
                        "Sağ çizgiler",
                        "F8",
                        state.table_mode == TableMode::RightLines,
                        !mirrored,
                    ) {
                        state.table_mode = TableMode::RightLines;
                        state.active_table_side = LineSide::Right;
                        state.select_single_row(0);
                        ui.close_menu();
                    }
                    if menu_radio_item(
                        ui,
                        "Sol işaretler",
                        "F9",
                        state.table_mode == TableMode::LeftMarkers,
                        true,
                    ) {
                        state.table_mode = TableMode::LeftMarkers;
                        state.active_table_side = LineSide::Left;
                        state.select_single_row(0);
                        ui.close_menu();
                    }
                    if menu_radio_item(
                        ui,
                        "Sağ işaretler",
                        "F10",
                        state.table_mode == TableMode::RightMarkers,
                        !mirrored,
                    ) {
                        state.table_mode = TableMode::RightMarkers;
                        state.active_table_side = LineSide::Right;
                        state.select_single_row(0);
                        ui.close_menu();
                    }
                    ui.separator();
                    if menu_item(ui, "Sol ile sağ tarafı değiştir", "", !mirrored) {
                        actions.push(AppAction::SwapLeftRight);
                        ui.close_menu();
                    }
                });

                // ── ? ───────────────────────────────────────────────────────────
                ui.menu_button("?", |ui| {
                    ui.set_min_width(210.0);
                    if menu_item(ui, "Yardım konuları", "F1", true) {
                        actions.push(AppAction::ShowHelp);
                        ui.close_menu();
                    }
                    if menu_item(ui, "Hakkında & Güncellemeler...", "", true) {
                        actions.push(AppAction::ShowAbout);
                        ui.close_menu();
                    }
                });
            });
        });

    // Thin divider below menu bar
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right(), rect.top()),
        ],
        Stroke::new(1.0_f32, colors::BORDER_LIGHT),
    );
}

/// Render the streamlined Modern Light Studio single-row toolbar.
pub fn show_toolbar(
    ui: &mut Ui,
    doc: &mut FormDocument,
    can_undo: bool,
    can_redo: bool,
    state: &mut UiState,
    available_update_tag: Option<&str>,
    actions: &mut Vec<AppAction>,
) {
    let elem = doc.elements.get(state.selected_elem);
    let mirrored = elem.map(|e| e.mirrored).unwrap_or(true);
    let in_markers = state.table_mode.is_markers();
    let active_side = if mirrored {
        LineSide::Left
    } else {
        state.active_table_side
    };

    // Closure status for active side's lines (dX=0 at end of contour)
    let (sum_x, sum_y, line_count) = elem
        .map(|e| {
            let lines = e.lines(active_side);
            let sx: i32 = lines.iter().map(|l| l.stitches).sum();
            let sy: i32 = lines.iter().map(|l| l.rows).sum();
            (sx, sy, lines.len())
        })
        .unwrap_or((0, 0, 0));

    Frame::NONE
        .fill(colors::CARD_BG)
        .inner_margin(Margin {
            left: 8,
            right: 8,
            top: 5,
            bottom: 5,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // ── File actions ─────────────────────────────────────────────────
                if studio_icon_chip(ui, ChipIcon::Bitmap(IconKind::NewFile), "Yeni", "Yeni Kalıp (Ctrl+N)", false, true) {
                    actions.push(AppAction::NewFile);
                }
                if studio_icon_chip(ui, ChipIcon::Bitmap(IconKind::OpenShp), "Aç .shp", "İlmek Kalıbı (.shp) aç", false, true) {
                    actions.push(AppAction::OpenShp);
                }
                if studio_icon_chip(ui, ChipIcon::Bitmap(IconKind::OpenShv), "Aç .shv", "mm Kalıbı (.shv) aç (Ctrl+O)", false, true) {
                    actions.push(AppAction::OpenShv);
                }
                if studio_icon_chip(ui, ChipIcon::Bitmap(IconKind::Save), "Kaydet", "Kalıbı kaydet (Ctrl+S)", false, true) {
                    actions.push(AppAction::Save);
                }
                if studio_icon_chip(ui, ChipIcon::None, "Farklı Kaydet", "Adıyla kaydet (F12)", false, true) {
                    actions.push(AppAction::SaveAs);
                }

                tb_sep(ui);

                // ── Undo / Redo ──────────────────────────────────────────────────
                if studio_icon_chip(ui, ChipIcon::Glyph(StudioGlyph::Undo), "Geri Al", "Geri Al (Ctrl+Z)", false, can_undo) {
                    actions.push(AppAction::Undo);
                }
                if studio_icon_chip(ui, ChipIcon::Glyph(StudioGlyph::Redo), "İleri Al", "İleri Al (Ctrl+Y)", false, can_redo) {
                    actions.push(AppAction::Redo);
                }

                tb_sep(ui);

                // ── 1 cm İlmek Yoğunluğu (mandatory direct toolbar input) ────────
                let factor = (doc.header.density_base_mm / 10.0).max(0.1);
                let mut new_dx: Option<f64> = None;
                let mut new_dy: Option<f64> = None;

                ui.label(
                    egui::RichText::new("1 cm Yoğunluk:")
                        .font(bold_font(11.0))
                        .color(colors::TEXT_SECONDARY),
                );

                toolbar_micro_icon(ui, IconKind::HorizontalArrow, 14.0);
                if let Some(txt) = win32_numeric_text_edit(
                    ui,
                    "tb_density_x_1cm",
                    format!("{:.2}", doc.header.density_x / factor),
                    48.0,
                    true,
                ) {
                    if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                        if v > 0.0 && v.is_finite() {
                            let stored = (v * factor).clamp(1.0, 500.0);
                            if (stored - doc.header.density_x).abs() > 1e-6 {
                                new_dx = Some(stored);
                            }
                        }
                    }
                }

                toolbar_micro_icon(ui, IconKind::VerticalArrow, 14.0);
                if let Some(txt) = win32_numeric_text_edit(
                    ui,
                    "tb_density_y_1cm",
                    format!("{:.2}", doc.header.density_y / factor),
                    48.0,
                    true,
                ) {
                    if let Ok(v) = txt.trim().replace(',', ".").parse::<f64>() {
                        if v > 0.0 && v.is_finite() {
                            let stored = (v * factor).clamp(1.0, 500.0);
                            if (stored - doc.header.density_y).abs() > 1e-6 {
                                new_dy = Some(stored);
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

                tb_sep(ui);

                // ── Mode & View toggles ──────────────────────────────────────────
                if studio_icon_chip(
                    ui,
                    ChipIcon::Glyph(StudioGlyph::Mirror),
                    "Aynalama",
                    "Sağ tarafı otomatik aynala",
                    mirrored,
                    true,
                ) {
                    actions.push(AppAction::MirrorToggled(!mirrored));
                }

                if studio_icon_chip(
                    ui,
                    ChipIcon::Glyph(StudioGlyph::Columns),
                    "Detaylar",
                    "Basamak, Faktör, Arta kalan ve Grup sütunlarını göster/gizle",
                    state.show_detail_columns,
                    true,
                ) {
                    state.show_detail_columns = !state.show_detail_columns;
                }

                if studio_icon_chip(
                    ui,
                    ChipIcon::Bitmap(IconKind::LeftMarkers),
                    "İşaretler",
                    "Çizgiler / İşaretler görünümü arasında geçiş yap (F7/F9)",
                    in_markers,
                    true,
                ) {
                    if in_markers {
                        state.table_mode = match state.active_table_side {
                            LineSide::Left => TableMode::LeftLines,
                            LineSide::Right if !mirrored => TableMode::RightLines,
                            LineSide::Right => TableMode::LeftLines,
                        };
                    } else {
                        state.table_mode = match state.active_table_side {
                            LineSide::Left => TableMode::LeftMarkers,
                            LineSide::Right if !mirrored => TableMode::RightMarkers,
                            LineSide::Right => TableMode::LeftMarkers,
                        };
                    }
                    state.editing_cell = None;
                    state.select_single_row(0);
                }

                if studio_icon_chip(
                    ui,
                    ChipIcon::Glyph(StudioGlyph::Settings),
                    "Kalıp Ayarları",
                    "Genel kesim verileri, Standart özellikler ve Yönergeler",
                    state.settings_popover_open,
                    true,
                ) {
                    state.settings_popover_open = !state.settings_popover_open;
                }

                // ── Right-aligned Live Closure Badge / Update Pill ──────────────
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if line_count == 0 {
                        studio_badge(
                            ui,
                            None,
                            "Boş kalıp",
                            colors::INPUT_DISABLED_BG,
                            colors::TEXT_SECONDARY,
                            colors::BORDER_MID,
                        );
                    } else if sum_x == 0 {
                        studio_badge(
                            ui,
                            Some(StudioGlyph::Check),
                            &format!("dX=0, Boy={sum_y}"),
                            colors::CLOSING_ROW_GREEN,
                            colors::CLOSING_ROW_TEXT,
                            Color32::from_rgb(0x86, 0xEF, 0xAC),
                        );
                    } else if studio_fix_closure_button(
                        ui,
                        &format!("Kapanışı Düzelt (dX={sum_x})"),
                        "Konturu merkez eksene (dX=0) kapatmak için bitiş çizgisini ayarla veya ekle",
                    ) {
                        state.active_table_side = active_side;
                        state.table_mode = if active_side == LineSide::Left {
                            TableMode::LeftLines
                        } else {
                            TableMode::RightLines
                        };
                        actions.push(AppAction::CreateClosingLine);
                    }

                    if let Some(tag) = available_update_tag {
                        if studio_update_pill(ui, tag) {
                            actions.push(AppAction::ShowAbout);
                        }
                    }
                });
            });
        });

    // Crisp 1px bottom border under toolbar strip
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right(), rect.top()),
        ],
        Stroke::new(1.0_f32, colors::BORDER_MID),
    );
}

#[derive(Clone, Copy)]
enum ChipIcon {
    None,
    Bitmap(IconKind),
    Glyph(StudioGlyph),
}

fn toolbar_micro_icon(ui: &mut Ui, kind: IconKind, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), rect, kind, true);
}

fn studio_icon_chip(
    ui: &mut Ui,
    icon: ChipIcon,
    label: &str,
    tooltip: &str,
    active: bool,
    enabled: bool,
) -> bool {
    let text_col = if !enabled {
        colors::TEXT_DISABLED
    } else if active {
        colors::WIN_BLUE
    } else {
        colors::TEXT_BLACK
    };
    let font = if active { bold_font(11.0) } else { regular_font(11.0) };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, text_col);
    let has_icon = !matches!(icon, ChipIcon::None);
    let icon_w = if has_icon { 18.0 } else { 0.0 };
    let w = (icon_w + galley.size().x + 14.0).max(32.0);
    let h = 24.0;

    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);

    let (fill, border) = if !enabled {
        (colors::INPUT_DISABLED_BG, colors::BORDER_LIGHT)
    } else if active {
        (colors::WIN_BLUE_FILL, colors::WIN_BLUE)
    } else if resp.hovered() {
        (colors::WIN_BLUE_HOVER, colors::WIN_BLUE)
    } else {
        (colors::CARD_BG, colors::BORDER_MID)
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );

    let mut text_x = rect.left() + 7.0;
    match icon {
        ChipIcon::None => {}
        ChipIcon::Bitmap(ic) => {
            let ic_rect = Rect::from_min_size(
                egui::pos2(rect.left() + 6.0, rect.center().y - 7.0),
                vec2(14.0, 14.0),
            );
            paint_icon(painter, ic_rect, ic, enabled);
            text_x = ic_rect.right() + 4.0;
        }
        ChipIcon::Glyph(gl) => {
            let ic_rect = Rect::from_min_size(
                egui::pos2(rect.left() + 6.0, rect.center().y - 7.0),
                vec2(14.0, 14.0),
            );
            paint_studio_glyph(painter, ic_rect, gl, text_col);
            text_x = ic_rect.right() + 4.0;
        }
    }
    painter.galley(
        egui::pos2(text_x, rect.center().y - galley.size().y * 0.5),
        galley,
        text_col,
    );

    let clicked = enabled && resp.clicked();
    if !tooltip.is_empty() {
        resp.on_hover_text(tooltip);
    }
    clicked
}

fn studio_update_pill(ui: &mut Ui, tag: &str) -> bool {
    let text = format!("Güncelleme: {tag}");
    let text_col = Color32::from_rgb(0x03, 0x69, 0xA1);
    let galley = ui
        .painter()
        .layout_no_wrap(text, bold_font(10.5), text_col);
    let size = vec2(galley.size().x + 16.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let (fill, border) = if resp.hovered() {
        (Color32::from_rgb(0xBA, 0xE6, 0xFD), Color32::from_rgb(0x02, 0x84, 0xC7))
    } else {
        (Color32::from_rgb(0xE0, 0xF2, 0xFE), Color32::from_rgb(0x38, 0xBD, 0xF8))
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(11), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(11),
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        egui::pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        text_col,
    );
    let clicked = resp.clicked();
    resp.on_hover_text(format!(
        "Yeni sürüm mevcut ({tag})! Hakkında & Güncellemeler penceresini açmak için tıklayın."
    ));
    clicked
}

fn studio_fix_closure_button(ui: &mut Ui, text: &str, tooltip: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        bold_font(10.5),
        colors::ACCENT_AMBER_TEXT,
    );
    let size = vec2(galley.size().x + 16.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let (fill, border) = if resp.hovered() {
        (Color32::from_rgb(0xFD, 0xE6, 0x8A), Color32::from_rgb(0xD9, 0x77, 0x06))
    } else {
        (colors::ACCENT_AMBER_BG, Color32::from_rgb(0xFC, 0xD3, 0x4D))
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        egui::pos2(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        colors::ACCENT_AMBER_TEXT,
    );
    let clicked = resp.clicked();
    resp.on_hover_text(tooltip);
    clicked
}

fn studio_badge(
    ui: &mut Ui,
    glyph: Option<StudioGlyph>,
    text: &str,
    fill: Color32,
    text_col: Color32,
    border_col: Color32,
) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), bold_font(10.5), text_col);
    let icon_w = if glyph.is_some() { 15.0 } else { 0.0 };
    let size = vec2(icon_w + galley.size().x + 14.0, 22.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border_col),
        egui::StrokeKind::Inside,
    );
    let mut text_x = rect.left() + 7.0;
    if let Some(gl) = glyph {
        let ic_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 6.0, rect.center().y - 6.0),
            vec2(12.0, 12.0),
        );
        paint_studio_glyph(painter, ic_rect, gl, text_col);
        text_x = ic_rect.right() + 3.0;
    }
    painter.galley(
        egui::pos2(text_x, rect.center().y - galley.size().y * 0.5),
        galley,
        text_col,
    );
}

fn tb_sep(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(6.0, 22.0), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [egui::pos2(x, rect.top() + 3.0), egui::pos2(x, rect.bottom() - 3.0)],
        Stroke::new(1.0_f32, colors::BORDER_MID),
    );
}

fn menu_item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    let btn = egui::Button::new(label).shortcut_text(shortcut);
    ui.add_enabled(enabled, btn).clicked()
}

fn menu_radio_item(
    ui: &mut Ui,
    label: &str,
    shortcut: &str,
    selected: bool,
    enabled: bool,
) -> bool {
    let prefix = if selected { "● " } else { "   " };
    let text = format!("{prefix}{label}");
    let btn = egui::Button::new(egui::RichText::new(text).font(regular_font(11.5)))
        .shortcut_text(shortcut);
    ui.add_enabled(enabled, btn).clicked()
}

/// Draw a small mini shape preview thumbnail inside `rect` (used by Left Panel element strip & Top Panel).
pub fn paint_shape_thumbnail(
    painter: &egui::Painter,
    rect: Rect,
    polylines_mm: &[(&[stoll_kalip_editoru::math::Vec2], Color32, f32)],
    bounds: (f64, f64, f64, f64),
    draw_center_axis: bool,
    selected_border: Option<Color32>,
) {
    painter.rect_filled(rect, CornerRadius::ZERO, colors::CANVAS_BLUE);

    let (min_x, max_x, min_y, max_y) = bounds;
    let span_x = (max_x - min_x).abs().max(1.0);
    let span_y = (max_y - min_y).abs().max(1.0);
    let pad = 6.0;
    let inner = rect.shrink(pad);
    if inner.width() <= 2.0 || inner.height() <= 2.0 {
        return;
    }

    let scale = (inner.width() as f64 / span_x).min(inner.height() as f64 / span_y);
    let cx_world = 0.5 * (min_x + max_x);
    let cy_world = 0.5 * (min_y + max_y);
    let to_screen = |x: f64, y: f64| -> egui::Pos2 {
        let sx = inner.center().x + ((x - cx_world) * scale) as f32;
        let sy = inner.center().y - ((y - cy_world) * scale) as f32;
        egui::pos2(sx, sy)
    };

    // White dash-dot center axis (X = 0)
    if draw_center_axis {
        let x0 = to_screen(0.0, 0.0).x.clamp(rect.left() + 2.0, rect.right() - 2.0);
        let mut y = rect.top() + 3.0;
        let mut long = true;
        while y < rect.bottom() - 3.0 {
            let seg = if long { 6.0 } else { 2.0 };
            let y_end = (y + seg).min(rect.bottom() - 3.0);
            painter.line_segment(
                [egui::pos2(x0, y), egui::pos2(x0, y_end)],
                Stroke::new(1.0_f32, colors::AXIS_WHITE),
            );
            y = y_end + 3.0;
            long = !long;
        }
    }

    // Draw polylines
    for &(pts, color, width) in polylines_mm {
        if pts.len() >= 2 {
            let screen_pts: Vec<egui::Pos2> = pts.iter().map(|p| to_screen(p.x, p.y)).collect();
            painter.add(egui::Shape::line(screen_pts, Stroke::new(width, color)));
        }
    }

    // Frame border
    let border_col = selected_border.unwrap_or(colors::BORDER_3D_LO);
    let border_w: f32 = if selected_border.is_some() { 1.5 } else { 1.0 };
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(border_w, border_col),
        egui::StrokeKind::Inside,
    );
}
