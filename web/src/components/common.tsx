import type { ReactNode } from "react";
import { RANGES, type Range } from "../api/types";
import type { AsyncState } from "../api/useAsync";

export function PageHead({ title, subtitle, children }: { title: string; subtitle?: string; children?: ReactNode }) {
  return (
    <header className="page-head">
      <div>
        <h1>{title}</h1>
        {subtitle ? <p>{subtitle}</p> : null}
      </div>
      {children}
    </header>
  );
}

export function StatTile({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="panel tile">
      <div className="label">{label}</div>
      <div className="value">{value}</div>
      {hint ? <div className="hint">{hint}</div> : null}
    </div>
  );
}

export function RangePicker({ value, onChange, options = RANGES }: { value: Range; onChange: (r: Range) => void; options?: readonly Range[] }) {
  return (
    <div className="segmented" role="group" aria-label="Time range">
      {options.map((range) => (
        <button key={range} type="button" aria-pressed={value === range} onClick={() => onChange(range)}>
          {range}
        </button>
      ))}
    </div>
  );
}

export function ErrorBanner({ error, onRetry }: { error: Error; onRetry?: () => void }) {
  return (
    <div className="banner error" role="alert">
      <span>{error.message}</span>
      {onRetry ? (
        <button type="button" className="btn small" onClick={onRetry}>
          Retry
        </button>
      ) : null}
    </div>
  );
}

/** Renders the loading and failure states of an async load, and the content once it has data. */
export function Loaded<T>({ state, children }: { state: AsyncState<T>; children: (data: T) => ReactNode }) {
  if (state.data !== undefined) {
    return (
      <>
        {state.error ? <ErrorBanner error={state.error} onRetry={state.reload} /> : null}
        {children(state.data)}
      </>
    );
  }
  if (state.error) return <ErrorBanner error={state.error} onRetry={state.reload} />;
  return (
    <div className="empty" role="status">
      Loading...
    </div>
  );
}

type Tone = "good" | "neutral" | "warning" | "serious" | "critical";

const ICONS: Record<Tone, ReactNode> = {
  good: <path d="M3 8.5l3 3 7-7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />,
  neutral: <circle cx="8" cy="8" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.6" />,
  warning: <path d="M8 2l6.5 11.5h-13zM8 6.5v3M8 11.5v.01" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" strokeLinecap="round" />,
  serious: <path d="M8 2l6.5 11.5h-13zM8 6.5v3M8 11.5v.01" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" strokeLinecap="round" />,
  critical: <path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />,
};

/** Status is always icon + label; colour alone never carries it. */
export function StatusBadge({ tone, label }: { tone: Tone; label: string }) {
  return (
    <span className={`status ${tone}`}>
      <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
        {ICONS[tone]}
      </svg>
      {label}
    </span>
  );
}

export function Dialog({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => event.key === "Escape" && onClose()}
      >
        <h2>{title}</h2>
        {children}
      </div>
    </div>
  );
}
