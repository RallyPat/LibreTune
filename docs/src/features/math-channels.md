# Math Channels & Expression Engine

LibreTune includes a powerful runtime expression engine that powers **Math Channels** (OutputChannels). This system allows the ECU definition (INI) to define complex calculated values derived from raw ECU sensor data.

## Overview

Math Channels are defined in the `[OutputChannels]` section of the INI file. They differ from raw channels (which read bytes directly from memory) in that they are computed on the host computer.

**Key Features:**
- **Standard Math**: Addition, subtraction, multiplication, division, modulo.
- **Bitwise Operations**: AND (`&`), OR (`|`), XOR (`^`), Left Shift (`<<`), Right Shift (`>>`).
- **Comparisons**: Greater (`>`), Less (`<`), Equal (`==`), Not Equal (`!=`), etc.
- **Logical Operators**: AND (`&&`), OR (`||`), NOT (`!`).
- **Ternary Operator**: Conditional assignment (`condition ? true_val : false_val`).
- **Functions**: Built-in library of mathematical and utility functions.

## Expression Syntax

Expressions are written in standard infix notation. Variable names refer to other Output Channels or Constants.

### Examples
```ini
; Simple arithmetic
dutyCycle = { (pw * rpm) / 1200 }

; Conditional logic (Ternary)
engineState = { rpm > 500 ? (tps > 80 ? 3 : 1) : 0 }

; Bitwise operations
fanStatus = { (outputs & 4) != 0 }

; Function calls
smoothedAfr = { round(afr) }
```

## Built-in Functions

The expression engine supports the following functions:

### Mathematical
| Function | Description | Example |
|----------|-------------|---------|
| `min(a, b)` | Returns smaller value | `min(clt, 100)` |
| `max(a, b)` | Returns larger value | `max(tps, 0)` |
| `abs(x)` | Absolute value | `abs(map - baro)` |
| `round(x)` | Round to nearest integer | `round(lambda * 100)` |
| `sqrt(x)` | Square root | `sqrt(x_sq)` |
| `log(x)` | Natural logarithm | `log(pressure)` |
| `exp(x)` | Exponential (e^x) | `exp(factor)` |
| `sin(x)`, `cos(x)`, `tan(x)` | Trigonometry (radians) | `sin(angle)` |

### System & Utility
| Function | Description | Example |
|----------|-------------|---------|
| `timeNow()` | Current system time (ms) | `timeNow() / 1000` |
| `isOnline()` | Returns 1 if connected, 0 otherwise | `isOnline()` |
| `isNaN(x)` | Returns 1 if value is NaN | `isNaN(sensor)` |

### Conditional (Legacy)
| Function | Description | Example |
|----------|-------------|---------|
| `if(cond, true, false)` | Equivalent to ternary | `if(rpm>0, 1, 0)` |

## Evaluation Order

The **Evaluator** determines the correct order of operations automatically. You do not need to manually order channels in the INI file.
1.  **Raw Channels** are read from the ECU memory block.
2.  **Math Channels** are computed in passes:
    *   **Pass 1**: Channels depending only on raw values.
    *   **Pass 2**: Channels depending on Pass 1 results.
    *   **Pass 3**: Deeply nested dependencies.

Circular dependencies (e.g., `A = B + 1` and `B = A + 1`) will result in `NaN` (Not a Number) or fallback to 0.

## Performance

Expressions are parsed once when the ECU definition is loaded and cached. Evaluation at runtime is highly optimized to handle hundreds of channels at 100Hz+ without significant CPU load.
