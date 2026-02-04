import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./LuaConsole.css";

interface LuaExecutionResult {
  stdout: string;
  return_value?: string | null;
}

interface LuaConsoleProps {
  isOpen?: boolean;
}

export function LuaConsole({ isOpen = true }: LuaConsoleProps) {
  const [script, setScript] = useState("print('Hello from Lua')");
  const [output, setOutput] = useState<string[]>([]);
  const [isRunning, setIsRunning] = useState(false);

  if (!isOpen) return null;

  const handleRun = async () => {
    setIsRunning(true);
    try {
      const result = await invoke<LuaExecutionResult>("run_lua_script", { script });
      const lines: string[] = [];
      if (result.stdout) {
        lines.push(...result.stdout.split("\n").filter(Boolean));
      }
      if (result.return_value !== undefined && result.return_value !== null) {
        lines.push(`=> ${result.return_value}`);
      }
      setOutput((prev) => [...prev, ...lines]);
    } catch (e) {
      setOutput((prev) => [...prev, `Error: ${String(e)}`]);
    } finally {
      setIsRunning(false);
    }
  };

  const handleClear = () => setOutput([]);

  return (
    <div className="lua-console">
      <div className="lua-console-header">
        <div className="lua-console-title">Lua Console</div>
        <div className="lua-console-actions">
          <button className="lua-btn" onClick={handleRun} disabled={isRunning}>
            {isRunning ? "Running..." : "Run"}
          </button>
          <button className="lua-btn secondary" onClick={handleClear}>
            Clear
          </button>
        </div>
      </div>

      <textarea
        className="lua-console-editor"
        value={script}
        onChange={(e) => setScript(e.target.value)}
        spellCheck={false}
      />

      <div className="lua-console-output">
        {output.length === 0 ? (
          <div className="lua-console-empty">No output yet.</div>
        ) : (
          output.map((line, idx) => (
            <div key={`${idx}-${line}`} className="lua-console-line">
              {line}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
