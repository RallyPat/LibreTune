//! Data types for `.tuneView` files.

use std::collections::BTreeMap;

/// XML namespace for `.tuneView` documents.
pub const TUNE_VIEW_NAMESPACE: &str = "http://www.EFIAnalytics.com/:tuneView";

/// Root document for a `.tuneView` file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TuneView {
    /// `<bibliography>` attributes (author, company, viewName, writeDate, ...).
    pub bibliography: BTreeMap<String, String>,
    /// `<versionInfo>` attributes (enabledCondition, fileFormat, firmwareSignature, ...).
    pub version_info: BTreeMap<String, String>,
    /// Optional base64-encoded `<previewImage>` payload.
    pub preview_image: Option<String>,
    /// `<tuningView>` attributes (Id, ShieldedDuringEdit, ...).
    pub tuning_view_attrs: BTreeMap<String, String>,
    /// Ordered `<tuneComp>` entries inside `<tuningView>`.
    pub tune_comps: Vec<TuneComp>,
}

/// A single `<tuneComp type="...">` entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TuneComp {
    /// Value of the `type` attribute (e.g. `TableEditorPanel`,
    /// `TuneSlectableTable`, `TuneSettingsPanel`, `Gauge`).
    pub comp_type: String,
    /// Ordered child properties.
    pub properties: Vec<TuneProp>,
}

/// A single property element inside a `<tuneComp>`.
///
/// Stock TS property elements look like
/// `<RelativeHeight type="double">0.366</RelativeHeight>` or
/// `<WarnColor alpha="255" blue="0" green="242" red="242" type="Color">-85552</WarnColor>`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TuneProp {
    /// Element tag name (the property name).
    pub name: String,
    /// Value of the `type` attribute, if any.
    pub ts_type: Option<String>,
    /// All other attributes, sorted by name for deterministic round-trip.
    pub attrs: BTreeMap<String, String>,
    /// Element text content (already XML-unescaped).
    pub text: String,
}
