//! Bridge between plugin ControllerAccess calls and LibreTune ECU layer
//!
//! This module translates TunerStudio plugin API calls to LibreTune's
//! existing ECU communication infrastructure.

use super::protocol::{ControllerParameterData, OutputChannelData};
use crate::ini::{DataType, EcuDefinition, Shape};
use crate::tune::{TuneFile, TuneValue};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Bridge providing ECU data to plugins
pub struct ControllerBridge {
    /// Current ECU definition
    definition: Arc<RwLock<Option<EcuDefinition>>>,
    /// Current tune data
    tune: Arc<RwLock<Option<TuneFile>>>,
    /// Cached realtime data (updated by realtime stream)
    realtime_cache: Arc<RwLock<HashMap<String, f64>>>,
    /// Output channel subscriptions: channel_name -> list of subscriber IDs
    output_subscriptions: RwLock<HashMap<String, Vec<String>>>,
    /// Parameter subscriptions: param_name -> list of subscriber IDs
    param_subscriptions: RwLock<HashMap<String, Vec<String>>>,
}

impl ControllerBridge {
    pub fn new(
        definition: Arc<RwLock<Option<EcuDefinition>>>,
        tune: Arc<RwLock<Option<TuneFile>>>,
    ) -> Self {
        Self {
            definition,
            tune,
            realtime_cache: Arc::new(RwLock::new(HashMap::new())),
            output_subscriptions: RwLock::new(HashMap::new()),
            param_subscriptions: RwLock::new(HashMap::new()),
        }
    }

    /// Update realtime data cache (called from realtime stream)
    pub fn update_realtime(&self, data: HashMap<String, f64>) {
        if let Ok(mut cache) = self.realtime_cache.write() {
            *cache = data;
        }
    }

    /// Get list of all output channel names
    pub fn get_output_channel_names(&self) -> Vec<String> {
        let def_guard = self.definition.read().ok();
        let def = match def_guard.as_ref().and_then(|g| g.as_ref()) {
            Some(d) => d,
            None => return vec![],
        };

        def.output_channels.keys().cloned().collect()
    }

    /// Get output channel data by name
    pub fn get_output_channel(&self, name: &str) -> Option<OutputChannelData> {
        let def_guard = self.definition.read().ok();
        let def = def_guard.as_ref().and_then(|g| g.as_ref())?;

        let channel = def.output_channels.get(name)?;
        let value = self
            .realtime_cache
            .read()
            .ok()?
            .get(name)
            .copied()
            .unwrap_or(0.0);

        Some(OutputChannelData {
            name: name.to_string(),
            value,
            units: channel.units.clone(),
            // OutputChannel doesn't have min/max, use sensible defaults
            min_value: 0.0,
            max_value: 100.0,
        })
    }

    /// Subscribe to output channel updates
    pub fn subscribe_output(&self, channel_name: &str, subscriber_id: &str) {
        if let Ok(mut subs) = self.output_subscriptions.write() {
            subs.entry(channel_name.to_string())
                .or_default()
                .push(subscriber_id.to_string());
        }
    }

    /// Unsubscribe from output channel
    pub fn unsubscribe_output(&self, subscriber_id: &str) {
        if let Ok(mut subs) = self.output_subscriptions.write() {
            for subscribers in subs.values_mut() {
                subscribers.retain(|s| s != subscriber_id);
            }
        }
    }

    /// Get list of all parameter names
    pub fn get_parameter_names(&self) -> Vec<String> {
        let def_guard = self.definition.read().ok();
        let def = match def_guard.as_ref().and_then(|g| g.as_ref()) {
            Some(d) => d,
            None => return vec![],
        };

        def.constants.keys().cloned().collect()
    }

    /// Get controller parameter data by name
    pub fn get_parameter(&self, name: &str) -> Option<ControllerParameterData> {
        let def_guard = self.definition.read().ok();
        let def = def_guard.as_ref().and_then(|g| g.as_ref())?;

        let constant = def.constants.get(name)?;
        let tune_guard = self.tune.read().ok();
        let tune = tune_guard.as_ref().and_then(|g| g.as_ref());

        // Determine parameter class based on data type and shape
        let param_class = if constant.data_type == DataType::Bits {
            "bits"
        } else {
            match &constant.shape {
                Shape::Scalar => "scalar",
                Shape::Array1D(_) | Shape::Array2D { .. } => "array",
            }
        };

        // Get value from tune
        let scalar_value = if param_class == "scalar" {
            tune.and_then(|t| {
                t.get_value(name).and_then(|v| match v {
                    TuneValue::Scalar(val) => Some(*val),
                    _ => None,
                })
            })
        } else {
            None
        };

        let array_values = if param_class == "array" {
            tune.and_then(|t| {
                t.get_value(name).and_then(|v| match v {
                    TuneValue::Array(arr) => {
                        // Convert 1D array to 2D for API compatibility
                        Some(vec![arr.clone()])
                    }
                    _ => None,
                })
            })
        } else {
            None
        };

        let string_value = if param_class == "bits" {
            tune.and_then(|t| {
                t.get_value(name).and_then(|v| match v {
                    TuneValue::String(s) => Some(s.clone()),
                    TuneValue::Scalar(val) => {
                        // Bits fields might be stored as scalar (index)
                        let idx = *val as usize;
                        constant.bit_options.get(idx).cloned()
                    }
                    _ => None,
                })
            })
        } else {
            None
        };

        // Get shape dimensions
        let shape = match &constant.shape {
            Shape::Array1D(len) => Some([*len as i32, 1]),
            Shape::Array2D { rows, cols } => Some([*rows as i32, *cols as i32]),
            Shape::Scalar => None,
        };

        Some(ControllerParameterData {
            name: name.to_string(),
            param_class: param_class.to_string(),
            units: constant.units.clone(),
            min: constant.min,
            max: constant.max,
            decimal_places: constant.digits as i32,
            scalar_value,
            array_values,
            string_value,
            options: if param_class == "bits" {
                Some(constant.bit_options.clone())
            } else {
                None
            },
            shape,
        })
    }

    /// Update a scalar parameter value
    pub fn update_scalar(&self, name: &str, value: f64) -> Result<(), String> {
        let mut tune_guard = self.tune.write().map_err(|e| e.to_string())?;
        let tune = tune_guard
            .as_mut()
            .ok_or_else(|| "No tune loaded".to_string())?;

        tune.set_constant(name, TuneValue::Scalar(value));
        Ok(())
    }

    /// Update an array parameter value
    pub fn update_array(&self, name: &str, values: Vec<Vec<f64>>) -> Result<(), String> {
        let mut tune_guard = self.tune.write().map_err(|e| e.to_string())?;
        let tune = tune_guard
            .as_mut()
            .ok_or_else(|| "No tune loaded".to_string())?;

        // Flatten 2D to 1D for storage
        let flat: Vec<f64> = values.into_iter().flatten().collect();
        tune.set_constant(name, TuneValue::Array(flat));
        Ok(())
    }

    /// Update a bits/string parameter value
    pub fn update_string(&self, name: &str, value: &str) -> Result<(), String> {
        let mut tune_guard = self.tune.write().map_err(|e| e.to_string())?;
        let tune = tune_guard
            .as_mut()
            .ok_or_else(|| "No tune loaded".to_string())?;

        tune.set_constant(name, TuneValue::String(value.to_string()));
        Ok(())
    }

    /// Subscribe to parameter changes
    pub fn subscribe_parameter(&self, param_name: &str, subscriber_id: &str) {
        if let Ok(mut subs) = self.param_subscriptions.write() {
            subs.entry(param_name.to_string())
                .or_default()
                .push(subscriber_id.to_string());
        }
    }

    /// Unsubscribe from parameter changes
    pub fn unsubscribe_parameter(&self, subscriber_id: &str) {
        if let Ok(mut subs) = self.param_subscriptions.write() {
            for subscribers in subs.values_mut() {
                subscribers.retain(|s| s != subscriber_id);
            }
        }
    }

    /// Get subscribers for an output channel (for notification dispatch)
    pub fn get_output_subscribers(&self, channel_name: &str) -> Vec<String> {
        self.output_subscriptions
            .read()
            .ok()
            .and_then(|subs| subs.get(channel_name).cloned())
            .unwrap_or_default()
    }

    /// Get subscribers for a parameter (for notification dispatch)
    pub fn get_parameter_subscribers(&self, param_name: &str) -> Vec<String> {
        self.param_subscriptions
            .read()
            .ok()
            .and_then(|subs| subs.get(param_name).cloned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let def = Arc::new(RwLock::new(None));
        let tune = Arc::new(RwLock::new(None));
        let bridge = ControllerBridge::new(def, tune);
        
        assert!(bridge.get_output_channel_names().is_empty());
        assert!(bridge.get_parameter_names().is_empty());
    }

    #[test]
    fn test_realtime_cache() {
        let def = Arc::new(RwLock::new(None));
        let tune = Arc::new(RwLock::new(None));
        let bridge = ControllerBridge::new(def, tune);
        
        let mut data = HashMap::new();
        data.insert("rpm".to_string(), 3500.0);
        data.insert("afr".to_string(), 14.7);
        
        bridge.update_realtime(data);
        
        let cache = bridge.realtime_cache.read().unwrap();
        assert_eq!(cache.get("rpm"), Some(&3500.0));
        assert_eq!(cache.get("afr"), Some(&14.7));
    }

    #[test]
    fn test_subscriptions() {
        let def = Arc::new(RwLock::new(None));
        let tune = Arc::new(RwLock::new(None));
        let bridge = ControllerBridge::new(def, tune);
        
        bridge.subscribe_output("rpm", "plugin1");
        bridge.subscribe_output("rpm", "plugin2");
        bridge.subscribe_output("afr", "plugin1");
        
        let rpm_subs = bridge.get_output_subscribers("rpm");
        assert_eq!(rpm_subs.len(), 2);
        
        bridge.unsubscribe_output("plugin1");
        
        let rpm_subs = bridge.get_output_subscribers("rpm");
        assert_eq!(rpm_subs.len(), 1);
        assert_eq!(rpm_subs[0], "plugin2");
    }
}
