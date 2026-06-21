import { describe, it, expect } from "vitest";
import { toAppError, isConflict } from "@/services/error";

describe("toAppError", () => {
  it("passes through a well-formed AppError", () => {
    const err = { kind: "Conflict", message: "已存在" };
    expect(toAppError(err)).toEqual(err);
  });

  it("wraps an unknown error as Io", () => {
    const result = toAppError("boom");
    expect(result.kind).toBe("Io");
    expect(result.message).toBe("boom");
  });

  it("detects conflict errors", () => {
    expect(isConflict({ kind: "Conflict", message: "x" })).toBe(true);
    expect(isConflict({ kind: "NotFound", message: "x" })).toBe(false);
  });
});
