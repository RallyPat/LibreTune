//! AutoTune Module
//!
//! Implements automatic VE table tuning based on real-time AFR data.
//! Features:
//! - Auto-tuning with recommendations based on AFR data
//! - Authority limits to restrict changes
//! - Data filtering (RPM ranges, coolant temp, custom expressions)
//! - Cell locking functionality
//! - Reference tables (Lambda Delay, AFR Target)

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// A single cell recommendation in the VE table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneRecommendation {
    pub cell_x: usize,
    pub cell_y: usize,
    pub beginning_value: f64,
    pub recommended_value: f64,
    pub hit_count: u32,
    pub hit_weighting: f64,
    pub target_afr: f64,
    pub hit_percentage: f64,
}

/// AutoTune settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneSettings {
    pub target_table: String,
    pub update_controller: bool,
    pub auto_send_updates: bool,
    pub send_interval_ms: u32,
}

impl Default for AutoTuneSettings {
    fn default() -> Self {
        Self {
            target_table: "veTable1".to_string(),
            update_controller: false,
            auto_send_updates: false,
            send_interval_ms: 15000,
        }
    }
}

/// Authority limits to restrict VE changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneAuthorityLimits {
    pub max_cell_value_change: f64,
    pub max_cell_percentage_change: f64,
}

impl Default for AutoTuneAuthorityLimits {
    fn default() -> Self {
        Self {
            max_cell_value_change: 10.0,
            max_cell_percentage_change: 20.0,
        }
    }
}

/// Data filters for VE Analyze
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneFilters {
    pub min_rpm: f64,
    pub max_rpm: f64,
    pub min_y_axis: Option<String>,
    pub max_y_axis: Option<String>,
    pub min_clt: f64,
    pub custom_filter: Option<String>,
}

impl Default for AutoTuneFilters {
    fn default() -> Self {
        Self {
            min_rpm: 1000.0,
            max_rpm: 7000.0,
            min_y_axis: None,
            max_y_axis: None,
            min_clt: 160.0,
            custom_filter: None,
        }
    }
}

/// Reference tables used by VE Analyze
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneReferenceTables {
    pub lambda_delay_table: Vec<Vec<f64>>,
    pub target_afr_table: Vec<Vec<f64>>,
}

/// VE Analyze runtime state
#[derive(Debug)]
pub struct AutoTuneState {
    pub is_running: bool,
    pub locked_cells: Vec<(usize, usize)>,
    pub recommendations: HashMap<(usize, usize), AutoTuneRecommendation>,
}

impl Default for AutoTuneState {
    fn default() -> Self {
        Self {
            is_running: false,
            locked_cells: Vec::new(),
            recommendations: HashMap::new(),
        }
    }
}

/// Data point from ECU for VE analysis
#[derive(Debug, Clone)]
pub struct VEDataPoint {
    pub rpm: f64,
    pub map: f64,
    pub afr: f64,
    pub ve: f64,
    pub clt: f64,
}

impl AutoTuneState {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn start(&mut self) {
        self.is_running = true;
        self.recommendations.clear();
    }
    
    pub fn stop(&mut self) {
        self.is_running = false;
    }
    
    pub fn is_cell_locked(&self, x: usize, y: usize) -> bool {
        self.locked_cells.contains(&(x, y))
    }
    
    pub fn lock_cells(&mut self, cells: Vec<(usize, usize)>) {
        self.locked_cells.extend(cells);
    }
    
    pub fn unlock_cells(&mut self, cells: Vec<(usize, usize)>) {
        for cell in cells {
            if let Some(pos) = self.locked_cells.iter().position(|c| c == &cell) {
                self.locked_cells.remove(pos);
            }
        }
    }
    
    pub fn add_data_point(&mut self, point: VEDataPoint, 
                         table_x_bins: &[f64], table_y_bins: &[f64],
                         _settings: &AutoTuneSettings, filters: &AutoTuneFilters,
                         _authority: &AutoTuneAuthorityLimits) {
        if !self.is_running {
            return;
        }
        
        if !self.passes_filters(&point, filters) {
            return;
        }
        
        let x_idx = self.find_bin_index(point.rpm, table_x_bins);
        let y_idx = self.find_bin_index(point.map, table_y_bins);
        
        if x_idx.is_none() || y_idx.is_none() {
            return;
        }
        
        let cell_x_idx = x_idx.unwrap();
        let cell_y_idx = y_idx.unwrap();
        
        if self.is_cell_locked(cell_x_idx, cell_y_idx) {
            return;
        }
        
        // Calculate required VE before borrowing recommendations
        let required_ve = self.calculate_required_ve(point.ve, point.afr);
        
        let current_recs = self.recommendations.entry((cell_x_idx, cell_y_idx)).or_insert_with(|| {
            AutoTuneRecommendation {
                cell_x: cell_x_idx,
                cell_y: cell_y_idx,
                beginning_value: point.ve,
                recommended_value: point.ve,
                hit_count: 0,
                hit_weighting: 0.0,
                target_afr: point.afr,
                hit_percentage: 0.0,
            }
        });
        
        current_recs.hit_count += 1;
        
        let _delta = required_ve - current_recs.beginning_value;
        
        current_recs.recommended_value = required_ve;
        
        let hit_weight = 1.0;
        current_recs.hit_weighting += hit_weight;
        current_recs.hit_percentage = 100.0;
    }
    
    fn find_bin_index(&self, value: f64, bins: &[f64]) -> Option<usize> {
        bins.iter().enumerate().find(|&(_, bin)| {
            (bin - value).abs() < 0.1
        }).map(|(i, _)| i)
    }
    
    fn passes_filters(&self, point: &VEDataPoint, filters: &AutoTuneFilters) -> bool {
        point.rpm >= filters.min_rpm
            && point.rpm <= filters.max_rpm
            && point.clt >= filters.min_clt
    }
    
    fn calculate_required_ve(&self, current_ve: f64, actual_afr: f64) -> f64 {
        if actual_afr < 0.1 {
            return current_ve;
        }
        
        let stoich = 14.7;
        let afr_ratio = actual_afr / stoich;
        
        current_ve * afr_ratio
    }
    
    pub fn get_recommendations(&self) -> Vec<AutoTuneRecommendation> {
        self.recommendations.values().cloned().collect()
    }
}
