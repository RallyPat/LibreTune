import { useState, useEffect, useRef } from 'react';
import './IndicatorRow.css';
import { useRealtimeStore } from '../../stores/realtimeStore';

export interface FrontPageIndicator {
  expression: string;
  label_off: string;
  label_on: string;
  bg_off: string;
  fg_off: string;
  bg_on: string;
  fg_on: string;
}

interface IndicatorRowProps {
  indicators: FrontPageIndicator[];
  constantValues?: Record<string, number>;
  columnCount?: number | 'auto';
  fillEmptyCells?: boolean;
  textFitMode?: 'scale' | 'wrap';
}

/**
 * Evaluates a simple boolean expression against realtime data.
 * Supports: variable names, comparisons (<, >, <=, >=, ==, !=), 
 * bitwise AND (&), logical AND (&&), logical OR (||), parentheses.
 */
function evaluateIndicatorExpression(
  expression: string,
  data: Record<string, number>,
  constants: Record<string, number> = {}
): boolean {
  // Merge realtime data with constants (realtime takes precedence)
  const context = { ...constants, ...data };
  
  try {
    // Clean up the expression
    let expr = expression.trim();
    
    // Handle simple variable name (truthy check)
    if (/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(expr)) {
      const val = context[expr];
      return val !== undefined && val !== 0;
    }
    
    // Replace variable names with their values
    // This is a simple replacement - more complex expressions would need a proper parser
    const tokens = expr.match(/([a-zA-Z_][a-zA-Z0-9_]*|[0-9.]+|&&|\|\||[<>=!&]+|[()]+)/g);
    if (!tokens) return false;
    
    let result = '';
    for (const token of tokens) {
      if (/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(token)) {
        const val = context[token];
        result += val !== undefined ? val : 0;
      } else {
        result += token;
      }
    }
    
    // Safely evaluate the expression
    // Replace bitwise & with JavaScript's bitwise AND behavior for truthiness
    // Note: This is a simplified evaluator - complex expressions may need more robust parsing
    
    // Convert bitwise AND results to boolean
    // e.g., "status & 8" should be truthy if bit 3 is set
    const evalFn = new Function('return (' + result + ') ? true : false');
    return evalFn();
  } catch (e) {
    console.warn(`Failed to evaluate indicator expression: ${expression}`, e);
    return false;
  }
}

/**
 * Adjusts indicator colors for proper contrast in dark themes.
 * INI files often define white backgrounds with gray text which is unreadable on dark themes.
 */
function adjustColorsForTheme(bgColor: string, fgColor: string): { bg: string; fg: string } {
  const theme = document.documentElement.dataset.theme || 'dark';
  
  // Only adjust for dark themes
  if (theme === 'light') {
    return { bg: bgColor, fg: fgColor };
  }
  
  // Check if background is light (white, near-white, or light gray)
  const isLightBg = /^(white|#fff|#ffffff|#f[0-9a-f]{5}|#e[0-9a-f]{5}|#d[0-9a-f]{5}|#c[0-9a-f]{5})$/i.test(bgColor.trim()) ||
    /rgb\s*\(\s*2[0-5]\d/i.test(bgColor);
  
  // Check if it's a status color (red, green, yellow) - these should keep their colors
  const isStatusColor = /red|green|yellow|#ff0000|#00ff00|#ffff00|#ff4444|#44ff44|#ffff44/i.test(bgColor);
  
  if (isLightBg && !isStatusColor) {
    // Convert light "off" state indicators to dark theme appropriate colors
    return { bg: '#3a3a3a', fg: '#b0b0b0' };
  }
  
  return { bg: bgColor, fg: fgColor };
}

// Minimum cell width in pixels (must match CSS minmax value)
const MIN_CELL_WIDTH = 110;

export default function IndicatorRow({ 
  indicators, 
  constantValues = {},
  columnCount = 'auto',
  fillEmptyCells = false,
  textFitMode = 'scale',
}: IndicatorRowProps) {
  // Get realtime data from Zustand store
  const realtimeData = useRealtimeStore((state) => state.channels);
  
  const containerRef = useRef<HTMLDivElement>(null);
  const [actualCols, setActualCols] = useState(12);

  // Use ResizeObserver to detect actual column count when using auto-columns
  useEffect(() => {
    if (columnCount !== 'auto' || !containerRef.current) return;

    const updateColumnCount = () => {
      if (containerRef.current) {
        const width = containerRef.current.offsetWidth;
        // Account for gap (1px) and padding
        const cols = Math.max(1, Math.floor(width / (MIN_CELL_WIDTH + 1)));
        setActualCols(cols);
      }
    };

    // Initial calculation
    updateColumnCount();

    const resizeObserver = new ResizeObserver(updateColumnCount);
    resizeObserver.observe(containerRef.current);

    return () => resizeObserver.disconnect();
  }, [columnCount]);

  if (!indicators || indicators.length === 0) {
    return null;
  }

  // Determine column class for CSS grid
  const columnClass = columnCount === 'auto' 
    ? 'auto-columns' 
    : `cols-${columnCount}`;

  // Text fit mode class
  const textClass = textFitMode === 'wrap' ? 'text-wrap' : 'text-scale';

  // Calculate empty cells needed for fill mode
  // Use actualCols (from ResizeObserver) when in auto mode, otherwise use explicit column count
  const cols = typeof columnCount === 'number' ? columnCount : actualCols;
  const remainder = indicators.length % cols;
  const emptyCellCount = fillEmptyCells && remainder > 0 ? cols - remainder : 0;

  return (
    <div className="indicator-row-container" ref={containerRef}>
      <div className={`indicator-row ${columnClass} ${textClass}`}>
        {indicators.map((indicator, index) => {
          const isOn = evaluateIndicatorExpression(indicator.expression, realtimeData, constantValues);
          const label = isOn ? indicator.label_on : indicator.label_off;
          const rawBg = isOn ? indicator.bg_on : indicator.bg_off;
          const rawFg = isOn ? indicator.fg_on : indicator.fg_off;
          const { bg: bgColor, fg: fgColor } = adjustColorsForTheme(rawBg, rawFg);

          return (
            <div
              key={index}
              className="indicator-item"
              style={{
                backgroundColor: bgColor,
                color: fgColor,
                borderColor: isOn ? '#222' : '#444',
              }}
              title={`Expression: ${indicator.expression}\nState: ${isOn ? 'ON' : 'OFF'}`}
            >
              <span className="indicator-label">{label}</span>
            </div>
          );
        })}
        {/* Empty filler cells */}
        {Array.from({ length: emptyCellCount }).map((_, i) => (
          <div key={`empty-${i}`} className="indicator-item empty" />
        ))}
      </div>
    </div>
  );
}
