/**
 * Shared type definitions for eslint-plugin-groq.
 */

export interface LintFinding {
  start: number;
  end: number;
  message: string;
  severity: "error" | "warning" | "suggestion";
  rule_id: string;
}

export interface LintResult {
  success: boolean;
  findings: LintFinding[];
  error?: string;
}

export interface Location {
  line: number;
  column: number;
}
