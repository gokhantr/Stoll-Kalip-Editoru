use crate::model::{
    enums::{format_01, format_ja_nein},
    BindingOffProps, ConnectPoint, ConnectionPoints, CurveData, CurveStep, FadeOutProps,
    FormDocument, FormHeader, KanteChildSlot, LineSegment, ModuleAssignment, NarrowingProps,
    PointEntry, PointOffset, RawSectionMeta, ShapeElement, WideningProps,
};

const CRLF: &str = "\r\n";

/// Serialize a `FormDocument` into the Stoll INI-like `Form` text stream.
/// Preserves exact original key order, section order, formatting, and unknown keys for 100% lossless round-trips.
pub fn serialize_form_text(doc: &FormDocument) -> String {
    let mut out = String::with_capacity(64 * 1024);

    write_form_header(&mut out, &doc.header);

    for el in &doc.elements {
        write_element(&mut out, el);
    }

    if !doc.ole.form_trailing_newline && out.ends_with(CRLF) {
        out.truncate(out.len() - CRLF.len());
    }
    out
}

/// Encode `Form` text into UTF-16LE bytes (matching original BOM presence — Stoll defaults to NO BOM).
pub fn encode_form_utf16le(text: &str, with_bom: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() * 2 + if with_bom { 2 } else { 0 });
    if with_bom {
        bytes.extend_from_slice(&[0xFF, 0xFE]);
    }
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

fn push_kv(out: &mut String, key: &str, val: &str) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(val);
    out.push_str(CRLF);
}

fn fmt_f2(v: f64) -> String {
    // Normalize -0.00 to 0.00
    let clean = if v.abs() < 0.0005 { 0.0 } else { v };
    format!("{:.2}", clean)
}

fn fmt_bool_preserving(raw: &RawSectionMeta, key: &str, value: bool, default_ja_nein: bool) -> String {
    if raw.uses_ja_nein(key, default_ja_nein) {
        format_ja_nein(value).to_string()
    } else {
        format_01(value).to_string()
    }
}

/// Format an f64 matching raw decimal precision when originally stored as integer vs float (e.g. `AbstandXMaschen`).
fn fmt_num_preserving(raw: &RawSectionMeta, key: &str, value: f64) -> String {
    if let Some(orig) = raw.get_raw(key) {
        if !orig.contains('.') && (value - value.round()).abs() < 1e-6 {
            return format!("{}", value.round() as i64);
        }
    }
    fmt_f2(value)
}

enum KeyEmit {
    Keep,
    Replace(String),
    Omit,
}

/// Helper: emit all keys from `raw.raw_entries` in original order (substituting or omitting typed values via `resolve`),
/// then emit any newly populated known keys from `canonical_order` not yet emitted.
fn emit_section_keys_ex<F>(
    out: &mut String,
    raw: &RawSectionMeta,
    mut resolve: F,
    canonical_order: &[(&str, Option<String>)],
) where
    F: FnMut(&str, &str) -> KeyEmit,
{
    if raw.raw_entries.is_empty() {
        for (k, val_opt) in canonical_order {
            if let Some(val) = val_opt {
                push_kv(out, k, val);
            }
        }
        return;
    }

    let mut emitted_keys: Vec<&str> = Vec::with_capacity(raw.raw_entries.len());
    for (k, orig_v) in &raw.raw_entries {
        match resolve(k.as_str(), orig_v.as_str()) {
            KeyEmit::Replace(v) => push_kv(out, k, &v),
            KeyEmit::Keep => push_kv(out, k, orig_v),
            KeyEmit::Omit => {}
        }
        emitted_keys.push(k.as_str());
    }

    // Append any newly added non-empty/explicit keys that weren't in the original file
    for (k, val_opt) in canonical_order {
        if let Some(val) = val_opt {
            if !emitted_keys.contains(k) {
                push_kv(out, k, val);
            }
        }
    }
}

fn select_extra_added<'a>(
    raw: &RawSectionMeta,
    defaults: Vec<(&'a str, Option<String>)>,
    candidates: Vec<(&'a str, Option<String>)>,
) -> Vec<(&'a str, Option<String>)> {
    if raw.raw_entries.is_empty() {
        defaults
    } else {
        candidates
            .into_iter()
            .filter(|(k, v)| v.is_some() && raw.get_raw(k).is_none())
            .collect()
    }
}

fn emit_section_keys<F>(
    out: &mut String,
    raw: &RawSectionMeta,
    mut resolve: F,
    canonical_order: &[(&str, Option<String>)],
) where
    F: FnMut(&str, &str) -> Option<String>,
{
    emit_section_keys_ex(
        out,
        raw,
        |k, orig| match resolve(k, orig) {
            Some(v) => KeyEmit::Replace(v),
            None => KeyEmit::Keep,
        },
        canonical_order,
    );
}

fn write_form_header(out: &mut String, h: &FormHeader) {
    let hdr = h
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[Form] = {}", h.form_id));
    out.push_str(&hdr);
    out.push_str(CRLF);

    if h.raw.raw_entries.is_empty() {
        push_kv(out, "Name", &h.name);
        push_kv(out, "TagId", &h.tag_id);
        push_kv(out, "Version", &h.version);
        push_kv(out, "SchnittArt", &h.cut_type.to_string());
        push_kv(out, "ErstellDatum", &h.created_timestamp.to_string());
        if !h.comment.is_empty() {
            push_kv(out, "Kommentar", &h.comment);
        }
        push_kv(out, "SchreibSchutz", format_01(h.write_protected));
        push_kv(out, "Standard", format_01(h.is_standard));
        push_kv(out, "EingabeFormat", h.input_format.to_ini());
        push_kv(out, "EingabeDisplayFormat", h.display_unit.to_ini());
        push_kv(out, "HandstrickMethode", format_01(h.hand_knitting_method));
        push_kv(out, "FormAnfangType", &h.form_start_type.to_string());
        push_kv(out, "MaschenDichteAuf", &fmt_f2(h.density_base_mm));
        push_kv(out, "Breite", &fmt_f2(h.width_mm));
        push_kv(out, "Höhe", &fmt_f2(h.height_mm));
        push_kv(out, "MaschenDichteX", &fmt_f2(h.density_x));
        push_kv(out, "MaschenDichteY", &fmt_f2(h.density_y));
        push_kv(out, "KawAlleNadeln", format_01(h.kaw_all_needles));
        if let Some(v) = h.is_from_gks {
            push_kv(out, "IsFromGks", &v.to_string());
        }
        if let Some(v) = h.gks_xml_file_schema {
            push_kv(out, "GksXmlFileSchema", &fmt_f2(v));
        }
        push_kv(
            out,
            "AnzExterneLokaleModule",
            &h.external_local_modules.len().to_string(),
        );
        for m in &h.external_local_modules {
            push_kv(out, "GuidExterneLokaleModule", &m.guid);
            push_kv(out, "NameExterneLokaleModule", &m.name);
        }
        if let Some(g) = h.gauge {
            push_kv(out, "Feinheit", &g.to_string());
        }
        if let Some(v) = h.last_line_tag_id {
            push_kv(out, "LastLinienTagId", &v.to_string());
        }
        if let Some(v) = h.last_free_line_tag_id {
            push_kv(out, "LastFreeLinienTagId", &v.to_string());
        }
        if let Some(v) = h.last_point_tag_id {
            push_kv(out, "LastPunkteTagId", &v.to_string());
        }
        if let Some(v) = h.last_free_point_tag_id {
            push_kv(out, "LastFreePunkteTagId", &v.to_string());
        }
        return;
    }

    // Preserve original raw key order & exact duplicate GuidExterneLokaleModule / NameExterneLokaleModule handling
    let mut ext_guid_idx = 0usize;
    let mut ext_name_idx = 0usize;
    for (k, orig_v) in &h.raw.raw_entries {
        if k == "Kommentar" && h.comment.is_empty() && !orig_v.is_empty() {
            continue;
        }
        let v = match k.as_str() {
            "Name" => h.name.clone(),
            "TagId" => h.tag_id.clone(),
            "Version" => h.version.clone(),
            "SchnittArt" => h.cut_type.to_string(),
            "ErstellDatum" => h.created_timestamp.to_string(),
            "Kommentar" => h.comment.clone(),
            "SchreibSchutz" => fmt_bool_preserving(&h.raw, k, h.write_protected, false),
            "Standard" => fmt_bool_preserving(&h.raw, k, h.is_standard, false),
            "EingabeFormat" => h.input_format.to_ini().to_string(),
            "EingabeDisplayFormat" => h.display_unit.to_ini().to_string(),
            "HandstrickMethode" => fmt_bool_preserving(&h.raw, k, h.hand_knitting_method, false),
            "FormAnfangType" => h.form_start_type.to_string(),
            "MaschenDichteAuf" => fmt_f2(h.density_base_mm),
            "Breite" => fmt_f2(h.width_mm),
            "Höhe" => fmt_f2(h.height_mm),
            "MaschenDichteX" => fmt_f2(h.density_x),
            "MaschenDichteY" => fmt_f2(h.density_y),
            "KawAlleNadeln" => fmt_bool_preserving(&h.raw, k, h.kaw_all_needles, false),
            "IsFromGks" => h
                .is_from_gks
                .map(|x| x.to_string())
                .unwrap_or_else(|| orig_v.clone()),
            "GksXmlFileSchema" => h
                .gks_xml_file_schema
                .map(fmt_f2)
                .unwrap_or_else(|| orig_v.clone()),
            "AnzExterneLokaleModule" => {
                if h.external_local_modules.is_empty() {
                    orig_v.clone()
                } else {
                    h.external_local_modules.len().to_string()
                }
            }
            "GuidExterneLokaleModule" => {
                let s = h
                    .external_local_modules
                    .get(ext_guid_idx)
                    .map(|m| m.guid.clone())
                    .unwrap_or_else(|| orig_v.clone());
                ext_guid_idx += 1;
                s
            }
            "NameExterneLokaleModule" => {
                let s = h
                    .external_local_modules
                    .get(ext_name_idx)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| orig_v.clone());
                ext_name_idx += 1;
                s
            }
            "Feinheit" => h
                .gauge
                .map(|x| x.to_string())
                .unwrap_or_else(|| orig_v.clone()),
            "LastLinienTagId" => h
                .last_line_tag_id
                .map(|x| x.to_string())
                .unwrap_or_else(|| orig_v.clone()),
            "LastFreeLinienTagId" => h
                .last_free_line_tag_id
                .map(|x| x.to_string())
                .unwrap_or_else(|| orig_v.clone()),
            "LastPunkteTagId" => h
                .last_point_tag_id
                .map(|x| x.to_string())
                .unwrap_or_else(|| orig_v.clone()),
            "LastFreePunkteTagId" => h
                .last_free_point_tag_id
                .map(|x| x.to_string())
                .unwrap_or_else(|| orig_v.clone()),
            _ => orig_v.clone(),
        };
        push_kv(out, k, &v);
    }
    for (key, opt_val) in [
        ("Kommentar", (!h.comment.is_empty()).then(|| h.comment.clone())),
        ("Feinheit", h.gauge.map(|g| g.to_string())),
        ("LastLinienTagId", h.last_line_tag_id.map(|v| v.to_string())),
        (
            "LastFreeLinienTagId",
            h.last_free_line_tag_id.map(|v| v.to_string()),
        ),
        ("LastPunkteTagId", h.last_point_tag_id.map(|v| v.to_string())),
        (
            "LastFreePunkteTagId",
            h.last_free_point_tag_id.map(|v| v.to_string()),
        ),
    ] {
        if let Some(val) = opt_val {
            if h.raw.get_raw(key).is_none() {
                push_kv(out, key, &val);
            }
        }
    }
}

fn write_element(out: &mut String, el: &ShapeElement) {
    let hdr = el
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[Element] = {}", el.index));
    out.push_str(&hdr);
    out.push_str(CRLF);

    let defaults: Vec<(&str, Option<String>)> = vec![
        ("Name", (!el.name.is_empty()).then(|| el.name.clone())),
        ("GuidId", (!el.guid_id.is_empty()).then(|| el.guid_id.clone())),
        ("ElementTagId", el.element_tag_id.map(|v| v.to_string())),
        ("ArtTagId", Some(el.art_tag_id.to_string())),
        ("Versteckt", Some(format_01(el.hidden).to_string())),
        ("Template", Some(format_01(el.template).to_string())),
        ("Gespiegelt", Some(format_01(el.mirrored).to_string())),
        ("SpiegelVersatz", Some(el.mirror_offset.to_string())),
        ("AnfangsBreite", Some(fmt_f2(el.start_width))),
        ("AbstandXmm", Some(fmt_f2(el.offset_x_mm))),
        ("AbstandXmmBasis", Some(fmt_f2(el.offset_x_mm_base))),
        ("AbstandYmm", Some(fmt_f2(el.offset_y_mm))),
        ("AbstandXMaschen", Some(fmt_f2(el.offset_x_stitches))),
        (
            "AbstandXMaschenBasis",
            Some(fmt_f2(el.offset_x_stitches_base)),
        ),
        ("AbstandYMaschen", Some(fmt_f2(el.offset_y_rows))),
        ("AbstandYBezugslinie", Some(el.y_ref_line.to_id().to_string())),
        ("AbstandHaelften", Some(el.distance_halves.to_string())),
        ("AbstandXMarkMm", Some(fmt_f2(el.offset_x_mark_mm))),
        ("AbstandXMarkMaschen", Some(fmt_f2(el.offset_x_mark_stitches))),
        ("AbstandYMarkMm", Some(fmt_f2(el.offset_y_mark_mm))),
        ("AbstandYMarkMaschen", Some(fmt_f2(el.offset_y_mark_rows))),
        ("Z-Order", el.z_order.map(|v| v.to_string())),
    ];

    let extra_added: Vec<(&str, Option<String>)> = if el.raw.raw_entries.is_empty() {
        defaults
    } else {
        let mut add = Vec::new();
        if !el.name.is_empty() && el.raw.get_raw("Name").is_none() {
            add.push(("Name", Some(el.name.clone())));
        }
        add
    };

    emit_section_keys_ex(
        out,
        &el.raw,
        |k, orig| match k {
            "Name" => {
                if el.name.is_empty() && !orig.is_empty() {
                    KeyEmit::Omit
                } else {
                    KeyEmit::Replace(el.name.clone())
                }
            }
            "GuidId" => KeyEmit::Replace(el.guid_id.clone()),
            "ElementTagId" => el
                .element_tag_id
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "ArtTagId" => KeyEmit::Replace(el.art_tag_id.to_string()),
            "Versteckt" => KeyEmit::Replace(fmt_bool_preserving(&el.raw, k, el.hidden, false)),
            "Template" => KeyEmit::Replace(fmt_bool_preserving(&el.raw, k, el.template, false)),
            "Gespiegelt" => KeyEmit::Replace(fmt_bool_preserving(&el.raw, k, el.mirrored, false)),
            "SpiegelVersatz" => KeyEmit::Replace(el.mirror_offset.to_string()),
            "AnfangsBreite" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.start_width)),
            "AbstandXmm" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_x_mm)),
            "AbstandXmmBasis" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_x_mm_base)),
            "AbstandYmm" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_y_mm)),
            "AbstandXMaschen" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_x_stitches)),
            "AbstandXMaschenBasis" => {
                KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_x_stitches_base))
            }
            "AbstandYMaschen" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_y_rows)),
            "AbstandYBezugslinie" => KeyEmit::Replace(el.y_ref_line.to_id().to_string()),
            "AbstandHaelften" => KeyEmit::Replace(el.distance_halves.to_string()),
            "AbstandXMarkMm" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_x_mark_mm)),
            "AbstandXMarkMaschen" => {
                KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_x_mark_stitches))
            }
            "AbstandYMarkMm" => KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_y_mark_mm)),
            "AbstandYMarkMaschen" => {
                KeyEmit::Replace(fmt_num_preserving(&el.raw, k, el.offset_y_mark_rows))
            }
            "Z-Order" => el
                .z_order
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            _ => KeyEmit::Keep,
        },
        &extra_added,
    );

    if el.has_left_lines_section || !el.left_lines.is_empty() {
        out.push_str("[LinienLinks]");
        out.push_str(CRLF);
        for line in &el.left_lines {
            write_kante(out, line);
        }
    }

    if el.has_right_lines_section || !el.right_lines.is_empty() {
        out.push_str("[LinienRechts]");
        out.push_str(CRLF);
        for line in &el.right_lines {
            write_kante(out, line);
        }
    }

    if el.has_left_points_section || !el.left_points.is_empty() {
        out.push_str("[PunkteLinks]");
        out.push_str(CRLF);
        for pt in &el.left_points {
            write_point(out, pt);
        }
    }

    if el.has_right_points_section || !el.right_points.is_empty() {
        out.push_str("[PunkteRechts]");
        out.push_str(CRLF);
        for pt in &el.right_points {
            write_point(out, pt);
        }
    }
}

fn write_kante(out: &mut String, k: &LineSegment) {
    let hdr = k
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[Kante] = {}", k.index));
    out.push_str(&hdr);
    out.push_str(CRLF);

    let defaults: Vec<(&str, Option<String>)> = vec![
        ("GuidId", (!k.guid_id.is_empty()).then(|| k.guid_id.clone())),
        (
            "ParamGuidId",
            (!k.param_guid_id.is_empty()).then(|| k.param_guid_id.clone()),
        ),
        ("TagID", Some(k.tag_id.to_string())),
        ("Breite", Some(fmt_f2(k.width_mm))),
        ("Höhe", Some(fmt_f2(k.height_mm))),
        ("NadelZahl", Some(k.stitches.to_string())),
        ("Reihen", Some(k.rows.to_string())),
        ("Gruppe", Some(k.group.to_string())),
        ("Kommentar", (!k.comment.is_empty()).then(|| k.comment.clone())),
        ("Funktion", Some(k.function.to_id().to_string())),
        ("UserSetFunction", Some(format_01(k.user_set_function).to_string())),
        ("StufungX", Some(k.step_x.to_string())),
        ("StufungY", Some(k.step_y.to_string())),
        ("StufungFaktor", Some(k.step_factor.to_string())),
        ("RestX", Some(k.rest_x.to_string())),
        ("RestY", Some(k.rest_y.to_string())),
        (
            "IsHandKnittingSuppressFirstY",
            k.is_hand_knitting_suppress_first_y.map(|v| v.to_string()),
        ),
        ("MitAbketteln", k.with_bind_off.map(|v| v.to_string())),
        ("GoreKnitting", k.gore_knitting.map(|v| v.to_string())),
        (
            "GoreDataNumberOfRowsAdditional",
            k.gore_rows_additional.map(|v| v.to_string()),
        ),
        (
            "GoreDataNumberOfRowsInGore",
            k.gore_rows_in_gore.map(|v| v.to_string()),
        ),
        (
            "GoreDataAlignNumberOfRows",
            k.gore_align_rows.map(|v| v.to_string()),
        ),
        (
            "IsImitatedNeckProcessing",
            k.is_imitated_neck_processing.map(|v| v.to_string()),
        ),
        ("GksLineState", k.gks_line_state.map(|v| v.to_string())),
        ("KurvenForm", Some(k.kurven_form.to_string())),
        ("ML-NP_Vorne", Some(k.ml_np_front.to_string())),
        ("ML-NP_Hinten", Some(k.ml_np_back.to_string())),
        (
            "ZweiteFestigkeit",
            Some(format_01(k.second_stitch_length).to_string()),
        ),
        ("ML-NPZ_Vorne", Some(k.ml_npz_front.to_string())),
        ("ML-NPZ_Hinten", Some(k.ml_npz_back.to_string())),
        ("ML-NP_Vorne_Index", Some(k.ml_np_front_index.to_string())),
        ("ML-NP_Hinten_Index", Some(k.ml_np_back_index.to_string())),
        ("ML-Breite", Some(k.ml_width.to_string())),
        ("LastLine", Some(k.last_line.to_string())),
        ("ChangeAttrib", Some(k.change_attrib.to_string())),
        ("DefaultUsed", Some(k.default_used.to_string())),
        ("GroupStart", Some(format_ja_nein(k.group_start).to_string())),
        (
            "DefaultMindern",
            Some(format_ja_nein(k.default_narrowing).to_string()),
        ),
        ("DefaultZunehmen", Some(k.default_widening.to_string())),
        ("DefaultHoehe", Some(k.default_height.to_string())),
        ("StartbezugElement", k.start_ref_element.map(|v| v.to_string())),
        ("FillColor", k.fill_color.map(|v| v.to_string())),
        ("BorderFilling", k.border_filling.map(|v| v.to_string())),
        ("BorderOptionColor", k.border_option_color.map(|v| v.to_string())),
        (
            "BorderOptionInside",
            k.border_option_inside.map(|v| v.to_string()),
        ),
    ];

    // If the user added a comment or optional/neck/gore/color flags to a parsed line that didn't originally have them, emit them!
    let extra_added = select_extra_added(
        &k.raw,
        defaults,
        vec![
            ("Kommentar", (!k.comment.is_empty()).then(|| k.comment.clone())),
            (
                "IsHandKnittingSuppressFirstY",
                k.is_hand_knitting_suppress_first_y.map(|v| v.to_string()),
            ),
            ("MitAbketteln", k.with_bind_off.map(|v| v.to_string())),
            ("GoreKnitting", k.gore_knitting.map(|v| v.to_string())),
            (
                "GoreDataNumberOfRowsAdditional",
                k.gore_rows_additional.map(|v| v.to_string()),
            ),
            (
                "GoreDataNumberOfRowsInGore",
                k.gore_rows_in_gore.map(|v| v.to_string()),
            ),
            (
                "GoreDataAlignNumberOfRows",
                k.gore_align_rows.map(|v| v.to_string()),
            ),
            (
                "IsImitatedNeckProcessing",
                k.is_imitated_neck_processing.map(|v| v.to_string()),
            ),
            ("GksLineState", k.gks_line_state.map(|v| v.to_string())),
            (
                "StartbezugElement",
                k.start_ref_element.map(|v| v.to_string()),
            ),
            ("FillColor", k.fill_color.map(|v| v.to_string())),
            ("BorderFilling", k.border_filling.map(|v| v.to_string())),
            (
                "BorderOptionColor",
                k.border_option_color.map(|v| v.to_string()),
            ),
            (
                "BorderOptionInside",
                k.border_option_inside.map(|v| v.to_string()),
            ),
        ],
    );

    emit_section_keys_ex(
        out,
        &k.raw,
        |key, orig| match key {
            "GuidId" => KeyEmit::Replace(k.guid_id.clone()),
            "ParamGuidId" => KeyEmit::Replace(k.param_guid_id.clone()),
            "TagID" => KeyEmit::Replace(k.tag_id.to_string()),
            "Breite" => KeyEmit::Replace(fmt_f2(k.width_mm)),
            "Höhe" => KeyEmit::Replace(fmt_f2(k.height_mm)),
            "NadelZahl" => KeyEmit::Replace(k.stitches.to_string()),
            "Reihen" => KeyEmit::Replace(k.rows.to_string()),
            "Gruppe" => KeyEmit::Replace(k.group.to_string()),
            "Kommentar" => {
                if k.comment.is_empty() && !orig.is_empty() {
                    KeyEmit::Omit
                } else {
                    KeyEmit::Replace(k.comment.clone())
                }
            }
            "Funktion" => KeyEmit::Replace(k.function.to_id().to_string()),
            "UserSetFunction" => {
                KeyEmit::Replace(fmt_bool_preserving(&k.raw, key, k.user_set_function, false))
            }
            "StufungX" => KeyEmit::Replace(k.step_x.to_string()),
            "StufungY" => KeyEmit::Replace(k.step_y.to_string()),
            "StufungFaktor" => KeyEmit::Replace(k.step_factor.to_string()),
            "RestX" => KeyEmit::Replace(k.rest_x.to_string()),
            "RestY" => KeyEmit::Replace(k.rest_y.to_string()),
            "IsHandKnittingSuppressFirstY" => k
                .is_hand_knitting_suppress_first_y
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "MitAbketteln" => k
                .with_bind_off
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "GoreKnitting" => k
                .gore_knitting
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "GoreDataNumberOfRowsAdditional" => k
                .gore_rows_additional
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "GoreDataNumberOfRowsInGore" => k
                .gore_rows_in_gore
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "GoreDataAlignNumberOfRows" => k
                .gore_align_rows
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "IsImitatedNeckProcessing" => k
                .is_imitated_neck_processing
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "GksLineState" => k
                .gks_line_state
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "KurvenForm" => KeyEmit::Replace(k.kurven_form.to_string()),
            "ML-NP_Vorne" => KeyEmit::Replace(k.ml_np_front.to_string()),
            "ML-NP_Hinten" => KeyEmit::Replace(k.ml_np_back.to_string()),
            "ZweiteFestigkeit" => {
                KeyEmit::Replace(fmt_bool_preserving(&k.raw, key, k.second_stitch_length, false))
            }
            "ML-NPZ_Vorne" => KeyEmit::Replace(k.ml_npz_front.to_string()),
            "ML-NPZ_Hinten" => KeyEmit::Replace(k.ml_npz_back.to_string()),
            "ML-NP_Vorne_Index" => KeyEmit::Replace(k.ml_np_front_index.to_string()),
            "ML-NP_Hinten_Index" => KeyEmit::Replace(k.ml_np_back_index.to_string()),
            "ML-Breite" => KeyEmit::Replace(k.ml_width.to_string()),
            "LastLine" => KeyEmit::Replace(k.last_line.to_string()),
            "ChangeAttrib" => KeyEmit::Replace(k.change_attrib.to_string()),
            "DefaultUsed" => KeyEmit::Replace(k.default_used.to_string()),
            "GroupStart" => {
                KeyEmit::Replace(fmt_bool_preserving(&k.raw, key, k.group_start, true))
            }
            "DefaultMindern" => {
                KeyEmit::Replace(fmt_bool_preserving(&k.raw, key, k.default_narrowing, true))
            }
            "DefaultZunehmen" => KeyEmit::Replace(k.default_widening.to_string()),
            "DefaultHoehe" => KeyEmit::Replace(k.default_height.to_string()),
            "StartbezugElement" => k
                .start_ref_element
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "FillColor" => k
                .fill_color
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "BorderFilling" => k
                .border_filling
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "BorderOptionColor" => k
                .border_option_color
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            "BorderOptionInside" => k
                .border_option_inside
                .map(|v| KeyEmit::Replace(v.to_string()))
                .unwrap_or(KeyEmit::Keep),
            _ => KeyEmit::Keep,
        },
        &extra_added,
    );

    // Emit child sub-sections in recorded order, plus any newly added children not yet in child_order
    let mut emitted_fade = false;
    let mut emitted_nar = false;
    let mut emitted_wid = false;
    let mut emitted_bo = false;
    let mut emitted_curve = false;
    let mut emitted_cp = false;
    let mut emitted_pts = vec![false; k.points.len()];

    for slot in &k.child_order {
        match *slot {
            KanteChildSlot::FadeOut => {
                if let Some(ref f) = k.fade_out {
                    write_fade_out(out, f);
                    emitted_fade = true;
                }
            }
            KanteChildSlot::Narrowing => {
                if let Some(ref n) = k.narrowing {
                    write_narrowing(out, n);
                    emitted_nar = true;
                }
            }
            KanteChildSlot::Widening => {
                if let Some(ref w) = k.widening {
                    write_widening(out, w);
                    emitted_wid = true;
                }
            }
            KanteChildSlot::BindingOff => {
                if let Some(ref b) = k.binding_off {
                    write_binding_off(out, b);
                    emitted_bo = true;
                }
            }
            KanteChildSlot::Point(idx) => {
                if let Some(p) = k.points.get(idx) {
                    write_point(out, p);
                    emitted_pts[idx] = true;
                }
            }
            KanteChildSlot::Curve => {
                if let Some(ref c) = k.curve {
                    write_curve(out, c);
                    emitted_curve = true;
                }
            }
            KanteChildSlot::ConnectionPoints => {
                if let Some(ref cp) = k.connection_points {
                    write_connection_points(out, cp);
                    emitted_cp = true;
                }
            }
        }
    }

    // Emit any newly added sub-sections not already emitted
    if !emitted_fade {
        if let Some(ref f) = k.fade_out {
            write_fade_out(out, f);
        }
    }
    if !emitted_nar {
        if let Some(ref n) = k.narrowing {
            write_narrowing(out, n);
        }
    }
    if !emitted_wid {
        if let Some(ref w) = k.widening {
            write_widening(out, w);
        }
    }
    if !emitted_bo {
        if let Some(ref b) = k.binding_off {
            write_binding_off(out, b);
        }
    }
    for (idx, p) in k.points.iter().enumerate() {
        if !emitted_pts[idx] {
            write_point(out, p);
        }
    }
    if !emitted_curve {
        if let Some(ref c) = k.curve {
            write_curve(out, c);
        }
    }
    if !emitted_cp {
        if let Some(ref cp) = k.connection_points {
            write_connection_points(out, cp);
        }
    }
}

fn write_module_art(out: &mut String, m: &ModuleAssignment) {
    let hdr = m
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[ModuleArt] = {}", m.module_art_id));
    out.push_str(&hdr);
    out.push_str(CRLF);

    let defaults: Vec<(&str, Option<String>)> = vec![
        ("ID", Some(m.id.clone())),
        ("Breite", Some(m.width.to_string())),
        ("AbkettelGrenze", Some(m.bind_off_limit.to_string())),
        ("MinderPosition", Some(m.narrowing_position.to_string())),
        ("FairIsleStatus", Some(m.fair_isle_status.to_string())),
        ("FillGuid", Some(m.fill_guid.clone())),
        ("BorderOptGuid", Some(m.border_opt_guid.clone())),
        ("BorderWidthLeft", Some(m.border_width_left.to_string())),
        ("BorderWidthRight", Some(m.border_width_right.to_string())),
        ("BorderWidthUp", Some(m.border_width_up.to_string())),
        ("BorderWidthDown", Some(m.border_width_down.to_string())),
    ];

    let def = ModuleAssignment::default();
    let extra_added = select_extra_added(
        &m.raw,
        defaults,
        vec![
            ("ID", (m.id != def.id).then(|| m.id.clone())),
            ("Breite", (m.width != def.width).then(|| m.width.to_string())),
            (
                "AbkettelGrenze",
                (m.bind_off_limit != def.bind_off_limit).then(|| m.bind_off_limit.to_string()),
            ),
            (
                "MinderPosition",
                (m.narrowing_position != def.narrowing_position)
                    .then(|| m.narrowing_position.to_string()),
            ),
            (
                "FairIsleStatus",
                (m.fair_isle_status != def.fair_isle_status).then(|| m.fair_isle_status.to_string()),
            ),
            (
                "FillGuid",
                (m.fill_guid != def.fill_guid).then(|| m.fill_guid.clone()),
            ),
            (
                "BorderOptGuid",
                (m.border_opt_guid != def.border_opt_guid).then(|| m.border_opt_guid.clone()),
            ),
            (
                "BorderWidthLeft",
                (m.border_width_left != def.border_width_left)
                    .then(|| m.border_width_left.to_string()),
            ),
            (
                "BorderWidthRight",
                (m.border_width_right != def.border_width_right)
                    .then(|| m.border_width_right.to_string()),
            ),
            (
                "BorderWidthUp",
                (m.border_width_up != def.border_width_up).then(|| m.border_width_up.to_string()),
            ),
            (
                "BorderWidthDown",
                (m.border_width_down != def.border_width_down)
                    .then(|| m.border_width_down.to_string()),
            ),
        ],
    );

    emit_section_keys(
        out,
        &m.raw,
        |key, _| match key {
            "ID" => Some(m.id.clone()),
            "Breite" => Some(m.width.to_string()),
            "AbkettelGrenze" => Some(m.bind_off_limit.to_string()),
            "MinderPosition" => Some(m.narrowing_position.to_string()),
            "FairIsleStatus" => Some(m.fair_isle_status.to_string()),
            "FillGuid" => Some(m.fill_guid.clone()),
            "BorderOptGuid" => Some(m.border_opt_guid.clone()),
            "BorderWidthLeft" => Some(m.border_width_left.to_string()),
            "BorderWidthRight" => Some(m.border_width_right.to_string()),
            "BorderWidthUp" => Some(m.border_width_up.to_string()),
            "BorderWidthDown" => Some(m.border_width_down.to_string()),
            _ => None,
        },
        &extra_added,
    );
}

fn write_fade_out(out: &mut String, f: &FadeOutProps) {
    out.push_str("[Ausblenden]");
    out.push_str(CRLF);
    let defaults: Vec<(&str, Option<String>)> = vec![
        (
            "ModulFarbeVerwenden",
            Some(format_ja_nein(f.use_module_color).to_string()),
        ),
        ("FuerAlleStrickarten", Some(f.for_all_structures.to_string())),
        (
            "KeinAutomatAusblenden",
            Some(format_01(f.no_auto_fade_out).to_string()),
        ),
        ("OffsetX", Some(f.offset_x.to_string())),
        ("FillOffsetX", Some(f.fill_offset_x.to_string())),
        ("FillOffsetY", Some(f.fill_offset_y.to_string())),
    ];
    let def = FadeOutProps::default();
    let extra_added = select_extra_added(
        &f.raw,
        defaults,
        vec![
            (
                "ModulFarbeVerwenden",
                (f.use_module_color != def.use_module_color)
                    .then(|| format_ja_nein(f.use_module_color).to_string()),
            ),
            (
                "FuerAlleStrickarten",
                (f.for_all_structures != def.for_all_structures)
                    .then(|| f.for_all_structures.to_string()),
            ),
            (
                "KeinAutomatAusblenden",
                (f.no_auto_fade_out != def.no_auto_fade_out)
                    .then(|| format_01(f.no_auto_fade_out).to_string()),
            ),
            (
                "OffsetX",
                (f.offset_x != def.offset_x).then(|| f.offset_x.to_string()),
            ),
            (
                "FillOffsetX",
                (f.fill_offset_x != def.fill_offset_x).then(|| f.fill_offset_x.to_string()),
            ),
            (
                "FillOffsetY",
                (f.fill_offset_y != def.fill_offset_y).then(|| f.fill_offset_y.to_string()),
            ),
        ],
    );
    emit_section_keys(
        out,
        &f.raw,
        |key, _| match key {
            "ModulFarbeVerwenden" => Some(fmt_bool_preserving(&f.raw, key, f.use_module_color, true)),
            "FuerAlleStrickarten" => Some(f.for_all_structures.to_string()),
            "KeinAutomatAusblenden" => {
                Some(fmt_bool_preserving(&f.raw, key, f.no_auto_fade_out, false))
            }
            "OffsetX" => Some(f.offset_x.to_string()),
            "FillOffsetX" => Some(f.fill_offset_x.to_string()),
            "FillOffsetY" => Some(f.fill_offset_y.to_string()),
            _ => None,
        },
        &extra_added,
    );
    for m in &f.modules {
        write_module_art(out, m);
    }
}

fn write_narrowing(out: &mut String, n: &NarrowingProps) {
    out.push_str("[Mindern]");
    out.push_str(CRLF);
    let defaults: Vec<(&str, Option<String>)> = vec![
        ("FuerAlleStrickarten", Some(n.for_all_structures.to_string())),
        (
            "SofortAusfuehren",
            Some(format_01(n.execute_immediately).to_string()),
        ),
        ("AnzahlMaschen", Some(n.stitch_count.to_string())),
        (
            "ModulFarbeVerwenden",
            Some(format_ja_nein(n.use_module_color).to_string()),
        ),
    ];
    let def = NarrowingProps::default();
    let extra_added = select_extra_added(
        &n.raw,
        defaults,
        vec![
            (
                "FuerAlleStrickarten",
                (n.for_all_structures != def.for_all_structures)
                    .then(|| n.for_all_structures.to_string()),
            ),
            (
                "SofortAusfuehren",
                (n.execute_immediately != def.execute_immediately)
                    .then(|| format_01(n.execute_immediately).to_string()),
            ),
            (
                "AnzahlMaschen",
                (n.stitch_count != def.stitch_count).then(|| n.stitch_count.to_string()),
            ),
            (
                "ModulFarbeVerwenden",
                (n.use_module_color != def.use_module_color)
                    .then(|| format_ja_nein(n.use_module_color).to_string()),
            ),
        ],
    );
    emit_section_keys(
        out,
        &n.raw,
        |key, _| match key {
            "FuerAlleStrickarten" => Some(n.for_all_structures.to_string()),
            "SofortAusfuehren" => Some(fmt_bool_preserving(&n.raw, key, n.execute_immediately, false)),
            "AnzahlMaschen" => Some(n.stitch_count.to_string()),
            "ModulFarbeVerwenden" => Some(fmt_bool_preserving(&n.raw, key, n.use_module_color, true)),
            _ => None,
        },
        &extra_added,
    );
    for m in &n.modules {
        write_module_art(out, m);
    }
}

fn write_widening(out: &mut String, w: &WideningProps) {
    out.push_str("[Zunehmen]");
    out.push_str(CRLF);
    let defaults: Vec<(&str, Option<String>)> = vec![
        ("FuerAlleStrickarten", Some(w.for_all_structures.to_string())),
        (
            "SofortAusfuehren",
            Some(format_ja_nein(w.execute_immediately).to_string()),
        ),
        (
            "ModulFarbeVerwenden",
            Some(format_ja_nein(w.use_module_color).to_string()),
        ),
    ];
    let def = WideningProps::default();
    let extra_added = select_extra_added(
        &w.raw,
        defaults,
        vec![
            (
                "FuerAlleStrickarten",
                (w.for_all_structures != def.for_all_structures)
                    .then(|| w.for_all_structures.to_string()),
            ),
            (
                "SofortAusfuehren",
                (w.execute_immediately != def.execute_immediately)
                    .then(|| format_ja_nein(w.execute_immediately).to_string()),
            ),
            (
                "ModulFarbeVerwenden",
                (w.use_module_color != def.use_module_color)
                    .then(|| format_ja_nein(w.use_module_color).to_string()),
            ),
        ],
    );
    emit_section_keys(
        out,
        &w.raw,
        |key, _| match key {
            "FuerAlleStrickarten" => Some(w.for_all_structures.to_string()),
            "SofortAusfuehren" => Some(fmt_bool_preserving(&w.raw, key, w.execute_immediately, true)),
            "ModulFarbeVerwenden" => Some(fmt_bool_preserving(&w.raw, key, w.use_module_color, true)),
            _ => None,
        },
        &extra_added,
    );
    for m in &w.modules {
        write_module_art(out, m);
    }
}

fn write_binding_off(out: &mut String, b: &BindingOffProps) {
    out.push_str("[Abketteln]");
    out.push_str(CRLF);
    let defaults: Vec<(&str, Option<String>)> = vec![
        ("FuerAlleStrickarten", Some(b.for_all_structures.to_string())),
        (
            "ModulFarbeVerwenden",
            Some(format_ja_nein(b.use_module_color).to_string()),
        ),
    ];
    let def = BindingOffProps::default();
    let extra_added = select_extra_added(
        &b.raw,
        defaults,
        vec![
            (
                "FuerAlleStrickarten",
                (b.for_all_structures != def.for_all_structures)
                    .then(|| b.for_all_structures.to_string()),
            ),
            (
                "ModulFarbeVerwenden",
                (b.use_module_color != def.use_module_color)
                    .then(|| format_ja_nein(b.use_module_color).to_string()),
            ),
        ],
    );
    emit_section_keys(
        out,
        &b.raw,
        |key, _| match key {
            "FuerAlleStrickarten" => Some(b.for_all_structures.to_string()),
            "ModulFarbeVerwenden" => Some(fmt_bool_preserving(&b.raw, key, b.use_module_color, true)),
            _ => None,
        },
        &extra_added,
    );
    for m in &b.modules {
        write_module_art(out, m);
    }
}

fn write_point_offset(out: &mut String, o: &PointOffset) {
    let hdr = o
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[Offset] = {}", o.index));
    out.push_str(&hdr);
    out.push_str(CRLF);
    let defaults = vec![
        ("X", Some(o.x.to_string())),
        ("Y", Some(o.y.to_string())),
    ];
    let extra_added = select_extra_added(
        &o.raw,
        defaults,
        vec![
            ("X", (o.x != 0).then(|| o.x.to_string())),
            ("Y", (o.y != 0).then(|| o.y.to_string())),
        ],
    );
    emit_section_keys(
        out,
        &o.raw,
        |key, _| match key {
            "X" => Some(o.x.to_string()),
            "Y" => Some(o.y.to_string()),
            _ => None,
        },
        &extra_added,
    );
}

fn write_point(out: &mut String, p: &PointEntry) {
    let hdr = p
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[Punkt] = {}", p.index));
    out.push_str(&hdr);
    out.push_str(CRLF);
    let defaults: Vec<(&str, Option<String>)> = vec![
        ("PunktTagId", Some(p.punkt_tag_id.to_string())),
        ("PunktArtTagId", Some(p.punkt_art_tag_id.to_string())),
        ("GruppeTagId", Some(p.gruppe_tag_id.to_string())),
        ("FuerAlleStrickarten", Some(p.for_all_structures.to_string())),
        (
            "ModulFarbeVerwenden",
            Some(format_ja_nein(p.use_module_color).to_string()),
        ),
        ("MarkKeAlignment", p.mark_ke_alignment.map(|v| v.to_string())),
        ("NumberOfMarks", p.number_of_marks.map(|v| v.to_string())),
    ];
    let def = PointEntry::default();
    let extra_added = select_extra_added(
        &p.raw,
        defaults,
        vec![
            (
                "PunktTagId",
                (p.punkt_tag_id != def.punkt_tag_id).then(|| p.punkt_tag_id.to_string()),
            ),
            (
                "PunktArtTagId",
                (p.punkt_art_tag_id != def.punkt_art_tag_id)
                    .then(|| p.punkt_art_tag_id.to_string()),
            ),
            (
                "GruppeTagId",
                (p.gruppe_tag_id != def.gruppe_tag_id).then(|| p.gruppe_tag_id.to_string()),
            ),
            (
                "FuerAlleStrickarten",
                (p.for_all_structures != def.for_all_structures)
                    .then(|| p.for_all_structures.to_string()),
            ),
            (
                "ModulFarbeVerwenden",
                (p.use_module_color != def.use_module_color)
                    .then(|| format_ja_nein(p.use_module_color).to_string()),
            ),
            ("MarkKeAlignment", p.mark_ke_alignment.map(|v| v.to_string())),
            ("NumberOfMarks", p.number_of_marks.map(|v| v.to_string())),
        ],
    );
    emit_section_keys(
        out,
        &p.raw,
        |key, _| match key {
            "PunktTagId" => Some(p.punkt_tag_id.to_string()),
            "PunktArtTagId" => Some(p.punkt_art_tag_id.to_string()),
            "GruppeTagId" => Some(p.gruppe_tag_id.to_string()),
            "FuerAlleStrickarten" => Some(p.for_all_structures.to_string()),
            "ModulFarbeVerwenden" => Some(fmt_bool_preserving(&p.raw, key, p.use_module_color, true)),
            "MarkKeAlignment" => p.mark_ke_alignment.map(|v| v.to_string()),
            "NumberOfMarks" => p.number_of_marks.map(|v| v.to_string()),
            _ => None,
        },
        &extra_added,
    );
    for o in &p.offsets {
        write_point_offset(out, o);
    }
    for m in &p.modules {
        write_module_art(out, m);
    }
}

fn write_curve_step(out: &mut String, s: &CurveStep) {
    let hdr = s
        .raw
        .original_header
        .clone()
        .unwrap_or_else(|| format!("[KurvenLinie] = {}", s.index));
    out.push_str(&hdr);
    out.push_str(CRLF);
    let defaults: Vec<(&str, Option<String>)> = vec![
        ("TagID", Some(s.tag_id.to_string())),
        ("GruppenFaktor", Some(s.group_factor.to_string())),
        ("GruppenTagId", Some(s.group_tag_id.to_string())),
        ("StufungBreite", Some(s.step_width.to_string())),
        ("StufungHoehe", Some(s.step_height.to_string())),
        ("Faktor", Some(s.factor.to_string())),
        (
            "IsHandKnittingSuppressFirstY",
            s.suppress_first_y.map(|v| v.to_string()),
        ),
        ("MindernBreite", Some(s.narrowing_width.to_string())),
        ("ZunehmenBreite", Some(s.widening_width.to_string())),
        ("AusblendenBreite", Some(s.fade_out_width.to_string())),
    ];
    let extra_added = select_extra_added(
        &s.raw,
        defaults,
        vec![
            (
                "GruppenFaktor",
                (s.group_factor != 1).then(|| s.group_factor.to_string()),
            ),
            (
                "GruppenTagId",
                (s.group_tag_id != 0).then(|| s.group_tag_id.to_string()),
            ),
            (
                "IsHandKnittingSuppressFirstY",
                s.suppress_first_y.map(|v| v.to_string()),
            ),
            (
                "MindernBreite",
                (s.narrowing_width != 0).then(|| s.narrowing_width.to_string()),
            ),
            (
                "ZunehmenBreite",
                (s.widening_width != 0).then(|| s.widening_width.to_string()),
            ),
            (
                "AusblendenBreite",
                (s.fade_out_width != 0).then(|| s.fade_out_width.to_string()),
            ),
        ],
    );
    emit_section_keys(
        out,
        &s.raw,
        |key, _| match key {
            "TagID" => Some(s.tag_id.to_string()),
            "GruppenFaktor" => Some(s.group_factor.to_string()),
            "GruppenTagId" => Some(s.group_tag_id.to_string()),
            "StufungBreite" => Some(s.step_width.to_string()),
            "StufungHoehe" => Some(s.step_height.to_string()),
            "Faktor" => Some(s.factor.to_string()),
            "IsHandKnittingSuppressFirstY" => s.suppress_first_y.map(|v| v.to_string()),
            "MindernBreite" => Some(s.narrowing_width.to_string()),
            "ZunehmenBreite" => Some(s.widening_width.to_string()),
            "AusblendenBreite" => Some(s.fade_out_width.to_string()),
            _ => None,
        },
        &extra_added,
    );
}

/// Serialize a standalone `[Kurve]` + `[KurvenLinie]` block for `.crv` export.
pub fn serialize_curve_text(c: &CurveData) -> String {
    let mut out = String::with_capacity(2048);
    write_curve(&mut out, c);
    out
}

fn write_curve(out: &mut String, c: &CurveData) {
    out.push_str("[Kurve]");
    out.push_str(CRLF);
    let defaults = vec![("Funktion", Some(c.function.to_string()))];
    let extra_added = select_extra_added(
        &c.raw,
        defaults,
        vec![("Funktion", Some(c.function.to_string()))],
    );
    emit_section_keys(
        out,
        &c.raw,
        |key, _| match key {
            "Funktion" => Some(c.function.to_string()),
            _ => None,
        },
        &extra_added,
    );
    for s in &c.steps {
        write_curve_step(out, s);
    }
}

fn write_connect_point(out: &mut String, cp: &ConnectPoint) {
    out.push_str("[ConnectPoint]");
    out.push_str(CRLF);
    let defaults = vec![
        ("ConnectId", Some(cp.connect_id.to_string())),
        ("ConnectType", cp.connect_type.map(|v| v.to_string())),
        ("Direction", Some(format_ja_nein(cp.direction).to_string())),
    ];
    let def = ConnectPoint::default();
    let extra_added = select_extra_added(
        &cp.raw,
        defaults,
        vec![
            ("ConnectType", cp.connect_type.map(|v| v.to_string())),
            (
                "Direction",
                (cp.direction != def.direction).then(|| format_ja_nein(cp.direction).to_string()),
            ),
        ],
    );
    emit_section_keys(
        out,
        &cp.raw,
        |key, _| match key {
            "ConnectId" => Some(cp.connect_id.to_string()),
            "ConnectType" => cp.connect_type.map(|v| v.to_string()),
            "Direction" => Some(fmt_bool_preserving(&cp.raw, key, cp.direction, true)),
            _ => None,
        },
        &extra_added,
    );
}

fn write_connection_points(out: &mut String, cp: &ConnectionPoints) {
    out.push_str("[ConnectionPoints]");
    out.push_str(CRLF);
    for (k, v) in &cp.raw.raw_entries {
        push_kv(out, k, v);
    }
    for pt in &cp.points {
        write_connect_point(out, pt);
    }
}
