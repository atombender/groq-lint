/**
 * ESLint rule that lints groq`...` template literals in JS/TS files.
 */

import type { Rule, SourceCode } from "eslint";
import type { Node, TemplateLiteral, TaggedTemplateExpression } from "estree";
import { lint } from "./wasm-loader";
import type { Location } from "./types";

interface RuleOptions {
  tagNames?: string[];
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

const rule: Rule.RuleModule = {
  meta: {
    type: "problem",
    docs: {
      description: "Lint GROQ queries in groq`...` template literals",
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
        },
        additionalProperties: false,
      },
    ],
  },

  create(context: Rule.RuleContext): Rule.RuleListener {
    const options = (context.options[0] || {}) as RuleOptions;
    const tagNames = new Set(options.tagNames || ["groq"]);
    const sourceCode = context.sourceCode;

    return {
      TaggedTemplateExpression(node: Node) {
        const taggedNode = node as TaggedTemplateExpression;
        const tagName = getTagName(taggedNode);

        if (!tagName || !tagNames.has(tagName)) {
          return;
        }

        const query = extractTemplateString(taggedNode.quasi);
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
            taggedNode.quasi,
            finding.start,
            sourceCode
          );
          const endLoc = templateOffsetToLocation(
            taggedNode.quasi,
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
      },
    };
  },
};

export default rule;
