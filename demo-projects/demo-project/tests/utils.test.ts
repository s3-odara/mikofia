import { formatString, calculateSum } from "../src/utils";

describe("utils", () => {
  it("should format string", () => {
    expect(formatString("  HELLO  ")).toBe("hello");
  });

  it("should calculate sum", () => {
    expect(calculateSum(2, 3)).toBe(5);
  });
});
