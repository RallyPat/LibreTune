/**
 * Evaluates a simple boolean INI indicator expression against realtime + constant values.
 */
export function evaluateIndicatorExpression(
  expression: string,
  data: Record<string, number>,
  constants: Record<string, number> = {},
): boolean {
  const context = { ...constants, ...data };

  try {
    const expr = expression.trim();

    if (/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(expr)) {
      const val = context[expr];
      return val !== undefined && val !== 0;
    }

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

    const evalFn = new Function('return (' + result + ') ? true : false');
    return evalFn();
  } catch {
    return false;
  }
}

export function extractExpressionVariables(expressions: string[]): string[] {
  const vars = new Set<string>();
  for (const expression of expressions) {
    const tokens = expression.match(/[a-zA-Z_][a-zA-Z0-9_]*/g);
    tokens?.forEach((token) => vars.add(token));
  }
  return Array.from(vars);
}
