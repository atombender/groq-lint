/**
 * ESLint processor for .groq files.
 * Treats the entire file content as a GROQ query.
 */

import * as fs from "fs";
import type { Linter } from "eslint";
import { lint } from "./wasm-loader";
import type { Location } from "./types";

/**
 * Convert a character offset to line/column.
 */
function offsetToLocation(text: string, offset: number): Location {
  let line = 1;
  let column = 0;
  for (let i = 0; i < offset && i < text.length; i++) {
    if (text[i] === "\n") {
      line++;
      column = 0;
    } else {
      column++;
    }
  }
  return { line, column };
}

const processor: Linter.Processor = {
  meta: {
    name: "groq",
    version: "0.1.0",
  },

  /**
   * Process .groq files - extract the query for linting.
   * We return a virtual JS file that our rule can process.
   */
  preprocess(text: string, filename: string): Linter.ProcessorFile[] {
    return [{ text, filename }];
  },

  /**
   * Post-process: run the GROQ linter and return ESLint messages.
   */
  postprocess(
    _messages: Linter.LintMessage[][],
    filename: string
  ): Linter.LintMessage[] {
    // Read the original file content
    let text: string;
    try {
      text = fs.readFileSync(filename, "utf8");
    } catch {
      return [];
    }

    const result = lint(text);

    if (!result.success) {
      return [
        {
          ruleId: "groq/parse-error",
          severity: 2,
          message: result.error || "Failed to parse GROQ query",
          line: 1,
          column: 1,
        },
      ];
    }

    return result.findings.map((finding) => {
      const start = offsetToLocation(text, finding.start);
      const end = offsetToLocation(text, finding.end);

      return {
        ruleId: `groq/${finding.rule_id}`,
        severity: 1 as const,
        message: finding.message,
        line: start.line,
        column: start.column + 1,
        endLine: end.line,
        endColumn: end.column + 1,
      };
    });
  },

  supportsAutofix: false,
};

export default processor;
