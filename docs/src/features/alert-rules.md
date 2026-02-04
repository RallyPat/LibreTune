# Alert Rules

LibreTune can warn you when a table edit changes values more than a safe threshold. This helps prevent accidental large edits during tuning sessions.

## What Gets Flagged

Alert Rules watch table edits and bulk operations (Scale, Set Equal, Smooth, Interpolate, Rebin). When a change exceeds the configured thresholds, LibreTune shows a warning toast with the largest delta detected.

## Configure Thresholds

Open **Settings → Alert Rules** and set:

- **Warn on large table changes**: Enable or disable warnings.
- **Absolute Change Threshold**: The minimum absolute change that triggers a warning.
- **Percent Change Threshold**: The minimum percent change that triggers a warning.

A warning appears if **either** threshold is exceeded.

## Recommended Defaults

- **Absolute Threshold**: 5.0
- **Percent Threshold**: 10%

These defaults are conservative enough for fuel/ignition tables without being overly noisy.

## Notes

- Warnings are informational only and do not block edits.
- Warnings apply to bulk operations and direct cell edits.
- Percent change is calculated from the previous value.
