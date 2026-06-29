// ============================================================
// RetroX RNMDL AST (GN-Z11)
// Abstract Syntax Tree node definitions.
// Rust 1.95.0 | Edition 2021 | FROZEN
// ============================================================

#[derive(Debug, Clone)]
pub struct ModuleDeclaration {
    pub version: String,
    pub modules: Vec<String>,
    pub line:    usize,
}

#[derive(Debug, Clone)]
pub struct DocumentHeader {
    pub declarations:       Vec<ModuleDeclaration>,
    pub implement_bugfixsets: bool,
}

#[derive(Debug, Clone)]
pub enum Node {
    Document {
        header:   DocumentHeader,
        children: Vec<Node>,
    },
    Container {
        id:       Option<String>,
        children: Vec<Node>,
        line:     usize,
    },
    Section {
        id:       Option<String>,
        children: Vec<Node>,
        line:     usize,
    },
    Heading {
        level: u8,
        text:  String,
        line:  usize,
    },
    Paragraph {
        text: String,
        line: usize,
    },
    Image {
        path: String,
        alt:  String,
        line: usize,
    },
    Comment {
        content: String,
        #[allow(dead_code)]
        line:    usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RnmdlVersion {
    GnZ11,
    MilkyWay,
    Sol,
    Luna,
    Andromeda,
    Pallas,
    Halley,
    Ceres,
    Chiron,
    Numeric(String),
    Unknown(String),
}

impl RnmdlVersion {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "gn-z11" | "gnz11" | "v0.0.0"       => RnmdlVersion::GnZ11,
            "milky-way" | "milkyway" | "v1.0.0"  => RnmdlVersion::MilkyWay,
            "sol" | "v1.1.0"                      => RnmdlVersion::Sol,
            "luna" | "v1.1.1"                     => RnmdlVersion::Luna,
            "andromeda" | "v2.0.0"                => RnmdlVersion::Andromeda,
            "pallas"                              => RnmdlVersion::Pallas,
            "halley"                              => RnmdlVersion::Halley,
            "ceres"                               => RnmdlVersion::Ceres,
            "chiron"                              => RnmdlVersion::Chiron,
            s if s.starts_with('v')              => RnmdlVersion::Numeric(s.to_string()),
            other                                => RnmdlVersion::Unknown(other.to_string()),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            RnmdlVersion::GnZ11      => "GN-Z11 (v0.0.0)".to_string(),
            RnmdlVersion::MilkyWay   => "MILKY-WAY (v1.0.0)".to_string(),
            RnmdlVersion::Sol        => "SOL (v1.1.0)".to_string(),
            RnmdlVersion::Luna       => "LUNA (v1.1.1)".to_string(),
            RnmdlVersion::Andromeda  => "ANDROMEDA (v2.0.0)".to_string(),
            RnmdlVersion::Pallas     => "PALLAS (bugfixset)".to_string(),
            RnmdlVersion::Halley     => "HALLEY (bugfixset)".to_string(),
            RnmdlVersion::Ceres      => "CERES (bugfixset)".to_string(),
            RnmdlVersion::Chiron     => "CHIRON (bugfixset)".to_string(),
            RnmdlVersion::Numeric(v) => v.clone(),
            RnmdlVersion::Unknown(v) => format!("UNKNOWN({})", v),
        }
    }
}