/**
 * ESLint plugin for linting GROQ queries.
 *
 * Supports:
 * - Standalone .groq files
 * - groq`...` template literals in JS/TS files
 */

import type { ESLint, Linter } from "eslint";
import groqProcessor from "./processor";
import groqLintRule from "./groq-lint-rule";

interface PluginConfig extends Linter.Config {
  plugins?: Record<string, ESLint.Plugin>;
}

interface Plugin extends ESLint.Plugin {
  configs: {
    recommended: PluginConfig;
    "recommended-groq-files": PluginConfig;
    "recommended-all": PluginConfig[];
  };
}

const plugin: Plugin = {
  meta: {
    name: "eslint-plugin-groq",
    version: "0.1.0",
  },

  processors: {
    groq: groqProcessor,
  },

  rules: {
    "no-lint-errors": groqLintRule,
  },

  configs: {
    recommended: {} as PluginConfig,
    "recommended-groq-files": {} as PluginConfig,
    "recommended-all": [],
  },
};

// Add recommended config for flat config (ESLint 9+)
plugin.configs.recommended = {
  plugins: {
    groq: plugin,
  },
  rules: {
    "groq/no-lint-errors": "warn",
  },
};

// Add config for .groq files
plugin.configs["recommended-groq-files"] = {
  plugins: {
    groq: plugin,
  },
  files: ["**/*.groq"],
  processor: "groq/groq",
};

// Combined config for both JS and .groq files
plugin.configs["recommended-all"] = [
  plugin.configs.recommended,
  plugin.configs["recommended-groq-files"],
];

export = plugin;
