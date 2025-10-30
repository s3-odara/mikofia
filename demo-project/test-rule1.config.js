// Test Rule 1: kebab-case only
async function checkKebabCase(ctx) {
  if (ctx.extension !== "ts") {
    return { type: "skip", reason: "Not a TypeScript file" };
  }

  const filename = ctx.name.replace(/\.ts$/, "");
  if (filename.includes("_")) {
    return {
      type: "fail",
      violation: {
        key: "invalid-naming-convention",
        message: `File "${ctx.name}" uses snake_case. Use kebab-case instead.`,
      },
    };
  }

  return { type: "pass" };
}

const config = {
  nodes: [
    {
      path: "src",
      kind: "directory",
      children: [
        {
          path: "*.ts",
          kind: "file",
          rules: [checkKebabCase],
        },
      ],
    },
  ],
};

export default config;
