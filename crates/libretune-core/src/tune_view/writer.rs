//! Writer for TunerStudio `.tuneView` XML files.
//!
//! Emits a deterministic representation of [`TuneView`] that round-trips
//! through [`super::parse_tune_view`] back to an equal struct.

use super::types::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Serialize a [`TuneView`] back to XML.
pub fn write_tune_view(tv: &TuneView) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    out.push_str("<tuneView xmlns=\"");
    out.push_str(TUNE_VIEW_NAMESPACE);
    out.push_str("\">\n");

    if !tv.bibliography.is_empty() {
        out.push_str("<bibliography");
        write_attrs(&mut out, &tv.bibliography);
        out.push_str("/>\n");
    }
    if !tv.version_info.is_empty() {
        out.push_str("<versionInfo");
        write_attrs(&mut out, &tv.version_info);
        out.push_str("/>\n");
    }
    if let Some(img) = &tv.preview_image {
        out.push_str("<previewImage>");
        out.push_str(img);
        out.push_str("</previewImage>\n");
    }

    out.push_str("<tuningView");
    write_attrs(&mut out, &tv.tuning_view_attrs);
    out.push_str(">\n");

    for comp in &tv.tune_comps {
        write_comp(&mut out, comp);
    }

    out.push_str("</tuningView>\n");
    out.push_str("</tuneView>\n");
    out
}

fn write_comp(out: &mut String, comp: &TuneComp) {
    let _ = writeln!(
        out,
        "<tuneComp type=\"{}\">",
        xml_escape_attr(&comp.comp_type)
    );
    for prop in &comp.properties {
        write_prop(out, prop);
    }
    out.push_str("</tuneComp>\n");
}

fn write_prop(out: &mut String, prop: &TuneProp) {
    out.push('<');
    out.push_str(&prop.name);
    // type attribute (preserved order: type first if present, then sorted attrs)
    if let Some(t) = &prop.ts_type {
        let _ = write!(out, " type=\"{}\"", xml_escape_attr(t));
    }
    write_attrs(out, &prop.attrs);

    if prop.text.is_empty() {
        out.push_str("/>\n");
    } else {
        out.push('>');
        out.push_str(&xml_escape_text(&prop.text));
        let _ = writeln!(out, "</{}>", prop.name);
    }
}

fn write_attrs(out: &mut String, attrs: &BTreeMap<String, String>) {
    for (k, v) in attrs {
        let _ = write!(out, " {}=\"{}\"", k, xml_escape_attr(v));
    }
}

fn xml_escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
    out
}
