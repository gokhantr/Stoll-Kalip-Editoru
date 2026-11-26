use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, Frame, Margin, PointerButton, Pos2, Rect, Sense,
    Stroke, StrokeKind, Ui,
};

use stoll_kalip_editoru::math::{
    mm_to_rows, mm_to_stitches, rows_to_mm, stitches_to_mm, DocumentGeometry, SegmentSpan,
};
use stoll_kalip_editoru::model::{ElementType, FormDocument, InputFormat, LineSide};

use crate::ui::theme::{bold_font, colors, regular_font};
use crate::ui::{
    line_function_default_tab, CurveEditorRequest, LinePropsRequest, TableMode, UiState,
};

/// Display unit override for the floating `Kesim:` Graphics window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphUnitMode {
    /// Follow the document's `EingabeFormat` (`[İlmekler]` for `.shp`, `[mm]` for `.shv`).
    Auto,
    Stitches,
    Millimeters,
}

/// Interactive state for the floating `Kesim:` Graphics Canvas (`M3_FormGraphDlg`, Screenshot `222138.png`).
#[derive(Debug, Clone)]
pub struct GraphicsWindowState {
    /// Zoom multiplier (`1.0` = fit to view).
    pub zoom: f32,
    /// Pan offset in screen pixels relative to fitted center.
    pub pan_px: egui::Vec2,
    /// Show subtle coordinate gridlines (`#A0B8D0`).
    pub show_grid: bool,
    /// Show standalone & line-attached marker points (default `false` matching Stoll `222138.png`).
    pub show_markers: bool,
    /// Show line segment numbers (`1, 2, ...`) along contours.
    pub show_line_numbers: bool,
    /// Show white dash-dot center symmetry axis (`X = 0`).
    pub show_mirror_axis: bool,
    /// Highlight active/selected row in red (default `false` matching Stoll `222138.png`; auto-enables on canvas click).
    pub show_selected_highlight: bool,
    /// Coordinate axis display unit (`[İlmekler]` or `[mm]`).
    pub unit_mode: GraphUnitMode,
    /// Track last hovered segment `(elem_idx, side, line_idx)` for subtle hover feedback.
    hovered_segment: Option<(usize, LineSide, usize)>,
    /// Interactive `Shift+Drag` ruler measurement in world coordinates `(start, end)`.
    measure_world: Option<((f64, f64), (f64, f64))>,
}

impl Default for GraphicsWindowState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_px: egui::Vec2::ZERO,
            show_grid: true,
            show_markers: false,
            show_line_numbers: false,
            show_mirror_axis: true,
            show_selected_highlight: false,
            unit_mode: GraphUnitMode::Auto,
            hovered_segment: None,
            measure_world: None,
        }
    }
}

impl GraphicsWindowState {
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_px = egui::Vec2::ZERO;
        self.measure_world = None;
    }

    /// Render the floating, resizable, draggable `Kesim: <file_path>` window matching Screenshot `222138.png`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &FormDocument,
        geom: &DocumentGeometry,
        state: &mut UiState,
    ) {
        if !state.show_graphics {
            return;
        }

        let mut open = state.show_graphics;
        let title = match &doc.file_path {
            Some(p) => format!("Kesim: {}", p.display()),
            None => {
                let nm = doc.header.name.trim();
                format!("Kesim: {}", if nm.is_empty() { "Yeni" } else { nm })
            }
        };

        egui::Window::new(&title)
            .id(egui::Id::new("floating_graphics_window"))
            .title_bar(false)
            .default_size(vec2(660.0, 496.0))
            .min_size(vec2(340.0, 260.0))
            .resizable(true)
            .collapsible(false)
            .frame(
                Frame::NONE
                    .fill(colors::CANVAS_BLUE)
                    .stroke(Stroke::new(1.0_f32, colors::BORDER_DARK))
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                // Classic Win32 white `#FFFFFF` titlebar (`222138.png`, `DLG-01`)
                if paint_win32_graph_titlebar(ui, &title) {
                    open = false;
                }
                self.paint_canvas(ui, doc, geom, state);
            });

        state.show_graphics = open;
    }

    fn use_mm_axes(&self, doc: &FormDocument) -> bool {
        match self.unit_mode {
            GraphUnitMode::Auto => doc.header.input_format == InputFormat::Mm,
            GraphUnitMode::Stitches => false,
            GraphUnitMode::Millimeters => true,
        }
    }

    fn paint_canvas(
        &mut self,
        ui: &mut Ui,
        doc: &FormDocument,
        geom: &DocumentGeometry,
        state: &mut UiState,
    ) {
        let avail = ui.available_size();
        let (rect, response) =
            ui.allocate_exact_size(avail.max(vec2(200.0, 160.0)), Sense::click_and_drag());

        let painter = ui.painter().with_clip_rect(rect);
        // Exact Stoll steel-blue canvas background `#78A0C8`
        painter.rect_filled(rect, CornerRadius::ZERO, colors::CANVAS_BLUE);

        let use_mm = self.use_mm_axes(doc);

        // Compute world bounds in active display coordinates (stitches/rows or mm)
        let (mut min_x, mut max_x, mut min_y, mut max_y) = compute_world_bounds(geom, use_mm);
        // Ensure symmetric X around 0 if the shape spans both sides, matching Screenshot 222138.png
        let max_abs_x = min_x.abs().max(max_x.abs()).max(20.0);
        if min_x < 0.0 && max_x > 0.0 {
            min_x = -max_abs_x;
            max_x = max_abs_x;
        }
        min_y = min_y.min(0.0);
        max_y = max_y.max(40.0);

        // Choose nice tick intervals matching Stoll (`25` on X for ±100, `40` on Y for 0..360)
        let x_step = nice_axis_step((max_x - min_x) / 8.0);
        let y_step = nice_axis_step((max_y - min_y) / 9.0);

        let axis_min_x = (min_x / x_step).floor() * x_step;
        let axis_max_x = (max_x / x_step).ceil() * x_step;
        let axis_min_y = (min_y / y_step).floor() * y_step;
        // Keep Y top tight to shape top (like 368 -> top tick 360 in 222138.png) instead of wasting a full extra grid band
        let axis_max_y = max_y.max(axis_min_y + y_step);

        // Plot margins leaving crisp space for Y labels on left, `[İlmekler]` on top, X labels on bottom
        let pad_left = 54.0;
        let pad_right = 24.0;
        let pad_top = 34.0;
        let pad_bottom = 34.0;

        let plot_rect = Rect::from_min_max(
            pos2(rect.left() + pad_left, rect.top() + pad_top),
            pos2(
                (rect.right() - pad_right).max(rect.left() + pad_left + 40.0),
                (rect.bottom() - pad_bottom).max(rect.top() + pad_top + 40.0),
            ),
        );

        let span_x = (axis_max_x - axis_min_x).max(1.0);
        let span_y = (axis_max_y - axis_min_y).max(1.0);

        // Independent X/Y auto-fit fill matching Stoll `222138.png` (`-100..+100` fills width, `0..368` fills height)
        let base_scale_x = (plot_rect.width() as f64 / span_x).max(1e-4);
        let base_scale_y = (plot_rect.height() as f64 / span_y).max(1e-4);
        let scale_x = base_scale_x * (self.zoom as f64);
        let scale_y = base_scale_y * (self.zoom as f64);

        let world_cx = (axis_min_x + axis_max_x) * 0.5;
        let world_cy = (axis_min_y + axis_max_y) * 0.5;
        let screen_cx = plot_rect.center().x + self.pan_px.x;
        let screen_cy = plot_rect.center().y + self.pan_px.y;

        let world_to_screen = |wx: f64, wy: f64| -> Pos2 {
            let sx = screen_cx + ((wx - world_cx) * scale_x) as f32;
            let sy = screen_cy - ((wy - world_cy) * scale_y) as f32;
            pos2(sx, sy)
        };

        let screen_to_world = |p: Pos2| -> (f64, f64) {
            let wx = world_cx + ((p.x - screen_cx) as f64) / scale_x;
            let wy = world_cy - ((p.y - screen_cy) as f64) / scale_y;
            (wx, wy)
        };

        // ── Zoom, Pan & Shift+Drag Measure interactions (`DLG-02`) ──────────────
        let shift_held = ui.input(|i| i.modifiers.shift);
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.measure_world = None;
        }

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let factor = (1.0 + scroll * 0.0025).clamp(0.75, 1.35);
                let old_zoom = self.zoom;
                let new_zoom = (self.zoom * factor).clamp(0.25, 25.0);
                if let Some(hover_pos) = response.hover_pos() {
                    // Zoom centered around cursor position
                    let rel = hover_pos - (plot_rect.center() + self.pan_px);
                    let ratio = new_zoom / old_zoom;
                    self.pan_px -= rel * (ratio - 1.0);
                }
                self.zoom = new_zoom;
            }
        }

        if shift_held && response.drag_started_by(PointerButton::Primary) {
            if let Some(hp) = response.interact_pointer_pos() {
                let w = screen_to_world(hp);
                self.measure_world = Some((w, w));
            }
        } else if shift_held && response.dragged_by(PointerButton::Primary) {
            if let (Some((start, _)), Some(hp)) = (self.measure_world, response.interact_pointer_pos())
            {
                self.measure_world = Some((start, screen_to_world(hp)));
            }
        } else if response.dragged_by(PointerButton::Middle)
            || (response.dragged_by(PointerButton::Primary) && !shift_held)
        {
            self.pan_px += response.drag_delta();
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        if response.hovered() {
            if shift_held {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if self.hovered_segment.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        // Compute visible world range for dynamic grid & axes when zoomed/panned
        let (vis_x0, vis_y1) = screen_to_world(plot_rect.left_top());
        let (vis_x1, vis_y0) = screen_to_world(plot_rect.right_bottom());

        // Dynamic tick step when zoomed in deeply
        let vis_x_step = nice_axis_step((vis_x1 - vis_x0) / 8.0);
        let vis_y_step = nice_axis_step((vis_y1 - vis_y0) / 9.0);
        let draw_x_step = if (self.zoom - 1.0).abs() < 0.02 {
            x_step
        } else {
            vis_x_step
        };
        let draw_y_step = if (self.zoom - 1.0).abs() < 0.02 {
            y_step
        } else {
            vis_y_step
        };

        let x_axis_left = world_to_screen(axis_min_x, 0.0).x.clamp(plot_rect.left(), plot_rect.right());
        let x_axis_right = world_to_screen(axis_max_x, 0.0).x.clamp(plot_rect.left(), plot_rect.right());
        let y_axis_bottom = world_to_screen(0.0, axis_min_y).y.clamp(plot_rect.top(), plot_rect.bottom());
        let y_axis_top = world_to_screen(0.0, axis_max_y).y.clamp(plot_rect.top(), plot_rect.bottom());

        // When zoomed/panned, anchor the X-axis at y=0 (or bottom of plot if y=0 offscreen)
        // and Y-axis at axis_min_x (or left of plot if offscreen)
        let y_zero_screen = world_to_screen(0.0, 0.0)
            .y
            .clamp(plot_rect.top(), plot_rect.bottom());
        let x_left_axis_screen = if (self.zoom - 1.0).abs() < 0.02 && self.pan_px.length() < 1.0 {
            world_to_screen(axis_min_x, 0.0).x
        } else {
            plot_rect.left()
        };

        let grid_left = x_left_axis_screen.min(plot_rect.left());
        let grid_right = x_axis_right.max(plot_rect.right());
        let grid_top = y_axis_top.min(plot_rect.top());
        let grid_bottom = y_zero_screen.max(y_axis_bottom);

        // ── 1. Subtle coordinate gridlines (`#A0B8D0`) ──────────────────────────
        let tick_x_start = if (self.zoom - 1.0).abs() < 0.02 && self.pan_px.length() < 1.0 {
            (axis_min_x / draw_x_step).round() as i64
        } else {
            ((vis_x0 / draw_x_step).floor() as i64) - 1
        };
        let tick_x_end = if (self.zoom - 1.0).abs() < 0.02 && self.pan_px.length() < 1.0 {
            (axis_max_x / draw_x_step).round() as i64
        } else {
            ((vis_x1 / draw_x_step).ceil() as i64) + 1
        };
        let tick_y_start = if (self.zoom - 1.0).abs() < 0.02 && self.pan_px.length() < 1.0 {
            (axis_min_y / draw_y_step).round() as i64
        } else {
            ((vis_y0 / draw_y_step).floor() as i64) - 1
        };
        let tick_y_end = if (self.zoom - 1.0).abs() < 0.02 && self.pan_px.length() < 1.0 {
            (axis_max_y / draw_y_step).floor() as i64
        } else {
            ((vis_y1 / draw_y_step).ceil() as i64) + 1
        };

        if self.show_grid {
            let grid_stroke = Stroke::new(1.0_f32, colors::GRAPH_GRID);
            for ix in tick_x_start..=tick_x_end {
                let wx = ix as f64 * draw_x_step;
                let sx = world_to_screen(wx, 0.0).x;
                if sx >= x_left_axis_screen - 0.5 && sx <= plot_rect.right() + 8.0 {
                    painter.line_segment([pos2(sx, grid_top), pos2(sx, y_zero_screen)], grid_stroke);
                }
            }
            for iy in tick_y_start..=tick_y_end {
                let wy = iy as f64 * draw_y_step;
                let sy = world_to_screen(0.0, wy).y;
                if sy >= plot_rect.top() - 4.0 && sy <= y_zero_screen + 0.5 {
                    painter.line_segment(
                        [pos2(x_left_axis_screen, sy), pos2(grid_right, sy)],
                        grid_stroke,
                    );
                }
            }
        }

        // ── 2. Black X and Y Axes with tick marks & bold labels (`222138.png`) ──
        let axis_stroke = Stroke::new(1.0_f32, colors::BORDER_BLACK);
        // Horizontal X axis at y = 0 (extending slightly left/right like Stoll)
        let x_line_l = (x_left_axis_screen - 32.0).max(rect.left() + 4.0);
        let x_line_r = (grid_right + 14.0).min(rect.right() - 4.0);
        painter.line_segment(
            [pos2(x_line_l, y_zero_screen), pos2(x_line_r, y_zero_screen)],
            axis_stroke,
        );
        // Vertical Y axis at left edge of grid
        let y_line_t = (grid_top - 4.0).max(rect.top() + 18.0);
        let y_line_b = (y_zero_screen + 16.0).min(rect.bottom() - 4.0);
        painter.line_segment(
            [pos2(x_left_axis_screen, y_line_t), pos2(x_left_axis_screen, y_line_b)],
            axis_stroke,
        );

        // Unit label `[İlmekler]` or `[mm]` at top-left of Y-axis (`222138.png`)
        let unit_label = if use_mm { "[mm]" } else { "[İlmekler]" };
        painter.text(
            pos2(x_left_axis_screen + 4.0, (y_line_t - 4.0).max(rect.top() + 14.0)),
            Align2::LEFT_BOTTOM,
            unit_label,
            bold_font(12.5),
            colors::TEXT_BLACK,
        );

        // X-axis ticks & labels (10px downward ticks matching `222138.png`)
        let mut last_x_lbl_right = f32::NEG_INFINITY;
        for ix in tick_x_start..=tick_x_end {
            let wx = ix as f64 * draw_x_step;
            let sx = world_to_screen(wx, 0.0).x;
            if sx < x_left_axis_screen - 1.0 || sx > plot_rect.right() + 10.0 {
                continue;
            }
            // Tick mark downward 10px
            painter.line_segment(
                [pos2(sx, y_zero_screen), pos2(sx, y_zero_screen + 10.0)],
                axis_stroke,
            );
            if sx - 16.0 > last_x_lbl_right {
                let txt = format_axis_num(wx);
                painter.text(
                    pos2(sx, y_zero_screen + 12.0),
                    Align2::CENTER_TOP,
                    &txt,
                    bold_font(11.5),
                    colors::TEXT_BLACK,
                );
                last_x_lbl_right = sx + 18.0;
            }
        }

        // Y-axis ticks & labels (12px leftward ticks, bold numbers resting just above each tick like `222138.png`)
        let mut last_y_lbl_top = f32::INFINITY;
        for iy in tick_y_start..=tick_y_end {
            let wy = iy as f64 * draw_y_step;
            if wy < -1e-6 {
                continue;
            }
            let sy = world_to_screen(0.0, wy).y;
            if sy < plot_rect.top() - 6.0 || sy > y_zero_screen + 1.0 {
                continue;
            }
            // Tick mark leftward 12px
            painter.line_segment(
                [pos2(x_left_axis_screen - 12.0, sy), pos2(x_left_axis_screen, sy)],
                axis_stroke,
            );
            if sy + 12.0 < last_y_lbl_top {
                let txt = format_axis_num(wy);
                painter.text(
                    pos2(x_left_axis_screen - 4.0, sy - 2.0),
                    Align2::RIGHT_BOTTOM,
                    &txt,
                    bold_font(11.5),
                    colors::TEXT_BLACK,
                );
                last_y_lbl_top = sy;
            }
        }

        // ── 3. White dash-dot vertical center symmetry axis (`X = 0`) ────────────
        if self.show_mirror_axis {
            let ax = world_to_screen(0.0, 0.0).x;
            if ax >= rect.left() && ax <= rect.right() {
                draw_vertical_dash_dot(
                    &painter,
                    ax,
                    grid_top.max(rect.top() + 8.0),
                    y_zero_screen.min(rect.bottom() - 8.0),
                    colors::AXIS_WHITE,
                );
            }
        }

        // ── 4. Shape contours + hit-testing for click/hover selection ────────────
        let hover_pos = response.hover_pos();
        let mut closest_hit: Option<(f32, usize, LineSide, usize)> = None;

        let active_elem = state.selected_elem;
        let active_side = state.active_table_side;
        let mut placed_num_rects: Vec<Rect> = Vec::new();

        for (ei, eg) in geom.elements.iter().enumerate() {
            if eg.hidden {
                continue;
            }

            let base_color = if ei == 0 || eg.element_type == ElementType::BasicShape {
                colors::CONTOUR_GREEN
            } else {
                match eg.element_type {
                    ElementType::Neckline | ElementType::NecklineMarking => colors::SECONDARY_YELLOW,
                    ElementType::Hole | ElementType::HoleMarking => Color32::from_rgb(0x40, 0xFF, 0xFF),
                    ElementType::Gore => Color32::from_rgb(0xFF, 0xA0, 0x30),
                    _ => colors::SECONDARY_YELLOW,
                }
            };

            let sides: [(LineSide, &[SegmentSpan]); 2] = [
                (LineSide::Left, &eg.left_segments),
                (LineSide::Right, &eg.right_segments),
            ];

            for (side, segs) in sides {
                for seg in segs {
                    let pts_src = if use_mm {
                        &seg.polyline_mm
                    } else {
                        &seg.polyline_st
                    };
                    if pts_src.len() < 2 {
                        continue;
                    }

                    let screen_pts: Vec<Pos2> = pts_src
                        .iter()
                        .map(|p| world_to_screen(p.x, p.y))
                        .collect();

                    let is_selected = self.show_selected_highlight
                        && ei == active_elem
                        && (side == active_side || (eg.mirrored && active_side == LineSide::Left))
                        && state.selected_rows.contains(&seg.line_index);
                    let is_hovered =
                        self.hovered_segment == Some((ei, side, seg.line_index));

                    // Hit test against hover cursor
                    if let Some(hp) = hover_pos {
                        let d = dist_to_polyline(hp, &screen_pts);
                        if d < 7.5 {
                            if closest_hit.map(|(best, _, _, _)| d < best).unwrap_or(true) {
                                closest_hit = Some((d, ei, side, seg.line_index));
                            }
                        }
                    }

                    let (stroke_col, stroke_w): (Color32, f32) = if is_selected {
                        (colors::SELECTED_LINE_RED, 2.0_f32)
                    } else if is_hovered {
                        (Color32::from_rgb(0xFF, 0xFF, 0x60), 1.8_f32)
                    } else {
                        (base_color, 1.0_f32)
                    };

                    for w in screen_pts.windows(2) {
                        painter.line_segment([w[0], w[1]], Stroke::new(stroke_w, stroke_col));
                    }

                    // Endpoint dots on selected segment
                    if is_selected {
                        if let (Some(&first), Some(&last)) =
                            (screen_pts.first(), screen_pts.last())
                        {
                            painter.circle_filled(first, 3.0, Color32::YELLOW);
                            painter.circle_filled(last, 3.0, Color32::YELLOW);
                        }
                    }

                    // Optional line number labels (skip tiny segments & overlapping labels)
                    if self.show_line_numbers && screen_pts.len() >= 2 {
                        let first_pt = *screen_pts.first().unwrap();
                        let last_pt = *screen_pts.last().unwrap();
                        let is_horiz = seg.delta_st.1 == 0;
                        let skip_dup = eg.mirrored
                            && side == LineSide::Right
                            && is_horiz
                            && seg.start_st.0.abs().min(seg.end_st.0.abs()) <= 2;
                        if first_pt.distance(last_pt) >= 14.0 && !skip_dup {
                            let mid = screen_pts[screen_pts.len() / 2];
                            let num_txt = format!("{}", seg.line_index + 1);
                            let lbl_pos = pos2(
                                mid.x + if side == LineSide::Left { -10.0 } else { 10.0 },
                                mid.y - 6.0,
                            );
                            let col = if is_selected {
                                Color32::YELLOW
                            } else {
                                Color32::WHITE
                            };
                            let galley = painter.layout_no_wrap(num_txt, bold_font(10.0), col);
                            let r = Rect::from_center_size(lbl_pos, galley.size() + vec2(4.0, 2.0));
                            if rect.contains_rect(r)
                                && !placed_num_rects.iter().any(|p| p.intersects(r))
                            {
                                painter.galley(r.min + vec2(2.0, 1.0), galley, col);
                                placed_num_rects.push(r);
                            }
                        }
                    }
                }
            }

            // ── 5. Marker points (standalone & line-attached) ────────────────────
            if self.show_markers {
                if let Some(el_doc) = doc.elements.get(ei) {
                    paint_element_markers(
                        &painter,
                        doc,
                        el_doc,
                        eg,
                        use_mm,
                        &world_to_screen,
                    );
                }
            }
        }

        self.hovered_segment = closest_hit.map(|(_, ei, side, li)| (ei, side, li));

        // ── 6. Click / Double-click interaction on canvas ────────────────────────
        if response.double_clicked() {
            if let Some((_, ei, side, li)) = closest_hit {
                let effective_side = if doc.elements.get(ei).map(|e| e.mirrored).unwrap_or(true) {
                    LineSide::Left
                } else {
                    side
                };
                state.selected_elem = ei;
                state.active_table_side = effective_side;
                state.table_mode = match effective_side {
                    LineSide::Left => TableMode::LeftLines,
                    LineSide::Right => TableMode::RightLines,
                };
                state.select_single_row(li);
                state.focused_cell = Some((li, 11));
                let target_line = doc.elements.get(ei).and_then(|e| e.lines(effective_side).get(li));
                if target_line.and_then(|l| l.curve.as_ref()).is_some() {
                    state.curve_editor_req = Some(CurveEditorRequest {
                        elem_idx: ei,
                        side: effective_side,
                        line_idx: li,
                    });
                } else {
                    let init_tab = target_line
                        .map(|l| line_function_default_tab(l.function.to_id()))
                        .unwrap_or(0);
                    state.line_props_req = Some(LinePropsRequest {
                        elem_idx: ei,
                        side: effective_side,
                        line_idx: li,
                        initial_tab: init_tab,
                    });
                }
            } else {
                // Double-click empty canvas resets zoom / fits to view
                self.reset_view();
            }
        } else if response.clicked() {
            if let Some((_, ei, side, li)) = closest_hit {
                let effective_side = if doc.elements.get(ei).map(|e| e.mirrored).unwrap_or(true) {
                    LineSide::Left
                } else {
                    side
                };
                self.show_selected_highlight = true;
                state.selected_elem = ei;
                state.active_table_side = effective_side;
                state.table_mode = match effective_side {
                    LineSide::Left => TableMode::LeftLines,
                    LineSide::Right => TableMode::RightLines,
                };
                state.select_single_row(li);
                state.focused_cell = Some((li, 2));
                state.editing_cell = None;
            }
        }

        // ── 7. Live hover coordinate & segment readout overlay ───────────────────
        if let Some(hp) = hover_pos {
            let (wx, wy) = screen_to_world(hp);
            let (st_x, st_y, mm_x, mm_y) = if use_mm {
                (
                    mm_to_stitches(wx, &doc.header),
                    mm_to_rows(wy, &doc.header),
                    wx,
                    wy,
                )
            } else {
                (
                    wx.round() as i32,
                    wy.round() as i32,
                    stitches_to_mm(wx.round() as i32, &doc.header),
                    rows_to_mm(wy.round() as i32, &doc.header),
                )
            };

            let seg_info = if let Some((ei, side, li)) = self.hovered_segment {
                let s_lbl = match side {
                    LineSide::Left => "Sol",
                    LineSide::Right => "Sağ",
                };
                format!("  |  E{} {} #{}", ei + 1, s_lbl, li + 1)
            } else {
                String::new()
            };

            let zoom_info = if (self.zoom - 1.0).abs() > 0.02 {
                format!("  |  {:.0}%", self.zoom * 100.0)
            } else {
                String::new()
            };

            let readout = format!(
                "X: {} ({:.1} mm)  Y: {} ({:.1} mm){}{}",
                st_x, mm_x, st_y, mm_y, seg_info, zoom_info
            );

            let galley = painter.layout_no_wrap(readout, regular_font(10.5), Color32::WHITE);
            let badge_size = galley.size() + vec2(10.0, 4.0);
            let badge_rect = Rect::from_min_size(
                pos2(rect.right() - badge_size.x - 6.0, rect.top() + 5.0),
                badge_size,
            );
            painter.rect_filled(
                badge_rect,
                CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(20, 42, 70, 195),
            );
            painter.rect_stroke(
                badge_rect,
                CornerRadius::ZERO,
                Stroke::new(1.0_f32, Color32::from_rgb(160, 195, 230)),
                StrokeKind::Inside,
            );
            painter.galley(badge_rect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
        }

        // ── 7b. Interactive `Shift+Drag` Measure Ruler Overlay (`DLG-02`) ────────
        if let Some(((ax, ay), (bx, by))) = self.measure_world {
            let pa = world_to_screen(ax, ay);
            let pb = world_to_screen(bx, by);
            let ruler_col = Color32::from_rgb(0xFF, 0xF0, 0x40);
            draw_dashed_segment(&painter, pa, pb, Stroke::new(1.4_f32, ruler_col));
            for p in [pa, pb] {
                painter.line_segment(
                    [pos2(p.x - 5.0, p.y), pos2(p.x + 5.0, p.y)],
                    Stroke::new(1.2_f32, Color32::WHITE),
                );
                painter.line_segment(
                    [pos2(p.x, p.y - 5.0), pos2(p.x, p.y + 5.0)],
                    Stroke::new(1.2_f32, Color32::WHITE),
                );
            }
            let (dx_st, dy_st, dx_mm, dy_mm) = if use_mm {
                let dmx = bx - ax;
                let dmy = by - ay;
                (
                    mm_to_stitches(dmx, &doc.header),
                    mm_to_rows(dmy, &doc.header),
                    dmx,
                    dmy,
                )
            } else {
                let dst = (bx - ax).round() as i32;
                let drw = (by - ay).round() as i32;
                (
                    dst,
                    drw,
                    stitches_to_mm(dst, &doc.header),
                    rows_to_mm(drw, &doc.header),
                )
            };
            let dist_mm = (dx_mm * dx_mm + dy_mm * dy_mm).sqrt();
            let lbl = format!(
                "Ölçüm: dX={} ({:.1} mm)  dY={} ({:.1} mm)  L={:.1} mm",
                dx_st, dx_mm, dy_st, dy_mm, dist_mm
            );
            let galley = painter.layout_no_wrap(lbl, bold_font(10.5), Color32::WHITE);
            let mid = pos2((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5 - 16.0);
            let bsz = galley.size() + vec2(10.0, 4.0);
            let brect = Rect::from_center_size(
                pos2(
                    mid.x.clamp(rect.left() + bsz.x * 0.5 + 4.0, rect.right() - bsz.x * 0.5 - 4.0),
                    mid.y.clamp(rect.top() + bsz.y * 0.5 + 4.0, rect.bottom() - bsz.y * 0.5 - 4.0),
                ),
                bsz,
            );
            painter.rect_filled(
                brect,
                CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(15, 32, 56, 220),
            );
            painter.rect_stroke(
                brect,
                CornerRadius::ZERO,
                Stroke::new(1.0_f32, ruler_col),
                StrokeKind::Inside,
            );
            painter.galley(brect.min + vec2(5.0, 2.0), galley, Color32::WHITE);
        }

        // ── 8. Right-click context menu ──────────────────────────────────────────
        response.context_menu(|ui| {
            ui.set_min_width(210.0);
            if ui.button("Yakınlaştırmayı sıfırla (Sığdır)").clicked() {
                self.reset_view();
                ui.close_menu();
            }
            if ui.button("Seçili çizgiye yakınlaştır").clicked() {
                self.zoom_to_selected(
                    geom,
                    state,
                    use_mm,
                    plot_rect,
                    world_cx,
                    world_cy,
                    base_scale_x,
                    base_scale_y,
                );
                ui.close_menu();
            }
            if self.measure_world.is_some() && ui.button("Ölçümü temizle").clicked() {
                self.measure_world = None;
                ui.close_menu();
            }
            ui.separator();
            ui.checkbox(&mut self.show_grid, "Izgarayı göster");
            ui.checkbox(&mut self.show_markers, "İşaretleri göster");
            ui.checkbox(&mut self.show_line_numbers, "Çizgi numaralarını göster");
            ui.checkbox(&mut self.show_mirror_axis, "Orta ekseni göster");
            ui.checkbox(&mut self.show_selected_highlight, "Seçili çizgiyi vurgula");
            ui.separator();
            ui.label(egui::RichText::new("Eksen birimi:").font(bold_font(10.5)));
            ui.radio_value(&mut self.unit_mode, GraphUnitMode::Auto, "Otomatik (Belge)");
            ui.radio_value(&mut self.unit_mode, GraphUnitMode::Stitches, "[İlmekler]");
            ui.radio_value(&mut self.unit_mode, GraphUnitMode::Millimeters, "[mm]");
        });

        let _ = (grid_left, grid_bottom, x_axis_left);
    }

    #[allow(clippy::too_many_arguments)]
    fn zoom_to_selected(
        &mut self,
        geom: &DocumentGeometry,
        state: &UiState,
        use_mm: bool,
        plot_rect: Rect,
        world_cx: f64,
        world_cy: f64,
        base_scale_x: f64,
        base_scale_y: f64,
    ) {
        let Some(eg) = geom.elements.get(state.selected_elem) else {
            return;
        };
        let segs = match state.active_table_side {
            LineSide::Left => &eg.left_segments,
            LineSide::Right => &eg.right_segments,
        };
        let mut s_min_x = f64::INFINITY;
        let mut s_max_x = f64::NEG_INFINITY;
        let mut s_min_y = f64::INFINITY;
        let mut s_max_y = f64::NEG_INFINITY;
        for seg in segs {
            if !state.selected_rows.contains(&seg.line_index) {
                continue;
            }
            let pts = if use_mm { &seg.polyline_mm } else { &seg.polyline_st };
            for p in pts {
                s_min_x = s_min_x.min(p.x);
                s_max_x = s_max_x.max(p.x);
                s_min_y = s_min_y.min(p.y);
                s_max_y = s_max_y.max(p.y);
            }
        }
        if !s_min_x.is_finite() {
            return;
        }
        let sel_w = (s_max_x - s_min_x).max(12.0);
        let sel_h = (s_max_y - s_min_y).max(12.0);
        let zx = (plot_rect.width() as f64 * 0.65) / (sel_w * base_scale_x);
        let zy = (plot_rect.height() as f64 * 0.65) / (sel_h * base_scale_y);
        let new_zoom = (zx.min(zy) as f32).clamp(1.0, 12.0);
        let target_cx = (s_min_x + s_max_x) * 0.5;
        let target_cy = (s_min_y + s_max_y) * 0.5;
        self.zoom = new_zoom;
        self.pan_px = vec2(
            -((target_cx - world_cx) * base_scale_x * new_zoom as f64) as f32,
            ((target_cy - world_cy) * base_scale_y * new_zoom as f64) as f32,
        );
        self.show_selected_highlight = true;
    }
}

/// Classic Win32 white `#FFFFFF` titlebar (`222138.png`, `DLG-01`) with mini graph icon & hover-red `X`.
/// Returns `true` if the close button was clicked.
fn paint_win32_graph_titlebar(ui: &mut Ui, title: &str) -> bool {
    let w = ui.available_width().max(240.0);
    let (bar_rect, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(bar_rect, CornerRadius::ZERO, Color32::WHITE);
    painter.line_segment(
        [bar_rect.left_bottom(), bar_rect.right_bottom()],
        Stroke::new(1.0_f32, Color32::from_rgb(0xD8, 0xD8, 0xD8)),
    );

    // Small 14x14 Stoll graph icon on left
    let ic = Rect::from_min_size(pos2(bar_rect.left() + 7.0, bar_rect.top() + 6.0), vec2(14.0, 14.0));
    painter.rect_filled(ic, CornerRadius::ZERO, colors::CANVAS_BLUE);
    painter.rect_stroke(ic, CornerRadius::ZERO, Stroke::new(1.0_f32, colors::BORDER_DARK), StrokeKind::Inside);
    painter.line_segment(
        [pos2(ic.left() + 3.0, ic.bottom() - 2.0), pos2(ic.left() + 3.0, ic.top() + 3.0)],
        Stroke::new(1.2_f32, colors::CONTOUR_GREEN),
    );
    painter.line_segment(
        [pos2(ic.right() - 3.0, ic.bottom() - 2.0), pos2(ic.right() - 3.0, ic.top() + 3.0)],
        Stroke::new(1.2_f32, colors::CONTOUR_GREEN),
    );
    painter.line_segment(
        [pos2(ic.left() + 3.0, ic.top() + 3.0), pos2(ic.right() - 3.0, ic.top() + 3.0)],
        Stroke::new(1.2_f32, colors::CONTOUR_GREEN),
    );

    // Close button (`X`) on right (36x25)
    let close_rect = Rect::from_min_max(
        pos2(bar_rect.right() - 36.0, bar_rect.top()),
        pos2(bar_rect.right(), bar_rect.bottom() - 1.0),
    );
    let close_resp = ui.interact(close_rect, ui.id().with("graph_close_x"), Sense::click());
    let close_hov = close_resp.hovered();
    if close_hov {
        painter.rect_filled(close_rect, CornerRadius::ZERO, Color32::from_rgb(0xE8, 0x11, 0x23));
    }
    let xc = close_rect.center();
    let x_col = if close_hov { Color32::WHITE } else { colors::TEXT_BLACK };
    let x_st = Stroke::new(1.1_f32, x_col);
    painter.line_segment([pos2(xc.x - 4.5, xc.y - 4.5), pos2(xc.x + 4.5, xc.y + 4.5)], x_st);
    painter.line_segment([pos2(xc.x - 4.5, xc.y + 4.5), pos2(xc.x + 4.5, xc.y - 4.5)], x_st);

    // Left-aligned 11.5pt regular black title text clipped before the X button
    let text_clip = Rect::from_min_max(
        pos2(ic.right() + 6.0, bar_rect.top()),
        pos2(close_rect.left() - 6.0, bar_rect.bottom()),
    );
    painter.with_clip_rect(text_clip).text(
        pos2(text_clip.left(), bar_rect.center().y),
        Align2::LEFT_CENTER,
        title,
        regular_font(11.5),
        colors::TEXT_BLACK,
    );

    close_resp.clicked()
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

fn compute_world_bounds(geom: &DocumentGeometry, use_mm: bool) -> (f64, f64, f64, f64) {
    if use_mm {
        if geom.bounds.0.is_finite() {
            return geom.bounds;
        }
        return (-250.0, 250.0, 0.0, 500.0);
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for eg in &geom.elements {
        if eg.hidden {
            continue;
        }
        for seg in eg.left_segments.iter().chain(eg.right_segments.iter()) {
            for p in &seg.polyline_st {
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x);
                min_y = min_y.min(p.y);
                max_y = max_y.max(p.y);
            }
        }
    }

    if !min_x.is_finite() {
        (-100.0, 100.0, 0.0, 360.0)
    } else {
        (min_x, max_x, min_y, max_y)
    }
}

/// Choose crisp Stoll-like round axis intervals (`1, 2, 5, 10, 20, 25, 40, 50, 100, ...`).
fn nice_axis_step(raw_step: f64) -> f64 {
    if raw_step <= 0.0 || !raw_step.is_finite() {
        return 10.0;
    }
    let exp = raw_step.log10().floor();
    let mag = 10.0_f64.powf(exp);
    let norm = raw_step / mag;
    let nice = if norm <= 1.2 {
        1.0
    } else if norm <= 2.2 {
        2.0
    } else if norm <= 2.8 {
        2.5
    } else if norm <= 4.5 {
        4.0
    } else if norm <= 6.0 {
        5.0
    } else {
        10.0
    };
    (nice * mag).max(1.0)
}

fn format_axis_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-4 {
        let iv = v.round() as i64;
        format!("{iv}")
    } else {
        format!("{v:.1}")
    }
}

fn draw_vertical_dash_dot(painter: &egui::Painter, x: f32, y_top: f32, y_bot: f32, color: Color32) {
    let stroke = Stroke::new(1.0_f32, color);
    let mut y = y_top;
    while y < y_bot {
        let d_end = (y + 10.0).min(y_bot);
        painter.line_segment([pos2(x, y), pos2(x, d_end)], stroke);
        y += 14.0;
        if y < y_bot {
            let dot_end = (y + 2.0).min(y_bot);
            painter.line_segment([pos2(x, y), pos2(x, dot_end)], stroke);
        }
        y += 6.0;
    }
}

fn paint_element_markers(
    painter: &egui::Painter,
    doc: &FormDocument,
    el_doc: &stoll_kalip_editoru::model::ShapeElement,
    eg: &stoll_kalip_editoru::math::ElementGeometry,
    use_mm: bool,
    world_to_screen: &impl Fn(f64, f64) -> Pos2,
) {
    let marker_col = Color32::from_rgb(0xFF, 0xFF, 0x40);
    let marker_border = Color32::from_rgb(0x30, 0x30, 0x00);

    let draw_diamond = |p: Pos2| {
        let r = 4.0;
        let pts = vec![
            pos2(p.x, p.y - r),
            pos2(p.x + r, p.y),
            pos2(p.x, p.y + r),
            pos2(p.x - r, p.y),
        ];
        painter.add(egui::Shape::convex_polygon(
            pts,
            marker_col,
            Stroke::new(1.0_f32, marker_border),
        ));
    };

    // Standalone markers (`[PunkteLinks]` / `[PunkteRechts]`)
    for (pts, sign) in [(&el_doc.left_points, 1.0_f64), (&el_doc.right_points, 1.0_f64)] {
        for pt in pts {
            for off in &pt.offsets {
                let wx = if use_mm {
                    eg.origin_mm.x + stitches_to_mm(off.x, &doc.header) * sign
                } else {
                    eg.origin_st.0 as f64 + (off.x as f64) * sign
                };
                let wy = if use_mm {
                    eg.origin_mm.y + rows_to_mm(off.y, &doc.header)
                } else {
                    eg.origin_st.1 as f64 + (off.y as f64)
                };
                draw_diamond(world_to_screen(wx, wy));
            }
        }
    }

    // Line-attached markers (Başlama / Bitiş `[Punkt]` with assigned modules or non-zero offsets)
    for (lines, segs) in [
        (&el_doc.left_lines, &eg.left_segments),
        (&el_doc.right_lines, &eg.right_segments),
    ] {
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
                    if use_mm {
                        (seg.start_mm.x, seg.start_mm.y)
                    } else {
                        (seg.start_st.0 as f64, seg.start_st.1 as f64)
                    }
                } else if use_mm {
                    (seg.end_mm.x, seg.end_mm.y)
                } else {
                    (seg.end_st.0 as f64, seg.end_st.1 as f64)
                };
                let (ox, oy) = pt.offsets.first().map(|o| (o.x, o.y)).unwrap_or((0, 0));
                let wx = anchor.0
                    + if use_mm {
                        stitches_to_mm(ox, &doc.header)
                    } else {
                        ox as f64
                    };
                let wy = anchor.1
                    + if use_mm {
                        rows_to_mm(oy, &doc.header)
                    } else {
                        oy as f64
                    };
                draw_diamond(world_to_screen(wx, wy));
            }
        }
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
