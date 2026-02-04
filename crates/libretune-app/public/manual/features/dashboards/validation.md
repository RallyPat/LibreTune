# Dashboard Validation

LibreTune can validate dashboards and surface issues that might prevent gauges from rendering correctly.

## Where to Find Validation

When a dashboard loads, LibreTune runs a validation check and shows a **Validate** button in the dashboard header if any issues are detected. Click it to view details.

## Error vs Warning

- **Errors**: Critical issues (e.g., unknown channels, invalid ranges) that can break gauge rendering.
- **Warnings**: Non-critical issues (e.g., tiny gauges, poor contrast, out-of-bounds elements).

## Common Issues

- **Unknown output channel**: The gauge references a channel that doesn’t exist in the current ECU definition.
- **Invalid range**: The gauge minimum is greater than or equal to the maximum.
- **Missing embedded image**: A referenced image wasn’t included in the dashboard file.

## Recommended Workflow

1. Load the dashboard.
2. Open **Validate** and review errors first.
3. Fix gauges in Designer Mode or edit the source dashboard file.
4. Re-load the dashboard to confirm errors are cleared.
