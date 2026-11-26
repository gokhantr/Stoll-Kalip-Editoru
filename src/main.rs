#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;

use std::path::PathBuf;

use crate::app::KalipEditorApp;

fn main() -> eframe::Result<()> {
    let initial_path: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    let icon_data = build_app_icon();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Yeni - Stoll Kalıp Editörü")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([960.0, 640.0])
        .with_drag_and_drop(true);
    viewport = viewport.with_icon(icon_data);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Stoll Kalıp Editörü",
        options,
        Box::new(move |cc| Ok(Box::new(KalipEditorApp::new(cc, initial_path)))),
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
