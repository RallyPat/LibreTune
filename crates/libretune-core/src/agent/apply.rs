//! Pure application of approved table actions to a z-value grid.
//!
//! The command layer reads the current table grid, hands it here together
//! with the approved actions for that table, and writes the result back
//! through the normal page-write path. Keeping this step pure (no app state,
//! no ECU I/O) makes the apply path unit-testable and guarantees the grid a
//! user approved is the grid that gets written.
//!
//! Cell indexing convention: [`crate::action_scripting::Action`] stores cells
//! as `(x, y)` = (column, row), matching the tool schema exposed to the
//! model. [`crate::table_ops`] uses `(row, col)` tuples; conversion happens
//! here, in one place.

use crate::action_scripting::Action;

/// Apply `actions` (TableEdit and BulkOperation variants only) to
/// `z_values` in order, in place.
///
/// - TableEdit sets the cell to `new_value` after bounds-checking.
/// - BulkOperation dispatches to the corresponding [`crate::table_ops`]
///   function over the listed cells (converted from `(x, y)` to
///   `(row, col)`).
///
/// Any other action variant, an out-of-range cell, or a bulk operation
/// missing a required parameter fails with a human-readable error naming the
/// offending action. On error the grid may be partially mutated — callers
/// discard it on failure and never write a partial result.
pub fn apply_table_actions_to_grid(
    z_values: &mut Vec<Vec<f64>>,
    actions: &[Action],
) -> Result<(), String> {
    for action in actions {
        match action {
            Action::TableEdit {
                x_index,
                y_index,
                new_value,
                ..
            } => {
                let (x, y) = (*x_index as usize, *y_index as usize);
                // Bounds-check up front (closures in `ok_or_else` cannot
                // borrow the grid while `get_mut` holds it).
                let rows = z_values.len();
                if y >= rows {
                    return Err(format!("table edit row {y} out of range ({rows} rows)"));
                }
                let cols = z_values[y].len();
                if x >= cols {
                    return Err(format!(
                        "table edit column {x} out of range ({cols} columns)"
                    ));
                }
                z_values[y][x] = *new_value;
            }
            Action::BulkOperation {
                operation,
                cells,
                parameters,
                ..
            } => {
                // Action cells are (x, y) = (col, row); table_ops wants
                // (row, col).
                let table_cells: Vec<(usize, usize)> = cells
                    .iter()
                    .map(|&(x, y)| (y as usize, x as usize))
                    .collect();

                match operation.as_str() {
                    "scale" => {
                        let factor = parameters.get("factor").copied().unwrap_or(1.0);
                        let next = crate::table_ops::scale_cells(z_values, table_cells, factor);
                        *z_values = next;
                    }
                    "smooth" => {
                        let factor = parameters.get("factor").copied().unwrap_or(1.0);
                        let next = crate::table_ops::smooth_table(z_values, table_cells, factor);
                        *z_values = next;
                    }
                    "interpolate" => {
                        let next = crate::table_ops::interpolate_cells(z_values, table_cells);
                        *z_values = next;
                    }
                    "set_equal" => {
                        let value = parameters.get("value").copied().ok_or_else(|| {
                            "set_equal bulk operation requires a 'value' parameter".to_string()
                        })?;
                        crate::table_ops::set_cells_equal(z_values, table_cells, value);
                    }
                    other => {
                        return Err(format!("unsupported bulk operation '{other}'"));
                    }
                }
            }
            other => {
                return Err(format!(
                    "action {:?} is not a table action; only TableEdit and BulkOperation can be applied to a grid",
                    action_kind(other)
                ));
            }
        }
    }
    Ok(())
}

/// Short stable name for an action, for error messages (Debug on the full
/// action would dump every cell of a bulk operation).
fn action_kind(action: &Action) -> &'static str {
    match action {
        Action::TableEdit { .. } => "TableEdit",
        Action::ConstantChange { .. } => "ConstantChange",
        Action::BulkOperation { .. } => "BulkOperation",
        Action::ExecuteLuaScript { .. } => "ExecuteLuaScript",
        Action::Pause { .. } => "Pause",
        Action::SendCommand { .. } => "SendCommand",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn grid() -> Vec<Vec<f64>> {
        vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ]
    }

    fn edit(x: u16, y: u16, value: f64) -> Action {
        Action::TableEdit {
            table_name: "t".into(),
            x_index: x,
            y_index: y,
            new_value: value,
            old_value: None,
        }
    }

    #[test]
    fn applies_single_edit() {
        let mut z = grid();
        // (x=2, y=0) is row 0, column 2 → currently 3.0
        apply_table_actions_to_grid(&mut z, &[edit(2, 0, 99.0)]).unwrap();
        assert_eq!(z[0][2], 99.0);
        // Untouched cells survive.
        assert_eq!(z[2][0], 7.0);
    }

    #[test]
    fn applies_edits_in_order() {
        let mut z = grid();
        // Second edit lands on the value the first edit wrote.
        apply_table_actions_to_grid(&mut z, &[edit(0, 0, 10.0), edit(0, 0, 20.0)]).unwrap();
        assert_eq!(z[0][0], 20.0);
    }

    #[test]
    fn rejects_out_of_range_row() {
        let mut z = grid();
        let err = apply_table_actions_to_grid(&mut z, &[edit(0, 5, 1.0)]).unwrap_err();
        assert!(err.contains("row 5"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_out_of_range_column() {
        let mut z = grid();
        let err = apply_table_actions_to_grid(&mut z, &[edit(9, 1, 1.0)]).unwrap_err();
        assert!(err.contains("column 9"), "unexpected error: {err}");
    }

    #[test]
    fn scale_converts_xy_to_row_col() {
        let mut z = grid();
        // Cells (x=0,y=2) and (x=2,y=0) → grid[2][0]=7 and grid[0][2]=3.
        let mut params = HashMap::new();
        params.insert("factor".to_string(), 10.0);
        let action = Action::BulkOperation {
            operation: "scale".into(),
            table_name: "t".into(),
            cells: vec![(0, 2), (2, 0)],
            parameters: params,
            old_values: None,
        };
        apply_table_actions_to_grid(&mut z, &[action]).unwrap();
        assert_eq!(z[2][0], 70.0);
        assert_eq!(z[0][2], 30.0);
        // Everything else untouched.
        assert_eq!(z[1][1], 5.0);
    }

    #[test]
    fn set_equal_requires_value() {
        let mut z = grid();
        let action = Action::BulkOperation {
            operation: "set_equal".into(),
            table_name: "t".into(),
            cells: vec![(1, 1)],
            parameters: HashMap::new(),
            old_values: None,
        };
        let err = apply_table_actions_to_grid(&mut z, &[action]).unwrap_err();
        assert!(err.contains("'value'"), "unexpected error: {err}");
    }

    #[test]
    fn set_equal_writes_cells() {
        let mut z = grid();
        let mut params = HashMap::new();
        params.insert("value".to_string(), 42.0);
        let action = Action::BulkOperation {
            operation: "set_equal".into(),
            table_name: "t".into(),
            cells: vec![(0, 0), (2, 2)],
            parameters: params,
            old_values: None,
        };
        apply_table_actions_to_grid(&mut z, &[action]).unwrap();
        assert_eq!(z[0][0], 42.0);
        assert_eq!(z[2][2], 42.0);
        assert_eq!(z[1][1], 5.0);
    }

    #[test]
    fn interpolate_over_selection() {
        let mut z = grid();
        // Full-grid selection: corners 1,3,7,9 → centre becomes 5.0.
        let mut cells = Vec::new();
        for y in 0..3u16 {
            for x in 0..3u16 {
                cells.push((x, y));
            }
        }
        let action = Action::BulkOperation {
            operation: "interpolate".into(),
            table_name: "t".into(),
            cells,
            parameters: HashMap::new(),
            old_values: None,
        };
        apply_table_actions_to_grid(&mut z, &[action]).unwrap();
        assert_eq!(z[1][1], 5.0);
        // Corners keep their values.
        assert_eq!(z[0][0], 1.0);
        assert_eq!(z[2][2], 9.0);
    }

    #[test]
    fn rejects_non_table_action() {
        let mut z = grid();
        let err =
            apply_table_actions_to_grid(&mut z, &[Action::Pause { duration_ms: 10 }]).unwrap_err();
        assert!(err.contains("Pause"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_unknown_operation() {
        let mut z = grid();
        let action = Action::BulkOperation {
            operation: "rebin".into(),
            table_name: "t".into(),
            cells: vec![(0, 0)],
            parameters: HashMap::new(),
            old_values: None,
        };
        let err = apply_table_actions_to_grid(&mut z, &[action]).unwrap_err();
        assert!(err.contains("rebin"), "unexpected error: {err}");
    }

    #[test]
    fn smooth_changes_center_of_flat_region() {
        // A grid with a single spike: smoothing must pull the spike toward
        // its neighborhood.
        let mut z = vec![vec![1.0; 3]; 3];
        z[1][1] = 10.0;
        let mut params = HashMap::new();
        params.insert("factor".to_string(), 1.0);
        let action = Action::BulkOperation {
            operation: "smooth".into(),
            table_name: "t".into(),
            cells: vec![(1, 1)],
            parameters: params,
            old_values: None,
        };
        apply_table_actions_to_grid(&mut z, &[action]).unwrap();
        assert!(z[1][1] < 10.0, "spike should shrink: {}", z[1][1]);
        assert!(
            z[1][1] > 1.0,
            "spike should not flatten below neighbors: {}",
            z[1][1]
        );
    }
}
