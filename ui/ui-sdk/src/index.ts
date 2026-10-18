import type { EventFeed } from "./events";
export { EventFeed, type FeedSnapshot, type ConnectionState } from "./events";
export { browserTabId, type GearEvent } from "./stream-protocol";
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
  identityPath: string;
  tenants: Record<
    string,
    {
      name: string;
      character: string;
      role: string;
      portrait: string;
      portraitPosition?: string;
      portraitZoom?: number;
    }
  >;
  events: { topic: string; type: string };
  settings: {
    effectivePath: string;
    approvalKey: string;
    fields: Array<{
      key: string;
      label: string;
      kind: "boolean" | "string" | "number";
    }>;
  };
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
  private recentToken: string | undefined;
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
      base.includes("#") ||
      base.split("/").includes("..") ||
      /[\u0000-\u0020\u007f]/.test(base) ||
      /%(?:2e|2f|5c)/i.test(base)
    )
      throw Error("API base must be a same-origin path");
  }
  async request(route: string, options: RequestInit = {}): Promise<Response> {
    if (
      !route.startsWith("/") ||
      route.startsWith("//") ||
      route.includes("\\") ||
      route.includes("#") ||
      route.split(/[/?]/).includes("..") ||
      /[\u0000-\u0020\u007f]/.test(route) ||
      /%(?:2e|2f|5c)/i.test(route.split("?")[0])
    )
      throw Error("Invalid API route");
    const signal = options.signal
      ? AbortSignal.any([this.lifecycle, options.signal])
      : this.lifecycle;
    const send = async (refresh = false) => {
      const headers = new Headers(options.headers);
      this.recentToken = await this.token(refresh);
      headers.set("Authorization", "Bearer " + this.recentToken);
      if (options.body && !headers.has("Content-Type"))
        headers.set("Content-Type", "application/json");
      return fetch(
        (this.base.endsWith("/") ? this.base.slice(0, -1) : this.base) + route,
        { ...options, headers, signal, credentials: "omit", redirect: "error" },
      );
    };
    let response = await send();
    if (response.status === 401 && !(options.body instanceof ReadableStream)) {
      await response.body?.cancel();
      response = await send(true);
    }
    if (!response.ok) {
      let problem: Problem = {
        status: response.status,
        title: response.statusText,
      };
      try {
        problem = {
          ...problem,
          ...(await response.json()),
          status: response.status,
        };
      } catch {
        /* Empty errors still carry HTTP status. */
      }
      throw new ApiError(problem);
    }
    return response;
  }
  async cleanup(route: string): Promise<void> {
    if (
      !this.recentToken ||
      !/^\/event-broker\/v1\/(subscriptions|consumer-groups)\/[^/?#]+$/.test(
        route,
      )
    )
      return;
    try {
      await fetch(
        (this.base.endsWith("/") ? this.base.slice(0, -1) : this.base) + route,
        {
          method: "DELETE",
          headers: { Authorization: "Bearer " + this.recentToken },
          keepalive: true,
          signal: AbortSignal.timeout(3000),
          credentials: "omit",
          redirect: "error",
        },
      );
    } catch {
      /* Best-effort bounded cleanup can outlive a session abort. */
    }
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
  events: EventFeed;
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
