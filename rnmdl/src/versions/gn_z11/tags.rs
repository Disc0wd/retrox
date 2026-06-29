// ============================================================
// RNMDL GN-Z11 Tag Registry
// Defines all tags available in GN-Z11 and which module
// each tag belongs to. FROZEN at GN-Z11.
// Rust 1.95.0 | Edition 2021
// ============================================================

/// A tag available in this version.
#[derive(Debug, Clone)]
pub struct TagInfo {
    /// The tag name as it appears in the document e.g. "h1"
    pub name:    &'static str,
    /// The module that must be declared to use this tag
    pub module:  &'static str,
}

/// All tags available in GN-Z11.
pub const TAGS: &[TagInfo] = &[
    // ── text module ────────────────────────────────────────
    TagInfo { name: "container", module: "text" },
    TagInfo { name: "section",   module: "text" },
    TagInfo { name: "h1",        module: "text" },
    TagInfo { name: "h2",        module: "text" },
    TagInfo { name: "h3",        module: "text" },
    TagInfo { name: "heading",   module: "text" },
    TagInfo { name: "paragraph", module: "text" },
    TagInfo { name: "p",         module: "text" },
    // ── images module ──────────────────────────────────────
    TagInfo { name: "image",     module: "images" },
];

/// Look up a tag by name. Returns None if not in GN-Z11.
pub fn get_tag(name: &str) -> Option<&'static TagInfo> {
    TAGS.iter().find(|t| t.name == name)
}

/// All modules available in GN-Z11.
pub const MODULES: &[&str] = &["text", "images"];

/// Which version introduced each module.
/// Used to generate helpful error messages.
pub const MODULE_INTRODUCTIONS: &[(&str, &str)] = &[
    ("text",   "GN-Z11 (v0.0.0)"),
    ("images", "GN-Z11 (v0.0.0)"),
    // Future versions add entries here
    ("links",  "MILKY-WAY (v1.0.0)"),
    ("lists",  "MILKY-WAY (v1.0.0)"),
    ("tables", "MILKY-WAY (v1.0.0)"),
    ("styling", "SOL (v1.1.0)"),
    ("layout",  "SOL (v1.1.0)"),
    ("forms",   "LUNA (v1.1.1)"),
    ("embeds",  "ANDROMEDA (v2.0.0)"),
];

/// Which version introduced each tag.
/// Used to generate helpful error messages.
pub const TAG_INTRODUCTIONS: &[(&str, &str)] = &[
    ("container", "GN-Z11 (v0.0.0)"),
    ("section",   "GN-Z11 (v0.0.0)"),
    ("h1",        "GN-Z11 (v0.0.0)"),
    ("h2",        "GN-Z11 (v0.0.0)"),
    ("h3",        "GN-Z11 (v0.0.0)"),
    ("heading",   "GN-Z11 (v0.0.0)"),
    ("paragraph", "GN-Z11 (v0.0.0)"),
    ("p",         "GN-Z11 (v0.0.0)"),
    ("image",     "GN-Z11 (v0.0.0)"),
    // Future versions add entries here
    ("link",  "MILKY-WAY (v1.0.0)"),
    ("ul",    "MILKY-WAY (v1.0.0)"),
    ("ol",    "MILKY-WAY (v1.0.0)"),
    ("li",    "MILKY-WAY (v1.0.0)"),
    ("table", "MILKY-WAY (v1.0.0)"),
    ("tr",    "MILKY-WAY (v1.0.0)"),
    ("td",    "MILKY-WAY (v1.0.0)"),
    ("th",    "MILKY-WAY (v1.0.0)"),
];

/// Get the version that introduced a module, for error messages.
pub fn module_introduced_in(module: &str) -> Option<&'static str> {
    MODULE_INTRODUCTIONS.iter()
        .find(|(m, _)| *m == module)
        .map(|(_, v)| *v)
}

/// Get the version that introduced a tag, for error messages.
pub fn tag_introduced_in(tag: &str) -> Option<&'static str> {
    TAG_INTRODUCTIONS.iter()
        .find(|(t, _)| *t == tag)
        .map(|(_, v)| *v)
}