# Dashboard Validation

LibreTune can validate dashboards and surface issues that might prevent gauges from rendering correctly — and suggest one-click fixes for the most common problem.

## Where to Find Validation

Validation runs automatically whenever a dashboard changes. The header shows a **Validate** button with error/warning counts; click it to open the validation panel.

## Error vs Warning

- **Errors**: Critical issues (e.g., unknown channels, invalid ranges) that can break gauge rendering.
- **Warnings**: Non-critical issues (e.g., tiny gauges, out-of-bounds elements).

Channel checks run against the **currently loaded INI definition**, so a dashboard authored for one firmware shows its unknown channels as soon as it's opened against another.

## Common Issues

- **Unknown output channel**: The gauge references a channel that doesn't exist in the current ECU definition.
- **Invalid range**: The gauge minimum is greater than or equal to the maximum.
- **Missing embedded image**: A referenced image wasn't included in the dashboard file.

## Suggested Channel Remaps

When gauges reference channels the loaded INI doesn't define, the validation panel lists **suggested remaps**: each shows the unknown channel, a likely match from your INI, and an **Apply** button. Suggestions come from:

- A cross-firmware synonym table (e.g. `coolant` ↔ `clt`, `rpm` ↔ `engineSpeed`, `pulseWidth` ↔ `pw`)
- A guarded fuzzy match on channel names

Applying a remap rebinds that gauge to the suggested channel immediately; the list refreshes and applied entries drop out. This makes dashboards built for one ECU (including LibreTune's Speeduino-authored defaults) quickly usable on another — apply the suggestions, then save the dashboard.

## Recommended Workflow

1. Load the dashboard.
2. Open **Validate** and review errors first.
3. Apply suggested channel remaps where the match is right.
4. Fix remaining gauges in Designer Mode.
5. Save, and re-validate to confirm the panel is clean.
