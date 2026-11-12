/** Response shapes of the BFF's `/admin-api`. Mirrors `bff/src/interface/http/dto.rs`. */

export type Range = "1h" | "24h" | "7d" | "30d" | "90d";
export const RANGES: readonly Range[] = ["1h", "24h", "7d", "30d", "90d"];

export interface Me {
  email: string;
}

export interface ApiStats {
  users_total: number;
  guilds_total: number;
  channels_total: number;
  bots_total: number;
  signups_24h: number;
}

export interface GatewayStats {
  sessions_online: number;
  sessions_by_node: Record<string, number>;
  guild_processes: number;
  voice_allocations: number;
}

export interface Overview {
  generated_at: string;
  platform: ApiStats | null;
  gateway: GatewayStats | null;
  messages_24h: { standard: number; ephemeral: number; p2p: number; total: number } | null;
  voice_minutes_24h: number | null;
  dau: number | null;
  mau: number | null;
  voice_nodes: number | null;
  voice_users: number | null;
  degraded: string[];
}

export interface UsagePoint {
  t: string;
  messages_standard: number;
  messages_ephemeral: number;
  messages_p2p: number;
  voice_minutes: number;
  sessions_peak: number;
  active_calls_peak: number;
  signups: number;
}

export interface Usage {
  range: Range;
  granularity: string;
  points: UsagePoint[];
  active_users: { day: string; dau: number; mau: number }[];
}

export interface VoiceNode {
  node_id: string;
  ws_url: string;
  public_ip: string;
  udp_port: number;
  users: number;
  cpu_pct: number;
  egress_bps: number;
  draining: boolean;
  version: string;
}

export interface Region {
  region: string;
  nodes: number;
  accepting_nodes: number;
  draining_nodes: number;
  users: number;
  egress_bps: number;
  avg_cpu_pct: number;
  max_cpu_pct: number;
  quality: {
    samples: number;
    jitter_ms: number;
    loss_pct: number;
    rtt_ms: number;
    bitrate_kbps: number;
  } | null;
  node_list: VoiceNode[];
}

export interface VoiceRegions {
  etcd_healthy: boolean;
  quality_window_minutes: number;
  regions: Region[];
}

export type ServiceStatus = "healthy" | "degraded" | "down" | "unknown";

export interface ServiceHealth {
  service: string;
  status: ServiceStatus;
  instances_up: number;
  instances_total: number;
  request_rate: number | null;
  error_ratio: number | null;
  latency_p95_ms: number | null;
}

export interface LabelledValue {
  label: string;
  value: number;
}

export interface PlatformHealth {
  metrics_reachable: boolean;
  services: ServiceHealth[];
  consumer_lag: LabelledValue[];
  db_latency_p95_ms: LabelledValue[];
  gateway: GatewayStats | null;
}

export type HealthPanel =
  | "request_rate"
  | "error_ratio"
  | "latency_p95"
  | "consumer_lag"
  | "db_latency_p95"
  | "gateway_sessions";

export interface HealthSeries {
  panel: HealthPanel;
  range: Range;
  series: { label: string; points: [string, number][] }[];
}

export interface UserSummary {
  id: string;
  username: string;
  global_name: string | null;
  email: string | null;
  bot: boolean;
  created_at: string | null;
  suspended_until: string | null;
}

export interface UserPage {
  users: UserSummary[];
  next_after: string | null;
}

export interface GuildSummary {
  id: string;
  name: string;
  owner_id: string | null;
  member_count: number | null;
  voice_region: string | null;
}

export interface Flag {
  key: string;
  description: string;
  enabled: boolean;
  rollout_percent: number;
  updated_at: string;
  updated_by: string;
}

export interface FlagInput {
  description: string;
  enabled: boolean;
  rollout_percent: number;
}

export interface NotificationDraft {
  /** `"all"` for everyone, or one user. */
  target: "all" | { user_id: string };
  title: string;
  body: string;
}

/** What the notifier counted: devices reached, handed to the push queue, dropped. */
export interface PushReport {
  devices: number;
  queued: number;
  dropped: number;
  truncated: boolean;
}

export interface AuditEntry {
  id: number;
  at: string;
  actor_email: string;
  action: string;
  target: string;
  details: unknown;
}

export interface AuditPage {
  entries: AuditEntry[];
  next_before: number | null;
}

export type ReportStatus = "open" | "resolved" | "dismissed";
export type ReportResolution = "resolved" | "dismissed";

export interface ReportParty {
  id: string;
  username: string | null;
}

export interface ReportSummary {
  id: string;
  kind: "message" | "user";
  category: string;
  note: string | null;
  status: ReportStatus;
  created_at: string;
  reporter: ReportParty;
  reported: ReportParty;
  channel_id: string | null;
  guild_id: string | null;
  message_id: string | null;
  delivery_mode: string | null;
  /** Message text: copied by the server ("server") or submitted by the reporter ("reporter"). */
  content: string | null;
  content_source: "server" | "reporter" | null;
  resolved_at: string | null;
  resolved_by: string | null;
  resolution_note: string | null;
}

export interface ReportPage {
  reports: ReportSummary[];
  next_after: string | null;
}
