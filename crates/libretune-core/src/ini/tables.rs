//! Table editor definitions parser
//!
//! Parses [TableEditor] sections which define 2D/3D table editing interfaces.

use serde::{Deserialize, Serialize};

/// A table editor definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDefinition {
    /// Table name/identifier
    pub name: String,
    
    /// Map name used in menu references (from table = tableName, mapName, ...)
    /// Menus reference tables by this map_name, not the name field
    pub map_name: Option<String>,
    
    /// Display title
    pub title: String,
    
    /// Table type (2D or 3D)
    pub table_type: TableType,
    
    /// Main map/data constant name
    pub map: String,
    
    /// X-axis constant name (bins)
    pub x_bins: String,
    
    /// X-axis output channel for highlighting
    pub x_output_channel: Option<String>,
    
    /// Y-axis constant name (bins) - only for 3D tables
    pub y_bins: Option<String>,
    
    /// Y-axis output channel for highlighting - only for 3D tables
    pub y_output_channel: Option<String>,
    
    /// Page number for the table data
    pub page: u8,
    
    /// Number of columns
    pub x_size: usize,
    
    /// Number of rows (1 for 2D tables)
    pub y_size: usize,
    
    /// Up-from color (high values)
    pub up_color: Option<String>,
    
    /// Down-from color (low values)
    pub down_color: Option<String>,
    
    /// Grid height for display
    pub grid_height: Option<f32>,
    
    /// Grid orientation
    pub grid_orient: Option<u8>,
    
    /// Help text
    pub help: Option<String>,
    
    /// X-axis label (from xyLabels)
    pub x_label: Option<String>,
    
    /// Y-axis label (from xyLabels)
    pub y_label: Option<String>,
}

/// Type of table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableType {
    /// 2D table (one axis)
    TwoD,
    /// 3D table (two axes)
    ThreeD,
}

impl TableDefinition {
    /// Create a new 2D table definition
    pub fn new_2d(name: impl Into<String>, map: impl Into<String>, x_bins: impl Into<String>, x_size: usize) -> Self {
        Self {
            name: name.into(),
            map_name: None,
            title: String::new(),
            table_type: TableType::TwoD,
            map: map.into(),
            x_bins: x_bins.into(),
            x_output_channel: None,
            y_bins: None,
            y_output_channel: None,
            page: 0,
            x_size,
            y_size: 1,
            up_color: None,
            down_color: None,
            grid_height: None,
            grid_orient: None,
            help: None,
            x_label: None,
            y_label: None,
        }
    }
    
    /// Create a new 3D table definition
    pub fn new_3d(
        name: impl Into<String>,
        map: impl Into<String>,
        x_bins: impl Into<String>,
        y_bins: impl Into<String>,
        x_size: usize,
        y_size: usize,
    ) -> Self {
        Self {
            name: name.into(),
            map_name: None,
            title: String::new(),
            table_type: TableType::ThreeD,
            map: map.into(),
            x_bins: x_bins.into(),
            x_output_channel: None,
            y_bins: Some(y_bins.into()),
            y_output_channel: None,
            page: 0,
            x_size,
            y_size,
            up_color: None,
            down_color: None,
            grid_height: None,
            grid_orient: None,
            help: None,
            x_label: None,
            y_label: None,
        }
    }
    
    /// Check if this is a 3D table
    pub fn is_3d(&self) -> bool {
        self.table_type == TableType::ThreeD
    }
    
    /// Total number of cells in the table
    pub fn cell_count(&self) -> usize {
        self.x_size * self.y_size
    }
}

impl Default for TableDefinition {
    fn default() -> Self {
        Self::new_2d("", "", "", 0)
    }
}

/// A 2D curve editor definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveDefinition {
    /// Curve name/identifier
    pub name: String,
    
    /// Display title
    pub title: String,
    
    /// X-axis constant name (bins)
    pub x_bins: String,
    
    /// X-axis output channel for highlighting
    pub x_output_channel: Option<String>,
    
    /// Y-axis constant name (values)
    pub y_bins: String,
    
    /// Column labels (X label, Y label)
    pub column_labels: (String, String),
    
    /// X-axis range and step (min, max, step)
    pub x_axis: Option<(f32, f32, f32)>,
    
    /// Y-axis range and step (min, max, step)
    pub y_axis: Option<(f32, f32, f32)>,
    
    /// Size (width, height) - number of points
    pub size: Option<usize>,
    
    /// Page number for the curve data
    pub page: u8,
    
    /// Help text
    pub help: Option<String>,
}

impl CurveDefinition {
    /// Create a new curve definition
    pub fn new(name: impl Into<String>, x_bins: impl Into<String>, y_bins: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: String::new(),
            x_bins: x_bins.into(),
            x_output_channel: None,
            y_bins: y_bins.into(),
            column_labels: (String::new(), String::new()),
            x_axis: None,
            y_axis: None,
            size: None,
            page: 0,
            help: None,
        }
    }
}

impl Default for CurveDefinition {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_2d_table() {
        let table = TableDefinition::new_2d("cranking", "crankingTable", "crankingBins", 10);
        assert!(!table.is_3d());
        assert_eq!(table.cell_count(), 10);
    }
    
    #[test]
    fn test_3d_table() {
        let table = TableDefinition::new_3d("veTable1", "veTable1Map", "rpmBins", "fuelLoadBins", 16, 16);
        assert!(table.is_3d());
        assert_eq!(table.cell_count(), 256);
    }
}
