pub mod curves;
pub mod geometry;
pub mod operations;
pub mod units;

pub use curves::{
    apply_curve_to_line, check_curve_feasibility, compress_pairs_to_steps, curve_steps_sum,
    curve_steps_sum_slice, detect_curve_formula_from_steps, detect_even_courses_from_steps,
    expand_curve_steps, extract_curve_module_widths, generate_curve_steps,
    mirror_curve_steps_single_system, normalize_curve_groups, resample_user_curve,
    resolve_curve_module_widths, CourseParityMode, CurveFeasibility, CurveGenParams,
    CurveStepDrive, DetectedCurveFormula, DEFAULT_STD_FADEOUT_WIDTH, DEFAULT_STD_NARROW_WIDTH,
    DEFAULT_STD_WIDEN_WIDTH,
};
pub use geometry::{
    compute_range_summary, element_distance_from_top, evaluate_document_geometry,
    set_element_distance_from_bottom, set_element_distance_from_top, snapshot_endline_distances,
    sync_all_endline_elements, sync_endline_element_y, validate_document, DocumentGeometry,
    ElementGeometry, RangeSummary, SegmentSpan, ValidationIssue, ValidationReport,
    ValidationSeverity, Vec2,
};
pub use operations::{
    add_element, append_line, apply_directives_to_lines, apply_guidelines,
    apply_standard_preset_to_line, apply_standard_widths_to_all, auto_sync_closing_line_for_side,
    auto_sync_closing_lines, closing_line_index, copy_line, copy_markers, delete_element,
    delete_line, element_auto_detects_shaping, element_has_base_line, ensure_all_closing_lines,
    ensure_closing_line_for_side, import_external_table_lines, insert_line_before,
    multiply_marking_points, parse_external_table_lines, paste_insert_line, paste_insert_markers,
    paste_line, paste_replace_markers, recalc_all_auto_line_functions, renumber_points,
    swap_left_right, sync_all_mirrored_elements, sync_mirrored_right_lines,
    update_or_append_closing_line,
};
pub use units::{
    auto_detect_line_function, auto_detect_line_function_for_element,
    auto_detect_line_function_in_context, calculate_and_set_stufung, convert_input_format,
    mm_to_rows, mm_to_stitches, recalc_document_for_density_change, recalc_line_from_mm,
    recalc_line_from_mm_for_side, recalc_line_from_step_edit,
    recalc_line_from_step_edit_for_element, recalc_line_from_steps,
    recalc_line_from_stitches_and_rows, recalc_line_from_stitches_and_rows_for_side, round2,
    rows_to_mm, stitches_to_mm, stoll_round,
};
