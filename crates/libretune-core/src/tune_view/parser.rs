//! Parser for TunerStudio `.tuneView` XML files.

use super::types::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use std::collections::BTreeMap;

/// Errors that can occur while parsing a `.tuneView` file.
#[derive(Debug, thiserror::Error)]
pub enum TuneViewParseError {
    /// Underlying XML parsing error from quick-xml.
    #[error("XML parsing error: {0}")]
    XmlError(#[from] quick_xml::Error),
    /// Document does not contain the expected root element.
    #[error("Invalid tuneView document: {0}")]
    InvalidFormat(String),
    /// I/O error reading the file.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Parse a `.tuneView` document from a string.
pub fn parse_tune_view(xml: &str) -> Result<TuneView, TuneViewParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut tv = TuneView::default();
    let mut buf = Vec::new();

    // Context tracking
    let mut in_tune_view = false;
    let mut in_tuning_view = false;
    let mut current_comp: Option<TuneComp> = None;
    let mut current_prop: Option<TuneProp> = None;
    let mut current_text = String::new();
    let mut in_preview_image = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "tuneView" => {
                        in_tune_view = true;
                    }
                    "tuningView" if in_tune_view => {
                        in_tuning_view = true;
                        tv.tuning_view_attrs = collect_attrs(e);
                    }
                    "previewImage" if in_tune_view => {
                        in_preview_image = true;
                        current_text.clear();
                    }
                    "tuneComp" if in_tuning_view => {
                        let attrs = collect_attrs(e);
                        let comp_type = attrs.get("type").cloned().unwrap_or_default();
                        current_comp = Some(TuneComp {
                            comp_type,
                            properties: Vec::new(),
                        });
                    }
                    _ if current_comp.is_some() => {
                        // Child property of a tuneComp
                        current_prop = Some(make_prop(&name, e));
                        current_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "bibliography" if in_tune_view => {
                        tv.bibliography = collect_attrs_excluding(e, &[]);
                    }
                    "versionInfo" if in_tune_view => {
                        tv.version_info = collect_attrs_excluding(e, &[]);
                    }
                    "tuneComp" if in_tuning_view => {
                        // Empty tuneComp (no children).
                        let attrs = collect_attrs(e);
                        let comp_type = attrs.get("type").cloned().unwrap_or_default();
                        tv.tune_comps.push(TuneComp {
                            comp_type,
                            properties: Vec::new(),
                        });
                    }
                    _ if current_comp.is_some() => {
                        // Self-closing property element, e.g. <Id type="String"/>
                        let prop = make_prop(&name, e);
                        if let Some(comp) = current_comp.as_mut() {
                            comp.properties.push(prop);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let raw = e.decode().map(|c| c.into_owned()).unwrap_or_default();
                current_text.push_str(&raw);
            }
            Ok(Event::CData(ref e)) => {
                current_text.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::End(ref e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "tuneView" => in_tune_view = false,
                    "tuningView" => in_tuning_view = false,
                    "previewImage" => {
                        if in_preview_image {
                            tv.preview_image = Some(strip_whitespace(&current_text));
                        }
                        in_preview_image = false;
                        current_text.clear();
                    }
                    "tuneComp" => {
                        if let Some(comp) = current_comp.take() {
                            tv.tune_comps.push(comp);
                        }
                    }
                    _ => {
                        // Closing a property element inside a tuneComp
                        if let (Some(mut prop), Some(comp)) =
                            (current_prop.take(), current_comp.as_mut())
                        {
                            if prop.name == name {
                                prop.text = std::mem::take(&mut current_text);
                                comp.properties.push(prop);
                            } else {
                                // Mismatched close — restore and ignore
                                current_prop = Some(prop);
                            }
                        }
                        current_text.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(TuneViewParseError::XmlError(e)),
            _ => {}
        }
        buf.clear();
    }

    if tv.bibliography.is_empty()
        && tv.version_info.is_empty()
        && tv.tune_comps.is_empty()
        && tv.preview_image.is_none()
    {
        return Err(TuneViewParseError::InvalidFormat(
            "no recognized tuneView content".into(),
        ));
    }

    Ok(tv)
}

fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    // Strip optional namespace prefix.
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.into_owned(),
    }
}

fn collect_attrs(e: &BytesStart<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for attr in e.attributes().flatten() {
        let key = local_name(attr.key.as_ref());
        if let Ok(val) = attr.normalized_value(XmlVersion::Implicit1_0) {
            out.insert(key, val.into_owned());
        }
    }
    out
}

fn collect_attrs_excluding(e: &BytesStart<'_>, exclude: &[&str]) -> BTreeMap<String, String> {
    let mut out = collect_attrs(e);
    for k in exclude {
        out.remove(*k);
    }
    out
}

fn make_prop(name: &str, e: &BytesStart<'_>) -> TuneProp {
    let mut attrs = collect_attrs(e);
    let ts_type = attrs.remove("type");
    TuneProp {
        name: name.to_string(),
        ts_type,
        attrs,
        text: String::new(),
    }
}

fn strip_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tune_view::write_tune_view;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<tuneView xmlns="http://www.EFIAnalytics.com/:tuneView">
<bibliography author="Test" company="LibreTune" viewName="Demo" writeDate="2026-04-29"/>
<versionInfo enabledCondition="" fileFormat="1.0" firmwareSignature="MS3 Format 0545.02 "/>
<previewImage>QUJD</previewImage>
<tuningView Id="42" ShieldedDuringEdit="false">
<tuneComp type="TuneSettingsPanel">
<Dirty type="boolean">true</Dirty>
<SettingPanelName type="String">antilag</SettingPanelName>
<Id type="String"/>
<RelativeHeight type="double">1.0</RelativeHeight>
<WarnColor alpha="255" blue="0" green="242" red="242" type="Color">-85552</WarnColor>
</tuneComp>
<tuneComp type="TuneSlectableTable">
<SelectedTableName type="String">als_timing_tbl</SelectedTableName>
</tuneComp>
</tuningView>
</tuneView>
"#;

    #[test]
    fn parses_minimal_tune_view() {
        let tv = parse_tune_view(SAMPLE).expect("parse ok");
        assert_eq!(
            tv.bibliography.get("author").map(String::as_str),
            Some("Test")
        );
        assert_eq!(
            tv.version_info.get("firmwareSignature").map(String::as_str),
            Some("MS3 Format 0545.02 ")
        );
        assert_eq!(tv.preview_image.as_deref(), Some("QUJD"));
        assert_eq!(tv.tune_comps.len(), 2);

        let comp0 = &tv.tune_comps[0];
        assert_eq!(comp0.comp_type, "TuneSettingsPanel");
        assert_eq!(comp0.properties.len(), 5);

        let id = comp0.properties.iter().find(|p| p.name == "Id").unwrap();
        assert_eq!(id.ts_type.as_deref(), Some("String"));
        assert!(id.text.is_empty());

        let warn = comp0
            .properties
            .iter()
            .find(|p| p.name == "WarnColor")
            .unwrap();
        assert_eq!(warn.ts_type.as_deref(), Some("Color"));
        assert_eq!(warn.attrs.get("alpha").map(String::as_str), Some("255"));
        assert_eq!(warn.text, "-85552");
    }

    #[test]
    fn round_trips_minimal_tune_view() {
        let parsed = parse_tune_view(SAMPLE).expect("parse ok");
        let xml = write_tune_view(&parsed);
        let reparsed = parse_tune_view(&xml).expect("reparse ok");
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn rejects_empty_document() {
        let err = parse_tune_view("<?xml version=\"1.0\"?><other/>");
        assert!(matches!(err, Err(TuneViewParseError::InvalidFormat(_))));
    }

    #[test]
    fn handles_namespace_prefix() {
        let xml = r#"<?xml version="1.0"?>
<tv:tuneView xmlns:tv="http://www.EFIAnalytics.com/:tuneView">
<tv:bibliography author="x"/>
<tv:tuningView>
<tv:tuneComp type="X"><tv:Foo type="String">bar</tv:Foo></tv:tuneComp>
</tv:tuningView>
</tv:tuneView>"#;
        let tv = parse_tune_view(xml).expect("parse ok");
        assert_eq!(tv.tune_comps.len(), 1);
        assert_eq!(tv.tune_comps[0].properties[0].text, "bar");
    }
}
