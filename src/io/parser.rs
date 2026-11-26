use std::sync::Arc;

use crate::model::{
    enums::{parse_stoll_bool, DisplayUnit, InputFormat, LineFunction, YReferenceLine},
    BindingOffProps, ConnectPoint, ConnectionPoints, CurveData, CurveStep, ExternalLocalModule,
    FadeOutProps, FormDocument, FormHeader, KanteChildSlot, LineSegment, ModuleAssignment,
    NarrowingProps, OlePreservedStreams, PointEntry, PointOffset, RawSectionMeta, ShapeElement,
    WideningProps,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveSection {
    None,
    Form,
    Element,
    Kante,
    FadeOut,
    Narrowing,
    Widening,
    BindingOff,
    Point(PointOwner),
    PointOffset(PointOwner),
    ModuleArt(ModuleOwner),
    Curve,
    CurveStep,
    ConnectionPoints,
    ConnectPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointOwner {
    Kante,
    ElementLeft,
    ElementRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleOwner {
    FadeOut,
    Narrowing,
    Widening,
    BindingOff,
    Point(PointOwner),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveSide {
    Left,
    Right,
    PointsLeft,
    PointsRight,
}

/// Decode raw UTF-16LE bytes (with or without BOM) into a Rust `String`, returning `(text, had_bom, trailing_newline)`.
pub fn decode_form_utf16le(bytes: &[u8]) -> Result<(String, bool, bool), String> {
    if bytes.len() % 2 != 0 {
        return Err(format!("Form stream has odd byte length: {}", bytes.len()));
    }
    let (had_bom, slice) = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        (true, &bytes[2..])
    } else {
        (false, bytes)
    };
    let mut u16s = Vec::with_capacity(slice.len() / 2);
    for chunk in slice.chunks_exact(2) {
        u16s.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    let text = String::from_utf16(&u16s).map_err(|e| format!("Invalid UTF-16LE in Form stream: {e}"))?;
    let trailing_newline = text.ends_with("\r\n") || text.ends_with('\n');
    Ok((text, had_bom, trailing_newline))
}

/// Parse the decoded INI-like text of a Stoll `Form` stream into a `FormDocument`.
pub fn parse_form_text(text: &str, ole: OlePreservedStreams) -> Result<FormDocument, String> {
    let mut header = FormHeader {
        raw: RawSectionMeta::new(),
        is_from_gks: None,
        gks_xml_file_schema: None,
        last_line_tag_id: None,
        last_free_line_tag_id: None,
        last_point_tag_id: None,
        ..FormHeader::default()
    };
    header.name.clear();
    header.tag_id.clear();
    header.version.clear();

    let mut elements: Vec<ShapeElement> = Vec::new();
    let mut active = ActiveSection::None;
    let mut active_side = ActiveSide::Left;
    let mut pending_ext_guid: Option<String> = None;

    for raw_line in text.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') {
            let (sec_name, sec_val) = parse_section_header(trimmed);
            match sec_name {
                "[Form]" => {
                    header.form_id = sec_val.and_then(|s| s.parse().ok()).unwrap_or(0);
                    header.raw.original_header = Some(trimmed.to_string());
                    active = ActiveSection::Form;
                }
                "[Element]" => {
                    let idx = sec_val.and_then(|s| s.parse().ok()).unwrap_or((elements.len() + 1) as i32);
                    let mut el = ShapeElement {
                        index: idx,
                        name: String::new(),
                        guid_id: String::new(),
                        z_order: None,
                        has_left_lines_section: false,
                        has_right_lines_section: false,
                        raw: RawSectionMeta {
                            original_header: Some(trimmed.to_string()),
                            raw_entries: Vec::new(),
                        },
                        ..ShapeElement::default()
                    };
                    el.mirrored = false;
                    elements.push(el);
                    active_side = ActiveSide::Left;
                    active = ActiveSection::Element;
                }
                "[LinienLinks]" => {
                    if let Some(el) = elements.last_mut() {
                        el.has_left_lines_section = true;
                    }
                    active_side = ActiveSide::Left;
                    active = ActiveSection::Element;
                }
                "[LinienRechts]" => {
                    if let Some(el) = elements.last_mut() {
                        el.has_right_lines_section = true;
                    }
                    active_side = ActiveSide::Right;
                    active = ActiveSection::Element;
                }
                "[PunkteLinks]" => {
                    if let Some(el) = elements.last_mut() {
                        el.has_left_points_section = true;
                    }
                    active_side = ActiveSide::PointsLeft;
                    active = ActiveSection::Element;
                }
                "[PunkteRechts]" => {
                    if let Some(el) = elements.last_mut() {
                        el.has_right_points_section = true;
                    }
                    active_side = ActiveSide::PointsRight;
                    active = ActiveSection::Element;
                }
                "[Kante]" => {
                    if let Some(el) = elements.last_mut() {
                        let list = match active_side {
                            ActiveSide::Right => &mut el.right_lines,
                            _ => &mut el.left_lines,
                        };
                        let idx = sec_val.and_then(|s| s.parse().ok()).unwrap_or((list.len() + 1) as i32);
                        let line = LineSegment {
                            index: idx,
                            guid_id: String::new(),
                            param_guid_id: String::new(),
                            tag_id: 0,
                            is_hand_knitting_suppress_first_y: None,
                            with_bind_off: None,
                            gore_knitting: None,
                            gore_rows_additional: None,
                            gore_rows_in_gore: None,
                            gore_align_rows: None,
                            is_imitated_neck_processing: None,
                            gks_line_state: None,
                            start_ref_element: None,
                            fill_color: None,
                            border_filling: None,
                            border_option_color: None,
                            border_option_inside: None,
                            connection_points: None,
                            child_order: Vec::new(),
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                            ..LineSegment::default()
                        };
                        list.push(line);
                        active = ActiveSection::Kante;
                    }
                }
                "[Ausblenden]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        k.fade_out = Some(FadeOutProps {
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                            ..FadeOutProps::default()
                        });
                        k.child_order.push(KanteChildSlot::FadeOut);
                        active = ActiveSection::FadeOut;
                    }
                }
                "[Mindern]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        k.narrowing = Some(NarrowingProps {
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                            ..NarrowingProps::default()
                        });
                        k.child_order.push(KanteChildSlot::Narrowing);
                        active = ActiveSection::Narrowing;
                    }
                }
                "[Zunehmen]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        k.widening = Some(WideningProps {
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                            ..WideningProps::default()
                        });
                        k.child_order.push(KanteChildSlot::Widening);
                        active = ActiveSection::Widening;
                    }
                }
                "[Abketteln]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        k.binding_off = Some(BindingOffProps {
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                            ..BindingOffProps::default()
                        });
                        k.child_order.push(KanteChildSlot::BindingOff);
                        active = ActiveSection::BindingOff;
                    }
                }
                "[Punkt]" => {
                    let owner = match active_side {
                        ActiveSide::PointsLeft => PointOwner::ElementLeft,
                        ActiveSide::PointsRight => PointOwner::ElementRight,
                        _ => PointOwner::Kante,
                    };
                    let new_pt = |default_idx: i32| PointEntry {
                        index: sec_val.and_then(|s| s.parse().ok()).unwrap_or(default_idx),
                        mark_ke_alignment: None,
                        number_of_marks: None,
                        offsets: Vec::new(),
                        modules: Vec::new(),
                        raw: RawSectionMeta {
                            original_header: Some(trimmed.to_string()),
                            raw_entries: Vec::new(),
                        },
                        ..PointEntry::default()
                    };
                    match owner {
                        PointOwner::Kante => {
                            if let Some(k) = current_kante_mut(&mut elements, active_side) {
                                let p_idx = k.points.len();
                                k.points.push(new_pt(p_idx as i32));
                                k.child_order.push(KanteChildSlot::Point(p_idx));
                                active = ActiveSection::Point(owner);
                            }
                        }
                        PointOwner::ElementLeft => {
                            if let Some(el) = elements.last_mut() {
                                let idx = (el.left_points.len() + 1) as i32;
                                el.left_points.push(new_pt(idx));
                                active = ActiveSection::Point(owner);
                            }
                        }
                        PointOwner::ElementRight => {
                            if let Some(el) = elements.last_mut() {
                                let idx = (el.right_points.len() + 1) as i32;
                                el.right_points.push(new_pt(idx));
                                active = ActiveSection::Point(owner);
                            }
                        }
                    }
                }
                "[Offset]" => {
                    let owner = match active {
                        ActiveSection::Point(o)
                        | ActiveSection::PointOffset(o)
                        | ActiveSection::ModuleArt(ModuleOwner::Point(o)) => o,
                        _ => PointOwner::Kante,
                    };
                    if let Some(p) = current_point_mut(&mut elements, active_side, owner) {
                        let idx = sec_val
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(p.offsets.len() as i32);
                        p.offsets.push(PointOffset {
                            index: idx,
                            x: 0,
                            y: 0,
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                        });
                        active = ActiveSection::PointOffset(owner);
                    }
                }
                "[ModuleArt]" => {
                    let owner = match active {
                        ActiveSection::FadeOut | ActiveSection::ModuleArt(ModuleOwner::FadeOut) => {
                            Some(ModuleOwner::FadeOut)
                        }
                        ActiveSection::Narrowing | ActiveSection::ModuleArt(ModuleOwner::Narrowing) => {
                            Some(ModuleOwner::Narrowing)
                        }
                        ActiveSection::Widening | ActiveSection::ModuleArt(ModuleOwner::Widening) => {
                            Some(ModuleOwner::Widening)
                        }
                        ActiveSection::BindingOff | ActiveSection::ModuleArt(ModuleOwner::BindingOff) => {
                            Some(ModuleOwner::BindingOff)
                        }
                        ActiveSection::Point(po)
                        | ActiveSection::PointOffset(po)
                        | ActiveSection::ModuleArt(ModuleOwner::Point(po)) => {
                            Some(ModuleOwner::Point(po))
                        }
                        _ => None,
                    };
                    if let Some(owner) = owner {
                        let id = sec_val.and_then(|s| s.parse().ok()).unwrap_or(0);
                        let ma = ModuleAssignment {
                            module_art_id: id,
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                            ..ModuleAssignment::default()
                        };
                        match owner {
                            ModuleOwner::FadeOut => {
                                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                                    if let Some(ref mut f) = k.fade_out {
                                        f.modules.push(ma);
                                    }
                                }
                            }
                            ModuleOwner::Narrowing => {
                                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                                    if let Some(ref mut n) = k.narrowing {
                                        n.modules.push(ma);
                                    }
                                }
                            }
                            ModuleOwner::Widening => {
                                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                                    if let Some(ref mut w) = k.widening {
                                        w.modules.push(ma);
                                    }
                                }
                            }
                            ModuleOwner::BindingOff => {
                                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                                    if let Some(ref mut b) = k.binding_off {
                                        b.modules.push(ma);
                                    }
                                }
                            }
                            ModuleOwner::Point(po) => {
                                if let Some(p) = current_point_mut(&mut elements, active_side, po) {
                                    p.modules.push(ma);
                                }
                            }
                        }
                        active = ActiveSection::ModuleArt(owner);
                    }
                }
                "[Kurve]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        k.curve = Some(CurveData {
                            function: 3,
                            steps: Vec::new(),
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                        });
                        k.child_order.push(KanteChildSlot::Curve);
                        active = ActiveSection::Curve;
                    }
                }
                "[KurvenLinie]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        if k.curve.is_none() {
                            k.curve = Some(CurveData::default());
                            k.child_order.push(KanteChildSlot::Curve);
                        }
                        if let Some(ref mut c) = k.curve {
                            let idx = sec_val
                                .and_then(|s| s.parse().ok())
                                .unwrap_or((c.steps.len() + 1) as i32);
                            c.steps.push(CurveStep {
                                index: idx,
                                suppress_first_y: None,
                                raw: RawSectionMeta {
                                    original_header: Some(trimmed.to_string()),
                                    raw_entries: Vec::new(),
                                },
                                ..CurveStep::default()
                            });
                            active = ActiveSection::CurveStep;
                        }
                    }
                }
                "[ConnectionPoints]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        k.connection_points = Some(ConnectionPoints {
                            points: Vec::new(),
                            raw: RawSectionMeta {
                                original_header: Some(trimmed.to_string()),
                                raw_entries: Vec::new(),
                            },
                        });
                        k.child_order.push(KanteChildSlot::ConnectionPoints);
                        active = ActiveSection::ConnectionPoints;
                    }
                }
                "[ConnectPoint]" => {
                    if let Some(k) = current_kante_mut(&mut elements, active_side) {
                        if k.connection_points.is_none() {
                            k.connection_points = Some(ConnectionPoints::default());
                            k.child_order.push(KanteChildSlot::ConnectionPoints);
                        }
                        if let Some(ref mut cp) = k.connection_points {
                            cp.points.push(ConnectPoint {
                                raw: RawSectionMeta {
                                    original_header: Some(trimmed.to_string()),
                                    raw_entries: Vec::new(),
                                },
                                ..ConnectPoint::default()
                            });
                            active = ActiveSection::ConnectPoint;
                        }
                    }
                }
                _ => {
                    // Unknown section header: record safely under current section raw entries if possible
                }
            }
            continue;
        }

        // Key = Value line
        let Some((k_raw, v_raw)) = trimmed.split_once('=') else {
            continue;
        };
        let key = k_raw.trim();
        let val = v_raw.trim();

        match active {
            ActiveSection::Form => {
                header.raw.push_raw(key.to_string(), val.to_string());
                match key {
                    "Name" => header.name = val.to_string(),
                    "TagId" => header.tag_id = val.to_string(),
                    "Version" => header.version = val.to_string(),
                    "SchnittArt" => header.cut_type = val.parse().unwrap_or(-1),
                    "ErstellDatum" => header.created_timestamp = val.parse().unwrap_or(0),
                    "Kommentar" => header.comment = val.to_string(),
                    "SchreibSchutz" => header.write_protected = parse_stoll_bool(val),
                    "Standard" => header.is_standard = parse_stoll_bool(val),
                    "EingabeFormat" => header.input_format = InputFormat::from_ini(val),
                    "EingabeDisplayFormat" => header.display_unit = DisplayUnit::from_ini(val),
                    "HandstrickMethode" => header.hand_knitting_method = parse_stoll_bool(val),
                    "FormAnfangType" => header.form_start_type = val.parse().unwrap_or(2),
                    "MaschenDichteAuf" => header.density_base_mm = val.parse().unwrap_or(100.0),
                    "Breite" => header.width_mm = val.parse().unwrap_or(0.0),
                    "Höhe" => header.height_mm = val.parse().unwrap_or(0.0),
                    "MaschenDichteX" => header.density_x = val.parse().unwrap_or(30.0),
                    "MaschenDichteY" => header.density_y = val.parse().unwrap_or(30.0),
                    "KawAlleNadeln" => header.kaw_all_needles = parse_stoll_bool(val),
                    "IsFromGks" => header.is_from_gks = val.parse().ok(),
                    "GksXmlFileSchema" => header.gks_xml_file_schema = val.parse().ok(),
                    "GuidExterneLokaleModule" => {
                        pending_ext_guid = Some(val.to_string());
                    }
                    "NameExterneLokaleModule" => {
                        if let Some(guid) = pending_ext_guid.take() {
                            header.external_local_modules.push(ExternalLocalModule {
                                guid,
                                name: val.to_string(),
                            });
                        }
                    }
                    "Feinheit" => header.gauge = val.parse().ok(),
                    "LastLinienTagId" => header.last_line_tag_id = val.parse().ok(),
                    "LastFreeLinienTagId" => header.last_free_line_tag_id = val.parse().ok(),
                    "LastPunkteTagId" => header.last_point_tag_id = val.parse().ok(),
                    "LastFreePunkteTagId" => header.last_free_point_tag_id = val.parse().ok(),
                    _ => {}
                }
            }
            ActiveSection::Element => {
                if let Some(el) = elements.last_mut() {
                    el.raw.push_raw(key.to_string(), val.to_string());
                    match key {
                        "Name" => el.name = val.to_string(),
                        "GuidId" => el.guid_id = val.to_string(),
                        "ElementTagId" => el.element_tag_id = val.parse().ok(),
                        "ArtTagId" => el.art_tag_id = val.parse().unwrap_or(1),
                        "Versteckt" => el.hidden = parse_stoll_bool(val),
                        "Template" => el.template = parse_stoll_bool(val),
                        "Gespiegelt" => el.mirrored = parse_stoll_bool(val),
                        "SpiegelVersatz" => el.mirror_offset = val.parse().unwrap_or(0),
                        "AnfangsBreite" => el.start_width = val.parse().unwrap_or(0.0),
                        "AbstandXmm" => el.offset_x_mm = val.parse().unwrap_or(0.0),
                        "AbstandXmmBasis" => el.offset_x_mm_base = val.parse().unwrap_or(0.0),
                        "AbstandYmm" => el.offset_y_mm = val.parse().unwrap_or(0.0),
                        "AbstandXMaschen" => el.offset_x_stitches = val.parse().unwrap_or(0.0),
                        "AbstandXMaschenBasis" => {
                            el.offset_x_stitches_base = val.parse().unwrap_or(0.0)
                        }
                        "AbstandYMaschen" => el.offset_y_rows = val.parse().unwrap_or(0.0),
                        "AbstandYBezugslinie" => {
                            el.y_ref_line = YReferenceLine::from_id(val.parse().unwrap_or(1))
                        }
                        "AbstandHaelften" => el.distance_halves = val.parse().unwrap_or(0),
                        "AbstandXMarkMm" => el.offset_x_mark_mm = val.parse().unwrap_or(0.0),
                        "AbstandXMarkMaschen" => {
                            el.offset_x_mark_stitches = val.parse().unwrap_or(0.0)
                        }
                        "AbstandYMarkMm" => el.offset_y_mark_mm = val.parse().unwrap_or(0.0),
                        "AbstandYMarkMaschen" => el.offset_y_mark_rows = val.parse().unwrap_or(0.0),
                        "Z-Order" => el.z_order = val.parse().ok(),
                        _ => {}
                    }
                }
            }
            ActiveSection::Kante => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    k.raw.push_raw(key.to_string(), val.to_string());
                    match key {
                        "GuidId" => k.guid_id = val.to_string(),
                        "ParamGuidId" => k.param_guid_id = val.to_string(),
                        "TagID" => k.tag_id = val.parse().unwrap_or(0),
                        "Breite" => k.width_mm = val.parse().unwrap_or(0.0),
                        "Höhe" => k.height_mm = val.parse().unwrap_or(0.0),
                        "NadelZahl" => k.stitches = val.parse().unwrap_or(0),
                        "Reihen" => k.rows = val.parse().unwrap_or(0),
                        "Gruppe" => k.group = val.parse().unwrap_or(0),
                        "Kommentar" => k.comment = val.to_string(),
                        "Funktion" => k.function = LineFunction::from_id(val.parse().unwrap_or(0)),
                        "UserSetFunction" => k.user_set_function = parse_stoll_bool(val),
                        "StufungX" => k.step_x = val.parse().unwrap_or(0),
                        "StufungY" => k.step_y = val.parse().unwrap_or(0),
                        "StufungFaktor" => k.step_factor = val.parse().unwrap_or(1),
                        "RestX" => k.rest_x = val.parse().unwrap_or(0),
                        "RestY" => k.rest_y = val.parse().unwrap_or(0),
                        "IsHandKnittingSuppressFirstY" => {
                            k.is_hand_knitting_suppress_first_y = val.parse().ok()
                        }
                        "MitAbketteln" => k.with_bind_off = val.parse().ok(),
                        "GoreKnitting" => k.gore_knitting = val.parse().ok(),
                        "GoreDataNumberOfRowsAdditional" => {
                            k.gore_rows_additional = val.parse().ok()
                        }
                        "GoreDataNumberOfRowsInGore" => k.gore_rows_in_gore = val.parse().ok(),
                        "GoreDataAlignNumberOfRows" => k.gore_align_rows = val.parse().ok(),
                        "IsImitatedNeckProcessing" => {
                            k.is_imitated_neck_processing = val.parse().ok()
                        }
                        "GksLineState" => k.gks_line_state = val.parse().ok(),
                        "KurvenForm" => k.kurven_form = val.parse().unwrap_or(0),
                        "ML-NP_Vorne" => k.ml_np_front = val.parse().unwrap_or(0),
                        "ML-NP_Hinten" => k.ml_np_back = val.parse().unwrap_or(0),
                        "ZweiteFestigkeit" => k.second_stitch_length = parse_stoll_bool(val),
                        "ML-NPZ_Vorne" => k.ml_npz_front = val.parse().unwrap_or(0),
                        "ML-NPZ_Hinten" => k.ml_npz_back = val.parse().unwrap_or(0),
                        "ML-NP_Vorne_Index" => k.ml_np_front_index = val.parse().unwrap_or(0),
                        "ML-NP_Hinten_Index" => k.ml_np_back_index = val.parse().unwrap_or(0),
                        "ML-Breite" => k.ml_width = val.parse().unwrap_or(0),
                        "LastLine" => k.last_line = val.parse().unwrap_or(0),
                        "ChangeAttrib" => k.change_attrib = val.parse().unwrap_or(0),
                        "DefaultUsed" => k.default_used = val.parse().unwrap_or(0),
                        "GroupStart" => k.group_start = parse_stoll_bool(val),
                        "DefaultMindern" => k.default_narrowing = parse_stoll_bool(val),
                        "DefaultZunehmen" => k.default_widening = val.parse().unwrap_or(0),
                        "DefaultHoehe" => k.default_height = val.parse().unwrap_or(0),
                        "StartbezugElement" => k.start_ref_element = val.parse().ok(),
                        "FillColor" => k.fill_color = val.parse().ok(),
                        "BorderFilling" => k.border_filling = val.parse().ok(),
                        "BorderOptionColor" => k.border_option_color = val.parse().ok(),
                        "BorderOptionInside" => k.border_option_inside = val.parse().ok(),
                        _ => {}
                    }
                }
            }
            ActiveSection::FadeOut => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut f) = k.fade_out {
                        f.raw.push_raw(key.to_string(), val.to_string());
                        match key {
                            "ModulFarbeVerwenden" => f.use_module_color = parse_stoll_bool(val),
                            "FuerAlleStrickarten" => f.for_all_structures = val.parse().unwrap_or(0),
                            "KeinAutomatAusblenden" => f.no_auto_fade_out = parse_stoll_bool(val),
                            "OffsetX" => f.offset_x = val.parse().unwrap_or(0),
                            "FillOffsetX" => f.fill_offset_x = val.parse().unwrap_or(0),
                            "FillOffsetY" => f.fill_offset_y = val.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            }
            ActiveSection::Narrowing => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut n) = k.narrowing {
                        n.raw.push_raw(key.to_string(), val.to_string());
                        match key {
                            "FuerAlleStrickarten" => n.for_all_structures = val.parse().unwrap_or(0),
                            "SofortAusfuehren" => n.execute_immediately = parse_stoll_bool(val),
                            "AnzahlMaschen" => n.stitch_count = val.parse().unwrap_or(1),
                            "ModulFarbeVerwenden" => n.use_module_color = parse_stoll_bool(val),
                            _ => {}
                        }
                    }
                }
            }
            ActiveSection::Widening => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut w) = k.widening {
                        w.raw.push_raw(key.to_string(), val.to_string());
                        match key {
                            "FuerAlleStrickarten" => w.for_all_structures = val.parse().unwrap_or(0),
                            "SofortAusfuehren" => w.execute_immediately = parse_stoll_bool(val),
                            "ModulFarbeVerwenden" => w.use_module_color = parse_stoll_bool(val),
                            _ => {}
                        }
                    }
                }
            }
            ActiveSection::BindingOff => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut b) = k.binding_off {
                        if key != "AbkettelTyp" {
                            b.raw.push_raw(key.to_string(), val.to_string());
                        }
                        match key {
                            "FuerAlleStrickarten" => b.for_all_structures = val.parse().unwrap_or(0),
                            "ModulFarbeVerwenden" => b.use_module_color = parse_stoll_bool(val),
                            "AbkettelTyp" => b.bind_off_type = val.parse().ok(),
                            _ => {}
                        }
                    }
                }
            }
            ActiveSection::Point(owner) => {
                if let Some(p) = current_point_mut(&mut elements, active_side, owner) {
                    p.raw.push_raw(key.to_string(), val.to_string());
                    match key {
                        "PunktTagId" => p.punkt_tag_id = val.parse().unwrap_or(0),
                        "PunktArtTagId" => p.punkt_art_tag_id = val.parse().unwrap_or(0),
                        "GruppeTagId" => p.gruppe_tag_id = val.parse().unwrap_or(0),
                        "FuerAlleStrickarten" => p.for_all_structures = val.parse().unwrap_or(0),
                        "ModulFarbeVerwenden" => p.use_module_color = parse_stoll_bool(val),
                        "MarkKeAlignment" => p.mark_ke_alignment = val.parse().ok(),
                        "NumberOfMarks" => p.number_of_marks = val.parse().ok(),
                        _ => {}
                    }
                }
            }
            ActiveSection::PointOffset(owner) => {
                if let Some(p) = current_point_mut(&mut elements, active_side, owner) {
                    if let Some(o) = p.offsets.last_mut() {
                        o.raw.push_raw(key.to_string(), val.to_string());
                        match key {
                            "X" => o.x = val.parse().unwrap_or(0),
                            "Y" => o.y = val.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            }
            ActiveSection::ModuleArt(owner) => {
                let target = match owner {
                    ModuleOwner::FadeOut => current_kante_mut(&mut elements, active_side)
                        .and_then(|k| k.fade_out.as_mut())
                        .and_then(|x| x.modules.last_mut()),
                    ModuleOwner::Narrowing => current_kante_mut(&mut elements, active_side)
                        .and_then(|k| k.narrowing.as_mut())
                        .and_then(|x| x.modules.last_mut()),
                    ModuleOwner::Widening => current_kante_mut(&mut elements, active_side)
                        .and_then(|k| k.widening.as_mut())
                        .and_then(|x| x.modules.last_mut()),
                    ModuleOwner::BindingOff => current_kante_mut(&mut elements, active_side)
                        .and_then(|k| k.binding_off.as_mut())
                        .and_then(|x| x.modules.last_mut()),
                    ModuleOwner::Point(po) => current_point_mut(&mut elements, active_side, po)
                        .and_then(|p| p.modules.last_mut()),
                };
                if let Some(ma) = target {
                    ma.raw.push_raw(key.to_string(), val.to_string());
                    match key {
                        "ID" => ma.id = val.to_string(),
                        "Breite" => ma.width = val.parse().unwrap_or(0),
                        "AbkettelGrenze" => ma.bind_off_limit = val.parse().unwrap_or(999),
                        "MinderPosition" => ma.narrowing_position = val.parse().unwrap_or(4),
                        "FairIsleStatus" => ma.fair_isle_status = val.parse().unwrap_or(0),
                        "FillGuid" => ma.fill_guid = val.to_string(),
                        "BorderOptGuid" => ma.border_opt_guid = val.to_string(),
                        "BorderWidthLeft" => ma.border_width_left = val.parse().unwrap_or(0),
                        "BorderWidthRight" => ma.border_width_right = val.parse().unwrap_or(0),
                        "BorderWidthUp" => ma.border_width_up = val.parse().unwrap_or(0),
                        "BorderWidthDown" => ma.border_width_down = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            ActiveSection::Curve => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut c) = k.curve {
                        c.raw.push_raw(key.to_string(), val.to_string());
                        if key == "Funktion" {
                            c.function = val.parse().unwrap_or(3);
                        }
                    }
                }
            }
            ActiveSection::CurveStep => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut c) = k.curve {
                        if let Some(st) = c.steps.last_mut() {
                            st.raw.push_raw(key.to_string(), val.to_string());
                            match key {
                                "TagID" => st.tag_id = val.parse().unwrap_or(0),
                                "GruppenFaktor" => st.group_factor = val.parse().unwrap_or(1),
                                "GruppenTagId" => st.group_tag_id = val.parse().unwrap_or(0),
                                "StufungBreite" => st.step_width = val.parse().unwrap_or(0),
                                "StufungHoehe" => st.step_height = val.parse().unwrap_or(0),
                                "Faktor" => st.factor = val.parse().unwrap_or(1),
                                "IsHandKnittingSuppressFirstY" => {
                                    st.suppress_first_y = val.parse().ok()
                                }
                                "MindernBreite" => st.narrowing_width = val.parse().unwrap_or(0),
                                "ZunehmenBreite" => st.widening_width = val.parse().unwrap_or(0),
                                "AusblendenBreite" => st.fade_out_width = val.parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                    }
                }
            }
            ActiveSection::ConnectionPoints => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut cp) = k.connection_points {
                        cp.raw.push_raw(key.to_string(), val.to_string());
                    }
                }
            }
            ActiveSection::ConnectPoint => {
                if let Some(k) = current_kante_mut(&mut elements, active_side) {
                    if let Some(ref mut cp) = k.connection_points {
                        if let Some(pt) = cp.points.last_mut() {
                            pt.raw.push_raw(key.to_string(), val.to_string());
                            match key {
                                "ConnectId" => pt.connect_id = val.parse().unwrap_or(1),
                                "ConnectType" => pt.connect_type = val.parse().ok(),
                                "Direction" => pt.direction = parse_stoll_bool(val),
                                _ => {}
                            }
                        }
                    }
                }
            }
            ActiveSection::None => {}
        }
    }

    Ok(FormDocument {
        header,
        elements,
        ole: Arc::new(ole),
        guidelines: Default::default(),
        file_path: None,
        dirty: false,
    })
}

fn parse_section_header(line: &str) -> (&str, Option<&str>) {
    if let Some(end_bracket) = line.find(']') {
        let name = &line[..=end_bracket];
        let rest = line[end_bracket + 1..].trim();
        if let Some(val) = rest.strip_prefix('=') {
            (name, Some(val.trim()))
        } else {
            (name, None)
        }
    } else {
        (line, None)
    }
}

fn current_kante_mut(elements: &mut [ShapeElement], side: ActiveSide) -> Option<&mut LineSegment> {
    let el = elements.last_mut()?;
    match side {
        ActiveSide::Right => el.right_lines.last_mut(),
        _ => el.left_lines.last_mut(),
    }
}

fn current_point_mut(
    elements: &mut [ShapeElement],
    side: ActiveSide,
    owner: PointOwner,
) -> Option<&mut PointEntry> {
    let el = elements.last_mut()?;
    match owner {
        PointOwner::ElementLeft => el.left_points.last_mut(),
        PointOwner::ElementRight => el.right_points.last_mut(),
        PointOwner::Kante => match side {
            ActiveSide::Right => el.right_lines.last_mut()?.points.last_mut(),
            _ => el.left_lines.last_mut()?.points.last_mut(),
        },
    }
}
