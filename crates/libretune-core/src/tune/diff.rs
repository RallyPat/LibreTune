//! Tune comparison / diff
//!
//! Compare two tune files to find differences.

use super::file::{TuneFile, TuneValue};
use serde::Serialize;

/// A difference between two tune files
#[derive(Debug, Clone, Serialize)]
pub struct TuneDifference {
    /// Name of the constant
    pub name: String,
    /// Value in the first tune (None if not present)
    pub value_a: Option<TuneValue>,
    /// Value in the second tune (None if not present)
    pub value_b: Option<TuneValue>,
    /// Numeric difference (for scalar values)
    pub numeric_diff: Option<f64>,
    /// Percentage difference (for scalar values)
    pub percent_diff: Option<f64>,
}

/// Result of comparing two tune files
#[derive(Debug, Serialize)]
pub struct TuneDiff {
    /// Differences found
    pub differences: Vec<TuneDifference>,
    /// Whether the signatures match
    pub signature_match: bool,
    /// Signature of tune A
    pub signature_a: String,
    /// Signature of tune B
    pub signature_b: String,
    /// Total constants in tune A
    pub total_constants_a: usize,
    /// Total constants in tune B
    pub total_constants_b: usize,
}

impl TuneDiff {
    /// Compare two tune files
    pub fn compare(tune_a: &TuneFile, tune_b: &TuneFile) -> Self {
        let mut differences = Vec::new();

        // Check signature
        let signature_match = tune_a.signature == tune_b.signature;

        // Find all unique constant names
        let mut all_names: Vec<&String> = tune_a
            .constants
            .keys()
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
                // Compute numeric diffs for scalars
                let (numeric_diff, percent_diff) = match (value_a, value_b) {
                    (Some(TuneValue::Scalar(a)), Some(TuneValue::Scalar(b))) => {
                        let diff = b - a;
                        let pct = if a.abs() > 1e-9 {
                            Some((diff / a) * 100.0)
                        } else {
                            None
                        };
                        (Some(diff), pct)
                    }
                    _ => (None, None),
                };

                differences.push(TuneDifference {
                    name: name.clone(),
                    value_a: value_a.cloned(),
                    value_b: value_b.cloned(),
                    numeric_diff,
                    percent_diff,
                });
            }
        }

        Self {
            differences,
            signature_match,
            signature_a: tune_a.signature.clone(),
            signature_b: tune_b.signature.clone(),
            total_constants_a: tune_a.constants.len(),
            total_constants_b: tune_b.constants.len(),
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

    /// Apply selected changes from source tune to target tune (cherry-pick merge)
    /// `constant_names` is the list of constants to apply from the "B" side
    /// Returns the number of constants merged
    pub fn merge_selected(
        target: &mut TuneFile,
        source: &TuneFile,
        constant_names: &[String],
    ) -> usize {
        let mut merged = 0;
        for name in constant_names {
            if let Some(value) = source.constants.get(name) {
                target.set_constant(name.clone(), value.clone());
                // Preserve the page mapping from source if available
                if let Some(page) = source.constant_pages.get(name) {
                    target.constant_pages.insert(name.clone(), *page);
                }
                merged += 1;
            }
        }
        if merged > 0 {
            target.touch();
        }
        merged
    }
}

/// Check if two TuneValues are equal
fn values_equal(a: &TuneValue, b: &TuneValue) -> bool {
    match (a, b) {
        (TuneValue::Scalar(x), TuneValue::Scalar(y)) => (x - y).abs() < 1e-9,
        (TuneValue::Bool(x), TuneValue::Bool(y)) => x == y,
        (TuneValue::String(x), TuneValue::String(y)) => x == y,
        (TuneValue::Array(x), TuneValue::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(a, b): (&f64, &f64)| (a - b).abs() < 1e-9)
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
        // Should have numeric diff
        assert!((diff.differences[0].numeric_diff.unwrap() - 2.0).abs() < 1e-9);
        assert!((diff.differences[0].percent_diff.unwrap() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn test_merge_selected() {
        let mut target = TuneFile::new("test");
        let mut source = TuneFile::new("test");

        target.set_constant("reqFuel", TuneValue::Scalar(10.0));
        target.set_constant("advance", TuneValue::Scalar(15.0));
        source.set_constant("reqFuel", TuneValue::Scalar(12.0));
        source.set_constant("advance", TuneValue::Scalar(20.0));
        source.set_constant("newConst", TuneValue::Scalar(5.0));

        // Only merge reqFuel and newConst, leave advance alone
        let merged = TuneDiff::merge_selected(
            &mut target,
            &source,
            &["reqFuel".to_string(), "newConst".to_string()],
        );
        assert_eq!(merged, 2);

        // reqFuel should be updated
        if let Some(TuneValue::Scalar(v)) = target.get_constant("reqFuel") {
            assert!((v - 12.0).abs() < 1e-9);
        } else {
            panic!("reqFuel not found");
        }

        // advance should remain unchanged
        if let Some(TuneValue::Scalar(v)) = target.get_constant("advance") {
            assert!((v - 15.0).abs() < 1e-9);
        } else {
            panic!("advance not found");
        }

        // newConst should be added
        assert!(target.get_constant("newConst").is_some());
    }
}
