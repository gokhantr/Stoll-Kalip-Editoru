use crate::math::curves::{
    apply_curve_to_line, mirror_curve_steps_single_system, resolve_curve_module_widths,
    generate_curve_steps, CourseParityMode, CurveGenParams,
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
    LineSegment, LineSide, ModuleAssignment, PointEntry, PointOffset, ShapeElement, YReferenceLine,
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
/// Protects `BaseLine` (`Temel`) invariant: refuses to overwrite a locked `Temel` row unless source is `BaseLine`,
/// and remaps `BaseLine` -> `None` when pasting onto `row_idx > 0`.
pub fn paste_line(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
    source: &LineSegment,
) -> bool {
    let endline_dists = snapshot_endline_distances(doc);
    let Some(el_check) = doc.elements.get(elem_idx) else {
        return false;
    };
    if row_idx >= el_check.lines(side).len() {
        return false;
    }
    if el_check.is_base_line_row(side, row_idx) && source.function != LineFunction::BaseLine {
        return false;
    }
    let tag_id = doc.next_line_tag_id();
    let Some(el) = doc.elements.get_mut(elem_idx) else {
        return false;
    };
    let lines = el.lines_mut(side);
    let mut cloned = source.clone();
    if row_idx > 0 && cloned.function == LineFunction::BaseLine {
        cloned.function = LineFunction::None;
    }
    cloned.tag_id = tag_id;
    cloned.guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
    cloned.param_guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
    lines[row_idx] = cloned;
    el.renumber_lines(side);
    sync_all_endline_elements(doc, &endline_dists);
    doc.dirty = true;
    true
}

/// Insert a cloned line segment BEFORE `row_idx` (`Ekle+Yapıştır`, `Ctrl+Shift+V`) with a single fresh `TagID` & GUIDs.
pub fn paste_insert_line(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
    row_idx: usize,
    source: &LineSegment,
) -> Option<usize> {
    let endline_dists = snapshot_endline_distances(doc);
    let tag_id = doc.next_line_tag_id();
    let el = doc.elements.get_mut(elem_idx)?;
    let is_cutout = matches!(
        el.element_type(),
        ElementType::Neckline | ElementType::Armhole | ElementType::Hole | ElementType::Opening
    );
    let lines = el.lines_mut(side);
    let pos = row_idx.min(lines.len());
    let mut cloned = source.clone();
    if (pos > 0 || is_cutout) && cloned.function == LineFunction::BaseLine {
        cloned.function = LineFunction::None;
    }
    cloned.tag_id = tag_id;
    cloned.guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
    cloned.param_guid_id = format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase());
    cloned.index = (pos + 1) as i32;
    lines.insert(pos, cloned);
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
            if left.single_system_curve && !c.steps.is_empty() {
                c.steps = mirror_curve_steps_single_system(&c.steps, true);
                for (idx, s) in c.steps.iter_mut().enumerate() {
                    s.index = (idx + 1) as i32;
                    s.tag_id = s.index;
                    s.raw.original_header = Some(format!("[KurvenLinie] = {}", s.index));
                }
            } else {
                for s in &mut c.steps {
                    s.step_width = -s.step_width;
                }
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

    // Also mirror standalone markers (`[PunkteLinks]` -> `[PunkteRechts]`), preserving/allocating unique PunktTagIds
    let left_pt_count = doc.elements[elem_idx].left_points.len();
    let mut new_pt_tag_ids = Vec::with_capacity(left_pt_count);
    for i in 0..left_pt_count {
        let existing_pt_tag = doc.elements[elem_idx]
            .right_points
            .get(i)
            .map(|p| p.punkt_tag_id)
            .filter(|&t| t > 0);
        let tag = existing_pt_tag.unwrap_or_else(|| doc.next_point_tag_id());
        new_pt_tag_ids.push(tag);
    }

    let el = &mut doc.elements[elem_idx];
    let mut mirrored_pts = Vec::with_capacity(el.left_points.len());
    for (i, pt) in el.left_points.iter().enumerate() {
        let mut rpt = pt.clone();
        rpt.punkt_tag_id = new_pt_tag_ids[i];
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
    let std_nw = doc.guidelines.std_narrow_w;
    let std_ww = doc.guidelines.std_widen_w;
    let std_fw = doc.guidelines.std_fadeout_w;
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
                        && !(l.rows == 0 && l.default_widening == 1)
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
                        let (nw, ww, fw) =
                            resolve_curve_module_widths(line, std_nw, std_ww, std_fw);
                        let parity_mode = if line.single_system_curve {
                            match side {
                                LineSide::Left => CourseParityMode::SingleSystemLeftOdd,
                                LineSide::Right => CourseParityMode::SingleSystemRightEven,
                            }
                        } else {
                            CourseParityMode::Any
                        };
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
                            parity_mode,
                            ..Default::default()
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
    if source.is_empty() || elem_idx >= doc.elements.len() {
        return 0;
    }
    let mut cloned_list: Vec<PointEntry> = Vec::with_capacity(source.len());
    for src in source {
        let mut pt = src.clone();
        pt.punkt_tag_id = doc.next_point_tag_id();
        cloned_list.push(pt);
    }
    let is_mirrored;
    let count = cloned_list.len();
    {
        let el = &mut doc.elements[elem_idx];
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
        for (i, pt) in cloned_list.into_iter().enumerate() {
            let idx = start_row + i;
            if idx < pts.len() {
                pts[idx] = pt;
            } else {
                pts.push(pt);
            }
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
    if source.is_empty() || elem_idx >= doc.elements.len() {
        return 0;
    }
    let mut cloned_list: Vec<PointEntry> = Vec::with_capacity(source.len());
    for src in source {
        let mut pt = src.clone();
        pt.punkt_tag_id = doc.next_point_tag_id();
        cloned_list.push(pt);
    }
    let is_mirrored;
    let count = cloned_list.len();
    {
        let el = &mut doc.elements[elem_idx];
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
        for (i, pt) in cloned_list.into_iter().enumerate() {
            pts.insert(base + i, pt);
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

        let parse_f = |s: &str| -> Option<f64> {
            s.replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
        };
        let Some(boy_val) = parse_f(cols[0]) else {
            continue; // skip header or non-finite row
        };
        let Some(en_val) = parse_f(cols[1]) else {
            continue;
        };

        let mut seg = LineSegment {
            index: (result.len() + 1) as i32,
            ..LineSegment::default()
        };

        if header.input_format == InputFormat::Mm {
            seg.height_mm = boy_val.clamp(-100_000.0, 100_000.0);
            seg.width_mm = en_val.clamp(-100_000.0, 100_000.0);
            recalc_line_from_mm(&mut seg, header);
        } else {
            seg.rows = boy_val.round().clamp(-100_000.0, 100_000.0) as i32;
            seg.stitches = en_val.round().clamp(-100_000.0, 100_000.0) as i32;
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

/// Smart closing-line helper matching Stoll `vecformeditor.exe` 1:1:
/// - If `sum_x == 0`, does nothing and returns `None`.
/// - If the last row (`len >= 2`) is already a horizontal closing row (`rows == 0 && function != BaseLine`),
///   adjusts its `stitches` (`-= sum_x`), sets `last_line = 0` and `default_widening = 1` (`DefaultZunehmen = 1`).
/// - Otherwise appends a clean closing horizontal row (`stitches = -sum_x, rows = 0, last_line = 0, default_widening = 1, default_used = 0`, zero sub-sections).
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
        let lines = el.lines_mut(side);
        for (i, l) in lines.iter_mut().enumerate() {
            if i != idx && l.default_widening == 1 {
                l.default_widening = 0;
            }
        }
        if let Some(line) = lines.get_mut(idx) {
            line.stitches -= sum_x;
            line.rows = 0;
            line.last_line = 0;
            line.default_widening = 1;
            line.default_used = 0;
            line.function = LineFunction::None;
            line.user_set_function = false;
            recalc_line_from_stitches_and_rows(line, &hdr);
        }
        idx
    } else {
        let pos = append_line(doc, elem_idx, side)?;
        let el = doc.elements.get_mut(elem_idx)?;
        let lines = el.lines_mut(side);
        for (i, l) in lines.iter_mut().enumerate() {
            if i != pos && l.default_widening == 1 {
                l.default_widening = 0;
            }
        }
        if let Some(line) = lines.get_mut(pos) {
            line.stitches = -sum_x;
            line.rows = 0;
            line.last_line = 0;
            line.default_widening = 1;
            line.default_used = 0;
            line.comment.clear();
            line.function = LineFunction::None;
            line.user_set_function = false;
            line.fade_out = None;
            line.narrowing = None;
            line.widening = None;
            line.binding_off = None;
            line.connection_points = None;
            line.child_order.clear();
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

/// Returns `Some(last_idx)` if the last line on `side` of `el` is an active horizontal closing row
/// (`rows == 0 && function != BaseLine`, and either `default_widening == 1` (`DefaultZunehmen = 1`)
/// or it is already a non-zero horizontal row closing `sum_x == 0`).
/// Never hijacks a freshly added blank row (`rows == 0 && stitches == 0 && default_widening == 0`).
pub fn closing_line_index(el: &ShapeElement, side: LineSide) -> Option<usize> {
    let lines = el.lines(side);
    if lines.len() < 2 {
        return None;
    }
    let last_i = lines.len() - 1;
    let last = &lines[last_i];
    if last.rows == 0
        && last.function != LineFunction::BaseLine
        && (last.default_widening == 1
            || (last.stitches != 0 && lines.iter().map(|l| l.stitches).sum::<i32>() == 0))
    {
        Some(last_i)
    } else {
        None
    }
}

/// Automatically recalculate an EXISTING closing horizontal row on `(elem_idx, side)`
/// so `sum_x == 0`, preserving any user-chosen `LineFunction` on the closing row.
/// Also clears any stale `default_widening = 1` on non-closing rows so Stoll `DefaultZunehmen = 1`
/// appears exclusively on the active closing row (with `LastLine = 0`).
/// Does NOT append a new closing row if none exists yet (use `ensure_closing_line_for_side` for that).
pub fn auto_sync_closing_line_for_side(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
) -> bool {
    let hdr = doc.header.clone();
    let (existing_closing, is_mirrored) = {
        let Some(el) = doc.elements.get(elem_idx) else {
            return false;
        };
        if el.lines(side).is_empty() {
            return false;
        }
        (closing_line_index(el, side), el.mirrored)
    };

    let mut changed = false;
    {
        let el = &mut doc.elements[elem_idx];
        let lines = el.lines_mut(side);
        for (i, line) in lines.iter_mut().enumerate() {
            if Some(i) != existing_closing && line.default_widening == 1 {
                line.default_widening = 0;
                changed = true;
            }
        }
        if let Some(c_idx) = existing_closing {
            let sum_others: i32 = lines[..c_idx].iter().map(|l| l.stitches).sum();
            let target_stitches = -sum_others;
            if let Some(line) = lines.get_mut(c_idx) {
                if line.stitches != target_stitches
                    || line.rows != 0
                    || line.last_line != 0
                    || line.default_widening != 1
                {
                    line.stitches = target_stitches;
                    line.rows = 0;
                    line.last_line = 0;
                    line.default_widening = 1;
                    if !line.user_set_function && line.function == LineFunction::None {
                        line.default_used = 0;
                    }
                    recalc_line_from_stitches_and_rows(line, &hdr);
                    changed = true;
                }
            }
        }
    }

    if changed {
        if is_mirrored && side == LineSide::Left {
            sync_mirrored_right_lines(doc, elem_idx);
        }
        doc.dirty = true;
    }
    changed
}

/// Ensure a closing horizontal row exists and closes `sum_x == 0` on `(elem_idx, side)`
/// (updates existing closing row or appends a clean Stoll closing row with `LastLine = 0`,
/// `DefaultZunehmen = 1`, `DefaultUsed = 0`, and zero sub-sections if missing and `has_height && sum_x != 0`).
pub fn ensure_closing_line_for_side(
    doc: &mut FormDocument,
    elem_idx: usize,
    side: LineSide,
) -> bool {
    if auto_sync_closing_line_for_side(doc, elem_idx, side) {
        return true;
    }
    let hdr = doc.header.clone();
    let (existing_closing, has_height, sum_x, is_mirrored) = {
        let Some(el) = doc.elements.get(elem_idx) else {
            return false;
        };
        let lines = el.lines(side);
        if lines.is_empty() {
            return false;
        }
        let closing = closing_line_index(el, side);
        let has_h = lines.iter().any(|l| l.rows != 0);
        let sx: i32 = lines.iter().map(|l| l.stitches).sum();
        (closing, has_h, sx, el.mirrored)
    };
    if existing_closing.is_some() || !has_height || sum_x == 0 {
        return false;
    }
    let tag_id = doc.next_line_tag_id();
    let el = &mut doc.elements[elem_idx];
    let lines = el.lines_mut(side);
    for l in lines.iter_mut() {
        if l.default_widening == 1 {
            l.default_widening = 0;
        }
    }
    let pos = lines.len();
    let mut closing = LineSegment {
        index: (pos + 1) as i32,
        tag_id,
        stitches: -sum_x,
        rows: 0,
        function: LineFunction::None,
        user_set_function: false,
        last_line: 0,
        default_used: 0,
        default_widening: 1,
        connection_points: None,
        child_order: Vec::new(),
        ..LineSegment::default()
    };
    recalc_line_from_stitches_and_rows(&mut closing, &hdr);
    lines.push(closing);
    if side == LineSide::Right {
        el.has_right_lines_section = true;
    } else {
        el.has_left_lines_section = true;
    }
    el.renumber_lines(side);
    if is_mirrored && side == LineSide::Left {
        sync_mirrored_right_lines(doc, elem_idx);
    }
    doc.dirty = true;
    true
}

/// Ensure every element and active side in `doc` has a closing row before saving.
pub fn ensure_all_closing_lines(doc: &mut FormDocument) -> bool {
    let mut any_changed = false;
    for elem_idx in 0..doc.elements.len() {
        if ensure_closing_line_for_side(doc, elem_idx, LineSide::Left) {
            any_changed = true;
        }
        let sync_right = doc
            .elements
            .get(elem_idx)
            .map(|el| !el.mirrored && !el.right_lines.is_empty())
            .unwrap_or(false);
        if sync_right && ensure_closing_line_for_side(doc, elem_idx, LineSide::Right) {
            any_changed = true;
        }
    }
    any_changed
}

/// Automatically synchronize the closing horizontal row (`En / İğne`) on every element and side
/// whenever any line changes so every contour stays mathematically closed (`sum_x == 0`),
/// keeping any user-chosen function on the closing line (default `LineFunction::None`).
pub fn auto_sync_closing_lines(doc: &mut FormDocument) -> bool {
    let mut any_changed = false;
    for elem_idx in 0..doc.elements.len() {
        if auto_sync_closing_line_for_side(doc, elem_idx, LineSide::Left) {
            any_changed = true;
        }
        let sync_right = doc
            .elements
            .get(elem_idx)
            .map(|el| !el.mirrored && !el.right_lines.is_empty())
            .unwrap_or(false);
        if sync_right && auto_sync_closing_line_for_side(doc, elem_idx, LineSide::Right) {
            any_changed = true;
        }
    }
    any_changed
}

fn set_active_module_width_on_list(modules: &mut Vec<ModuleAssignment>, width: i32) {
    if let Some(m) = modules.iter_mut().find(|m| m.module_art_id == 0) {
        m.width = width;
    } else if let Some(m) = modules.iter_mut().find(|m| m.module_art_id >= 0) {
        m.width = width;
    } else if let Some(m) = modules.first_mut() {
        m.width = width;
    } else {
        modules.push(ModuleAssignment {
            module_art_id: 0,
            width,
            ..ModuleAssignment::default()
        });
    }
}

/// Apply global standard widths & height (`Standart özellikler` -> `çevir al`) across ALL elements
/// and sides in `doc`:
/// - Updates `doc.guidelines.std_narrow_w` / `std_widen_w`.
/// - Updates `[Mindern]` / `[Zunehmen]` module widths AND `[KurvenLinie]` rows (`MindernBreite`,
///   `ZunehmenBreite`) on `Narrowing`/`NarrowingMulti` (`reglan_w`) and `Widening` (`artirma_w`) lines.
/// - Updates vertical step height (`StufungY` / `boy_h`) on straight vertical lines (`stitches == 0 && rows != 0`).
/// - Syncs mirrored right lines automatically.
pub fn apply_standard_widths_to_all(
    doc: &mut FormDocument,
    reglan_w: i32,
    artirma_w: i32,
    boy_h: i32,
) -> usize {
    let nw = reglan_w.max(0);
    let ww = artirma_w.max(0);
    let bh = boy_h.max(0);

    doc.guidelines.std_narrow_w = nw;
    doc.guidelines.std_widen_w = ww;

    let mut modified_count = 0usize;
    let elem_count = doc.elements.len();

    for ei in 0..elem_count {
        let is_mirrored = doc.elements[ei].mirrored;
        let is_basic = doc.elements[ei].element_type() == ElementType::BasicShape;
        let sides: &[LineSide] = if is_mirrored {
            &[LineSide::Left]
        } else {
            &[LineSide::Left, LineSide::Right]
        };

        let mut elem_changed = false;
        for &side in sides {
            let lines = doc.elements[ei].lines_mut(side);
            for (r_idx, line) in lines.iter_mut().enumerate() {
                if (is_basic && r_idx == 0)
                    || line.function == LineFunction::BaseLine
                    || (r_idx > 0 && line.rows == 0 && line.default_widening == 1)
                {
                    continue;
                }

                let mut line_changed = false;
                match line.function {
                    LineFunction::Narrowing | LineFunction::NarrowingMulti => {
                        let nar = line.narrowing.get_or_insert_with(Default::default);
                        set_active_module_width_on_list(&mut nar.modules, nw);
                        line.ensure_child_slot(KanteChildSlot::Narrowing);
                        line.default_narrowing = true;
                        if let Some(ref mut cv) = line.curve {
                            for st in &mut cv.steps {
                                st.narrowing_width = nw;
                                st.widening_width = 0;
                            }
                        }
                        line_changed = true;
                    }
                    LineFunction::Widening => {
                        let wid = line.widening.get_or_insert_with(Default::default);
                        set_active_module_width_on_list(&mut wid.modules, ww);
                        line.ensure_child_slot(KanteChildSlot::Widening);
                        if let Some(ref mut cv) = line.curve {
                            for st in &mut cv.steps {
                                st.widening_width = ww;
                                st.narrowing_width = 0;
                            }
                        }
                        line_changed = true;
                    }
                    _ if line.stitches == 0 && line.rows != 0 && bh > 0 && !line.is_curve() => {
                        calculate_and_set_stufung(line, 0, bh);
                        line.default_height = 1;
                        line_changed = true;
                    }
                    _ => {}
                }

                if line_changed {
                    modified_count += 1;
                    elem_changed = true;
                }
            }
        }

        if elem_changed && is_mirrored {
            sync_mirrored_right_lines(doc, ei);
        }
    }

    if modified_count > 0 {
        doc.dirty = true;
    }
    modified_count
}

