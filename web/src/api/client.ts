import type {
  AuditPage,
  Flag,
  FlagInput,
  GuildSummary,
  HealthPanel,
  HealthSeries,
  Me,
  Overview,
  PlatformHealth,
  Range,
  ReportPage,
  ReportResolution,
  ReportStatus,
  ReportSummary,
  Usage,
  UserPage,
  VoiceRegions,
} from "./types";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

const BASE = "/admin-api";

/** The BFF requires this header on state-changing requests (see bff/src/interface/http/auth.rs). */
const CSRF_HEADER = { "x-discordy-admin": "1" };

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const mutates = init?.method !== undefined && init.method !== "GET";
  const response = await fetch(`${BASE}${path}`, {
    ...init,
    headers: {
      accept: "application/json",
      ...(mutates ? CSRF_HEADER : {}),
      ...(init?.body ? { "content-type": "application/json" } : {}),
    },
  });
  if (!response.ok) {
    throw new ApiError(response.status, await errorMessage(response));
  }
  return response.status === 204 ? (undefined as T) : ((await response.json()) as T);
}

async function errorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: string; message?: string };
    return body.message ?? body.error ?? response.statusText;
  } catch {
    return response.statusText || `HTTP ${response.status}`;
  }
}

const query = (params: Record<string, string | number | undefined>) => {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") search.set(key, String(value));
  }
  const text = search.toString();
  return text ? `?${text}` : "";
};

const post = (path: string, body?: unknown) =>
  request<void>(path, { method: "POST", body: body === undefined ? undefined : JSON.stringify(body) });

export const api = {
  me: () => request<Me>("/me"),
  overview: () => request<Overview>("/overview"),
  usage: (range: Range) => request<Usage>(`/usage${query({ range })}`),
  voiceRegions: () => request<VoiceRegions>("/voice/regions"),
  drainNode: (nodeId: string) => post(`/voice/nodes/${encodeURIComponent(nodeId)}/drain`),
  undrainNode: (nodeId: string) => post(`/voice/nodes/${encodeURIComponent(nodeId)}/undrain`),
  platformHealth: () => request<PlatformHealth>("/platform/health"),
  healthSeries: (panel: HealthPanel, range: Range) =>
    request<HealthSeries>(`/platform/health/series${query({ panel, range })}`),
  searchUsers: (text: string, after?: string) =>
    request<UserPage>(`/users${query({ query: text, after, limit: 25 })}`),
  suspendUser: (userId: string, reason: string, until?: string) =>
    post(`/users/${encodeURIComponent(userId)}/suspend`, { reason, until: until || undefined }),
  reports: (status: ReportStatus, after?: string) =>
    request<ReportPage>(`/reports${query({ status, after, limit: 25 })}`),
  resolveReport: (id: string, resolution: ReportResolution, note?: string) =>
    request<ReportSummary>(`/reports/${encodeURIComponent(id)}/resolve`, {
      method: "POST",
      body: JSON.stringify({ resolution, note: note || undefined }),
    }),
  searchGuilds: (text: string) =>
    request<{ guilds: GuildSummary[] }>(`/guilds${query({ query: text })}`),
  flags: () => request<{ flags: Flag[] }>("/flags"),
  putFlag: (key: string, input: FlagInput) =>
    request<Flag>(`/flags/${encodeURIComponent(key)}`, { method: "PUT", body: JSON.stringify(input) }),
  deleteFlag: (key: string) => request<void>(`/flags/${encodeURIComponent(key)}`, { method: "DELETE" }),
  audit: (before?: number) => request<AuditPage>(`/audit${query({ before, limit: 50 })}`),
};

export type Api = typeof api;
