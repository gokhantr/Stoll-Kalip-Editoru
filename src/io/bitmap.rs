use crate::math::geometry::{evaluate_document_geometry, Vec2};
use crate::model::{ElementType, FormDocument};

/// Decoded top-down RGBA image from Stoll's `MusterBitmap` stream (or `M3_UTL_Bitmap`).
#[derive(Debug, Clone)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    /// Row-major top-to-bottom RGBA8 pixels (`width * height * 4`).
    pub rgba: Vec<u8>,
}

/// Decode Stoll's `MusterBitmap` stream (`[u32 width][u32 height][w*h*4 bottom-up BGRX]`)
/// into a top-down RGBA8 `PreviewImage` ready for `egui`.
pub fn decode_muster_bitmap(bytes: &[u8]) -> Option<PreviewImage> {
    if bytes.len() < 8 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return None;
    }
    let expected =
        8usize.checked_add((width as usize).checked_mul(height as usize)?.checked_mul(4)?)?;
    if bytes.len() < expected {
        return None;
    }

    let w = width as usize;
    let h = height as usize;
    let mut rgba = vec![0u8; w * h * 4];
    let src = &bytes[8..expected];

    // Stoll stores BOTTOM-UP BGRX: src row 0 is bottom -> dst row (h - 1)
    for dst_y in 0..h {
        let src_y = h - 1 - dst_y;
        let src_row = &src[src_y * w * 4..(src_y + 1) * w * 4];
        let dst_row = &mut rgba[dst_y * w * 4..(dst_y + 1) * w * 4];
        for x in 0..w {
            let si = x * 4;
            dst_row[si] = src_row[si + 2];     // R
            dst_row[si + 1] = src_row[si + 1]; // G
            dst_row[si + 2] = src_row[si];     // B
            dst_row[si + 3] = 255;             // A
        }
    }

    Some(PreviewImage { width, height, rgba })
}

/// Render the current `FormDocument` shape geometry into a valid Stoll `MusterBitmap` stream
/// (`[u32 width][u32 height=202][w*h*4 bottom-up BGRX]`), matching Stoll's exact preview palette:
/// - Background: slate blue `#78A0C8` -> `BGRX(200, 160, 120, 0)`
/// - Shape contour outline: lime green `#11FF7C` -> `BGRX(124, 255, 17, 0)`
/// - Center symmetry axis (dash-dot): white `#FFFFFF` -> `BGRX(255, 255, 255, 0)`
pub fn render_muster_bitmap(doc: &FormDocument, target_height: u32) -> Vec<u8> {
    let geom = evaluate_document_geometry(doc);
    let (min_x, max_x, min_y, max_y) = geom.bounds;
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);

    // Stoll uses fixed height = 202 and proportional width
    let height = target_height.clamp(32, 256);
    let aspect = span_x / span_y;
    let width = ((height as f64 * aspect).round() as u32).clamp(32, 256);

    let bg: [u8; 4] = [200, 160, 120, 0];
    let green: [u8; 4] = [124, 255, 17, 0];
    let white: [u8; 4] = [255, 255, 255, 0];

    let mut buf = Vec::with_capacity(8 + (width * height * 4) as usize);
    buf.extend_from_slice(&width.to_le_bytes());
    buf.extend_from_slice(&height.to_le_bytes());
    for _ in 0..(width * height) {
        buf.extend_from_slice(&bg);
    }

    let pad = 6.0;
    let avail_w = (width as f64 - 2.0 * pad).max(1.0);
    let avail_h = (height as f64 - 2.0 * pad).max(1.0);
    let scale = (avail_w / span_x).min(avail_h / span_y);
    let ox = pad + (avail_w - span_x * scale) * 0.5;
    let oy = pad + (avail_h - span_y * scale) * 0.5;

    // Bottom-up pixel mapping: y=0 is bottom (hem), y=height-1 is top (shoulder)
    let to_px = |p: Vec2| -> (i32, i32) {
        let px = (ox + (p.x - min_x) * scale).round() as i32;
        let py = (oy + (p.y - min_y) * scale).round() as i32;
        (px, py)
    };

    // 1. Draw vertical dash-dot center axis at x = 0 in white
    let (axis_x, _) = to_px(Vec2::new(0.0, 0.0));
    if axis_x >= 0 && axis_x < width as i32 {
        for py in 0..height as i32 {
            // Dash-dot pattern: 6 on, 3 off, 1 on, 3 off (period 13)
            let m = py % 13;
            if m < 6 || m == 9 {
                let idx = 8 + ((py * width as i32 + axis_x) * 4) as usize;
                buf[idx..idx + 4].copy_from_slice(&white);
            }
        }
    }

    // 2. Draw shape contours
    for el_geom in &geom.elements {
        if el_geom.hidden {
            continue;
        }
        let color = if el_geom.element_type == ElementType::BasicShape {
            green
        } else {
            white
        };
        for poly in [&el_geom.left_polyline, &el_geom.right_polyline] {
            for w in poly.windows(2) {
                let (x0, y0) = to_px(w[0]);
                let (x1, y1) = to_px(w[1]);
                draw_line_bgrx(&mut buf[8..], width as i32, height as i32, x0, y0, x1, y1, color);
            }
        }
    }

    buf
}

fn draw_line_bgrx(
    pixels: &mut [u8],
    w: i32,
    h: i32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < w && y0 >= 0 && y0 < h {
            let idx = ((y0 * w + x0) * 4) as usize;
            pixels[idx..idx + 4].copy_from_slice(&color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}
