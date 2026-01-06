/**
 * WASM loader for groq-lint.
 * The wasm-pack generated code handles synchronous loading in Node.js.
 */

import type { LintResult } from "./types";

// eslint-disable-next-line @typescript-eslint/no-require-imports
const wasm = require("../wasm/groq_lint.js") as { lint: (query: string) => string };

/**
 * Lint a GROQ query string.
 */
export function lint(query: string): LintResult {
  const resultJson = wasm.lint(query);
  return JSON.parse(resultJson) as LintResult;
}
