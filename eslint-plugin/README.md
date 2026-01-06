# eslint-plugin-groq-lint

ESLint plugin for linting GROQ queries that uses [groq-lint](https://github.com/atombender/groq-lint).

## Features

- Lint standalone `.groq` files
- Lint `groq`...`` template literals in JavaScript/TypeScript files
- Performance and correctness checks for GROQ queries

## Installation

```bash
npm install eslint-plugin-groq-lint --save-dev
```

## Usage

### ESLint 9+ (Flat Config)

```js
// eslint.config.js
import groq from "eslint-plugin-groq-lint";

export default [
  // Lint groq`...` in JS/TS files
  groq.configs.recommended,

  // Lint .groq files
  groq.configs["recommended-groq-files"],

  // Or use both:
  // ...groq.configs["recommended-all"],
];
```

### ESLint 8 (Legacy Config)

```json
{
  "plugins": ["groq"],
  "rules": {
    "groq/no-lint-errors": "error"
  },
  "overrides": [
    {
      "files": ["*.groq"],
      "processor": "groq/groq"
    }
  ]
}
```

## Rules

### `groq/no-lint-errors`

Lints GROQ queries in `groq`...`` template literals.

```js
// Bad
const query = groq`*[_type == "post"][2000...2010]`;
//                                    ^^
//                                    Warning: Deep pagination is slow. Consider using cursor-based pagination (e.g., using `_id`).

// Good
const query = groq`*[_type == "post"]`;
```

## License

MIT. See repository root.
