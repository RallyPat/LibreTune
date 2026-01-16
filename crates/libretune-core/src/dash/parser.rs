//! TS dashboard XML parser.
//!
//! Parses .dash and .gauge files in TS XML format.

use super::types::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::io::BufRead;

/// Errors that can occur during dashboard parsing.
#[derive(Debug, thiserror::Error)]
pub enum DashParseError {
    #[error("XML parsing error: {0}")]
    XmlError(#[from] quick_xml::Error),
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
    #[error("Missing required element: {0}")]
    MissingElement(String),
    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Parser state for tracking current element context.
#[derive(Debug, Default)]
struct ParserState {
    in_gauge_cluster: bool,
    in_dash_comp: bool,
    in_image_file: bool,
    current_dash_comp_type: Option<String>,
    current_property: Option<String>,
    current_gauge: Option<GaugeConfig>,
    current_indicator: Option<IndicatorConfig>,
    current_image: Option<EmbeddedImage>,
    // For color parsing
    current_color: Option<TsColor>,
    color_property: Option<String>,
}

/// Parse a TunerStudio .dash file from a string.
pub fn parse_dash_file(xml: &str) -> Result<DashFile, DashParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut dash = DashFile::default();
    let mut state = ParserState::default();
    let mut buf = Vec::new();
    let mut text_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                handle_start_element(&mut dash, &mut state, e, &reader)?;
                text_buf.clear();
            }
            Ok(Event::End(ref e)) => {
                handle_end_element(&mut dash, &mut state, e, &text_buf)?;
                text_buf.clear();
            }
            Ok(Event::Empty(ref e)) => {
                // Self-closing elements like <bibliography ... />
                handle_empty_element(&mut dash, &mut state, e)?;
            }
            Ok(Event::Text(ref e)) => {
                text_buf = e.unescape().unwrap_or_default().to_string();
            }
            Ok(Event::CData(ref e)) => {
                text_buf = String::from_utf8_lossy(e.as_ref()).to_string();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DashParseError::XmlError(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(dash)
}

/// Parse a TunerStudio .gauge file from a string.
pub fn parse_gauge_file(xml: &str) -> Result<GaugeFile, DashParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut gauge_file = GaugeFile::default();
    let mut state = ParserState::default();
    let mut buf = Vec::new();
    let mut text_buf = String::new();
    let mut gauge_depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "gauge" {
                    gauge_depth = gauge_depth.saturating_add(1);
                } else if gauge_depth > 0 {
                    if name == "dashComp" {
                        state.in_dash_comp = true;
                        state.current_dash_comp_type = Some("Gauge".to_string());
                        state.current_gauge = Some(GaugeConfig::default());
                    } else if name == "imageFile" {
                        state.in_image_file = true;
                        state.current_image = Some(parse_image_file_attributes(e));
                    } else if state.in_dash_comp {
                        state.current_property = Some(name.clone());
                        if is_color_property(&name) {
                            state.color_property = Some(name.clone());
                            state.current_color = Some(parse_color_attributes(e));
                        }
                    }
                }
                text_buf.clear();
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "gauge" {
                    if gauge_depth > 0 {
                        gauge_depth -= 1;
                    }
                } else if name == "dashComp" && state.in_dash_comp {
                    if let Some(gauge) = state.current_gauge.take() {
                        gauge_file.gauge = gauge;
                    }
                    state.in_dash_comp = false;
                    state.current_dash_comp_type = None;
                } else if name == "imageFile" && state.in_image_file {
                    if let Some(mut img) = state.current_image.take() {
                        img.data = text_buf.trim().to_string();
                        gauge_file.embedded_images.push(img);
                    }
                    state.in_image_file = false;
                } else if state.in_dash_comp {
                    // Handle color end
                    if is_color_property(&name) {
                        if let (Some(mut color), Some(prop)) =
                            (state.current_color.take(), state.color_property.take())
                        {
                            if let Ok(int_val) = text_buf.trim().parse::<i32>() {
                                color = TsColor::from_argb_int(int_val);
                            }
                            if let Some(ref mut gauge) = state.current_gauge {
                                set_gauge_color(gauge, &prop, color);
                            }
                        }
                    } else if state.current_property.as_deref() == Some(&name) {
                        // Set the property value
                        if let Some(ref mut gauge) = state.current_gauge {
                            set_gauge_property(gauge, &name, &text_buf);
                        }
                        state.current_property = None;
                    }
                }
                text_buf.clear();
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "bibliography" {
                    gauge_file.bibliography = parse_bibliography_attributes(e);
                } else if name == "versionInfo" {
                    gauge_file.version_info = parse_version_info_attributes(e);
                } else if state.in_dash_comp && is_color_property(&name) {
                    let color = parse_color_attributes(e);
                    if let Some(ref mut gauge) = state.current_gauge {
                        set_gauge_color(gauge, &name, color);
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                text_buf = e.unescape().unwrap_or_default().to_string();
            }
            Ok(Event::CData(ref e)) => {
                text_buf = String::from_utf8_lossy(e.as_ref()).to_string();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DashParseError::XmlError(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(gauge_file)
}

fn handle_start_element<R: BufRead>(
    dash: &mut DashFile,
    state: &mut ParserState,
    e: &BytesStart,
    _reader: &Reader<R>,
) -> Result<(), DashParseError> {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

    match name.as_str() {
        "gaugeCluster" => {
            state.in_gauge_cluster = true;
            dash.gauge_cluster = parse_gauge_cluster_attributes(e);
        }
        "dashComp" => {
            state.in_dash_comp = true;
            let comp_type = get_attribute(e, "type").unwrap_or_default();
            state.current_dash_comp_type = Some(comp_type.clone());
            let comp_type_lower = comp_type.to_lowercase();
            if comp_type_lower.contains("indicator") {
                state.current_indicator = Some(IndicatorConfig::default());
            } else {
                // Default to gauge for unknown types
                state.current_gauge = Some(GaugeConfig::default());
            }
        }
        "imageFile" => {
            state.in_image_file = true;
            state.current_image = Some(parse_image_file_attributes(e));
        }
        _ => {
            if state.in_dash_comp {
                state.current_property = Some(name.clone());
                // Check for color elements
                if is_color_property(&name) {
                    state.color_property = Some(name.clone());
                    state.current_color = Some(parse_color_attributes(e));
                }
            }
        }
    }

    Ok(())
}

fn handle_end_element(
    dash: &mut DashFile,
    state: &mut ParserState,
    e: &quick_xml::events::BytesEnd,
    text: &str,
) -> Result<(), DashParseError> {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

    match name.as_str() {
        "gaugeCluster" => {
            state.in_gauge_cluster = false;
        }
        "dashComp" => {
            if let Some(gauge) = state.current_gauge.take() {
                dash.gauge_cluster
                    .components
                    .push(DashComponent::Gauge(Box::new(gauge)));
            } else if let Some(indicator) = state.current_indicator.take() {
                dash.gauge_cluster
                    .components
                    .push(DashComponent::Indicator(indicator));
            }
            state.in_dash_comp = false;
            state.current_dash_comp_type = None;
        }
        "imageFile" => {
            if let Some(mut img) = state.current_image.take() {
                img.data = text.trim().to_string();
                dash.gauge_cluster.embedded_images.push(img);
            }
            state.in_image_file = false;
        }
        _ => {
            if state.in_dash_comp {
                // Handle color end
                if is_color_property(&name) {
                    if let (Some(mut color), Some(prop)) =
                        (state.current_color.take(), state.color_property.take())
                    {
                        if let Ok(int_val) = text.trim().parse::<i32>() {
                            color = TsColor::from_argb_int(int_val);
                        }
                        if let Some(ref mut gauge) = state.current_gauge {
                            set_gauge_color(gauge, &prop, color);
                        } else if let Some(ref mut indicator) = state.current_indicator {
                            set_indicator_color(indicator, &prop, color);
                        }
                    }
                } else if state.current_property.as_deref() == Some(&name) {
                    // Set the property value
                    if let Some(ref mut gauge) = state.current_gauge {
                        set_gauge_property(gauge, &name, text);
                    } else if let Some(ref mut indicator) = state.current_indicator {
                        set_indicator_property(indicator, &name, text);
                    }
                    state.current_property = None;
                }
            }
        }
    }

    Ok(())
}

fn handle_empty_element(
    dash: &mut DashFile,
    state: &mut ParserState,
    e: &BytesStart,
) -> Result<(), DashParseError> {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

    match name.as_str() {
        "bibliography" => {
            dash.bibliography = parse_bibliography_attributes(e);
        }
        "versionInfo" => {
            dash.version_info = parse_version_info_attributes(e);
        }
        _ if state.in_dash_comp && is_color_property(&name) => {
            let color = parse_color_attributes(e);
            if let Some(ref mut gauge) = state.current_gauge {
                set_gauge_color(gauge, &name, color);
            } else if let Some(ref mut indicator) = state.current_indicator {
                set_indicator_color(indicator, &name, color);
            }
        }
        _ => {}
    }

    Ok(())
}

fn get_attribute(e: &BytesStart, name: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == name.as_bytes() {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
        }
    }
    None
}

fn parse_bibliography_attributes(e: &BytesStart) -> Bibliography {
    Bibliography {
        author: get_attribute(e, "author").unwrap_or_default(),
        company: get_attribute(e, "company").unwrap_or_default(),
        write_date: get_attribute(e, "writeDate").unwrap_or_default(),
    }
}

fn parse_version_info_attributes(e: &BytesStart) -> VersionInfo {
    VersionInfo {
        file_format: get_attribute(e, "fileFormat").unwrap_or_else(|| "3.0".to_string()),
        firmware_signature: get_attribute(e, "firmwareSignature"),
    }
}

fn parse_image_file_attributes(e: &BytesStart) -> EmbeddedImage {
    let type_str = get_attribute(e, "type").unwrap_or_default();
    let resource_type = match type_str.to_lowercase().as_str() {
        "png" => ResourceType::Png,
        "gif" => ResourceType::Gif,
        "ttf" => ResourceType::Ttf,
        _ => ResourceType::Png,
    };

    EmbeddedImage {
        file_name: get_attribute(e, "fileName").unwrap_or_default(),
        image_id: get_attribute(e, "imageId").unwrap_or_default(),
        resource_type,
        data: String::new(), // Will be filled from element text
    }
}

fn parse_gauge_cluster_attributes(e: &BytesStart) -> GaugeCluster {
    let mut cluster = GaugeCluster::default();

    if let Some(val) = get_attribute(e, "antiAliasing") {
        cluster.anti_aliasing = val.parse().unwrap_or(true);
    }
    if let Some(val) = get_attribute(e, "clusterBackgroundColor") {
        if let Ok(int_val) = val.parse::<i32>() {
            cluster.cluster_background_color = TsColor::from_argb_int(int_val);
        }
    }
    if let Some(val) = get_attribute(e, "backgroundDitherColor") {
        if !val.is_empty() {
            if let Ok(int_val) = val.parse::<i32>() {
                cluster.background_dither_color = Some(TsColor::from_argb_int(int_val));
            }
        }
    }
    if let Some(val) = get_attribute(e, "clusterBackgroundImageFileName") {
        if !val.is_empty() {
            cluster.cluster_background_image_file_name = Some(val);
        }
    }
    if let Some(val) = get_attribute(e, "clusterBackgroundImageStyle") {
        cluster.cluster_background_image_style = match val.as_str() {
            "Tile" => BackgroundStyle::Tile,
            "Stretch" => BackgroundStyle::Stretch,
            "Center" => BackgroundStyle::Center,
            "Fit" => BackgroundStyle::Fit,
            _ => BackgroundStyle::Tile,
        };
    }

    cluster
}

fn is_color_property(name: &str) -> bool {
    matches!(
        name,
        "BackColor"
            | "FontColor"
            | "TrimColor"
            | "WarnColor"
            | "CriticalColor"
            | "NeedleColor"
            | "OnTextColor"
            | "OffTextColor"
            | "OnBackgroundColor"
            | "OffBackgroundColor"
    )
}

fn parse_color_attributes(e: &BytesStart) -> TsColor {
    let mut color = TsColor::default();

    if let Some(val) = get_attribute(e, "alpha") {
        color.alpha = val.parse().unwrap_or(255);
    }
    if let Some(val) = get_attribute(e, "red") {
        color.red = val.parse().unwrap_or(0);
    }
    if let Some(val) = get_attribute(e, "green") {
        color.green = val.parse().unwrap_or(0);
    }
    if let Some(val) = get_attribute(e, "blue") {
        color.blue = val.parse().unwrap_or(0);
    }

    color
}

fn set_gauge_color(gauge: &mut GaugeConfig, prop: &str, color: TsColor) {
    match prop {
        "BackColor" => gauge.back_color = color,
        "FontColor" => gauge.font_color = color,
        "TrimColor" => gauge.trim_color = color,
        "WarnColor" => gauge.warn_color = color,
        "CriticalColor" => gauge.critical_color = color,
        "NeedleColor" => gauge.needle_color = color,
        _ => {}
    }
}

fn set_indicator_color(indicator: &mut IndicatorConfig, prop: &str, color: TsColor) {
    match prop {
        "OnTextColor" => indicator.on_text_color = color,
        "OffTextColor" => indicator.off_text_color = color,
        "OnBackgroundColor" => indicator.on_background_color = color,
        "OffBackgroundColor" => indicator.off_background_color = color,
        _ => {}
    }
}

fn set_gauge_property(gauge: &mut GaugeConfig, prop: &str, value: &str) {
    let value = value.trim();

    match prop {
        "Id" => gauge.id = value.to_string(),
        "Title" => gauge.title = value.to_string(),
        "Units" => gauge.units = value.to_string(),
        "Value" => gauge.value = value.parse().unwrap_or(0.0),
        "OutputChannel" => gauge.output_channel = value.to_string(),
        "Min" => gauge.min = value.parse().unwrap_or(0.0),
        "Max" => gauge.max = value.parse().unwrap_or(100.0),
        "MinVP" => {
            gauge.min_vp = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "MaxVP" => {
            gauge.max_vp = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "DefaultMin" => gauge.default_min = value.parse().ok(),
        "DefaultMax" => gauge.default_max = value.parse().ok(),
        "PegLimits" => gauge.peg_limits = value.parse().unwrap_or(true),
        "LowWarning" => gauge.low_warning = value.parse().ok(),
        "HighWarning" => gauge.high_warning = value.parse().ok(),
        "LowCritical" => gauge.low_critical = value.parse().ok(),
        "HighCritical" => gauge.high_critical = value.parse().ok(),
        "LowWarningVP" => {
            gauge.low_warning_vp = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "HighWarningVP" => {
            gauge.high_warning_vp = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "LowCriticalVP" => {
            gauge.low_critical_vp = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "HighCriticalVP" => {
            gauge.high_critical_vp = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "ValueDigits" => gauge.value_digits = value.parse().unwrap_or(0),
        "LabelDigits" => gauge.label_digits = value.parse().unwrap_or(0),
        "FontFamily" => gauge.font_family = value.to_string(),
        "FontSizeAdjustment" => gauge.font_size_adjustment = value.parse().unwrap_or(0),
        "ItalicFont" => gauge.italic_font = value.parse().unwrap_or(false),
        "SweepAngle" => gauge.sweep_angle = value.parse().unwrap_or(270),
        "StartAngle" => gauge.start_angle = value.parse().unwrap_or(135),
        "FaceAngle" => gauge.face_angle = value.parse().unwrap_or(270),
        "SweepBeginDegree" => gauge.sweep_begin_degree = value.parse().unwrap_or(135),
        "CounterClockwise" => gauge.counter_clockwise = value.parse().unwrap_or(false),
        "MajorTicks" => gauge.major_ticks = value.parse().unwrap_or(-1.0),
        "MinorTicks" => gauge.minor_ticks = value.parse().unwrap_or(-1.0),
        "RelativeX" => gauge.relative_x = value.parse().unwrap_or(0.0),
        "RelativeY" => gauge.relative_y = value.parse().unwrap_or(0.0),
        "RelativeWidth" => gauge.relative_width = value.parse().unwrap_or(0.25),
        "RelativeHeight" => gauge.relative_height = value.parse().unwrap_or(0.25),
        "BorderWidth" => gauge.border_width = value.parse().unwrap_or(3),
        "ShortestSize" => gauge.shortest_size = value.parse().unwrap_or(50),
        "ShapeLockedToAspect" => gauge.shape_locked_to_aspect = value.parse().unwrap_or(false),
        "AntialiasingOn" => gauge.antialiasing_on = value.parse().unwrap_or(true),
        "BackgroundImageFileName" => {
            gauge.background_image_file_name = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "NeedleImageFileName" => {
            gauge.needle_image_file_name = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "ShowHistory" => gauge.show_history = value.parse().unwrap_or(false),
        "HistoryValue" => gauge.history_value = value.parse().unwrap_or(0.0),
        "HistoryDelay" => gauge.history_delay = value.parse().unwrap_or(15000),
        "NeedleSmoothing" => gauge.needle_smoothing = value.parse().unwrap_or(1),
        "ShortClickAction" => {
            gauge.short_click_action = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "LongClickAction" => {
            gauge.long_click_action = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "DisplayValueAt180" => gauge.display_value_at_180 = value.parse().unwrap_or(false),
        "GaugeStyle" => gauge.gauge_style = value.to_string(),
        "GaugePainter" => gauge.gauge_painter = GaugePainter::from_ts_string(value),
        "RunDemo" => gauge.run_demo = value.parse().unwrap_or(false),
        _ => {}
    }
}

fn set_indicator_property(indicator: &mut IndicatorConfig, prop: &str, value: &str) {
    let value = value.trim();

    match prop {
        "Id" => indicator.id = value.to_string(),
        "Value" => indicator.value = value.parse().unwrap_or(0.0),
        "OutputChannel" => indicator.output_channel = value.to_string(),
        "OnText" => indicator.on_text = value.to_string(),
        "OffText" => indicator.off_text = value.to_string(),
        "OnImageFileName" => {
            indicator.on_image_file_name = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "OffImageFileName" => {
            indicator.off_image_file_name = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "RelativeX" => indicator.relative_x = value.parse().unwrap_or(0.0),
        "RelativeY" => indicator.relative_y = value.parse().unwrap_or(0.0),
        "RelativeWidth" => indicator.relative_width = value.parse().unwrap_or(0.1),
        "RelativeHeight" => indicator.relative_height = value.parse().unwrap_or(0.05),
        "FontFamily" => indicator.font_family = value.to_string(),
        "ItalicFont" => indicator.italic_font = value.parse().unwrap_or(false),
        "AntialiasingOn" => indicator.antialiasing_on = value.parse().unwrap_or(true),
        "ShortClickAction" => {
            indicator.short_click_action = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "LongClickAction" => {
            indicator.long_click_action = if value == "null" || value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "EcuConfigurationName" => {
            indicator.ecu_configuration_name = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        "Painter" => indicator.indicator_painter = IndicatorPainter::from_ts_string(value),
        "RunDemo" => indicator.run_demo = value.parse().unwrap_or(false),
        _ => {}
    }
}

/// Load a dashboard file from path.
pub fn load_dash_file(path: &std::path::Path) -> Result<DashFile, DashParseError> {
    let content = std::fs::read_to_string(path)?;
    parse_dash_file(&content)
}

/// Load a gauge template file from path.
pub fn load_gauge_file(path: &std::path::Path) -> Result<GaugeFile, DashParseError> {
    let content = std::fs::read_to_string(path)?;
    parse_gauge_file(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dash() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<dsh xmlns="http://www.EFIAnalytics.com/:dsh">
    <bibliography author="Test" company="TestCo" writeDate="2025-01-01"/>
    <versionInfo fileFormat="3.0" firmwareSignature="speeduino"/>
    <gaugeCluster antiAliasing="true" clusterBackgroundColor="-16777216">
        <dashComp type="Gauge">
            <Title type="String">RPM</Title>
            <OutputChannel type="String">rpm</OutputChannel>
            <Min type="double">0.0</Min>
            <Max type="double">8000.0</Max>
            <GaugePainter type="GaugePainter">Analog Gauge</GaugePainter>
            <RelativeX type="double">0.1</RelativeX>
            <RelativeY type="double">0.1</RelativeY>
            <RelativeWidth type="double">0.3</RelativeWidth>
            <RelativeHeight type="double">0.4</RelativeHeight>
        </dashComp>
    </gaugeCluster>
</dsh>"#;

        let result = parse_dash_file(xml);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());

        let dash = result.unwrap();
        assert_eq!(dash.bibliography.author, "Test");
        assert_eq!(dash.version_info.file_format, "3.0");
        assert_eq!(dash.gauge_cluster.components.len(), 1);

        if let DashComponent::Gauge(ref gauge) = dash.gauge_cluster.components[0] {
            assert_eq!(gauge.title, "RPM");
            assert_eq!(gauge.output_channel, "rpm");
            assert_eq!(gauge.min, 0.0);
            assert_eq!(gauge.max, 8000.0);
            assert_eq!(gauge.gauge_painter, GaugePainter::AnalogGauge);
        } else {
            panic!("Expected Gauge component");
        }
    }

    #[test]
    fn test_parse_color() {
        let color = TsColor::from_argb_int(-16777216);
        assert_eq!(color.alpha, 255);
        assert_eq!(color.red, 0);
        assert_eq!(color.green, 0);
        assert_eq!(color.blue, 0);

        let color = TsColor::from_argb_int(-65536);
        assert_eq!(color.alpha, 255);
        assert_eq!(color.red, 255);
        assert_eq!(color.green, 0);
        assert_eq!(color.blue, 0);
    }

    #[test]
    fn test_gauge_painter_parsing() {
        assert_eq!(
            GaugePainter::from_ts_string("Analog Gauge"),
            GaugePainter::AnalogGauge
        );
        assert_eq!(
            GaugePainter::from_ts_string("Basic Readout"),
            GaugePainter::BasicReadout
        );
        assert_eq!(
            GaugePainter::from_ts_string("Asymetric Sweep Gauge"),
            GaugePainter::AsymmetricSweepGauge
        );
        assert_eq!(
            GaugePainter::from_ts_string("Horizontal Bar Gauge"),
            GaugePainter::HorizontalBarGauge
        );
        assert_eq!(
            GaugePainter::from_ts_string(
                "com.efiAnalytics.tunerStudio.renderers.AnalogGaugePainter"
            ),
            GaugePainter::AnalogGauge
        );
        assert_eq!(
            GaugePainter::from_ts_string(
                "com.efiAnalytics.tunerStudio.renderers.BasicReadoutGaugePainter"
            ),
            GaugePainter::BasicReadout
        );
        assert_eq!(
            GaugePainter::from_ts_string(
                "com.efiAnalytics.tunerStudio.renderers.HorizontalBarPainter"
            ),
            GaugePainter::HorizontalBarGauge
        );
        assert_eq!(
            GaugePainter::from_ts_string(
                "com.efiAnalytics.tunerStudio.renderers.RoundAnalogGaugePainter"
            ),
            GaugePainter::RoundGauge
        );
        assert_eq!(
            GaugePainter::from_ts_string("Horizontal Dashed Bar Gauge"),
            GaugePainter::HorizontalDashedBar
        );
    }
}
