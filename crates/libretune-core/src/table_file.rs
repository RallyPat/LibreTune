//! TunerStudio `.table` file format — reader/writer.
//!
//! A single-table export/import format (`<tableData>` XML), distinct from
//! the full-tune `.msq` format: one table's X/Y axis bins plus its Z-value
//! grid, nothing else. Verified against two real TunerStudio-exported
//! `.table` files (a 16x16 ignition table and a 2x8 injector dead-time
//! table) rather than guessed from documentation, which doesn't describe
//! the format in any detail.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use quick_xml::XmlVersion;

#[derive(Debug, thiserror::Error)]
pub enum TableFileError {
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("XML attribute error: {0}")]
    XmlAttr(#[from] quick_xml::events::attributes::AttrError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML encoding error: {0}")]
    Encoding(#[from] quick_xml::encoding::EncodingError),
    #[error("not a TunerStudio table file (missing <tableData>)")]
    NotATableFile,
    #[error("malformed table file: {0}")]
    Malformed(String),
}

/// A parsed `.table` file, in the same shape `TableData` (the Tauri command
/// struct) already uses, so the caller can validate/apply it directly.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTableFile {
    pub cols: usize,
    pub rows: usize,
    pub x_bins: Vec<f64>,
    pub y_bins: Vec<f64>,
    pub z_values: Vec<Vec<f64>>,
}

/// Write a single table to TunerStudio's `.table` XML format.
///
/// `x_name`/`y_name` land in the `name` attribute TunerStudio puts on
/// `<xAxis>`/`<yAxis>` (the axis's own constant name, e.g. "rpmBins") —
/// informational only, nothing on import depends on it matching anything.
pub fn write_table_file(
    x_name: &str,
    y_name: &str,
    x_bins: &[f64],
    y_bins: &[f64],
    z_values: &[Vec<f64>],
) -> Result<String, TableFileError> {
    let cols = x_bins.len();
    let rows = y_bins.len();
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);

    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("no"),
    )))?;

    let mut root = BytesStart::new("tableData");
    root.push_attribute(("xmlns", "http://www.EFIAnalytics.com/:table"));
    writer.write_event(Event::Start(root))?;

    let mut bib = BytesStart::new("bibliography");
    bib.push_attribute(("author", "LibreTune"));
    bib.push_attribute(("company", "LibreTune, open source"));
    bib.push_attribute(("writeDate", chrono_like_now().as_str()));
    writer.write_event(Event::Empty(bib))?;

    let mut version = BytesStart::new("versionInfo");
    version.push_attribute(("fileFormat", "1.0"));
    writer.write_event(Event::Empty(version))?;

    let mut table = BytesStart::new("table");
    table.push_attribute(("cols", cols.to_string().as_str()));
    table.push_attribute(("rows", rows.to_string().as_str()));
    writer.write_event(Event::Start(table))?;

    let mut x_axis = BytesStart::new("xAxis");
    x_axis.push_attribute(("cols", cols.to_string().as_str()));
    x_axis.push_attribute(("name", x_name));
    writer.write_event(Event::Start(x_axis))?;
    for v in x_bins {
        writer.write_event(Event::Text(BytesText::new(&format!("{}\n", v))))?;
    }
    writer.write_event(Event::End(BytesEnd::new("xAxis")))?;

    let mut y_axis = BytesStart::new("yAxis");
    y_axis.push_attribute(("name", y_name));
    y_axis.push_attribute(("rows", rows.to_string().as_str()));
    writer.write_event(Event::Start(y_axis))?;
    for v in y_bins {
        writer.write_event(Event::Text(BytesText::new(&format!("{}\n", v))))?;
    }
    writer.write_event(Event::End(BytesEnd::new("yAxis")))?;

    let mut z_elem = BytesStart::new("zValues");
    z_elem.push_attribute(("cols", cols.to_string().as_str()));
    z_elem.push_attribute(("rows", rows.to_string().as_str()));
    writer.write_event(Event::Start(z_elem))?;
    for row in z_values {
        let line = row
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        writer.write_event(Event::Text(BytesText::new(&format!("{}\n", line))))?;
    }
    writer.write_event(Event::End(BytesEnd::new("zValues")))?;

    writer.write_event(Event::End(BytesEnd::new("table")))?;
    writer.write_event(Event::End(BytesEnd::new("tableData")))?;

    let result = writer.into_inner();
    Ok(String::from_utf8_lossy(&result).to_string())
}

/// Parse a `.table` file. Tolerant of the exact xmlns URI (TunerStudio
/// builds have shipped more than one — "usEasyDocs.com" and
/// "EFIAnalytics.com" have both been observed in the wild) and of
/// attribute order, since only the element/attribute *names* are load-bearing.
pub fn parse_table_file(xml: &str) -> Result<ParsedTableFile, TableFileError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut cols: Option<usize> = None;
    let mut rows: Option<usize> = None;
    let mut x_bins = Vec::new();
    let mut y_bins = Vec::new();
    let mut z_values = Vec::new();
    let mut in_x_axis = false;
    let mut in_y_axis = false;
    let mut in_z_values = false;
    let mut saw_table_data = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name();
                let local = name.as_ref();
                match local {
                    b"tableData" => saw_table_data = true,
                    b"table" => {
                        for attr in e.attributes() {
                            let attr = attr?;
                            match attr.key.as_ref() {
                                b"cols" => {
                                    cols = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .ok()
                                        .and_then(|v| v.parse().ok())
                                }
                                b"rows" => {
                                    rows = attr
                                        .normalized_value(XmlVersion::Implicit1_0)
                                        .ok()
                                        .and_then(|v| v.parse().ok())
                                }
                                _ => {}
                            }
                        }
                    }
                    b"xAxis" => in_x_axis = true,
                    b"yAxis" => in_y_axis = true,
                    b"zValues" => in_z_values = true,
                    _ => {}
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"xAxis" => in_x_axis = false,
                b"yAxis" => in_y_axis = false,
                b"zValues" => in_z_values = false,
                _ => {}
            },
            Event::Text(t) => {
                let text = t.decode()?.into_owned();
                if in_x_axis {
                    x_bins.extend(parse_whitespace_separated_floats(&text));
                } else if in_y_axis {
                    y_bins.extend(parse_whitespace_separated_floats(&text));
                } else if in_z_values {
                    for line in text.lines() {
                        let row = parse_whitespace_separated_floats(line);
                        if !row.is_empty() {
                            z_values.push(row);
                        }
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }

    if !saw_table_data {
        return Err(TableFileError::NotATableFile);
    }

    let cols = cols.ok_or_else(|| TableFileError::Malformed("missing <table cols=...>".into()))?;
    let rows = rows.ok_or_else(|| TableFileError::Malformed("missing <table rows=...>".into()))?;

    if x_bins.len() != cols {
        return Err(TableFileError::Malformed(format!(
            "xAxis has {} values, table declares {} cols",
            x_bins.len(),
            cols
        )));
    }
    if y_bins.len() != rows {
        return Err(TableFileError::Malformed(format!(
            "yAxis has {} values, table declares {} rows",
            y_bins.len(),
            rows
        )));
    }
    if z_values.len() != rows || z_values.iter().any(|r| r.len() != cols) {
        return Err(TableFileError::Malformed(format!(
            "zValues is not a {}x{} grid",
            cols, rows
        )));
    }

    Ok(ParsedTableFile {
        cols,
        rows,
        x_bins,
        y_bins,
        z_values,
    })
}

fn parse_whitespace_separated_floats(text: &str) -> Vec<f64> {
    text.split_whitespace()
        .filter_map(|tok| tok.parse::<f64>().ok())
        .collect()
}

/// A writeDate string in the same shape TunerStudio's own exports use
/// (`Fri Aug 14 21:39:32 COT 2026`) — informational only, nothing parses it
/// back on import. Hand-rolled instead of pulling in a date/time crate for
/// one cosmetic field.
fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch {} UTC", secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verbatim content from a real TunerStudio-exported 2x8 table
    // (injectorsDeadTime), used here to lock the parser to the real format
    // rather than to our own writer's idea of it.
    const REAL_EXPORT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<tableData xmlns="http://www.EFIAnalytics.com/:table">
<bibliography author="EFI Analytics - philip.tobin@yahoo.com" company="EFI Analytics, copyright 2010, All Rights Reserved." writeDate="Fri Aug 14 22:08:01 COT 2026"/>
<versionInfo fileFormat="1.0"/>
<table cols="8" rows="2">
<xAxis cols="8" name="VBatt">
         6.00
         8.00
         10.00
         11.00
         12.00
         13.00
         14.00
         15.00
      </xAxis>
<yAxis name="pressureCorrectionReference" rows="2">
         206.80
         413.70
      </yAxis>
<zValues cols="8" rows="2">
         4.0 2.4 1.65 1.25 1.05 0.85 0.79 0.70
         3.08 1.65 1.15 1.19 0.99 0.76 0.64 0.6
      </zValues>
</table>
</tableData>
"#;

    #[test]
    fn parses_a_real_tunerstudio_export() {
        let parsed = parse_table_file(REAL_EXPORT).expect("parses");
        assert_eq!(parsed.cols, 8);
        assert_eq!(parsed.rows, 2);
        assert_eq!(
            parsed.x_bins,
            vec![6.0, 8.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0]
        );
        assert_eq!(parsed.y_bins, vec![206.80, 413.70]);
        assert_eq!(parsed.z_values[0][0], 4.0);
        assert_eq!(parsed.z_values[1][7], 0.6);
    }

    #[test]
    fn round_trips_through_our_own_writer() {
        let x_bins = vec![600.0, 1200.0, 3000.0];
        let y_bins = vec![10.0, 50.0];
        let z_values = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let xml = write_table_file("rpmBins", "loadBins", &x_bins, &y_bins, &z_values).unwrap();
        let parsed = parse_table_file(&xml).unwrap();
        assert_eq!(parsed.x_bins, x_bins);
        assert_eq!(parsed.y_bins, y_bins);
        assert_eq!(parsed.z_values, z_values);
    }

    #[test]
    fn rejects_a_non_table_file() {
        let err = parse_table_file("<notATable/>").unwrap_err();
        assert!(matches!(err, TableFileError::NotATableFile));
    }

    #[test]
    fn rejects_dimension_mismatch() {
        // cols says 8 but xAxis only has 2 values.
        let bad = REAL_EXPORT.replace(r#"cols="8""#, r#"cols="9""#);
        assert!(parse_table_file(&bad).is_err());
    }
}
