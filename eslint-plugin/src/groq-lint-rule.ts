/**
 * ESLint rule that lints groq`...` template literals and defineQuery() calls in JS/TS files.
 */

import type { Rule, SourceCode } from "eslint";
import type {
  Node,
  TemplateLiteral,
  TaggedTemplateExpression,
  CallExpression,
  Literal,
} from "estree";
import { lint } from "./wasm-loader";
import type { Location } from "./types";

interface RuleOptions {
  tagNames?: string[];
  functionNames?: string[];
}

/**
 * Convert a character offset within the template to source location.
 */
function templateOffsetToLocation(
  node: TemplateLiteral,
  offset: number,
  sourceCode: SourceCode
): Location {
  const range = node.range;
  if (!range) {
    return { line: 1, column: 0 };
  }

  // Get the start of the template content (after the opening backtick)
  const templateStart = range[0] + 1;
  const absoluteOffset = templateStart + offset;

  const loc = sourceCode.getLocFromIndex(absoluteOffset);
  return { line: loc.line, column: loc.column };
}

/**
 * Extract the raw string from a template literal.
 * For now, we only handle simple templates without expressions.
 */
function extractTemplateString(node: TemplateLiteral): string | null {
  if (node.expressions.length > 0) {
    return null;
  }

  return node.quasis.map((quasi) => quasi.value.raw).join("");
}

/**
 * Get the tag name from a tagged template expression.
 */
function getTagName(node: TaggedTemplateExpression): string | null {
  const tag = node.tag;

  if (tag.type === "Identifier") {
    return tag.name;
  }

  if (
    tag.type === "MemberExpression" &&
    tag.property.type === "Identifier"
  ) {
    return tag.property.name;
  }

  return null;
}

/**
 * Get the function name from a call expression.
 */
function getFunctionName(node: CallExpression): string | null {
  const callee = node.callee;

  if (callee.type === "Identifier") {
    return callee.name;
  }

  if (
    callee.type === "MemberExpression" &&
    callee.property.type === "Identifier"
  ) {
    return callee.property.name;
  }

  return null;
}

/**
 * Convert a character offset within a string literal to source location.
 */
function stringOffsetToLocation(
  node: Literal,
  offset: number,
  sourceCode: SourceCode
): Location {
  const range = node.range;
  if (!range) {
    return { line: 1, column: 0 };
  }

  // Get the start of the string content (after the opening quote)
  const stringStart = range[0] + 1;
  const absoluteOffset = stringStart + offset;

  const loc = sourceCode.getLocFromIndex(absoluteOffset);
  return { line: loc.line, column: loc.column };
}

const rule: Rule.RuleModule = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Lint GROQ queries in groq`...` template literals and defineQuery() calls",
      category: "Possible Errors",
      recommended: true,
    },
    schema: [
      {
        type: "object",
        properties: {
          tagNames: {
            type: "array",
            items: { type: "string" },
            default: ["groq"],
            description: "Tag names to treat as GROQ queries",
          },
          functionNames: {
            type: "array",
            items: { type: "string" },
            default: ["defineQuery"],
            description: "Function names to treat as GROQ query wrappers",
          },
        },
        additionalProperties: false,
      },
    ],
  },

  create(context: Rule.RuleContext): Rule.RuleListener {
    const options = (context.options[0] || {}) as RuleOptions;
    const tagNames = new Set(options.tagNames || ["groq"]);
    const functionNames = new Set(options.functionNames || ["defineQuery"]);
    const sourceCode = context.sourceCode;

    /**
     * Lint a GROQ query from a template literal.
     */
    function lintTemplateLiteral(node: Node, template: TemplateLiteral): void {
      const query = extractTemplateString(template);
      if (query === null) {
        context.report({
          node,
          message:
            "GROQ template with interpolations cannot be statically analyzed. Consider using parameters ($param) instead.",
        });
        return;
      }

      const result = lint(query);

      if (!result.success) {
        context.report({
          node,
          message: `GROQ parse error: ${result.error}`,
        });
        return;
      }

      for (const finding of result.findings) {
        const startLoc = templateOffsetToLocation(
          template,
          finding.start,
          sourceCode
        );
        const endLoc = templateOffsetToLocation(
          template,
          finding.end,
          sourceCode
        );

        context.report({
          loc: {
            start: startLoc,
            end: endLoc,
          },
          message: `[${finding.rule_id}] ${finding.message}`,
        });
      }
    }

    /**
     * Lint a GROQ query from a string literal.
     */
    function lintStringLiteral(node: Node, literal: Literal): void {
      if (typeof literal.value !== "string") {
        return;
      }

      const query = literal.value;
      const result = lint(query);

      if (!result.success) {
        context.report({
          node,
          message: `GROQ parse error: ${result.error}`,
        });
        return;
      }

      for (const finding of result.findings) {
        const startLoc = stringOffsetToLocation(
          literal,
          finding.start,
          sourceCode
        );
        const endLoc = stringOffsetToLocation(literal, finding.end, sourceCode);

        context.report({
          loc: {
            start: startLoc,
            end: endLoc,
          },
          message: `[${finding.rule_id}] ${finding.message}`,
        });
      }
    }

    return {
      TaggedTemplateExpression(node: Node) {
        const taggedNode = node as TaggedTemplateExpression;
        const tagName = getTagName(taggedNode);

        if (!tagName || !tagNames.has(tagName)) {
          return;
        }

        lintTemplateLiteral(node, taggedNode.quasi);
      },

      CallExpression(node: Node) {
        const callNode = node as CallExpression;
        const funcName = getFunctionName(callNode);

        if (!funcName || !functionNames.has(funcName)) {
          return;
        }

        // Check the first argument
        const arg = callNode.arguments[0];
        if (!arg) {
          return;
        }

        if (arg.type === "Literal" && typeof arg.value === "string") {
          // defineQuery("*[_type == 'post']")
          lintStringLiteral(node, arg);
        } else if (arg.type === "TemplateLiteral") {
          // defineQuery(`*[_type == "post"]`)
          lintTemplateLiteral(node, arg);
        }
        // Note: defineQuery(groq`...`) is handled by TaggedTemplateExpression
      },
    };
  },
};

export default rule;
