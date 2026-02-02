# Space Optimization for LibreTune Dialogs

## Overview
This PR implements space optimization for LibreTune dialog layouts, specifically addressing the issue where panels were stacking vertically and requiring users to scroll, even when horizontal space was available.

## Problem Statement
From issue: "Let's take advantage of this real-estate. this space can be optimized. may be move the TSP2 to the right here. have everything in one window instead of me having to scroll down. Put the box header to the left not right."

### Original Issues:
1. Panels stacked vertically in single column (wasted horizontal space)
2. Required scrolling to see all TPS sensor settings
3. Panel headers were right-aligned instead of left-aligned
4. Large graph/gauge area on right had plenty of unused space

## Solution

### 1. Multi-Column Layout Support
Implemented support for "West" and "East" positioning in dialog panels, allowing side-by-side layouts.

**INI Format:**
```ini
dialog = tpsSensor, "", border
    panel = tpsSensorLeft, West
    panel = tpsGauges, East
```

**Result:** 
- `tpsSensorLeft` renders in left column
- `tpsGauges` renders in right column
- Both visible simultaneously without scrolling

### 2. Panel Title Alignment
Changed panel title alignment from right to left for better readability.

**Before:** `Title ▶` (right-aligned)
**After:** `▶ Title` (left-aligned)

### 3. Backward Compatibility
Dialogs without position parameters continue to work as before:
- Panels without position render full-width
- Existing dialogs are unaffected
- Only dialogs with "West"/"East" get new layout

## Technical Implementation

### Frontend Changes

#### DialogRenderer.css
```css
/* New grid layout for East/West panels */
.dialog-view .dialog-row-container {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  align-items: start;
}

.dialog-view .dialog-column {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
}

/* Panel title alignment fix */
.dialog-view .panel-title {
  text-align: left; /* Changed from right */
}
```

#### DialogRenderer.tsx
```typescript
// New function to organize components by position
const organizeComponents = () => {
  const rows: { west: DialogComponent[], east: DialogComponent[], unpositioned: DialogComponent[] }[] = [];
  let currentRow = null;
  
  for (const comp of definition.components) {
    const position = comp.position?.toLowerCase();
    
    if (position === 'west' || position === 'east') {
      // Group positioned panels into rows
      if (!currentRow || currentRow[position].length > 0) {
        currentRow = { west: [], east: [], unpositioned: [] };
        rows.push(currentRow);
      }
      currentRow[position].push(comp);
    } else {
      // Unpositioned components render full-width
      if (!currentRow) {
        currentRow = { west: [], east: [], unpositioned: [] };
        rows.push(currentRow);
      }
      currentRow.unpositioned.push(comp);
    }
  }
  
  return rows;
};
```

### Backend Changes

#### types.rs
```rust
pub enum DialogComponent {
    // ... other variants ...
    Panel {
        name: String,
        position: Option<String>,  // NEW: Position parameter
        visibility_condition: Option<String>,
    },
}
```

#### parser.rs
```rust
// Parse position from INI: panel = name, Position, {condition}
let position = parts.get(1)
    .filter(|p| !p.trim().starts_with('{'))
    .map(|p| p.trim().to_string());
```

## Visual Comparison

### Before (Vertical Stack - Requires Scrolling)
```
┌────────────────────────────────┐
│ Header: Throttle Body #1       │
├────────────────────────────────┤
│ ┌────────────────────────────┐ │
│ │ Settings Panel         ▶│ │
│ │ • TBI Diameter             │ │
│ │ • TPS Sensor 1             │ │
│ │ • TPS Sensor 1 Closed      │ │
│ │ • TPS Sensor 1 Open        │ │
│ │ • TPS Sensor 2             │ │
│ │ • TPS Sensor 2 Closed      │ │
│ │ • TPS Sensor 2 Open        │ │
│ │ • TPS Filter               │ │
│ └────────────────────────────┘ │
│          ⬇ SCROLL ⬇            │
│ ┌────────────────────────────┐ │
│ │ Graph Panel            ▶│ │
│ │ [Throttle Graph Area]      │ │
│ └────────────────────────────┘ │
└────────────────────────────────┘
```

### After (Side-by-Side - No Scrolling)
```
┌──────────────────────────────────────────────┐
│ Header: Throttle Body #1                     │
├──────────────────────────────────────────────┤
│ ┌──────────────────┬─────────────────────┐ │
│ │ ▶ Settings       │ ▶ Graph             │ │
│ │ • TBI Diameter   │ [Throttle Graph]    │ │
│ │ • TPS Sensor 1   │                     │ │
│ │ • TPS Sensor 1   │                     │ │
│ │   Closed         │                     │ │
│ │ • TPS Sensor 1   │                     │ │
│ │   Open           │                     │ │
│ │ • TPS Sensor 2   │                     │ │
│ │ • TPS Sensor 2   │                     │ │
│ │   Closed         │                     │ │
│ │ • TPS Sensor 2   │                     │ │
│ │   Open           │                     │ │
│ │ • TPS Filter     │                     │ │
│ └──────────────────┴─────────────────────┘ │
└──────────────────────────────────────────────┘
```

## Benefits

1. **✅ No More Scrolling**: All content visible in one view
2. **✅ Better Space Usage**: Horizontal space now utilized effectively
3. **✅ Improved UX**: Settings and graphs visible simultaneously
4. **✅ Left-Aligned Headers**: Easier to scan and more conventional
5. **✅ Backward Compatible**: Existing dialogs work unchanged

## Files Modified

- `crates/libretune-app/src/components/dialogs/DialogRenderer.css` - Added grid layout classes
- `crates/libretune-app/src/components/dialogs/DialogRenderer.tsx` - Implemented position-based rendering
- `crates/libretune-core/src/ini/types.rs` - Added position field to Panel component
- `crates/libretune-core/src/ini/parser.rs` - Parse position from INI files

## Testing

To test this feature:
1. Navigate to "Engine → Electronic Throttle Body → TPS Sensor" dialog
2. Observe that settings and graph are now side-by-side
3. Verify no scrolling is required
4. Check that panel titles are left-aligned

## Future Enhancements

Possible future improvements:
- Support for "North", "South", "Center" positions
- Responsive layout that adapts to window size
- User preference for column widths
- Support for more than 2 columns

## View Visual Mockup

Open `dialog-layout-comparison.html` in a browser to see a visual comparison of before/after layouts.
