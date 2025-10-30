# @mikofia/types

TypeScript type definitions for [Mikofia](https://github.com/med0rei/mikofia) file structure validator.

## Installation

```bash
npm install --save-dev @mikofia/types
```

## Usage

### JavaScript with JSDoc

Use JSDoc comments to get type checking and IDE autocomplete in JavaScript files:

```javascript
// @ts-check
/// <reference types="@mikofia/types" />

/** @type {import('@mikofia/types').Config} */
const config = {
  nodes: [
    {
      path: "src",
      existence: "required",
      kind: "directory",
      children: [
        {
          path: "*.rs",
          existence: "required",
          kind: "file",
        },
      ],
      strict: true,
    },
  ],
};

export default config;
```

### Custom Rules

Define custom validation rules with full type safety:

```javascript
// @ts-check

/** @type {import('@mikofia/types').RuleFunction} */
async function checkNamingConvention(ctx) {
  if (!ctx.name.match(/^[a-z0-9-]+$/)) {
    return {
      type: "fail",
      violation: {
        key: "invalid-naming",
        message: `Invalid name: ${ctx.name}. Use lowercase, numbers, and hyphens only.`,
      },
    };
  }
  return { type: "pass" };
}

/** @type {import('@mikofia/types').Config} */
const config = {
  nodes: [
    {
      path: "src",
      existence: "required",
      kind: "directory",
      rules: [checkNamingConvention],
    },
  ],
};

export default config;
```

## Type Reference

See [types.d.ts](./types.d.ts) for complete type definitions.

### Key Types

- `Config` - Main configuration object
- `Node` - File or directory node in the structure tree
- `Existence` - "required" | "optional" | "absent"
- `NodeKind` - "file" | "directory" | "any"
- `RuleFunction` - Custom validation rule function
- `EvaluationContext` - Context provided to rules
- `RuleResult` - Result of rule evaluation

## License

MIT
