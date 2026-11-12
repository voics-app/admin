import { useEffect, useState } from "react";
import { NavLink, Navigate, Route, Routes } from "react-router-dom";
import { api } from "./api/client";
import { BRAND } from "./brand";
import { useAsync } from "./api/useAsync";
import { ErrorBanner } from "./components/common";
import { AuditLogPage } from "./pages/AuditLog";
import { FeatureFlagsPage } from "./pages/FeatureFlags";
import { OverviewPage } from "./pages/Overview";
import { PlatformHealthPage } from "./pages/PlatformHealth";
import { ReportsPage } from "./pages/Reports";
import { SendNotificationPage } from "./pages/SendNotification";
import { TrustSafetyPage } from "./pages/TrustSafety";
import { UsagePage } from "./pages/Usage";
import { VoiceRegionsPage } from "./pages/VoiceRegions";

const NAV = [
  { to: "/overview", label: "Overview" },
  { to: "/usage", label: "Usage" },
  { to: "/voice", label: "Voice regions" },
  { to: "/health", label: "Platform health" },
  { to: "/safety", label: "Trust & Safety" },
  { to: "/reports", label: "Reports" },
  { to: "/notify", label: "Send notification" },
  { to: "/flags", label: "Feature flags" },
  { to: "/audit", label: "Audit log" },
];

type Theme = "dark" | "light";

function storedTheme(): Theme {
  try {
    const value = localStorage.getItem("admin-theme");
    if (value === "light" || value === "dark") return value;
  } catch {
    // storage unavailable: fall through to the OS preference
  }
  return window.matchMedia?.("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function useTheme() {
  const [theme, setTheme] = useState<Theme>(storedTheme);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("admin-theme", theme);
    } catch {
      // not persisted
    }
  }, [theme]);
  return [theme, () => setTheme((t) => (t === "dark" ? "light" : "dark"))] as const;
}

export function App() {
  const me = useAsync(api.me, []);
  const [theme, toggleTheme] = useTheme();

  if (me.error && !me.data) {
    return (
      <main>
        <h1>Access denied</h1>
        <p className="muted">The admin portal could not verify your identity.</p>
        <ErrorBanner error={me.error} onRetry={me.reload} />
      </main>
    );
  }

  if (!me.data) {
    return (
      <main role="status" className="empty">
        Signing in...
      </main>
    );
  }

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            D
          </span>
          {BRAND.adminTitle}
        </div>
        <nav className="nav" aria-label="Sections">
          {NAV.map((item) => (
            <NavLink key={item.to} to={item.to}>
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="sidebar-footer">
          <span>{me.data.email}</span>
          <button type="button" className="btn small" onClick={toggleTheme}>
            {theme === "dark" ? "Light theme" : "Dark theme"}
          </button>
        </div>
      </aside>
      <main>
        <Routes>
          <Route path="/" element={<Navigate to="/overview" replace />} />
          <Route path="/overview" element={<OverviewPage />} />
          <Route path="/usage" element={<UsagePage />} />
          <Route path="/voice" element={<VoiceRegionsPage />} />
          <Route path="/health" element={<PlatformHealthPage />} />
          <Route path="/safety" element={<TrustSafetyPage />} />
          <Route path="/reports" element={<ReportsPage />} />
          <Route path="/notify" element={<SendNotificationPage />} />
          <Route path="/flags" element={<FeatureFlagsPage />} />
          <Route path="/audit" element={<AuditLogPage />} />
          <Route path="*" element={<Navigate to="/overview" replace />} />
        </Routes>
      </main>
    </div>
  );
}
