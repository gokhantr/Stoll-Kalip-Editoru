use crate::math::curves::{
    apply_curve_to_line, extract_curve_module_widths, generate_curve_steps, CurveGenParams,
};
use crate::math::geometry::{
    snapshot_endline_distances, sync_all_endline_elements,
};
use crate::math::units::{
    auto_detect_line_function_for_element, calculate_and_set_stufung, recalc_line_from_mm,
    recalc_line_from_stitches_and_rows, round2, rows_to_mm,
};
use crate::model::{
    CurveForm, ElementType, FormDocument, FormHeader, InputFormat, KanteChildSlot, LineFunction,
    LineSegment, LineSide, PointEntry, PointOffset, ShapeElement, YReferenceLine,
};

/// Insert a new empty line segment BEFORE `row_idx` (`Yeni Çizgiyi Öne Ekle`, `Insert`, `50574`).
pub fn insert_line_before(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
) -> Option<usize> {
    let endline_dists = snapshot_endline_distances(doc);

    let hdr = doc.header.clone();
    let tag_id = doc.next_line_tag_id();
    let el = doc.elements.get_mut(elem_idx)?;
    let is_cutout = matches!(
        el.element_type(),
        ElementType::Neckline | ElementType::Armhole | ElementType::Hole | ElementType::Opening
    );
    let lines = el.lines_mut(side);
    let pos = row_idx.min(lines.len());
    let default_func = if pos == 0 && !is_cutout {
        LineFunction::BaseLine
    } else {
        LineFunction::None
    };
    let mut new_line = LineSegment {
        tag_id,
        function: default_func,
        ..LineSegment::default()
    };
    if pos == 0 && !is_cutout {
        new_line.rows = 0;
        new_line.stitches = if side == LineSide::Left { -80 } else { 80 };
        recalc_line_from_stitches_and_rows(&mut new_line, &hdr);
    }
    new_line.index = (pos + 1) as i32;
    lines.insert(pos, new_line);
    if side == LineSide::Right {
        el.has_right_lines_section = true;
    } else {
        el.has_left_lines_section = true;
    }
    el.renumber_lines(side);
    sync_all_endline_elements(doc, &endline_dists);
    doc.dirty = true;
    Some(pos)
}

/// Append a new line segment at the end of the table (`Yeni Çizgiyi Sona Ekle`, `50573`).
pub fn append_line(doc: &mut FormDocument, elem_idx: usize, side: LineSide) -> Option<usize> {
    let len = doc.elements.get(elem_idx)?.lines(side).len();
    insert_line_before(doc, elem_idx, side, len)
}

/// Delete the selected line segment (`Seçilen Çizgileri Sil`, `Delete`, `50570`).
pub fn delete_line(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
) -> Option<LineSegment> {
    let endline_dists = snapshot_endline_distances(doc);

    let el = doc.elements.get_mut(elem_idx)?;
    let lines = el.lines_mut(side);
    if row_idx >= lines.len() {
        return None;
    }
    let removed = lines.remove(row_idx);
    el.renumber_lines(side);
    sync_all_endline_elements(doc, &endline_dists);
    doc.dirty = true;
    Some(removed)
}

/// Copy a line segment (`Çizgi Kopyala`, `Ctrl+C`, `50575`).
pub fn copy_line(doc: &FormDocument, elem_idx: usize, side: LineSide, row_idx: usize) -> Option<LineSegment> {
    doc.elements.get(elem_idx)?.lines(side).get(row_idx).cloned()
}

/// Paste/replace a line segment (`Çizgi Değiştir`, `Ctrl+V`, `50576`).
/// Assigns fresh GUIDs & TagID so Stoll never encounters duplicate IDs.
pub fn paste_line(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
    source: &LineSegment,
) -> bool {
    let endline_dists = snapshot_endline_distances(doc);
    let tag_id = doc.next_line_tag_id();
    let Some(el) = doc.elements.get_mut(elem_idx) else {
        return false;
    };
    let lines = el.lines_mut(side);
    if row_idx >= lines.len() {
        return false;
    }
    let mut cloned = source.clone();
    cloned.tag_id = tag_id;
    cloned.guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
    cloned.param_guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
    lines[row_idx] = cloned;
    el.renumber_lines(side);
    sync_all_endline_elements(doc, &endline_dists);
    doc.dirty = true;
    true
}

/// Add a new shape element (`Yeni Eleman`, `50560`) matching Stoll VecFormEditor 1:1 (`[ENG-03]`):
/// creates a blank secondary element with 0 lines, blank `Ad:` (when `name` is `None`),
/// `mirrored: true`, and `y_ref_line = EndLine` (for `Neckline`) or `BaseLine` (for others).
pub fn add_element(doc: &mut FormDocument, element_type: ElementType, name: Option<String>) -> usize {
    let next_idx = (doc.elements.len() + 1) as i32;
    let el_name = name.unwrap_or_default();

    let y_ref = if element_type == ElementType::Neckline {
        YReferenceLine::EndLine
    } else {
        YReferenceLine::BaseLine
    };

    let (init_y_rows, init_y_mm) = if y_ref == YReferenceLine::EndLine {
        let basic_h_rows: i32 = doc
            .basic_element()
            .map(|e| e.left_lines.iter().map(|l| l.rows).sum())
            .unwrap_or(0);
        let basic_h_mm = if doc.header.input_format == InputFormat::Mm {
            round2(rows_to_mm(basic_h_rows, &doc.header))
        } else {
            0.0
        };
        (basic_h_rows as f64, basic_h_mm)
    } else {
        (0.0, 0.0)
    };

    let mut el = ShapeElement {
        index: next_idx,
        name: el_name,
        art_tag_id: element_type.to_id(),
        mirrored: true,
        start_width: 0.0,
        offset_y_rows: init_y_rows,
        offset_y_mm: init_y_mm,
        y_ref_line: y_ref,
        has_left_lines_section: true,
        left_lines: Vec::new(),
        has_right_lines_section: false,
        right_lines: Vec::new(),
        ..ShapeElement::default()
    };
    el.raw.original_header = Some(format!("[Element] = {next_idx}"));

    doc.elements.push(el);
    doc.renumber_elements();
    doc.dirty = true;
    doc.elements.len() - 1
}

/// Delete a shape element (`Elemanı Sil`, `50561`). Refuses to delete Element 1 (`BasicShape`) or the last remaining element.
pub fn delete_element(doc: &mut FormDocument, elem_idx: usize) -> Option<ShapeElement> {
    if elem_idx == 0 || doc.elements.len() <= 1 || elem_idx >= doc.elements.len() {
        return None;
    }
    let removed = doc.elements.remove(elem_idx);
    doc.renumber_elements();
    doc.dirty = true;
    Some(removed)
}

/// Adopt ("Standart özellikler -> devir al") knitting properties (`[Ausblenden]`, `[Mindern]`,
/// `[Zunehmen]`, `[Abketteln]`, `[Punkt]`) from a standard preset `[Kante]` into a target line segment.
/// Preserves the target line's own geometry (`Breite`, `Höhe`, `NadelZahl`, `Reihen`, `Stufung*`, `Curve`).
pub fn apply_standard_preset_to_line(target: &mut LineSegment, preset: &LineSegment) {
    target.fade_out = preset.fade_out.clone();
    target.narrowing = preset.narrowing.clone();
    target.widening = preset.widening.clone();
    target.binding_off = preset.binding_off.clone();
    if !preset.points.is_empty() {
        target.points = preset.points.clone();
    }
    target.default_used = preset.index;
    if target.comment.is_empty() && !preset.comment.is_empty() {
        target.comment = preset.comment.clone();
    }

    if target.fade_out.is_some() {
        target.ensure_child_slot(KanteChildSlot::FadeOut);
    }
    if target.narrowing.is_some() {
        target.ensure_child_slot(KanteChildSlot::Narrowing);
    }
    if target.widening.is_some() {
        target.ensure_child_slot(KanteChildSlot::Widening);
    }
    if target.binding_off.is_some() {
        target.ensure_child_slot(KanteChildSlot::BindingOff);
    }
    for i in 0..target.points.len() {
        target.ensure_child_slot(KanteChildSlot::Point(i));
    }
}

/// Apply guidelines ("Yönergeler -> devir al") across either the selected element or all elements.
/// Normalizes even rows if `prefer_even_rows` is active and enforces `step_factor >= 1`.
pub fn apply_guidelines(doc: &mut FormDocument, selected_elem_idx: usize) -> usize {
    let endline_dists = snapshot_endline_distances(doc);
    let g = doc.guidelines.clone();
    let hdr = doc.header.clone();
    let mut modified_count = 0usize;

    for (idx, el) in doc.elements.iter_mut().enumerate() {
        if !g.apply_to_all_elements && idx != selected_elem_idx {
            continue;
        }
        for line in el.left_lines.iter_mut().chain(el.right_lines.iter_mut()) {
            let mut changed = false;
            if line.step_factor < 1 {
                line.step_factor = 1;
                changed = true;
            }
            if g.prefer_even_rows && line.rows != 0 && line.rows % 2 != 0 {
                line.rows += if line.rows > 0 { 1 } else { -1 };
                recalc_line_from_stitches_and_rows(line, &hdr);
                changed = true;
            }
            if changed {
                modified_count += 1;
            }
        }
    }

    if modified_count > 0 {
        sync_all_endline_elements(doc, &endline_dists);
        doc.dirty = true;
    }
    modified_count
}

/// Multiply marking points (`Noktaları Çoğalt`, Dialog 4277): adds `count` offsets stepped by `(dx, dy)`
/// starting from `(start_x, start_y)` onto a line's `[Punkt]` entry (creating one if absent).
pub fn multiply_marking_points(
    line: &mut LineSegment,
    point_index: usize,
    start_x: i32,
    start_y: i32,
    dx: i32,
    dy: i32,
    count: usize,
) {
    if count == 0 {
        return;
    }
    while line.points.len() <= point_index {
        let p_idx = line.points.len();
        line.points.push(PointEntry {
            index: p_idx as i32,
            ..PointEntry::default()
        });
        line.ensure_child_slot(KanteChildSlot::Point(p_idx));
    }
    let pt = &mut line.points[point_index];
    pt.offsets.clear();
    for i in 0..count {
        pt.offsets.push(PointOffset {
            index: i as i32,
            x: start_x + dx * (i as i32),
            y: start_y + dy * (i as i32),
            raw: Default::default(),
        });
    }
    pt.number_of_marks = Some(count as i32);
}

/// Synchronize `[LinienRechts]` as the exact horizontal mirror of `[LinienLinks]` when `Gespiegelt == 1`
/// (per `MATH_AND_CURVES_SPEC.md` Section 5.2).
pub fn sync_mirrored_right_lines(doc: &mut FormDocument, elem_idx: usize) {
    let Some(is_mirrored) = doc.elements.get(elem_idx).map(|e| e.mirrored) else {
        return;
    };
    if !is_mirrored {
        return;
    }

    let left_count = doc.elements[elem_idx].left_lines.len();
    let mut new_tag_ids = Vec::with_capacity(left_count);
    for i in 0..left_count {
        let existing_tag = doc.elements[elem_idx]
            .right_lines
            .get(i)
            .map(|l| l.tag_id)
            .filter(|&t| t > 0);
        let tag = existing_tag.unwrap_or_else(|| doc.next_line_tag_id());
        new_tag_ids.push(tag);
    }

    let el = &mut doc.elements[elem_idx];
    let mut mirrored_right = Vec::with_capacity(el.left_lines.len());
    for (i, left) in el.left_lines.iter().enumerate() {
        let mut r = left.clone();
        r.tag_id = new_tag_ids[i];
        if let Some(prev_r) = el.right_lines.get(i) {
            r.guid_id = prev_r.guid_id.clone();
            r.param_guid_id = prev_r.param_guid_id.clone();
        } else {
            r.guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
            r.param_guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
        }
        r.width_mm = -left.width_mm;
        r.stitches = -left.stitches;
        r.step_x = -left.step_x;
        r.rest_x = -left.rest_x;
        if let Some(ref mut c) = r.curve {
            for s in &mut c.steps {
                s.step_width = -s.step_width;
            }
        }
        for pt in &mut r.points {
            for off in &mut pt.offsets {
                off.x = -off.x;
            }
        }
        mirrored_right.push(r);
    }
    el.right_lines = mirrored_right;
    el.has_right_lines_section = !el.right_lines.is_empty();
    el.renumber_lines(LineSide::Right);

    // Also mirror standalone markers (`[PunkteLinks]` -> `[PunkteRechts]`)
    let mut mirrored_pts = Vec::with_capacity(el.left_points.len());
    for pt in &el.left_points {
        let mut rpt = pt.clone();
        for off in &mut rpt.offsets {
            off.x = -off.x;
        }
        mirrored_pts.push(rpt);
    }
    el.right_points = mirrored_pts;
    el.has_right_points_section = !el.right_points.is_empty();
    renumber_points(&mut el.right_points);

    doc.dirty = true;
}

/// Synchronize all mirrored elements in the document (`[ENG-04]`).
pub fn sync_all_mirrored_elements(doc: &mut FormDocument) {
    for idx in 0..doc.elements.len() {
        if doc.elements[idx].mirrored {
            sync_mirrored_right_lines(doc, idx);
        }
    }
}

/// Swap left and right sides of an asymmetric (`!el.mirrored`) element (`[ENG-05]`),
/// mirroring horizontal deltas, curve steps, line point offsets, and standalone markers.
pub fn swap_left_right(doc: &mut FormDocument, elem_idx: usize) -> bool {
    let Some(el) = doc.elements.get_mut(elem_idx) else {
        return false;
    };
    if el.mirrored {
        return false;
    }

    std::mem::swap(&mut el.left_lines, &mut el.right_lines);
    std::mem::swap(&mut el.left_points, &mut el.right_points);

    for l in el.left_lines.iter_mut().chain(el.right_lines.iter_mut()) {
        l.stitches = -l.stitches;
        l.width_mm = -l.width_mm;
        l.step_x = -l.step_x;
        l.rest_x = -l.rest_x;
        if let Some(ref mut c) = l.curve {
            for s in &mut c.steps {
                s.step_width = -s.step_width;
            }
        }
        for pt in &mut l.points {
            for off in &mut pt.offsets {
                off.x = -off.x;
            }
        }
    }

    for pt in el.left_points.iter_mut().chain(el.right_points.iter_mut()) {
        for off in &mut pt.offsets {
            off.x = -off.x;
        }
    }

    el.has_left_lines_section = !el.left_lines.is_empty();
    el.has_right_lines_section = !el.right_lines.is_empty();
    el.has_left_points_section = !el.left_points.is_empty();
    el.has_right_points_section = !el.right_points.is_empty();
    el.renumber_lines(LineSide::Left);
    el.renumber_lines(LineSide::Right);
    renumber_points(&mut el.left_points);
    renumber_points(&mut el.right_points);
    doc.dirty = true;
    true
}

/// Apply `Yönergeler` (`Reglan`, `Artırma`, `Boy`) to selected lines (`[ENG-08]`):
/// - Non-curve lines (`row > 0`): calls `calculate_and_set_stufung` (preserving `N` and `R`) and sets `default_*` flags.
/// - Formula curves (`Line`, `Step`, `JCurve`, `SCurve`): regenerates curve steps with directive `step_x`.
/// - Mirrors right lines automatically when `el.mirrored && side == LineSide::Left`.
pub fn apply_directives_to_lines(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    rows: &[usize],
    directive_narrowing: i32,
    directive_widening: i32,
    directive_height: i32,
) -> usize {
    let hdr = doc.header.clone();
    let mut modified = 0usize;
    let is_mirrored;

    {
        let Some(el) = doc.elements.get_mut(elem_idx) else {
            return 0;
        };
        is_mirrored = el.mirrored;
        let is_basic = el.element_type() == ElementType::BasicShape;
        let lines = el.lines_mut(side);
        let target_rows: Vec<usize> = if rows.is_empty() {
            lines
                .iter()
                .enumerate()
                .filter(|&(i, l)| {
                    !(is_basic && i == 0)
                        && l.function != LineFunction::BaseLine
                        && !(l.rows == 0 && l.last_line != 0)
                })
                .map(|(i, _)| i)
                .collect()
        } else {
            rows.iter()
                .copied()
                .filter(|&r| {
                    !(is_basic && r == 0)
                        && lines
                            .get(r)
                            .map(|l| l.function != LineFunction::BaseLine)
                            .unwrap_or(false)
                })
                .collect()
        };

        for r in target_rows {
            let Some(line) = lines.get_mut(r) else {
                continue;
            };
            if let Some(ref c) = line.curve {
                if !c.steps.is_empty() {
                    let form = CurveForm::from_id(c.function);
                    if matches!(
                        form,
                        CurveForm::Line | CurveForm::Step | CurveForm::JCurve | CurveForm::SCurve
                    ) {
                        let step_mag = match line.function {
                            LineFunction::Narrowing | LineFunction::NarrowingMulti => {
                                directive_narrowing.abs().max(1)
                            }
                            LineFunction::Widening => directive_widening.abs().max(1),
                            _ => 1,
                        };
                        let (nw, ww, fw) = extract_curve_module_widths(line);
                        let steps = generate_curve_steps(&CurveGenParams {
                            curve_form: form,
                            total_dx: line.stitches,
                            total_dy: line.rows,
                            step_x: if line.stitches < 0 {
                                -step_mag
                            } else {
                                step_mag
                            },
                            step_y: directive_height.max(0),
                            narrowing_width: nw,
                            widening_width: ww,
                            fade_out_width: fw,
                        });
                        apply_curve_to_line(line, &hdr, form, steps);
                        modified += 1;
                    }
                    continue;
                }
            }

            match line.function {
                LineFunction::Narrowing | LineFunction::NarrowingMulti if line.stitches != 0 => {
                    let sx = directive_narrowing.abs().max(1) * line.stitches.signum();
                    let sy = directive_height.max(0);
                    calculate_and_set_stufung(line, sx, sy);
                    line.default_narrowing = true;
                    modified += 1;
                }
                LineFunction::Widening if line.stitches != 0 => {
                    let sx = directive_widening.abs().max(1) * line.stitches.signum();
                    let sy = directive_height.max(0);
                    calculate_and_set_stufung(line, sx, sy);
                    line.default_widening = 1;
                    modified += 1;
                }
                _ if line.stitches == 0 && line.rows != 0 && directive_height > 0 => {
                    calculate_and_set_stufung(line, 0, directive_height);
                    line.default_height = 1;
                    modified += 1;
                }
                _ => {}
            }
        }
    }

    if modified > 0 {
        if is_mirrored && side == LineSide::Left {
            sync_mirrored_right_lines(doc, elem_idx);
        }
        doc.dirty = true;
    }
    modified
}

/// Renumber standalone marking points (`[Punkt] = 1..=P`).
pub fn renumber_points(pts: &mut [PointEntry]) {
    for (i, pt) in pts.iter_mut().enumerate() {
        pt.index = (i + 1) as i32;
        pt.raw.original_header = Some(format!("[Punkt] = {}", pt.index));
    }
}

/// Copy selected standalone markers (`[ENG-12]`).
pub fn copy_markers(
    doc: &FormDocument,
    elem_idx: usize,
    side: LineSide,
    rows: &[usize],
) -> Vec<PointEntry> {
    let Some(el) = doc.elements.get(elem_idx) else {
        return Vec::new();
    };
    let pts = match side {
        LineSide::Left => &el.left_points,
        LineSide::Right => &el.right_points,
    };
    let mut sorted = rows.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
        .into_iter()
        .filter_map(|r| pts.get(r).cloned())
        .collect()
}

/// Paste/replace standalone markers starting at `start_row` (`[ENG-12]`).
pub fn paste_replace_markers(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    start_row: usize,
    source: &[PointEntry],
) -> usize {
    if source.is_empty() {
        return 0;
    }
    let is_mirrored;
    let mut count = 0usize;
    {
        let Some(el) = doc.elements.get_mut(elem_idx) else {
            return 0;
        };
        is_mirrored = el.mirrored;
        let pts = match side {
            LineSide::Left => {
                el.has_left_points_section = true;
                &mut el.left_points
            }
            LineSide::Right => {
                el.has_right_points_section = true;
                &mut el.right_points
            }
        };
        for (i, src) in source.iter().enumerate() {
            let idx = start_row + i;
            if idx < pts.len() {
                pts[idx] = src.clone();
            } else {
                pts.push(src.clone());
            }
            count += 1;
        }
        renumber_points(pts);
    }
    if is_mirrored && side == LineSide::Left {
        sync_mirrored_right_lines(doc, elem_idx);
    }
    doc.dirty = true;
    count
}

/// Paste/insert standalone markers before `before_row` (`[ENG-12]`).
pub fn paste_insert_markers(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    before_row: usize,
    source: &[PointEntry],
) -> usize {
    if source.is_empty() {
        return 0;
    }
    let is_mirrored;
    let mut count = 0usize;
    {
        let Some(el) = doc.elements.get_mut(elem_idx) else {
            return 0;
        };
        is_mirrored = el.mirrored;
        let pts = match side {
            LineSide::Left => {
                el.has_left_points_section = true;
                &mut el.left_points
            }
            LineSide::Right => {
                el.has_right_points_section = true;
                &mut el.right_points
            }
        };
        let base = before_row.min(pts.len());
        for (i, src) in source.iter().enumerate() {
            pts.insert(base + i, src.clone());
            count += 1;
        }
        renumber_points(pts);
    }
    if is_mirrored && side == LineSide::Left {
        sync_mirrored_right_lines(doc, elem_idx);
    }
    doc.dirty = true;
    count
}

/// Parse CSV/TSV/semicolon external table text into `LineSegment`s (`[ENG-11]`).
/// Supported row formats:
/// - `Boy, En`
/// - `Boy, En, BoyBasamak, EnBasamak, Faktor[, Yorum]`
pub fn parse_external_table_lines(
    text: &str,
    header: &FormHeader,
    side: LineSide,
) -> Vec<LineSegment> {
    let mut result = Vec::new();
    let is_left = side == LineSide::Left;

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let sep = if trimmed.contains('\t') {
            '\t'
        } else if trimmed.contains(';') {
            ';'
        } else {
            ','
        };
        let cols: Vec<&str> = trimmed.split(sep).map(|s| s.trim()).collect();
        if cols.len() < 2 {
            continue;
        }

        let parse_f = |s: &str| -> Option<f64> { s.replace(',', ".").parse::<f64>().ok() };
        let Some(boy_val) = parse_f(cols[0]) else {
            continue; // skip header row
        };
        let Some(en_val) = parse_f(cols[1]) else {
            continue;
        };

        let mut seg = LineSegment {
            index: (result.len() + 1) as i32,
            ..LineSegment::default()
        };

        if header.input_format == InputFormat::Mm {
            seg.height_mm = boy_val;
            seg.width_mm = en_val;
            recalc_line_from_mm(&mut seg, header);
        } else {
            seg.rows = boy_val.round() as i32;
            seg.stitches = en_val.round() as i32;
            if cols.len() >= 5 {
                if let (Some(sy), Some(sx)) = (
                    cols[2].parse::<i32>().ok(),
                    cols[3].parse::<i32>().ok(),
                ) {
                    seg.step_y = sy;
                    seg.step_x = sx;
                }
                if let Ok(f) = cols[4].parse::<i32>() {
                    seg.step_factor = f.max(1);
                }
            }
            recalc_line_from_stitches_and_rows(&mut seg, header);
        }

        if cols.len() >= 6 && !cols[5].is_empty() {
            seg.comment = cols[5].to_string();
        }
        auto_detect_line_function_for_element(&mut seg, result.is_empty(), is_left, ElementType::BasicShape);
        result.push(seg);
    }

    result
}

/// Import CSV/TSV external table rows into an element (`[ENG-11]`), assigning fresh `TagID`s
/// and syncing mirrored right lines when `el.mirrored && side == LineSide::Left`.
pub fn import_external_table_lines(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    text: &str,
    replace_existing: bool,
) -> usize {
    let endline_dists = snapshot_endline_distances(doc);

    let hdr = doc.header.clone();
    let elem_type = doc
        .elements
        .get(elem_idx)
        .map(|e| e.element_type())
        .unwrap_or(ElementType::BasicShape);
    let mut parsed = parse_external_table_lines(text, &hdr, side);
    if parsed.is_empty() {
        return 0;
    }
    let existing_len = doc
        .elements
        .get(elem_idx)
        .map(|e| if replace_existing { 0 } else { e.lines(side).len() })
        .unwrap_or(0);
    for (i, seg) in parsed.iter_mut().enumerate() {
        seg.tag_id = doc.next_line_tag_id();
        auto_detect_line_function_for_element(
            seg,
            existing_len + i == 0,
            side == LineSide::Left,
            elem_type,
        );
    }
    let count = parsed.len();
    let is_mirrored;
    {
        let Some(el) = doc.elements.get_mut(elem_idx) else {
            return 0;
        };
        is_mirrored = el.mirrored;
        let lines = el.lines_mut(side);
        if replace_existing {
            *lines = parsed;
        } else {
            lines.extend(parsed);
        }
        if side == LineSide::Left {
            el.has_left_lines_section = true;
        } else {
            el.has_right_lines_section = true;
        }
        el.renumber_lines(side);
    }
    sync_all_endline_elements(doc, &endline_dists);
    if is_mirrored && side == LineSide::Left {
        sync_mirrored_right_lines(doc, elem_idx);
    }
    doc.dirty = true;
    count
}

/// Smart closing-line helper:
/// - If `sum_x == 0`, does nothing and returns `None`.
/// - If the last row (`len >= 2`) is already a horizontal closing row (`rows == 0 && function != BaseLine`),
///   adjusts its `stitches` (`-= sum_x`) and `last_line = 1` instead of appending a duplicate row.
/// - Otherwise appends a new closing horizontal row (`stitches = -sum_x, rows = 0, last_line = 1`).
/// Automatically recalculates `mm`/`Stufung` and syncs mirrored right lines if `el.mirrored`.
pub fn update_or_append_closing_line(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
) -> Option<usize> {
    let (sum_x, existing_closing_idx, is_mirrored) = {
        let el = doc.elements.get(elem_idx)?;
        let lines = el.lines(side);
        let sum_x: i32 = lines.iter().map(|l| l.stitches).sum();
        if sum_x == 0 {
            return None;
        }
        let closing_idx = if lines.len() >= 2 {
            let last_i = lines.len() - 1;
            let last = &lines[last_i];
            if last.rows == 0 && last.function != LineFunction::BaseLine {
                Some(last_i)
            } else {
                None
            }
        } else {
            None
        };
        (sum_x, closing_idx, el.mirrored)
    };

    let hdr = doc.header.clone();
    let target_idx = if let Some(idx) = existing_closing_idx {
        let el = doc.elements.get_mut(elem_idx)?;
        if let Some(line) = el.lines_mut(side).get_mut(idx) {
            line.stitches -= sum_x;
            line.rows = 0;
            line.last_line = 1;
            line.function = LineFunction::None;
            line.user_set_function = false;
            recalc_line_from_stitches_and_rows(line, &hdr);
        }
        idx
    } else {
        let pos = append_line(doc, elem_idx, side)?;
        let el = doc.elements.get_mut(elem_idx)?;
        if let Some(line) = el.lines_mut(side).get_mut(pos) {
            line.stitches = -sum_x;
            line.rows = 0;
            line.last_line = 1;
            line.function = LineFunction::None;
            line.user_set_function = false;
            recalc_line_from_stitches_and_rows(line, &hdr);
        }
        pos
    };

    if is_mirrored && side == LineSide::Left {
        sync_mirrored_right_lines(doc, elem_idx);
    }
    doc.dirty = true;
    Some(target_idx)
}

