import type { AppError, AppErrorKind } from "@/bindings/types";

/** 将后端抛出的错误归一化为 AppError 形状。 */
export function toAppError(e: unknown): AppError {
  if (
    e &&
    typeof e === "object" &&
    "kind" in e &&
    "message" in e &&
    typeof (e as Record<string, unknown>).message === "string"
  ) {
    return e as AppError;
  }
  return { kind: "Io" as AppErrorKind, message: String(e) };
}

export function isConflict(e: unknown): boolean {
  return toAppError(e).kind === "Conflict";
}
