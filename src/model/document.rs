use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::model::enums::{
    CurveForm, DisplayUnit, ElementType, InputFormat, LineFunction, LineSide, XReferenceAxis,
    YReferenceLine,
};
use crate::model::props::{
    BindingOffProps, ConnectionPoints, CurveData, ExternalLocalModule, FadeOutProps,
    KanteChildSlot, NarrowingProps, PointEntry, RawSectionMeta, WideningProps,
};

/// `[Form] = 0` — Global shape document header ("Genel kesim verileri").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormHeader {
    pub form_id: i32,
    pub name: String,
    pub tag_id: String,
    pub version: String,
    pub cut_type: i32,              // SchnittArt (-1)
    pub created_timestamp: i64,     // ErstellDatum
    pub comment: String,            // Kommentar (omitted if empty)
    pub write_protected: bool,      // SchreibSchutz
    pub is_standard: bool,          // Standard
    pub input_format: InputFormat,  // EingabeFormat (Raster | Maschen | RasterNo)
    pub display_unit: DisplayUnit,  // EingabeDisplayFormat (mm | cm | inch)
    pub hand_knitting_method: bool, // HandstrickMethode
    pub form_start_type: i32,       // FormAnfangType (2)
    /// `MaschenDichteAuf` — Reference length in mm (typically 100.00 = 10 cm)
    pub density_base_mm: f64,
    /// `Breite` — Form header width (usually 0.00)
    pub width_mm: f64,
    /// `Höhe` — Form header height (usually 0.00)
    pub height_mm: f64,
    /// `MaschenDichteX` — Stitches per `density_base_mm` (En sıklığı / Wp10cm)
    pub density_x: f64,
    /// `MaschenDichteY` — Rows per `density_base_mm` (Boy sıklığı / Rp10cm)
    pub density_y: f64,
    /// `KawAlleNadeln` — Knit & wear all needles (0/1)
    pub kaw_all_needles: bool,
    pub is_from_gks: Option<i32>,
    pub gks_xml_file_schema: Option<f64>,
    pub external_local_modules: Vec<ExternalLocalModule>,
    pub gauge: Option<i32>, // Feinheit (e.g. 12, 14, 18)
    pub last_line_tag_id: Option<i32>,
    pub last_free_line_tag_id: Option<i32>,
    pub last_point_tag_id: Option<i32>,
    pub last_free_point_tag_id: Option<i32>,
    pub raw: RawSectionMeta,
}

impl Default for FormHeader {
    fn default() -> Self {
        Self {
            form_id: 0,
            name: "Yeni_Kalip".to_string(),
            tag_id: format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase()),
            version: "M1 SHX-Version 0.36".to_string(),
            cut_type: -1,
            created_timestamp: chrono::Utc::now().timestamp(),
            comment: String::new(),
            write_protected: false,
            is_standard: false,
            input_format: InputFormat::Steps,
            display_unit: DisplayUnit::Mm,
            hand_knitting_method: false,
            form_start_type: 2,
            density_base_mm: 100.0,
            width_mm: 0.0,
            height_mm: 0.0,
            density_x: 30.0,
            density_y: 30.0,
            kaw_all_needles: false,
            is_from_gks: Some(0),
            gks_xml_file_schema: Some(0.0),
            external_local_modules: Vec::new(),
            gauge: None,
            last_line_tag_id: Some(0),
            last_free_line_tag_id: None,
            last_point_tag_id: Some(0),
            last_free_point_tag_id: None,
            raw: RawSectionMeta::default(),
        }
    }
}

/// `[Kante] = <idx>` — A single edge/line segment in `[LinienLinks]` or `[LinienRechts]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineSegment {
    pub index: i32,
    pub guid_id: String,
    pub param_guid_id: String,
    pub tag_id: i32,

    /// `Breite` — Horizontal delta in mm
    pub width_mm: f64,
    /// `Höhe` — Vertical delta in mm
    pub height_mm: f64,
    /// `NadelZahl` — Horizontal delta in stitches (En İlmekler)
    pub stitches: i32,
    /// `Reihen` — Vertical delta in rows (Boy İlmekler)
    pub rows: i32,

    /// `Gruppe` — Group number
    pub group: i32,
    /// `Kommentar` — Comment / Description ("Açıklama")
    pub comment: String,
    /// `Funktion` — Line function ID
    pub function: LineFunction,
    /// `UserSetFunction` (0/1)
    pub user_set_function: bool,

    /// `StufungX` — Step width in stitches (En Basamak)
    pub step_x: i32,
    /// `StufungY` — Step height in rows (Boy Basamak)
    pub step_y: i32,
    /// `StufungFaktor` — Repeat factor (Faktör, >= 1)
    pub step_factor: i32,
    /// `RestX` — Remainder X in stitches (En Arta kalan)
    pub rest_x: i32,
    /// `RestY` — Remainder Y in rows (Boy Arta kalan)
    pub rest_y: i32,

    pub is_hand_knitting_suppress_first_y: Option<i32>,
    pub with_bind_off: Option<i32>,              // MitAbketteln
    pub gore_knitting: Option<i32>,              // GoreKnitting
    pub gore_rows_additional: Option<i32>,       // GoreDataNumberOfRowsAdditional
    pub gore_rows_in_gore: Option<i32>,          // GoreDataNumberOfRowsInGore
    pub gore_align_rows: Option<i32>,            // GoreDataAlignNumberOfRows
    pub is_imitated_neck_processing: Option<i32>,// IsImitatedNeckProcessing
    pub gks_line_state: Option<i32>,             // GksLineState

    /// `KurvenForm` — Legacy curve flag (`0` or `1`); actual curve type lives in `[Kurve] -> Funktion`
    pub kurven_form: i32,

    /// Stitch length / NP parameters (`ML-NP_*`)
    pub ml_np_front: i32,
    pub ml_np_back: i32,
    pub second_stitch_length: bool, // ZweiteFestigkeit
    pub ml_npz_front: i32,
    pub ml_npz_back: i32,
    pub ml_np_front_index: i32,
    pub ml_np_back_index: i32,
    pub ml_width: i32, // ML-Breite

    pub last_line: i32,          // LastLine (-102..103, usually 0 or 2)
    pub change_attrib: i32,      // ChangeAttrib
    pub default_used: i32,       // DefaultUsed
    pub group_start: bool,       // GroupStart (ja/nein)
    pub default_narrowing: bool, // DefaultMindern (ja/nein)
    pub default_widening: i32,   // DefaultZunehmen
    pub default_height: i32,     // DefaultHoehe

    pub start_ref_element: Option<i32>,    // StartbezugElement
    pub fill_color: Option<i32>,           // FillColor
    pub border_filling: Option<i32>,       // BorderFilling
    pub border_option_color: Option<i32>,  // BorderOptionColor
    pub border_option_inside: Option<i32>, // BorderOptionInside

    // Sub-sections (Line Properties tabs 2..8 + Curve steps)
    pub fade_out: Option<FadeOutProps>,
    pub narrowing: Option<NarrowingProps>,
    pub widening: Option<WideningProps>,
    pub binding_off: Option<BindingOffProps>,
    pub points: Vec<PointEntry>,
    pub curve: Option<CurveData>,
    pub connection_points: Option<ConnectionPoints>,

    /// Original sub-section emission order for bit-identical round-trip
    pub child_order: Vec<KanteChildSlot>,
    pub raw: RawSectionMeta,
}

impl Default for LineSegment {
    fn default() -> Self {
        Self {
            index: 1,
            guid_id: format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase()),
            param_guid_id: format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase()),
            tag_id: 1,
            width_mm: 0.0,
            height_mm: 0.0,
            stitches: 0,
            rows: 0,
            group: 0,
            comment: String::new(),
            function: LineFunction::None,
            user_set_function: false,
            step_x: 0,
            step_y: 0,
            step_factor: 1,
            rest_x: 0,
            rest_y: 0,
            is_hand_knitting_suppress_first_y: Some(0),
            with_bind_off: Some(0),
            gore_knitting: Some(0),
            gore_rows_additional: Some(0),
            gore_rows_in_gore: Some(0),
            gore_align_rows: Some(0),
            is_imitated_neck_processing: Some(0),
            gks_line_state: Some(0),
            kurven_form: 0,
            ml_np_front: 0,
            ml_np_back: 0,
            second_stitch_length: false,
            ml_npz_front: 0,
            ml_npz_back: 0,
            ml_np_front_index: 0,
            ml_np_back_index: 0,
            ml_width: 0,
            last_line: 0,
            change_attrib: 0,
            default_used: 0,
            group_start: false,
            default_narrowing: false,
            default_widening: 0,
            default_height: 0,
            start_ref_element: None,
            fill_color: None,
            border_filling: None,
            border_option_color: None,
            border_option_inside: None,
            fade_out: None,
            narrowing: None,
            widening: None,
            binding_off: None,
            points: Vec::new(),
            curve: None,
            connection_points: Some(ConnectionPoints::default()),
            child_order: vec![KanteChildSlot::ConnectionPoints],
            raw: RawSectionMeta::default(),
        }
    }
}

impl LineSegment {
    /// Ensure a child slot is registered in `child_order` when adding a new sub-section.
    pub fn ensure_child_slot(&mut self, slot: KanteChildSlot) {
        if !self.child_order.contains(&slot) {
            // Stoll always places [Kurve] first among [Kante] sub-sections, before [Ausblenden]/[Mindern]/etc.
            if matches!(slot, KanteChildSlot::Curve) {
                self.child_order.insert(0, slot);
                return;
            }
            // Insert before ConnectionPoints if present, to match Stoll's native order.
            if let Some(pos) = self
                .child_order
                .iter()
                .position(|s| matches!(s, KanteChildSlot::ConnectionPoints))
            {
                if !matches!(slot, KanteChildSlot::ConnectionPoints) {
                    self.child_order.insert(pos, slot);
                    return;
                }
            }
            self.child_order.push(slot);
        }
    }

    pub fn is_curve(&self) -> bool {
        self.curve.as_ref().map(|c| !c.steps.is_empty()).unwrap_or(false)
    }

    pub fn curve_form(&self) -> CurveForm {
        self.curve
            .as_ref()
            .map(|c| CurveForm::from_id(c.function))
            .unwrap_or(CurveForm::Line)
    }
}

/// `[Element] = <idx>` — A shape part (e.g. 1: Ana Kesim, 2: Yaka Kesimi, 3: Delik, ...).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeElement {
    pub index: i32,
    pub name: String,
    pub guid_id: String,
    pub element_tag_id: Option<i32>,
    pub art_tag_id: i32,
    /// `Versteckt` — Hidden (0/1)
    pub hidden: bool,
    /// `Template` (0/1)
    pub template: bool,
    /// `Gespiegelt` — Mirrored / Symmetric (`Aynalama`, 0/1)
    pub mirrored: bool,
    /// `SpiegelVersatz` — Mirror offset
    pub mirror_offset: i32,
    /// `AnfangsBreite` — Start width (in mm for `.shv` or reference units)
    pub start_width: f64,
    /// `AbstandXmm` — X offset from element middle axis (mm)
    pub offset_x_mm: f64,
    /// `AbstandXmmBasis` — X offset from basic shape middle axis (mm)
    pub offset_x_mm_base: f64,
    /// `AbstandYmm` — Y offset (mm)
    pub offset_y_mm: f64,
    /// `AbstandXMaschen` — X offset from element middle axis (stitches)
    pub offset_x_stitches: f64,
    /// `AbstandXMaschenBasis` — X offset from basic shape middle axis (stitches)
    pub offset_x_stitches_base: f64,
    /// `AbstandYMaschen` — Y offset (rows)
    pub offset_y_rows: f64,
    /// `AbstandYBezugslinie` — 1: Zemin çizgi, 2: Bitiş çizgisi
    pub y_ref_line: YReferenceLine,
    /// `AbstandHaelften` — Distance between halves (`Yarım şekillerin mesafesi`)
    pub distance_halves: i32,
    pub offset_x_mark_mm: f64,
    pub offset_x_mark_stitches: f64,
    pub offset_y_mark_mm: f64,
    pub offset_y_mark_rows: f64,
    pub z_order: Option<i32>,

    /// UI helper: which X reference axis is selected in left panel
    #[serde(skip)]
    pub x_ref_axis: XReferenceAxis,

    /// Whether `[LinienLinks]` section header existed
    pub has_left_lines_section: bool,
    pub left_lines: Vec<LineSegment>,
    /// Whether `[LinienRechts]` section header existed
    pub has_right_lines_section: bool,
    pub right_lines: Vec<LineSegment>,
    /// Standalone markers under `[PunkteLinks]` ("Sol işaretler")
    pub has_left_points_section: bool,
    pub left_points: Vec<PointEntry>,
    /// Standalone markers under `[PunkteRechts]` ("Sağ işaretler")
    pub has_right_points_section: bool,
    pub right_points: Vec<PointEntry>,

    pub raw: RawSectionMeta,
}

impl Default for ShapeElement {
    fn default() -> Self {
        Self {
            index: 1,
            name: "Grundform".to_string(),
            guid_id: format!("{{{}}}", uuid::Uuid::new_v4().to_string().to_uppercase()),
            element_tag_id: None,
            art_tag_id: ElementType::BasicShape.to_id(),
            hidden: false,
            template: false,
            mirrored: true,
            mirror_offset: 0,
            start_width: 500.0,
            offset_x_mm: 0.0,
            offset_x_mm_base: 0.0,
            offset_y_mm: 0.0,
            offset_x_stitches: 0.0,
            offset_x_stitches_base: 0.0,
            offset_y_rows: 0.0,
            y_ref_line: YReferenceLine::BaseLine,
            distance_halves: 0,
            offset_x_mark_mm: 0.0,
            offset_x_mark_stitches: 0.0,
            offset_y_mark_mm: 0.0,
            offset_y_mark_rows: 0.0,
            z_order: Some(0),
            x_ref_axis: XReferenceAxis::MiddleAxis,
            has_left_lines_section: true,
            left_lines: Vec::new(),
            has_right_lines_section: false,
            right_lines: Vec::new(),
            has_left_points_section: false,
            left_points: Vec::new(),
            has_right_points_section: false,
            right_points: Vec::new(),
            raw: RawSectionMeta::default(),
        }
    }
}

impl ShapeElement {
    pub fn element_type(&self) -> ElementType {
        ElementType::from_id(self.art_tag_id)
    }

    pub fn lines(&self, side: LineSide) -> &[LineSegment] {
        match side {
            LineSide::Left => &self.left_lines,
            LineSide::Right => &self.right_lines,
        }
    }

    pub fn lines_mut(&mut self, side: LineSide) -> &mut Vec<LineSegment> {
        match side {
            LineSide::Left => &mut self.left_lines,
            LineSide::Right => &mut self.right_lines,
        }
    }

    /// Returns `true` if `row_idx` on `side` is a locked `Temel` (`BaseLine`) row.
    /// Only `BasicShape` (`Element 1`) locks Row 1 (`row_idx == 0`) unconditionally;
    /// secondary elements (`Yaka Kesimi`, `Tiktak`, `Delik`, `Açıklık`, etc.) only lock rows
    /// explicitly marked `LineFunction::BaseLine` (`func_id == 8`).
    pub fn is_base_line_row(&self, side: LineSide, row_idx: usize) -> bool {
        let is_func_base = self
            .lines(side)
            .get(row_idx)
            .map(|l| l.function == LineFunction::BaseLine)
            .unwrap_or(false);
        is_func_base || (self.element_type() == ElementType::BasicShape && row_idx == 0)
    }

    /// Re-number `index` (1..=N) of lines on a side after insert/delete.
    pub fn renumber_lines(&mut self, side: LineSide) {
        let lines = self.lines_mut(side);
        for (i, line) in lines.iter_mut().enumerate() {
            line.index = (i + 1) as i32;
            line.raw.original_header = Some(format!("[Kante] = {}", line.index));
        }
    }
}

/// Preserved non-`Form` OLE2 Compound Document streams so saving never destroys metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OlePreservedStreams {
    /// Raw `\x05SummaryInformation` stream bytes (if present in source file).
    pub summary_info: Option<Vec<u8>>,
    /// Raw `MusterBitmap` stream bytes (regenerated on save unless preserved).
    pub muster_bitmap: Option<Vec<u8>>,
    /// Any additional unknown OLE2 streams preserved verbatim `(stream_path, bytes)`.
    pub extra_streams: Vec<(String, Vec<u8>)>,
    /// Whether the original `Form` stream had a UTF-16LE BOM (`FF FE`) — Stoll writes WITHOUT BOM.
    pub form_had_bom: bool,
    /// Whether the original `Form` stream had a trailing CRLF.
    pub form_trailing_newline: bool,
}

/// Guidelines / Standards UI settings ("Yönergeler" in top panel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guidelines {
    pub enabled: bool,
    pub apply_to_all_elements: bool,
    pub min_width_stitches: i32,
    pub max_narrowing_step: i32,
    pub prefer_even_rows: bool,
}

impl Default for Guidelines {
    fn default() -> Self {
        Self {
            enabled: false,
            apply_to_all_elements: true,
            min_width_stitches: 4,
            max_narrowing_step: 4,
            prefer_even_rows: true,
        }
    }
}

/// Complete in-memory Stoll `.shp` / `.shv` shape document.
#[derive(Debug, Clone, PartialEq)]
pub struct FormDocument {
    pub header: FormHeader,
    pub elements: Vec<ShapeElement>,
    pub ole: OlePreservedStreams,
    pub guidelines: Guidelines,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
}

impl Default for FormDocument {
    fn default() -> Self {
        Self::new_default_template()
    }
}

impl FormDocument {
    /// Create a clean blank new shape document matching Stoll `vecformeditor.exe` on startup / `Dosya -> Yeni` (#48):
    /// empty lines table (0 lines), blank names, default density 100.0 / 30.0 / 30.0.
    pub fn new_default_template() -> Self {
        let mut header = FormHeader::default();
        header.name = String::new();
        header.density_base_mm = 100.0;
        header.density_x = 30.0;
        header.density_y = 30.0;
        header.last_line_tag_id = Some(0);

        let mut elem = ShapeElement {
            index: 1,
            name: String::new(),
            art_tag_id: ElementType::BasicShape.to_id(),
            mirrored: true,
            start_width: 500.0,
            has_left_lines_section: true,
            left_lines: Vec::new(),
            has_right_lines_section: false,
            right_lines: Vec::new(),
            ..Default::default()
        };
        elem.raw.original_header = Some("[Element] = 1".to_string());

        Self {
            header,
            elements: vec![elem],
            ole: OlePreservedStreams {
                form_had_bom: false,
                form_trailing_newline: true,
                ..Default::default()
            },
            guidelines: Guidelines::default(),
            file_path: None,
            dirty: false,
        }
    }

    /// Allocate the next unique line `TagID`.
    pub fn next_line_tag_id(&mut self) -> i32 {
        let max_existing = self
            .elements
            .iter()
            .flat_map(|e| e.left_lines.iter().chain(e.right_lines.iter()))
            .map(|l| l.tag_id)
            .max()
            .unwrap_or(0);
        let cur = self.header.last_line_tag_id.unwrap_or(0).max(max_existing);
        let next = cur + 1;
        self.header.last_line_tag_id = Some(next);
        next
    }

    /// Re-number elements `1..=N`.
    pub fn renumber_elements(&mut self) {
        for (i, el) in self.elements.iter_mut().enumerate() {
            el.index = (i + 1) as i32;
            el.raw.original_header = Some(format!("[Element] = {}", el.index));
        }
    }

    pub fn basic_element(&self) -> Option<&ShapeElement> {
        self.elements
            .iter()
            .find(|e| e.art_tag_id == ElementType::BasicShape.to_id())
            .or_else(|| self.elements.first())
    }
}
