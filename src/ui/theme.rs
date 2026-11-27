use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Rect, Shadow, Stroke, Style, TextStyle,
    Ui, Vec2, Visuals,
};

/// Modern Light Studio color palette (clean crisp light studio, 1px subtle borders, slate text).
pub mod colors {
    use egui::Color32;

    /// Window shell background (`#F4F6FA`)
    pub const CHROME_BG: Color32 = Color32::from_rgb(0xF4, 0xF6, 0xFA);
    /// Elevated card / menu bar background (`#FFFFFF`)
    pub const CARD_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Menu bar background (`#FFFFFF`)
    pub const MENU_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Editable input / cell background (`#FFFFFF`)
    pub const INPUT_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Disabled input / panel sunken background (`#F1F5F9`)
    pub const INPUT_DISABLED_BG: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
    /// Read-only / greyed grid cell background (`#F1F5F9`)
    pub const CELL_READONLY_BG: Color32 = Color32::from_rgb(0xF1, 0xF5, 0xF9);
    /// Button face inside grid (`Kavis` button)
    pub const CELL_BTN_BG: Color32 = Color32::from_rgb(0xF8, 0xFA, 0xFC);
    /// Closing row highlight green (`#DCFCE7`)
    pub const CLOSING_ROW_GREEN: Color32 = Color32::from_rgb(0xDC, 0xFC, 0xE7);
    /// Positive / closing text green (`#16A34A`)
    pub const CLOSING_ROW_TEXT: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
    /// Studio CAD canvas background (`#F8FAFC` / subtle slate)
    pub const CANVAS_BLUE: Color32 = Color32::from_rgb(0x78, 0xA0, 0xC8);
    /// Bright green shape contour outline
    pub const CONTOUR_GREEN: Color32 = Color32::from_rgb(0x11, 0xFF, 0x7C);
    /// Center symmetry axis white (`#FFFFFF`)
    pub const AXIS_WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Selected line segment highlight red (`#EF4444`)
    pub const SELECTED_LINE_RED: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);
    /// Secondary element contour amber/yellow (`#F59E0B`)
    pub const SECONDARY_YELLOW: Color32 = Color32::from_rgb(0xFF, 0xE0, 0x40);
    /// Graphics window grid lines (`#E2E8F0`)
    pub const GRAPH_GRID: Color32 = Color32::from_rgb(0xA0, 0xB8, 0xD0);

    /// Crisp 1px Studio borders
    pub const BORDER_DARK: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);
    pub const BORDER_MID: Color32 = Color32::from_rgb(0xDC, 0xE2, 0xEC);
    pub const BORDER_LIGHT: Color32 = Color32::from_rgb(0xE2, 0xE8, 0xF0);
    pub const BORDER_BLACK: Color32 = Color32::from_rgb(0xDC, 0xE2, 0xEC);
    #[allow(dead_code)]
    pub const BORDER_3D_HI: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const BORDER_3D_LO: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);

    /// Primary blue & hover accents
    pub const WIN_BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    pub const WIN_BLUE_FILL: Color32 = Color32::from_rgb(0xDB, 0xEA, 0xFE);
    pub const WIN_BLUE_HOVER: Color32 = Color32::from_rgb(0xEF, 0xF6, 0xFF);
    pub const CELL_FOCUS_BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    pub const ROW_SELECT_TINT: Color32 = Color32::from_rgb(0xEF, 0xF6, 0xFF);

    /// Text colors
    pub const TEXT_BLACK: Color32 = Color32::from_rgb(0x1E, 0x29, 0x3B);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B);
    pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
    pub const CURVE_GLYPH_BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    pub const STATUS_ERR_RED: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);
    pub const STATUS_OK_GREEN: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
    pub const ACCENT_AMBER_BG: Color32 = Color32::from_rgb(0xFE, 0xF3, 0xC7);
    pub const ACCENT_AMBER_TEXT: Color32 = Color32::from_rgb(0xB4, 0x53, 0x09);
}

/// Load Windows `Segoe UI` (regular + bold + italic if available) so Turkish glyphs
/// (`ğ, Ğ, ı, İ, ö, Ö, ş, Ş, ü, Ü, ç, Ç`) render identically to native Win32 MFC.
pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let default_prop_tail = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let candidates = [
        ("segoe_ui", r"C:\Windows\Fonts\segoeui.ttf"),
        ("tahoma", r"C:\Windows\Fonts\tahoma.ttf"),
        ("arial", r"C:\Windows\Fonts\arial.ttf"),
    ];

    for (name, path) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_owned(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, name.to_owned());
            break;
        }
    }

    // Load Segoe UI Symbol as fallback for technical glyphs if installed on Windows
    let has_seguisym = if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\seguisym.ttf") {
        fonts
            .font_data
            .insert("segoe_sym".to_owned(), egui::FontData::from_owned(bytes).into());
        if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
            let insert_at = if fonts.font_data.contains_key("segoe_ui") {
                1.min(prop.len())
            } else {
                0
            };
            prop.insert(insert_at, "segoe_sym".to_owned());
        }
        true
    } else {
        false
    };

    // Bold family for grid headers & bold Fonksiyon cells
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeuib.ttf")
        .or_else(|_| std::fs::read(r"C:\Windows\Fonts\tahomabd.ttf"))
    {
        fonts
            .font_data
            .insert("segoe_bold".to_owned(), egui::FontData::from_owned(bytes).into());
        let mut bold_chain = vec!["segoe_bold".to_owned()];
        if fonts.font_data.contains_key("segoe_ui") {
            bold_chain.push("segoe_ui".to_owned());
        }
        if has_seguisym {
            bold_chain.push("segoe_sym".to_owned());
        }
        for fb in &default_prop_tail {
            if !bold_chain.contains(fb) {
                bold_chain.push(fb.clone());
            }
        }
        fonts
            .families
            .insert(FontFamily::Name("Bold".into()), bold_chain);
    }

    // Bold-Italic family for UserSetFunction=1 (`Remayözleme` in screenshot 222001.png)
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeuiz.ttf")
        .or_else(|_| std::fs::read(r"C:\Windows\Fonts\segoeuii.ttf"))
    {
        fonts.font_data.insert(
            "segoe_bold_italic".to_owned(),
            egui::FontData::from_owned(bytes).into(),
        );
        let mut bi_chain = vec!["segoe_bold_italic".to_owned()];
        if fonts.font_data.contains_key("segoe_ui") {
            bi_chain.push("segoe_ui".to_owned());
        }
        if has_seguisym {
            bi_chain.push("segoe_sym".to_owned());
        }
        for fb in &default_prop_tail {
            if !bi_chain.contains(fb) {
                bi_chain.push(fb.clone());
            }
        }
        fonts.families.insert(
            FontFamily::Name("BoldItalic".into()),
            bi_chain,
        );
    }

    // Monospace fallback
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
        fonts
            .font_data
            .insert("consolas".to_owned(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "consolas".to_owned());
    }

    ctx.set_fonts(fonts);
}

/// Apply Modern Light Studio visual styling (crisp light surfaces, subtle 4px rounding, 1px borders).
pub fn apply_win32_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::light();
    visuals.window_fill = colors::CARD_BG;
    visuals.panel_fill = colors::CHROME_BG;
    visuals.faint_bg_color = colors::INPUT_DISABLED_BG;
    visuals.extreme_bg_color = colors::INPUT_BG;
    visuals.code_bg_color = colors::INPUT_BG;
    visuals.override_text_color = Some(colors::TEXT_BLACK);

    let r4 = CornerRadius::same(4);
    let r6 = CornerRadius::same(6);
    visuals.window_corner_radius = r6;
    visuals.menu_corner_radius = r4;
    visuals.window_stroke = Stroke::new(1.0_f32, colors::BORDER_MID);
    visuals.window_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(28),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(24),
    };

    visuals.selection.bg_fill = colors::WIN_BLUE;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

    // Non-interactive (labels, frames)
    visuals.widgets.noninteractive.bg_fill = colors::CHROME_BG;
    visuals.widgets.noninteractive.weak_bg_fill = colors::CHROME_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER_MID);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_BLACK);
    visuals.widgets.noninteractive.corner_radius = r4;

    // Inactive buttons / comboboxes
    visuals.widgets.inactive.bg_fill = colors::CARD_BG;
    visuals.widgets.inactive.weak_bg_fill = colors::CARD_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, colors::BORDER_DARK);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_BLACK);
    visuals.widgets.inactive.corner_radius = r4;

    // Hovered
    visuals.widgets.hovered.bg_fill = colors::WIN_BLUE_HOVER;
    visuals.widgets.hovered.weak_bg_fill = colors::WIN_BLUE_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, colors::WIN_BLUE);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_BLACK);
    visuals.widgets.hovered.corner_radius = r4;
    visuals.widgets.hovered.expansion = 0.0;

    // Active / pressed
    visuals.widgets.active.bg_fill = colors::WIN_BLUE_FILL;
    visuals.widgets.active.weak_bg_fill = colors::WIN_BLUE_FILL;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, colors::WIN_BLUE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_BLACK);
    visuals.widgets.active.corner_radius = r4;
    visuals.widgets.active.expansion = 0.0;

    // Open dropdown
    visuals.widgets.open.bg_fill = colors::WIN_BLUE_FILL;
    visuals.widgets.open.weak_bg_fill = colors::WIN_BLUE_FILL;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, colors::WIN_BLUE);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, colors::TEXT_BLACK);
    visuals.widgets.open.corner_radius = r4;

    let mut style = Style::default();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.window_margin = Margin::same(8);
    style.spacing.button_padding = Vec2::new(8.0, 3.0);
    style.spacing.interact_size = Vec2::new(22.0, 20.0);
    style.spacing.combo_height = 240.0;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.floating = false;

    style.text_styles = [
        (TextStyle::Small, FontId::new(10.5, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(14.5, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
    ]
    .into();

    ctx.set_style(style);
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("Bold".into()))
}

pub fn bold_italic_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("BoldItalic".into()))
}

pub fn regular_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Draw a classic Win32 etched `GroupBox` frame with caption embedded in the top border
/// on the default `CHROME_BG` (`#F0F0F0`) surface.
pub fn win32_group_box<R>(
    ui: &mut Ui,
    title: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    win32_group_box_on(ui, title, colors::CHROME_BG, add_contents)
}

/// Draw a classic Win32 etched `GroupBox` frame with caption mask matching `bg_color`
/// (use `#F9F9F9` inside tab pages, `#F0F0F0` on dialog/panel surfaces).
pub fn win32_group_box_on<R>(
    ui: &mut Ui,
    title: &str,
    bg_color: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let start_pos = ui.cursor().min;
    let avail_w = ui.available_width();

    // Reserve space for caption line + inner content
    let inner = ui.scope(|ui| {
        ui.add_space(7.0);
        let frame = egui::Frame::NONE.inner_margin(Margin {
            left: 6,
            right: 6,
            top: 7,
            bottom: 5,
        });
        frame.show(ui, |ui| {
            ui.set_min_width((avail_w - 14.0).max(40.0));
            add_contents(ui)
        })
    });

    let box_rect = Rect::from_min_max(
        egui::pos2(start_pos.x + 1.0, start_pos.y + 6.0),
        egui::pos2(
            (start_pos.x + avail_w - 1.0).max(inner.response.rect.right() + 1.0),
            inner.response.rect.bottom() + 1.0,
        ),
    );

    let painter = ui.painter();
    painter.rect_stroke(
        box_rect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, colors::BORDER_MID),
        egui::StrokeKind::Inside,
    );

    if !title.is_empty() {
        let galley = painter.layout_no_wrap(
            title.to_owned(),
            regular_font(11.0),
            colors::TEXT_BLACK,
        );
        let title_pos = egui::pos2(box_rect.left() + 6.0, start_pos.y);
        let bg_rect = Rect::from_min_size(
            egui::pos2(title_pos.x - 2.0, title_pos.y),
            Vec2::new(galley.size().x + 4.0, galley.size().y),
        );
        painter.rect_filled(bg_rect, CornerRadius::ZERO, bg_color);
        painter.galley(title_pos, galley, colors::TEXT_BLACK);
    }

    inner.inner.inner
}

/// Scope that styles `egui::ComboBox` widgets with classic Win32 white `#FFFFFF` fill
/// and crisp `1px #ADADAD` border (matching Stoll screenshots 1:1).
pub fn win32_combo_scope<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        let enabled = ui.is_enabled();
        let fill = if enabled {
            colors::INPUT_BG
        } else {
            colors::INPUT_DISABLED_BG
        };
        let border = if enabled {
            Color32::from_rgb(0xAD, 0xAD, 0xAD)
        } else {
            colors::BORDER_LIGHT
        };
        let w = &mut ui.style_mut().visuals.widgets;
        w.inactive.bg_fill = fill;
        w.inactive.weak_bg_fill = fill;
        w.inactive.bg_stroke = Stroke::new(1.0_f32, border);
        w.hovered.bg_fill = colors::INPUT_BG;
        w.hovered.weak_bg_fill = colors::INPUT_BG;
        w.hovered.bg_stroke = Stroke::new(1.0_f32, colors::WIN_BLUE);
        w.open.bg_fill = colors::INPUT_BG;
        w.open.weak_bg_fill = colors::INPUT_BG;
        w.open.bg_stroke = Stroke::new(1.0_f32, colors::WIN_BLUE);
        add_contents(ui)
    })
    .inner
}

/// Classic Win32 checkbox: 13×13 crisp bordered square (`#FFFFFF` + `#707070` unchecked,
/// Windows blue `#0078D7` + crisp white checkmark when checked).
pub fn win32_checkbox(ui: &mut Ui, checked: &mut bool, label: &str) -> egui::Response {
    let enabled = ui.is_enabled();
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let galley = if label.is_empty() {
        None
    } else {
        Some(ui.painter().layout_no_wrap(
            label.to_owned(),
            regular_font(11.0),
            if enabled {
                colors::TEXT_BLACK
            } else {
                colors::TEXT_DISABLED
            },
        ))
    };
    let box_sz = 13.0_f32;
    let total_w = box_sz + galley.as_ref().map(|g| 5.0 + g.size().x).unwrap_or(0.0);
    let total_h = box_sz.max(galley.as_ref().map(|g| g.size().y).unwrap_or(0.0)).max(16.0);
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(total_w, total_h), sense);
    if enabled && resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let painter = ui.painter();
    let box_rect = Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - box_sz * 0.5),
        Vec2::splat(box_sz),
    );
    let (fill, stroke_col) = if !enabled {
        (Color32::from_rgb(0xE6, 0xE6, 0xE6), Color32::from_rgb(0xBC, 0xBC, 0xBC))
    } else if *checked {
        (colors::WIN_BLUE, colors::WIN_BLUE)
    } else if resp.hovered() {
        (colors::INPUT_BG, colors::WIN_BLUE)
    } else {
        (colors::INPUT_BG, Color32::from_rgb(0x70, 0x70, 0x70))
    };
    painter.rect_filled(box_rect, CornerRadius::same(3), fill);
    painter.rect_stroke(
        box_rect,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, stroke_col),
        egui::StrokeKind::Inside,
    );
    if *checked {
        let chk_col = if enabled {
            Color32::WHITE
        } else {
            Color32::from_rgb(0x90, 0x90, 0x90)
        };
        let p0 = egui::pos2(box_rect.left() + 2.5, box_rect.center().y + 0.5);
        let p1 = egui::pos2(box_rect.left() + 5.2, box_rect.bottom() - 3.0);
        let p2 = egui::pos2(box_rect.right() - 2.3, box_rect.top() + 3.0);
        painter.line_segment([p0, p1], Stroke::new(1.6_f32, chk_col));
        painter.line_segment([p1, p2], Stroke::new(1.6_f32, chk_col));
    }
    if let Some(g) = galley {
        let pos = egui::pos2(box_rect.right() + 5.0, rect.center().y - g.size().y * 0.5);
        painter.galley(pos, g, colors::TEXT_BLACK);
    }
    resp
}

/// Classic Win32 radio button: 13×13 white circle with `#707070` ring unselected,
/// `#0078D7` ring + center dot selected.
pub fn win32_radio<V: PartialEq>(
    ui: &mut Ui,
    current: &mut V,
    target: V,
    label: &str,
) -> egui::Response {
    let enabled = ui.is_enabled();
    let selected = *current == target;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let galley = if label.is_empty() {
        None
    } else {
        Some(ui.painter().layout_no_wrap(
            label.to_owned(),
            regular_font(11.0),
            if enabled {
                colors::TEXT_BLACK
            } else {
                colors::TEXT_DISABLED
            },
        ))
    };
    let d = 13.0_f32;
    let total_w = d + galley.as_ref().map(|g| 5.0 + g.size().x).unwrap_or(0.0);
    let total_h = d.max(galley.as_ref().map(|g| g.size().y).unwrap_or(0.0)).max(16.0);
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(total_w, total_h), sense);
    if enabled && resp.clicked() && !selected {
        *current = target;
        resp.mark_changed();
    }
    let painter = ui.painter();
    let center = egui::pos2(rect.left() + d * 0.5, rect.center().y);
    let (fill, ring) = if !enabled {
        (Color32::from_rgb(0xE6, 0xE6, 0xE6), Color32::from_rgb(0xBC, 0xBC, 0xBC))
    } else if selected {
        (colors::INPUT_BG, colors::WIN_BLUE)
    } else if resp.hovered() {
        (colors::INPUT_BG, colors::WIN_BLUE)
    } else {
        (colors::INPUT_BG, Color32::from_rgb(0x70, 0x70, 0x70))
    };
    painter.circle_filled(center, d * 0.5 - 0.5, fill);
    painter.circle_stroke(center, d * 0.5 - 0.5, Stroke::new(1.2_f32, ring));
    if selected {
        let dot_col = if enabled {
            colors::WIN_BLUE
        } else {
            Color32::from_rgb(0x90, 0x90, 0x90)
        };
        painter.circle_filled(center, 3.0, dot_col);
    }
    if let Some(g) = galley {
        let pos = egui::pos2(rect.left() + d + 5.0, rect.center().y - g.size().y * 0.5);
        painter.galley(pos, g, colors::TEXT_BLACK);
    }
    resp
}

/// Classic Win32 push button with crisp `1px #ADADAD` border (`#E1E1E1` fill, `#CCCCCC` disabled).
pub fn win32_button(ui: &mut Ui, text: &str, min_size: Vec2, enabled: bool) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let text_col = if enabled {
        colors::TEXT_BLACK
    } else {
        Color32::from_rgb(0x83, 0x83, 0x83)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), regular_font(11.0), text_col);
    let size = Vec2::new(
        min_size.x.max(galley.size().x + 14.0),
        min_size.y.max(galley.size().y + 6.0),
    );
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
    let pos = egui::pos2(
        rect.center().x - galley.size().x * 0.5,
        rect.center().y - galley.size().y * 0.5,
    );
    painter.galley(pos, galley, text_col);
    resp
}

#[derive(Clone)]
struct NumericEditMem {
    buf: String,
    pristine: bool,
}

/// Returns `true` if `s` is a zero literal (`"0"`, `"0.0"`, `"0.00"`, etc.).
pub fn is_zero_display(s: &str) -> bool {
    matches!(
        s.trim(),
        "0" | "0.0" | "0.00" | "-0" | "+0" | "0,0" | "0,00"
    )
}

/// Guard against the egui `"0"` caret-at-end trap when editing a numeric cell that initially held `"0"`.
/// While `pristine` is true, any newly typed character concatenated onto `"0"` (`"0-80"`, `"-0"`, `"05"`, `"50"`)
/// strips the stale `"0"` automatically. Once any change occurs, `pristine` becomes `false`.
/// Returns `true` if `buf` was rewritten so the caller can move the cursor to the end.
pub fn apply_pristine_zero_guard(prev_buf: &str, buf: &mut String, pristine: &mut bool) -> bool {
    let mut modified = false;
    if *buf != prev_buf {
        if *pristine {
            if is_zero_display(prev_buf) {
                let pt = prev_buf.trim();
                if buf.starts_with(pt) && buf.len() > pt.len() {
                    let tail = &buf[pt.len()..];
                    if tail == "." || tail == "," {
                        *buf = format!("0{tail}");
                        modified = true;
                    } else if tail
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '-' || c == '+')
                    {
                        *buf = tail.to_string();
                        modified = true;
                    }
                } else if buf.ends_with(pt) && buf.len() > pt.len() {
                    let head = &buf[..buf.len() - pt.len()];
                    if head == "." || head == "," {
                        *buf = format!("0{head}");
                        modified = true;
                    } else if head
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '-' || c == '+')
                    {
                        *buf = head.to_string();
                        modified = true;
                    }
                }
            }
            *pristine = false;
        }
        if (buf.starts_with("0-") || buf.starts_with("0+")) && buf.len() >= 2 {
            *buf = buf[1..].to_string();
            modified = true;
        }
    }
    modified
}

/// Right-aligned Win32 white sunken numeric `TextEdit` that commits only on Enter or focus-loss
/// (no drag-pixel recalculation spam, 1:1 classic Win32 Edit control look).
pub fn win32_numeric_text_edit(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash,
    display_value: String,
    width: f32,
    enabled: bool,
) -> Option<String> {
    let mem_id = ui.make_persistent_id(id_salt);
    let mut committed: Option<String> = None;

    if !enabled {
        ui.ctx().data_mut(|d| d.remove::<NumericEditMem>(mem_id));
        win32_input_frame(false).show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new((width - 8.0).max(20.0), 15.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new(display_value)
                            .font(regular_font(11.0))
                            .color(colors::TEXT_DISABLED),
                    );
                },
            );
        });
        return None;
    }

    let mut mem = ui
        .ctx()
        .data_mut(|d| d.get_temp::<NumericEditMem>(mem_id))
        .unwrap_or_else(|| NumericEditMem {
            buf: display_value.clone(),
            pristine: true,
        });
    let prev_buf = mem.buf.clone();

    let mut output_opt: Option<egui::text_edit::TextEditOutput> = None;
    win32_input_frame(true).show(ui, |ui| {
        let te = egui::TextEdit::singleline(&mut mem.buf)
            .font(regular_font(11.0))
            .horizontal_align(egui::Align::RIGHT)
            .frame(false)
            .desired_width((width - 8.0).max(20.0));
        output_opt = Some(te.show(ui));
    });

    if let Some(mut output) = output_opt {
        let resp = output.response;
        if resp.gained_focus() {
            mem.pristine = true;
            let len = mem.buf.chars().count();
            output.state.cursor.set_char_range(Some(
                egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                ),
            ));
            output.state.clone().store(ui.ctx(), resp.id);
        } else if resp.has_focus()
            && ui.input(|i| {
                i.key_pressed(egui::Key::ArrowLeft)
                    || i.key_pressed(egui::Key::ArrowRight)
                    || i.key_pressed(egui::Key::Home)
                    || i.key_pressed(egui::Key::End)
                    || i.key_pressed(egui::Key::Backspace)
                    || i.key_pressed(egui::Key::Delete)
            })
        {
            mem.pristine = false;
        }

        if apply_pristine_zero_guard(&prev_buf, &mut mem.buf, &mut mem.pristine) {
            let len = mem.buf.chars().count();
            output.state.cursor.set_char_range(Some(
                egui::text::CCursorRange::one(egui::text::CCursor::new(len)),
            ));
            output.state.store(ui.ctx(), resp.id);
        }

        let enter = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let esc = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            ui.ctx().data_mut(|d| d.remove::<NumericEditMem>(mem_id));
            resp.surrender_focus();
        } else if enter || resp.lost_focus() {
            if mem.buf != display_value {
                committed = Some(mem.buf.clone());
            }
            ui.ctx().data_mut(|d| d.remove::<NumericEditMem>(mem_id));
            if enter {
                resp.surrender_focus();
            }
        } else if resp.has_focus() {
            ui.ctx().data_mut(|d| d.insert_temp(mem_id, mem));
        } else {
            ui.ctx().data_mut(|d| d.remove::<NumericEditMem>(mem_id));
        }
    }
    committed
}

/// Classic Win32 floating/modal window with a crisp 26px `#FFFFFF` titlebar,
/// optional mini Stoll icon, left-aligned regular black title text, and hover-red `×` close button.
pub fn win32_window(
    ctx: &egui::Context,
    id: egui::Id,
    title: &str,
    open: &mut bool,
    default_size: Option<Vec2>,
    resizable: bool,
    show_icon: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    if !*open {
        return;
    }
    let mut win = egui::Window::new(title)
        .id(id)
        .title_bar(false)
        .collapsible(false)
        .resizable(resizable)
        .frame(
            egui::Frame::NONE
                .fill(colors::CARD_BG)
                .corner_radius(CornerRadius::same(6))
                .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                .shadow(Shadow {
                    offset: [0, 4],
                    blur: 14,
                    spread: 0,
                    color: Color32::from_black_alpha(32),
                }),
        );
    if let Some(sz) = default_size {
        win = win.default_size(sz);
    }

    let mut close_clicked = false;
    win.show(ctx, |ui| {
        // Custom 26px white Win32 titlebar (Sense::hover lets egui::Window drag naturally)
        let title_h = 26.0_f32;
        let avail_w = ui.available_width();
        let (bar_rect, _) =
            ui.allocate_exact_size(Vec2::new(avail_w, title_h), egui::Sense::hover());

        let painter = ui.painter();
        painter.rect_filled(bar_rect, CornerRadius::ZERO, Color32::WHITE);
        painter.line_segment(
            [bar_rect.left_bottom(), bar_rect.right_bottom()],
            Stroke::new(1.0_f32, Color32::from_rgb(0xE0, 0xE0, 0xE0)),
        );

        let mut text_left = bar_rect.left() + 8.0;
        if show_icon {
            let icon_r = Rect::from_min_size(
                egui::pos2(bar_rect.left() + 6.0, bar_rect.center().y - 7.0),
                Vec2::splat(14.0),
            );
            painter.rect_filled(icon_r, CornerRadius::ZERO, colors::CANVAS_BLUE);
            painter.rect_stroke(
                icon_r,
                CornerRadius::ZERO,
                Stroke::new(1.0_f32, Color32::from_rgb(0x30, 0x50, 0x78)),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                icon_r.shrink(3.0),
                CornerRadius::ZERO,
                Stroke::new(1.0_f32, colors::CONTOUR_GREEN),
                egui::StrokeKind::Inside,
            );
            text_left = icon_r.right() + 6.0;
        }

        let close_w = 32.0_f32;
        let close_rect = Rect::from_min_max(
            egui::pos2(bar_rect.right() - close_w, bar_rect.top()),
            bar_rect.max,
        );
        let close_resp = ui.interact(close_rect, id.with("win32_close"), egui::Sense::click());
        if close_resp.hovered() {
            painter.rect_filled(close_rect, CornerRadius::ZERO, Color32::from_rgb(0xE8, 0x11, 0x23));
        }
        let x_col = if close_resp.hovered() {
            Color32::WHITE
        } else {
            colors::TEXT_BLACK
        };
        let xc = close_rect.center();
        painter.line_segment(
            [egui::pos2(xc.x - 4.0, xc.y - 4.0), egui::pos2(xc.x + 4.0, xc.y + 4.0)],
            Stroke::new(1.1_f32, x_col),
        );
        painter.line_segment(
            [egui::pos2(xc.x + 4.0, xc.y - 4.0), egui::pos2(xc.x - 4.0, xc.y + 4.0)],
            Stroke::new(1.1_f32, x_col),
        );
        if close_resp.clicked() {
            close_clicked = true;
        }

        // Clipped title text
        let title_clip = Rect::from_min_max(
            egui::pos2(text_left, bar_rect.top()),
            egui::pos2(close_rect.left() - 4.0, bar_rect.bottom()),
        );
        let clipped_painter = painter.with_clip_rect(title_clip);
        clipped_painter.text(
            egui::pos2(text_left, bar_rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            regular_font(11.5),
            colors::TEXT_BLACK,
        );

        // Inner content
        egui::Frame::NONE
            .fill(colors::CHROME_BG)
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                add_contents(ui);
            });
    });

    if close_clicked {
        *open = false;
    }
}

/// Elevated Light Studio card container (`#FFFFFF`, 4px radius, 1px `#DCE2EC` border).
pub fn studio_card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(colors::CARD_BG)
        .corner_radius(CornerRadius::same(4))
        .stroke(Stroke::new(1.0_f32, colors::BORDER_MID))
        .inner_margin(Margin::same(8))
}

/// Draw a crisp sunken white box (or read-only light slate box) around an inner widget.
pub fn win32_input_frame(editable: bool) -> egui::Frame {
    egui::Frame::NONE
        .fill(if editable {
            colors::INPUT_BG
        } else {
            colors::INPUT_DISABLED_BG
        })
        .corner_radius(CornerRadius::same(4))
        .stroke(Stroke::new(
            1.0_f32,
            if editable {
                colors::BORDER_DARK
            } else {
                colors::BORDER_LIGHT
            },
        ))
        .inner_margin(Margin::symmetric(5, 2))
}

/// Paint a crisp Studio button rectangle (used for `Kavis` cell).
pub fn paint_win32_3d_button(painter: &egui::Painter, rect: Rect, pressed: bool, enabled: bool) {
    let bg = if !enabled {
        colors::INPUT_DISABLED_BG
    } else if pressed {
        colors::WIN_BLUE_FILL
    } else {
        colors::CELL_BTN_BG
    };
    let border = if enabled && pressed {
        colors::WIN_BLUE
    } else {
        colors::BORDER_DARK
    };
    let r = rect.shrink(1.0);
    painter.rect_filled(r, CornerRadius::same(3), bg);
    painter.rect_stroke(
        r,
        CornerRadius::same(3),
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
}
