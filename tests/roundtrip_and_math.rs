use std::path::Path;

use stoll_kalip_editoru::catalog::{
    export_settings_bundle_with_template_path, import_settings_bundle_to_dir,
    knitting_structure_name_tr, ModuleCategory, StollCatalog, UserSettings, KNITTING_STRUCTURES,
};
use stoll_kalip_editoru::io::bitmap::{decode_muster_bitmap, render_muster_bitmap};
use stoll_kalip_editoru::io::parser::decode_form_utf16le;
use stoll_kalip_editoru::io::serializer::{encode_form_utf16le, serialize_form_text};
use stoll_kalip_editoru::io::{
    load_curve_file, load_file, load_from_bytes, parse_dxf_contour, save_curve_file, save_file,
    save_to_bytes, save_to_bytes_with_options, SaveOptions,
};
use stoll_kalip_editoru::math::{
    add_element, append_line, apply_curve_to_line, apply_directives_to_lines, apply_guidelines,
    apply_standard_preset_to_line, auto_detect_line_function_for_element, calculate_and_set_stufung,
    compute_range_summary, convert_input_format, copy_line, copy_markers, curve_steps_sum,
    delete_element, delete_line, element_distance_from_top, evaluate_document_geometry,
    generate_curve_steps, import_external_table_lines, insert_line_before, mm_to_rows,
    mm_to_stitches, multiply_marking_points, paste_insert_markers, paste_line,
    paste_replace_markers, recalc_document_for_density_change, recalc_line_from_mm,
    recalc_line_from_step_edit, recalc_line_from_steps, recalc_line_from_stitches_and_rows,
    resample_user_curve, rows_to_mm, set_element_distance_from_top, stitches_to_mm,
    swap_left_right, sync_mirrored_right_lines, update_or_append_closing_line, validate_document,
    CurveGenParams,
};
use stoll_kalip_editoru::model::{
    BindingOffProps, CurveData, CurveForm, ElementType, FormDocument, InputFormat, KanteChildSlot,
    LineFunction, LineSegment, LineSide, PointEntry, ShapeCutType, UndoStack, YReferenceLine,
};

fn make_sample_rect_doc() -> FormDocument {
    let mut doc = FormDocument::new_default_template();
    doc.header.name = "Sample_Rect".to_string();
    let hdr = doc.header.clone();
    let mut l1 = LineSegment {
        index: 1,
        tag_id: 1,
        stitches: -75,
        rows: 0,
        function: LineFunction::BaseLine,
        ..Default::default()
    };
    recalc_line_from_stitches_and_rows(&mut l1, &hdr);
    let mut l2 = LineSegment {
        index: 2,
        tag_id: 2,
        stitches: 0,
        rows: 180,
        function: LineFunction::None,
        ..Default::default()
    };
    recalc_line_from_stitches_and_rows(&mut l2, &hdr);
    let mut l3 = LineSegment {
        index: 3,
        tag_id: 3,
        stitches: 75,
        rows: 0,
        function: LineFunction::None,
        last_line: 1,
        ..Default::default()
    };
    recalc_line_from_stitches_and_rows(&mut l3, &hdr);
    doc.elements[0].left_lines = vec![l1, l2, l3];
    doc
}

fn assert_lossless_roundtrip_file(path: &str) {
    if !Path::new(path).exists() {
        eprintln!("Skipping missing test corpus file: {path}");
        return;
    }

    let orig_bytes = std::fs::read(path).expect("read original shp/shv");
    let orig_doc = load_from_bytes(&orig_bytes).expect("parse original shp/shv");

    let orig_form_bytes = {
        let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(&orig_bytes)).unwrap();
        let mut s = comp.open_stream("/Form").unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut s, &mut buf).unwrap();
        buf
    };
    let (orig_text, orig_bom, _) = decode_form_utf16le(&orig_form_bytes).unwrap();
    assert!(!orig_bom, "Stoll Form stream must not have BOM: {path}");

    let rt_text = serialize_form_text(&orig_doc);
    let rt_form_bytes = encode_form_utf16le(&rt_text, orig_doc.ole.form_had_bom);

    let orig_lines: Vec<&str> = orig_text.lines().collect();
    let rt_lines: Vec<&str> = rt_text.lines().collect();
    assert_eq!(
        orig_lines.len(),
        rt_lines.len(),
        "Line count mismatch in {path}: orig={} vs rt={}",
        orig_lines.len(),
        rt_lines.len()
    );
    for (idx, (a, b)) in orig_lines.iter().zip(rt_lines.iter()).enumerate() {
        assert_eq!(
            a,
            b,
            "Line {} mismatch in {path}:\n  orig: {:?}\n  rt:   {:?}",
            idx + 1,
            a,
            b
        );
    }
    assert_eq!(
        orig_form_bytes, rt_form_bytes,
        "UTF-16LE Form stream byte mismatch in {path}"
    );

    let saved_ole = save_to_bytes_with_options(
        &orig_doc,
        SaveOptions {
            regenerate_bitmap: false,
            bitmap_max_dim: 202,
        },
    )
    .expect("save OLE2 bytes");
    let reloaded = load_from_bytes(&saved_ole).expect("reload saved OLE2");
    assert_eq!(orig_doc.header.name, reloaded.header.name);
    assert_eq!(orig_doc.elements.len(), reloaded.elements.len());
    assert_eq!(orig_doc.ole.summary_info, reloaded.ole.summary_info);
    assert_eq!(
        serialize_form_text(&orig_doc),
        serialize_form_text(&reloaded),
        "Re-parsed OLE2 Form text differs for {path}"
    );
}

#[test]
fn test_real_stoll_corpus_lossless_roundtrip() {
    let corpus = [
        r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultform.shp",
        r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultform_user.shp",
        r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultformk&wbody.shp",
        r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultformk&wneck.shp",
        r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultformk&wsleeve.shp",
        r"D:\DESEN\SU-TEKSTIL\HW27-28\SUW27-06\SUW27-06-ON_ARKA.shp",
        r"D:\Stoll\M1plus\8.0.010\Form\1_raglan-back-38.shv",
        r"D:\Stoll\M1plus\8.0.010\Form\1_raglan-front-38.shv",
        r"D:\Stoll\M1plus\8.0.010\Form\1_raglan-sleeve-38.shv",
        r"D:\Stoll\M1plus\8.0.010\Form\2_set-in-front-v-neck-38.shv",
        r"D:\Stoll\M1plus\8.0.010\Form\3_set-in-l-round-front-r-neck-38.shv",
        r"D:\Stoll\M1plus\8.0.010\Form\5_top-front.shv",
        r"D:\Stoll\M1plus\8.0.010\Form\6_french-front-v.shv",
    ];
    for path in corpus {
        assert_lossless_roundtrip_file(path);
    }

    // Also sweep ALL .shv/.shp files directly inside D:\Stoll\M1plus\8.0.010\Form
    let form_dir = Path::new(r"D:\Stoll\M1plus\8.0.010\Form");
    if form_dir.exists() {
        let mut count = 0usize;
        if let Ok(entries) = std::fs::read_dir(form_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("shp") || ext.eq_ignore_ascii_case("shv") {
                        assert_lossless_roundtrip_file(&p.to_string_lossy());
                        count += 1;
                    }
                }
            }
        }
        assert!(count >= 10, "Expected >=10 real Stoll Form files tested, got {count}");
    }
}

#[test]
fn test_exact_disassembled_j_and_s_curves() {
    // Verified reference vector from SUW27-06-ON_ARKA.shp Kante 6:
    // Input: dx = 14, dy = 30, step_x = 2
    // Expected: (2,2)x1, (2,3)x2, (2,5)x3, (2,7)x1
    let j_steps = generate_curve_steps(&CurveGenParams {
        curve_form: CurveForm::JCurve,
        total_dx: 14,
        total_dy: 30,
        step_x: 2,
        step_y: 0,
        narrowing_width: 2,
        widening_width: 0,
        fade_out_width: 2,
    });
    let j_tuples: Vec<(i32, i32, i32)> = j_steps
        .iter()
        .map(|s| (s.step_width, s.step_height, s.factor))
        .collect();
    assert_eq!(j_tuples, vec![(2, 2, 1), (2, 3, 2), (2, 5, 3), (2, 7, 1)]);

    // Verified S-curve vector from MATH_AND_CURVES_SPEC.md Section 4.5:
    // Input: dx = 20, dy = 40, step_x = 2
    // Expected: (2,3)x1, (2,4)x1, (2,5)x2, (2,6)x1, (2,5)x2, (2,3)x2, (2,1)x1
    let s_steps = generate_curve_steps(&CurveGenParams {
        curve_form: CurveForm::SCurve,
        total_dx: 20,
        total_dy: 40,
        step_x: 2,
        step_y: 0,
        narrowing_width: 6,
        widening_width: 0,
        fade_out_width: 2,
    });
    let s_tuples: Vec<(i32, i32, i32)> = s_steps
        .iter()
        .map(|s| (s.step_width, s.step_height, s.factor))
        .collect();
    assert_eq!(
        s_tuples,
        vec![
            (2, 3, 1),
            (2, 4, 1),
            (2, 5, 2),
            (2, 6, 1),
            (2, 5, 2),
            (2, 3, 2),
            (2, 1, 1)
        ]
    );
}

#[test]
fn test_unit_conversions_and_calculate_and_set_stufung() {
    let mut doc = make_sample_rect_doc();
    doc.header.density_base_mm = 100.0;
    doc.header.density_x = 26.0;
    doc.header.density_y = 45.0;

    assert_eq!(mm_to_stitches(100.0, &doc.header), 26);
    assert_eq!(mm_to_rows(100.0, &doc.header), 45);
    assert!((stitches_to_mm(26, &doc.header) - 100.0).abs() < 1e-6);
    assert!((rows_to_mm(45, &doc.header) - 100.0).abs() < 1e-6);

    let line = &mut doc.elements[0].left_lines[1];
    line.step_x = 1;
    line.step_y = 4;
    line.step_factor = 15;
    line.rest_x = 0;
    line.rest_y = 2;
    recalc_line_from_steps(line, &doc.header);
    assert_eq!(line.stitches, 15);
    assert_eq!(line.rows, 62);
    assert!((line.width_mm - 57.69).abs() < 0.02);
    assert!((line.height_mm - 137.78).abs() < 0.02);

    // CalculateAndSetStufung exact cases
    line.stitches = 20;
    line.rows = 83;
    calculate_and_set_stufung(line, 2, 0);
    assert_eq!(line.step_factor, 10);
    assert_eq!(line.step_x, 2);
    assert_eq!(line.step_y, 8);
    assert_eq!(line.rest_x, 0);
    assert_eq!(line.rest_y, 3);

    line.width_mm = 100.0;
    line.height_mm = 200.0;
    recalc_line_from_mm(line, &doc.header);
    assert_eq!(line.stitches, 26);
    assert_eq!(line.rows, 90);

    doc.header.density_x = 30.0;
    recalc_document_for_density_change(&mut doc);
    convert_input_format(&mut doc, InputFormat::Stitches);
    assert_eq!(doc.header.input_format, InputFormat::Stitches);
    convert_input_format(&mut doc, InputFormat::Steps);
    assert_eq!(doc.header.input_format, InputFormat::Steps);
    convert_input_format(&mut doc, InputFormat::Mm);
    assert_eq!(doc.header.input_format, InputFormat::Mm);
}

#[test]
fn test_all_curve_forms_closure_and_resampling() {
    let cases = [
        (CurveForm::Line, 35, 112, 1),
        (CurveForm::Line, -28, 84, 2),
        (CurveForm::Step, 20, 60, 2),
        (CurveForm::JCurve, 40, 120, 2),
        (CurveForm::JCurve, -18, 52, 2),
        (CurveForm::SCurve, 32, 96, 2),
        (CurveForm::SCurve, -24, 68, 2),
    ];

    for (form, dx, dy, sx) in cases {
        let params = CurveGenParams {
            curve_form: form,
            total_dx: dx,
            total_dy: dy,
            step_x: sx,
            step_y: 0,
            narrowing_width: 6,
            widening_width: 0,
            fade_out_width: 2,
        };
        let steps = generate_curve_steps(&params);
        assert!(!steps.is_empty(), "Curve steps must not be empty for {form:?}");
        for s in &steps {
            assert!(s.factor >= 1, "Faktor must be >= 1");
            assert_eq!(s.narrowing_width, 6);
            assert_eq!(s.fade_out_width, 2);
        }
        let curve = CurveData {
            function: form.to_id(),
            steps,
            raw: Default::default(),
        };
        let (sum_x, sum_y) = curve_steps_sum(&curve);
        assert_eq!(sum_x, dx, "dx sum mismatch for {form:?}");
        assert_eq!(sum_y, dy, "dy sum mismatch for {form:?}");

        let resampled = resample_user_curve(&curve, dx + dx.signum() * 4, dy + 8);
        let r_curve = CurveData {
            function: CurveForm::UserCurve.to_id(),
            steps: resampled,
            raw: Default::default(),
        };
        let (rx, ry) = curve_steps_sum(&r_curve);
        assert_eq!(rx, dx + dx.signum() * 4);
        assert_eq!(ry, dy + 8);
    }

    let mut doc = make_sample_rect_doc();
    let steps = generate_curve_steps(&CurveGenParams {
        curve_form: CurveForm::JCurve,
        total_dx: 30,
        total_dy: 100,
        step_x: 2,
        ..Default::default()
    });
    let hdr = doc.header.clone();
    apply_curve_to_line(&mut doc.elements[0].left_lines[1], &hdr, CurveForm::JCurve, steps);
    doc.elements[0].left_lines[2].stitches = 45;
    recalc_line_from_stitches_and_rows(&mut doc.elements[0].left_lines[2], &hdr);
    sync_mirrored_right_lines(&mut doc, 0);

    let report = validate_document(&doc);
    assert!(
        !report.has_errors(),
        "Expected 0 validation errors, got: {:?}",
        report.issues
    );
}

#[test]
fn test_geometry_bitmap_catalog_and_operations() {
    // Blank new document starts with 0 lines (#48 item 5)
    let blank = FormDocument::new_default_template();
    assert!(blank.elements[0].left_lines.is_empty());
    assert_eq!(blank.elements[0].name, "");

    let mut doc = make_sample_rect_doc();
    let mut undo = UndoStack::default();

    let geom = evaluate_document_geometry(&doc);
    assert_eq!(geom.elements.len(), 1);
    assert_eq!(geom.elements[0].total_height_rows, 180);
    let span = compute_range_summary(&geom.elements[0], LineSide::Left, 0, 2).unwrap();
    assert_eq!(span.row_start, 1);
    assert_eq!(span.row_end, 3);
    assert_eq!(span.delta_st, (0, 180));

    let bmp = render_muster_bitmap(&doc, 202);
    let decoded = decode_muster_bitmap(&bmp).expect("valid rendered MusterBitmap");
    assert_eq!(decoded.height, 202);
    assert!(decoded.width >= 32);

    let bytes = save_to_bytes(&doc).expect("save new doc");
    let reloaded = load_from_bytes(&bytes).expect("reload new doc");
    assert_eq!(reloaded.header.name, doc.header.name);
    assert_eq!(reloaded.elements[0].left_lines.len(), 3);

    undo.push("Insert line", &doc);
    let pos = insert_line_before(&mut doc, 0, LineSide::Left, 1).unwrap();
    assert_eq!(pos, 1);
    assert_eq!(doc.elements[0].left_lines.len(), 4);
    assert!(undo.undo(&mut doc));
    assert_eq!(doc.elements[0].left_lines.len(), 3);
    assert!(undo.redo(&mut doc));
    assert_eq!(doc.elements[0].left_lines.len(), 4);

    let copied = copy_line(&doc, 0, LineSide::Left, 0).unwrap();
    assert!(paste_line(&mut doc, 0, LineSide::Left, 1, &copied));
    assert!(delete_line(&mut doc, 0, LineSide::Left, 1).is_some());
    assert!(append_line(&mut doc, 0, LineSide::Left).is_some());

    let neck_idx = add_element(&mut doc, ElementType::Neckline, None);
    assert_eq!(neck_idx, 1);
    assert_eq!(doc.elements.len(), 2);
    assert_eq!(doc.elements[neck_idx].name, "");
    assert!(delete_element(&mut doc, neck_idx).is_some());
    assert_eq!(doc.elements.len(), 1);

    multiply_marking_points(&mut doc.elements[0].left_lines[0], 0, 10, 5, 4, 2, 3);
    assert_eq!(doc.elements[0].left_lines[0].points[0].offsets.len(), 3);
    let _ = apply_guidelines(&mut doc, 0);

    let mut cat = StollCatalog::load_with_autodetect();
    assert!(!cat.stoll_presets.is_empty());
    assert_eq!(KNITTING_STRUCTURES.len(), 13);
    assert_eq!(knitting_structure_name_tr(0), "Strüktür tekplaka");
    assert_eq!(knitting_structure_name_tr(12), "Pelüş");
    apply_standard_preset_to_line(
        &mut doc.elements[0].left_lines[0],
        &cat.stoll_presets[0].template_line,
    );
    let mod_name = cat.module_name("{70037774-DA09-11d3-BE14-00A0C9D4B941}");
    assert!(!mod_name.is_empty() && mod_name != "(Modül yok)");
    let _ = cat.resolve_module("{70037774-DA09-11d3-BE14-00A0C9D4B941}");
    if cat.stoll_data_root.is_some() {
        let deleted_mod = cat
            .resolve_module("{A0EB0C74-54C4-4295-9B98-63BC5E75BECF}")
            .expect("resolve .M1_Module_Deleted fallback");
        assert!(!deleted_mod.name.is_empty());
        assert!(
            deleted_mod.thumbnail.is_some(),
            "expected real M3_UTL_Bitmap thumbnail from .M1_Module_Deleted"
        );
    }

    let real_path = r"D:\DESEN\SU-TEKSTIL\HW27-28\SUW27-06\SUW27-06-ON_ARKA.shp";
    if Path::new(real_path).exists() {
        let real_doc = load_file(real_path).unwrap();
        let mb = real_doc.ole.muster_bitmap.as_ref().expect("MusterBitmap present");
        let img = decode_muster_bitmap(mb).expect("decode real MusterBitmap");
        assert_eq!(img.width, 103);
        assert_eq!(img.height, 202);
    }
}

#[test]
fn test_phase2_engine_parity_eng01_to_eng13() {
    // [ENG-01] & [ENG-02] Stoll DB category indexing & cut-type preset reload
    let mut cat = StollCatalog::load_with_autodetect();
    if cat.stoll_data_root.is_some() {
        assert!(cat.modules_for_category(ModuleCategory::Narrowing).len() >= 25);
        assert!(cat.modules_for_category(ModuleCategory::NarrowingMultistage).len() == 6);
        assert!(cat.modules_for_category(ModuleCategory::Widening).len() >= 10);
        assert!(cat.modules_for_category(ModuleCategory::BindOff).len() >= 10);
        assert!(cat.modules_for_category(ModuleCategory::FadeOut).len() >= 5);
        assert!(cat.modules_for_category(ModuleCategory::Start).len() >= 5);
        assert!(cat.modules_for_category(ModuleCategory::End).len() >= 2);
        assert!(cat.modules_for_category(ModuleCategory::Point).len() >= 2);
        assert!(
            cat.modules_for_category_and_structure(ModuleCategory::Narrowing, 0)
                .len()
                >= 20
        );
        // Empty structure slot must NOT leak modules from other knitting structures
        assert!(
            cat.modules_for_category_and_structure(ModuleCategory::Widening, 2)
                .is_empty()
        );

        cat.reload_for_cut_type(ShapeCutType::KwBodyFront);
        assert_eq!(cat.active_cut_type, ShapeCutType::KwBodyFront);
        assert!(!cat.stoll_presets.is_empty());
        cat.reload_for_cut_type(ShapeCutType::Standard);
        assert_eq!(cat.active_cut_type, ShapeCutType::Standard);
    }

    // [ENG-03] Blank secondary element on add_element
    let mut doc = make_sample_rect_doc();
    let neck_idx = add_element(&mut doc, ElementType::Neckline, None);
    assert!(doc.elements[neck_idx].left_lines.is_empty());
    assert_eq!(doc.elements[neck_idx].name, "");
    assert_eq!(element_distance_from_top(&doc, neck_idx), (0.0, 0.0));

    // [ENG-07] Bitiş çizgisi positioning & Error 13655 overflow guard
    let _ = append_line(&mut doc, neck_idx, LineSide::Left);
    doc.elements[neck_idx].left_lines[0].rows = 40;
    doc.elements[neck_idx].left_lines[0].stitches = 20;
    assert!(set_element_distance_from_top(&mut doc, neck_idx, 10.0, false).is_ok());
    assert_eq!(doc.elements[neck_idx].offset_y_rows, 130.0); // 180 - 40 - 10
    assert!(set_element_distance_from_top(&mut doc, neck_idx, 200.0, false).is_err());

    // [ENG-04] & [ENG-05] Mirroring + SwapLeftRight (lines + points + curves)
    doc.elements[0].left_points.push(PointEntry {
        index: 1,
        ..PointEntry::default()
    });
    doc.elements[0].left_points[0].offsets[0].x = -12;
    sync_mirrored_right_lines(&mut doc, 0);
    assert_eq!(doc.elements[0].right_lines.len(), 3);
    assert_eq!(doc.elements[0].right_points.len(), 1);
    assert_eq!(doc.elements[0].right_points[0].offsets[0].x, 12);

    doc.elements[0].mirrored = false;
    assert!(swap_left_right(&mut doc, 0));
    assert_eq!(doc.elements[0].left_points[0].offsets[0].x, -12);

    // [ENG-06] StepX/StepY edit preserves N/R in İlmekler mode; Factor edit updates N/R
    let hdr = doc.header.clone();
    let line = &mut doc.elements[0].left_lines[1];
    line.stitches = 20;
    line.rows = 83;
    line.step_x = 2;
    line.step_y = 0;
    recalc_line_from_step_edit(line, &hdr, false, false, true);
    assert_eq!(line.stitches, 20);
    assert_eq!(line.rows, 83);
    assert_eq!(line.step_factor, 10);
    assert_eq!(line.rest_y, 3);
    line.step_factor = 12;
    recalc_line_from_step_edit(line, &hdr, true, false, true);
    assert_eq!(line.stitches, 24);
    assert_eq!(line.rows, 99);

    // [ENG-08] ApplyDirectives
    doc.elements[0].left_lines[1].function = LineFunction::Narrowing;
    let changed = apply_directives_to_lines(&mut doc, 0, LineSide::Left, &[1], 2, 1, 4);
    assert_eq!(changed, 1);
    assert_eq!(doc.elements[0].left_lines[1].step_x, 2);
    assert_eq!(doc.elements[0].left_lines[1].step_y, 4);

    // [ENG-09] BindOffType persistence round-trip
    doc.elements[0].left_lines[1].binding_off = Some(BindingOffProps {
        bind_off_type: Some(1),
        ..BindingOffProps::default()
    });
    let saved = save_to_bytes(&doc).unwrap();
    let reloaded = load_from_bytes(&saved).unwrap();
    assert_eq!(
        reloaded.elements[0].left_lines[1]
            .binding_off
            .as_ref()
            .and_then(|b| b.bind_off_type),
        Some(1)
    );

    // [ENG-10] .crv save & load round-trip
    let crv_steps = generate_curve_steps(&CurveGenParams {
        curve_form: CurveForm::JCurve,
        total_dx: 14,
        total_dy: 30,
        step_x: 2,
        ..Default::default()
    });
    let crv = CurveData {
        function: CurveForm::JCurve.to_id(),
        steps: crv_steps,
        raw: Default::default(),
    };
    let tmp_crv = std::env::temp_dir().join("kalipeditor_test_curve.crv");
    save_curve_file(&crv, &tmp_crv).unwrap();
    let loaded_crv = load_curve_file(&tmp_crv).unwrap();
    let _ = std::fs::remove_file(&tmp_crv);
    assert_eq!(loaded_crv.function, CurveForm::JCurve.to_id());
    assert_eq!(curve_steps_sum(&loaded_crv), (14, 30));

    // [ENG-11] External CSV/TSV table import
    let csv = "Boy;En;BoyBas;EnBas;Faktor;Yorum\n0;-60;0;-60;1;Taban\n100;20;4;1;20;Reglan\n";
    let imported = import_external_table_lines(&mut doc, neck_idx, LineSide::Left, csv, true);
    assert_eq!(imported, 2);
    assert_eq!(doc.elements[neck_idx].left_lines[0].stitches, -60);
    assert_eq!(doc.elements[neck_idx].left_lines[1].comment, "Reglan");

    // [ENG-12] Real marker clipboard
    let copied_pts = copy_markers(&doc, 0, LineSide::Left, &[0]);
    assert_eq!(copied_pts.len(), 1);
    assert_eq!(paste_insert_markers(&mut doc, 0, LineSide::Left, 0, &copied_pts), 1);
    assert_eq!(doc.elements[0].left_points.len(), 2);
    assert_eq!(paste_replace_markers(&mut doc, 0, LineSide::Left, 1, &copied_pts), 1);

    // [ENG-13] ASCII DXF contour import
    let dxf = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n10\n0.0\n20\n0.0\n10\n-200.0\n20\n0.0\n10\n-200.0\n20\n500.0\n10\n0.0\n20\n500.0\n0\nENDSEC\n0\nEOF\n";
    let dxf_doc = parse_dxf_contour(dxf, "TestDXF").unwrap();
    assert_eq!(dxf_doc.header.input_format, InputFormat::Mm);
    assert_eq!(dxf_doc.elements[0].left_lines.len(), 3);
}

#[test]
fn test_bucket1_regression_new_shp_validation_curves_and_neck() {
    // 1. Brand-new blank .shp creation -> Temel Row 1 -> lines + J-curve -> Neckline EndLine -> save -> reload -> validate
    let mut doc = FormDocument::new_default_template();
    assert_eq!(doc.elements[0].start_width, 500.0);

    // Row 1 defaults to BaseLine (Temel)
    let r0 = append_line(&mut doc, 0, LineSide::Left).unwrap();
    assert_eq!(r0, 0);
    assert_eq!(doc.elements[0].left_lines[0].function, LineFunction::BaseLine);

    let hdr = doc.header.clone();
    doc.elements[0].left_lines[0].stitches = -90;
    doc.elements[0].left_lines[0].rows = 0;
    recalc_line_from_stitches_and_rows(&mut doc.elements[0].left_lines[0], &hdr);
    auto_detect_line_function_for_element(
        &mut doc.elements[0].left_lines[0],
        true,
        true,
        ElementType::BasicShape,
    );
    assert_eq!(doc.elements[0].left_lines[0].function, LineFunction::BaseLine);

    // Row 2: straight side (140 rows)
    let r1 = append_line(&mut doc, 0, LineSide::Left).unwrap();
    doc.elements[0].left_lines[r1].stitches = 0;
    doc.elements[0].left_lines[r1].rows = 140;
    recalc_line_from_stitches_and_rows(&mut doc.elements[0].left_lines[r1], &hdr);

    // Row 3: J-curve armhole (+20 stitches, +40 rows) -> [Kurve] must be first in child_order!
    let r2 = append_line(&mut doc, 0, LineSide::Left).unwrap();
    let j_steps = generate_curve_steps(&CurveGenParams {
        curve_form: CurveForm::JCurve,
        total_dx: 20,
        total_dy: 40,
        step_x: 2,
        narrowing_width: 4,
        ..Default::default()
    });
    apply_curve_to_line(
        &mut doc.elements[0].left_lines[r2],
        &hdr,
        CurveForm::JCurve,
        j_steps,
    );
    auto_detect_line_function_for_element(
        &mut doc.elements[0].left_lines[r2],
        false,
        true,
        ElementType::BasicShape,
    );
    assert_eq!(doc.elements[0].left_lines[r2].function, LineFunction::Narrowing);
    assert!(matches!(
        doc.elements[0].left_lines[r2].child_order.first(),
        Some(KanteChildSlot::Curve)
    ));

    // Unclosed mirrored shape MUST trigger validation warning EVEN AFTER sync_mirrored_right_lines!
    sync_mirrored_right_lines(&mut doc, 0);
    let warn_report = validate_document(&doc);
    assert!(
        warn_report
            .warnings()
            .any(|i| i.message.contains("kapanmıyor")),
        "Expected closure warning on unclosed mirrored shape even with right_lines populated"
    );

    // Smart closing line: appends closing row (+70 stitches), then adjusts it if another line changes
    let r3 = update_or_append_closing_line(&mut doc, 0, LineSide::Left).unwrap();
    assert_eq!(r3, 3);
    assert_eq!(doc.elements[0].left_lines[r3].stitches, 70);
    // Modify row 2 stitches and call update_or_append_closing_line again -> adjusts row 4 in-place!
    doc.elements[0].left_lines[r1].stitches = -10;
    recalc_line_from_stitches_and_rows(&mut doc.elements[0].left_lines[r1], &hdr);
    let r3_adj = update_or_append_closing_line(&mut doc, 0, LineSide::Left).unwrap();
    assert_eq!(r3_adj, 3);
    assert_eq!(doc.elements[0].left_lines.len(), 4);
    assert_eq!(doc.elements[0].left_lines[3].stitches, 80);

    // Secondary Neckline element anchored at EndLine (Bitiş çizgisi) keeps dist_from_top == 0 when lines are added!
    let neck_idx = add_element(&mut doc, ElementType::Neckline, None);
    assert_eq!(doc.elements[neck_idx].y_ref_line, YReferenceLine::EndLine);
    assert_eq!(doc.elements[neck_idx].offset_y_rows, 180.0);
    assert_eq!(element_distance_from_top(&doc, neck_idx), (0.0, 0.0));

    let n0 = append_line(&mut doc, neck_idx, LineSide::Left).unwrap();
    doc.elements[neck_idx].left_lines[n0].stitches = -20;
    doc.elements[neck_idx].left_lines[n0].rows = 30;
    recalc_line_from_stitches_and_rows(&mut doc.elements[neck_idx].left_lines[n0], &hdr);
    // Simulate re-anchoring after height edit & check EndLine on insert/delete
    stoll_kalip_editoru::math::sync_endline_element_y(&mut doc, neck_idx, 0.0);
    assert_eq!(doc.elements[neck_idx].offset_y_rows, 150.0);
    assert_eq!(element_distance_from_top(&doc, neck_idx).0, 0.0);

    // Cutout auto-detection: -20 on left side of Neckline is Narrowing (Reglan), NOT Widening!
    auto_detect_line_function_for_element(
        &mut doc.elements[neck_idx].left_lines[n0],
        true,
        true,
        ElementType::Neckline,
    );
    assert_eq!(
        doc.elements[neck_idx].left_lines[n0].function,
        LineFunction::Narrowing
    );
    let n1 = update_or_append_closing_line(&mut doc, neck_idx, LineSide::Left).unwrap();
    assert_eq!(n1, 1);
    sync_mirrored_right_lines(&mut doc, neck_idx);

    // Save to disk with empty header.name -> populates Name from file stem & passes validation cleanly
    let tmp_shp = std::env::temp_dir().join("yeni_kalip_e2e_test.shp");
    save_file(&doc, &tmp_shp).unwrap();
    let reloaded = load_file(&tmp_shp).unwrap();
    let _ = std::fs::remove_file(&tmp_shp);
    assert_eq!(reloaded.header.name, "yeni_kalip_e2e_test");
    let final_report = validate_document(&reloaded);
    assert!(
        final_report.issues.is_empty(),
        "Expected 0 issues on valid brand-new saved .shp, got: {:?}",
        final_report.issues
    );

    // 2. CurveForm::Step preserves step_x_mag > 1 on resize and clears empty [Kurve] on (0,0)
    let mut step_line = LineSegment {
        stitches: 20,
        rows: 40,
        ..Default::default()
    };
    let step_curve = generate_curve_steps(&CurveGenParams {
        curve_form: CurveForm::Step,
        total_dx: 20,
        total_dy: 40,
        step_x: 2,
        ..Default::default()
    });
    assert_eq!(step_curve[0].step_width, 0); // CalcFormula_LineOben leading vertical step
    apply_curve_to_line(&mut step_line, &hdr, CurveForm::Step, step_curve);
    step_line.stitches = 24;
    step_line.rows = 48;
    recalc_line_from_stitches_and_rows(&mut step_line, &hdr);
    let first_nonzero_sx = step_line
        .curve
        .as_ref()
        .unwrap()
        .steps
        .iter()
        .find(|s| s.step_width != 0)
        .map(|s| s.step_width.abs())
        .unwrap();
    assert_eq!(first_nonzero_sx, 2);
    step_line.stitches = 0;
    step_line.rows = 0;
    recalc_line_from_stitches_and_rows(&mut step_line, &hdr);
    assert!(step_line.curve.is_none());

    // 3. validate_document catches sign-inverted curve & y_off==0 overflow
    let mut bad_doc = doc.clone();
    bad_doc.elements[0].left_lines[r2].stitches = -20; // opposite sign of +20 curve sum!
    bad_doc.elements[neck_idx].offset_y_rows = 0.0;
    bad_doc.elements[neck_idx].left_lines[n0].rows = 250; // 0 + 250 > 180!
    let bad_rep = validate_document(&bad_doc);
    assert!(bad_rep.errors().any(|e| e.message.contains("Kavis basamak toplamı")));
    assert!(bad_rep.errors().any(|e| e.message.contains("0x3557")));

    // 4. Clearing Kommentar omits it from serialized Form text instead of leaving blank `Kommentar = `
    bad_doc.header.raw.push_raw("Kommentar".to_string(), "Eski yorum".to_string());
    bad_doc.header.comment.clear();
    let ser = serialize_form_text(&bad_doc);
    assert!(!ser.contains("Kommentar ="));

    // 5. Odd AbstandHaelften = 1 preserves 0.5-stitch mm offset per side
    let mut gap_doc = FormDocument::new_default_template();
    gap_doc.elements[0].distance_halves = 1;
    let _ = append_line(&mut gap_doc, 0, LineSide::Left);
    gap_doc.elements[0].left_lines[0].stitches = -10;
    let g = evaluate_document_geometry(&gap_doc);
    assert!(g.elements[0].left_segments[0].start_mm.x < -1.0);
}

#[test]
fn test_5_fixes_axis_geometry_row1_closing_none_and_settings_bundle() {
    use stoll_kalip_editoru::math::auto_detect_line_function_in_context;
    use stoll_kalip_editoru::model::NarrowingProps;

    // 1. SUW26-168_ARKA-38.shp Element 2 geometry (-126 -> -76 -> -126 left, +126 -> +76 -> +126 right) + lossless roundtrip
    let suw_path = r"D:\DESEN\SU-TEKSTIL\2027 YAZLIK KATALOG\İMALAT\SUW26-168\SUW26-168_ARKA-38.shp";
    if Path::new(suw_path).exists() {
        assert_lossless_roundtrip_file(suw_path);
        let suw_doc = load_file(suw_path).unwrap();
        let geom = evaluate_document_geometry(&suw_doc);
        let el2 = &geom.elements[1];
        assert_eq!(el2.element_index, 1);
        assert!(!el2.mirrored);
        assert_eq!(el2.left_segments.len(), 4);
        assert_eq!(el2.right_segments.len(), 4);

        // Left side starts at base(0) - offset_x(126) = -126, moves +50 to -76, returns -50 to -126
        assert_eq!(el2.left_segments[0].start_st.0, -126);
        assert_eq!(el2.left_segments[1].end_st.0, -76);
        assert_eq!(el2.left_segments[3].end_st.0, -126);

        // Right side starts at base(0) + offset_x(126) = +126, moves -50 to +76, returns +50 to +126
        assert_eq!(el2.right_segments[0].start_st.0, 126);
        assert_eq!(el2.right_segments[1].end_st.0, 76);
        assert_eq!(el2.right_segments[3].end_st.0, 126);
    }

    // Also test synthetic Orta eksen + Ana kesimin orta ekseni on both Element 1 & secondary Element 2
    let mut axis_doc = FormDocument::new_default_template();
    axis_doc.elements[0].offset_x_stitches = 10.0;
    axis_doc.elements[0].offset_x_mm = stitches_to_mm(10, &axis_doc.header);
    let _ = append_line(&mut axis_doc, 0, LineSide::Left);
    // 2. Row 1 (Temel) defaults to -80 on Left side
    assert_eq!(axis_doc.elements[0].left_lines[0].function, LineFunction::BaseLine);
    assert_eq!(axis_doc.elements[0].left_lines[0].stitches, -80);
    sync_mirrored_right_lines(&mut axis_doc, 0);
    assert_eq!(axis_doc.elements[0].right_lines[0].stitches, 80);

    let axis_geom = evaluate_document_geometry(&axis_doc);
    assert_eq!(axis_geom.elements[0].left_segments[0].start_st.0, -10);
    assert_eq!(axis_geom.elements[0].left_segments[0].end_st.0, -90);
    assert_eq!(axis_geom.elements[0].right_segments[0].start_st.0, 10);
    assert_eq!(axis_geom.elements[0].right_segments[0].end_st.0, 90);

    let sec = add_element(&mut axis_doc, ElementType::Gore, Some("Tiktak".into()));
    axis_doc.elements[sec].mirrored = false;
    axis_doc.elements[sec].offset_x_stitches = 126.0;
    axis_doc.elements[sec].offset_x_stitches_base = 20.0;
    let l0 = append_line(&mut axis_doc, sec, LineSide::Left).unwrap();
    axis_doc.elements[sec].left_lines[l0].stitches = 50;
    axis_doc.elements[sec].left_lines[l0].rows = 24;
    let r0 = append_line(&mut axis_doc, sec, LineSide::Right).unwrap();
    axis_doc.elements[sec].right_lines[r0].stitches = -50;
    axis_doc.elements[sec].right_lines[r0].rows = 24;
    let sec_geom = evaluate_document_geometry(&axis_doc);
    // Left starts at 20 - 126 = -106; Right starts at 20 + 126 = +146
    assert_eq!(sec_geom.elements[sec].left_segments[0].start_st.0, -106);
    assert_eq!(sec_geom.elements[sec].left_segments[0].end_st.0, -56);
    assert_eq!(sec_geom.elements[sec].right_segments[0].start_st.0, 146);
    assert_eq!(sec_geom.elements[sec].right_segments[0].end_st.0, 96);

    // 3. Closing rows MUST have LineFunction::None and user_set_function == false
    let mut close_doc = FormDocument::new_default_template();
    let _ = append_line(&mut close_doc, 0, LineSide::Left);
    assert_eq!(close_doc.elements[0].left_lines[0].stitches, -80);
    let side_row = append_line(&mut close_doc, 0, LineSide::Left).unwrap();
    close_doc.elements[0].left_lines[side_row].rows = 120;
    close_doc.elements[0].left_lines[side_row].stitches = 0;
    recalc_line_from_stitches_and_rows(&mut close_doc.elements[0].left_lines[side_row], &close_doc.header);

    let close_idx = update_or_append_closing_line(&mut close_doc, 0, LineSide::Left).unwrap();
    assert_eq!(close_idx, 2);
    assert_eq!(close_doc.elements[0].left_lines[close_idx].stitches, 80);
    assert_eq!(close_doc.elements[0].left_lines[close_idx].rows, 0);
    assert_eq!(close_doc.elements[0].left_lines[close_idx].function, LineFunction::None);
    assert!(!close_doc.elements[0].left_lines[close_idx].user_set_function);

    // Editing closing row via auto_detect must keep LineFunction::None
    close_doc.elements[0].left_lines[close_idx].stitches = 75;
    auto_detect_line_function_for_element(
        &mut close_doc.elements[0].left_lines[close_idx],
        false,
        true,
        ElementType::BasicShape,
    );
    assert_eq!(close_doc.elements[0].left_lines[close_idx].function, LineFunction::None);

    let mut plain_closing = LineSegment {
        stitches: 80,
        rows: 0,
        last_line: 0,
        ..Default::default()
    };
    auto_detect_line_function_in_context(
        &mut plain_closing,
        false,
        true,
        true,
        ElementType::BasicShape,
    );
    assert_eq!(plain_closing.function, LineFunction::None);

    // 4. UserSettings + custom template save/delete + export/import bundle in temp directory
    let tmp_root = std::env::temp_dir().join(format!("kalipeditor_settings_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_root);
    std::fs::create_dir_all(&tmp_root).unwrap();

    let src_dir = tmp_root.join("src");
    let dst_dir = tmp_root.join("dst");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(&dst_dir).unwrap();

    let settings = UserSettings {
        default_density_x_per_cm: 3.25,
        default_density_y_per_cm: 4.5,
        default_template_is_user: true,
        default_template_index: 0,
        auto_apply_default_template: true,
        snap_even_rows: true,
        show_dimensions: false,
        show_detail_columns: true,
        custom_stoll_defaultform_path: Some(Path::new(r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultform.shp").to_path_buf()),
    };
    let ini_path = src_dir.join("settings.ini");
    settings.save_to_path(&ini_path).unwrap();
    let loaded_settings = UserSettings::load_from_path(&ini_path);
    assert_eq!(loaded_settings, settings);

    let mut cat = StollCatalog::load_from_root(None, ShapeCutType::Standard);
    let mut sample_tpl = LineSegment::default();
    sample_tpl.narrowing = Some(NarrowingProps {
        stitch_count: 2,
        ..Default::default()
    });
    sample_tpl.ml_np_front = 115;
    let user_shp = src_dir.join("defaultform_user.shp");
    let idx0 = cat
        .save_line_as_user_preset_to(&user_shp, "Benim Ozel Sablon", &sample_tpl)
        .unwrap();
    assert_eq!(idx0, 0);
    assert_eq!(cat.user_presets.len(), 1);
    assert!(cat.user_presets[0].display_name.contains("Benim Ozel Sablon"));
    assert_eq!(cat.user_presets[0].template_line.ml_np_front, 115);

    let bundle_path = tmp_root.join("yedek.kalipcfg");
    export_settings_bundle_with_template_path(&settings, &user_shp, &bundle_path).unwrap();
    let (imported_settings, templates_imported) =
        import_settings_bundle_to_dir(&bundle_path, &dst_dir).unwrap();
    assert!(templates_imported);
    assert_eq!(imported_settings, settings);

    let mut cat2 = StollCatalog::load_from_root(None, ShapeCutType::Standard);
    cat2.reload_user_presets_from(&dst_dir.join("defaultform_user.shp"));
    assert_eq!(cat2.user_presets.len(), 1);
    assert_eq!(cat2.user_presets[0].template_line.ml_np_front, 115);

    cat2.delete_user_preset_from(&dst_dir.join("defaultform_user.shp"), 0)
        .unwrap();
    assert!(cat2.user_presets.is_empty());

    let _ = std::fs::remove_dir_all(&tmp_root);
}

#[cfg(target_os = "windows")]
fn assert_windows_stg_open_storage_ok(path: &Path) {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "ole32")]
    extern "system" {
        fn StgOpenStorage(
            pwcs_name: *const u16,
            pstg_priority: *mut c_void,
            grf_mode: u32,
            snb_exclude: *mut c_void,
            reserved: u32,
            ppstg_open: *mut *mut c_void,
        ) -> i32;
    }

    const STGM_READ_SHARE_DENY_WRITE: u32 = 0x0000_0020;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut stg: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        StgOpenStorage(
            wide.as_ptr(),
            std::ptr::null_mut(),
            STGM_READ_SHARE_DENY_WRITE,
            std::ptr::null_mut(),
            0,
            &mut stg,
        )
    };
    assert_eq!(
        hr,
        0,
        "Windows StgOpenStorage failed on {} with HRESULT {:#010x}",
        path.display(),
        hr as u32
    );
    if !stg.is_null() {
        unsafe {
            let vtbl = *(stg as *const *const *const c_void);
            let release: extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*vtbl.add(2));
            release(stg);
        }
    }
}

fn assert_ole2_v3_valid(bytes: &[u8], expect_summary: bool, label: &str) {
    assert!(bytes.len() >= 512, "{label}: CFB smaller than 512B");
    assert_eq!(
        &bytes[0..8],
        &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
        "{label}: bad OLE2 signature"
    );
    let major_ver = u16::from_le_bytes([bytes[26], bytes[27]]);
    let sector_shift = u16::from_le_bytes([bytes[30], bytes[31]]);
    let num_dir_sectors = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
    assert_eq!(major_ver, 3, "{label}: must be CFB v3");
    assert_eq!(sector_shift, 9, "{label}: must use 512-byte sectors");
    assert_eq!(
        num_dir_sectors, 0,
        "{label}: MS-CFB v3 requires num_dir_sectors == 0"
    );

    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(bytes)).unwrap();
    assert!(comp.exists("/Form"), "{label}: missing /Form");
    assert!(comp.exists("/MusterBitmap"), "{label}: missing /MusterBitmap");
    if expect_summary {
        assert!(
            comp.exists("/\x05SummaryInformation"),
            "{label}: missing /\\x05SummaryInformation"
        );
    }
    let mut bmp_bytes = Vec::new();
    std::io::Read::read_to_end(&mut comp.open_stream("/MusterBitmap").unwrap(), &mut bmp_bytes)
        .unwrap();
    assert!(
        decode_muster_bitmap(&bmp_bytes).is_some(),
        "{label}: MusterBitmap failed to decode"
    );
}

#[test]
fn test_ole2_v3_stoll_compatibility() {
    let tmp_dir = std::env::temp_dir().join(format!("kalipeditor_ole2_v3_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    // 1. Small newly-created document (Form < 4096B -> uses MiniStream/MiniFAT, no SummaryInfo)
    let sample_doc = make_sample_rect_doc();
    let sample_bytes = save_to_bytes(&sample_doc).unwrap();
    assert_ole2_v3_valid(&sample_bytes, false, "sample_rect");
    let sample_path = tmp_dir.join("sample_rect.shp");
    save_file(&sample_doc, &sample_path).unwrap();
    #[cfg(target_os = "windows")]
    assert_windows_stg_open_storage_ok(&sample_path);
    let sample_reloaded = load_file(&sample_path).unwrap();
    assert_eq!(
        serialize_form_text(&sample_doc),
        serialize_form_text(&sample_reloaded)
    );

    // 2. Default blank template
    let blank_doc = FormDocument::new_default_template();
    let blank_path = tmp_dir.join("blank.shp");
    save_file(&blank_doc, &blank_path).unwrap();
    assert_ole2_v3_valid(&std::fs::read(&blank_path).unwrap(), false, "blank_template");
    #[cfg(target_os = "windows")]
    assert_windows_stg_open_storage_ok(&blank_path);

    // 3. Real Stoll corpus file (including user-reported SUW26-168_ARKA-40-orjinal.shp if present)
    for real_path in [
        r"C:\Users\gokha\Desktop\TEST\SUW26-168_ARKA-40-orjinal.shp",
        r"D:\Stoll\M1plus\8.0.010\ConfigData\defaultform.shp",
    ] {
        if Path::new(real_path).exists() {
            let doc = load_file(real_path).unwrap();
            let has_summary = doc.ole.summary_info.is_some();
            let out_bytes = save_to_bytes(&doc).unwrap();
            assert_ole2_v3_valid(&out_bytes, has_summary, real_path);
            let out_path = tmp_dir.join(
                Path::new(real_path)
                    .file_name()
                    .unwrap_or_default(),
            );
            save_file(&doc, &out_path).unwrap();
            #[cfg(target_os = "windows")]
            assert_windows_stg_open_storage_ok(&out_path);
            let reloaded = load_file(&out_path).unwrap();
            assert_eq!(
                serialize_form_text(&doc),
                serialize_form_text(&reloaded),
                "Form roundtrip mismatch after OLE2 save/load for {real_path}"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_audit_bug_fixes_elemanlar_math_and_ops() {
    // BUG-01 & BUG-02: element_distance_from_top & offset_y_mm preservation in Stitches vs Mm mode
    let mut doc = make_sample_rect_doc(); // InputFormat::Stitches, H_base = 180 rows, density_y = 200 -> 0.5 mm/row
    let neck_idx = add_element(&mut doc, ElementType::Neckline, None);
    assert_eq!(doc.elements[neck_idx].offset_y_mm, 0.0); // BUG-02: stays 0.0 in Stitches mode
    let _ = append_line(&mut doc, neck_idx, LineSide::Left);
    doc.elements[neck_idx].left_lines[0].rows = 30;
    doc.elements[neck_idx].left_lines[0].stitches = -20;
    // Simulate real Stoll .shp: offset_y_rows = 120, offset_y_mm = 0.0
    doc.elements[neck_idx].offset_y_rows = 120.0;
    doc.elements[neck_idx].offset_y_mm = 0.0;
    let (d_rows, d_mm) = element_distance_from_top(&doc, neck_idx);
    assert_eq!(d_rows, 30.0); // 180 - (120 + 30)
    assert_eq!(d_mm, stoll_kalip_editoru::math::round2(rows_to_mm(30, &doc.header)));

    assert!(set_element_distance_from_top(&mut doc, neck_idx, 10.0, false).is_ok());
    assert_eq!(doc.elements[neck_idx].offset_y_rows, 140.0);
    assert_eq!(doc.elements[neck_idx].offset_y_mm, 0.0); // BUG-02: untouched in Stitches mode

    assert!(stoll_kalip_editoru::math::set_element_distance_from_bottom(&mut doc, neck_idx, 130.0, false).is_ok());
    assert_eq!(doc.elements[neck_idx].offset_y_rows, 130.0);
    assert_eq!(doc.elements[neck_idx].offset_y_mm, 0.0);

    // BUG-01 in Mm mode: uses explicit height_mm sum
    let mut mm_doc = make_sample_rect_doc();
    mm_doc.header.input_format = InputFormat::Mm;
    mm_doc.elements[0].left_lines[1].height_mm = 100.0; // explicit mm differs from rows_to_mm(180)=90
    let mm_neck = add_element(&mut mm_doc, ElementType::Neckline, None);
    let _ = append_line(&mut mm_doc, mm_neck, LineSide::Left);
    mm_doc.elements[mm_neck].left_lines[0].rows = 20;
    mm_doc.elements[mm_neck].left_lines[0].height_mm = 12.5;
    assert!(set_element_distance_from_top(&mut mm_doc, mm_neck, 7.5, true).is_ok());
    assert!((mm_doc.elements[mm_neck].offset_y_mm - 80.0).abs() < 1e-6); // 100 - 12.5 - 7.5
    let (_, mm_top) = element_distance_from_top(&mm_doc, mm_neck);
    assert!((mm_top - 7.5).abs() < 1e-6);

    // BUG-03: is_base_line_row & apply_directives_to_lines on secondary element Row 0
    assert!(doc.elements[0].is_base_line_row(LineSide::Left, 0));
    assert!(!doc.elements[0].is_base_line_row(LineSide::Left, 1));
    assert!(!doc.elements[neck_idx].is_base_line_row(LineSide::Left, 0));
    doc.elements[neck_idx].left_lines[0].function = LineFunction::Narrowing;
    doc.elements[neck_idx].left_lines[0].curve = Some(CurveData {
        function: 1,
        steps: stoll_kalip_editoru::math::generate_curve_steps(&CurveGenParams {
            curve_form: CurveForm::Line,
            total_dx: -20,
            total_dy: 30,
            step_x: -1,
            step_y: 0,
            narrowing_width: 1,
            widening_width: 0,
            fade_out_width: 0,
        }),
        ..Default::default()
    });
    let updated_dirs = stoll_kalip_editoru::math::apply_directives_to_lines(
        &mut doc,
        neck_idx,
        LineSide::Left,
        &[],
        2,
        1,
        1,
    );
    assert_eq!(updated_dirs, 1);
    assert!(doc.elements[neck_idx].left_lines[0]
        .curve
        .as_ref()
        .unwrap()
        .steps
        .iter()
        .all(|s| s.step_width == -2 || s.step_width == 0));

    // BUG-05: delete_element refuses Element 0
    assert!(delete_element(&mut doc, 0).is_none());
    assert!(doc.elements.len() >= 2);

    // BUG-07: negative y_bottom rejected even when Element 1 height == 0; apply_guidelines keeps EndLine anchored
    let mut empty_base = FormDocument::new_default_template();
    let sec = add_element(&mut empty_base, ElementType::Hole, None);
    assert!(stoll_kalip_editoru::math::set_element_distance_from_bottom(&mut empty_base, sec, -5.0, false).is_err());

    let mut guide_doc = make_sample_rect_doc();
    guide_doc.elements[0].left_lines[1].rows = 179; // odd rows
    let g_neck = add_element(&mut guide_doc, ElementType::Neckline, None);
    let _ = append_line(&mut guide_doc, g_neck, LineSide::Left);
    guide_doc.elements[g_neck].left_lines[0].rows = 20;
    assert!(set_element_distance_from_top(&mut guide_doc, g_neck, 5.0, false).is_ok());
    guide_doc.guidelines.prefer_even_rows = true;
    guide_doc.guidelines.apply_to_all_elements = true;
    let mod_cnt = stoll_kalip_editoru::math::apply_guidelines(&mut guide_doc, 0);
    assert!(mod_cnt >= 1);
    assert_eq!(element_distance_from_top(&guide_doc, g_neck).0, 5.0);

    // BUG-08: recalc_line_from_step_edit_for_element respects cutout polarity and closing context
    let mut cutout_line = LineSegment {
        step_x: -2,
        step_y: 4,
        step_factor: 5,
        ..LineSegment::default()
    };
    stoll_kalip_editoru::math::recalc_line_from_step_edit_for_element(
        &mut cutout_line,
        &doc.header,
        true,
        true,
        false,
        true,
        ElementType::Neckline,
    );
    assert_eq!(cutout_line.stitches, -10);
    assert_eq!(cutout_line.rows, 20);
    assert_eq!(cutout_line.function, LineFunction::Narrowing);

    let mut closing_line = LineSegment {
        step_x: 20,
        step_y: 0,
        step_factor: 1,
        last_line: 1,
        ..LineSegment::default()
    };
    stoll_kalip_editoru::math::recalc_line_from_step_edit_for_element(
        &mut closing_line,
        &doc.header,
        true,
        false,
        true,
        true,
        ElementType::Neckline,
    );
    assert_eq!(closing_line.function, LineFunction::None);

    // Real corpus check if present
    let real_suw = r"C:\Users\gokha\Desktop\TEST\SUW26-168_ARKA-40-orjinal.shp";
    if Path::new(real_suw).exists() {
        let suw = load_file(real_suw).unwrap();
        if suw.elements.len() > 1 {
            let (dr, dm) = element_distance_from_top(&suw, 1);
            let expected_mm = stoll_kalip_editoru::math::round2(rows_to_mm(dr.round() as i32, &suw.header));
            assert!((dm - expected_mm).abs() < 1e-6);
            assert!(!suw.elements[1].is_base_line_row(LineSide::Left, 0));
        }
    }
}
