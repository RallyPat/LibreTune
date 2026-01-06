//! TunerStudio dashboard XML writer.
//!
//! Writes .dash and .gauge files in TunerStudio's XML format (version 3.0).

use super::types::*;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Write;

/// Errors that can occur during dashboard writing.
#[derive(Debug, thiserror::Error)]
pub enum DashWriteError {
    #[error("XML writing error: {0}")]
    XmlError(#[from] quick_xml::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Write a dashboard to TunerStudio XML format (version 3.0).
pub fn write_dash_file(dash: &DashFile) -> Result<String, DashWriteError> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("no"),
    )))?;

    // Root element with namespace
    let mut root = BytesStart::new("dsh");
    root.push_attribute(("xmlns", "http://www.EFIAnalytics.com/:dsh"));
    writer.write_event(Event::Start(root))?;

    // Bibliography
    write_bibliography(&mut writer, &dash.bibliography)?;

    // Version info
    write_version_info(&mut writer, &dash.version_info)?;

    // Gauge cluster
    write_gauge_cluster(&mut writer, &dash.gauge_cluster)?;

    // Close root
    writer.write_event(Event::End(BytesEnd::new("dsh")))?;

    let result = writer.into_inner();
    Ok(String::from_utf8_lossy(&result).to_string())
}

/// Write a gauge template to TunerStudio XML format.
pub fn write_gauge_file(gauge_file: &GaugeFile) -> Result<String, DashWriteError> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 4);

    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("no"),
    )))?;

    // Outer gauge element with namespace
    let mut root = BytesStart::new("gauge");
    root.push_attribute(("xmlns", "http://www.EFIAnalytics.com/:gauge"));
    writer.write_event(Event::Start(root))?;

    // Bibliography
    write_bibliography(&mut writer, &gauge_file.bibliography)?;

    // Version info
    write_version_info(&mut writer, &gauge_file.version_info)?;

    // Embedded images
    for img in &gauge_file.embedded_images {
        write_embedded_image(&mut writer, img)?;
    }

    // Inner gauge element containing the dashComp
    let inner_gauge = BytesStart::new("gauge");
    writer.write_event(Event::Start(inner_gauge))?;

    // Write the gauge as a dashComp
    write_gauge_component(&mut writer, &gauge_file.gauge)?;

    writer.write_event(Event::End(BytesEnd::new("gauge")))?;

    // Close outer gauge
    writer.write_event(Event::End(BytesEnd::new("gauge")))?;

    let result = writer.into_inner();
    Ok(String::from_utf8_lossy(&result).to_string())
}

fn write_bibliography<W: Write>(
    writer: &mut Writer<W>,
    bib: &Bibliography,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new("bibliography");
    elem.push_attribute(("author", bib.author.as_str()));
    elem.push_attribute(("company", bib.company.as_str()));
    elem.push_attribute(("writeDate", bib.write_date.as_str()));
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

fn write_version_info<W: Write>(
    writer: &mut Writer<W>,
    vi: &VersionInfo,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new("versionInfo");
    elem.push_attribute(("fileFormat", vi.file_format.as_str()));
    if let Some(ref sig) = vi.firmware_signature {
        elem.push_attribute(("firmwareSignature", sig.as_str()));
    }
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

fn write_gauge_cluster<W: Write>(
    writer: &mut Writer<W>,
    cluster: &GaugeCluster,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new("gaugeCluster");

    elem.push_attribute((
        "antiAliasing",
        if cluster.anti_aliasing {
            "true"
        } else {
            "false"
        },
    ));
    let bg_color = cluster.cluster_background_color.to_argb_int().to_string();
    elem.push_attribute(("clusterBackgroundColor", bg_color.as_str()));

    if let Some(ref dither) = cluster.background_dither_color {
        let dither_color = dither.to_argb_int().to_string();
        elem.push_attribute(("backgroundDitherColor", dither_color.as_str()));
    }

    if let Some(ref img) = cluster.cluster_background_image_file_name {
        elem.push_attribute(("clusterBackgroundImageFileName", img.as_str()));
    }

    let style = match cluster.cluster_background_image_style {
        BackgroundStyle::Tile => "Tile",
        BackgroundStyle::Stretch => "Stretch",
        BackgroundStyle::Center => "Center",
        BackgroundStyle::Fit => "Fit",
    };
    elem.push_attribute(("clusterBackgroundImageStyle", style));

    writer.write_event(Event::Start(elem))?;

    // Write embedded images
    for img in &cluster.embedded_images {
        write_embedded_image(writer, img)?;
    }

    // Write components
    for component in &cluster.components {
        match component {
            DashComponent::Gauge(gauge) => write_gauge_component(writer, &*gauge)?,
            DashComponent::Indicator(indicator) => write_indicator_component(writer, indicator)?,
        }
    }

    writer.write_event(Event::End(BytesEnd::new("gaugeCluster")))?;
    Ok(())
}

fn write_embedded_image<W: Write>(
    writer: &mut Writer<W>,
    img: &EmbeddedImage,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new("imageFile");
    elem.push_attribute(("fileName", img.file_name.as_str()));
    elem.push_attribute(("imageId", img.image_id.as_str()));

    let type_str = match img.resource_type {
        ResourceType::Png => "png",
        ResourceType::Gif => "gif",
        ResourceType::Ttf => "ttf",
    };
    elem.push_attribute(("type", type_str));

    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(BytesText::new(&img.data)))?;
    writer.write_event(Event::End(BytesEnd::new("imageFile")))?;

    Ok(())
}

fn write_gauge_component<W: Write>(
    writer: &mut Writer<W>,
    gauge: &GaugeConfig,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new("dashComp");
    elem.push_attribute(("type", "Gauge"));
    writer.write_event(Event::Start(elem))?;

    // Write all gauge properties
    write_string_property(writer, "Id", &gauge.id)?;
    write_string_property(writer, "Title", &gauge.title)?;
    write_string_property(writer, "Units", &gauge.units)?;
    write_double_property(writer, "Value", gauge.value)?;
    write_string_property(writer, "OutputChannel", &gauge.output_channel)?;
    write_double_property(writer, "Min", gauge.min)?;
    write_double_property(writer, "Max", gauge.max)?;

    if let Some(ref vp) = gauge.min_vp {
        write_string_property(writer, "MinVP", vp)?;
    }
    if let Some(ref vp) = gauge.max_vp {
        write_string_property(writer, "MaxVP", vp)?;
    }
    if let Some(v) = gauge.default_min {
        write_double_property(writer, "DefaultMin", v)?;
    }
    if let Some(v) = gauge.default_max {
        write_double_property(writer, "DefaultMax", v)?;
    }

    write_boolean_property(writer, "PegLimits", gauge.peg_limits)?;

    if let Some(v) = gauge.low_warning {
        write_double_property(writer, "LowWarning", v)?;
    }
    if let Some(v) = gauge.high_warning {
        write_double_property(writer, "HighWarning", v)?;
    }
    if let Some(v) = gauge.low_critical {
        write_double_property(writer, "LowCritical", v)?;
    }
    if let Some(v) = gauge.high_critical {
        write_double_property(writer, "HighCritical", v)?;
    }

    if let Some(ref vp) = gauge.low_warning_vp {
        write_string_property(writer, "LowWarningVP", vp)?;
    }
    if let Some(ref vp) = gauge.high_warning_vp {
        write_string_property(writer, "HighWarningVP", vp)?;
    }
    if let Some(ref vp) = gauge.low_critical_vp {
        write_string_property(writer, "LowCriticalVP", vp)?;
    }
    if let Some(ref vp) = gauge.high_critical_vp {
        write_string_property(writer, "HighCriticalVP", vp)?;
    }

    write_int_property(writer, "ValueDigits", gauge.value_digits)?;
    write_int_property(writer, "LabelDigits", gauge.label_digits)?;
    write_string_property(writer, "FontFamily", &gauge.font_family)?;
    write_int_property(writer, "FontSizeAdjustment", gauge.font_size_adjustment)?;
    write_boolean_property(writer, "ItalicFont", gauge.italic_font)?;

    write_int_property(writer, "SweepAngle", gauge.sweep_angle)?;
    write_int_property(writer, "StartAngle", gauge.start_angle)?;
    write_int_property(writer, "FaceAngle", gauge.face_angle)?;
    write_int_property(writer, "SweepBeginDegree", gauge.sweep_begin_degree)?;
    write_boolean_property(writer, "CounterClockwise", gauge.counter_clockwise)?;

    write_double_property(writer, "MajorTicks", gauge.major_ticks)?;
    write_double_property(writer, "MinorTicks", gauge.minor_ticks)?;

    // Colors
    write_color_property(writer, "BackColor", &gauge.back_color)?;
    write_color_property(writer, "FontColor", &gauge.font_color)?;
    write_color_property(writer, "TrimColor", &gauge.trim_color)?;
    write_color_property(writer, "WarnColor", &gauge.warn_color)?;
    write_color_property(writer, "CriticalColor", &gauge.critical_color)?;
    write_color_property(writer, "NeedleColor", &gauge.needle_color)?;

    // Position
    write_double_property(writer, "RelativeX", gauge.relative_x)?;
    write_double_property(writer, "RelativeY", gauge.relative_y)?;
    write_double_property(writer, "RelativeWidth", gauge.relative_width)?;
    write_double_property(writer, "RelativeHeight", gauge.relative_height)?;

    write_int_property(writer, "BorderWidth", gauge.border_width)?;
    write_int_property(writer, "ShortestSize", gauge.shortest_size)?;
    write_boolean_property(writer, "ShapeLockedToAspect", gauge.shape_locked_to_aspect)?;
    write_boolean_property(writer, "AntialiasingOn", gauge.antialiasing_on)?;

    if let Some(ref img) = gauge.background_image_file_name {
        write_string_property(writer, "BackgroundImageFileName", img)?;
    } else {
        write_string_property(writer, "BackgroundImageFileName", "null")?;
    }

    if let Some(ref img) = gauge.needle_image_file_name {
        write_string_property(writer, "NeedleImageFileName", img)?;
    } else {
        write_string_property(writer, "NeedleImageFileName", "null")?;
    }

    write_boolean_property(writer, "ShowHistory", gauge.show_history)?;
    write_double_property(writer, "HistoryValue", gauge.history_value)?;
    write_int_property(writer, "HistoryDelay", gauge.history_delay)?;
    write_int_property(writer, "NeedleSmoothing", gauge.needle_smoothing)?;

    if let Some(ref action) = gauge.short_click_action {
        write_string_property(writer, "ShortClickAction", action)?;
    } else {
        write_string_property(writer, "ShortClickAction", "null")?;
    }

    if let Some(ref action) = gauge.long_click_action {
        write_string_property(writer, "LongClickAction", action)?;
    } else {
        write_string_property(writer, "LongClickAction", "null")?;
    }

    write_boolean_property(writer, "DisplayValueAt180", gauge.display_value_at_180)?;
    write_string_property(writer, "GaugeStyle", &gauge.gauge_style)?;
    write_string_property(writer, "GaugePainter", gauge.gauge_painter.to_ts_string())?;
    write_boolean_property(writer, "RunDemo", gauge.run_demo)?;

    writer.write_event(Event::End(BytesEnd::new("dashComp")))?;
    Ok(())
}

fn write_indicator_component<W: Write>(
    writer: &mut Writer<W>,
    indicator: &IndicatorConfig,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new("dashComp");
    elem.push_attribute(("type", "Indicator"));
    writer.write_event(Event::Start(elem))?;

    write_string_property(writer, "Id", &indicator.id)?;
    write_double_property(writer, "Value", indicator.value)?;
    write_string_property(writer, "OutputChannel", &indicator.output_channel)?;
    write_string_property(writer, "OnText", &indicator.on_text)?;
    write_string_property(writer, "OffText", &indicator.off_text)?;

    if let Some(ref img) = indicator.on_image_file_name {
        write_string_property(writer, "OnImageFileName", img)?;
    } else {
        write_string_property(writer, "OnImageFileName", "null")?;
    }

    if let Some(ref img) = indicator.off_image_file_name {
        write_string_property(writer, "OffImageFileName", img)?;
    } else {
        write_string_property(writer, "OffImageFileName", "null")?;
    }

    // Colors
    write_color_property(writer, "OnTextColor", &indicator.on_text_color)?;
    write_color_property(writer, "OffTextColor", &indicator.off_text_color)?;
    write_color_property(writer, "OnBackgroundColor", &indicator.on_background_color)?;
    write_color_property(
        writer,
        "OffBackgroundColor",
        &indicator.off_background_color,
    )?;

    // Position
    write_double_property(writer, "RelativeX", indicator.relative_x)?;
    write_double_property(writer, "RelativeY", indicator.relative_y)?;
    write_double_property(writer, "RelativeWidth", indicator.relative_width)?;
    write_double_property(writer, "RelativeHeight", indicator.relative_height)?;

    write_string_property(writer, "FontFamily", &indicator.font_family)?;
    write_boolean_property(writer, "ItalicFont", indicator.italic_font)?;
    write_boolean_property(writer, "AntialiasingOn", indicator.antialiasing_on)?;

    if let Some(ref action) = indicator.short_click_action {
        write_string_property(writer, "ShortClickAction", action)?;
    } else {
        write_string_property(writer, "ShortClickAction", "null")?;
    }

    if let Some(ref action) = indicator.long_click_action {
        write_string_property(writer, "LongClickAction", action)?;
    } else {
        write_string_property(writer, "LongClickAction", "null")?;
    }

    if let Some(ref name) = indicator.ecu_configuration_name {
        write_string_property(writer, "EcuConfigurationName", name)?;
    }

    write_string_property(
        writer,
        "Painter",
        indicator.indicator_painter.to_ts_string(),
    )?;
    write_boolean_property(writer, "RunDemo", indicator.run_demo)?;

    writer.write_event(Event::End(BytesEnd::new("dashComp")))?;
    Ok(())
}

fn write_string_property<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: &str,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new(name);
    elem.push_attribute(("type", "String"));
    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_double_property<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: f64,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new(name);
    elem.push_attribute(("type", "double"));
    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(BytesText::new(&value.to_string())))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_int_property<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: i32,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new(name);
    elem.push_attribute(("type", "int"));
    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(BytesText::new(&value.to_string())))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_boolean_property<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    value: bool,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new(name);
    elem.push_attribute(("type", "boolean"));
    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(BytesText::new(if value {
        "true"
    } else {
        "false"
    })))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn write_color_property<W: Write>(
    writer: &mut Writer<W>,
    name: &str,
    color: &TsColor,
) -> Result<(), DashWriteError> {
    let mut elem = BytesStart::new(name);
    elem.push_attribute(("type", "java.awt.Color"));
    let alpha = color.alpha.to_string();
    let red = color.red.to_string();
    let green = color.green.to_string();
    let blue = color.blue.to_string();
    elem.push_attribute(("alpha", alpha.as_str()));
    elem.push_attribute(("red", red.as_str()));
    elem.push_attribute(("green", green.as_str()));
    elem.push_attribute(("blue", blue.as_str()));
    writer.write_event(Event::Empty(elem))?;
    Ok(())
}

/// Save a dashboard to a file.
pub fn save_dash_file(dash: &DashFile, path: &std::path::Path) -> Result<(), DashWriteError> {
    let xml = write_dash_file(dash)?;
    std::fs::write(path, xml)?;
    Ok(())
}

/// Save a gauge template to a file.
pub fn save_gauge_file(
    gauge_file: &GaugeFile,
    path: &std::path::Path,
) -> Result<(), DashWriteError> {
    let xml = write_gauge_file(gauge_file)?;
    std::fs::write(path, xml)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_simple_dash() {
        let mut dash = DashFile::default();
        dash.bibliography = Bibliography {
            author: "Test Author".to_string(),
            company: "Test Company".to_string(),
            write_date: "2025-01-01".to_string(),
        };
        dash.version_info = VersionInfo {
            file_format: "3.0".to_string(),
            firmware_signature: Some("speeduino".to_string()),
        };

        let mut gauge = GaugeConfig::default();
        gauge.title = "RPM".to_string();
        gauge.output_channel = "rpm".to_string();
        gauge.min = 0.0;
        gauge.max = 8000.0;
        gauge.gauge_painter = GaugePainter::AnalogGauge;
        gauge.relative_x = 0.1;
        gauge.relative_y = 0.1;
        gauge.relative_width = 0.3;
        gauge.relative_height = 0.4;

        dash.gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(gauge)));

        let result = write_dash_file(&dash);
        assert!(result.is_ok(), "Failed to write: {:?}", result.err());

        let xml = result.unwrap();
        assert!(xml.contains("xmlns=\"http://www.EFIAnalytics.com/:dsh\""));
        assert!(xml.contains("author=\"Test Author\""));
        assert!(xml.contains("<Title type=\"String\">RPM</Title>"));
        assert!(xml.contains("<OutputChannel type=\"String\">rpm</OutputChannel>"));
        assert!(xml.contains("Analog Gauge"));
    }

    #[test]
    fn test_roundtrip() {
        let mut dash = DashFile::default();
        dash.bibliography = Bibliography {
            author: "Roundtrip Test".to_string(),
            company: "LibreTune".to_string(),
            write_date: "2025-01-01".to_string(),
        };

        let mut gauge = GaugeConfig::default();
        gauge.id = "gauge1".to_string();
        gauge.title = "AFR".to_string();
        gauge.output_channel = "afr".to_string();
        gauge.min = 10.0;
        gauge.max = 20.0;
        gauge.gauge_painter = GaugePainter::BasicReadout;
        gauge.back_color = TsColor {
            alpha: 255,
            red: 50,
            green: 50,
            blue: 50,
        };

        dash.gauge_cluster
            .components
            .push(DashComponent::Gauge(Box::new(gauge)));

        // Write to XML
        let xml = write_dash_file(&dash).unwrap();

        // Parse back
        let parsed = super::super::parser::parse_dash_file(&xml).unwrap();

        assert_eq!(parsed.bibliography.author, "Roundtrip Test");
        assert_eq!(parsed.gauge_cluster.components.len(), 1);

        if let DashComponent::Gauge(ref g) = parsed.gauge_cluster.components[0] {
            assert_eq!(g.title, "AFR");
            assert_eq!(g.output_channel, "afr");
            assert_eq!(g.min, 10.0);
            assert_eq!(g.max, 20.0);
            assert_eq!(g.gauge_painter, GaugePainter::BasicReadout);
        } else {
            panic!("Expected Gauge");
        }
    }
}
