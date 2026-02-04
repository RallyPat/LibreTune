# Lua Scripting Technical Reference

LibreTune includes a sandboxed Lua 5.4 runtime for advanced automation and custom tuning workflows.

## Runtime Architecture

### Sandboxing

**Allowed Libraries** (restricted subset):
- `BASE` - Core functions (print, type, tonumber, etc.)
- `TABLE` - Table manipulation (table.insert, table.remove, etc.)
- `STRING` - String operations (string.sub, string.format, etc.)
- `MATH` - Math functions (math.sin, math.sqrt, etc.)

**Disabled Libraries** (security):
- `OS` - Operating system interaction (os.execute, os.remove, etc.)
- `IO` - File I/O (io.open, io.read, etc.)
- `PACKAGE` - Module loading (require, package.path, etc.)
- `DEBUG` - Debugger hooks (debug.getinfo, etc.)

**Rationale**: Prevent scripts from:
- Reading/writing arbitrary files
- Executing system commands
- Accessing network resources
- Loading untrusted code

### Execution Model

**Single-Threaded**:
```
User clicks "Run" → Tauri command → Rust executor → mlua → Lua VM → Result
```

**Timeout Protection**:
```rust
lua.set_hook(HookTriggers::COUNT, move |_lua, _debug| {
    if execution_time > MAX_EXECUTION_TIME {
        Err(Error::RuntimeError("Script timeout".to_string()))
    } else {
        Ok(())
    }
});
```

**Default Timeout**: 5 seconds (prevents infinite loops)

## API Reference

### Global Functions

#### `print(...)`

Captures output to script console (does not write to stdout).

**Example**:
```lua
print("Hello from Lua!")
print("Value:", 42, "Bool:", true)
```

**Output**:
```
Hello from Lua!
Value: 42 Bool: true
```

#### `type(value)`

Returns type of value as string.

**Example**:
```lua
print(type(42))        -- "number"
print(type("hello"))   -- "string"
print(type({}))        -- "table"
```

#### `tonumber(str)`, `tostring(val)`

Type conversion functions.

**Example**:
```lua
local n = tonumber("123.45")  -- 123.45
local s = tostring(42)        -- "42"
```

### Table Library

#### `table.insert(list, [pos,] value)`

Insert value into table.

**Example**:
```lua
local t = {10, 20, 30}
table.insert(t, 40)      -- {10, 20, 30, 40}
table.insert(t, 2, 15)   -- {10, 15, 20, 30, 40}
```

#### `table.remove(list, [pos])`

Remove value from table.

**Example**:
```lua
local t = {10, 20, 30}
table.remove(t, 2)  -- {10, 30}
table.remove(t)     -- {10} (removes last)
```

#### `table.concat(list, [sep])`

Join table elements into string.

**Example**:
```lua
local t = {"a", "b", "c"}
print(table.concat(t, ", "))  -- "a, b, c"
```

### String Library

#### `string.sub(s, i, [j])`

Extract substring.

**Example**:
```lua
local s = "hello world"
print(string.sub(s, 1, 5))   -- "hello"
print(string.sub(s, 7))      -- "world"
```

#### `string.format(fmt, ...)`

Format string (printf-style).

**Example**:
```lua
local rpm = 3500
local afr = 14.7
print(string.format("RPM: %d, AFR: %.1f", rpm, afr))
-- "RPM: 3500, AFR: 14.7"
```

### Math Library

#### `math.sin(x)`, `math.cos(x)`, `math.tan(x)`

Trigonometric functions (radians).

**Example**:
```lua
local angle = math.pi / 4
print(math.sin(angle))  -- 0.707...
```

#### `math.sqrt(x)`, `math.pow(x, y)`

Exponentiation and roots.

**Example**:
```lua
print(math.sqrt(16))     -- 4.0
print(math.pow(2, 10))   -- 1024.0
```

#### `math.abs(x)`, `math.ceil(x)`, `math.floor(x)`

Rounding and absolute value.

**Example**:
```lua
print(math.abs(-42))    -- 42
print(math.ceil(3.2))   -- 4
print(math.floor(3.8))  -- 3
```

#### `math.min(...)`

Returns minimum value.

**Example**:
```lua
print(math.min(10, 5, 8))  -- 5
```

#### `math.max(...)`

Returns maximum value.

**Example**:
```lua
print(math.max(10, 5, 8))  -- 10
```

## Use Cases

### 1. Batch Table Operations

**Problem**: Apply custom formula to all VE table cells

**Solution**:
```lua
-- Example: Increase VE by 5% in RPM range 2000-4000
for rpm_idx = 1, 16 do
    local rpm = rpm_bins[rpm_idx]
    if rpm >= 2000 and rpm <= 4000 then
        for map_idx = 1, 16 do
            ve_table[rpm_idx][map_idx] = ve_table[rpm_idx][map_idx] * 1.05
        end
    end
end
```

### 2. Custom Tuning Formulas

**Problem**: Calculate required VE for target boost pressure

**Solution**:
```lua
-- Lambda target: 0.85 (rich for boost)
-- Boost: 10 psi (1.68 bar absolute)
-- Base VE: 85%

function boost_ve_correction(base_ve, boost_psi)
    local boost_ratio = (boost_psi * 0.0689 + 1.0) / 1.0  -- PSI to bar
    return base_ve * boost_ratio * 0.85  -- 15% enrichment
end

local corrected_ve = boost_ve_correction(85, 10)
print("Target VE:", corrected_ve)  -- ~122%
```

### 3. Data Validation

**Problem**: Find and report out-of-range values

**Solution**:
```lua
function validate_afr_table(afr_table)
    local errors = {}
    for i = 1, #afr_table do
        for j = 1, #afr_table[i] do
            local afr = afr_table[i][j]
            if afr < 10 or afr > 20 then
                table.insert(errors, string.format("Cell [%d][%d]: %.1f", i, j, afr))
            end
        end
    end
    return errors
end

local errors = validate_afr_table(afr_table)
if #errors > 0 then
    print("AFR table validation errors:")
    for _, err in ipairs(errors) do
        print(" - ", err)
    end
end
```

### 4. Unit Conversion

**Problem**: Convert entire table from AFR to Lambda

**Solution**:
```lua
-- Stoichiometric AFR for gasoline: 14.7
function afr_to_lambda(afr)
    return afr / 14.7
end

for i = 1, #afr_table do
    for j = 1, #afr_table[i] do
        lambda_table[i][j] = afr_to_lambda(afr_table[i][j])
    end
end
```

## Script Output

### Print Capture

**Implementation**:
```rust
let output = Arc::new(Mutex::new(String::new()));
let output_clone = Arc::clone(&output);

lua.globals().set(
    "print",
    lua.create_function(move |lua_ctx, args: mlua::Variadic<mlua::Value>| {
        let mut output = output_clone.lock().unwrap();
        let formatted = args.iter()
            .map(|v| format_value(v, lua_ctx))
            .collect::<Vec<_>>()
            .join("\t");
        output.push_str(&formatted);
        output.push('\n');
        Ok(())
    })?,
)?;
```

**Result Structure**:
```rust
pub struct LuaResult {
    pub stdout: String,       // All print() calls
    pub return_value: String, // Last expression value
    pub error: Option<String>, // Runtime error (if any)
}
```

### Error Handling

**Syntax Errors**:
```lua
local x = 10 +  -- Missing operand
```

**Output**:
```
Error: [string "script"]:1: unexpected symbol near '<eof>'
```

**Runtime Errors**:
```lua
local x = nil
print(x.field)  -- Attempt to index nil
```

**Output**:
```
Error: [string "script"]:2: attempt to index a nil value (local 'x')
```

## Performance Characteristics

| Operation | Complexity | Typical Time |
|-----------|-----------|--------------|
| Script initialization | O(1) | <1ms |
| Execute 100 lines | O(n) | <5ms |
| Table iteration (16×16) | O(n²) | <1ms |
| String concatenation | O(n) | <1ms per 1000 chars |

**Memory Limits**:
- Heap size: 10 MB (per script execution)
- Stack depth: 1000 (recursion limit)

## Security Model

### Threat Model

**Protected Against**:
- Arbitrary file system access
- Network access
- Code injection attacks
- Resource exhaustion (timeout)
- Privilege escalation

**NOT Protected Against**:
- Logical errors in user scripts
- Intentional corruption of tune data
- CPU-bound infinite loops (covered by timeout)

### Best Practices

1. **Always test scripts on backup tunes**
2. **Use version control before running scripts**
3. **Validate script output before applying**
4. **Keep scripts small and focused**
5. **Avoid hardcoded values (use variables)**

## Script Examples

### Example 1: Progressive VE Scaling

```lua
-- Scale VE table progressively by RPM
-- Low RPM: +0%, High RPM: +15%

local rpm_min, rpm_max = 1000, 7000
local scale_min, scale_max = 1.0, 1.15

for i = 1, #ve_table do
    local rpm = rpm_bins[i]
    local progress = (rpm - rpm_min) / (rpm_max - rpm_min)
    progress = math.max(0, math.min(1, progress))  -- Clamp 0-1
    
    local scale = scale_min + (scale_max - scale_min) * progress
    
    for j = 1, #ve_table[i] do
        ve_table[i][j] = ve_table[i][j] * scale
    end
end

print("VE table scaled progressively")
```

### Example 2: Copy Region Between Tables

```lua
-- Copy high-load region from Table A to Table B

local rpm_start, rpm_end = 3000, 6000
local map_threshold = 80

for i = 1, #table_a do
    local rpm = rpm_bins[i]
    if rpm >= rpm_start and rpm <= rpm_end then
        for j = 1, #table_a[i] do
            local map = map_bins[j]
            if map >= map_threshold then
                table_b[i][j] = table_a[i][j]
            end
        end
    end
end

print("High-load region copied")
```

### Example 3: Smooth Transitions

```lua
-- Create smooth transition between two VE values

function lerp(a, b, t)
    return a + (b - a) * t
end

local start_rpm, end_rpm = 2000, 3000
local start_ve, end_ve = 70, 85

for i = 1, #ve_table do
    local rpm = rpm_bins[i]
    if rpm >= start_rpm and rpm <= end_rpm then
        local t = (rpm - start_rpm) / (end_rpm - start_rpm)
        local target_ve = lerp(start_ve, end_ve, t)
        
        for j = 1, #ve_table[i] do
            ve_table[i][j] = target_ve
        end
    end
end

print("Smooth transition created")
```

## Source Code Reference

- Lua runtime: `crates/libretune-core/src/lua/mod.rs`
- Tauri command: `crates/libretune-app/src-tauri/src/lib.rs` (run_lua_script)
- Console UI: `crates/libretune-app/src/components/console/LuaConsole.tsx`
- Dependencies: `mlua = { version = "0.10", features = ["lua54", "vendored"] }`

## See Also

- [Lua Scripting User Guide](../features/lua-scripting.md) - Basic usage and examples
- [Action Manager](../features/tools.md#action-manager) - Recording and replaying actions
- [Table Operations](./table-operations.md) - Built-in table manipulation functions
