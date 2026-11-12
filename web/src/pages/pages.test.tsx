import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../App";
import { api } from "../api/client";
import type { Overview, PlatformHealth, ReportSummary, VoiceRegions } from "../api/types";
import { AuditLogPage } from "./AuditLog";
import { FeatureFlagsPage } from "./FeatureFlags";
import { OverviewPage } from "./Overview";
import { PlatformHealthPage } from "./PlatformHealth";
import { ReportsPage } from "./Reports";
import { SendNotificationPage } from "./SendNotification";
import { TrustSafetyPage } from "./TrustSafety";
import { UsagePage } from "./Usage";
import { VoiceRegionsPage } from "./VoiceRegions";

vi.mock("../api/client", async (importOriginal) => {
  const original = await importOriginal<typeof import("../api/client")>();
  const fns = Object.fromEntries(Object.keys(original.api).map((name) => [name, vi.fn()]));
  return { ...original, api: fns };
});

const mocked = api as unknown as Record<keyof typeof api, ReturnType<typeof vi.fn>>;

beforeEach(() => {
  for (const fn of Object.values(mocked)) fn.mockReset();
});

const overview: Overview = {
  generated_at: "2026-10-17T10:00:00Z",
  platform: { users_total: 1200, guilds_total: 40, channels_total: 300, bots_total: 5, signups_24h: 12 },
  gateway: { sessions_online: 77, sessions_by_node: {}, guild_processes: 33, voice_allocations: 2 },
  messages_24h: { standard: 900, ephemeral: 50, p2p: 50, total: 1000 },
  voice_minutes_24h: 125,
  dau: 300,
  mau: 900,
  voice_nodes: 2,
  voice_users: 8,
  degraded: [],
};

describe("Overview", () => {
  it("renders platform totals", async () => {
    mocked.overview.mockResolvedValue(overview);
    render(<OverviewPage />);
    expect(await screen.findByText("1,200")).toBeInTheDocument();
    expect(screen.getByText("2.1 h")).toBeInTheDocument();
  });

  it("flags unavailable sources and shows n/a", async () => {
    mocked.overview.mockResolvedValue({ ...overview, platform: null, degraded: ["api"] });
    render(<OverviewPage />);
    expect(await screen.findByText(/platform totals \(api\)/)).toBeInTheDocument();
    expect(screen.getAllByText("n/a").length).toBeGreaterThan(0);
  });

  it("reports a failed load with a retry", async () => {
    mocked.overview.mockRejectedValue(new Error("upstream down"));
    render(<OverviewPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("upstream down");
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});

describe("Usage", () => {
  it("reloads when the range changes", async () => {
    mocked.usage.mockResolvedValue({ range: "24h", granularity: "hour", points: [], active_users: [] });
    render(<UsagePage />);
    await waitFor(() => expect(mocked.usage).toHaveBeenCalledWith("24h"));
    await userEvent.click(screen.getByRole("button", { name: "7d" }));
    await waitFor(() => expect(mocked.usage).toHaveBeenCalledWith("7d"));
  });

  it("offers a table view for every chart", async () => {
    mocked.usage.mockResolvedValue({
      range: "24h",
      granularity: "hour",
      points: [
        { t: "2026-10-17T09:00:00Z", messages_standard: 5, messages_ephemeral: 1, messages_p2p: 2, voice_minutes: 3, sessions_peak: 4, active_calls_peak: 1, signups: 1 },
      ],
      active_users: [],
    });
    render(<UsagePage />);
    const messages = await screen.findByRole("region", { name: "Messages" });
    await userEvent.click(within(messages).getByRole("button", { name: "Table" }));
    expect(within(messages).getByRole("columnheader", { name: "Ephemeral (E2EE)" })).toBeInTheDocument();
  });
});

const regions: VoiceRegions = {
  etcd_healthy: true,
  quality_window_minutes: 15,
  regions: [
    {
      region: "nl-home",
      nodes: 1,
      accepting_nodes: 1,
      draining_nodes: 0,
      users: 4,
      egress_bps: 2_000_000,
      avg_cpu_pct: 12,
      max_cpu_pct: 12,
      quality: null,
      node_list: [{ node_id: "vn-1", ws_url: "ws://x", public_ip: "10.0.0.1", udp_port: 40000, users: 4, cpu_pct: 12, egress_bps: 2_000_000, draining: false, version: "0.1.0" }],
    },
  ],
};

describe("Voice regions", () => {
  it("drains a node only after confirmation", async () => {
    mocked.voiceRegions.mockResolvedValue(regions);
    mocked.drainNode.mockResolvedValue(undefined);
    render(<VoiceRegionsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Drain vn-1" }));
    expect(mocked.drainNode).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Drain node" }));
    await waitFor(() => expect(mocked.drainNode).toHaveBeenCalledWith("vn-1"));
  });

  it("warns when etcd is unreachable", async () => {
    mocked.voiceRegions.mockResolvedValue({ ...regions, etcd_healthy: false });
    render(<VoiceRegionsPage />);
    expect(await screen.findByText(/etcd is unreachable/)).toBeInTheDocument();
  });
});

describe("Platform health", () => {
  it("pairs each status with a text label", async () => {
    const health: PlatformHealth = {
      metrics_reachable: true,
      services: [
        { service: "api", status: "healthy", instances_up: 2, instances_total: 2, request_rate: 12, error_ratio: 0.001, latency_p95_ms: 40 },
        { service: "gateway", status: "down", instances_up: 0, instances_total: 1, request_rate: null, error_ratio: null, latency_p95_ms: null },
      ],
      consumer_lag: [],
      db_latency_p95_ms: [],
      gateway: null,
    };
    mocked.platformHealth.mockResolvedValue(health);
    mocked.healthSeries.mockResolvedValue({ panel: "request_rate", range: "1h", series: [] });
    render(<PlatformHealthPage />);
    expect(await screen.findByText("Healthy")).toBeInTheDocument();
    expect(screen.getByText("Down")).toBeInTheDocument();
    await waitFor(() => expect(mocked.healthSeries).toHaveBeenCalledTimes(6));
  });

  it("labels services with no scrape targets as no data", async () => {
    const health: PlatformHealth = {
      metrics_reachable: true,
      services: [
        { service: "messages", status: "unknown", instances_up: 0, instances_total: 0, request_rate: null, error_ratio: null, latency_p95_ms: null },
      ],
      consumer_lag: [],
      db_latency_p95_ms: [],
      gateway: null,
    };
    mocked.platformHealth.mockResolvedValue(health);
    mocked.healthSeries.mockResolvedValue({ panel: "request_rate", range: "1h", series: [] });
    render(<PlatformHealthPage />);
    expect(await screen.findByText("No data")).toBeInTheDocument();
  });
});

describe("Trust & Safety", () => {
  it("suspends a user with a reason", async () => {
    mocked.searchUsers.mockResolvedValue({
      users: [{ id: "42", username: "spammer", global_name: null, email: null, bot: false, created_at: null, suspended_until: null }],
      next_after: null,
    });
    mocked.suspendUser.mockResolvedValue(undefined);
    render(<TrustSafetyPage />);
    await userEvent.type(screen.getByLabelText("Search users"), "spam");
    await userEvent.click(screen.getByRole("button", { name: "Search users" }));
    await userEvent.click(await screen.findByRole("button", { name: "Suspend spammer" }));
    const submit = screen.getByRole("button", { name: "Suspend user" });
    expect(submit).toBeDisabled();
    await userEvent.type(screen.getByLabelText(/Reason/), "spam links");
    await userEvent.click(submit);
    await waitFor(() => expect(mocked.suspendUser).toHaveBeenCalledWith("42", "spam links", undefined));
  });
});

describe("Feature flags", () => {
  it("creates a flag", async () => {
    mocked.flags.mockResolvedValue({ flags: [] });
    mocked.putFlag.mockResolvedValue({});
    render(<FeatureFlagsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "New flag" }));
    await userEvent.type(screen.getByLabelText("Key"), "voice.video");
    await userEvent.click(screen.getByLabelText(/Enabled/));
    await userEvent.click(screen.getByRole("button", { name: "Save flag" }));
    await waitFor(() => expect(mocked.putFlag).toHaveBeenCalledWith("voice.video", { description: "", enabled: true, rollout_percent: 100 }));
  });

  it("shows server validation errors in the dialog", async () => {
    mocked.flags.mockResolvedValue({ flags: [] });
    mocked.putFlag.mockRejectedValue(new Error("invalid flag key"));
    render(<FeatureFlagsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "New flag" }));
    await userEvent.type(screen.getByLabelText("Key"), "Bad Key");
    await userEvent.click(screen.getByRole("button", { name: "Save flag" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("invalid flag key");
  });
});

describe("Audit log", () => {
  it("pages backwards through older entries", async () => {
    const entry = (id: number) => ({ id, at: "2026-10-17T10:00:00Z", actor_email: "a@x.io", action: `act.${id}`, target: "t", details: { n: id } });
    mocked.audit.mockResolvedValueOnce({ entries: [entry(2)], next_before: 2 }).mockResolvedValueOnce({ entries: [entry(1)], next_before: null });
    render(<AuditLogPage />);
    expect(await screen.findByText("act.2")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Load older entries" }));
    expect(await screen.findByText("act.1")).toBeInTheDocument();
    expect(mocked.audit).toHaveBeenLastCalledWith(2);
    expect(screen.queryByRole("button", { name: "Load older entries" })).not.toBeInTheDocument();
  });
});

describe("App", () => {
  it("shows the signed-in admin and navigates", async () => {
    mocked.me.mockResolvedValue({ email: "root@voics.nl" });
    mocked.overview.mockResolvedValue(overview);
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByText("root@voics.nl")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Audit log" })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "Overview" })).toBeInTheDocument();
  });

  it("blocks the shell when the identity is rejected", async () => {
    mocked.me.mockRejectedValue(new Error("not on the allow-list"));
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByRole("heading", { name: "Access denied" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  });
});

describe("Reports", () => {
  const report = (overrides: Partial<ReportSummary> = {}): ReportSummary => ({
    id: "9",
    kind: "message",
    category: "harassment",
    note: "keeps doing it",
    status: "open",
    created_at: "2026-11-01T10:00:00Z",
    reporter: { id: "1", username: "ben" },
    reported: { id: "2", username: "ann" },
    channel_id: "3",
    guild_id: "4",
    message_id: "5",
    delivery_mode: "standard",
    content: "you are the worst",
    content_source: "server",
    resolved_at: null,
    resolved_by: null,
    resolution_note: null,
    ...overrides,
  });

  it("shows server-copied evidence and dismisses with a note", async () => {
    mocked.reports.mockResolvedValue({ reports: [report()], next_after: null });
    mocked.resolveReport.mockResolvedValue(report({ status: "dismissed" }));
    render(<ReportsPage />);
    expect(await screen.findByText("Copied by server")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Review report 9" }));
    expect(screen.getByText("you are the worst")).toBeInTheDocument();
    expect(screen.getByText("keeps doing it")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText(/Note/), "not abuse");
    await userEvent.click(screen.getByRole("button", { name: "Dismiss report" }));
    await waitFor(() => expect(mocked.resolveReport).toHaveBeenCalledWith("9", "dismissed", "not abuse"));
    expect(await screen.findByRole("status")).toHaveTextContent("Report 9 dismissed.");
  });

  it("marks reporter-submitted evidence as unverified and resolves", async () => {
    mocked.reports.mockResolvedValue({
      reports: [report({ content_source: "reporter", delivery_mode: "ephemeral", content: "what I saw" })],
      next_after: null,
    });
    mocked.resolveReport.mockResolvedValue(report({ status: "resolved" }));
    render(<ReportsPage />);
    expect(await screen.findByText("From reporter")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Review report 9" }));
    expect(screen.getByText(/Unverified/)).toHaveTextContent("ephemeral");
    await userEvent.click(screen.getByRole("button", { name: "Mark resolved" }));
    await waitFor(() => expect(mocked.resolveReport).toHaveBeenCalledWith("9", "resolved", undefined));
  });

  it("switches status, pages with the cursor and hides actions on closed reports", async () => {
    mocked.reports
      .mockResolvedValueOnce({ reports: [report()], next_after: "9" })
      .mockResolvedValueOnce({ reports: [report({ id: "8" })], next_after: null })
      .mockResolvedValueOnce({
        reports: [report({ status: "resolved", resolved_by: "ops@x.io", resolution_note: "banned" })],
        next_after: null,
      });
    render(<ReportsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Load more" }));
    await waitFor(() => expect(mocked.reports).toHaveBeenLastCalledWith("open", "9"));
    expect(await screen.findByRole("button", { name: "Review report 8" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Resolved" }));
    await userEvent.click(await screen.findByRole("button", { name: "Review report 9" }));
    expect(screen.getByText(/Resolved by ops@x.io/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mark resolved" })).not.toBeInTheDocument();
  });

  it("explains a report with no stored content and shows API errors", async () => {
    mocked.reports.mockResolvedValue({
      reports: [report({ content: null, content_source: null, delivery_mode: "p2p" })],
      next_after: null,
    });
    mocked.resolveReport.mockRejectedValue(new Error("the report is already closed"));
    render(<ReportsPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Review report 9" }));
    expect(screen.getByText(/the server holds no copy/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Mark resolved" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("already closed");
  });
});

describe("Send notification", () => {
  const alice = { id: "42", username: "alice", global_name: null, email: null, bot: false, created_at: null, suspended_until: null };

  it("sends to one user after showing the device count", async () => {
    mocked.searchUsers.mockResolvedValue({ users: [alice], next_after: null });
    mocked.previewNotification.mockResolvedValue({ devices: 2, queued: 0, dropped: 0, truncated: false });
    mocked.sendNotification.mockResolvedValue({ devices: 2, queued: 2, dropped: 0, truncated: false });
    render(<SendNotificationPage />);
    const review = screen.getByRole("button", { name: "Review and send" });
    expect(review).toBeDisabled();
    await userEvent.type(screen.getByLabelText("Find the recipient"), "ali");
    await userEvent.click(screen.getByRole("button", { name: "Search recipient" }));
    await userEvent.click(await screen.findByRole("button", { name: "Choose alice" }));
    await userEvent.type(screen.getByLabelText("Title"), "Maintenance");
    await userEvent.type(screen.getByLabelText("Message"), "Back at 22:00");
    expect(within(screen.getByLabelText("Notification preview")).getByText("Maintenance")).toBeInTheDocument();
    await userEvent.click(review);
    const dialog = await screen.findByRole("dialog", { name: "Send to @alice?" });
    expect(within(dialog).getByRole("status")).toHaveTextContent("2 devices of this user");
    expect(mocked.sendNotification).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Send notification" }));
    await waitFor(() =>
      expect(mocked.sendNotification).toHaveBeenCalledWith({ target: { user_id: "42" }, title: "Maintenance", body: "Back at 22:00" }),
    );
    expect(await screen.findByText(/Sent to 2 of 2 devices/)).toBeInTheDocument();
  });

  it("states the device count before sending to everyone", async () => {
    mocked.previewNotification.mockResolvedValue({ devices: 1234, queued: 0, dropped: 0, truncated: false });
    mocked.sendNotification.mockResolvedValue({ devices: 1234, queued: 1230, dropped: 4, truncated: false });
    render(<SendNotificationPage />);
    await userEvent.click(screen.getByRole("button", { name: "Everyone" }));
    await userEvent.type(screen.getByLabelText("Title"), "Hello");
    await userEvent.type(screen.getByLabelText("Message"), "World");
    await userEvent.click(screen.getByRole("button", { name: "Review and send" }));
    const dialog = await screen.findByRole("dialog", { name: "Send to everyone?" });
    expect(within(dialog).getByText("1,234 devices")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Send to 1,234 devices" }));
    await waitFor(() => expect(mocked.sendNotification).toHaveBeenCalledWith({ target: "all", title: "Hello", body: "World" }));
    expect(await screen.findByText(/Sent to 1,230 of 1,234 devices; 4 could not be queued/)).toBeInTheDocument();
  });

  it("cannot send when nobody has a device, and shows a failed send", async () => {
    mocked.previewNotification.mockResolvedValueOnce({ devices: 0, queued: 0, dropped: 0, truncated: false });
    render(<SendNotificationPage />);
    await userEvent.click(screen.getByRole("button", { name: "Everyone" }));
    await userEvent.type(screen.getByLabelText("Title"), "Hello");
    await userEvent.type(screen.getByLabelText("Message"), "World");
    await userEvent.click(screen.getByRole("button", { name: "Review and send" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("button", { name: /Send to/ })).toBeDisabled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    mocked.previewNotification.mockResolvedValueOnce({ devices: 3, queued: 0, dropped: 0, truncated: false });
    mocked.sendNotification.mockRejectedValue(new Error("another notification to everyone is still being sent"));
    await userEvent.click(screen.getByRole("button", { name: "Review and send" }));
    await userEvent.click(await screen.findByRole("button", { name: "Send to 3 devices" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("still being sent");
  });

  it("shows a preview failure on the page", async () => {
    mocked.previewNotification.mockRejectedValue(new Error("NOTIFIER_INTERNAL_URL is not configured"));
    render(<SendNotificationPage />);
    await userEvent.click(screen.getByRole("button", { name: "Everyone" }));
    await userEvent.type(screen.getByLabelText("Title"), "Hello");
    await userEvent.type(screen.getByLabelText("Message"), "World");
    await userEvent.click(screen.getByRole("button", { name: "Review and send" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not configured");
  });
});
