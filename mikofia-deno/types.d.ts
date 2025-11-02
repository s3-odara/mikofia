/**
 * Mikofia TypeScript Type Definitions
 *
 * These types can be used in JavaScript config files with JSDoc comments:
 * @example
 * // @ts-check
 * /// <reference path="node_modules/@mikofia/types/types.d.ts" />
 *
 * /** @type {import('@mikofia/types').Config} *\/
 * const config = { ... };
 * export default config;
 */

/**
 * Specifies whether a file or directory must exist, may exist, or must not exist.
 */
export type Existence = "required" | "optional" | "absent";

/**
 * Specifies the expected type of a filesystem item.
 */
export type NodeKind = "file" | "directory" | "any";

/**
 * Information about a file's parent directory.
 */
export interface ParentInfo {
  /** Absolute path to the parent directory */
  path: string;
  /** Name of the parent directory */
  name: string;
}

/**
 * Information about a sibling file or directory.
 */
export interface SiblingInfo {
  /** Name of the sibling */
  name: string;
  /** Whether the sibling is a file */
  is_file: boolean;
  /** Whether the sibling is a directory */
  is_directory: boolean;
}

/**
 * Context provided to custom validation rules.
 */
export interface EvaluationContext {
  /** Absolute path to the file or directory being validated */
  path: string;
  /** Name of the file or directory */
  name: string;
  /** File extension (if applicable) */
  extension?: string;
  /** Information about the parent directory */
  parent?: ParentInfo;
  /** List of sibling files and directories */
  siblings: SiblingInfo[];
}

/**
 * Result of a validation rule check.
 */
export type RuleResult =
  | { type: "pass" }
  | { type: "fail"; violation: { key: string; message: string } }
  | { type: "skip"; reason: string };

/**
 * Custom validation rule function.
 *
 * @param ctx - Context information about the file or directory being validated
 * @returns Promise or synchronous result indicating pass, fail, or skip
 *
 * @example
 * async function checkNamingConvention(ctx) {
 *   if (!ctx.name.match(/^[a-z0-9-]+$/)) {
 *     return {
 *       type: "fail",
 *       violation: {
 *         key: "invalid-naming",
 *         message: `Invalid name: ${ctx.name}. Use lowercase, numbers, and hyphens only.`
 *       }
 *     };
 *   }
 *   return { type: "pass" };
 * }
 */
export type RuleFunction = (
  ctx: EvaluationContext,
) => RuleResult | Promise<RuleResult>;

/**
 * A node in the file structure tree.
 */
export interface Node {
  /**
   * Path to the file or directory, relative to parent node.
   * Can include glob patterns like "*.rs" or "test?.txt"
   */
  path: string;

  /**
   * Whether this item must exist, may exist, or must not exist.
   * @default "optional"
   */
  existence?: Existence;

  /**
   * Expected type of this item.
   * @default "any"
   */
  kind?: NodeKind;

  /**
   * Child nodes (for directories).
   * @default []
   */
  children?: Node[];

  /**
   * If true, only explicitly listed children are allowed in this directory.
   * Any unlisted files or directories will be reported as violations.
   * @default null
   */
  strict?: boolean | null;

  /**
   * Glob patterns to ignore within this node's scope.
   * These patterns are combined with parent and global ignore patterns.
   * Patterns match recursively (including subdirectories), similar to .gitignore behavior.
   * @default []
   * @example ["*.test.tsx", "*.log", "temp"]
   */
  ignore?: string[];

  /**
   * Custom validation rules to apply to this node.
   * @default []
   */
  rules?: RuleFunction[];
}

/**
 * Main configuration object for mikofia.
 */
export interface Config {
/**
 * Glob patterns for files and directories to ignore.
 * These patterns will be excluded from all checks, including strict mode.
 * @default []
 * @example ["node_modules", "target", "*.log", "**/ .DS_Store"]
   */
  ignore?: string[]

/** Root nodes of the file structure tree */
nodes: Node[];
}
