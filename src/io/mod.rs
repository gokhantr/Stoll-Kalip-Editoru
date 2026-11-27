pub mod bitmap;
pub mod parser;
pub mod serializer;

use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use thiserror::Error;

use crate::io::bitmap::render_muster_bitmap;
use crate::io::parser::{decode_form_utf16le, parse_form_text};
use crate::io::serializer::{encode_form_utf16le, serialize_form_text};
use crate::model::{FormDocument, OlePreservedStreams};

pub const STREAM_FORM: &str = "/Form";
pub const STREAM_MUSTER_BITMAP: &str = "/MusterBitmap";
pub const STREAM_SUMMARY_INFO: &str = "/\x05SummaryInformation";

#[derive(Debug, Error)]
pub enum IoError {
    #[error("Dosya I/O hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("OLE2/CFB format hatası: {0}")]
    Ole(String),
    #[error("Form stream eksik veya geçersiz: {0}")]
    Parse(String),
}

/// Load a Stoll `.shp` or `.shv` OLE2 Compound File from disk.
pub fn load_file(path: impl AsRef<Path>) -> Result<FormDocument, IoError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    let mut doc = load_from_bytes(&bytes)?;
    doc.file_path = Some(path.to_path_buf());
    doc.dirty = false;
    Ok(doc)
}

/// Load a Stoll `.shp` or `.shv` OLE2 Compound File from an in-memory byte slice.
pub fn load_from_bytes(bytes: &[u8]) -> Result<FormDocument, IoError> {
    let cursor = Cursor::new(bytes);
    let mut comp = cfb::CompoundFile::open(cursor).map_err(|e| IoError::Ole(e.to_string()))?;

    let mut form_bytes: Option<Vec<u8>> = None;
    let mut ole = OlePreservedStreams::default();

    let stream_paths: Vec<String> = comp
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    for sp in stream_paths {
        let mut s = comp
            .open_stream(&sp)
            .map_err(|e| IoError::Ole(format!("{sp}: {e}")))?;
        let mut data = Vec::new();
        s.read_to_end(&mut data)?;

        let norm = sp.replace('\\', "/");
        if norm == "/Form" || norm == "Form" {
            form_bytes = Some(data);
        } else if norm == "/MusterBitmap" || norm == "MusterBitmap" {
            ole.muster_bitmap = Some(data);
        } else if norm == "/\x05SummaryInformation" || norm == "\x05SummaryInformation" {
            ole.summary_info = Some(data);
        } else {
            ole.extra_streams.push((norm, data));
        }
    }

    let raw_form = form_bytes.ok_or_else(|| IoError::Parse("OLE2 dosyasında 'Form' akışı bulunamadı".into()))?;
    let (text, had_bom, trailing_newline) =
        decode_form_utf16le(&raw_form).map_err(IoError::Parse)?;
    ole.form_had_bom = had_bom;
    ole.form_trailing_newline = trailing_newline;

    parse_form_text(&text, ole).map_err(IoError::Parse)
}

/// Save options controlling `MusterBitmap` regeneration vs verbatim preservation.
#[derive(Debug, Clone, Copy)]
pub struct SaveOptions {
    /// If true, regenerate `MusterBitmap` from current shape geometry; if false, preserve original bytes if present.
    pub regenerate_bitmap: bool,
    pub bitmap_max_dim: u32,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            regenerate_bitmap: true,
            bitmap_max_dim: 202,
        }
    }
}

/// Save a `FormDocument` to a `.shp` or `.shv` OLE2 Compound File on disk.
pub fn save_file(doc: &FormDocument, path: impl AsRef<Path>) -> Result<(), IoError> {
    save_file_with_options(doc, path, SaveOptions::default())
}

/// Save a `FormDocument` to disk with custom `SaveOptions`.
pub fn save_file_with_options(
    doc: &FormDocument,
    path: impl AsRef<Path>,
    options: SaveOptions,
) -> Result<(), IoError> {
    let path_ref = path.as_ref();
    let bytes = if doc.header.name.trim().is_empty() {
        let mut named = doc.clone();
        if let Some(stem) = path_ref.file_stem().and_then(|s| s.to_str()) {
            if !stem.is_empty() {
                named.header.name = stem.to_string();
            }
        }
        save_to_bytes_with_options(&named, options)?
    } else {
        save_to_bytes_with_options(doc, options)?
    };
    let mut f = File::create(path_ref)?;
    f.write_all(&bytes)?;
    Ok(())
}

/// Serialize a `FormDocument` to an in-memory OLE2 Compound File byte vector.
pub fn save_to_bytes(doc: &FormDocument) -> Result<Vec<u8>, IoError> {
    save_to_bytes_with_options(doc, SaveOptions::default())
}

/// Serialize a `FormDocument` to an in-memory OLE2 byte vector with custom `SaveOptions`.
pub fn save_to_bytes_with_options(
    doc: &FormDocument,
    options: SaveOptions,
) -> Result<Vec<u8>, IoError> {
    let cursor = Cursor::new(Vec::new());
    let mut comp =
        cfb::CompoundFile::create_with_version(cfb::Version::V3, cursor)
            .map_err(|e| IoError::Ole(e.to_string()))?;

    // 1. Write `MusterBitmap` first so it sits at Root Entry's child in the CFB
    //    directory BST (`Form` on its left, `\x05SummaryInformation` on its right),
    //    matching Stoll's balanced directory tree topology.
    let bitmap_bytes = if options.regenerate_bitmap || doc.ole.muster_bitmap.is_none() {
        render_muster_bitmap(doc, options.bitmap_max_dim)
    } else {
        doc.ole.muster_bitmap.clone().unwrap_or_else(|| vec![0u8; 8])
    };
    {
        let mut s = comp
            .create_stream(STREAM_MUSTER_BITMAP)
            .map_err(|e| IoError::Ole(e.to_string()))?;
        s.write_all(&bitmap_bytes)?;
    }

    // 2. Write `Form` stream (UTF-16LE, exact BOM/CRLF preservation)
    let form_text = serialize_form_text(doc);
    let form_bytes = encode_form_utf16le(&form_text, doc.ole.form_had_bom);
    {
        let mut s = comp
            .create_stream(STREAM_FORM)
            .map_err(|e| IoError::Ole(e.to_string()))?;
        s.write_all(&form_bytes)?;
    }

    // 3. Preserve `\x05SummaryInformation` stream if present
    if let Some(ref summary) = doc.ole.summary_info {
        let mut s = comp
            .create_stream(STREAM_SUMMARY_INFO)
            .map_err(|e| IoError::Ole(e.to_string()))?;
        s.write_all(summary)?;
    }

    // 4. Preserve any additional unknown streams verbatim (creating parent storages if nested)
    for (sp, data) in &doc.ole.extra_streams {
        let path_obj = std::path::Path::new(sp);
        if let Some(parent) = path_obj.parent() {
            let p_str = parent.to_string_lossy();
            if !p_str.is_empty() && p_str != "/" && p_str != "\\" {
                let _ = comp.create_storage_all(parent);
            }
        }
        let mut s = comp
            .create_stream(sp)
            .map_err(|e| IoError::Ole(e.to_string()))?;
        s.write_all(data)?;
    }

    comp.flush().map_err(|e| IoError::Ole(e.to_string()))?;
    let mut bytes = comp.into_inner().into_inner();

    // MS-CFB Section 2.2: "If Major Version is 3, the Number of Directory Sectors MUST be zero."
    // `cfb` 0.7 hardcodes `num_dir_sectors = 1` in `create_with_version`, which causes Windows
    // `StgOpenStorage` / MFC (`vecformeditor.exe`) to reject the file with `STG_E_DOCFILECORRUPT`
    // (`0x80030109` / "Belge dosyası bozuk.").
    if bytes.len() >= 512 && u16::from_le_bytes([bytes[26], bytes[27]]) == 3 {
        bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
    }

    Ok(bytes)
}

/// Serialize a standalone `[Kurve]` + `[KurvenLinie]` curve into an OLE2 CFB v3 `.FormCurve` byte vector
/// containing a single UTF-16LE (no BOM) + CRLF `Form` stream matching Stoll M1plus 1:1.
pub fn save_form_curve_to_bytes(curve: &crate::model::CurveData) -> Result<Vec<u8>, IoError> {
    let cursor = Cursor::new(Vec::new());
    let mut comp = cfb::CompoundFile::create_with_version(cfb::Version::V3, cursor)
        .map_err(|e| IoError::Ole(e.to_string()))?;

    let text = crate::io::serializer::serialize_curve_text(curve);
    let form_bytes = encode_form_utf16le(&text, false);
    {
        let mut s = comp
            .create_stream(STREAM_FORM)
            .map_err(|e| IoError::Ole(e.to_string()))?;
        s.write_all(&form_bytes)?;
    }

    comp.flush().map_err(|e| IoError::Ole(e.to_string()))?;
    let mut bytes = comp.into_inner().into_inner();
    if bytes.len() >= 512 && u16::from_le_bytes([bytes[26], bytes[27]]) == 3 {
        bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
    }
    Ok(bytes)
}

/// Save a standalone curve (`[Kurve]` + `[KurvenLinie]` blocks) to a Stoll `.FormCurve` OLE2 CFB v3 file.
pub fn save_form_curve(path: impl AsRef<Path>, curve: &crate::model::CurveData) -> Result<(), IoError> {
    let bytes = save_form_curve_to_bytes(curve)?;
    let mut f = File::create(path.as_ref())?;
    f.write_all(&bytes)?;
    Ok(())
}

/// Load a standalone Stoll `.FormCurve` OLE2 CFB v3 file from disk into `CurveData`.
pub fn load_form_curve(path: impl AsRef<Path>) -> Result<crate::model::CurveData, IoError> {
    let bytes = std::fs::read(path.as_ref())?;
    load_form_curve_from_bytes(&bytes)
}

/// Load a standalone Stoll `.FormCurve` OLE2 CFB v3 file from an in-memory byte slice into `CurveData`.
pub fn load_form_curve_from_bytes(bytes: &[u8]) -> Result<crate::model::CurveData, IoError> {
    let cursor = Cursor::new(bytes);
    let mut comp = cfb::CompoundFile::open(cursor).map_err(|e| IoError::Ole(e.to_string()))?;

    let mut form_bytes: Option<Vec<u8>> = None;
    let stream_paths: Vec<String> = comp
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    for sp in stream_paths {
        let norm = sp.replace('\\', "/");
        if norm == "/Form" || norm == "Form" {
            let mut s = comp
                .open_stream(&sp)
                .map_err(|e| IoError::Ole(format!("{sp}: {e}")))?;
            let mut data = Vec::new();
            s.read_to_end(&mut data)?;
            form_bytes = Some(data);
            break;
        }
    }

    let raw_form = form_bytes
        .ok_or_else(|| IoError::Parse("FormCurve dosyasında 'Form' akışı bulunamadı.".into()))?;
    let (text, _had_bom, _trailing_nl) =
        decode_form_utf16le(&raw_form).map_err(IoError::Parse)?;

    // Wrap into a minimal synthetic Form stream and reuse the lossless parser
    let wrapped = format!(
        "[Form] = 0\r\nName = CurveImport\r\n[Element] = 1\r\n[LinienLinks]\r\n[Kante] = 1\r\n{text}\r\n"
    );
    let doc = parse_form_text(&wrapped, OlePreservedStreams::default()).map_err(IoError::Parse)?;
    let curve = doc
        .elements
        .into_iter()
        .next()
        .and_then(|e| e.left_lines.into_iter().next())
        .and_then(|l| l.curve)
        .ok_or_else(|| {
            IoError::Parse("FormCurve dosyasında [Kurve] / [KurvenLinie] bölümü bulunamadı.".into())
        })?;
    if curve.steps.is_empty() {
        return Err(IoError::Parse("FormCurve dosyası hiç basamak içermiyor.".into()));
    }
    Ok(curve)
}

/// Import a lightweight ASCII `.dxf` (`LWPOLYLINE` / `LINE` entities) into a `FormDocument` in `InputFormat::Mm`.
pub fn import_dxf_file(path: impl AsRef<Path>) -> Result<FormDocument, IoError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("DXF_Kalip");
    let mut doc = parse_dxf_contour(&text, stem)?;
    doc.file_path = None;
    doc.dirty = true;
    Ok(doc)
}

/// Parse ASCII DXF `LWPOLYLINE` / `LINE` contour vertices into a `FormDocument` (`InputFormat::Mm`).
pub fn parse_dxf_contour(text: &str, name: &str) -> Result<FormDocument, IoError> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    let mut pairs: Vec<(i32, &str)> = Vec::with_capacity(lines.len() / 2);
    let mut i = 0usize;
    while i + 1 < lines.len() {
        if let Ok(code) = lines[i].parse::<i32>() {
            pairs.push((code, lines[i + 1]));
            i += 2;
        } else {
            i += 1;
        }
    }

    let mut vertices: Vec<(f64, f64)> = Vec::new();
    let mut idx = 0usize;
    while idx < pairs.len() {
        if pairs[idx].0 == 0 && pairs[idx].1.eq_ignore_ascii_case("LWPOLYLINE") {
            let mut poly: Vec<(f64, f64)> = Vec::new();
            let mut cur_x: Option<f64> = None;
            let mut closed_poly = false;
            idx += 1;
            while idx < pairs.len() && pairs[idx].0 != 0 {
                match pairs[idx].0 {
                    70 => {
                        if let Ok(flags) = pairs[idx].1.parse::<i32>() {
                            closed_poly = (flags & 1) != 0;
                        }
                    }
                    10 => {
                        cur_x = pairs[idx]
                            .1
                            .parse::<f64>()
                            .ok()
                            .filter(|v| v.is_finite());
                    }
                    20 => {
                        if let (Some(x), Ok(y)) = (cur_x.take(), pairs[idx].1.parse::<f64>()) {
                            if y.is_finite() {
                                poly.push((x, y));
                            }
                        }
                    }
                    _ => {}
                }
                idx += 1;
            }
            if closed_poly && poly.len() >= 3 {
                let first = poly[0];
                let last = *poly.last().unwrap();
                if (first.0 - last.0).abs() > 1e-4 || (first.1 - last.1).abs() > 1e-4 {
                    poly.push(first);
                }
            }
            if poly.len() > vertices.len() {
                vertices = poly;
            }
            continue;
        }
        idx += 1;
    }

    // Fallback: collect and chain matching LINE entities if no LWPOLYLINE was found
    if vertices.len() < 2 {
        let mut segs: Vec<((f64, f64), (f64, f64))> = Vec::new();
        idx = 0;
        while idx < pairs.len() {
            if pairs[idx].0 == 0 && pairs[idx].1.eq_ignore_ascii_case("LINE") {
                let (mut x0, mut y0, mut x1, mut y1) = (None, None, None, None);
                idx += 1;
                while idx < pairs.len() && pairs[idx].0 != 0 {
                    match pairs[idx].0 {
                        10 => x0 = pairs[idx].1.parse::<f64>().ok().filter(|v| v.is_finite()),
                        20 => y0 = pairs[idx].1.parse::<f64>().ok().filter(|v| v.is_finite()),
                        11 => x1 = pairs[idx].1.parse::<f64>().ok().filter(|v| v.is_finite()),
                        21 => y1 = pairs[idx].1.parse::<f64>().ok().filter(|v| v.is_finite()),
                        _ => {}
                    }
                    idx += 1;
                }
                if let (Some(ax), Some(ay), Some(bx), Some(by)) = (x0, y0, x1, y1) {
                    if (bx - ax).abs() >= 1e-4 || (by - ay).abs() >= 1e-4 {
                        segs.push(((ax, ay), (bx, by)));
                    }
                }
                continue;
            }
            idx += 1;
        }
        if !segs.is_empty() {
            let mut used = vec![false; segs.len()];
            let start_i = segs
                .iter()
                .position(|&(a, _)| a.0.abs() < 1e-2 && a.1.abs() < 1e-2)
                .unwrap_or(0);
            used[start_i] = true;
            let (a0, mut cur_end) = segs[start_i];
            let mut line_pts = vec![a0, cur_end];
            loop {
                let mut found = None;
                for (si, &(sa, sb)) in segs.iter().enumerate() {
                    if used[si] {
                        continue;
                    }
                    if (sa.0 - cur_end.0).abs() < 1e-2 && (sa.1 - cur_end.1).abs() < 1e-2 {
                        found = Some((si, sb));
                        break;
                    }
                    if (sb.0 - cur_end.0).abs() < 1e-2 && (sb.1 - cur_end.1).abs() < 1e-2 {
                        found = Some((si, sa));
                        break;
                    }
                }
                match found {
                    Some((si, next_pt)) => {
                        used[si] = true;
                        cur_end = next_pt;
                        line_pts.push(next_pt);
                    }
                    None => break,
                }
            }
            vertices = line_pts;
        }
    }

    if vertices.len() < 2 {
        return Err(IoError::Parse(
            "DXF dosyasında geçerli LWPOLYLINE veya LINE konturu bulunamadı.".into(),
        ));
    }

    let mut doc = FormDocument::new_default_template();
    doc.header.name = name.to_string();
    doc.header.input_format = crate::model::InputFormat::Mm;
    doc.header.display_unit = crate::model::DisplayUnit::Mm;

    let hdr = doc.header.clone();
    let mut kantes = Vec::new();
    for w in vertices.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        if dx.abs() < 1e-4 && dy.abs() < 1e-4 {
            continue;
        }
        let is_first_seg = kantes.is_empty();
        let tag_id = doc.next_line_tag_id();
        let mut seg = crate::model::LineSegment {
            index: (kantes.len() + 1) as i32,
            tag_id,
            width_mm: crate::math::units::round2(dx),
            height_mm: crate::math::units::round2(dy),
            ..Default::default()
        };
        crate::math::units::recalc_line_from_mm(&mut seg, &hdr);
        crate::math::units::auto_detect_line_function(&mut seg, is_first_seg, true);
        kantes.push(seg);
    }

    if kantes.is_empty() {
        return Err(IoError::Parse(
            "DXF konturundan sıfır olmayan çizgi segmenti üretilemedi.".into(),
        ));
    }
    if let Some(last) = kantes.last_mut() {
        if last.rows == 0 && last.function != crate::model::LineFunction::BaseLine {
            last.last_line = 0;
            last.default_widening = 1;
        }
    }

    // Check whether the imported DXF contour is a full closed polygon vs a half-shape.
    // A closed polygon (first == last vertex) must NOT be mirrored onto itself.
    let is_closed_polygon = vertices.len() >= 4
        && (vertices.first().unwrap().0 - vertices.last().unwrap().0).abs() < 1e-3
        && (vertices.first().unwrap().1 - vertices.last().unwrap().1).abs() < 1e-3;

    let el = &mut doc.elements[0];
    el.left_lines = kantes;
    el.right_lines.clear();
    if is_closed_polygon {
        el.mirrored = false;
        el.has_right_lines_section = false;
        el.renumber_lines(crate::model::LineSide::Left);
    } else {
        el.mirrored = true;
        el.renumber_lines(crate::model::LineSide::Left);
        crate::math::operations::sync_mirrored_right_lines(&mut doc, 0);
    }
    doc.dirty = true;
    Ok(doc)
}
