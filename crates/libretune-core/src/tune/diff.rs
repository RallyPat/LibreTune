//! Tune comparison / diff
//!
//! Compare two tune files to find differences.

use super::file::{TuneFile, TuneValue};

/// A difference between two tune files
#[derive(Debug, Clone)]
pub struct TuneDifference {
    /// Name of the constant
    pub name: String,
    /// Value in the first tune (None if not present)
    pub value_a: Option<TuneValue>,
    /// Value in the second tune (None if not present)
    pub value_b: Option<TuneValue>,
}

/// Result of comparing two tune files
pub struct TuneDiff {
    /// Differences found
    pub differences: Vec<TuneDifference>,
    /// Whether the signatures match
    pub signature_match: bool,
}

impl TuneDiff {
    /// Compare two tune files
    pub fn compare(tune_a: &TuneFile, tune_b: &TuneFile) -> Self {
        let mut differences = Vec::new();
        
        // Check signature
        let signature_match = tune_a.signature == tune_b.signature;
        
        // Find all unique constant names
        let mut all_names: Vec<&String> = tune_a.constants.keys()
            .chain(tune_b.constants.keys())
            .collect();
        all_names.sort();
        all_names.dedup();
        
        // Compare each constant
        for name in all_names {
            let value_a = tune_a.constants.get(name);
            let value_b = tune_b.constants.get(name);
            
            let is_different = match (value_a, value_b) {
                (None, None) => false,
                (Some(_), None) | (None, Some(_)) => true,
                (Some(a), Some(b)) => !values_equal(a, b),
            };
            
            if is_different {
                differences.push(TuneDifference {
                    name: name.clone(),
                    value_a: value_a.cloned(),
                    value_b: value_b.cloned(),
                });
            }
        }
        
        Self {
            differences,
            signature_match,
        }
    }
    
    /// Check if the tunes are identical
    pub fn is_identical(&self) -> bool {
        self.differences.is_empty() && self.signature_match
    }
    
    /// Get the number of differences
    pub fn difference_count(&self) -> usize {
        self.differences.len()
    }
}

/// Check if two TuneValues are equal
fn values_equal(a: &TuneValue, b: &TuneValue) -> bool {
    match (a, b) {
        (TuneValue::Scalar(x), TuneValue::Scalar(y)) => (x - y).abs() < 1e-9,
        (TuneValue::Bool(x), TuneValue::Bool(y)) => x == y,
        (TuneValue::String(x), TuneValue::String(y)) => x == y,
        (TuneValue::Array(x), TuneValue::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b): (&f64, &f64)| (a - b).abs() < 1e-9)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_diff_identical() {
        let tune_a = TuneFile::new("test");
        let tune_b = TuneFile::new("test");
        
        let diff = TuneDiff::compare(&tune_a, &tune_b);
        assert!(diff.is_identical());
    }
    
    #[test]
    fn test_diff_different() {
        let mut tune_a = TuneFile::new("test");
        let mut tune_b = TuneFile::new("test");
        
        tune_a.set_constant("reqFuel", TuneValue::Scalar(10.0));
        tune_b.set_constant("reqFuel", TuneValue::Scalar(12.0));
        
        let diff = TuneDiff::compare(&tune_a, &tune_b);
        assert!(!diff.is_identical());
        assert_eq!(diff.difference_count(), 1);
    }
}
