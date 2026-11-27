use crate::model::{
    CurveForm, ElementType, FormDocument, FormHeader, InputFormat, LineFunction, LineSegment,
    LineSide,
};

/// Stoll round-half-away-from-zero (`formaccess.dll`).
#[inline]
pub fn stoll_round(v: f64) -> i32 {
    if v > 0.0 {
        (v + 0.5) as i32
    } else if v < 0.0 {
        (v - 0.5) as i32
    } else {
        0
    }
}

/// Match the sign of `target` onto `val` (keeping zero as zero).
#[inline]
fn sign_match(val: i32, target: i32) -> i32 {
    if val == 0 {
        0
    } else if target < 0 {
        -val.abs()
    } else if target > 0 {
        val.abs()
    } else {
        val
    }
}

/// Convert millimeters to stitches using `MaschenDichteX` and `MaschenDichteAuf`.
pub fn mm_to_stitches(mm: f64, header: &FormHeader) -> i32 {
    if header.density_base_mm.abs() < 1e-9 {
        return 0;
    }
    stoll_round(mm * header.density_x / header.density_base_mm)
}

/// Convert millimeters to rows using `MaschenDichteY` and `MaschenDichteAuf`.
pub fn mm_to_rows(mm: f64, header: &FormHeader) -> i32 {
    if header.density_base_mm.abs() < 1e-9 {
        return 0;
    }
    stoll_round(mm * header.density_y / header.density_base_mm)
}

/// Convert stitches to millimeters using `MaschenDichteX` and `MaschenDichteAuf`.
pub fn stitches_to_mm(stitches: i32, header: &FormHeader) -> f64 {
    if header.density_x.abs() < 1e-9 {
        return 0.0;
    }
    (stitches as f64) * header.density_base_mm / header.density_x
}

/// Convert rows to millimeters using `MaschenDichteY` and `MaschenDichteAuf`.
pub fn rows_to_mm(rows: i32, header: &FormHeader) -> f64 {
    if header.density_y.abs() < 1e-9 {
        return 0.0;
    }
    (rows as f64) * header.density_base_mm / header.density_y
}

/// Exact disassembled `M3_FormLinie::CalculateAndSetStufung(in_sx, in_sy)` (`formaccess.dll @ RVA 0x575A0`).
/// Keeps `line.stitches` (N) and `line.rows` (R) fixed and computes `step_x`, `step_y`, `step_factor`, `rest_x`, `rest_y`.
pub fn calculate_and_set_stufung(line: &mut LineSegment, in_sx: i32, in_sy: i32) {
    let n = line.stitches;
    let r = line.rows;

    if in_sx == 0 && in_sy == 0 {
        line.step_factor = 1;
        line.step_x = n;
        line.step_y = r;
        line.rest_x = 0;
        line.rest_y = 0;
        return;
    }

    if in_sx == 0 {
        let f = (r / in_sy).abs().max(1);
        let sx = sign_match(n / f, n);
        let sy = sign_match(in_sy, r);
        line.step_factor = f;
        line.step_x = sx;
        line.step_y = sy;
        line.rest_x = n - sx * f;
        line.rest_y = r - sy * f;
        return;
    }

    if in_sy == 0 {
        let f = (n / in_sx).abs().max(1);
        let sx = sign_match(in_sx, n);
        let sy = sign_match(r / f, r);
        line.step_factor = f;
        line.step_x = sx;
        line.step_y = sy;
        line.rest_x = n - sx * f;
        line.rest_y = r - sy * f;
        return;
    }

    // Both non-zero
    let f = (n / in_sx).abs().max(1);
    let sx = sign_match(in_sx, n);
    let sy = sign_match(in_sy, r);
    line.step_factor = f;
    line.step_x = sx;
    line.step_y = sy;
    line.rest_x = n - sx * f;
    line.rest_y = r - sy * f;
}

/// Auto-detect `Funktion` for a line when `user_set_function == false` (per `MATH_AND_CURVES_SPEC.md` Section 3.3).
pub fn auto_detect_line_function(line: &mut LineSegment, is_first_row: bool, is_left_side: bool) {
    auto_detect_line_function_for_element(line, is_first_row, is_left_side, ElementType::BasicShape);
}

/// Element-aware `Funktion` auto-detection:
/// - On `BasicShape` (and non-cutouts): Row 1 (`is_first_row`) stays `BaseLine` (`Temel`).
/// - On cutouts (`Neckline`, `Armhole`, `Hole`, `Opening`): Row 1 horizontal is `BindOff` (`Remayözleme`),
///   and moving outward from cutout center narrows the knitted panel (`Narrowing`) while moving inward widens it (`Widening`).
/// - Closing horizontal rows (`!is_first_row && line.rows == 0 && line.last_line != 0`) stay `LineFunction::None`.
pub fn auto_detect_line_function_for_element(
    line: &mut LineSegment,
    is_first_row: bool,
    is_left_side: bool,
    elem_type: ElementType,
) {
    if line.user_set_function {
        return;
    }
    let has_base = crate::math::operations::element_has_base_line(elem_type);
    if !crate::math::operations::element_auto_detects_shaping(elem_type) {
        line.function = LineFunction::None;
        return;
    }
    let is_cutout = matches!(
        elem_type,
        ElementType::Neckline | ElementType::Armhole | ElementType::Hole | ElementType::Opening
    );
    if has_base && is_first_row {
        if line.rows == 0 {
            line.function = LineFunction::BaseLine;
            return;
        }
        if line.function == LineFunction::BaseLine {
            return;
        }
    }
    if is_cutout && is_first_row && line.rows == 0 && line.stitches != 0 {
        line.function = LineFunction::BindOff;
        return;
    }
    if !is_first_row && line.rows == 0 && line.default_widening == 1 {
        line.function = LineFunction::None;
        return;
    }
    if line.stitches == 0 {
        line.function = LineFunction::None;
        return;
    }
    // On BasicShape left side: +dX moves inward toward center (Narrowing), -dX moves outward (Widening).
    // On Cutout left side: -dX widens the cutout (Narrowing knitted panel), +dX narrows cutout (Widening panel).
    let inward = if is_cutout {
        if is_left_side {
            line.stitches < 0
        } else {
            line.stitches > 0
        }
    } else if is_left_side {
        line.stitches > 0
    } else {
        line.stitches < 0
    };
    if inward {
        line.function = LineFunction::Narrowing;
    } else {
        line.function = LineFunction::Widening;
    }
}

/// Context-aware `Funktion` auto-detection that keeps closing horizontal rows (`is_closing_row`)
/// at `LineFunction::None` unless `line.user_set_function == true`.
pub fn auto_detect_line_function_in_context(
    line: &mut LineSegment,
    is_first_row: bool,
    is_closing_row: bool,
    is_left_side: bool,
    elem_type: ElementType,
) {
    if line.user_set_function {
        return;
    }
    if !is_first_row && is_closing_row && line.rows == 0 {
        line.function = LineFunction::None;
        return;
    }
    auto_detect_line_function_for_element(line, is_first_row, is_left_side, elem_type);
}

/// Recalculate `width_mm`, `height_mm`, and steps (or regenerate `[Kurve]` steps)
/// after editing `line.stitches` or `line.rows` (`Boy İlmekler` / `En İlmekler`).
pub fn recalc_line_from_stitches_and_rows(line: &mut LineSegment, header: &FormHeader) {
    let inferred_side = if line.stitches < 0
        && matches!(
            line.function,
            LineFunction::Narrowing | LineFunction::NarrowingMulti
        ) {
        LineSide::Right
    } else {
        LineSide::Left
    };
    recalc_line_from_stitches_and_rows_for_side(line, header, inferred_side);
}

/// Side-aware variant of `recalc_line_from_stitches_and_rows` so `SingleSystem` (`Tek Sistem`)
/// parity and detected custom curve parameters (`En basamak`, `Boy basamak`, `Çift May`, `Min May`)
/// are preserved intact when line dimensions change.
pub fn recalc_line_from_stitches_and_rows_for_side(
    line: &mut LineSegment,
    header: &FormHeader,
    side: LineSide,
) {
    line.width_mm = round2(stitches_to_mm(line.stitches, header));
    line.height_mm = round2(rows_to_mm(line.rows, header));

    if let Some(ref curve) = line.curve {
        if !curve.steps.is_empty() {
            if line.stitches == 0 && line.rows == 0 {
                line.curve = None;
                line.step_x = 0;
                line.step_y = 0;
                line.step_factor = 1;
                line.rest_x = 0;
                line.rest_y = 0;
                return;
            }
            let (nw, ww, fw) = crate::math::curves::extract_curve_module_widths(line);
            let detected = crate::math::curves::detect_curve_formula_from_steps(
                curve,
                side,
                line.single_system_curve,
            );
            let (new_steps, save_form_id) = if let Some(det) = detected {
                let parity_mode = if det.single_system || line.single_system_curve {
                    match side {
                        LineSide::Left => crate::math::curves::CourseParityMode::SingleSystemLeftOdd,
                        LineSide::Right => {
                            crate::math::curves::CourseParityMode::SingleSystemRightEven
                        }
                    }
                } else if det.even_courses {
                    crate::math::curves::CourseParityMode::EvenCourses
                } else {
                    crate::math::curves::CourseParityMode::Any
                };
                let generated = crate::math::curves::generate_curve_steps(
                    &crate::math::curves::CurveGenParams {
                        curve_form: det.form,
                        total_dx: line.stitches,
                        total_dy: line.rows,
                        step_x: if line.stitches < 0 {
                            -det.step_x_mag
                        } else {
                            det.step_x_mag
                        },
                        step_y: if line.rows < 0 {
                            -det.step_y_mag
                        } else {
                            det.step_y_mag
                        },
                        narrowing_width: nw,
                        widening_width: ww,
                        fade_out_width: fw,
                        parity_mode,
                        step_drive: det.step_drive,
                        reverse_j: det.reverse_j,
                        min_step_height: det.min_step_height,
                    },
                );
                let is_custom = det.reverse_j
                    || det.even_courses
                    || det.single_system
                    || det.min_step_height > 0
                    || det.step_drive == crate::math::curves::CurveStepDrive::ByHeight
                    || det.step_x_mag != 1;
                let fid = if is_custom {
                    CurveForm::UserCurve.to_id()
                } else {
                    det.form.to_id()
                };
                (generated, fid)
            } else {
                (
                    crate::math::curves::resample_user_curve(curve, line.stitches, line.rows),
                    curve.function,
                )
            };
            if let Some(ref mut c) = line.curve {
                c.function = save_form_id;
                c.steps = new_steps;
                for (i, s) in c.steps.iter_mut().enumerate() {
                    s.index = (i + 1) as i32;
                    s.tag_id = s.index;
                    s.raw.original_header = Some(format!("[KurvenLinie] = {}", s.index));
                }
            }
            // Stoll zeroes step fields when [Kurve] is active
            line.step_x = 0;
            line.step_y = 0;
            line.step_factor = 0;
            line.rest_x = 0;
            line.rest_y = 0;
            return;
        }
    }

    if line.stitches == 0 && line.rows == 0 {
        calculate_and_set_stufung(line, 0, 0);
    } else if line.stitches == 0 && line.rows != 0 {
        if line.step_factor > 1 && line.step_y != 0 && line.step_x == 0 {
            let sy = line.step_y;
            calculate_and_set_stufung(line, 0, sy);
        } else {
            calculate_and_set_stufung(line, 0, 0);
        }
    } else if line.rows == 0 {
        calculate_and_set_stufung(line, 0, 0);
    } else if line.step_factor > 1 && (line.step_x != 0 || line.step_y != 0) {
        let sx = line.step_x;
        let sy = line.step_y;
        calculate_and_set_stufung(line, sx, sy);
    } else {
        calculate_and_set_stufung(line, 0, 0);
    }
}

/// Recalculate `stitches`, `rows`, and steps after editing `line.width_mm` or `line.height_mm`.
pub fn recalc_line_from_mm(line: &mut LineSegment, header: &FormHeader) {
    let saved_w = line.width_mm;
    let saved_h = line.height_mm;
    line.stitches = mm_to_stitches(saved_w, header);
    line.rows = mm_to_rows(saved_h, header);
    recalc_line_from_stitches_and_rows(line, header);
    if header.input_format == InputFormat::Mm {
        line.width_mm = round2(saved_w);
        line.height_mm = round2(saved_h);
    }
}

/// Side-aware variant of `recalc_line_from_mm`.
pub fn recalc_line_from_mm_for_side(line: &mut LineSegment, header: &FormHeader, side: LineSide) {
    let saved_w = line.width_mm;
    let saved_h = line.height_mm;
    line.stitches = mm_to_stitches(saved_w, header);
    line.rows = mm_to_rows(saved_h, header);
    recalc_line_from_stitches_and_rows_for_side(line, header, side);
    if header.input_format == InputFormat::Mm {
        line.width_mm = round2(saved_w);
        line.height_mm = round2(saved_h);
    }
}

/// Recalculate `stitches`, `rows`, `width_mm`, `height_mm` after editing `step_x`, `step_y`,
/// `step_factor`, `rest_x`, or `rest_y` (`Boy Basamak`, `En Basamak`, `Faktör`, `Arta kalan`).
pub fn recalc_line_from_steps(line: &mut LineSegment, header: &FormHeader) {
    if line.step_factor < 1 {
        line.step_factor = 1;
    }
    line.stitches = line.step_x * line.step_factor + line.rest_x;
    line.rows = line.step_y * line.step_factor + line.rest_y;
    line.width_mm = round2(stitches_to_mm(line.stitches, header));
    line.height_mm = round2(rows_to_mm(line.rows, header));
}

/// Exact Stoll recalculation rule after editing `StepX`, `StepY`, or `Factor` in the Lines grid (`[ENG-06]`):
/// - If `edited_factor` or `header.input_format == InputFormat::Stufung`: recalculates `stitches`/`rows` from steps (`recalc_line_from_steps`) and updates auto-detected `Funktion`.
/// - Otherwise (`İlmekler` / `mm` mode): keeps `stitches` (N) and `rows` (R) constant and calls `calculate_and_set_stufung(line, line.step_x, line.step_y)`.
pub fn recalc_line_from_step_edit(
    line: &mut LineSegment,
    header: &FormHeader,
    edited_factor: bool,
    is_first_row: bool,
    is_left_side: bool,
) {
    recalc_line_from_step_edit_for_element(
        line,
        header,
        edited_factor,
        is_first_row,
        false,
        is_left_side,
        ElementType::BasicShape,
    );
}

/// Element-aware and closing-row-aware variant of `recalc_line_from_step_edit`.
pub fn recalc_line_from_step_edit_for_element(
    line: &mut LineSegment,
    header: &FormHeader,
    edited_factor: bool,
    is_first_row: bool,
    is_closing_row: bool,
    is_left_side: bool,
    elem_type: ElementType,
) {
    if edited_factor || header.input_format == InputFormat::Stufung {
        recalc_line_from_steps(line, header);
        auto_detect_line_function_in_context(
            line,
            is_first_row,
            is_closing_row,
            is_left_side,
            elem_type,
        );
    } else {
        let sx = line.step_x;
        let sy = line.step_y;
        calculate_and_set_stufung(line, sx, sy);
    }
}

/// Recalculate all lines & element offsets across a document when stitch density (`MaschenDichteX/Y/Auf`) changes.
pub fn recalc_document_for_density_change(doc: &mut FormDocument) {
    let hdr = doc.header.clone();
    for el in &mut doc.elements {
        match hdr.input_format {
            InputFormat::Mm => {
                el.offset_x_stitches = mm_to_stitches(el.offset_x_mm, &hdr) as f64;
                el.offset_x_stitches_base = mm_to_stitches(el.offset_x_mm_base, &hdr) as f64;
                el.offset_y_rows = mm_to_rows(el.offset_y_mm, &hdr) as f64;
                for line in &mut el.left_lines {
                    recalc_line_from_mm_for_side(line, &hdr, LineSide::Left);
                }
                for line in &mut el.right_lines {
                    recalc_line_from_mm_for_side(line, &hdr, LineSide::Right);
                }
            }
            InputFormat::Steps | InputFormat::Stufung | InputFormat::Stitches => {
                el.offset_x_mm = round2(stitches_to_mm(el.offset_x_stitches.round() as i32, &hdr));
                el.offset_x_mm_base =
                    round2(stitches_to_mm(el.offset_x_stitches_base.round() as i32, &hdr));
                el.offset_y_mm = round2(rows_to_mm(el.offset_y_rows.round() as i32, &hdr));
                for line in el.left_lines.iter_mut().chain(el.right_lines.iter_mut()) {
                    line.width_mm = round2(stitches_to_mm(line.stitches, &hdr));
                    line.height_mm = round2(rows_to_mm(line.rows, &hdr));
                }
            }
        }
    }
    doc.dirty = true;
}

/// Convert the document's `EingabeFormat` (`Steps => Stitches`, `Stitches => Steps`, `mm <=> Stitches`).
pub fn convert_input_format(doc: &mut FormDocument, target: InputFormat) {
    if doc.header.input_format == target {
        return;
    }
    let endline_dists = crate::math::geometry::snapshot_endline_distances(doc);
    let old = doc.header.input_format;
    doc.header.input_format = target;
    let hdr = doc.header.clone();

    for el in &mut doc.elements {
        match (old, target) {
            (InputFormat::Mm, InputFormat::Stitches | InputFormat::Steps | InputFormat::Stufung) => {
                el.offset_x_stitches = mm_to_stitches(el.offset_x_mm, &hdr) as f64;
                el.offset_x_stitches_base = mm_to_stitches(el.offset_x_mm_base, &hdr) as f64;
                el.offset_y_rows = mm_to_rows(el.offset_y_mm, &hdr) as f64;
            }
            (InputFormat::Steps | InputFormat::Stufung | InputFormat::Stitches, InputFormat::Mm) => {
                el.offset_x_mm = round2(stitches_to_mm(el.offset_x_stitches.round() as i32, &hdr));
                el.offset_x_mm_base =
                    round2(stitches_to_mm(el.offset_x_stitches_base.round() as i32, &hdr));
                el.offset_y_mm = round2(rows_to_mm(el.offset_y_rows.round() as i32, &hdr));
            }
            _ => {}
        }
        for (side, lines) in [
            (LineSide::Left, &mut el.left_lines),
            (LineSide::Right, &mut el.right_lines),
        ] {
            for line in lines.iter_mut() {
                match (old, target) {
                    (InputFormat::Mm, InputFormat::Stitches | InputFormat::Steps | InputFormat::Stufung) => {
                        recalc_line_from_mm_for_side(line, &hdr, side);
                    }
                    (InputFormat::Steps | InputFormat::Stufung | InputFormat::Stitches, InputFormat::Mm) => {
                        line.width_mm = round2(stitches_to_mm(line.stitches, &hdr));
                        line.height_mm = round2(rows_to_mm(line.rows, &hdr));
                    }
                    (InputFormat::Steps | InputFormat::Stufung, InputFormat::Stitches) => {
                        if !line.is_curve()
                            && line.step_factor >= 1
                            && (line.step_x != 0 || line.step_y != 0 || line.rest_x != 0 || line.rest_y != 0)
                        {
                            line.stitches = line.step_x * line.step_factor + line.rest_x;
                            line.rows = line.step_y * line.step_factor + line.rest_y;
                        }
                        line.width_mm = round2(stitches_to_mm(line.stitches, &hdr));
                        line.height_mm = round2(rows_to_mm(line.rows, &hdr));
                    }
                    (InputFormat::Stitches, InputFormat::Steps | InputFormat::Stufung) => {
                        recalc_line_from_stitches_and_rows_for_side(line, &hdr, side);
                    }
                    _ => {}
                }
            }
        }
    }
    crate::math::geometry::sync_all_endline_elements(doc, &endline_dists);
    doc.dirty = true;
}

#[inline]
pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::curves::{
        apply_curve_to_line, curve_steps_sum_slice, detect_curve_formula_from_steps,
        generate_curve_steps, CourseParityMode, CurveGenParams,
    };

    #[test]
    fn test_recalc_line_preserves_curve_formula_params() {
        let hdr = FormHeader::default();
        let mut line = LineSegment {
            index: 2,
            tag_id: 2,
            stitches: 22,
            rows: 40,
            function: LineFunction::Narrowing,
            ..Default::default()
        };
        let steps = generate_curve_steps(&CurveGenParams {
            curve_form: CurveForm::JCurve,
            total_dx: 22,
            total_dy: 40,
            step_x: 2,
            narrowing_width: 6,
            parity_mode: CourseParityMode::EvenCourses,
            ..Default::default()
        });
        apply_curve_to_line(&mut line, &hdr, CurveForm::UserCurve, steps);

        line.stitches = 26;
        line.rows = 48;
        recalc_line_from_stitches_and_rows_for_side(&mut line, &hdr, LineSide::Left);

        let curve = line.curve.as_ref().expect("curve retained");
        assert_eq!(curve.function, CurveForm::UserCurve.to_id());
        assert_eq!(curve_steps_sum_slice(&curve.steps), (26, 48));
        assert!(curve.steps.iter().all(|s| s.step_width == 2 && s.step_height % 2 == 0));
        let det = detect_curve_formula_from_steps(curve, LineSide::Left, false).unwrap();
        assert_eq!(det.form, CurveForm::JCurve);
        assert_eq!(det.step_x_mag, 2);
        assert!(det.even_courses);
    }
}
