use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, Frame, Margin, PointerButton, Pos2,
    Rect, Sense, Stroke, StrokeKind, Ui,
};

use stoll_kalip_editoru::math::{
    auto_detect_line_function_in_context, element_distance_from_top, mm_to_rows, mm_to_stitches,
    recalc_line_from_stitches_and_rows, rows_to_mm, stitches_to_mm, sync_endline_element_y,
    sync_mirrored_right_lines, DocumentGeometry, SegmentSpan,
};
use stoll_kalip_editoru::model::{
    ElementType, FormDocument, LineSegment, LineSide, UndoStack, YReferenceLine,
};

use crate::ui::theme::{bold_font, regular_font};
use crate::ui::{
    line_function_default_tab, line_function_grid_label_tr, CurveEditorRequest, LinePropsRequest,
    TableMode, UiState,
};

/// Light Studio CAD palette constants (no dark mode).
mod studio {
    use egui::Color32;

    pub const CANVAS_BG: Color32 = Color32::from_rgb(0xF4, 0xF6, 0xFA);
    pub const GRID_LINE: Color32 = Color32::from_rgb(0xE4, 0xE9, 0xF2);
    pub const AXIS_LINE: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);
    pub const AXIS_TEXT: Color32 = Color32::from_rgb(0x47, 0x55, 0x69);
    pub const MIRROR_AXIS: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B);

    pub const PRIMARY_CONTOUR: Color32 = Color32::from_rgb(0x0F, 0x76, 0x6E);
    pub const PRIMARY_FILL: Color32 = Color32::from_rgba_premultiplied(0x0F, 0x76, 0x6E, 10);
    pub const NECKLINE_COLOR: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    pub const HOLE_COLOR: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
    pub const GORE_COLOR: Color32 = Color32::from_rgb(0x93, 0x33, 0xEA);

    pub const SELECTED_BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);
    pub const HOVER_RED: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);

    pub const DIM_LINE: Color32 = Color32::from_rgb(0x47, 0x55, 0x69);
    pub const DIM_PILL_BG: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 238);
    pub const DIM_PILL_BORDER: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);

    pub const HUD_BG: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 240);
    pub const HUD_BORDER: Color32 = Color32::from_rgb(0xCB, 0xD5, 0xE1);
    pub const HUD_TEXT: Color32 = Color32::from_rgb(0x1E, 0x29, 0x3B);
    pub const HUD_MUTED: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B);
    pub const HUD_ACTIVE_BG: Color32 = Color32::from_rgb(0xDB, 0xEA, 0xFE);
    pub const HUD_ACTIVE_BORDER: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);

    pub const RULER_COLOR: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleKind {
    VertexEnd,
    Midpoint,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HoverTarget {
    elem_idx: usize,
    side: LineSide,
    line_idx: usize,
    kind: HandleKind,
}

#[derive(Debug, Clone)]
struct DragState {
    elem_idx: usize,
    side: LineSide,
    line_idx: usize,
    orig_line: LineSegment,
    start_pointer_pos: Pos2,
    undo_pushed: bool,
}

/// Primary interactive Light Studio CAD canvas (`cad_canvas.rs`).
/// Supports Seçenek 1 (neighbor-preserving) vertex & midpoint dragging, live mirror sync,
/// dual stitches + cm dimension overlays, cursor-centered zoom/pan, and Shift+Drag ruler.
#[derive(Debug, Clone)]
pub struct CadCanvasState {
    pub zoom: f32,
    pub pan_px: egui::Vec2,
    pub show_grid: bool,
    pub show_markers: bool,
    pub show_line_numbers: bool,
    pub show_mirror_axis: bool,
    hovered_target: Option<HoverTarget>,
    drag_state: Option<DragState>,
    measure_world: Option<((f64, f64), (f64, f64))>,
}

impl Default for CadCanvasState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_px: egui::Vec2::ZERO,
            show_grid: true,
            show_markers: false,
            show_line_numbers: true,
            show_mirror_axis: true,
            hovered_target: None,
            drag_state: None,
            measure_world: None,
        }
    }
}

impl CadCanvasState {
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_px = egui::Vec2::ZERO;
        self.measure_world = None;
    }

    pub fn clear_drag(&mut self) {
        self.drag_state = None;
        self.hovered_target = None;
    }

    /// Render the interactive CAD canvas inside the given `ui`.
    /// Returns `true` if `doc` was mutated (so `app.rs` marks dirty and recomputes geometry).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        doc: &mut FormDocument,
        geom: &DocumentGeometry,
        undo: &mut UndoStack,
        state: &mut UiState,
    ) -> bool {
        let mut doc_mutated = false;

        let avail = ui.available_size();
        let canvas_size = vec2(avail.x.max(240.0), avail.y.max(220.0));
        let (rect, response) = ui.allocate_exact_size(canvas_size, Sense::click_and_drag());

        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, studio::CANVAS_BG);

        // Compute world bounds in physical mm so X/Y have true garment proportions
        let (mut min_x_mm, mut max_x_mm, mut min_y_mm, mut max_y_mm) =
            if geom.bounds.0.is_finite() {
                geom.bounds
            } else {
                (-250.0, 250.0, 0.0, 500.0)
            };

        let max_abs_x = min_x_mm.abs().max(max_x_mm.abs()).max(80.0);
        if min_x_mm < 0.0 && max_x_mm > 0.0 {
            min_x_mm = -max_abs_x;
            max_x_mm = max_abs_x;
        }
        min_y_mm = min_y_mm.min(0.0);
        max_y_mm = max_y_mm.max(120.0);

        // Leave generous margins for top HUD strip, bottom width dimension, and right height dimension pill
        let pad_left = 56.0;
        let pad_right = 118.0;
        let pad_top = 52.0;
        let pad_bottom = 54.0;

        let plot_rect = Rect::from_min_max(
            pos2(rect.left() + pad_left, rect.top() + pad_top),
            pos2(
                (rect.right() - pad_right).max(rect.left() + pad_left + 60.0),
                (rect.bottom() - pad_bottom).max(rect.top() + pad_top + 60.0),
            ),
        );

        let span_x_mm = (max_x_mm - min_x_mm).max(20.0);
        let span_y_mm = (max_y_mm - min_y_mm).max(20.0);

        let base_scale = ((plot_rect.width() as f64 / span_x_mm)
            .min(plot_rect.height() as f64 / span_y_mm))
        .max(1e-4);
        let scale = base_scale * (self.zoom as f64);

        let world_cx_mm = (min_x_mm + max_x_mm) * 0.5;
        let world_cy_mm = (min_y_mm + max_y_mm) * 0.5;
        let screen_cx = plot_rect.center().x + self.pan_px.x;
        let screen_cy = plot_rect.center().y + self.pan_px.y;

        let world_to_screen = |wx_mm: f64, wy_mm: f64| -> Pos2 {
            let sx = screen_cx + ((wx_mm - world_cx_mm) * scale) as f32;
            let sy = screen_cy - ((wy_mm - world_cy_mm) * scale) as f32;
            pos2(sx, sy)
        };

        let screen_to_world_mm = |p: Pos2| -> (f64, f64) {
            let wx = world_cx_mm + ((p.x - screen_cx) as f64) / scale;
            let wy = world_cy_mm - ((p.y - screen_cy) as f64) / scale;
            (wx, wy)
        };

        // ── 1. Light Studio Engineering Grid & Axes (in cm / mm) ─────────────────
        let (vis_x0, vis_y1) = screen_to_world_mm(rect.left_top());
        let (vis_x1, vis_y0) = screen_to_world_mm(rect.right_bottom());

        let step_mm = nice_axis_step_mm((vis_x1 - vis_x0) / 8.0);
        let ix0 = (vis_x0 / step_mm).floor() as i64 - 1;
        let ix1 = (vis_x1 / step_mm).ceil() as i64 + 1;
        let iy0 = (vis_y0 / step_mm).floor() as i64 - 1;
        let iy1 = (vis_y1 / step_mm).ceil() as i64 + 1;

        if self.show_grid {
            let grid_stroke = Stroke::new(1.0_f32, studio::GRID_LINE);
            for ix in ix0..=ix1 {
                let wx = ix as f64 * step_mm;
                let sx = world_to_screen(wx, 0.0).x;
                if sx >= rect.left() && sx <= rect.right() {
                    painter.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], grid_stroke);
                }
            }
            for iy in iy0..=iy1 {
                let wy = iy as f64 * step_mm;
                let sy = world_to_screen(0.0, wy).y;
                if sy >= rect.top() && sy <= rect.bottom() {
                    painter.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], grid_stroke);
                }
            }
        }

        // Baseline X axis (y = 0) & subtle left axis ticks
        let axis_stroke = Stroke::new(1.0_f32, studio::AXIS_LINE);
        let y_zero_screen = world_to_screen(0.0, 0.0).y;
        if y_zero_screen >= rect.top() + 36.0 && y_zero_screen <= rect.bottom() - 12.0 {
            painter.line_segment(
                [pos2(rect.left() + 40.0, y_zero_screen), pos2(rect.right() - 16.0, y_zero_screen)],
                axis_stroke,
            );
        }

        // Subtle cm ticks on left edge & bottom edge
        let mut last_y_lbl = f32::INFINITY;
        for iy in iy0..=iy1 {
            let wy = iy as f64 * step_mm;
            if wy < -1e-6 {
                continue;
            }
            let sy = world_to_screen(0.0, wy).y;
            if sy < rect.top() + 44.0 || sy > rect.bottom() - 22.0 {
                continue;
            }
            if sy + 24.0 < last_y_lbl {
                let cm_val = wy / 10.0;
                let lbl = if (cm_val - cm_val.round()).abs() < 0.05 {
                    format!("{:.0} cm", cm_val.round())
                } else {
                    format!("{cm_val:.1} cm")
                };
                painter.text(
                    pos2(rect.left() + 6.0, sy),
                    Align2::LEFT_CENTER,
                    lbl,
                    regular_font(9.5),
                    studio::AXIS_TEXT,
                );
                last_y_lbl = sy;
            }
        }

        // Center mirror axis (X = 0)
        if self.show_mirror_axis {
            let ax = world_to_screen(0.0, 0.0).x;
            if ax >= rect.left() && ax <= rect.right() {
                draw_vertical_dashed(
                    &painter,
                    ax,
                    rect.top() + 42.0,
                    rect.bottom() - 18.0,
                    studio::MIRROR_AXIS,
                );
            }
        }

        // ── 2. Pre-project segments once + Hit-Testing for Vertex Handles, Midpoints, and Edges ──
        let modal_open = state.any_modal_open();
        let projected_elems: Vec<(Vec<Vec<Pos2>>, Vec<Vec<Pos2>>)> = geom
            .elements
            .iter()
            .map(|eg| {
                if eg.hidden {
                    return (Vec::new(), Vec::new());
                }
                let l_pts = eg
                    .left_segments
                    .iter()
                    .map(|seg| {
                        seg.polyline_mm
                            .iter()
                            .map(|p| world_to_screen(p.x, p.y))
                            .collect()
                    })
                    .collect();
                let r_pts = eg
                    .right_segments
                    .iter()
                    .map(|seg| {
                        seg.polyline_mm
                            .iter()
                            .map(|p| world_to_screen(p.x, p.y))
                            .collect()
                    })
                    .collect();
                (l_pts, r_pts)
            })
            .collect();

        let hover_pos = if modal_open { None } else { response.hover_pos() };
        let shift_held = ui.input(|i| i.modifiers.shift);
        let mut best_handle: Option<(f32, HoverTarget)> = None;
        let mut best_edge: Option<(f32, HoverTarget)> = None;

        let active_elem = state.selected_elem;
        let active_side = state.active_table_side;

        if modal_open {
            self.hovered_target = None;
        } else if self.drag_state.is_none() && !shift_held {
            if let Some(hp) = hover_pos {
                for (ei, eg) in geom.elements.iter().enumerate() {
                    if eg.hidden {
                        continue;
                    }
                    let (ref l_pts, ref r_pts) = projected_elems[ei];
                    let sides: [(LineSide, &[SegmentSpan], &[Vec<Pos2>]); 2] = [
                        (LineSide::Left, &eg.left_segments, l_pts),
                        (LineSide::Right, &eg.right_segments, r_pts),
                    ];
                    for (side, segs, pts_list) in sides {
                        for (seg, screen_pts) in segs.iter().zip(pts_list.iter()) {
                            if screen_pts.len() < 2 {
                                continue;
                            }
                            let end_pt = *screen_pts.last().unwrap();
                            let mid_pt = polyline_midpoint(screen_pts);

                            let d_end = hp.distance(end_pt);
                            if d_end < 9.0
                                && best_handle.map(|(b, _)| d_end < b).unwrap_or(true)
                            {
                                best_handle = Some((
                                    d_end,
                                    HoverTarget {
                                        elem_idx: ei,
                                        side,
                                        line_idx: seg.line_index,
                                        kind: HandleKind::VertexEnd,
                                    },
                                ));
                            }

                            let d_mid = hp.distance(mid_pt);
                            if d_mid < 8.0
                                && best_handle.map(|(b, _)| d_mid < b).unwrap_or(true)
                            {
                                best_handle = Some((
                                    d_mid,
                                    HoverTarget {
                                        elem_idx: ei,
                                        side,
                                        line_idx: seg.line_index,
                                        kind: HandleKind::Midpoint,
                                    },
                                ));
                            }

                            let d_edge = dist_to_polyline(hp, screen_pts);
                            if d_edge < 7.5
                                && best_edge.map(|(b, _)| d_edge < b).unwrap_or(true)
                            {
                                best_edge = Some((
                                    d_edge,
                                    HoverTarget {
                                        elem_idx: ei,
                                        side,
                                        line_idx: seg.line_index,
                                        kind: HandleKind::Edge,
                                    },
                                ));
                            }
                        }
                    }
                }
            }
            self.hovered_target = best_handle.or(best_edge).map(|(_, t)| t);
        }

        // ── 3. Interactive Vertex/Midpoint Dragging (Seçenek 1: Komşu Korumalı) ──
        if !modal_open && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.measure_world = None;
            self.drag_state = None;
        }

        if !modal_open && response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let factor = (1.0 + scroll * 0.0025).clamp(0.75, 1.35);
                let old_zoom = self.zoom;
                let new_zoom = (self.zoom * factor).clamp(0.25, 25.0);
                if let Some(hp) = response.hover_pos() {
                    let rel = hp - (plot_rect.center() + self.pan_px);
                    let ratio = new_zoom / old_zoom;
                    self.pan_px -= rel * (ratio - 1.0);
                }
                self.zoom = new_zoom;
            }
        }

        if !modal_open && self.drag_state.is_none() && shift_held && response.drag_started_by(PointerButton::Primary) {
            if let Some(hp) = response.interact_pointer_pos() {
                let w = screen_to_world_mm(hp);
                self.measure_world = Some((w, w));
            }
        } else if !modal_open && self.drag_state.is_none() && shift_held && response.dragged_by(PointerButton::Primary) {
            if let (Some((start, _)), Some(hp)) =
                (self.measure_world, response.interact_pointer_pos())
            {
                self.measure_world = Some((start, screen_to_world_mm(hp)));
            }
        } else if !modal_open && response.drag_started_by(PointerButton::Primary) && !shift_held {
            if let Some(target) = self.hovered_target {
                if matches!(target.kind, HandleKind::VertexEnd | HandleKind::Midpoint) {
                    let is_mirrored = doc
                        .elements
                        .get(target.elem_idx)
                        .map(|e| e.mirrored)
                        .unwrap_or(true);
                    let effective_side = if is_mirrored {
                        LineSide::Left
                    } else {
                        target.side
                    };
                    if let Some(line) = doc
                        .elements
                        .get(target.elem_idx)
                        .and_then(|e| e.lines(effective_side).get(target.line_idx))
                    {
                        let start_pos = response
                            .interact_pointer_pos()
                            .or(hover_pos)
                            .unwrap_or(plot_rect.center());
                        self.drag_state = Some(DragState {
                            elem_idx: target.elem_idx,
                            side: target.side,
                            line_idx: target.line_idx,
                            orig_line: line.clone(),
                            start_pointer_pos: start_pos,
                            undo_pushed: false,
                        });
                        // Select dragged segment in table
                        state.selected_elem = target.elem_idx;
                        state.active_table_side = effective_side;
                        state.table_mode = match effective_side {
                            LineSide::Left => TableMode::LeftLines,
                            LineSide::Right => TableMode::RightLines,
                        };
                        state.select_single_row(target.line_idx);
                        state.focused_cell = Some((target.line_idx, 2));
                        state.editing_cell = None;
                    }
                }
            }
        }

        let mut active_drag_pill: Option<(Pos2, String)> = None;

        if response.dragged_by(PointerButton::Primary) {
            if let Some(ref mut ds) = self.drag_state {
                if let Some(cur_pos) = response.interact_pointer_pos() {
                    let delta_px = cur_pos - ds.start_pointer_pos;
                    let mut dx_mm = (delta_px.x as f64) / scale;
                    let mut dy_mm = -(delta_px.y as f64) / scale;

                    if shift_held {
                        if delta_px.x.abs() >= delta_px.y.abs() {
                            dy_mm = 0.0;
                        } else {
                            dx_mm = 0.0;
                        }
                    }

                    let mut dx_st = mm_to_stitches(dx_mm, &doc.header);
                    let mut dy_st = mm_to_rows(dy_mm, &doc.header);

                    if state.snap_even_rows && dy_st != 0 {
                        dy_st = ((dy_st as f64 / 2.0).round() as i32) * 2;
                    }

                    let is_mirrored = doc
                        .elements
                        .get(ds.elem_idx)
                        .map(|e| e.mirrored)
                        .unwrap_or(true);
                    let effective_side = if is_mirrored {
                        LineSide::Left
                    } else {
                        ds.side
                    };
                    if is_mirrored && ds.side == LineSide::Right {
                        dx_st = -dx_st;
                    }

                    let is_base_row = doc
                        .elements
                        .get(ds.elem_idx)
                        .map(|e| e.is_base_line_row(effective_side, ds.line_idx))
                        .unwrap_or(ds.line_idx == 0);
                    let lock_rows = is_base_row
                        || (ds.orig_line.last_line != 0 && ds.orig_line.rows == 0);
                    let new_stitches = ds.orig_line.stitches + dx_st;
                    let new_rows = if lock_rows {
                        ds.orig_line.rows
                    } else {
                        (ds.orig_line.rows + dy_st).max(0)
                    };

                    let cur_st_rows = doc
                        .elements
                        .get(ds.elem_idx)
                        .and_then(|e| e.lines(effective_side).get(ds.line_idx))
                        .map(|l| (l.stitches, l.rows));

                    let mut pill_label = None;
                    let header_copy = doc.header.clone();
                    if cur_st_rows != Some((new_stitches, new_rows)) {
                        if !ds.undo_pushed {
                            undo.push("Nokta sürükle", doc);
                            ds.undo_pushed = true;
                        }
                        let endline_dists: Vec<(usize, f64)> = (0..doc.elements.len())
                            .filter(|&ei| doc.elements[ei].y_ref_line == YReferenceLine::EndLine)
                            .map(|ei| (ei, element_distance_from_top(doc, ei).0))
                            .collect();
                        if let Some(el) = doc.elements.get_mut(ds.elem_idx) {
                            let elem_type = el.element_type();
                            let line_count = el.lines(effective_side).len();
                            let was_closing = ds.line_idx > 0
                                && ds.line_idx + 1 == line_count
                                && ds.orig_line.rows == 0
                                && ds.orig_line.last_line != 0;
                            if let Some(line) = el.lines_mut(effective_side).get_mut(ds.line_idx) {
                                *line = ds.orig_line.clone();
                                line.stitches = new_stitches;
                                line.rows = new_rows;
                                recalc_line_from_stitches_and_rows(line, &header_copy);
                                auto_detect_line_function_in_context(
                                    line,
                                    ds.line_idx == 0,
                                    was_closing,
                                    effective_side == LineSide::Left,
                                    elem_type,
                                );
                                doc_mutated = true;
                            }
                        }
                        if doc_mutated {
                            for (ei, dist_top) in endline_dists {
                                sync_endline_element_y(doc, ei, dist_top);
                            }
                        }
                    }
                    if doc_mutated && is_mirrored {
                        sync_mirrored_right_lines(doc, ds.elem_idx);
                    }
                    if let Some(line) = doc
                        .elements
                        .get(ds.elem_idx)
                        .and_then(|e| e.lines(effective_side).get(ds.line_idx))
                    {
                        let func_lbl = line_function_grid_label_tr(line.function.to_id());
                        let func_part = if func_lbl.is_empty() {
                            String::new()
                        } else {
                            format!(" ({func_lbl})")
                        };
                        let w_cm = stitches_to_mm(line.stitches, &header_copy) / 10.0;
                        let h_cm = rows_to_mm(line.rows, &header_copy) / 10.0;
                        let lock_hint = if shift_held { " [Eksen Kilit]" } else { "" };
                        pill_label = Some(format!(
                            "Satır {}{}{} | Boy: {} sıra ({:.1} cm) | En: {:+} ilmek ({:+.1} cm)",
                            ds.line_idx + 1,
                            func_part,
                            lock_hint,
                            line.rows,
                            h_cm,
                            line.stitches,
                            w_cm
                        ));
                    }
                    if let Some(lbl) = pill_label {
                        active_drag_pill = Some((cur_pos, lbl));
                    }
                }
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if !shift_held {
                // Pan on empty canvas drag
                self.pan_px += response.drag_delta();
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
        } else if response.dragged_by(PointerButton::Middle)
            || response.dragged_by(PointerButton::Secondary)
        {
            self.pan_px += response.drag_delta();
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }

        if doc_mutated {
            doc.dirty = true;
        }

        if response.drag_stopped() {
            self.drag_state = None;
        }

        if response.hovered() && self.drag_state.is_none() {
            if shift_held {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            } else if let Some(t) = self.hovered_target {
                match t.kind {
                    HandleKind::VertexEnd | HandleKind::Midpoint => {
                        ui.ctx().set_cursor_icon(CursorIcon::Grab);
                    }
                    HandleKind::Edge => {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                }
            }
        }

        // ── 4. Draw Shape Contours & Selection Handles ───────────────────────────
        let mut placed_num_rects: Vec<Rect> = Vec::new();
        for (ei, eg) in geom.elements.iter().enumerate() {
            if eg.hidden {
                continue;
            }

            let base_color = if ei == 0 || eg.element_type == ElementType::BasicShape {
                studio::PRIMARY_CONTOUR
            } else {
                match eg.element_type {
                    ElementType::Neckline | ElementType::NecklineMarking => studio::NECKLINE_COLOR,
                    ElementType::Hole
                    | ElementType::HoleMarking
                    | ElementType::Opening
                    | ElementType::OpeningMarking => studio::HOLE_COLOR,
                    ElementType::Gore | ElementType::Dart => studio::GORE_COLOR,
                    _ => studio::NECKLINE_COLOR,
                }
            };

            // Subtle translucent fill for closed mirrored BasicShape (supports concave armholes/necklines!)
            if (ei == 0 || eg.element_type == ElementType::BasicShape)
                && !eg.left_polyline.is_empty()
                && !eg.right_polyline.is_empty()
            {
                let mut poly: Vec<Pos2> = eg
                    .left_polyline
                    .iter()
                    .map(|p| world_to_screen(p.x, p.y))
                    .collect();
                for p in eg.right_polyline.iter().rev() {
                    poly.push(world_to_screen(p.x, p.y));
                }
                paint_concave_polygon_fill(&painter, &poly, studio::PRIMARY_FILL);
            }

            let (ref l_pts, ref r_pts) = projected_elems[ei];
            let sides: [(LineSide, &[SegmentSpan], &[Vec<Pos2>]); 2] = [
                (LineSide::Left, &eg.left_segments, l_pts),
                (LineSide::Right, &eg.right_segments, r_pts),
            ];

            for (side, segs, pts_list) in sides {
                for (seg, screen_pts) in segs.iter().zip(pts_list.iter()) {
                    if screen_pts.len() < 2 {
                        continue;
                    }

                    let is_selected = ei == active_elem
                        && (side == active_side || (eg.mirrored && active_side == LineSide::Left))
                        && state.selected_rows.contains(&seg.line_index);
                    let is_hovered = self
                        .hovered_target
                        .map(|t| t.elem_idx == ei && t.side == side && t.line_idx == seg.line_index)
                        .unwrap_or(false);

                    let (stroke_col, stroke_w) = if is_selected {
                        (studio::SELECTED_BLUE, 3.0_f32)
                    } else if state.all_elements_selected {
                        (studio::SELECTED_BLUE, 2.2_f32)
                    } else if is_hovered {
                        (studio::HOVER_RED, 2.5_f32)
                    } else {
                        (base_color, if ei == 0 { 2.0_f32 } else { 1.8_f32 })
                    };

                    painter.add(egui::Shape::line(
                        screen_pts.clone(),
                        Stroke::new(stroke_w, stroke_col),
                    ));

                    // Line number label with collision avoidance & outward offset on horizontal edges
                    if self.show_line_numbers && screen_pts.len() >= 2 {
                        let first_pt = *screen_pts.first().unwrap();
                        let last_pt = *screen_pts.last().unwrap();
                        let seg_px_len = first_pt.distance(last_pt);
                        // Skip duplicate mirrored horizontal center-closing label on Right side if near X=0
                        let is_horizontal = seg.delta_st.1 == 0;
                        let skip_dup_mirrored_horiz = eg.mirrored
                            && side == LineSide::Right
                            && is_horizontal
                            && seg.start_st.0.abs().min(seg.end_st.0.abs()) <= 2;

                        if seg_px_len >= 14.0 && !skip_dup_mirrored_horiz {
                            let mid = polyline_midpoint(&screen_pts);
                            let dir = last_pt - first_pt;
                            let len = dir.length().max(1.0);
                            // Normal pointing outward from shape
                            let base_pos = if is_horizontal {
                                let at_bottom = (seg.start_mm.y - eg.bounds.2).abs() < 2.0;
                                let dy_off = if at_bottom { 11.0 } else { -11.0 };
                                let dx_off = if eg.mirrored {
                                    // Place centered near X=0 for top closing line or offset cleanly
                                    0.0
                                } else {
                                    0.0
                                };
                                pos2(mid.x + dx_off, mid.y + dy_off)
                            } else {
                                // Outward perpendicular offset (left side -> -X, right side -> +X)
                                let nx = if side == LineSide::Left { -12.0 } else { 12.0 };
                                let ny = if (dir.x / len).abs() > 0.7 {
                                    if side == LineSide::Left { -6.0 } else { -6.0 }
                                } else {
                                    0.0
                                };
                                pos2(mid.x + nx, mid.y + ny)
                            };

                            let num_txt = format!("{}", seg.line_index + 1);
                            let font = bold_font(9.5);
                            let galley = painter.layout_no_wrap(
                                num_txt.clone(),
                                font,
                                if is_selected {
                                    studio::SELECTED_BLUE
                                } else {
                                    studio::AXIS_TEXT
                                },
                            );
                            let sz = galley.size() + vec2(4.0, 2.0);
                            let candidates = [
                                base_pos,
                                pos2(base_pos.x, base_pos.y - 12.0),
                                pos2(base_pos.x, base_pos.y + 12.0),
                            ];
                            for cand in candidates {
                                let cand_rect = Rect::from_center_size(cand, sz);
                                if rect.contains_rect(cand_rect)
                                    && !placed_num_rects.iter().any(|r| r.intersects(cand_rect))
                                {
                                    painter.galley(
                                        cand_rect.min + vec2(2.0, 1.0),
                                        galley,
                                        if is_selected {
                                            studio::SELECTED_BLUE
                                        } else {
                                            studio::AXIS_TEXT
                                        },
                                    );
                                    placed_num_rects.push(cand_rect);
                                    break;
                                }
                            }
                        }
                    }

                    // Draw interactive vertex & midpoint handles on selected or hovered segments
                    if is_selected || is_hovered {
                        let end_pt = *screen_pts.last().unwrap();
                        let mid_pt = polyline_midpoint(&screen_pts);

                        let end_hot = self
                            .hovered_target
                            .map(|t| {
                                t.elem_idx == ei
                                    && t.side == side
                                    && t.line_idx == seg.line_index
                                    && t.kind == HandleKind::VertexEnd
                            })
                            .unwrap_or(false)
                            || self
                                .drag_state
                                .as_ref()
                                .map(|d| {
                                    d.elem_idx == ei
                                        && d.side == side
                                        && d.line_idx == seg.line_index
                                })
                                .unwrap_or(false);

                        let mid_hot = self
                            .hovered_target
                            .map(|t| {
                                t.elem_idx == ei
                                    && t.side == side
                                    && t.line_idx == seg.line_index
                                    && t.kind == HandleKind::Midpoint
                            })
                            .unwrap_or(false);

                        // Vertex handle (r = 5px, white fill, #2563EB stroke; #EF4444 on hover/drag)
                        let v_stroke = if end_hot {
                            studio::HOVER_RED
                        } else {
                            studio::SELECTED_BLUE
                        };
                        painter.circle_filled(end_pt, 5.0, Color32::WHITE);
                        painter.circle_stroke(end_pt, 5.0, Stroke::new(2.0_f32, v_stroke));

                        // Edge midpoint handle
                        let m_stroke = if mid_hot {
                            studio::HOVER_RED
                        } else {
                            studio::SELECTED_BLUE
                        };
                        painter.circle_filled(mid_pt, 4.0, Color32::WHITE);
                        painter.circle_stroke(mid_pt, 4.0, Stroke::new(1.5_f32, m_stroke));
                    }
                }
            }

            if self.show_markers || state.table_mode.is_markers() {
                if let Some(el_doc) = doc.elements.get(ei) {
                    paint_element_markers(&painter, doc, el_doc, eg, &world_to_screen);
                }
            }
        }

        // ── 5. Dual Stitches + cm CAD Dimension Overlays ─────────────────────────
        if state.show_dimensions {
            paint_cad_dimensions(
                &painter,
                doc,
                geom,
                state,
                &world_to_screen,
                rect,
            );
        }

        // ── 6. Click / Double-click Selection & Dialog Launch ────────────────────
        if response.double_clicked() {
            if let Some(t) = self.hovered_target {
                let is_mirrored = doc
                    .elements
                    .get(t.elem_idx)
                    .map(|e| e.mirrored)
                    .unwrap_or(true);
                let effective_side = if is_mirrored {
                    LineSide::Left
                } else {
                    t.side
                };
                state.selected_elem = t.elem_idx;
                state.all_elements_selected = false;
                state.active_table_side = effective_side;
                state.table_mode = match effective_side {
                    LineSide::Left => TableMode::LeftLines,
                    LineSide::Right => TableMode::RightLines,
                };
                state.select_single_row(t.line_idx);
                state.focused_cell = Some((t.line_idx, 6));
                state.editing_cell = None;

                if let Some(line) = doc
                    .elements
                    .get(t.elem_idx)
                    .and_then(|e| e.lines(effective_side).get(t.line_idx))
                {
                    if line.curve.as_ref().map(|c| !c.steps.is_empty()).unwrap_or(false) {
                        state.curve_editor_req = Some(CurveEditorRequest {
                            elem_idx: t.elem_idx,
                            side: effective_side,
                            line_idx: t.line_idx,
                        });
                    } else {
                        state.line_props_req = Some(LinePropsRequest {
                            elem_idx: t.elem_idx,
                            side: effective_side,
                            line_idx: t.line_idx,
                            initial_tab: line_function_default_tab(line.function.to_id()),
                        });
                    }
                }
            } else {
                self.reset_view();
            }
        } else if response.clicked() {
            if let Some(t) = self.hovered_target {
                let is_mirrored = doc
                    .elements
                    .get(t.elem_idx)
                    .map(|e| e.mirrored)
                    .unwrap_or(true);
                let effective_side = if is_mirrored {
                    LineSide::Left
                } else {
                    t.side
                };
                state.selected_elem = t.elem_idx;
                state.all_elements_selected = false;
                state.active_table_side = effective_side;
                state.table_mode = match effective_side {
                    LineSide::Left => TableMode::LeftLines,
                    LineSide::Right => TableMode::RightLines,
                };
                state.select_single_row(t.line_idx);
                state.focused_cell = Some((t.line_idx, 2));
                state.editing_cell = None;
            }
        }

        // ── 7. Shift+Drag Measure Ruler Overlay ──────────────────────────────────
        if let Some(((ax, ay), (bx, by))) = self.measure_world {
            let pa = world_to_screen(ax, ay);
            let pb = world_to_screen(bx, by);
            draw_dashed_segment(&painter, pa, pb, Stroke::new(1.5_f32, studio::RULER_COLOR));
            for p in [pa, pb] {
                painter.circle_filled(p, 3.5, studio::RULER_COLOR);
            }
            let dmx = bx - ax;
            let dmy = by - ay;
            let dst = mm_to_stitches(dmx, &doc.header);
            let drw = mm_to_rows(dmy, &doc.header);
            let dist_cm = (dmx * dmx + dmy * dmy).sqrt() / 10.0;
            let lbl = format!(
                "Ölçüm: dX={} ilm ({:.1} cm) | dY={} sıra ({:.1} cm) | L={:.1} cm",
                dst,
                dmx / 10.0,
                drw,
                dmy / 10.0,
                dist_cm
            );
            paint_pill_label(
                &painter,
                pos2((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5 - 18.0),
                &lbl,
                studio::HUD_TEXT,
                studio::DIM_PILL_BG,
                studio::RULER_COLOR,
                rect,
            );
        }

        // ── 8. Floating Live Drag Pill ───────────────────────────────────────────
        if let Some((cursor, lbl)) = active_drag_pill {
            paint_pill_label(
                &painter,
                pos2(cursor.x + 12.0, cursor.y - 26.0),
                &lbl,
                Color32::WHITE,
                studio::SELECTED_BLUE,
                studio::SELECTED_BLUE,
                rect,
            );
        }

        // ── 9. Top-Left Overlay Strip + Top-Right Live Cursor Readout ────────────
        self.paint_top_overlay_strip(ui, rect, doc, state, hover_pos, &screen_to_world_mm);

        // ── 10. Right-Click Context Menu ─────────────────────────────────────────
        response.context_menu(|ui| {
            ui.set_min_width(210.0);
            if ui.button("Sığdır (Görünümü Sıfırla)").clicked() {
                self.reset_view();
                ui.close_menu();
            }
            if self.measure_world.is_some() && ui.button("Ölçümü temizle").clicked() {
                self.measure_world = None;
                ui.close_menu();
            }
            ui.separator();
            ui.checkbox(&mut self.show_grid, "Izgara");
            ui.checkbox(&mut state.show_dimensions, "Ölçüler (İlmek + cm)");
            ui.checkbox(&mut state.snap_even_rows, "Çift May Snap (2, 4, 6...)");
            ui.checkbox(&mut self.show_line_numbers, "Çizgi numaraları");
            ui.checkbox(&mut self.show_markers, "İşaretler");
            ui.checkbox(&mut self.show_mirror_axis, "Orta eksen (X=0)");
        });

        doc_mutated
    }

    fn paint_top_overlay_strip(
        &mut self,
        ui: &mut Ui,
        canvas_rect: Rect,
        doc: &FormDocument,
        state: &mut UiState,
        hover_pos: Option<Pos2>,
        screen_to_world_mm: &impl Fn(Pos2) -> (f64, f64),
    ) {
        let strip_rect = Rect::from_min_max(
            pos2(canvas_rect.left() + 10.0, canvas_rect.top() + 8.0),
            pos2(canvas_rect.right() - 10.0, canvas_rect.top() + 36.0),
        );

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(strip_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        Frame::NONE
            .fill(studio::HUD_BG)
            .stroke(Stroke::new(1.0_f32, studio::HUD_BORDER))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 3))
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                if hud_chip_button(ui, "Sığdır", false).clicked() {
                    self.reset_view();
                }

                ui.label(
                    egui::RichText::new(format!("{:.0}%", self.zoom * 100.0))
                        .font(bold_font(11.0))
                        .color(studio::HUD_MUTED),
                );

                ui.separator();

                if hud_chip_button(ui, "Izgara", self.show_grid).clicked() {
                    self.show_grid = !self.show_grid;
                }
                if hud_chip_button(ui, "Ölçüler", state.show_dimensions).clicked() {
                    state.show_dimensions = !state.show_dimensions;
                }
                if hud_chip_button(ui, "Çift May Snap", state.snap_even_rows).clicked() {
                    state.snap_even_rows = !state.snap_even_rows;
                }
            });

        // Top-right live cursor readout (drops onto second row on narrow left pane so HUD never collides)
        if let Some(hp) = hover_pos {
            if canvas_rect.contains(hp) {
                let (wx_mm, wy_mm) = screen_to_world_mm(hp);
                let st_x = mm_to_stitches(wx_mm, &doc.header);
                let st_y = mm_to_rows(wy_mm, &doc.header);
                let readout = format!(
                    "X: {} ilm / {:.1} cm   Y: {} sıra / {:.1} cm",
                    st_x,
                    wx_mm / 10.0,
                    st_y,
                    wy_mm / 10.0
                );
                let painter = ui.painter().with_clip_rect(canvas_rect);
                let galley = painter.layout_no_wrap(readout, regular_font(11.0), studio::HUD_TEXT);
                let sz = galley.size() + vec2(14.0, 8.0);
                let top_y = if canvas_rect.width() < 540.0 {
                    canvas_rect.top() + 38.0
                } else {
                    canvas_rect.top() + 8.0
                };
                let r = Rect::from_min_size(
                    pos2((canvas_rect.right() - sz.x - 10.0).max(canvas_rect.left() + 10.0), top_y),
                    sz,
                );
                painter.rect_filled(r, CornerRadius::same(6), studio::HUD_BG);
                painter.rect_stroke(
                    r,
                    CornerRadius::same(6),
                    Stroke::new(1.0_f32, studio::HUD_BORDER),
                    StrokeKind::Inside,
                );
                painter.galley(r.min + vec2(7.0, 4.0), galley, studio::HUD_TEXT);
            }
        }
    }
}

fn hud_chip_button(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    let bg = if active {
        studio::HUD_ACTIVE_BG
    } else {
        Color32::TRANSPARENT
    };
    let border = if active {
        studio::HUD_ACTIVE_BORDER
    } else {
        studio::HUD_BORDER
    };
    let text_col = if active {
        studio::SELECTED_BLUE
    } else {
        studio::HUD_TEXT
    };
    let btn = egui::Button::new(egui::RichText::new(label).font(bold_font(10.5)).color(text_col))
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(4))
        .min_size(vec2(0.0, 20.0));
    ui.add(btn)
}

fn paint_cad_dimensions(
    painter: &egui::Painter,
    doc: &FormDocument,
    geom: &DocumentGeometry,
    state: &UiState,
    world_to_screen: &impl Fn(f64, f64) -> Pos2,
    canvas_rect: Rect,
) {
    let Some(eg) = geom.elements.get(state.selected_elem).or_else(|| geom.elements.first()) else {
        return;
    };
    if !eg.bounds.0.is_finite() {
        return;
    }

    let (min_x_mm, max_x_mm, min_y_mm, max_y_mm) = eg.bounds;
    let total_w_st = eg.max_width_stitches;
    let total_w_cm = eg.max_width_mm / 10.0;
    let total_h_rows = eg.total_height_rows;
    let total_h_cm = eg.total_height_mm / 10.0;

    let dim_stroke = Stroke::new(1.2_f32, studio::DIM_LINE);
    let mut placed_dim_rects: Vec<Rect> = Vec::new();
    // Reserve top HUD strip & top-right readout band so pills never collide with top HUD
    let safe_dim_rect = Rect::from_min_max(
        pos2(canvas_rect.left() + 4.0, canvas_rect.top() + 42.0),
        pos2(canvas_rect.right() - 4.0, canvas_rect.bottom() - 4.0),
    );

    // 1. Bottom total width dimension line
    let bl = world_to_screen(min_x_mm, min_y_mm);
    let br = world_to_screen(max_x_mm, min_y_mm);
    let y_dim = (bl.y.max(br.y) + 22.0).min(canvas_rect.bottom() - 18.0);
    if (br.x - bl.x).abs() > 16.0 {
        painter.line_segment([pos2(bl.x, y_dim), pos2(br.x, y_dim)], dim_stroke);
        painter.line_segment([pos2(bl.x, y_dim - 5.0), pos2(bl.x, y_dim + 5.0)], dim_stroke);
        painter.line_segment([pos2(br.x, y_dim - 5.0), pos2(br.x, y_dim + 5.0)], dim_stroke);
        let w_lbl = format!("En: {} ilmek | {:.1} cm", total_w_st, total_w_cm);
        let r = paint_pill_label(
            painter,
            pos2((bl.x + br.x) * 0.5, y_dim),
            &w_lbl,
            studio::HUD_TEXT,
            studio::DIM_PILL_BG,
            studio::DIM_PILL_BORDER,
            safe_dim_rect,
        );
        placed_dim_rects.push(r);
    }

    // 2. Right total height dimension line (pill sits strictly to the right of x_dim so it never touches contour)
    let rt = world_to_screen(max_x_mm, max_y_mm);
    let rb = world_to_screen(max_x_mm, min_y_mm);
    let x_dim = (rt.x.max(rb.x) + 18.0).min(canvas_rect.right() - 98.0);
    if (rb.y - rt.y).abs() > 16.0 {
        painter.line_segment([pos2(x_dim, rt.y), pos2(x_dim, rb.y)], dim_stroke);
        painter.line_segment([pos2(x_dim - 5.0, rt.y), pos2(x_dim + 5.0, rt.y)], dim_stroke);
        painter.line_segment([pos2(x_dim - 5.0, rb.y), pos2(x_dim + 5.0, rb.y)], dim_stroke);
        let h_lbl = format!("Boy: {} sıra\n{:.1} cm", total_h_rows, total_h_cm);
        let galley = painter.layout_no_wrap(h_lbl, bold_font(10.5), studio::HUD_TEXT);
        let sz = galley.size() + vec2(10.0, 5.0);
        let pill_cx = (x_dim + 8.0 + sz.x * 0.5)
            .min(safe_dim_rect.right() - sz.x * 0.5 - 2.0)
            .max(safe_dim_rect.left() + sz.x * 0.5 + 2.0);
        let pill_cy = ((rt.y + rb.y) * 0.5)
            .clamp(safe_dim_rect.top() + sz.y * 0.5 + 2.0, safe_dim_rect.bottom() - sz.y * 0.5 - 2.0);
        let brect = Rect::from_center_size(pos2(pill_cx, pill_cy), sz);
        painter.rect_filled(brect, CornerRadius::same(4), studio::DIM_PILL_BG);
        painter.rect_stroke(
            brect,
            CornerRadius::same(4),
            Stroke::new(1.0_f32, studio::DIM_PILL_BORDER),
            StrokeKind::Inside,
        );
        painter.galley(brect.min + vec2(5.0, 2.5), galley, studio::HUD_TEXT);
        placed_dim_rects.push(brect);
    }

    // 3. Selected segment callout with collision avoidance against contour hem & other pills
    let segs = match state.active_table_side {
        LineSide::Left => &eg.left_segments,
        LineSide::Right => &eg.right_segments,
    };
    if let Some(&row_idx) = state.selected_rows.first() {
        if let Some(seg) = segs.iter().find(|s| s.line_index == row_idx) {
            if seg.polyline_mm.len() >= 2 {
                let screen_pts: Vec<Pos2> = seg
                    .polyline_mm
                    .iter()
                    .map(|p| world_to_screen(p.x, p.y))
                    .collect();
                let mid = polyline_midpoint(&screen_pts);
                let w_st = seg.delta_st.0;
                let h_rows = seg.delta_st.1;
                let w_cm = stitches_to_mm(w_st, &doc.header) / 10.0;
                let h_cm = rows_to_mm(h_rows, &doc.header) / 10.0;
                let callout = format!(
                    "#{}: {} sıra ({:.1} cm) x {} ilm ({:.1} cm)",
                    seg.line_index + 1,
                    h_rows,
                    h_cm,
                    w_st,
                    w_cm
                );
                let is_bottom_horiz =
                    h_rows == 0 && (seg.start_mm.y - min_y_mm).abs() < 2.0;
                let offset_x = if is_bottom_horiz {
                    0.0
                } else if state.active_table_side == LineSide::Left {
                    -14.0
                } else {
                    14.0
                };
                let candidates = if is_bottom_horiz {
                    [
                        pos2(mid.x + offset_x, mid.y - 26.0),
                        pos2(mid.x + offset_x, y_dim - 24.0),
                        pos2(mid.x + offset_x, mid.y - 46.0),
                    ]
                } else {
                    [
                        pos2(mid.x + offset_x, mid.y - 22.0),
                        pos2(mid.x + offset_x, mid.y + 22.0),
                        pos2(mid.x + offset_x, mid.y - 44.0),
                    ]
                };
                let mut chosen = candidates[0];
                let galley_probe =
                    painter.layout_no_wrap(callout.clone(), bold_font(10.5), studio::SELECTED_BLUE);
                let probe_sz = galley_probe.size() + vec2(10.0, 5.0);
                for c in candidates {
                    let r = Rect::from_center_size(c, probe_sz);
                    if !placed_dim_rects.iter().any(|p| p.intersects(r)) {
                        chosen = c;
                        break;
                    }
                }
                paint_pill_label(
                    painter,
                    chosen,
                    &callout,
                    studio::SELECTED_BLUE,
                    studio::DIM_PILL_BG,
                    studio::SELECTED_BLUE,
                    safe_dim_rect,
                );
            }
        }
    }
}

fn paint_pill_label(
    painter: &egui::Painter,
    center: Pos2,
    text: &str,
    text_color: Color32,
    bg_color: Color32,
    border_color: Color32,
    clamp_rect: Rect,
) -> Rect {
    let galley = painter.layout_no_wrap(text.to_string(), bold_font(10.5), text_color);
    let sz = galley.size() + vec2(10.0, 5.0);
    let cx = center
        .x
        .clamp(clamp_rect.left() + sz.x * 0.5 + 4.0, clamp_rect.right() - sz.x * 0.5 - 4.0);
    let cy = center
        .y
        .clamp(clamp_rect.top() + sz.y * 0.5 + 4.0, clamp_rect.bottom() - sz.y * 0.5 - 4.0);
    let brect = Rect::from_center_size(pos2(cx, cy), sz);
    painter.rect_filled(brect, CornerRadius::same(4), bg_color);
    painter.rect_stroke(
        brect,
        CornerRadius::same(4),
        Stroke::new(1.0_f32, border_color),
        StrokeKind::Inside,
    );
    painter.galley(brect.min + vec2(5.0, 2.5), galley, text_color);
    brect
}

/// Ear-clipping triangulation fill so concave garments (armholes, necklines, waist) fill cleanly without fan artifacts.
fn paint_concave_polygon_fill(painter: &egui::Painter, raw_pts: &[Pos2], fill: Color32) {
    if raw_pts.len() < 3 {
        return;
    }
    let mut pts: Vec<Pos2> = Vec::with_capacity(raw_pts.len());
    for &p in raw_pts {
        if pts.last().map(|q| q.distance_sq(p) > 0.25).unwrap_or(true) {
            pts.push(p);
        }
    }
    if pts.len() >= 2 && pts.first().unwrap().distance_sq(*pts.last().unwrap()) <= 0.25 {
        pts.pop();
    }
    if pts.len() < 3 {
        return;
    }

    let area2: f32 = pts
        .windows(2)
        .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
        .sum::<f32>()
        + (pts.last().unwrap().x * pts[0].y - pts[0].x * pts.last().unwrap().y);
    if area2.abs() < 1.0 {
        return;
    }
    if area2 < 0.0 {
        pts.reverse();
    }

    let mut mesh = egui::Mesh::default();
    for &p in &pts {
        mesh.colored_vertex(p, fill);
    }

    let mut idx: Vec<u32> = (0..pts.len() as u32).collect();
    let mut guard = 0usize;
    let max_iter = pts.len() * pts.len() + 8;

    while idx.len() > 3 && guard < max_iter {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let i0 = idx[(i + m - 1) % m];
            let i1 = idx[i];
            let i2 = idx[(i + 1) % m];
            let a = pts[i0 as usize];
            let b = pts[i1 as usize];
            let c = pts[i2 as usize];
            let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            if cross <= 1e-4 {
                continue;
            }
            let any_inside = idx.iter().any(|&k| {
                if k == i0 || k == i1 || k == i2 {
                    return false;
                }
                point_in_triangle(pts[k as usize], a, b, c)
            });
            if !any_inside {
                mesh.add_triangle(i0, i1, i2);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        mesh.add_triangle(idx[0], idx[1], idx[2]);
    }
    if !mesh.indices.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
}

fn point_in_triangle(p: Pos2, a: Pos2, b: Pos2, c: Pos2) -> bool {
    let c1 = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let c2 = (c.x - b.x) * (p.y - b.y) - (c.y - b.y) * (p.x - b.x);
    let c3 = (a.x - c.x) * (p.y - c.y) - (a.y - c.y) * (p.x - c.x);
    c1 >= -1e-4 && c2 >= -1e-4 && c3 >= -1e-4
}

fn nice_axis_step_mm(raw_step: f64) -> f64 {
    if raw_step <= 0.0 || !raw_step.is_finite() {
        return 50.0;
    }
    let exp = raw_step.log10().floor();
    let mag = 10.0_f64.powf(exp);
    let norm = raw_step / mag;
    let nice = if norm <= 1.2 {
        1.0
    } else if norm <= 2.2 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    (nice * mag).max(10.0)
}

fn draw_vertical_dashed(painter: &egui::Painter, x: f32, y_top: f32, y_bot: f32, color: Color32) {
    let stroke = Stroke::new(1.1_f32, color);
    let mut y = y_top;
    while y < y_bot {
        let end = (y + 8.0).min(y_bot);
        painter.line_segment([pos2(x, y), pos2(x, end)], stroke);
        y += 14.0;
    }
}

fn draw_dashed_segment(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    let delta = b - a;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let dir = delta / len;
    let mut t = 0.0;
    while t < len {
        let t2 = (t + 6.0).min(len);
        painter.line_segment([a + dir * t, a + dir * t2], stroke);
        t += 10.0;
    }
}

fn polyline_midpoint(pts: &[Pos2]) -> Pos2 {
    if pts.is_empty() {
        return Pos2::ZERO;
    }
    if pts.len() == 1 {
        return pts[0];
    }
    let first = pts.first().unwrap();
    let last = pts.last().unwrap();
    pos2((first.x + last.x) * 0.5, (first.y + last.y) * 0.5)
}

fn paint_element_markers(
    painter: &egui::Painter,
    doc: &FormDocument,
    el_doc: &stoll_kalip_editoru::model::ShapeElement,
    eg: &stoll_kalip_editoru::math::ElementGeometry,
    world_to_screen: &impl Fn(f64, f64) -> Pos2,
) {
    let marker_col = Color32::from_rgb(0xD9, 0x77, 0x06);
    let marker_border = Color32::WHITE;

    let draw_diamond = |p: Pos2| {
        let r = 4.5;
        let pts = vec![
            pos2(p.x, p.y - r),
            pos2(p.x + r, p.y),
            pos2(p.x, p.y + r),
            pos2(p.x - r, p.y),
        ];
        painter.add(egui::Shape::convex_polygon(
            pts,
            marker_col,
            Stroke::new(1.2_f32, marker_border),
        ));
    };

    for pts in [&el_doc.left_points, &el_doc.right_points] {
        for pt in pts {
            for off in &pt.offsets {
                let wx = eg.origin_mm.x + stitches_to_mm(off.x, &doc.header);
                let wy = eg.origin_mm.y + rows_to_mm(off.y, &doc.header);
                draw_diamond(world_to_screen(wx, wy));
            }
        }
    }

    let paint_side_line_markers = |lines: &[LineSegment], segs: &[SegmentSpan], mirror_x: bool| {
        for (line, seg) in lines.iter().zip(segs.iter()) {
            for pt in &line.points {
                let has_mod = pt
                    .modules
                    .iter()
                    .any(|m| m.id != "{00000000-0000-0000-0000-000000000000}" && !m.id.is_empty());
                let has_off = pt.offsets.iter().any(|o| o.x != 0 || o.y != 0);
                if !has_mod && !has_off {
                    continue;
                }
                let anchor = if pt.index == 0 {
                    (seg.start_mm.x, seg.start_mm.y)
                } else {
                    (seg.end_mm.x, seg.end_mm.y)
                };
                let (raw_ox, oy) = pt.offsets.first().map(|o| (o.x, o.y)).unwrap_or((0, 0));
                let ox = if mirror_x { -raw_ox } else { raw_ox };
                let wx = anchor.0 + stitches_to_mm(ox, &doc.header);
                let wy = anchor.1 + rows_to_mm(oy, &doc.header);
                draw_diamond(world_to_screen(wx, wy));
            }
        }
    };

    paint_side_line_markers(&el_doc.left_lines, &eg.left_segments, false);
    if el_doc.right_lines.is_empty() && el_doc.mirrored && !eg.right_segments.is_empty() {
        paint_side_line_markers(&el_doc.left_lines, &eg.right_segments, true);
    } else {
        paint_side_line_markers(&el_doc.right_lines, &eg.right_segments, false);
    }
}

fn dist_to_polyline(p: Pos2, pts: &[Pos2]) -> f32 {
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        let d = dist_to_segment(p, w[0], w[1]);
        if d < best {
            best = d;
        }
    }
    best
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    p.distance(proj)
}
