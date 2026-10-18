import { type GearRuntime, type EffectiveValue, scopeFor } from "@gears/ui-sdk";
export const effective = <T>(
  runtime: GearRuntime,
  key: string,
  signal?: AbortSignal,
) =>
  runtime.api.post<EffectiveValue<T>>(
    runtime.config.settings.effectivePath,
    { key, scope: scopeFor(runtime.identity) },
    AbortSignal.any([
      runtime.signal,
      AbortSignal.timeout(15000),
      ...(signal ? [signal] : []),
    ]),
  );
export function parseValue(kind: string, input: string): unknown {
  if (kind === "boolean") {
    if (!["true", "false"].includes(input))
      throw Error("Choose true or false.");
    return input === "true";
  }
  if (kind === "number") {
    if (!input.trim()) throw Error("Enter a number.");
    const value = Number(input);
    if (!Number.isFinite(value)) throw Error("Enter a finite number.");
    return value;
  }
  if (input.length > 1000) throw Error("Use at most 1,000 characters.");
  return input;
}

export const shouldCelebrate = (
  previous: boolean | null,
  next: boolean,
  occurredAt: string,
  mountedAt: number,
) => previous === false && next === true && Date.parse(occurredAt) >= mountedAt;
