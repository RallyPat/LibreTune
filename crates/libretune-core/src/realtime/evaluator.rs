use crate::ini::EcuDefinition;
use std::collections::HashMap;

/// Real-time data evaluator handles computing derived channels
pub struct Evaluator {
    /// Cached list of computed channel names to iterate over
    /// We use a fixed iteration order, but evaluate multiple times to resolve dependencies
    computed_channel_names: Vec<String>,
}

impl Evaluator {
    /// Create a new evaluator for the given ECU definition
    pub fn new(def: &EcuDefinition) -> Self {
        let mut computed = Vec::new();

        for (name, ch) in &def.output_channels {
            if ch.is_computed() {
                computed.push(name.clone());
            }
        }

        // Sort for deterministic behavior
        computed.sort();

        Self {
            computed_channel_names: computed,
        }
    }

    /// Process raw data and produce a full map of output channels
    ///
    /// # Arguments
    /// * `raw_data` - Raw byte buffer from ECU
    /// * `def` - ECU definition
    ///
    /// # Returns
    /// Map of channel name -> value (including both raw and computed)
    pub fn process(&self, raw_data: &[u8], def: &EcuDefinition) -> HashMap<String, f64> {
        // Start with empty values (or base context if we added it, but for now empty)
        let mut values = HashMap::new();

        // 1. Parse all raw (non-computed) channels from data block
        for (name, ch) in &def.output_channels {
            if !ch.is_computed() {
                if let Some(val) = ch.parse(raw_data, def.endianness) {
                    values.insert(name.clone(), val);
                }
            }
        }

        // 2. Evaluate computed channels
        // We do 3 passes to handle dependencies (e.g. Duty depends on PW, Duty% depends on Duty)
        // This is a simple alternative to topological sorting and robust against cycles
        for _ in 0..3 {
            let mut changes = 0;

            for name in &self.computed_channel_names {
                if let Some(ch) = def.output_channels.get(name) {
                    // Evaluate using current values as context
                    // This allows a channel to see values computed earlier in this loop or previous passes
                    if let Some(val) = ch.parse_with_context(raw_data, def.endianness, &values) {
                        // Check if value is new or changed significantly
                        let update = match values.get(name) {
                            None => true,
                            Some(old_val) => (old_val - val).abs() > 1e-7,
                        };

                        if update {
                            values.insert(name.clone(), val);
                            changes += 1;
                        }
                    }
                }
            }

            // If no changes occurred in this pass, we are stable
            if changes == 0 {
                break;
            }
        }

        values
    }
}
