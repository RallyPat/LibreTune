# Deprecation Notice: Java Plugin System

**Effective Date**: February 4, 2026  
**Status**: Deprecated (UI disabled, will be removed in future release)  
**Migration Path**: WASM Plugin System

---

## Summary

The Java/JVM plugin system for TunerStudio JAR compatibility is being deprecated in favor of LibreTune's native WASM plugin architecture. This system, while fully functional, will be removed in a future release to:

- **Eliminate external dependencies** (JRE 11+ requirement)
- **Reduce maintenance burden** (~5,100 lines of Java/Rust/TypeScript code)
- **Improve security** (WASM sandboxing vs. JVM full permissions)
- **Focus development efforts** on the native WASM plugin ecosystem

---

## Timeline

| Date | Milestone |
|------|-----------|
| **Feb 4, 2026** | Deprecation announced, Java plugin menu item disabled |
| **TBD (2-4 releases)** | Grace period for community feedback and migration |
| **TBD (after grace period)** | Complete removal of Java plugin code (~5,100 lines) |

---

## What This Means for Users

### If You Don't Use Java Plugins
**No action required.** This change will not affect your usage of LibreTune.

### If You Use TunerStudio JAR Plugins
1. **Immediate**: The "Plugins..." menu item in Tools has been disabled
2. **Short-term**: Continue using the previous LibreTune version if you need Java plugin support
3. **Long-term**: Wait for WASM plugin rewrites or contribute your own

---

## What This Means for Plugin Developers

### Migration Required
Java/Swing-based TunerStudio plugins must be rewritten as WASM modules to continue working with LibreTune.

### Key Differences

| Aspect | Java Plugins (Deprecated) | WASM Plugins (Current) |
|--------|---------------------------|------------------------|
| **Language** | Java (any JVM language) | Rust, C, C++, AssemblyScript, or any WASM-compilable language |
| **UI Framework** | Swing (introspected to React) | Native LibreTune React components |
| **Runtime** | JRE 11+ required | Built into LibreTune (wasmtime) |
| **Security** | Full JVM permissions | Permission model (ReadTables, WriteConstants, SubscribeChannels, ExecuteActions) |
| **Communication** | JSON-RPC over stdin/stdout | Direct WASM host function calls |
| **Distribution** | JAR files | WASM modules (.wasm files) |

### Resources
- **WASM Plugin System Documentation**: See `SPRINT_3_SUMMARY.md` for technical details
- **Migration Guide**: `docs/src/reference/java-to-wasm-migration.md` (coming soon)
- **Example Plugins**: Reference implementations planned for Sprint 4

---

## Technical Details

### Affected Components

**Backend (Rust)**:
- `crates/libretune-core/plugin-host/` - Java plugin host (19 files, ~2,000 lines)
- `crates/libretune-core/src/plugin/` - JVM subprocess management (4 files, ~1,042 lines)
- Tauri commands: `load_plugin`, `unload_plugin`, `list_plugins`, `get_plugin_ui`, `send_plugin_event`

**Frontend (TypeScript)**:
- `crates/libretune-app/src/components/plugin/` - Java plugin UI (7 files, ~1,878 lines)
- SwingRenderer component for Swing-to-React conversion
- EventBridge for bidirectional JVM communication

**Build System**:
- `crates/libretune-core/plugin-host/build.gradle` - Gradle build for plugin host JAR
- `crates/libretune-app/src-tauri/resources/libretune-plugin-host.jar` - Bundled JAR

**Dependencies**:
- Gson 2.10.1 (JSON serialization)
- SLF4J 2.0.9 (Logging)
- OpenJDK 11+ (Runtime)

### Code Removal Estimate
- **Total**: ~5,112 lines across Java, Rust, TypeScript
- **Files**: 30+ source files
- **Tauri Commands**: 8 commands
- **UI Components**: 7 React components

---

## Rationale

### Why Deprecate?

1. **Maintenance Burden**
   - Requires maintaining separate Java codebase with Gradle build
   - JRE detection logic for Windows/macOS/Linux
   - Swing UI introspection complexity

2. **Security Concerns**
   - Plugins run with full JVM permissions (no sandboxing)
   - No code signing validation
   - No resource limits (memory/CPU)
   - Arbitrary class loading from JAR files

3. **User Experience**
   - External dependency (JRE 11+) adds installation friction
   - Two competing plugin systems create confusion
   - No documentation for Java plugin development

4. **Architectural Redundancy**
   - WASM plugin system provides superior isolation and security
   - Native integration with LibreTune's React UI
   - Single-language backend (Rust) simplifies development

### Why Keep WASM?

1. **No External Dependencies**: wasmtime built into LibreTune
2. **Security**: Permission model + sandboxed execution
3. **Performance**: Native speed, minimal overhead
4. **Developer Experience**: Modern tooling, multiple language options
5. **Consistency**: Single plugin architecture to document and support

---

## Community Feedback

We're actively seeking feedback during the grace period:

1. **Are you currently using Java plugins?** Please let us know which ones.
2. **Would you be willing to rewrite plugins as WASM?** We can provide assistance.
3. **Do you need more time?** We'll consider extending the grace period if needed.

### How to Provide Feedback
- GitHub Issues: [LibreTune/issues](https://github.com/yourusername/LibreTune/issues)
- Community Forum: [TBD]
- Email: [TBD]

---

## Migration Guide (Preview)

Full guide coming in Sprint 4: `docs/src/reference/java-to-wasm-migration.md`

### Quick Comparison

**Java Plugin**:
```java
public class MyPlugin implements TunerStudioPlugin {
    @Override
    public String getId() { return "my-plugin"; }
    
    @Override
    public JComponent getUI(ControllerAccess controller) {
        JPanel panel = new JPanel();
        // Build Swing UI...
        return panel;
    }
}
```

**WASM Plugin (Rust)**:
```rust
#[no_mangle]
pub extern "C" fn init() -> i32 {
    log_message("my-plugin initialized".to_string());
    0
}

#[no_mangle]
pub extern "C" fn execute() -> i32 {
    let table_data = get_table_data("veTable1Tbl".to_string());
    // Process data...
    0
}
```

### Key Steps
1. **Choose WASM-compilable language** (Rust recommended)
2. **Rewrite business logic** (ECU data access, calculations)
3. **Replace Swing UI** with LibreTune native React components (or headless operation)
4. **Declare permissions** in plugin manifest (TOML file)
5. **Test in LibreTune** using Plugin Manager UI

---

## Frequently Asked Questions

### Q: Can I still use LibreTune without plugins?
**A:** Yes. Plugin support is optional. Core functionality (table editing, dashboards, AutoTune, data logging) is unaffected.

### Q: Will WASM plugins be compatible with TunerStudio?
**A:** No. WASM plugins are LibreTune-specific. They cannot run in TunerStudio. This is a intentional divergence to enable better security and integration.

### Q: Can I bundle Java plugins with my LibreTune fork?
**A:** Yes, until the code is removed. The Java plugin system is licensed under the same terms as LibreTune (see LICENSE). However, you'll need to maintain the Java codebase yourself after removal.

### Q: What if I need functionality that only exists in a Java plugin?
**A:** Please open a GitHub issue describing the use case. We may consider adding it as a core feature or creating an official WASM plugin.

### Q: How long is the grace period?
**A:** 2-4 releases (approximately 6-12 months). Actual timeline depends on community feedback.

---

## References

- **WASM Plugin System**: `SPRINT_3_SUMMARY.md`
- **Plugin API Documentation**: `crates/libretune-core/src/plugin_api.rs`
- **Example Plugins**: Coming in Sprint 4
- **TunerStudio Plugin Specification**: For historical reference only (third-party documentation)

---

## Contact

Questions or concerns about this deprecation? Please reach out:
- GitHub: [LibreTune/issues](https://github.com/yourusername/LibreTune/issues)
- Email: [TBD]

---

**Last Updated**: February 4, 2026
