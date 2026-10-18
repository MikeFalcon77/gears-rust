import type { ComponentType } from "react";
import type { QueryClient } from "@tanstack/react-query";
export interface Identity {
  subject_id: string;
  tenant_id: string;
}
export interface RuntimeConfig {
  apiBase: string;
  productName: string;
  firebase: Record<string, string>;
  portraits?: Record<string, string>;
}
export interface Problem {
  status: number;
  title: string;
  detail?: string;
  context?: Record<string, unknown>;
}
export class ApiError extends Error {
  constructor(public problem: Problem) {
    super(problem.detail || problem.title);
  }
}
export type TokenProvider = (refresh?: boolean) => Promise<string>;
export class ApiClient {
  constructor(
    private base: string,
    private token: TokenProvider,
    private lifecycle: AbortSignal,
  ) {
    if (
      !base.startsWith("/") ||
      base.startsWith("//") ||
      base.includes("\\") ||
      base.includes("?") ||
      base.includes("#")
    )
      throw Error("API base must be a same-origin path");
  }
  async request(route: string, options: RequestInit = {}): Promise<Response> {
    if (
      !route.startsWith("/") ||
      route.startsWith("//") ||
      route.includes("\\") ||
      route.includes("#") ||
      route.split(/[/?]/).includes("..")
    )
      throw Error("Invalid API route");
    const signal = options.signal
      ? AbortSignal.any([this.lifecycle, options.signal])
      : this.lifecycle;
    const headers = new Headers(options.headers);
    headers.set("Authorization", "Bearer " + (await this.token()));
    if (options.body && !headers.has("Content-Type"))
      headers.set("Content-Type", "application/json");
    const response = await fetch(
      (this.base.endsWith("/") ? this.base.slice(0, -1) : this.base) + route,
      { ...options, headers, signal, credentials: "omit", redirect: "error" },
    );
    if (!response.ok) {
      let problem: Problem = {
        status: response.status,
        title: response.statusText,
      };
      try {
        problem = { ...problem, ...(await response.json()) };
      } catch {
        /* Empty errors still carry HTTP status. */
      }
      throw new ApiError(problem);
    }
    return response;
  }
  async json<T>(route: string, options: RequestInit = {}): Promise<T> {
    return (await this.request(route, options)).json() as Promise<T>;
  }
  post<T>(route: string, body: unknown, signal?: AbortSignal) {
    return this.json<T>(route, {
      method: "POST",
      body: JSON.stringify(body),
      signal,
    });
  }
}
export interface GearRuntime {
  api: ApiClient;
  identity: Identity;
  query: QueryClient;
  config: RuntimeConfig;
  signal: AbortSignal;
  notify: (message: string) => void;
}
export interface GearPage {
  id: string;
  title: string;
  path: string;
  load: () => Promise<{ default: ComponentType<{ runtime: GearRuntime }> }>;
}
export interface GearWidget {
  id: string;
  title: string;
  load: GearPage["load"];
}
export interface GearUiPlugin {
  apiVersion: 1;
  id: string;
  displayName: string;
  navigation: ReadonlyArray<{ pageId: string; label: string; icon: string }>;
  pages: ReadonlyArray<GearPage>;
  widgets: ReadonlyArray<GearWidget>;
}
export const queryKey = (identity: Identity, ...parts: unknown[]) =>
  ["gear", identity.subject_id, identity.tenant_id, ...parts] as const;
export interface EffectiveValue<T = unknown> {
  key: string;
  scope: string;
  value: T;
  source: "own_override" | "inherited" | "schema_default";
  sourceScope: string | null;
  inheritanceTrail: Array<{ scope: string; hasOverride: boolean }>;
}
export const scopeFor = (identity: Identity) =>
  "/tenants/" + identity.tenant_id;
export const ICE_CREAM_KEY =
  "gts.cf.core.settings.setting_type.v1~cf.family_labs.lab.ice_cream_approved.v1~";
export const LAB_MESSAGE_KEY =
  "gts.cf.core.settings.setting_type.v1~cf.family_labs.lab.lab_message.v1~";
export const effective = <T>(
  runtime: GearRuntime,
  key: string,
  signal?: AbortSignal,
) =>
  runtime.api.post<EffectiveValue<T>>(
    "/family-labs/v1/effective:read",
    { key, scope: scopeFor(runtime.identity) },
    signal,
  );
