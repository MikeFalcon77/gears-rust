import { expect, it } from "vitest";
import { parseValue, shouldCelebrate } from "./model";
it("validates editable scalar values without accepting empty or nonfinite numbers", () => {
  expect(parseValue("boolean", "false")).toBe(false);
  expect(parseValue("number", "2.5")).toBe(2.5);
  expect(() => parseValue("number", "")).toThrow();
  expect(() => parseValue("number", "Infinity")).toThrow();
  expect(() => parseValue("boolean", "maybe")).toThrow();
  expect(() => parseValue("string", "x".repeat(1001))).toThrow();
});

it("celebrates a live false-to-true transition, never an initial true value or an old replay", () => {
  const mounted = Date.parse("2026-10-18T10:00:00Z");
  expect(shouldCelebrate(null, true, "2026-10-18T10:01:00Z", mounted)).toBe(
    false,
  );
  expect(shouldCelebrate(true, true, "2026-10-18T10:01:00Z", mounted)).toBe(
    false,
  );
  expect(shouldCelebrate(false, true, "2026-10-18T09:59:00Z", mounted)).toBe(
    false,
  );
  expect(shouldCelebrate(false, true, "2026-10-18T10:01:00Z", mounted)).toBe(
    true,
  );
  expect(shouldCelebrate(false, false, "2026-10-18T10:01:00Z", mounted)).toBe(
    false,
  );
});
