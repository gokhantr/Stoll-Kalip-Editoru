#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;

use std::path::PathBuf;

use stoll_kalip_editoru::catalog::{
    enable_per_monitor_dpi_awareness, primary_monitor_scale_factor,
    validate_saved_window_placement, UserSettings,
};

use crate::app::KalipEditorApp;

fn main() -> eframe::Result<()> {
    let initial_path: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);
    let is_capture = std::env::var_os("KALIPEDITOR_CAPTURE_DIR").is_some();

    let saved_placement = if !is_capture {
        enable_per_monitor_dpi_awareness();
        let settings = UserSettings::load();
        validate_saved_window_placement(&settings)
    } else {
        None
    };

    let icon_data = build_app_icon();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Yeni - Stoll Kalıp Editörü")
        .with_min_inner_size([960.0, 640.0])
        .with_drag_and_drop(true)
        .with_icon(icon_data);

    if let Some(p) = saved_placement {
        let scale = primary_monitor_scale_factor().max(0.5);
        viewport = viewport
            .with_position([p.x as f32 / scale, p.y as f32 / scale])
            .with_inner_size([p.w as f32 / scale, p.h as f32 / scale]);
    } else {
        viewport = viewport.with_inner_size([1360.0, 860.0]);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Stoll Kalıp Editörü",
        options,
        Box::new(move |cc| Ok(Box::new(KalipEditorApp::new(cc, initial_path, saved_placement)))),
    )
}

/// Load the custom 64×64 Stoll Kalıp Editörü icon (`assets/app_icon_64.rgba`).
fn build_app_icon() -> egui::IconData {
    const APP_ICON_RGBA: &[u8] = include_bytes!("../assets/app_icon_64.rgba");
    let w = u16::from_le_bytes([APP_ICON_RGBA[0], APP_ICON_RGBA[1]]) as u32;
    let h = u16::from_le_bytes([APP_ICON_RGBA[2], APP_ICON_RGBA[3]]) as u32;
    egui::IconData {
        rgba: APP_ICON_RGBA[4..].to_vec(),
        width: w,
        height: h,
    }
}
