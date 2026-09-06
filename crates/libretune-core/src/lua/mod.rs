//! Lua scripting engine (sandboxed)

use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Value, Variadic, VmState};
use std::sync::{Arc, Mutex};

/// Memory ceiling (bytes) for a single [`execute_script`] call. Guards
/// against a script that allocates without bound (e.g. growing a table
/// forever) exhausting the host process's memory.
const LUA_MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;

/// Instruction budget for a single [`execute_script`] call, enforced via a
/// `set_hook` triggered every `LUA_INSTRUCTION_BUDGET` VM instructions. A
/// script like `while true do end` runs no host calls and allocates nothing,
/// so neither a timeout nor the memory limit above would ever stop it —
/// this hook is what actually bounds a runaway loop's CPU time.
const LUA_INSTRUCTION_BUDGET: u32 = 10_000_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LuaExecutionResult {
    pub stdout: String,
    pub return_value: Option<String>,
    pub error: Option<String>,
}

fn format_value(value: &Value) -> Option<String> {
    match value {
        Value::Nil => None,
        Value::Boolean(v) => Some(v.to_string()),
        Value::Integer(v) => Some(v.to_string()),
        Value::Number(v) => Some(v.to_string()),
        Value::String(v) => Some(v.to_string_lossy().to_string()),
        Value::Table(_) => Some("<table>".to_string()),
        Value::Function(_) => Some("<function>".to_string()),
        Value::Thread(_) => Some("<thread>".to_string()),
        Value::UserData(_) => Some("<userdata>".to_string()),
        Value::LightUserData(_) => Some("<lightuserdata>".to_string()),
        Value::Error(err) => Some(format!("<error: {}>", err)),
        Value::Other(_) => Some("<other>".to_string()),
    }
}

pub fn execute_script(script: &str) -> Result<LuaExecutionResult, String> {
    let output = Arc::new(Mutex::new(Vec::<String>::new()));
    let output_writer = output.clone();

    // Create Lua with vendored Lua 5.4 and sandboxed standard libraries
    let lua_options = LuaOptions::new().catch_rust_panics(true);

    let lua = Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH, lua_options)
        .map_err(|e| format!("Failed to initialize Lua: {e}"))?;

    lua.set_memory_limit(LUA_MEMORY_LIMIT_BYTES)
        .map_err(|e| format!("Failed to set Lua memory limit: {e}"))?;

    // Errors out of the executing script once it has run past the
    // instruction budget, turning a runaway `while true do end` into a
    // returned error instead of a permanently hung tokio worker.
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(LUA_INSTRUCTION_BUDGET),
        |_lua, _debug| -> mlua::Result<VmState> {
            Err(mlua::Error::RuntimeError(format!(
                "Script exceeded the instruction budget of {LUA_INSTRUCTION_BUDGET} VM instructions"
            )))
        },
    );

    let print_fn = lua
        .create_function(move |_, args: Variadic<Value>| {
            let mut line = String::new();
            for (idx, value) in args.iter().enumerate() {
                if idx > 0 {
                    line.push('\t');
                }
                if let Some(text) = format_value(value) {
                    line.push_str(&text);
                } else {
                    line.push_str("nil");
                }
            }
            if let Ok(mut guard) = output_writer.lock() {
                guard.push(line);
            }
            Ok(())
        })
        .map_err(|e| format!("Failed to create print function: {e}"))?;

    lua.globals()
        .set("print", print_fn)
        .map_err(|e| format!("Failed to set globals: {e}"))?;

    let mut error: Option<String> = None;
    let result_value = match lua.load(script).eval::<Value>() {
        Ok(val) => val,
        Err(e) => {
            error = Some(format!("Lua error: {e}"));
            Value::Nil
        }
    };

    let stdout = output
        .lock()
        .map(|lines| lines.join("\n"))
        .unwrap_or_default();

    Ok(LuaExecutionResult {
        stdout,
        return_value: format_value(&result_value),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infinite_loop_returns_error_instead_of_hanging() {
        let result = execute_script("while true do end")
            .expect("execute_script only errors on Lua-init failure, not script errors");

        assert!(
            result.error.is_some(),
            "expected the instruction-budget hook to stop an infinite loop with an error"
        );
        let error = result.error.unwrap();
        assert!(
            error.contains("instruction budget"),
            "expected the instruction-budget error message, got: {error}"
        );
    }

    #[test]
    fn test_normal_script_still_executes_successfully() {
        let result = execute_script("return 1 + 1").expect("execute_script should succeed");
        assert_eq!(result.error, None);
        assert_eq!(result.return_value, Some("2".to_string()));
    }
}
