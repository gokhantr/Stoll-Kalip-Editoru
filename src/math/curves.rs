use std::collections::HashMap;

use crate::math::units::{round2, rows_to_mm, stitches_to_mm, stoll_round};
use crate::model::{CurveData, CurveForm, CurveStep, FormHeader, KanteChildSlot, LineSegment};

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
    /// Optional Y-driven step size for `CalcFormula_Line` when `step_x == 0 && step_y > 0`.
    pub step_y: i32,
    /// Preserved step-level narrowing module width (`MindernBreite`, Kural 45).
    pub narrowing_width: i32,
    /// Preserved step-level widening module width (`ZunehmenBreite`).
    pub widening_width: i32,
    /// Preserved step-level fade-out/edge width (`AusblendenBreite`).
    pub fade_out_width: i32,
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
        }
    }
}

/// Extract preserved step-level module widths `(MindernBreite, ZunehmenBreite, AusblendenBreite)` from an existing line.
pub fn extract_curve_module_widths(line: &LineSegment) -> (i32, i32, i32) {
    if let Some(ref c) = line.curve {
        if let Some(first) = c.steps.first() {
            return (
                first.narrowing_width,
                first.widening_width,
                first.fade_out_width,
            );
        }
    }
    let nw = line
        .narrowing
        .as_ref()
        .and_then(|n| n.modules.first())
        .map(|m| m.width)
        .unwrap_or(0);
    let ww = line
        .widening
        .as_ref()
        .and_then(|w| w.modules.first())
        .map(|m| m.width)
        .unwrap_or(0);
    let fw = line
        .fade_out
        .as_ref()
        .and_then(|f| f.modules.first())
        .map(|m| m.width)
        .unwrap_or(0);
    (nw, ww, fw)
}

/// Compute exact `(sum_dx, sum_dy)` of a `[Kurve]` step slice accounting for `Faktor`, `GruppenTagId`, and `GruppenFaktor`.
pub fn curve_steps_sum_slice(steps: &[CurveStep]) -> (i32, i32) {
    let mut group_factors: HashMap<i32, i32> = HashMap::new();
    for s in steps {
        if s.group_tag_id > 0 && s.group_factor > 0 {
            group_factors.insert(s.group_tag_id, s.group_factor);
        }
    }

    let mut tot_dx = 0i32;
    let mut tot_dy = 0i32;
    for s in steps {
        let mut mult = s.factor.max(0);
        if s.group_tag_id > 0 {
            mult *= *group_factors.get(&s.group_tag_id).unwrap_or(&1);
        }
        tot_dx += s.step_width * mult;
        tot_dy += s.step_height * mult;
    }
    (tot_dx, tot_dy)
}

/// Compute exact `(sum_dx, sum_dy)` of a `[Kurve]` accounting for `Faktor`, `GruppenTagId`, and `GruppenFaktor`.
pub fn curve_steps_sum(curve: &CurveData) -> (i32, i32) {
    curve_steps_sum_slice(&curve.steps)
}

/// Generate `[KurvenLinie]` steps using exact disassembled `formaccess.dll` formulas:
/// - `CurveForm::Line` (1): `CalcFormula_Line` (RVA `0x51BE0`)
/// - `CurveForm::Step` (2): `CalcFormula_LineOben` (RVA `0x51FC0`)
/// - `CurveForm::JCurve` (3): `CalcFormula_J` (RVA `0x51900`) — `(x^2 + 0.6*dx*x) * dy / (1.6*dx^2)`
/// - `CurveForm::SCurve` (4): `CalcFormula_S` (RVA `0x522E0`) — exact cubic polynomial in `u = 9.28*x/dx`
/// - `CurveForm::UserCurve` (5): defaults to `CalcFormula_Line` when creating fresh
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

    // Y-driven Bresenham line when step_x == 0 && step_y != 0
    if params.curve_form == CurveForm::Line && params.step_x == 0 && params.step_y != 0 {
        let pairs = calc_formula_line_y_driven(dx, dy, params.step_y);
        return compress_pairs_to_steps(
            &pairs,
            params.narrowing_width,
            params.widening_width,
            params.fade_out_width,
        );
    }

    let step_x_mag = params.step_x.abs().max(1).min(dx.abs());
    let sx = if dx < 0 { -step_x_mag } else { step_x_mag };

    match params.curve_form {
        CurveForm::Line | CurveForm::UserCurve => {
            let pairs = calc_formula_x_driven(dx, dy, sx, true, |x, eff_dx, eff_dy| {
                x * (eff_dy / eff_dx)
            });
            compress_pairs_to_steps(
                &pairs,
                params.narrowing_width,
                params.widening_width,
                params.fade_out_width,
            )
        }
        CurveForm::Step => {
            // CalcFormula_LineOben (RVA 0x51FC0)
            let pairs = calc_formula_x_driven(dx, dy, sx, false, |x, eff_dx, eff_dy| {
                x * (eff_dy / eff_dx)
            });
            let mut steps = compress_pairs_to_steps(
                &pairs,
                params.narrowing_width,
                params.widening_width,
                params.fade_out_width,
            );
            if !steps.is_empty() {
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
        CurveForm::JCurve => {
            // Exact CalcFormula_J (RVA 0x51900):
            // k = eff_dy / (1.6 * eff_dx^2)
            // y(x) = (x^2 + 0.6 * eff_dx * x) * k
            let pairs = calc_formula_x_driven(dx, dy, sx, true, |x, eff_dx, eff_dy| {
                let k = eff_dy / (1.6 * eff_dx * eff_dx);
                (x * x + 0.6 * eff_dx * x) * k
            });
            compress_pairs_to_steps(
                &pairs,
                params.narrowing_width,
                params.widening_width,
                params.fade_out_width,
            )
        }
        CurveForm::SCurve => {
            // Exact CalcFormula_S (RVA 0x522E0):
            // u = x * (9.28 / eff_dx)
            // P(u) = -0.018432*u^3 + 0.224866*u^2 + 0.592899*u - 0.187795
            // y(x) = (P(u) + 0.18) / 10.18 * eff_dy
            let pairs = calc_formula_x_driven(dx, dy, sx, true, |x, eff_dx, eff_dy| {
                let u = x * (9.28 / eff_dx);
                let p = -0.018432 * u * u * u + 0.224866 * u * u + 0.592899 * u - 0.187795;
                ((p + 0.18) / 10.18) * eff_dy
            });
            compress_pairs_to_steps(
                &pairs,
                params.narrowing_width,
                params.widening_width,
                params.fade_out_width,
            )
        }
    }
}

/// Shared X-driven stepping loop matching `formaccess.dll` (`CalcFormula_Line`, `_J`, `_S`):
/// includes first-step non-zero guard, exact endpoint clamping, monotonicity guard, and remainder step.
fn calc_formula_x_driven<F>(
    dx: i32,
    dy: i32,
    sx: i32,
    first_step_guard: bool,
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
            y_offset + stoll_round(eval_y(x_i as f64 - x_offset, eff_dx, eff_dy))
        };

        // First-step non-zero guard (MATH_AND_CURVES_SPEC.md Section 4.1)
        if i == 1 && first_step_guard && y_i == 0 && !is_final && dy != 0 {
            let y1 = dy.signum();
            y_i = y1;
            x_offset = sx as f64;
            y_offset = y1;
            eff_dx = (dx - sx) as f64;
            eff_dy = (dy - y1) as f64;
        }

        let mut delta_y = y_i - cur_y;
        // Monotonicity guard
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

    // Exact closure safety check (in case monotonicity guard clamped anything)
    if cur_x != dx || cur_y != dy {
        if let Some(last) = pairs.last_mut() {
            last.0 += dx - cur_x;
            last.1 += dy - cur_y;
        }
    }

    pairs
}

fn calc_formula_line_y_driven(dx: i32, dy: i32, step_y: i32) -> Vec<(i32, i32)> {
    let sy = if dy < 0 { -step_y.abs() } else { step_y.abs() };
    let abs_dy = dy.abs();
    let abs_sy = sy.abs().max(1);
    let num_full = abs_dy / abs_sy;
    let inv_slope = dx as f64 / dy as f64;

    let mut pairs = Vec::new();
    let mut cur_x = 0i32;
    let mut cur_y = 0i32;

    for i in 1..=num_full {
        let y_i = sy * i;
        let x_i = if y_i.abs() == abs_dy {
            dx
        } else {
            stoll_round(y_i as f64 * inv_slope)
        };
        let delta_x = x_i - cur_x;
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

    // Expand existing steps (accounting for group factors)
    let mut group_factors: HashMap<i32, i32> = HashMap::new();
    for s in &existing.steps {
        if s.group_tag_id > 0 && s.group_factor > 0 {
            group_factors.insert(s.group_tag_id, s.group_factor);
        }
    }

    let mut expanded: Vec<(i32, i32)> = Vec::new();
    for s in &existing.steps {
        let mut reps = s.factor.max(1);
        if s.group_tag_id > 0 {
            reps *= *group_factors.get(&s.group_tag_id).unwrap_or(&1);
        }
        for _ in 0..reps {
            expanded.push((s.step_width, s.step_height));
        }
    }

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
