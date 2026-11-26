use egui::{pos2, vec2, Color32, ColorImage, Painter, Rect, Stroke, TextureHandle, TextureOptions};

use crate::ui::theme::colors;

// Exact original Stoll M1plus VecFormEditor & formtable.dll bitmaps extracted from PE resources
const TB_10800_RGBA: &[u8] = include_bytes!("../../assets/icons/tb_10800.rgba");
const TB_10820_RGBA: &[u8] = include_bytes!("../../assets/icons/tb_10820.rgba");
const TB_10821_RGBA: &[u8] = include_bytes!("../../assets/icons/tb_10821.rgba");
const TB_10822_RGBA: &[u8] = include_bytes!("../../assets/icons/tb_10822.rgba");
const TB_10823_RGBA: &[u8] = include_bytes!("../../assets/icons/tb_10823.rgba");
const FT_4308_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4308.rgba");
const FT_4309_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4309.rgba");
const FT_4282_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4282.rgba");
const FT_4283_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4283.rgba");
const FT_4293_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4293.rgba");
const FT_4294_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4294.rgba");
const FT_4301_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4301.rgba");
const FT_4302_RGBA: &[u8] = include_bytes!("../../assets/icons/ft_4302.rgba");

/// All original Stoll M1plus VecFormEditor icons (1:1 PE bitmap strips + inline UI glyphs).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    // Standart toolbar (10800 — 11 sprites of 16×15)
    NewFile,
    OpenShv,
    OpenShp,
    OpenShpx,
    Save,
    AlwaysOnTop,
    ToggleGraph,
    LeftLines,
    RightLines,
    LeftMarkers,
    RightMarkers,

    // Giriş toolbar (10820 — 2 sprites of 16×15)
    SignCartesian,
    SignKnitting,

    // Elemanlar toolbar (10821 — 4 sprites of 16×15)
    AddElement,
    DeleteElement,
    SelectAllElements,
    NecklineProps,

    // Çizgiler toolbar (10822 — 9 sprites of 16×15)
    DeleteLine,
    GroupLines,
    CreateClosingLine,
    AppendLine,
    InsertLineBefore,
    CopyLines,
    PasteReplaceLines,
    PasteInsertLines,
    ImportExternalTable,

    // İşaretler toolbar (10823 — 8 sprites of 16×15)
    DeleteMarker,
    GroupMarkers,
    AppendMarker,
    InsertMarkerBefore,
    MultiplyMarkers,
    CopyMarkers,
    PasteReplaceMarkers,
    PasteInsertMarkers,

    // Curve Editor toolbar (formtable.dll 4308 & 4309 — 16×15)
    CurveLoad,
    CurveSave,
    CurveDelete,
    CurveGroup,
    CurveAppend,
    CurveInsert,
    CurveCopy,
    CurvePaste,

    // Inline UI micro-icons (formtable.dll 4282, 4283, 4293, 4294, 4301, 4302)
    HorizontalArrow,
    VerticalArrow,
    VerticalArrowFromBottom,
    VerticalArrowFromTop,
    StitchLoop,
    ElementBadge,
    CurveLine,
    CurveJ,
    CurveReverseJ,
    CurveS,
    CurveUser,

    // Clean vector toolbar / UI icons (also available via `StudioGlyph`)
    OpenFile,
    SaveFile,
    Pin,
    PinActive,
    UpdateAlert,
}

#[derive(Clone)]
struct IconAtlasPair {
    enabled: TextureHandle,
    disabled: TextureHandle,
    w: u32,
    h: u32,
}

#[derive(Clone, Copy)]
enum SpriteSheet {
    Tb10800,
    Tb10820,
    Tb10821,
    Tb10822,
    Tb10823,
    Ft4308,
    Ft4309,
    Ft4282,
    Ft4283,
    Ft4293,
    Ft4294,
    Ft4301,
    Ft4302,
}

impl SpriteSheet {
    fn key(self) -> &'static str {
        match self {
            Self::Tb10800 => "stoll_icon_tb_10800",
            Self::Tb10820 => "stoll_icon_tb_10820",
            Self::Tb10821 => "stoll_icon_tb_10821",
            Self::Tb10822 => "stoll_icon_tb_10822",
            Self::Tb10823 => "stoll_icon_tb_10823",
            Self::Ft4308 => "stoll_icon_ft_4308",
            Self::Ft4309 => "stoll_icon_ft_4309",
            Self::Ft4282 => "stoll_icon_ft_4282",
            Self::Ft4283 => "stoll_icon_ft_4283",
            Self::Ft4293 => "stoll_icon_ft_4293",
            Self::Ft4294 => "stoll_icon_ft_4294",
            Self::Ft4301 => "stoll_icon_ft_4301",
            Self::Ft4302 => "stoll_icon_ft_4302",
        }
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Tb10800 => TB_10800_RGBA,
            Self::Tb10820 => TB_10820_RGBA,
            Self::Tb10821 => TB_10821_RGBA,
            Self::Tb10822 => TB_10822_RGBA,
            Self::Tb10823 => TB_10823_RGBA,
            Self::Ft4308 => FT_4308_RGBA,
            Self::Ft4309 => FT_4309_RGBA,
            Self::Ft4282 => FT_4282_RGBA,
            Self::Ft4283 => FT_4283_RGBA,
            Self::Ft4293 => FT_4293_RGBA,
            Self::Ft4294 => FT_4294_RGBA,
            Self::Ft4301 => FT_4301_RGBA,
            Self::Ft4302 => FT_4302_RGBA,
        }
    }
}

fn make_disabled_rgba(pix: &[u8]) -> Vec<u8> {
    // Classic Win32 disabled embossed monochrome grey (#989898 - #B8B8B8, zero color leaks)
    let mut dis_rgba = Vec::with_capacity(pix.len());
    for chunk in pix.chunks_exact(4) {
        let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        if a == 0 {
            dis_rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let lum = ((r as u32 * 30 + g as u32 * 59 + b as u32 * 11) / 100) as u8;
            let grey = if lum < 160 {
                148u8.saturating_add(lum / 6)
            } else {
                210u8
            };
            dis_rgba.extend_from_slice(&[grey, grey, grey, a]);
        }
    }
    dis_rgba
}

fn load_sheet(ctx: &egui::Context, sheet: SpriteSheet) -> IconAtlasPair {
    let id = egui::Id::new(sheet.key());
    if let Some(pair) = ctx.data(|d| d.get_temp::<IconAtlasPair>(id)) {
        return pair;
    }

    let raw = sheet.bytes();
    let w = u16::from_le_bytes([raw[0], raw[1]]) as usize;
    let h = u16::from_le_bytes([raw[2], raw[3]]) as usize;
    let pix = &raw[4..];

    let img_enabled = ColorImage::from_rgba_unmultiplied([w, h], pix);
    let dis_rgba = make_disabled_rgba(pix);
    let img_disabled = ColorImage::from_rgba_unmultiplied([w, h], &dis_rgba);

    let enabled = ctx.load_texture(sheet.key(), img_enabled, TextureOptions::NEAREST);
    let disabled = ctx.load_texture(
        format!("{}_dis", sheet.key()),
        img_disabled,
        TextureOptions::NEAREST,
    );
    let pair = IconAtlasPair {
        enabled,
        disabled,
        w: w as u32,
        h: h as u32,
    };
    ctx.data_mut(|d| d.insert_temp(id, pair.clone()));
    pair
}

/// Build an exact horizontal + vertical mirror of Stoll's `CurveJ` bitmap sprite (`FT_4308_RGBA` sprite 7).
fn load_reverse_j_pair(ctx: &egui::Context) -> IconAtlasPair {
    let id = egui::Id::new("stoll_icon_curve_reverse_j");
    if let Some(pair) = ctx.data(|d| d.get_temp::<IconAtlasPair>(id)) {
        return pair;
    }

    let raw = FT_4308_RGBA;
    let sheet_w = u16::from_le_bytes([raw[0], raw[1]]) as usize;
    let sh = u16::from_le_bytes([raw[2], raw[3]]) as usize;
    let pix = &raw[4..];
    let sw = sheet_w / 9;
    let src_x0 = 7 * sw;

    let mut mirrored = vec![0u8; sw * sh * 4];
    for y in 0..sh {
        for x in 0..sw {
            let sx = src_x0 + (sw - 1 - x);
            let sy = sh - 1 - y;
            let si = (sy * sheet_w + sx) * 4;
            let di = (y * sw + x) * 4;
            mirrored[di..di + 4].copy_from_slice(&pix[si..si + 4]);
        }
    }

    let dis_rgba = make_disabled_rgba(&mirrored);
    let enabled = ctx.load_texture(
        "stoll_icon_curve_reverse_j",
        ColorImage::from_rgba_unmultiplied([sw, sh], &mirrored),
        TextureOptions::NEAREST,
    );
    let disabled = ctx.load_texture(
        "stoll_icon_curve_reverse_j_dis",
        ColorImage::from_rgba_unmultiplied([sw, sh], &dis_rgba),
        TextureOptions::NEAREST,
    );
    let pair = IconAtlasPair {
        enabled,
        disabled,
        w: sw as u32,
        h: sh as u32,
    };
    ctx.data_mut(|d| d.insert_temp(id, pair.clone()));
    pair
}

/// Map an `IconKind` to `(SpriteSheet, sprite_index, sprite_count)` if backed by an original Stoll PE bitmap.
fn icon_sprite(kind: IconKind) -> Option<(SpriteSheet, usize, usize)> {
    use IconKind::*;
    use SpriteSheet::*;
    match kind {
        // Standart (10800 — 11 icons)
        NewFile => Some((Tb10800, 0, 11)),
        OpenShv => Some((Tb10800, 1, 11)),
        OpenShp => Some((Tb10800, 2, 11)),
        OpenShpx => Some((Tb10800, 3, 11)),
        Save => Some((Tb10800, 4, 11)),
        AlwaysOnTop => Some((Tb10800, 5, 11)),
        ToggleGraph => Some((Tb10800, 6, 11)),
        LeftLines => Some((Tb10800, 7, 11)),
        RightLines => Some((Tb10800, 8, 11)),
        LeftMarkers => Some((Tb10800, 9, 11)),
        RightMarkers => Some((Tb10800, 10, 11)),

        // Giriş (10820 — 2 icons)
        SignCartesian => Some((Tb10820, 0, 2)),
        SignKnitting => Some((Tb10820, 1, 2)),

        // Elemanlar (10821 — 4 icons)
        AddElement => Some((Tb10821, 0, 4)),
        DeleteElement => Some((Tb10821, 1, 4)),
        SelectAllElements => Some((Tb10821, 2, 4)),
        NecklineProps => Some((Tb10821, 3, 4)),

        // Çizgiler (10822 — 9 icons)
        DeleteLine => Some((Tb10822, 0, 9)),
        GroupLines => Some((Tb10822, 1, 9)),
        CreateClosingLine => Some((Tb10822, 2, 9)),
        AppendLine => Some((Tb10822, 3, 9)),
        InsertLineBefore => Some((Tb10822, 4, 9)),
        CopyLines => Some((Tb10822, 5, 9)),
        PasteReplaceLines => Some((Tb10822, 6, 9)),
        PasteInsertLines => Some((Tb10822, 7, 9)),
        ImportExternalTable => Some((Tb10822, 8, 9)),

        // İşaretler (10823 — 8 icons)
        DeleteMarker => Some((Tb10823, 0, 8)),
        GroupMarkers => Some((Tb10823, 1, 8)),
        AppendMarker => Some((Tb10823, 2, 8)),
        InsertMarkerBefore => Some((Tb10823, 3, 8)),
        MultiplyMarkers => Some((Tb10823, 4, 8)),
        CopyMarkers => Some((Tb10823, 5, 8)),
        PasteReplaceMarkers => Some((Tb10823, 6, 8)),
        PasteInsertMarkers => Some((Tb10823, 7, 8)),

        // Curve Editor toolbar (formtable.dll 4308 & 4309)
        CurveLoad => Some((Ft4308, 0, 9)),
        CurveSave => Some((Ft4308, 1, 9)),
        CurveDelete => Some((Ft4308, 2, 9)),
        CurveGroup => Some((Ft4308, 3, 9)),
        CurveAppend => Some((Ft4308, 4, 9)),
        CurveInsert => Some((Ft4308, 5, 9)),
        CurveLine => Some((Ft4308, 6, 9)),
        CurveJ => Some((Ft4308, 7, 9)),
        CurveS => Some((Ft4308, 8, 9)),
        CurveCopy => Some((Ft4309, 5, 7)),
        CurvePaste => Some((Ft4309, 6, 7)),

        // Inline UI bitmaps
        HorizontalArrow => Some((Ft4282, 0, 1)),
        VerticalArrow => Some((Ft4283, 0, 1)),
        ElementBadge => Some((Ft4293, 0, 1)),
        StitchLoop => Some((Ft4294, 0, 1)),
        VerticalArrowFromBottom => Some((Ft4301, 0, 1)),
        VerticalArrowFromTop => Some((Ft4302, 0, 1)),

        CurveUser | CurveReverseJ | OpenFile | SaveFile | Pin | PinActive | UpdateAlert => None,
    }
}

fn paint_atlas_sprite(
    painter: &Painter,
    rect: Rect,
    pair: &IconAtlasPair,
    idx: usize,
    count: usize,
    enabled: bool,
    max_scale: f32,
) {
    let tex = if enabled { &pair.enabled } else { &pair.disabled };
    let sprite_w = (pair.w as f32) / (count as f32);
    let sprite_h = pair.h as f32;

    // Preserve exact native pixel aspect ratio without blurry stretching
    let scale = (rect.width() / sprite_w)
        .min(rect.height() / sprite_h)
        .min(max_scale.max(0.1));
    let draw_w = (sprite_w * scale).round().max(1.0);
    let draw_h = (sprite_h * scale).round().max(1.0);
    let c = rect.center();
    let draw_rect = Rect::from_center_size(pos2(c.x.round(), c.y.round()), vec2(draw_w, draw_h));

    let u0 = (idx as f32) / (count as f32);
    let u1 = ((idx + 1) as f32) / (count as f32);
    let uv = Rect::from_min_max(pos2(u0, 0.0), pos2(u1, 1.0));
    painter.image(tex.id(), draw_rect, uv, Color32::WHITE);
}

/// Paint an original Stoll bitmap icon centered cleanly inside `rect` (pixel-exact NEAREST sampling).
pub fn paint_icon(painter: &Painter, rect: Rect, kind: IconKind, enabled: bool) {
    paint_icon_scaled(painter, rect, kind, enabled, 1.0);
}

/// Paint an original Stoll bitmap icon centered cleanly inside `rect`, allowing crisp scaling up to `max_scale`.
pub fn paint_icon_scaled(
    painter: &Painter,
    rect: Rect,
    kind: IconKind,
    enabled: bool,
    max_scale: f32,
) {
    if kind == IconKind::CurveReverseJ {
        let pair = load_reverse_j_pair(painter.ctx());
        paint_atlas_sprite(painter, rect, &pair, 0, 1, enabled, max_scale);
        return;
    }

    if let Some((sheet, idx, count)) = icon_sprite(kind) {
        let pair = load_sheet(painter.ctx(), sheet);
        paint_atlas_sprite(painter, rect, &pair, idx, count, enabled, max_scale);
        return;
    }

    let glyph_col = if enabled {
        colors::TEXT_BLACK
    } else {
        Color32::from_rgb(0x9A, 0x9A, 0x9A)
    };
    match kind {
        IconKind::OpenFile => {
            paint_studio_glyph(painter, rect, StudioGlyph::OpenFile, glyph_col);
            return;
        }
        IconKind::SaveFile => {
            paint_studio_glyph(painter, rect, StudioGlyph::SaveFile, glyph_col);
            return;
        }
        IconKind::Pin => {
            paint_studio_glyph(painter, rect, StudioGlyph::Pin, glyph_col);
            return;
        }
        IconKind::PinActive => {
            let active_col = if enabled { colors::WIN_BLUE } else { glyph_col };
            paint_studio_glyph(painter, rect, StudioGlyph::PinActive, active_col);
            return;
        }
        IconKind::UpdateAlert => {
            let alert_col = if enabled {
                Color32::from_rgb(0xDC, 0x26, 0x26)
            } else {
                glyph_col
            };
            paint_studio_glyph(painter, rect, StudioGlyph::UpdateAlert, alert_col);
            return;
        }
        _ => {}
    }

    // Fallback for `CurveUser` (check-shaped blue curve glyph)
    let c = rect.center();
    let col = if enabled {
        colors::CURVE_GLYPH_BLUE
    } else {
        Color32::from_rgb(0x9A, 0x9A, 0x9A)
    };
    let stroke = Stroke::new(1.8_f32, col);
    let pts = vec![
        pos2(c.x - 4.0, c.y + 1.0),
        pos2(c.x - 1.0, c.y + 4.0),
        pos2(c.x + 4.5, c.y - 4.0),
    ];
    painter.add(egui::Shape::line(pts, stroke));
}

/// Crisp vector UI glyphs painted via `egui::Painter` so no missing Unicode font glyphs (`?`) ever appear.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioGlyph {
    NewFile,
    OpenFile,
    SaveFile,
    Pin,
    PinActive,
    UpdateAlert,
    Undo,
    Redo,
    Mirror,
    Columns,
    Settings,
    Info,
    UpdateArrow,
    Check,
    Swap,
    ChevronUp,
    ChevronDown,
}

/// Paint a crisp 14x14 vector glyph centered in `rect`.
pub fn paint_studio_glyph(painter: &Painter, rect: Rect, glyph: StudioGlyph, color: Color32) {
    let c = rect.center();
    let s = (rect.width().min(rect.height()) / 14.0).clamp(0.75, 1.5);
    let stroke = Stroke::new(1.4 * s, color);
    let thin = Stroke::new(1.15 * s, color);

    match glyph {
        StudioGlyph::NewFile => {
            // Crisp document page with folded top-right corner
            let l = c.x - 4.0 * s;
            let r = c.x + 4.0 * s;
            let t = c.y - 5.0 * s;
            let b = c.y + 5.0 * s;
            let f = 2.8 * s;
            let pts = vec![
                pos2(l, t),
                pos2(r - f, t),
                pos2(r, t + f),
                pos2(r, b),
                pos2(l, b),
            ];
            painter.add(egui::Shape::closed_line(pts, stroke));
            painter.line_segment([pos2(r - f, t), pos2(r - f, t + f)], thin);
            painter.line_segment([pos2(r - f, t + f), pos2(r, t + f)], thin);
        }
        StudioGlyph::OpenFile => {
            // Clean open folder silhouette
            let l = c.x - 5.2 * s;
            let r = c.x + 4.5 * s;
            let t = c.y - 4.2 * s;
            let b = c.y + 4.2 * s;
            let back = vec![
                pos2(l, b),
                pos2(l, t),
                pos2(c.x - 1.2 * s, t),
                pos2(c.x + 0.2 * s, t + 1.6 * s),
                pos2(r, t + 1.6 * s),
                pos2(r, t + 3.0 * s),
            ];
            painter.add(egui::Shape::line(back, stroke));
            let flap = vec![
                pos2(l, b),
                pos2(l + 1.8 * s, c.y - 0.6 * s),
                pos2(r + 1.0 * s, c.y - 0.6 * s),
                pos2(r - 0.6 * s, b),
            ];
            painter.add(egui::Shape::closed_line(flap, stroke));
        }
        StudioGlyph::SaveFile => {
            // Crisp floppy disk outline with top shutter & bottom label
            let l = c.x - 4.8 * s;
            let r = c.x + 4.8 * s;
            let t = c.y - 4.8 * s;
            let b = c.y + 4.8 * s;
            let bev = 2.0 * s;
            let outer = vec![
                pos2(l, t),
                pos2(r - bev, t),
                pos2(r, t + bev),
                pos2(r, b),
                pos2(l, b),
            ];
            painter.add(egui::Shape::closed_line(outer, stroke));
            let shutter = Rect::from_min_max(pos2(l + 2.2 * s, t), pos2(r - 2.6 * s, c.y - 1.2 * s));
            painter.rect_stroke(shutter, 0.5, thin, egui::StrokeKind::Inside);
            let label = Rect::from_min_max(pos2(l + 1.8 * s, c.y + 0.8 * s), pos2(r - 1.8 * s, b));
            painter.rect_stroke(label, 0.5, thin, egui::StrokeKind::Inside);
        }
        StudioGlyph::Pin => {
            // Sharp, unmistakable tilted 45° pushpin (off state: clean outline + angled needle)
            // Top cap -> tapered waist -> wide flange (all connected polygon outline)
            let outline = vec![
                pos2(c.x + 1.0 * s, c.y - 5.2 * s), // cap top-left
                pos2(c.x + 5.2 * s, c.y - 1.0 * s), // cap bottom-right
                pos2(c.x + 4.0 * s, c.y + 0.2 * s), // waist right
                pos2(c.x + 2.4 * s, c.y - 0.6 * s), // neck right
                pos2(c.x + 1.0 * s, c.y + 0.8 * s), // collar inner-right
                pos2(c.x + 2.0 * s, c.y + 2.6 * s), // flange bottom-right
                pos2(c.x - 2.6 * s, c.y - 2.0 * s), // flange top-left
                pos2(c.x - 0.8 * s, c.y - 1.0 * s), // collar inner-left
                pos2(c.x + 0.6 * s, c.y - 2.4 * s), // neck left
                pos2(c.x - 0.2 * s, c.y - 4.0 * s), // waist left
            ];
            painter.add(egui::Shape::closed_line(outline, Stroke::new(1.3 * s, color)));
            // Sharp diagonal needle pointing bottom-left
            painter.line_segment(
                [pos2(c.x - 0.3 * s, c.y + 0.3 * s), pos2(c.x - 4.8 * s, c.y + 4.8 * s)],
                Stroke::new(1.65 * s, color),
            );
        }
        StudioGlyph::PinActive => {
            // Active vertical pinned-down pushpin (bold accent fill + crisp outline + vertical needle)
            let cap = vec![
                pos2(c.x - 3.2 * s, c.y - 5.2 * s),
                pos2(c.x + 3.2 * s, c.y - 5.2 * s),
                pos2(c.x + 2.6 * s, c.y - 3.6 * s),
                pos2(c.x - 2.6 * s, c.y - 3.6 * s),
            ];
            let body = vec![
                pos2(c.x - 2.0 * s, c.y - 3.6 * s),
                pos2(c.x + 2.0 * s, c.y - 3.6 * s),
                pos2(c.x + 2.2 * s, c.y - 0.8 * s),
                pos2(c.x - 2.2 * s, c.y - 0.8 * s),
            ];
            let flange = vec![
                pos2(c.x - 4.4 * s, c.y - 0.8 * s),
                pos2(c.x + 4.4 * s, c.y - 0.8 * s),
                pos2(c.x + 4.4 * s, c.y + 1.0 * s),
                pos2(c.x - 4.4 * s, c.y + 1.0 * s),
            ];
            let border = Stroke::new(0.9 * s, color);
            painter.add(egui::Shape::convex_polygon(cap, color, border));
            painter.add(egui::Shape::convex_polygon(body, color, border));
            painter.add(egui::Shape::convex_polygon(flange, color, border));
            // Vertical needle pinned straight down
            painter.line_segment(
                [pos2(c.x, c.y + 1.0 * s), pos2(c.x, c.y + 5.4 * s)],
                Stroke::new(1.85 * s, color),
            );
        }
        StudioGlyph::UpdateAlert => {
            // Crisp circular badge with upward arrow & notification dot
            painter.circle_stroke(c, 5.2 * s, stroke);
            let top = pos2(c.x, c.y - 2.8 * s);
            let bot = pos2(c.x, c.y + 2.8 * s);
            painter.line_segment([bot, top], stroke);
            painter.line_segment([top, pos2(c.x - 2.4 * s, c.y - 0.4 * s)], stroke);
            painter.line_segment([top, pos2(c.x + 2.4 * s, c.y - 0.4 * s)], stroke);
            painter.circle_filled(pos2(c.x + 4.2 * s, c.y - 4.2 * s), 1.6 * s, color);
        }
        StudioGlyph::Undo => {
            // Crisp left-pointing curved back-arrow arch (↶), open at bottom — never a refresh loop
            let pts = vec![
                pos2(c.x + 4.4 * s, c.y + 3.2 * s),
                pos2(c.x + 3.6 * s, c.y + 0.3 * s),
                pos2(c.x + 1.8 * s, c.y - 1.8 * s),
                pos2(c.x - 0.8 * s, c.y - 2.3 * s),
                pos2(c.x - 3.2 * s, c.y - 1.1 * s),
                pos2(c.x - 4.4 * s, c.y + 0.5 * s),
            ];
            painter.add(egui::Shape::line(pts, stroke));
            let tip = pos2(c.x - 4.6 * s, c.y + 0.7 * s);
            painter.line_segment([tip, pos2(tip.x - 0.3 * s, tip.y - 3.5 * s)], stroke);
            painter.line_segment([tip, pos2(tip.x + 3.5 * s, tip.y - 0.4 * s)], stroke);
        }
        StudioGlyph::Redo => {
            // Exact horizontal mirror of Undo: right-pointing curved forward-arrow arch (↷)
            let pts = vec![
                pos2(c.x - 4.4 * s, c.y + 3.2 * s),
                pos2(c.x - 3.6 * s, c.y + 0.3 * s),
                pos2(c.x - 1.8 * s, c.y - 1.8 * s),
                pos2(c.x + 0.8 * s, c.y - 2.3 * s),
                pos2(c.x + 3.2 * s, c.y - 1.1 * s),
                pos2(c.x + 4.4 * s, c.y + 0.5 * s),
            ];
            painter.add(egui::Shape::line(pts, stroke));
            let tip = pos2(c.x + 4.6 * s, c.y + 0.7 * s);
            painter.line_segment([tip, pos2(tip.x + 0.3 * s, tip.y - 3.5 * s)], stroke);
            painter.line_segment([tip, pos2(tip.x - 3.5 * s, tip.y - 0.4 * s)], stroke);
        }
        StudioGlyph::Mirror => {
            // Vertical center symmetry axis
            for dy in [-4.5_f32, -1.5, 1.5] {
                painter.line_segment(
                    [pos2(c.x, c.y + dy * s), pos2(c.x, c.y + (dy + 2.0) * s)],
                    thin,
                );
            }
            // Left solid triangle + right outlined triangle
            let l_pts = vec![
                pos2(c.x - 1.8 * s, c.y - 3.5 * s),
                pos2(c.x - 5.2 * s, c.y + 3.2 * s),
                pos2(c.x - 1.8 * s, c.y + 3.2 * s),
            ];
            painter.add(egui::Shape::convex_polygon(l_pts, color, Stroke::NONE));
            let r_pts = vec![
                pos2(c.x + 1.8 * s, c.y - 3.5 * s),
                pos2(c.x + 5.2 * s, c.y + 3.2 * s),
                pos2(c.x + 1.8 * s, c.y + 3.2 * s),
            ];
            painter.add(egui::Shape::closed_line(r_pts, thin));
        }
        StudioGlyph::Columns => {
            let box_r = Rect::from_center_size(c, vec2(10.5 * s, 9.0 * s));
            painter.rect_stroke(box_r, 1.0, thin, egui::StrokeKind::Inside);
            painter.line_segment(
                [pos2(box_r.left(), box_r.top() + 3.0 * s), pos2(box_r.right(), box_r.top() + 3.0 * s)],
                thin,
            );
            let x1 = box_r.left() + box_r.width() / 3.0;
            let x2 = box_r.left() + box_r.width() * 2.0 / 3.0;
            painter.line_segment([pos2(x1, box_r.top()), pos2(x1, box_r.bottom())], thin);
            painter.line_segment([pos2(x2, box_r.top()), pos2(x2, box_r.bottom())], thin);
        }
        StudioGlyph::Settings => {
            painter.circle_stroke(c, 2.3 * s, stroke);
            let r_in = 3.6 * s;
            let r_out = 5.3 * s;
            for i in 0..8 {
                let ang = (i as f32) * std::f32::consts::FRAC_PI_4;
                let (sn, cs) = ang.sin_cos();
                painter.line_segment(
                    [pos2(c.x + cs * r_in, c.y + sn * r_in), pos2(c.x + cs * r_out, c.y + sn * r_out)],
                    stroke,
                );
            }
        }
        StudioGlyph::Info => {
            painter.circle_stroke(c, 5.2 * s, thin);
            painter.circle_filled(pos2(c.x, c.y - 2.4 * s), 0.9 * s, color);
            painter.line_segment(
                [pos2(c.x, c.y - 0.5 * s), pos2(c.x, c.y + 3.0 * s)],
                stroke,
            );
        }
        StudioGlyph::UpdateArrow => {
            let top = pos2(c.x, c.y - 4.2 * s);
            let bot = pos2(c.x, c.y + 4.2 * s);
            painter.line_segment([bot, top], stroke);
            painter.line_segment([top, pos2(c.x - 3.2 * s, c.y - 0.8 * s)], stroke);
            painter.line_segment([top, pos2(c.x + 3.2 * s, c.y - 0.8 * s)], stroke);
        }
        StudioGlyph::Check => {
            let p0 = pos2(c.x - 4.2 * s, c.y + 0.3 * s);
            let p1 = pos2(c.x - 1.2 * s, c.y + 3.3 * s);
            let p2 = pos2(c.x + 4.4 * s, c.y - 3.1 * s);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([p1, p2], stroke);
        }
        StudioGlyph::Swap => {
            // Top right-pointing arrow
            let y1 = c.y - 2.2 * s;
            painter.line_segment([pos2(c.x - 4.5 * s, y1), pos2(c.x + 4.2 * s, y1)], thin);
            painter.line_segment(
                [pos2(c.x + 4.2 * s, y1), pos2(c.x + 1.8 * s, y1 - 2.0 * s)],
                thin,
            );
            painter.line_segment(
                [pos2(c.x + 4.2 * s, y1), pos2(c.x + 1.8 * s, y1 + 2.0 * s)],
                thin,
            );
            // Bottom left-pointing arrow
            let y2 = c.y + 2.2 * s;
            painter.line_segment([pos2(c.x - 4.2 * s, y2), pos2(c.x + 4.5 * s, y2)], thin);
            painter.line_segment(
                [pos2(c.x - 4.2 * s, y2), pos2(c.x - 1.8 * s, y2 - 2.0 * s)],
                thin,
            );
            painter.line_segment(
                [pos2(c.x - 4.2 * s, y2), pos2(c.x - 1.8 * s, y2 + 2.0 * s)],
                thin,
            );
        }
        StudioGlyph::ChevronUp => {
            painter.line_segment(
                [pos2(c.x - 3.5 * s, c.y + 1.8 * s), pos2(c.x, c.y - 1.8 * s)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x, c.y - 1.8 * s), pos2(c.x + 3.5 * s, c.y + 1.8 * s)],
                stroke,
            );
        }
        StudioGlyph::ChevronDown => {
            painter.line_segment(
                [pos2(c.x - 3.5 * s, c.y - 1.8 * s), pos2(c.x, c.y + 1.8 * s)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x, c.y + 1.8 * s), pos2(c.x + 3.5 * s, c.y - 1.8 * s)],
                stroke,
            );
        }
    }
}

/// Load and cache the custom 64×64 `Stoll Kalıp Editörü` application icon texture.
pub fn load_app_icon_64(ctx: &egui::Context) -> TextureHandle {
    let key = egui::Id::new("stoll_app_icon_64_tex");
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(key)) {
        return tex;
    }
    const APP_ICON_64_RGBA: &[u8] = include_bytes!("../../assets/app_icon_64.rgba");
    let w = u16::from_le_bytes([APP_ICON_64_RGBA[0], APP_ICON_64_RGBA[1]]) as usize;
    let h = u16::from_le_bytes([APP_ICON_64_RGBA[2], APP_ICON_64_RGBA[3]]) as usize;
    let img = ColorImage::from_rgba_unmultiplied([w, h], &APP_ICON_64_RGBA[4..]);
    let tex = ctx.load_texture("stoll_app_icon_64", img, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(key, tex.clone()));
    tex
}
