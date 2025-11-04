// @ts-check
/// <reference path="../../mikofia-deno/types.d.ts" />

/**
 * Custom Rule 1: Check kebab-case naming convention for TypeScript files
 * TypeScript files should use kebab-case (e.g., my-file.ts), not snake_case
 * @type {import('../../mikofia-deno/types.d.ts').RuleFunction}
 */
async function checkKebabCase(ctx) {
  // Only check .ts files
  if (ctx.extension !== "ts") {
    return { type: "skip", reason: "Not a TypeScript file" };
  }

  // Extract filename without extension
  const filename = ctx.name.replace(/\.ts$/, "");

  // Check if it contains underscore (snake_case)
  if (filename.includes("_")) {
    return {
      type: "fail",
      violation: {
        key: "invalid-naming-convention",
        message: `File "${ctx.name}" uses snake_case. Use kebab-case instead (e.g., ${filename.replace(
          /_/g,
          "-",
        )}.ts)`,
      },
    };
  }

  // Check if it's valid kebab-case (lowercase letters, numbers, hyphens)
  if (!/^[a-z0-9-]+$/.test(filename)) {
    return {
      type: "fail",
      violation: {
        key: "invalid-naming-convention",
        message: `File "${ctx.name}" doesn't follow kebab-case convention. Use only lowercase letters, numbers, and hyphens.`,
      },
    };
  }

  return { type: "pass" };
}

/**
 * Custom Rule 2: Require README.md in root directory
 * Checks if siblings contain a README file
 * @type {import('../../mikofia-deno/types.d.ts').RuleFunction}
 */
async function requireReadme(ctx) {
  const hasReadme = ctx.siblings.some(
    (s) => s.name.toLowerCase() === "readme.md" && s.is_file,
  );

  if (!hasReadme) {
    return {
      type: "fail",
      violation: {
        key: "missing-readme",
        message: `Directory "${ctx.name}" must contain a README.md file`,
      },
    };
  }

  return { type: "pass" };
}

/**
 * Custom Rule 3: Require corresponding test file
 * For each .ts file in src/, there should be a .test.ts file in tests/
 * @type {import('../../mikofia-deno/types.d.ts').RuleFunction}
 */
async function requireCorrespondingTest(ctx) {
  // Only check .ts files, skip .test.ts files
  if (ctx.extension !== "ts" || ctx.name.endsWith(".test.ts")) {
    return { type: "skip", reason: "Not a source TypeScript file" };
  }

  // Only check files in src/ directory
  if (!ctx.parent || ctx.parent.name !== "src") {
    return { type: "skip", reason: "Not in src/ directory" };
  }

  // Construct expected test file path
  const basename = ctx.name.replace(/\.ts$/, "");
  const testFilePath = ctx.path
    .replace(/\/src\//, "/tests/")
    .replace(/\.ts$/, ".test.ts");

  // Check if test file exists using safe fs API
  try {
    const exists = ctx.fs.exists(testFilePath);
    if (!exists) {
      return {
        type: "fail",
        violation: {
          key: "missing-test-file",
          message: `Source file "${ctx.name}" is missing corresponding test file: tests/${basename}.test.ts`,
        },
      };
    }
  } catch (error) {
    return {
      type: "fail",
      violation: {
        key: "test-check-error",
        message: `Failed to check for test file: ${error.message}`,
      },
    };
  }

  return { type: "pass" };
}

/**
 * Custom Rule 4: Require minimum number of documentation files
 * docs/ directory should contain at least one .md file
 * @type {import('../../mikofia-deno/types.d.ts').RuleFunction}
 */
async function requireMinimumDocs(ctx) {
  // Only check docs directory
  if (ctx.name !== "docs" || !ctx.siblings) {
    return { type: "skip", reason: "Not docs directory" };
  }

  const mdFiles = ctx.siblings.filter(
    (s) => s.name.endsWith(".md") && s.is_file,
  );

  if (mdFiles.length < 1) {
    return {
      type: "fail",
      violation: {
        key: "insufficient-docs",
        message: `Directory "docs" must contain at least 1 markdown file, found ${mdFiles.length}`,
      },
    };
  }

  return { type: "pass" };
}

/**
 * Custom Rule 5: Check for forbidden file name patterns
 * Files should not contain "temp", "tmp", or "backup" in their names
 * @type {import('../../mikofia-deno/types.d.ts').RuleFunction}
 */
async function checkForbiddenNames(ctx) {
  const forbiddenPatterns = ["temp", "tmp", "backup"];
  const lowerName = ctx.name.toLowerCase();

  for (const pattern of forbiddenPatterns) {
    if (lowerName.includes(pattern)) {
      return {
        type: "fail",
        violation: {
          key: "forbidden-filename-pattern",
          message: `File "${ctx.name}" contains forbidden pattern "${pattern}". Temporary and backup files should not be committed.`,
        },
      };
    }
  }

  return { type: "pass" };
}

/** @type {import('../../mikofia-deno/types.d.ts').Config} */
const config = {
  nodes: [
    {
      path: "README.md",
      existence: "required",
      kind: "file",
    },
    {
      path: "src",
      existence: "required",
      kind: "directory",
      children: [
        {
          path: "*.ts",
          existence: "optional",
          kind: "file",
          rules: [checkKebabCase, requireCorrespondingTest],
        },
      ],
    },
    {
      path: "tests",
      existence: "required",
      kind: "directory",
      children: [
        {
          path: "*.test.ts",
          existence: "optional",
          kind: "file",
        },
      ],
    },
    {
      path: "docs",
      existence: "required",
      kind: "directory",
      rules: [requireMinimumDocs],
      children: [
        {
          path: "*.md",
          existence: "optional",
          kind: "file",
        },
      ],
    },
    // Check all files for forbidden patterns
    {
      path: "*",
      existence: "optional",
      kind: "any",
      rules: [checkForbiddenNames],
    },
  ],
};

export default config;
