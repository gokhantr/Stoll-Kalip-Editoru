use crate::math::curves::{curve_steps_sum, expand_curve_steps};
use crate::math::units::{rows_to_mm, stitches_to_mm};
use crate::model::{
    ElementType, FormDocument, InputFormat, LineSegment, LineSide, ShapeElement,
};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Evaluated geometry and start/end/delta summary (`Başlama / Bitiş / Fark`) for one line segment.
#[derive(Debug, Clone)]
pub struct SegmentSpan {
    pub line_index: usize,
    pub side: LineSide,
    pub start_mm: Vec2,
    pub end_mm: Vec2,
    pub delta_mm: Vec2,
    pub start_st: (i32, i32),
    pub end_st: (i32, i32),
    pub delta_st: (i32, i32),
    /// Polyline vertices in mm (for straight lines: 2 points; for curves: expanded stair/polyline points).
    pub polyline_mm: Vec<Vec2>,
    /// Polyline vertices in stitches/rows.
    pub polyline_st: Vec<Vec2>,
}

/// Evaluated 2D geometry for a single `ShapeElement`.
#[derive(Debug, Clone)]
pub struct ElementGeometry {
    pub element_index: usize,
    pub name: String,
    pub element_type: ElementType,
    pub hidden: bool,
    pub mirrored: bool,
    pub origin_mm: Vec2,
    pub origin_st: (i32, i32),
    pub left_segments: Vec<SegmentSpan>,
    pub right_segments: Vec<SegmentSpan>,
    pub left_polyline: Vec<Vec2>,
    pub right_polyline: Vec<Vec2>,
    pub total_height_rows: i32,
    pub total_height_mm: f64,
    pub max_width_stitches: i32,
    pub max_width_mm: f64,
    pub bounds: (f64, f64, f64, f64),
}

/// Evaluated 2D geometry for the entire `FormDocument`.
#[derive(Debug, Clone)]
pub struct DocumentGeometry {
    pub elements: Vec<ElementGeometry>,
    /// `(min_x, max_x, min_y, max_y)` in mm across all visible elements.
    pub bounds: (f64, f64, f64, f64),
}

/// Evaluate exact 2D geometry for every element in a `FormDocument`.
pub fn evaluate_document_geometry(doc: &FormDocument) -> DocumentGeometry {
    let mut evals: Vec<ElementGeometry> = Vec::with_capacity(doc.elements.len());

    // First pass: find basic shape height (in rows & mm) for `AbstandYBezugslinie = 2` (EndLine).
    let (basic_height_rows, basic_height_mm) = doc
        .basic_element()
        .map(|el| {
            let rows: i32 = el.left_lines.iter().map(|l| l.rows).sum();
            let mm: f64 = if doc.header.input_format == InputFormat::Mm {
                el.left_lines.iter().map(|l| l.height_mm).sum()
            } else {
                rows_to_mm(rows, &doc.header)
            };
            (rows, mm)
        })
        .unwrap_or((0, 0.0));

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (idx, el) in doc.elements.iter().enumerate() {
        let eg = evaluate_element_geometry(doc, el, idx, basic_height_rows, basic_height_mm);
        if !eg.hidden && eg.bounds.0.is_finite() {
            min_x = min_x.min(eg.bounds.0);
            max_x = max_x.max(eg.bounds.1);
            min_y = min_y.min(eg.bounds.2);
            max_y = max_y.max(eg.bounds.3);
        }
        evals.push(eg);
    }

    if !min_x.is_finite() {
        min_x = -250.0;
        max_x = 250.0;
        min_y = 0.0;
        max_y = 500.0;
    }

    DocumentGeometry {
        elements: evals,
        bounds: (min_x, max_x, min_y, max_y),
    }
}

fn evaluate_element_geometry(
    doc: &FormDocument,
    el: &ShapeElement,
    element_index: usize,
    basic_height_rows: i32,
    basic_height_mm: f64,
) -> ElementGeometry {
    let hdr = &doc.header;
    let use_mm = hdr.input_format == InputFormat::Mm;

    let el_height_rows: i32 = el.left_lines.iter().map(|l| l.rows).sum();
    let el_height_mm: f64 = if use_mm {
        el.left_lines.iter().map(|l| l.height_mm).sum()
    } else {
        rows_to_mm(el_height_rows, hdr)
    };

    // `AbstandXMaschenBasis` / `AbstandXmmBasis` (`Ana kesimin orta ekseni`) shifts the whole element
    // relative to X=0, while `AbstandXMaschen` / `AbstandXmm` (`Orta eksen`) separates Left (-offset)
    // and Right (+offset) symmetrically around that axis.
    let base_x_st = el.offset_x_stitches_base.round() as i32;
    let base_x_mm = if use_mm {
        el.offset_x_mm_base
    } else {
        stitches_to_mm(base_x_st, hdr)
    };
    let axis_x_st = el.offset_x_stitches.round() as i32;
    let axis_x_mm = if use_mm {
        el.offset_x_mm
    } else {
        stitches_to_mm(axis_x_st, hdr)
    };

    let origin_x_st = base_x_st;
    let origin_x_mm = base_x_mm;

    // Per MATH_AND_CURVES_SPEC.md Section 6:
    // AbstandYMaschen / AbstandYmm ALWAYS stores y_bottom measured upward from Y=0.
    // (When AbstandYBezugslinie == 2 / Bitiş çizgisi, the UI displays dist_from_top = H_base - (y_bottom + H_elem).)
    let origin_y_st = el.offset_y_rows.round() as i32;
    let origin_y_mm = if use_mm {
        el.offset_y_mm
    } else {
        rows_to_mm(origin_y_st, hdr)
    };
    let _ = (basic_height_rows, basic_height_mm);

    let half_gap_st = el.distance_halves / 2;
    let half_gap_mm = if hdr.density_x.abs() > 1e-9 {
        ((el.distance_halves as f64) * 0.5) * hdr.density_base_mm / hdr.density_x
    } else {
        0.0
    };

    let left_start_st = (base_x_st - axis_x_st - half_gap_st, origin_y_st);
    let left_start_mm = Vec2::new(base_x_mm - axis_x_mm - half_gap_mm, origin_y_mm);

    let (left_segments, left_polyline) =
        trace_side(doc, &el.left_lines, LineSide::Left, left_start_st, left_start_mm);

    let (right_segments, right_polyline) = if el.mirrored && el.right_lines.is_empty() {
        // Pure mirror of left polyline across `base_x` (`Ana kesimin orta ekseni`)
        let mirror_x_mm = |x: f64| 2.0 * base_x_mm - x;
        let mirror_x_st = |x: i32| 2 * base_x_st - x;

        let r_segs: Vec<SegmentSpan> = left_segments
            .iter()
            .map(|s| SegmentSpan {
                line_index: s.line_index,
                side: LineSide::Right,
                start_mm: Vec2::new(mirror_x_mm(s.start_mm.x), s.start_mm.y),
                end_mm: Vec2::new(mirror_x_mm(s.end_mm.x), s.end_mm.y),
                delta_mm: Vec2::new(-s.delta_mm.x, s.delta_mm.y),
                start_st: (mirror_x_st(s.start_st.0), s.start_st.1),
                end_st: (mirror_x_st(s.end_st.0), s.end_st.1),
                delta_st: (-s.delta_st.0, s.delta_st.1),
                polyline_mm: s
                    .polyline_mm
                    .iter()
                    .map(|p| Vec2::new(mirror_x_mm(p.x), p.y))
                    .collect(),
                polyline_st: s
                    .polyline_st
                    .iter()
                    .map(|p| Vec2::new(2.0 * (base_x_st as f64) - p.x, p.y))
                    .collect(),
            })
            .collect();
        let r_poly: Vec<Vec2> = left_polyline
            .iter()
            .map(|p| Vec2::new(mirror_x_mm(p.x), p.y))
            .collect();
        (r_segs, r_poly)
    } else if !el.right_lines.is_empty() {
        let right_start_st = (base_x_st + axis_x_st + half_gap_st, origin_y_st);
        let right_start_mm = Vec2::new(base_x_mm + axis_x_mm + half_gap_mm, origin_y_mm);
        trace_side(doc, &el.right_lines, LineSide::Right, right_start_st, right_start_mm)
    } else {
        (Vec::new(), Vec::new())
    };

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for p in left_polyline.iter().chain(right_polyline.iter()) {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }

    let max_width_mm = if min_x.is_finite() { max_x - min_x } else { 0.0 };
    let max_width_stitches = {
        let mut min_sx = i32::MAX;
        let mut max_sx = i32::MIN;
        for s in left_segments.iter().chain(right_segments.iter()) {
            min_sx = min_sx.min(s.start_st.0).min(s.end_st.0);
            max_sx = max_sx.max(s.start_st.0).max(s.end_st.0);
        }
        if min_sx <= max_sx { max_sx - min_sx } else { 0 }
    };

    ElementGeometry {
        element_index,
        name: el.name.clone(),
        element_type: el.element_type(),
        hidden: el.hidden,
        mirrored: el.mirrored,
        origin_mm: Vec2::new(origin_x_mm, origin_y_mm),
        origin_st: (origin_x_st, origin_y_st),
        left_segments,
        right_segments,
        left_polyline,
        right_polyline,
        total_height_rows: el_height_rows,
        total_height_mm: el_height_mm,
        max_width_stitches,
        max_width_mm,
        bounds: (min_x, max_x, min_y, max_y),
    }
}

fn trace_side(
    doc: &FormDocument,
    lines: &[LineSegment],
    side: LineSide,
    start_st: (i32, i32),
    start_mm: Vec2,
) -> (Vec<SegmentSpan>, Vec<Vec2>) {
    let hdr = &doc.header;
    let use_mm = hdr.input_format == InputFormat::Mm;

    let mut cur_st = start_st;
    let mut cur_mm = start_mm;
    let mut segments = Vec::with_capacity(lines.len());
    let mut full_poly = vec![cur_mm];

    for (idx, line) in lines.iter().enumerate() {
        let seg_start_st = cur_st;
        let seg_start_mm = cur_mm;

        let dx_st = line.stitches;
        let dy_st = line.rows;
        let dx_mm = if use_mm {
            line.width_mm
        } else {
            stitches_to_mm(dx_st, hdr)
        };
        let dy_mm = if use_mm {
            line.height_mm
        } else {
            rows_to_mm(dy_st, hdr)
        };

        let mut poly_st = vec![Vec2::new(cur_st.0 as f64, cur_st.1 as f64)];
        let mut poly_mm = vec![cur_mm];

        if let Some(ref curve) = line.curve {
            if !curve.steps.is_empty() {
                let mut sx = cur_st.0;
                let mut sy = cur_st.1;
                // Expand each step * factor and GruppenTagId/GruppenFaktor blocks to draw crisp Stoll curve contour
                for (sw, sh) in expand_curve_steps(&curve.steps) {
                    if sh != 0 {
                        sy += sh;
                        let pm = Vec2::new(
                            seg_start_mm.x + stitches_to_mm(sx - seg_start_st.0, hdr),
                            seg_start_mm.y + rows_to_mm(sy - seg_start_st.1, hdr),
                        );
                        poly_st.push(Vec2::new(sx as f64, sy as f64));
                        poly_mm.push(pm);
                        full_poly.push(pm);
                    }
                    if sw != 0 {
                        sx += sw;
                        let pm = Vec2::new(
                            seg_start_mm.x + stitches_to_mm(sx - seg_start_st.0, hdr),
                            seg_start_mm.y + rows_to_mm(sy - seg_start_st.1, hdr),
                        );
                        poly_st.push(Vec2::new(sx as f64, sy as f64));
                        poly_mm.push(pm);
                        full_poly.push(pm);
                    }
                }
            }
        }

        cur_st = (seg_start_st.0 + dx_st, seg_start_st.1 + dy_st);
        cur_mm = Vec2::new(seg_start_mm.x + dx_mm, seg_start_mm.y + dy_mm);

        if poly_mm.last().copied() != Some(cur_mm) {
            poly_st.push(Vec2::new(cur_st.0 as f64, cur_st.1 as f64));
            poly_mm.push(cur_mm);
            full_poly.push(cur_mm);
        }

        segments.push(SegmentSpan {
            line_index: idx,
            side,
            start_mm: seg_start_mm,
            end_mm: cur_mm,
            delta_mm: Vec2::new(dx_mm, dy_mm),
            start_st: seg_start_st,
            end_st: cur_st,
            delta_st: (dx_st, dy_st),
            polyline_mm: poly_mm,
            polyline_st: poly_st,
        });
    }

    (segments, full_poly)
}

/// Severity of a shape validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub element_index: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error)
    }
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Error)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == ValidationSeverity::Warning)
    }
}

/// Validate a `FormDocument` against Stoll M1plus integrity & Kural 54 rules.
pub fn validate_document(doc: &FormDocument) -> ValidationReport {
    let mut report = ValidationReport::default();

    let el1_height: i32 = doc
        .basic_element()
        .map(|e| e.left_lines.iter().map(|l| l.rows).sum())
        .unwrap_or(0);

    for (ei, el) in doc.elements.iter().enumerate() {
        let h_left: i32 = el.left_lines.iter().map(|l| l.rows).sum();
        let h_right: i32 = el.right_lines.iter().map(|l| l.rows).sum();

        // 1. Left-Right height symmetry
        if !el.left_lines.is_empty() && !el.right_lines.is_empty() && h_left != h_right {
            report.issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                element_index: ei,
                message: format!(
                    "Eleman {} ({}): Boy asimetrisi — Sol={} sıra != Sağ={} sıra (Fark: {})",
                    el.index,
                    el.name,
                    h_left,
                    h_right,
                    h_left - h_right
                ),
            });
        }

        // 2. Closed contour X check
        let sum_x_left: i32 = el.left_lines.iter().map(|l| l.stitches).sum();
        let sum_x_right: i32 = el.right_lines.iter().map(|l| l.stitches).sum();
        if el.mirrored {
            if sum_x_left != 0 && !el.left_lines.is_empty() {
                report.issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    element_index: ei,
                    message: format!(
                        "Eleman {} ({}): Sol kontur eksene kapanmıyor (Toplam dX={})",
                        el.index, el.name, sum_x_left
                    ),
                });
            }
        } else if !el.left_lines.is_empty() && !el.right_lines.is_empty() {
            if sum_x_left + sum_x_right != 0 {
                report.issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    element_index: ei,
                    message: format!(
                        "Eleman {} ({}): Yatay poligon açık (Sol dX={} + Sağ dX={} = {})",
                        el.index,
                        el.name,
                        sum_x_left,
                        sum_x_right,
                        sum_x_left + sum_x_right
                    ),
                });
            }
        } else if !el.left_lines.is_empty() && sum_x_left != 0 {
            report.issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                element_index: ei,
                message: format!(
                    "Eleman {} ({}): Sol kontur kapanmıyor (Toplam dX={})",
                    el.index, el.name, sum_x_left
                ),
            });
        }

        // 3. Element 2+ upper/lower bound check (Error 0x3557: 0 <= y_bottom && y_bottom + H_elem <= H_base)
        if ei >= 1 && el1_height > 0 {
            let y_off = el.offset_y_rows.round() as i32;
            if y_off < 0 || (y_off + h_left) > el1_height {
                report.issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    element_index: ei,
                    message: format!(
                        "Eleman {} ({}): Üst sınır taşması (Hata 0x3557) — y={} + boy={} > Ana kesim boyu {}",
                        el.index,
                        el.name,
                        y_off,
                        h_left,
                        el1_height
                    ),
                });
            }
        }

        // 4. Per-line curve StepsSum == Header and Faktor >= 1 check
        for (side_name, lines) in [("Sol", &el.left_lines), ("Sağ", &el.right_lines)] {
            for line in lines {
                if let Some(ref curve) = line.curve {
                    if !curve.steps.is_empty() {
                        for st in &curve.steps {
                            if st.factor <= 0 {
                                report.issues.push(ValidationIssue {
                                    severity: ValidationSeverity::Error,
                                    element_index: ei,
                                    message: format!(
                                        "Eleman {} {} Satır {} Basamak {}: Geçersiz Faktör ({} <= 0)",
                                        el.index, side_name, line.index, st.index, st.factor
                                    ),
                                });
                            }
                        }
                        let (sdx, sdy) = curve_steps_sum(curve);
                        if sdx != line.stitches || sdy != line.rows {
                            report.issues.push(ValidationIssue {
                                severity: ValidationSeverity::Error,
                                element_index: ei,
                                message: format!(
                                    "Eleman {} {} Satır {}: Başlık (N={}, R={}) != Kavis basamak toplamı (N={}, R={})",
                                    el.index, side_name, line.index, line.stitches, line.rows, sdx, sdy
                                ),
                            });
                        }
                    }
                }
            }
        }

        // 5. Even-row warning for BasicShape
        if el.element_type() == ElementType::BasicShape && h_left % 2 != 0 {
            report.issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                element_index: ei,
                message: format!(
                    "Eleman {} ({}): Toplam boy ({} sıra) tek sayı — mekik çift sıra kuralı uyarısı",
                    el.index, el.name, h_left
                ),
            });
        }

        // 6. Stoll 13656 even start width / base width check for BasicShape
        if el.element_type() == ElementType::BasicShape && doc.header.input_format != InputFormat::Mm
        {
            if el.start_width > 0.0 && (el.start_width - el.start_width.round()).abs() < 1e-4 {
                let sw = el.start_width.round() as i32;
                if sw % 2 != 0 {
                    report.issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        element_index: ei,
                        message: format!(
                            "Eleman {} ({}): Başlama eni ({}) tek sayı — çift en kuralı uyarısı (13656)",
                            el.index, el.name, sw
                        ),
                    });
                }
            }
            if !el.mirrored && !el.left_lines.is_empty() && !el.right_lines.is_empty() {
                let base_w = el.left_lines[0].stitches.abs() + el.right_lines[0].stitches.abs();
                if base_w % 2 != 0 {
                    report.issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        element_index: ei,
                        message: format!(
                            "Eleman {} ({}): Temel çizgi toplam eni ({} ilmek) tek sayı — çift en kuralı uyarısı (13656)",
                            el.index, el.name, base_w
                        ),
                    });
                }
            }
        }
    }

    report
}

/// Top panel `Başlama / Bitiş / Fark` range summary for selected line rows `[row_start ..= row_end]`.
#[derive(Debug, Clone, Default)]
pub struct RangeSummary {
    pub row_start: usize,
    pub row_end: usize,
    pub start_mm: Vec2,
    pub end_mm: Vec2,
    pub delta_mm: Vec2,
    pub start_st: (i32, i32),
    pub end_st: (i32, i32),
    pub delta_st: (i32, i32),
}

/// Compute `Başlama / Bitiş / Fark` over `[row_start ..= row_end]` for a given element and side.
pub fn compute_range_summary(
    el_geom: &ElementGeometry,
    side: LineSide,
    row_start: usize,
    row_end: usize,
) -> Option<RangeSummary> {
    let segs = match side {
        LineSide::Left => &el_geom.left_segments,
        LineSide::Right => &el_geom.right_segments,
    };
    if segs.is_empty() {
        return None;
    }
    let s = row_start.min(segs.len() - 1);
    let e = row_end.min(segs.len() - 1).max(s);
    let first = &segs[s];
    let last = &segs[e];
    Some(RangeSummary {
        row_start: s + 1,
        row_end: e + 1,
        start_mm: first.start_mm,
        end_mm: last.end_mm,
        delta_mm: Vec2::new(
            last.end_mm.x - first.start_mm.x,
            last.end_mm.y - first.start_mm.y,
        ),
        start_st: first.start_st,
        end_st: last.end_st,
        delta_st: (
            last.end_st.0 - first.start_st.0,
            last.end_st.1 - first.start_st.1,
        ),
    })
}

/// Compute `dist_from_top` (for `AbstandYBezugslinie == 2`, `Bitiş çizgisi`) in rows and mm.
pub fn element_distance_from_top(doc: &FormDocument, elem_idx: usize) -> (f64, f64) {
    let basic_h_rows: i32 = doc
        .basic_element()
        .map(|e| e.left_lines.iter().map(|l| l.rows).sum())
        .unwrap_or(0);
    let Some(el) = doc.elements.get(elem_idx) else {
        return (0.0, 0.0);
    };
    if el.left_lines.is_empty() && el.offset_y_rows.abs() < 1e-6 && el.offset_y_mm.abs() < 1e-6 {
        return (0.0, 0.0);
    }
    let el_h_rows: i32 = el.left_lines.iter().map(|l| l.rows).sum();
    let dist_rows = basic_h_rows as f64 - (el.offset_y_rows + el_h_rows as f64);
    let dist_mm = if doc.header.input_format == InputFormat::Mm {
        let basic_h_mm: f64 = doc
            .basic_element()
            .map(|e| e.left_lines.iter().map(|l| l.height_mm).sum())
            .unwrap_or(0.0);
        let el_h_mm: f64 = el.left_lines.iter().map(|l| l.height_mm).sum();
        crate::math::units::round2(basic_h_mm - (el.offset_y_mm + el_h_mm))
    } else {
        crate::math::units::round2(rows_to_mm(dist_rows.round() as i32, &doc.header))
    };
    (dist_rows, dist_mm)
}

/// Set an element's vertical position from the top of the basic shape (`Bitiş çizgisi`, `AbstandYBezugslinie == 2`).
/// Computes `y_bottom = H_base - H_elem - dist_from_top`, keeps `offset_y_rows` and `offset_y_mm` synchronized
/// (preserving `AbstandYmm` untouched in `Stitches` mode), and validates Stoll Error `13655` (`0x3557`).
pub fn set_element_distance_from_top(
    doc: &mut FormDocument,
    elem_idx: usize,
    dist_from_top: f64,
    is_mm: bool,
) -> Result<(), String> {
    let basic_h_rows: i32 = doc
        .basic_element()
        .map(|e| e.left_lines.iter().map(|l| l.rows).sum())
        .unwrap_or(0);
    let hdr = doc.header.clone();
    let basic_h_mm: f64 = if is_mm {
        doc.basic_element()
            .map(|e| e.left_lines.iter().map(|l| l.height_mm).sum())
            .unwrap_or(0.0)
    } else {
        rows_to_mm(basic_h_rows, &hdr)
    };

    let Some(el) = doc.elements.get(elem_idx) else {
        return Err("Geçersiz eleman indeksi.".to_string());
    };
    let el_h_rows: i32 = el.left_lines.iter().map(|l| l.rows).sum();
    let el_h_mm: f64 = if is_mm {
        el.left_lines.iter().map(|l| l.height_mm).sum()
    } else {
        rows_to_mm(el_h_rows, &hdr)
    };

    let (y_bottom_rows, y_bottom_mm) = if is_mm {
        let ym = basic_h_mm - el_h_mm - dist_from_top;
        let yr = crate::math::units::mm_to_rows(ym, &hdr) as f64;
        (yr, Some(crate::math::units::round2(ym)))
    } else {
        let yr = (basic_h_rows as f64 - el_h_rows as f64 - dist_from_top).round();
        (yr, None)
    };

    if elem_idx >= 1
        && (y_bottom_rows < -0.001
            || (basic_h_rows > 0 && (y_bottom_rows + el_h_rows as f64) > basic_h_rows as f64 + 0.001))
    {
        return Err(
            "Söz konusu elemanın yüksekliği, temel kesimin eleman yüksekliğinden daha büyüktür. (Hata 13655)"
                .to_string(),
        );
    }

    if let Some(el) = doc.elements.get_mut(elem_idx) {
        el.offset_y_rows = y_bottom_rows;
        if let Some(ym) = y_bottom_mm {
            el.offset_y_mm = ym;
        }
        doc.dirty = true;
    }
    Ok(())
}

/// Set an element's vertical position from the base line (`Zemin çizgi`, `AbstandYBezugslinie == 1`),
/// synchronizing `offset_y_rows` (and `offset_y_mm` in `Mm` mode) and validating Stoll Error `13655` (`0x3557`).
pub fn set_element_distance_from_bottom(
    doc: &mut FormDocument,
    elem_idx: usize,
    y_bottom: f64,
    is_mm: bool,
) -> Result<(), String> {
    let basic_h_rows: i32 = doc
        .basic_element()
        .map(|e| e.left_lines.iter().map(|l| l.rows).sum())
        .unwrap_or(0);
    let hdr = doc.header.clone();

    let Some(el) = doc.elements.get(elem_idx) else {
        return Err("Geçersiz eleman indeksi.".to_string());
    };
    let el_h_rows: i32 = el.left_lines.iter().map(|l| l.rows).sum();

    let (y_bottom_rows, y_bottom_mm) = if is_mm {
        let yr = crate::math::units::mm_to_rows(y_bottom, &hdr) as f64;
        (yr, Some(crate::math::units::round2(y_bottom)))
    } else {
        let yr = y_bottom.round();
        (yr, None)
    };

    if elem_idx >= 1
        && (y_bottom_rows < -0.001
            || (basic_h_rows > 0 && (y_bottom_rows + el_h_rows as f64) > basic_h_rows as f64 + 0.001))
    {
        return Err(
            "Söz konusu elemanın yüksekliği, temel kesimin eleman yüksekliğinden daha büyüktür. (Hata 13655)"
                .to_string(),
        );
    }

    if let Some(el) = doc.elements.get_mut(elem_idx) {
        el.offset_y_rows = y_bottom_rows;
        if let Some(ym) = y_bottom_mm {
            el.offset_y_mm = ym;
        }
        doc.dirty = true;
    }
    Ok(())
}

/// Re-anchor a secondary element with `y_ref_line == YReferenceLine::EndLine` after its height
/// (or basic shape height) changes, keeping `dist_from_top_rows` anchored to the top of `Element 1`.
pub fn sync_endline_element_y(doc: &mut FormDocument, elem_idx: usize, dist_from_top_rows: f64) {
    if elem_idx == 0 {
        return;
    }
    let basic_h_rows: i32 = doc
        .basic_element()
        .map(|e| e.left_lines.iter().map(|l| l.rows).sum())
        .unwrap_or(0);
    let hdr = doc.header.clone();
    let Some(el) = doc.elements.get_mut(elem_idx) else {
        return;
    };
    if el.y_ref_line != crate::model::YReferenceLine::EndLine {
        return;
    }
    let el_h_rows: i32 = el.left_lines.iter().map(|l| l.rows).sum();
    let y_bottom = (basic_h_rows as f64 - el_h_rows as f64 - dist_from_top_rows).max(0.0);
    el.offset_y_rows = y_bottom.round();
    if hdr.input_format == InputFormat::Mm {
        el.offset_y_mm = crate::math::units::round2(rows_to_mm(el.offset_y_rows as i32, &hdr));
    }
}

/// Snapshot `(elem_idx, dist_from_top_rows)` for all secondary elements anchored to `EndLine`
/// (`Bitiş çizgisi`) BEFORE mutating Element 1 or any secondary element's height.
pub fn snapshot_endline_distances(doc: &FormDocument) -> Vec<(usize, f64)> {
    (1..doc.elements.len())
        .filter(|&ei| doc.elements[ei].y_ref_line == crate::model::YReferenceLine::EndLine)
        .map(|ei| (ei, element_distance_from_top(doc, ei).0))
        .collect()
}

/// Re-anchor all snapshotted `EndLine` (`Bitiş çizgisi`) secondary elements after a height change.
pub fn sync_all_endline_elements(doc: &mut FormDocument, dists: &[(usize, f64)]) {
    for &(ei, dist_top) in dists {
        sync_endline_element_y(doc, ei, dist_top);
    }
}

