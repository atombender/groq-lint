const groq = require("../dist/index.js");
const tseslint = require("typescript-eslint");

module.exports = [
  // TypeScript support
  ...tseslint.configs.recommended,

  // Lint groq`...` template tags in JS/TS files
  {
    ...groq.configs.recommended,
    files: ["**/*.js", "**/*.ts", "**/*.tsx"],
  },

  // Lint .groq files
  groq.configs["recommended-groq-files"],
];
