//! Lua scripting engine (sandboxed)

use mlua::{Lua, LuaOptions, StdLib, Value, Variadic};
use std::sync::{Arc, Mutex};

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
    let lua_options = LuaOptions::new()
        .catch_rust_panics(true);
    
    let lua = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH,
        lua_options
    )
        .map_err(|e| format!("Failed to initialize Lua: {e}"))?;

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
