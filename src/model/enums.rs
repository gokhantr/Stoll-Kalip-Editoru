use serde::{Deserialize, Serialize};

/// Element type (`ArtTagId` in `[Element]`), 1:1 with `messagetextresources_7.dll`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementType {
    /// 1 - Ana Kesim (Grundschnitt)
    BasicShape = 1,
    /// 2 - Yaka Kesimi (Ausschnitt Hals)
    Neckline = 2,
    /// 3 - Kol Kesimi (Ausschnitt Ärmel)
    Armhole = 3,
    /// 4 - Delik (Loch)
    Hole = 4,
    /// 5 - Tiktak (Spickel / Gore)
    Gore = 5,
    /// 6 - Açıklık (Öffnung / Slit)
    Opening = 6,
    /// 7 - Kesim Yaka açıklığı - İşaretleme
    NecklineMarking = 7,
    /// 8 - Delik - İşaretleme
    HoleMarking = 8,
    /// 9 - Açıklık - İşaretleme
    OpeningMarking = 9,
    /// 10 - Pens (Abnäher / Dart)
    Dart = 10,
    /// 0 / other
    Unknown = 0,
}

impl ElementType {
    pub fn from_id(id: i32) -> Self {
        match id {
            1 => Self::BasicShape,
            2 => Self::Neckline,
            3 => Self::Armhole,
            4 => Self::Hole,
            5 => Self::Gore,
            6 => Self::Opening,
            7 => Self::NecklineMarking,
            8 => Self::HoleMarking,
            9 => Self::OpeningMarking,
            10 => Self::Dart,
            _ => Self::Unknown,
        }
    }

    pub fn to_id(self) -> i32 {
        self as i32
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::BasicShape => "Ana Kesim",
            Self::Neckline => "Yaka Kesimi",
            Self::Armhole => "Kol Kesimi",
            Self::Hole => "Delik",
            Self::Gore => "Tiktak",
            Self::Opening => "Açıklık",
            Self::NecklineMarking => "Kesim Yaka açıklığı - İşaretleme",
            Self::HoleMarking => "Delik - İşaretleme",
            Self::OpeningMarking => "Açıklık - İşaretleme",
            Self::Dart => "Pens",
            Self::Unknown => "Diğer",
        }
    }

    /// Standard types shown in the secondary Element `Tür` dropdown (matches VecFormEditor).
    pub fn secondary_dropdown_types() -> &'static [ElementType] {
        &[
            Self::Neckline,
            Self::Hole,
            Self::Gore,
            Self::Opening,
            Self::NecklineMarking,
            Self::HoleMarking,
            Self::OpeningMarking,
            Self::Dart,
        ]
    }

    pub fn all_standard() -> &'static [ElementType] {
        &[
            Self::BasicShape,
            Self::Neckline,
            Self::Armhole,
            Self::Hole,
            Self::Gore,
            Self::Opening,
            Self::NecklineMarking,
            Self::HoleMarking,
            Self::OpeningMarking,
            Self::Dart,
        ]
    }
}

/// Shape cut type (`SchnittArt` in `[Form]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShapeCutType {
    Standard = -1,
    KwBodyFront = 0,
    KwBodyBack = 1,
    KwSleeves = 2,
    KwNeck = 4,
}

impl ShapeCutType {
    pub fn from_id(id: i32) -> Self {
        match id {
            0 => Self::KwBodyFront,
            1 => Self::KwBodyBack,
            2 => Self::KwSleeves,
            4 => Self::KwNeck,
            _ => Self::Standard,
        }
    }

    pub fn to_id(self) -> i32 {
        self as i32
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::Standard => "Standart",
            Self::KwBodyFront => "KW beden, ön",
            Self::KwBodyBack => "KW beden, arka",
            Self::KwSleeves => "KW kollar",
            Self::KwNeck => "KW yaka",
        }
    }

    pub fn all() -> &'static [ShapeCutType] {
        &[
            Self::Standard,
            Self::KwBodyFront,
            Self::KwBodyBack,
            Self::KwSleeves,
            Self::KwNeck,
        ]
    }
}

/// Line function (`Funktion` in `[Kante]`), 1:1 with `SHP_FORMAT_SPEC.md` Section 5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineFunction {
    /// 0 - Normal / Straight (blank cell)
    None = 0,
    /// 1 - Reglan (Mindern / Narrowing)
    Narrowing = 1,
    /// 2 - Artırma (Zunehmen / Widening)
    Widening = 2,
    /// 3 - Remayözleme (Abketteln / Binding-off)
    BindOff = 3,
    /// 4 - Kenar düzeltimi (Ausblenden / Fade-out)
    FadeOut = 4,
    /// 5 - Tiktak (Spickel / Gore)
    Gore = 5,
    /// 8 - Temel (Basis / Base start line)
    BaseLine = 8,
    /// 9 - Reglan M (Mindern M / Multi-step narrowing)
    NarrowingMulti = 9,
}

impl LineFunction {
    pub fn from_id(id: i32) -> Self {
        match id {
            0 => Self::None,
            1 => Self::Narrowing,
            2 => Self::Widening,
            3 => Self::BindOff,
            4 => Self::FadeOut,
            5 => Self::Gore,
            8 => Self::BaseLine,
            9 => Self::NarrowingMulti,
            _ => Self::None,
        }
    }

    pub fn to_id(self) -> i32 {
        self as i32
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Narrowing => "Reglan",
            Self::Widening => "Artırma",
            Self::BindOff => "Remayözleme",
            Self::FadeOut => "Kenar düzeltimi",
            Self::Gore => "Tiktak",
            Self::BaseLine => "Temel",
            Self::NarrowingMulti => "Reglan M",
        }
    }

    pub fn all() -> &'static [LineFunction] {
        &[
            Self::None,
            Self::Narrowing,
            Self::Widening,
            Self::BindOff,
            Self::FadeOut,
            Self::Gore,
            Self::BaseLine,
            Self::NarrowingMulti,
        ]
    }
}

/// Curve function (`Funktion` inside `[Kurve]`), 1:1 with `SHP_FORMAT_SPEC.md` Section 4.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CurveForm {
    /// 1 - Düz çizgi (Line / Bresenham `/`)
    Line = 1,
    /// 2 - Basamak (Step `┌┘`)
    Step = 2,
    /// 3 - J-eğrisi (J-Curve `ノ`)
    JCurve = 3,
    /// 4 - S-eğrisi (S-Curve `∫`)
    SCurve = 4,
    /// 5 - Kullanıcı eğrisi (User Curve `✓`)
    UserCurve = 5,
}

impl CurveForm {
    pub fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Line,
            2 => Self::Step,
            3 => Self::JCurve,
            4 => Self::SCurve,
            5 => Self::UserCurve,
            _ => Self::Line,
        }
    }

    pub fn to_id(self) -> i32 {
        self as i32
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::Line => "Düz çizgi",
            Self::Step => "Basamak",
            Self::JCurve => "J-eğrisi",
            Self::SCurve => "S-eğrisi",
            Self::UserCurve => "Kullanıcı eğrisi",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Line => "/",
            Self::Step => "┌┘",
            Self::JCurve => "ノ",
            Self::SCurve => "∫",
            Self::UserCurve => "✓",
        }
    }
}

/// Input format (`EingabeFormat` in `[Form]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputFormat {
    /// `Raster` — İlmekler / Basamak (`.shp`)
    Steps,
    /// `Stufung` — Steps mode (legacy/converted)
    Stufung,
    /// `Maschen` — İlmekler (Stitches)
    Stitches,
    /// `RasterNo` — Uzunluk / mm (Vector `.shv`)
    Mm,
}

impl InputFormat {
    pub fn from_ini(s: &str) -> Self {
        match s.trim() {
            "Raster" => Self::Steps,
            "Stufung" => Self::Stufung,
            "Maschen" => Self::Stitches,
            "RasterNo" => Self::Mm,
            _ => Self::Steps,
        }
    }

    pub fn to_ini(self) -> &'static str {
        match self {
            Self::Steps => "Raster",
            Self::Stufung => "Stufung",
            Self::Stitches => "Maschen",
            Self::Mm => "RasterNo",
        }
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::Steps => "Basamaklar",
            Self::Stufung => "Basamaklar (Stufung)",
            Self::Stitches => "İlmekler",
            Self::Mm => "mm",
        }
    }
}

/// Display unit (`EingabeDisplayFormat` in `[Form]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisplayUnit {
    Mm,
    Cm,
    Inch,
}

impl DisplayUnit {
    pub fn from_ini(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "cm" => Self::Cm,
            "inch" | "zoll" | "in" => Self::Inch,
            _ => Self::Mm,
        }
    }

    pub fn to_ini(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::Inch => "inch",
        }
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::Inch => "inç",
        }
    }

    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            Self::Mm => mm,
            Self::Cm => mm / 10.0,
            Self::Inch => mm / 25.4,
        }
    }

    pub fn to_mm(self, val: f64) -> f64 {
        match self {
            Self::Mm => val,
            Self::Cm => val * 10.0,
            Self::Inch => val * 25.4,
        }
    }
}

/// Vertical reference line for `y mesafesi` (`AbstandYBezugslinie` in `[Element]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum YReferenceLine {
    /// 1 - Zemin çizgi (Base line / Ground line)
    BaseLine = 1,
    /// 2 - Bitiş çizgisi (End line / Top of basic shape)
    EndLine = 2,
}

impl YReferenceLine {
    pub fn from_id(id: i32) -> Self {
        match id {
            2 => Self::EndLine,
            _ => Self::BaseLine,
        }
    }

    pub fn to_id(self) -> i32 {
        self as i32
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::BaseLine => "Zemin çizgi",
            Self::EndLine => "Bitiş çizgisi",
        }
    }
}

/// Horizontal reference axis for `x mesafesi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum XReferenceAxis {
    #[default]
    MiddleAxis,
    BasicShapeMiddleAxis,
}

impl XReferenceAxis {
    pub fn label_tr(self) -> &'static str {
        match self {
            Self::MiddleAxis => "Orta eksen",
            Self::BasicShapeMiddleAxis => "Ana kesimin orta ekseni",
        }
    }
}

/// Which side of an element is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineSide {
    Left,
    Right,
}

/// Point kind (`PunktArtTagId` in `[Punkt]`), 1:1 with `SHP_FORMAT_SPEC.md` Section 4.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointKind {
    /// 0 - Normal
    Normal = 0,
    /// 1 - üst sol yaka kesimi
    TopLeftNeck = 1,
    /// 2 - Alt kol kesimi
    BottomArmhole = 2,
    /// 3 - Delik
    Hole = 3,
}

impl PointKind {
    pub fn from_id(id: i32) -> Self {
        match id {
            0 => Self::Normal,
            1 => Self::TopLeftNeck,
            2 => Self::BottomArmhole,
            3 => Self::Hole,
            _ => Self::Normal,
        }
    }

    pub fn to_id(self) -> i32 {
        self as i32
    }

    pub fn label_tr(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::TopLeftNeck => "üst sol yaka kesimi",
            Self::BottomArmhole => "Alt kol kesimi",
            Self::Hole => "Delik",
        }
    }
}

pub fn parse_stoll_bool(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "ja" | "1" | "true" | "yes")
}

pub fn format_ja_nein(b: bool) -> &'static str {
    if b { "ja" } else { "nein" }
}

pub fn format_01(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}
