use crate::math::units::{round2, rows_to_mm, stitches_to_mm, stoll_round};
use crate::model::{
    CurveData, CurveForm, CurveStep, FormHeader, KanteChildSlot, LineFunction, LineSegment,
};

/// Default document standard narrowing module width (`MindernBreite`).
pub const DEFAULT_STD_NARROW_WIDTH: i32 = 6;
/// Default document standard widening module width (`ZunehmenBreite`).
pub const DEFAULT_STD_WIDEN_WIDTH: i32 = 1;
/// Default document standard fade-out module width (`AusblendenBreite`).
pub const DEFAULT_STD_FADEOUT_WIDTH: i32 = 0;

/// Course-height parity constraint for generated curve steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CourseParityMode {
    /// Normal Stoll formula (`1, 2, 3...` any integer courses).
    #[default]
    Any,
    /// `Çift May`: all generated curve step heights are even (`2, 4, 6...`).
    EvenCourses,
    /// `Tek Sistem` (Left side): shaping steps land on odd courses (`1, 3, 5...`),
    /// providing a 1-course offset relative to Right's even courses (`2, 4, 6...`).
    SingleSystemLeftOdd,
    /// `Tek Sistem` (Right side): shaping steps land on even courses (`2, 4, 6...`).
    SingleSystemRightEven,
}

/// Driving axis for curve step generation (`En Basamak` vs `Boy Basamak`, mutually exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveStepDrive {
    /// Step by horizontal stitch width (`En Basamak`, `step_x`).
    #[default]
    ByWidth,
    /// Step by vertical course height (`Boy Basamak`, `step_y`).
    ByHeight,
}

/// Exact Stoll `formaccess.dll` curve generator parameters.
#[derive(Debug, Clone)]
pub struct CurveGenParams {
    pub curve_form: CurveForm,
    /// Total horizontal delta in stitches (`NadelZahl`, signed).
    pub total_dx: i32,
    /// Total vertical delta in rows (`Reihen`, signed, typically >= 0).
    pub total_dy: i32,
    /// Horizontal step size (`step_x`, same sign as `total_dx`; if 0 and `step_y == 0`, defaults to `sign(total_dx)`).
    pub step_x: i32,
    /// Vertical step size (`step_y`) used when `step_drive == CurveStepDrive::ByHeight` (or `step_x == 0 && step_y != 0`).
    pub step_y: i32,
    /// Preserved step-level narrowing module width (`MindernBreite`, Kural 45).
    pub narrowing_width: i32,
    /// Preserved step-level widening module width (`ZunehmenBreite`).
    pub widening_width: i32,
    /// Preserved step-level fade-out/edge width (`AusblendenBreite`).
    pub fade_out_width: i32,
    /// Course parity constraint (`Any`, `EvenCourses`, `SingleSystemLeftOdd`, `SingleSystemRightEven`).
    pub parity_mode: CourseParityMode,
    /// Mutually exclusive driving mode (`ByWidth` = `En Basamak`, `ByHeight` = `Boy Basamak`).
    pub step_drive: CurveStepDrive,
    /// When true, generates mirrored/symmetric `Ters J` (`y(x) = dy - J(dx - x)`).
    pub reverse_j: bool,
    /// Minimum vertical step height (`0` = disabled / `-`, `1..=9` = no step may have `|step_height| < min_step_height`).
    pub min_step_height: i32,
}

/// Feasibility result for `min_step_height` (`Min May`) under strict `En Basamağı` (`step_x`) preservation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurveFeasibility {
    pub feasible: bool,
    pub num_steps: i32,
    pub effective_min_height: i32,
    pub required_min_dy: i32,
    pub actual_dy: i32,
    pub warning_tr: Option<String>,
}

/// Detect whether an existing `[Kurve]` step list uses `Çift May` (all non-zero `step_height` are even)
/// without needing any extra non-Stoll fields in `.shp`.
pub fn detect_even_courses_from_steps(steps: &[CurveStep], single_system: bool) -> bool {
    if single_system {
        return false;
    }
    let mut has_nonzero = false;
    for s in steps {
        if s.step_height != 0 {
            has_nonzero = true;
            if s.step_height % 2 != 0 {
                return false;
            }
        }
    }
    has_nonzero
}

/// Check whether `params.min_step_height` (`Min May`) can be satisfied while strictly preserving
/// `En Basamağı` (`step_x`) and `CourseParityMode` without merging/altering `step_x`.
pub fn check_curve_feasibility(params: &CurveGenParams) -> CurveFeasibility {
    let abs_dx = params.total_dx.abs();
    let abs_dy = params.total_dy.abs();
    let min_h = params.min_step_height;

    if min_h <= 0 || abs_dx == 0 || abs_dy == 0 {
        return CurveFeasibility {
            feasible: true,
            num_steps: 0,
            effective_min_height: min_h.max(0),
            required_min_dy: 0,
            actual_dy: abs_dy,
            warning_tr: None,
        };
    }

    let drive_by_height = params.step_drive == CurveStepDrive::ByHeight
        || (params.step_x == 0 && params.step_y != 0);

    let even_mode = matches!(
        params.parity_mode,
        CourseParityMode::EvenCourses | CourseParityMode::SingleSystemRightEven
    );

    if drive_by_height {
        let (eff_min, required_dy) = if params.parity_mode == CourseParityMode::SingleSystemLeftOdd {
            let first_min = if min_h % 2 == 0 { min_h + 1 } else { min_h };
            let trailing = if params.total_dy % 2 == 0 { 1 } else { 0 };
            (first_min, first_min + trailing)
        } else {
            let em = if even_mode && min_h % 2 != 0 { min_h + 1 } else { min_h };
            (em, em)
        };
        let feasible = required_dy <= abs_dy;
        let warning_tr = if feasible {
            None
        } else {
            Some(format!(
                "Min May = {} için en az {} may boy gerekir (mevcut: {}). Boyu uzatın veya Min May değerini düşürün.",
                min_h, required_dy, abs_dy
            ))
        };
        return CurveFeasibility {
            feasible,
            num_steps: 1,
            effective_min_height: eff_min,
            required_min_dy: required_dy,
            actual_dy: abs_dy,
            warning_tr,
        };
    }

    let step_x_mag = params.step_x.abs().max(1).min(abs_dx);
    let num_full = abs_dx / step_x_mag;
    let rem_x = abs_dx - num_full * step_x_mag;
    let num_steps = num_full + if rem_x != 0 { 1 } else { 0 };

    let (eff_min, required_dy) = if params.parity_mode == CourseParityMode::SingleSystemLeftOdd {
        let first_min = if min_h % 2 == 0 { min_h + 1 } else { min_h };
        let sub_min = if min_h % 2 != 0 { min_h + 1 } else { min_h };
        let trailing = if params.total_dy % 2 == 0 { 1 } else { 0 };
        let req = first_min + (num_steps - 1).max(0) * sub_min + trailing;
        (sub_min, req)
    } else {
        let em = if even_mode && min_h % 2 != 0 { min_h + 1 } else { min_h };
        (em, num_steps * em)
    };

    let feasible = required_dy <= abs_dy;
    let warning_tr = if feasible {
        None
    } else if even_mode && eff_min != min_h {
        Some(format!(
            "Bu en basamağı ({}) ve Min May = {} (Çift May: {}) için en az {} may boy gerekir (mevcut: {}). Boyu uzatın veya en basamağını artırın.",
            step_x_mag, min_h, eff_min, required_dy, abs_dy
        ))
    } else {
        Some(format!(
            "Bu en basamağı ({}) ve Min May = {} için en az {} may boy gerekir (mevcut: {}). Boyu uzatın veya en basamağını artırın.",
            step_x_mag, min_h, required_dy, abs_dy
        ))
    };

    CurveFeasibility {
        feasible,
        num_steps,
        effective_min_height: eff_min,
        required_min_dy: required_dy,
        actual_dy: abs_dy,
        warning_tr,
    }
}

impl Default for CurveGenParams {
    fn default() -> Self {
        Self {
            curve_form: CurveForm::Line,
            total_dx: 0,
            total_dy: 0,
            step_x: 1,
            step_y: 0,
            narrowing_width: 0,
            widening_width: 0,
            fade_out_width: 0,
            parity_mode: CourseParityMode::Any,
            step_drive: CurveStepDrive::ByWidth,
            reverse_j: false,
            min_step_height: 0,
        }
    }
}

fn active_module_width(modules: &[crate::model::ModuleAssignment]) -> i32 {
    modules
        .iter()
        .find(|m| m.module_art_id == 0)
        .or_else(|| modules.iter().find(|m| m.module_art_id >= 0 && m.width > 0))
        .or_else(|| modules.iter().find(|m| m.width > 0))
        .or_else(|| modules.first())
        .map(|m| m.width)
        .unwrap_or(0)
}

/// Resolve `(MindernBreite, ZunehmenBreite, AusblendenBreite)` for a `LineSegment` using its
/// `LineFunction`, existing module/curve widths, and document standard defaults
/// `(std_narrow_w, std_widen_w, std_fadeout_w)`.
///
/// Never defaults `Narrowing`/`NarrowingMulti` (Reglan) curves to `(0, 1, 0)`.
pub fn resolve_curve_module_widths(
    line: &LineSegment,
    std_narrow_w: i32,
    std_widen_w: i32,
    std_fadeout_w: i32,
) -> (i32, i32, i32) {
    let step_first = line.curve.as_ref().and_then(|c| c.steps.first());

    let mod_nw = line
        .narrowing
        .as_ref()
        .map(|n| active_module_width(&n.modules))
        .unwrap_or(0);
    let mod_ww = line
        .widening
        .as_ref()
        .map(|w| active_module_width(&w.modules))
        .unwrap_or(0);
    let mod_fw = line
        .fade_out
        .as_ref()
        .map(|f| active_module_width(&f.modules))
        .unwrap_or(0);

    let step_nw = step_first.map(|s| s.narrowing_width).unwrap_or(0);
    let step_ww = step_first.map(|s| s.widening_width).unwrap_or(0);
    let step_fw = step_first.map(|s| s.fade_out_width).unwrap_or(0);

    match line.function {
        LineFunction::Narrowing | LineFunction::NarrowingMulti => {
            let nw = if mod_nw > 0 {
                mod_nw
            } else if step_nw > 0 {
                step_nw
            } else {
                std_narrow_w.max(0)
            };
            let fw = if mod_fw > 0 {
                mod_fw
            } else if step_fw > 0 {
                step_fw
            } else {
                std_fadeout_w.max(0)
            };
            (nw, 0, fw)
        }
        LineFunction::Widening => {
            let ww = if mod_ww > 0 {
                mod_ww
            } else if step_ww > 0 {
                step_ww
            } else {
                std_widen_w.max(0)
            };
            let fw = if mod_fw > 0 {
                mod_fw
            } else if step_fw > 0 {
                step_fw
            } else {
                std_fadeout_w.max(0)
            };
            (0, ww, fw)
        }
        LineFunction::FadeOut => {
            let fw = if mod_fw > 0 {
                mod_fw
            } else if step_fw > 0 {
                step_fw
            } else {
                std_fadeout_w.max(0)
            };
            (0, 0, fw)
        }
        _ => {
            if let Some(first) = step_first {
                (first.narrowing_width, first.widening_width, first.fade_out_width)
            } else {
                (mod_nw, mod_ww, mod_fw)
            }
        }
    }
}

/// Extract step-level module widths `(MindernBreite, ZunehmenBreite, AusblendenBreite)` from a line,
/// respecting `LineFunction` defaults (`6` for Reglan, `1` for Artırma) when unpopulated.
pub fn extract_curve_module_widths(line: &LineSegment) -> (i32, i32, i32) {
    resolve_curve_module_widths(
        line,
        DEFAULT_STD_NARROW_WIDTH,
        DEFAULT_STD_WIDEN_WIDTH,
        DEFAULT_STD_FADEOUT_WIDTH,
    )
}

/// Expand `[KurvenLinie]` steps into elementary `(dx, dy)` step pairs, repeating
/// contiguous `GruppenTagId > 0` blocks by their header row's `GruppenFaktor`.
pub fn expand_curve_steps(steps: &[CurveStep]) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < steps.len() {
        let gid = steps[i].group_tag_id;
        if gid > 0 {
            let start = i;
            while i < steps.len() && steps[i].group_tag_id == gid {
                i += 1;
            }
            let block = &steps[start..i];
            let gf = block
                .iter()
                .map(|s| s.group_factor)
                .find(|&f| f > 0)
                .unwrap_or(1);
            for _ in 0..gf {
                for s in block {
                    let reps = s.factor.max(1);
                    for _ in 0..reps {
                        out.push((s.step_width, s.step_height));
                    }
                }
            }
        } else {
            let reps = steps[i].factor.max(1);
            for _ in 0..reps {
                out.push((steps[i].step_width, steps[i].step_height));
            }
            i += 1;
        }
    }
    out
}

/// Normalize `GruppenFaktor` / `GruppenTagId` across `[KurvenLinie]` rows to match Stoll's format:
/// - First row of a contiguous `GruppenTagId > 0` block holds `GruppenFaktor = N >= 1`.
/// - Continuation rows of that block hold `GruppenFaktor = 0`.
/// - Ungrouped rows (`GruppenTagId == 0`) hold `GruppenFaktor = 1`.
pub fn normalize_curve_groups(steps: &mut [CurveStep]) {
    let mut i = 0;
    while i < steps.len() {
        let gid = steps[i].group_tag_id;
        if gid > 0 {
            let start = i;
            while i < steps.len() && steps[i].group_tag_id == gid {
                i += 1;
            }
            let gf = steps[start..i]
                .iter()
                .map(|s| s.group_factor)
                .max()
                .unwrap_or(1)
                .max(1);
            steps[start].group_factor = gf;
            for s in &mut steps[start + 1..i] {
                s.group_factor = 0;
            }
        } else {
            steps[i].group_tag_id = 0;
            steps[i].group_factor = 1;
            i += 1;
        }
    }
}

/// Compute exact `(sum_dx, sum_dy)` of a `[Kurve]` step slice accounting for `Faktor`, `GruppenTagId`, and `GruppenFaktor`.
pub fn curve_steps_sum_slice(steps: &[CurveStep]) -> (i32, i32) {
    let mut tot_dx = 0i32;
    let mut tot_dy = 0i32;
    let mut i = 0;
    while i < steps.len() {
        let gid = steps[i].group_tag_id;
        if gid > 0 {
            let start = i;
            while i < steps.len() && steps[i].group_tag_id == gid {
                i += 1;
            }
            let block = &steps[start..i];
            let gf = block
                .iter()
                .map(|s| s.group_factor)
                .find(|&f| f > 0)
                .unwrap_or(1);
            for s in block {
                let mult = s.factor.max(0) * gf;
                tot_dx += s.step_width * mult;
                tot_dy += s.step_height * mult;
            }
        } else {
            let mult = steps[i].factor.max(0);
            tot_dx += steps[i].step_width * mult;
            tot_dy += steps[i].step_height * mult;
            i += 1;
        }
    }
    (tot_dx, tot_dy)
}

/// Compute exact `(sum_dx, sum_dy)` of a `[Kurve]` accounting for `Faktor`, `GruppenTagId`, and `GruppenFaktor`.
pub fn curve_steps_sum(curve: &CurveData) -> (i32, i32) {
    curve_steps_sum_slice(&curve.steps)
}

/// Convert a Left/Right single-system (`Tek Sistem`) curve's steps to the opposite side:
/// - Negates `step_width` for mirroring.
/// - Shifts shaping course heights by `+1` course (`SingleSystemLeftOdd -> SingleSystemRightEven`)
///   or `-1` course (`SingleSystemRightEven -> SingleSystemLeftOdd`) while keeping `sum_dy` identical.
pub fn mirror_curve_steps_single_system(
    steps: &[CurveStep],
    target_right_even: bool,
) -> Vec<CurveStep> {
    if steps.is_empty() {
        return Vec::new();
    }
    let nw = steps[0].narrowing_width;
    let ww = steps[0].widening_width;
    let fw = steps[0].fade_out_width;
    let expanded = expand_curve_steps(steps);
    if expanded.is_empty() {
        return Vec::new();
    }
    let total_dy: i32 = expanded.iter().map(|&(_, dy)| dy).sum();
    if total_dy == 0 {
        let pairs: Vec<(i32, i32)> = expanded.into_iter().map(|(dx, dy)| (-dx, dy)).collect();
        return compress_pairs_to_steps(&pairs, nw, ww, fw);
    }

    let sign_y = total_dy.signum();
    let mut pairs: Vec<(i32, i32)> = Vec::with_capacity(expanded.len() + 1);
    let mut cur_src_y = 0i32;
    let mut cur_dst_y = 0i32;

    let has_shaping = expanded.iter().any(|&(dx, _)| dx != 0);
    if !has_shaping {
        let pairs: Vec<(i32, i32)> = expanded.into_iter().map(|(dx, dy)| (-dx, dy)).collect();
        return compress_pairs_to_steps(&pairs, nw, ww, fw);
    }

    let last_shaping_idx = expanded.iter().rposition(|&(dx, _)| dx != 0).unwrap();
    for (idx, (dx, dy)) in expanded.into_iter().enumerate() {
        cur_src_y += dy;
        if dx == 0 {
            continue;
        }
        let target_y = if target_right_even {
            let shifted = cur_src_y + sign_y;
            if sign_y > 0 {
                shifted.min(total_dy)
            } else {
                shifted.max(total_dy)
            }
        } else {
            let is_last = idx == last_shaping_idx;
            let shifted = cur_src_y - sign_y;
            if is_last && total_dy % 2 == 0 {
                total_dy - sign_y
            } else if sign_y > 0 {
                shifted.max(sign_y).min(total_dy)
            } else {
                shifted.min(sign_y).max(total_dy)
            }
        };
        let new_dy = target_y - cur_dst_y;
        cur_dst_y += new_dy;
        pairs.push((-dx, new_dy));
    }

    if cur_dst_y != total_dy {
        pairs.push((0, total_dy - cur_dst_y));
    }

    compress_pairs_to_steps(&pairs, nw, ww, fw)
}

#[inline]
fn round_even_with_sign(v: f64) -> i32 {
    2 * stoll_round(v * 0.5)
}

#[inline]
fn s_curve_norm(u: f64) -> f64 {
    let t = u * 9.28;
    let p = -0.018432 * t * t * t + 0.224866 * t * t + 0.592899 * t - 0.187795;
    ((p + 0.18) / 10.18).clamp(0.0, 1.0)
}

#[inline]
fn eval_norm_u_for_v(curve_form: CurveForm, reverse_j: bool, v_raw: f64) -> f64 {
    let v = v_raw.clamp(0.0, 1.0);
    if reverse_j {
        return (1.3 - (0.09 + 1.6 * (1.0 - v)).sqrt()).clamp(0.0, 1.0);
    }
    match curve_form {
        CurveForm::Line | CurveForm::Step | CurveForm::UserCurve => v,
        CurveForm::JCurve => (-0.3 + (0.09 + 1.6 * v).sqrt()).clamp(0.0, 1.0),
        CurveForm::SCurve => {
            let mut lo = 0.0f64;
            let mut hi = 1.0f64;
            for _ in 0..26 {
                let mid = 0.5 * (lo + hi);
                if s_curve_norm(mid) < v {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        }
    }
}

/// Generate `[KurvenLinie]` steps using exact disassembled `formaccess.dll` formulas:
/// - `CurveForm::Line` (1): `CalcFormula_Line` (RVA `0x51BE0`)
/// - `CurveForm::Step` (2): `CalcFormula_LineOben` (RVA `0x51FC0`)
/// - `CurveForm::JCurve` (3): `CalcFormula_J` (RVA `0x51900`) — `(x^2 + 0.6*dx*x) * dy / (1.6*dx^2)`
/// - `CurveForm::SCurve` (4): `CalcFormula_S` (RVA `0x522E0`) — exact cubic polynomial in `u = 9.28*x/dx`
/// - `CurveForm::UserCurve` (5) with `reverse_j = true`: symmetric `Ters J` (`y(x) = dy - J(dx - x)`)
/// - `CurveForm::UserCurve` (5) with `reverse_j = false`: defaults to `CalcFormula_Line` when creating fresh
///
/// Supports `CourseParityMode` (`Çift May` & `Tek Sistem`) and mutually exclusive `CurveStepDrive` (`ByWidth` / `ByHeight`).
pub fn generate_curve_steps(params: &CurveGenParams) -> Vec<CurveStep> {
    let dx = params.total_dx;
    let dy = params.total_dy;

    if dx == 0 && dy == 0 {
        return Vec::new();
    }
    if dx == 0 {
        return compress_pairs_to_steps(
            &[(0, dy)],
            params.narrowing_width,
            params.widening_width,
            params.fade_out_width,
        );
    }
    if dy == 0 {
        return compress_pairs_to_steps(
            &[(dx, 0)],
            params.narrowing_width,
            params.widening_width,
            params.fade_out_width,
        );
    }

    let drive_by_height = params.step_drive == CurveStepDrive::ByHeight
        || (params.step_x == 0 && params.step_y != 0);

    if drive_by_height {
        let feas = check_curve_feasibility(params);
        let eff_step_y = if params.min_step_height > 0 && feas.feasible {
            let sign_y = dy.signum();
            sign_y * params.step_y.abs().max(feas.effective_min_height)
        } else {
            params.step_y
        };
        let mut pairs = calc_formula_y_driven(
            params.curve_form,
            params.reverse_j,
            dx,
            dy,
            eff_step_y,
            params.parity_mode,
        );
        if params.min_step_height > 0 && feas.feasible && pairs.len() > 1 {
            // In ByHeight mode, fold only a trailing remainder smaller than effective_min_height into the previous step.
            let min_h = feas.effective_min_height;
            if let Some(&(last_dx, last_dy)) = pairs.last() {
                let is_ss_trailing = params.parity_mode == CourseParityMode::SingleSystemLeftOdd
                    && last_dx == 0
                    && last_dy.abs() == 1;
                if !is_ss_trailing && last_dy.abs() < min_h {
                    let (ldx, ldy) = pairs.pop().unwrap();
                    if let Some(prev) = pairs.last_mut() {
                        prev.0 += ldx;
                        prev.1 += ldy;
                    }
                }
            }
        }
        return compress_pairs_to_steps(
            &pairs,
            params.narrowing_width,
            params.widening_width,
            params.fade_out_width,
        );
    }

    let step_x_mag = params.step_x.abs().max(1).min(dx.abs());
    let sx = if dx < 0 { -step_x_mag } else { step_x_mag };

    let eval_curve = |x: f64, eff_dx: f64, eff_dy: f64| -> f64 {
        if params.reverse_j {
            let u = eff_dx - x;
            let k = eff_dy / (1.6 * eff_dx * eff_dx);
            eff_dy - (u * u + 0.6 * eff_dx * u) * k
        } else {
            match params.curve_form {
                CurveForm::Line | CurveForm::Step | CurveForm::UserCurve => x * (eff_dy / eff_dx),
                CurveForm::JCurve => {
                    let k = eff_dy / (1.6 * eff_dx * eff_dx);
                    (x * x + 0.6 * eff_dx * x) * k
                }
                CurveForm::SCurve => {
                    let u = x * (9.28 / eff_dx);
                    let p = -0.018432 * u * u * u + 0.224866 * u * u + 0.592899 * u - 0.187795;
                    ((p + 0.18) / 10.18) * eff_dy
                }
            }
        }
    };

    // Strict `En Basamağı` preservation with `Min May`:
    // NEVER merge adjacent steps or alter `step_x`. If feasible, distribute the excess height
    // (`total_dy - num_steps * eff_min`) via the selected formula and add `eff_min` to every step.
    // If infeasible, fall back to unmerged formula generation (and UI surfaces `check_curve_feasibility` warning).
    if params.min_step_height > 0 {
        let feas = check_curve_feasibility(params);
        if feas.feasible {
            let pairs = calc_formula_x_driven_with_min_height(
                dx,
                dy,
                sx,
                params.min_step_height,
                params.parity_mode,
                eval_curve,
            );
            return compress_pairs_to_steps(
                &pairs,
                params.narrowing_width,
                params.widening_width,
                params.fade_out_width,
            );
        }
    }

    let first_step_guard = params.reverse_j || params.curve_form != CurveForm::Step;
    let pairs = calc_formula_x_driven(dx, dy, sx, first_step_guard, params.parity_mode, eval_curve);
    let mut steps = compress_pairs_to_steps(
        &pairs,
        params.narrowing_width,
        params.widening_width,
        params.fade_out_width,
    );
    if !params.reverse_j
        && params.curve_form == CurveForm::Step
        && !steps.is_empty()
        && params.min_step_height <= 0
    {
        let dx0 = steps[0].step_width;
        let dy0 = steps[0].step_height;
        let f0 = steps[0].factor;
        if f0 > 1 {
            steps[0].factor = f0 - 1;
            let mut lead = steps[0].clone();
            lead.step_width = 0;
            lead.step_height = dy0;
            lead.factor = 1;
            steps.insert(0, lead);
        } else {
            steps[0].step_width = 0;
        }
        let mut tail = steps[0].clone();
        tail.step_width = dx0;
        tail.step_height = 0;
        tail.factor = 1;
        steps.push(tail);
        renumber_steps(&mut steps);
    }
    steps
}

/// Distribute `dy` across exact `sx` steps while guaranteeing EVERY shaping step has
/// `|step_height| >= eff_min` (and respects `CourseParityMode`), without ever merging or altering `step_x`.
/// Caller must ensure `check_curve_feasibility` is feasible.
fn calc_formula_x_driven_with_min_height<F>(
    dx: i32,
    dy: i32,
    sx: i32,
    min_step_height: i32,
    parity_mode: CourseParityMode,
    eval_y: F,
) -> Vec<(i32, i32)>
where
    F: Fn(f64, f64, f64) -> f64,
{
    let abs_dx = dx.abs();
    let abs_sx = sx.abs();
    let num_full = abs_dx / abs_sx;
    let rem_x = dx - sx * num_full;
    let num_steps = num_full + if rem_x != 0 { 1 } else { 0 };
    let sign_y = dy.signum();

    if parity_mode == CourseParityMode::SingleSystemLeftOdd {
        let first_min = if min_step_height % 2 == 0 {
            min_step_height + 1
        } else {
            min_step_height
        };
        let sub_min = if min_step_height % 2 != 0 {
            min_step_height + 1
        } else {
            min_step_height
        };
        let trailing = if dy % 2 == 0 { 1 } else { 0 };
        let target_last = dy - sign_y * trailing;
        let base_total = sign_y * (first_min + (num_steps - 1).max(0) * sub_min);
        let excess_dy = target_last - base_total;

        let mut pairs = if excess_dy == 0 {
            let mut p = Vec::with_capacity(num_steps as usize + 1);
            for _ in 0..num_full {
                p.push((sx, 0));
            }
            if rem_x != 0 {
                p.push((rem_x, 0));
            }
            p
        } else {
            calc_formula_x_driven(dx, excess_dy, sx, false, CourseParityMode::EvenCourses, eval_y)
        };

        for (idx, pair) in pairs.iter_mut().enumerate() {
            let add = if idx == 0 { first_min } else { sub_min };
            pair.1 += sign_y * add;
        }
        if trailing != 0 {
            pairs.push((0, sign_y));
        }
        return pairs;
    }

    let even_mode = matches!(
        parity_mode,
        CourseParityMode::EvenCourses | CourseParityMode::SingleSystemRightEven
    );
    let eff_min = if even_mode && min_step_height % 2 != 0 {
        min_step_height + 1
    } else {
        min_step_height
    };
    let excess_dy = dy - sign_y * (num_steps * eff_min);

    let mut pairs = if excess_dy == 0 {
        let mut p = Vec::with_capacity(num_steps as usize);
        for _ in 0..num_full {
            p.push((sx, 0));
        }
        if rem_x != 0 {
            p.push((rem_x, 0));
        }
        p
    } else {
        calc_formula_x_driven(dx, excess_dy, sx, false, parity_mode, eval_y)
    };

    for pair in &mut pairs {
        pair.1 += sign_y * eff_min;
    }
    pairs
}

/// Shared X-driven stepping loop matching `formaccess.dll` (`CalcFormula_Line`, `_J`, `_S`, `_RevJ`),
/// extended cleanly with `CourseParityMode` (`Any`, `EvenCourses`, `SingleSystemLeftOdd`, `SingleSystemRightEven`).
fn calc_formula_x_driven<F>(
    dx: i32,
    dy: i32,
    sx: i32,
    first_step_guard: bool,
    parity_mode: CourseParityMode,
    eval_y: F,
) -> Vec<(i32, i32)>
where
    F: Fn(f64, f64, f64) -> f64,
{
    let mut pairs: Vec<(i32, i32)> = Vec::new();
    let abs_dx = dx.abs();
    let abs_sx = sx.abs();
    let num_full = abs_dx / abs_sx;
    let rem_x = dx - sx * num_full;
    let sign_y = dy.signum();

    if parity_mode == CourseParityMode::SingleSystemLeftOdd {
        if dy.abs() <= 1 {
            pairs.push((dx, dy));
            return pairs;
        }
        // Target odd course for the final shaping step: dy - 1 if dy is even, else dy.
        let target_last_shaping = if dy % 2 == 0 { dy - sign_y } else { dy };
        let mut cur_x = 0i32;
        let mut cur_y = 0i32;
        let mut eff_dx = dx as f64;
        let mut eff_dy = dy as f64;
        let mut x_offset = 0.0f64;
        // Keep `even_offset` strictly EVEN so `even_offset + round_even(...)` is always even
        // and `even_y - sign_y` is guaranteed odd on every shaping step.
        let mut even_offset = 0i32;

        for i in 1..=num_full {
            let x_i = sx * i;
            let is_final = x_i.abs() == abs_dx && rem_x == 0;
            let y_i = if is_final {
                target_last_shaping
            } else {
                let mut even_y =
                    even_offset + round_even_with_sign(eval_y(x_i as f64 - x_offset, eff_dx, eff_dy));
                if i == 1 && first_step_guard && even_y == 0 && !is_final && dy.abs() >= 2 {
                    let e1 = 2 * sign_y;
                    even_y = e1;
                    x_offset = sx as f64;
                    even_offset = e1;
                    eff_dx = (dx - sx) as f64;
                    eff_dy = (dy - e1) as f64;
                }
                let odd_val = if even_y == 0 { 0 } else { even_y - sign_y };
                if sign_y > 0 {
                    odd_val.clamp(0, target_last_shaping)
                } else {
                    odd_val.clamp(target_last_shaping, 0)
                }
            };

            let mut delta_y = y_i - cur_y;
            if dy >= 0 && delta_y < 0 {
                delta_y = 0;
            } else if dy < 0 && delta_y > 0 {
                delta_y = 0;
            }
            pairs.push((sx, delta_y));
            cur_x += sx;
            cur_y += delta_y;
        }

        if rem_x != 0 {
            let delta_y = target_last_shaping - cur_y;
            let clamped_dy = if (dy >= 0 && delta_y < 0) || (dy < 0 && delta_y > 0) {
                0
            } else {
                delta_y
            };
            pairs.push((rem_x, clamped_dy));
            cur_x += rem_x;
            cur_y += clamped_dy;
        }

        if cur_x != dx {
            if let Some(last) = pairs.last_mut() {
                last.0 += dx - cur_x;
            }
        }
        if cur_y != dy {
            pairs.push((0, dy - cur_y));
        }
        return pairs;
    }

    let even_mode = matches!(
        parity_mode,
        CourseParityMode::EvenCourses | CourseParityMode::SingleSystemRightEven
    );

    let mut cur_y = 0i32;
    let mut cur_x = 0i32;
    let mut eff_dx = dx as f64;
    let mut eff_dy = dy as f64;
    let mut x_offset = 0.0f64;
    let mut y_offset = 0i32;

    for i in 1..=num_full {
        let x_i = sx * i;
        let is_final = x_i.abs() == abs_dx;

        let mut y_i = if is_final {
            dy
        } else {
            let raw_y = eval_y(x_i as f64 - x_offset, eff_dx, eff_dy);
            let rounded = if even_mode {
                round_even_with_sign(raw_y)
            } else {
                stoll_round(raw_y)
            };
            let raw_sum = y_offset + rounded;
            if dy >= 0 {
                raw_sum.clamp(0, dy)
            } else {
                raw_sum.clamp(dy, 0)
            }
        };

        if i == 1 && first_step_guard && y_i == 0 && !is_final && dy != 0 {
            let min_first = if even_mode && dy.abs() >= 2 { 2 } else { 1 };
            let y1 = sign_y * min_first;
            y_i = y1;
            x_offset = sx as f64;
            y_offset = y1;
            eff_dx = (dx - sx) as f64;
            eff_dy = (dy - y1) as f64;
        }

        let mut delta_y = y_i - cur_y;
        if dy >= 0 && delta_y < 0 {
            delta_y = 0;
        } else if dy < 0 && delta_y > 0 {
            delta_y = 0;
        }

        pairs.push((sx, delta_y));
        cur_x += sx;
        cur_y += delta_y;
    }

    if rem_x != 0 {
        let delta_y = dy - cur_y;
        pairs.push((rem_x, delta_y));
        cur_x += rem_x;
        cur_y += delta_y;
    }

    if cur_x != dx || cur_y != dy {
        if let Some(last) = pairs.last_mut() {
            last.0 += dx - cur_x;
            last.1 += dy - cur_y;
        }
    }

    pairs
}

fn calc_formula_y_driven(
    curve_form: CurveForm,
    reverse_j: bool,
    dx: i32,
    dy: i32,
    step_y: i32,
    parity_mode: CourseParityMode,
) -> Vec<(i32, i32)> {
    let abs_dy = dy.abs();
    let sign_y = dy.signum();

    if parity_mode == CourseParityMode::SingleSystemLeftOdd {
        if abs_dy <= 1 {
            return vec![(dx, dy)];
        }
        let sy_even = (step_y.abs() / 2).max(1) * 2;
        let target_last_shaping = if dy % 2 == 0 { dy - sign_y } else { dy };
        let abs_target = target_last_shaping.abs();

        // Generate odd cumulative course milestones: (sy_even - 1), +sy_even, ..., target_last_shaping
        let mut y_milestones: Vec<i32> = Vec::new();
        let first_mag = (sy_even - 1).min(abs_target).max(1);
        let mut m = first_mag;
        while m < abs_target {
            y_milestones.push(m * sign_y);
            m += sy_even;
        }
        y_milestones.push(target_last_shaping);

        let mut pairs = Vec::with_capacity(y_milestones.len() + 1);
        let mut cur_x = 0i32;
        let mut cur_y = 0i32;
        let last_idx = y_milestones.len() - 1;
        for (idx, y_i) in y_milestones.into_iter().enumerate() {
            let x_i = if idx == last_idx {
                dx
            } else {
                let v = (y_i as f64) / (target_last_shaping as f64);
                stoll_round((dx as f64) * eval_norm_u_for_v(curve_form, reverse_j, v))
            };
            let mut delta_x = x_i - cur_x;
            if dx >= 0 && delta_x < 0 {
                delta_x = 0;
            } else if dx < 0 && delta_x > 0 {
                delta_x = 0;
            }
            let delta_y = y_i - cur_y;
            pairs.push((delta_x, delta_y));
            cur_x += delta_x;
            cur_y += delta_y;
        }
        if cur_x != dx {
            if let Some(last) = pairs.last_mut() {
                last.0 += dx - cur_x;
            }
        }
        if cur_y != dy {
            pairs.push((0, dy - cur_y));
        }
        return pairs;
    }

    let even_mode = matches!(
        parity_mode,
        CourseParityMode::EvenCourses | CourseParityMode::SingleSystemRightEven
    );
    let abs_sy = if even_mode && abs_dy >= 2 {
        ((step_y.abs() / 2).max(1) * 2).min(abs_dy)
    } else {
        step_y.abs().max(1).min(abs_dy)
    };
    let sy = sign_y * abs_sy;
    let num_full = abs_dy / abs_sy;

    let mut pairs = Vec::new();
    let mut cur_x = 0i32;
    let mut cur_y = 0i32;

    for i in 1..=num_full {
        let y_i = sy * i;
        let x_i = if y_i.abs() == abs_dy {
            dx
        } else {
            let v = (y_i as f64) / (dy as f64);
            stoll_round((dx as f64) * eval_norm_u_for_v(curve_form, reverse_j, v))
        };
        let mut delta_x = x_i - cur_x;
        if dx >= 0 && delta_x < 0 {
            delta_x = 0;
        } else if dx < 0 && delta_x > 0 {
            delta_x = 0;
        }
        pairs.push((delta_x, sy));
        cur_x += delta_x;
        cur_y += sy;
    }

    if cur_y != dy || cur_x != dx {
        if cur_y != dy {
            pairs.push((dx - cur_x, dy - cur_y));
        } else if let Some(last) = pairs.last_mut() {
            last.0 += dx - cur_x;
        }
    }
    pairs
}

/// Proportionally resample a User Curve (`Kurve.Funktion = 5`) to `(new_dx, new_dy)`
/// preserving its normalized profile, intentional `StufungHoehe == 0` bind-off steps, and module widths (Kural 10 / 49 / 54).
pub fn resample_user_curve(existing: &CurveData, new_dx: i32, new_dy: i32) -> Vec<CurveStep> {
    if existing.steps.is_empty() {
        return generate_curve_steps(&CurveGenParams {
            curve_form: CurveForm::Line,
            total_dx: new_dx,
            total_dy: new_dy,
            step_x: if new_dx < 0 { -1 } else { 1 },
            ..Default::default()
        });
    }

    let nw = existing.steps[0].narrowing_width;
    let ww = existing.steps[0].widening_width;
    let fw = existing.steps[0].fade_out_width;

    let expanded = expand_curve_steps(&existing.steps);
    let (old_dx, old_dy) = curve_steps_sum(existing);
    if old_dx == 0 || old_dy == 0 || expanded.is_empty() {
        return generate_curve_steps(&CurveGenParams {
            curve_form: CurveForm::Line,
            total_dx: new_dx,
            total_dy: new_dy,
            step_x: if new_dx < 0 { -1 } else { 1 },
            narrowing_width: nw,
            widening_width: ww,
            fade_out_width: fw,
            ..Default::default()
        });
    }

    // Build cumulative normalized profile (u_k = x_k / old_dx, v_k = y_k / old_dy)
    let mut cum_x = 0i32;
    let mut cum_y = 0i32;
    let mut new_pairs: Vec<(i32, i32)> = Vec::with_capacity(expanded.len());
    let mut prev_nx = 0i32;
    let mut prev_ny = 0i32;
    let n_steps = expanded.len();

    for (idx, &(edx, edy)) in expanded.iter().enumerate() {
        cum_x += edx;
        cum_y += edy;
        let is_last = idx + 1 == n_steps;
        let target_nx = if is_last {
            new_dx
        } else {
            stoll_round((cum_x as f64 / old_dx as f64) * new_dx as f64)
        };
        let target_ny = if is_last {
            new_dy
        } else if edy == 0 {
            // Preserve intentional bind-off 0-row steps (Kural 49)
            prev_ny
        } else {
            stoll_round((cum_y as f64 / old_dy as f64) * new_dy as f64)
        };
        let dnx = target_nx - prev_nx;
        let dny = target_ny - prev_ny;
        if dnx != 0 || dny != 0 {
            new_pairs.push((dnx, dny));
            prev_nx = target_nx;
            prev_ny = target_ny;
        }
    }

    // Guarantee exact closure
    let sum_x: i32 = new_pairs.iter().map(|(x, _)| *x).sum();
    let sum_y: i32 = new_pairs.iter().map(|(_, y)| *y).sum();
    if let Some(last) = new_pairs.last_mut() {
        last.0 += new_dx - sum_x;
        last.1 += new_dy - sum_y;
    } else if new_dx != 0 || new_dy != 0 {
        new_pairs.push((new_dx, new_dy));
    }

    compress_pairs_to_steps(&new_pairs, nw, ww, fw)
}

/// Run-length compress consecutive identical `(dx, dy)` pairs into Stoll `[KurvenLinie]` structs (`Faktor >= 1`).
pub fn compress_pairs_to_steps(
    pairs: &[(i32, i32)],
    narrowing_width: i32,
    widening_width: i32,
    fade_out_width: i32,
) -> Vec<CurveStep> {
    let mut steps: Vec<CurveStep> = Vec::new();
    for &(dx, dy) in pairs {
        if dx == 0 && dy == 0 {
            continue;
        }
        if let Some(last) = steps.last_mut() {
            if last.step_width == dx && last.step_height == dy {
                last.factor += 1;
                continue;
            }
        }
        let idx = (steps.len() + 1) as i32;
        steps.push(CurveStep {
            index: idx,
            tag_id: idx,
            group_factor: 1,
            group_tag_id: 0,
            step_width: dx,
            step_height: dy,
            factor: 1,
            suppress_first_y: Some(0),
            narrowing_width,
            widening_width,
            fade_out_width,
            raw: Default::default(),
        });
    }
    steps
}

fn renumber_steps(steps: &mut [CurveStep]) {
    for (i, s) in steps.iter_mut().enumerate() {
        s.index = (i + 1) as i32;
        s.tag_id = s.index;
        if s.factor < 1 {
            s.factor = 1;
        }
        s.raw.original_header = Some(format!("[KurvenLinie] = {}", s.index));
    }
}

/// Apply generated/edited curve steps onto a `LineSegment`, setting `[Kurve].Funktion = curve_form`
/// and synchronizing `NadelZahl`, `Reihen`, `Breite`, `Höhe` (`StepsSum == Header`, Kural 54).
pub fn apply_curve_to_line(
    line: &mut LineSegment,
    header: &FormHeader,
    curve_form: CurveForm,
    mut steps: Vec<CurveStep>,
) {
    renumber_steps(&mut steps);
    let mut curve = line.curve.take().unwrap_or_default();
    curve.function = curve_form.to_id();
    curve.steps = steps;

    let (sum_dx, sum_dy) = curve_steps_sum(&curve);
    line.stitches = sum_dx;
    line.rows = sum_dy;
    line.width_mm = round2(stitches_to_mm(sum_dx, header));
    line.height_mm = round2(rows_to_mm(sum_dy, header));
    // Stoll zeroes step columns when a multi-step [Kurve] is active
    line.step_x = 0;
    line.step_y = 0;
    line.step_factor = 0;
    line.rest_x = 0;
    line.rest_y = 0;

    line.curve = Some(curve);
    line.ensure_child_slot(KanteChildSlot::Curve);
}
