// Integration tests for Lua scripting engine
// Tests the execute_script function directly without Tauri overhead

use libretune_core::lua::execute_script;

#[test]
fn test_lua_print_basic() {
    let script = r#"
        print("Hello from Lua!")
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert_eq!(result.stdout.trim(), "Hello from Lua!");
    assert!(result.error.is_none());
}

#[test]
fn test_lua_print_multiple_values() {
    let script = r#"
        print("Value:", 42, "Bool:", true)
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("Value:"));
    assert!(result.stdout.contains("42"));
    assert!(result.stdout.contains("true"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_variable_assignment() {
    let script = r#"
        x = 42
        y = "hello"
        print("x=", x, "y=", y)
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("x="));
    assert!(result.stdout.contains("42"));
    assert!(result.stdout.contains("y="));
    assert!(result.stdout.contains("hello"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_table_operations() {
    let script = r#"
        local t = {10, 20, 30}
        table.insert(t, 40)
        print("Table length:", #t)
        for i, v in ipairs(t) do
            print("Index", i, "=", v)
        end
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("Table length:"));
    assert!(result.stdout.contains("4"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_math_operations() {
    let script = r#"
        print("sqrt(16) =", math.sqrt(16))
        print("pow(2, 10) =", math.pow(2, 10))
        print("abs(-42) =", math.abs(-42))
        print("max(1,5,3) =", math.max(1, 5, 3))
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("sqrt(16)"));
    assert!(result.stdout.contains("4"));
    assert!(result.stdout.contains("pow(2, 10)"));
    assert!(result.stdout.contains("1024"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_string_operations() {
    let script = r#"
        local s = "hello world"
        print("Substring:", string.sub(s, 1, 5))
        print("Length:", #s)
        local formatted = string.format("Number: %d, Float: %.1f", 42, 3.14159)
        print(formatted)
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("Substring:"));
    assert!(result.stdout.contains("hello"));
    assert!(result.stdout.contains("Length:"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_function_definition() {
    let script = r#"
        function add(a, b)
            return a + b
        end
        
        result = add(10, 20)
        print("10 + 20 =", result)
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("10 + 20"));
    assert!(result.stdout.contains("30"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_syntax_error() {
    let script = r#"
        local x = 10 +  -- Missing operand
    "#;
    
    let result = execute_script(script).expect("Script execution should complete");
    
    // Should have error but execution should not panic
    assert!(result.error.is_some());
    let error_msg = result.error.unwrap();
    assert!(error_msg.contains("syntax") || error_msg.contains("unexpected"));
}

#[test]
fn test_lua_runtime_error() {
    let script = r#"
        local x = nil
        print(x.field)  -- Attempt to index nil
    "#;
    
    let result = execute_script(script).expect("Script execution should complete");
    
    assert!(result.error.is_some());
    let error_msg = result.error.unwrap();
    assert!(error_msg.contains("index") || error_msg.contains("nil"));
}

#[test]
fn test_lua_sandboxing_os_restricted() {
    // OS library should not be available in sandboxed environment
    let script = r#"
        if os then
            print("ERROR: os library should not be available")
        else
            print("OK: os library correctly restricted")
        end
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("OK"));
    assert!(!result.stdout.contains("ERROR"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_sandboxing_io_restricted() {
    // IO library should not be available in sandboxed environment
    let script = r#"
        if io then
            print("ERROR: io library should not be available")
        else
            print("OK: io library correctly restricted")
        end
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("OK"));
    assert!(!result.stdout.contains("ERROR"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_type_function() {
    let script = r#"
        print("Type of 42:", type(42))
        print("Type of 'hello':", type("hello"))
        print("Type of {}:", type({}))
        print("Type of true:", type(true))
        print("Type of nil:", type(nil))
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("number"));
    assert!(result.stdout.contains("string"));
    assert!(result.stdout.contains("table"));
    assert!(result.stdout.contains("boolean"));
    assert!(result.stdout.contains("nil"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_type_conversion() {
    let script = r#"
        local n = tonumber("123")
        local s = tostring(456)
        print("tonumber('123') =", n, "type:", type(n))
        print("tostring(456) =", s, "type:", type(s))
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("123"));
    assert!(result.stdout.contains("456"));
    assert!(result.error.is_none());
}

#[test]
fn test_lua_ve_table_scaling_example() {
    // Real-world example: scale VE table by percentage
    let script = r#"
        -- Simulate VE table data (2x2 for testing)
        local ve_table = {
            {100, 105},
            {110, 115}
        }
        
        -- Scale all values by 1.1 (+10%)
        for i = 1, #ve_table do
            for j = 1, #ve_table[i] do
                ve_table[i][j] = ve_table[i][j] * 1.1
            end
        end
        
        -- Verify scaling
        print("Original [1][1] would be ~", ve_table[1][1] / 1.1)
        print("Scaled [1][1] is now", ve_table[1][1])
        print("Scaled [2][2] is now", ve_table[2][2])
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("110")); // 100 * 1.1
    assert!(result.stdout.contains("126.5")); // 115 * 1.1
    assert!(result.error.is_none());
}

#[test]
fn test_lua_empty_script() {
    let script = "";
    
    let result = execute_script(script).expect("Script execution failed");
    
    // Empty script should not error
    assert!(result.error.is_none());
}

#[test]
fn test_lua_whitespace_script() {
    let script = "   \n  \n   ";
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.error.is_none());
}

#[test]
fn test_lua_comment_only_script() {
    let script = r#"
        -- This is a comment
        -- More comments here
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.error.is_none());
}

#[test]
fn test_lua_loop_and_conditionals() {
    let script = r#"
        local sum = 0
        for i = 1, 10 do
            if i % 2 == 0 then
                sum = sum + i
            end
        end
        print("Sum of even numbers 1-10:", sum)
    "#;
    
    let result = execute_script(script).expect("Script execution failed");
    
    assert!(result.stdout.contains("Sum of even numbers"));
    assert!(result.stdout.contains("30")); // 2+4+6+8+10
    assert!(result.error.is_none());
}
