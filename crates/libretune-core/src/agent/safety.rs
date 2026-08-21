//! Authority-limit clamping for proposed tune changes.
//!
//! Reuses [`crate::autotune::AutoTuneAuthorityLimits`] so that LLM-proposed
//! VE/table edits are bounded by exactly the same envelope the algorithmic
//! AutoTune engine uses. A proposal that exceeds the limit is *clamped* (not
//! rejected) and flagged so the UI can show "clamped from X to Y".

use crate::action_scripting::Action;
use crate::autotune::AutoTuneAuthorityLimits;
use serde::{Deserialize, Serialize};

/// Result of clamping a single [`Action::TableEdit`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClampResult {
    /// The (possibly modified) action after clamping.
    pub action: Action,
    /// Original requested value, if it was clamped.
    pub clamped_from: Option<f64>,
    /// The limit that was hit, if any.
    pub reason: Option<String>,
}

impl ClampResult {
    fn unchanged(action: Action) -> Self {
        Self {
            action,
            clamped_from: None,
            reason: None,
        }
    }
}

/// Clamp a single [`Action::TableEdit`] to the per-cell and percentage-change
/// limits. Other action variants pass through unchanged.
///
/// `current_value` is the cell's value *before* the proposed edit; required to
/// enforce `max_cell_percentage_change`. If `None`, only the absolute per-cell
/// limit is enforced.
pub fn clamp_table_edit(
    action: Action,
    limits: &AutoTuneAuthorityLimits,
    current_value: Option<f64>,
) -> ClampResult {
    let Action::TableEdit {
        ref table_name,
        x_index,
        y_index,
        new_value,
        old_value,
    } = action
    else {
        return ClampResult::unchanged(action);
    };

    let mut clamped = new_value;
    let mut clamped_from = None;
    let mut reason = None;

    // 1. Absolute per-cell change limit.
    if let Some(cur) = current_value {
        let delta = (new_value - cur).abs();
        if delta > limits.max_cell_value_change {
            let sign = if new_value >= cur { 1.0 } else { -1.0 };
            clamped = cur + sign * limits.max_cell_value_change;
            clamped_from = Some(new_value);
            reason = Some(format!(
                "per-cell change {:.2} exceeds limit {:.2}",
                delta, limits.max_cell_value_change
            ));
        }
    }

    // 2. Percentage change limit (relative to current value).
    if let Some(cur) = current_value {
        if cur.abs() > f64::MIN_POSITIVE {
            let pct = ((clamped - cur).abs() / cur.abs()) * 100.0;
            if pct > limits.max_cell_percentage_change {
                let sign = if clamped >= cur { 1.0 } else { -1.0 };
                let max_abs = cur.abs() * (limits.max_cell_percentage_change / 100.0);
                clamped = cur + sign * max_abs;
                if clamped_from.is_none() {
                    clamped_from = Some(new_value);
                }
                reason = Some(format!(
                    "percentage change {:.0}% exceeds limit {:.0}%",
                    pct, limits.max_cell_percentage_change
                ));
            }
        }
    }

    // 3. Absolute rails, last so nothing above can escape them. The two limits
    // above are both relative to `current_value`, so they bound one edit
    // against wherever the table already sits and cannot stop a series of
    // edits from walking a cell anywhere at all.
    let railed = limits.clamp_to_rails(clamped);
    if railed != clamped {
        reason = Some(format!(
            "value {:.2} outside the allowed range {:.2}..={:.2}",
            clamped,
            limits.min_cell_value.min(limits.max_cell_value),
            limits.min_cell_value.max(limits.max_cell_value)
        ));
        if clamped_from.is_none() {
            clamped_from = Some(new_value);
        }
        clamped = railed;
    }

    if clamped_from.is_some() {
        ClampResult {
            action: Action::TableEdit {
                table_name: table_name.clone(),
                x_index,
                y_index,
                new_value: clamped,
                old_value,
            },
            clamped_from,
            reason,
        }
    } else {
        ClampResult::unchanged(Action::TableEdit {
            table_name: table_name.clone(),
            x_index,
            y_index,
            new_value,
            old_value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_scripting::Action;
    use crate::autotune::AutoTuneAuthorityLimits;

    fn limits() -> AutoTuneAuthorityLimits {
        AutoTuneAuthorityLimits {
            max_cell_value_change: 5.0,
            max_cell_percentage_change: 10.0,
            ..Default::default()
        }
    }

    fn edit(new_value: f64) -> Action {
        Action::TableEdit {
            table_name: "veTable1".into(),
            x_index: 0,
            y_index: 0,
            new_value,
            old_value: Some(50.0),
        }
    }

    #[test]
    fn within_limits_unchanged() {
        let r = clamp_table_edit(edit(53.0), &limits(), Some(50.0));
        assert!(r.clamped_from.is_none());
    }

    #[test]
    fn absolute_limit_clamps() {
        // 50 -> 60 is +10, limit is 5.
        let r = clamp_table_edit(edit(60.0), &limits(), Some(50.0));
        assert_eq!(r.clamped_from, Some(60.0));
        match r.action {
            Action::TableEdit { new_value, .. } => {
                assert!((new_value - 55.0).abs() < 1e-9);
            }
            _ => panic!("expected TableEdit"),
        }
    }

    #[test]
    fn percentage_limit_clamps() {
        // Large absolute limit (100) so only the percentage path (10%) fires.
        // 100 -> 120 is +20%, clamped to 110 (+10%).
        let lim = AutoTuneAuthorityLimits {
            max_cell_value_change: 100.0,
            max_cell_percentage_change: 10.0,
            ..Default::default()
        };
        let r = clamp_table_edit(
            Action::TableEdit {
                table_name: "veTable1".into(),
                x_index: 0,
                y_index: 0,
                new_value: 120.0,
                old_value: Some(100.0),
            },
            &lim,
            Some(100.0),
        );
        assert!(
            r.reason.as_deref().unwrap().contains("percentage"),
            "got: {:?}",
            r.reason
        );
        match r.action {
            Action::TableEdit { new_value, .. } => {
                assert!((new_value - 110.0).abs() < 1e-9, "got {new_value}");
            }
            _ => panic!("expected TableEdit"),
        }
    }

    #[test]
    fn non_tableedit_passes_through() {
        let action = Action::Pause { duration_ms: 100 };
        let r = clamp_table_edit(action, &limits(), None);
        assert!(r.clamped_from.is_none());
    }
}

#[cfg(test)]
mod rail_tests {
    use super::*;
    use crate::action_scripting::Action;
    use crate::autotune::AutoTuneAuthorityLimits;

    fn railed(lo: f64, hi: f64) -> AutoTuneAuthorityLimits {
        AutoTuneAuthorityLimits {
            // Wide relative limits so only the rails can bite.
            max_cell_value_change: 1000.0,
            max_cell_percentage_change: 1000.0,
            min_cell_value: lo,
            max_cell_value: hi,
        }
    }

    fn edit(new_value: f64) -> Action {
        Action::TableEdit {
            table_name: "veTable1Tbl".to_string(),
            x_index: 0,
            y_index: 0,
            new_value,
            old_value: None,
        }
    }

    fn value_of(a: &Action) -> f64 {
        match a {
            Action::TableEdit { new_value, .. } => *new_value,
            _ => panic!("not a table edit"),
        }
    }

    /// The agent path enforces the same authority policy as a live session. If
    /// the rails only reach one of the two, the other can still write past them.
    #[test]
    fn the_agent_path_is_railed_too() {
        let r = clamp_table_edit(edit(400.0), &railed(0.0, 120.0), Some(100.0));
        assert_eq!(value_of(&r.action), 120.0, "ceiling must apply to agent edits");
        assert!(r.clamped_from.is_some(), "clamping must be reported, not silent");
        assert!(
            r.reason.as_deref().unwrap_or("").contains("outside the allowed range"),
            "reason should name the rail, got {:?}",
            r.reason
        );
    }

    #[test]
    fn the_agent_path_floor_applies() {
        let r = clamp_table_edit(edit(-50.0), &railed(10.0, 255.0), Some(100.0));
        assert_eq!(value_of(&r.action), 10.0);
    }

    #[test]
    fn an_in_range_edit_is_untouched() {
        let r = clamp_table_edit(edit(105.0), &railed(0.0, 255.0), Some(100.0));
        assert_eq!(value_of(&r.action), 105.0);
        assert!(r.clamped_from.is_none(), "no clamp should be reported");
    }
}
