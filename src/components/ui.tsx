import type { ReactNode } from "react";
import type { Confidence, DepStatus, Risk } from "../lib/api";
import { useI18n } from "../lib/i18n";

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={`rounded-xl border border-[var(--border)] bg-[var(--surface)] ${className}`}
    >
      {children}
    </div>
  );
}

export function Button({
  children,
  onClick,
  variant = "primary",
  disabled = false,
  className = "",
  type = "button",
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "ghost" | "danger";
  disabled?: boolean;
  className?: string;
  type?: "button" | "submit";
}) {
  const styles: Record<string, string> = {
    primary:
      "bg-[var(--accent)] text-[var(--accent-fg)] hover:opacity-90 disabled:bg-[var(--surface-2)] disabled:text-[var(--muted)]",
    ghost:
      "border border-[var(--border)] bg-[var(--surface)] text-[var(--fg)] hover:bg-[var(--surface-2)] disabled:opacity-40",
    danger:
      "border border-[var(--error)] text-[var(--error)] hover:bg-[var(--error-bg)] disabled:opacity-40",
  };
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      className={`inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed ${styles[variant]} ${className}`}
    >
      {children}
    </button>
  );
}

export function Banner({
  tone,
  title,
  children,
}: {
  tone: "info" | "success" | "warning" | "error";
  title?: string;
  children?: ReactNode;
}) {
  const tones: Record<string, string> = {
    info: "border-[var(--border)] bg-[var(--surface-2)] text-[var(--fg)]",
    success: "border-[var(--success)] bg-[var(--success-bg)] text-[var(--success)]",
    warning: "border-[var(--warning)] bg-[var(--warning-bg)] text-[var(--warning)]",
    error: "border-[var(--error)] bg-[var(--error-bg)] text-[var(--error)]",
  };
  return (
    <div className={`rounded-lg border px-4 py-3 text-sm ${tones[tone]}`} role={tone === "error" ? "alert" : "status"}>
      {title ? <div className="font-semibold">{title}</div> : null}
      {children ? <div className={title ? "mt-1 leading-relaxed" : "leading-relaxed"}>{children}</div> : null}
    </div>
  );
}

export function Pill({
  tone,
  children,
}: {
  tone: "neutral" | "success" | "warning" | "error" | "accent";
  children: ReactNode;
}) {
  const tones: Record<string, string> = {
    neutral: "bg-[var(--surface-2)] text-[var(--muted)]",
    success: "bg-[var(--success-bg)] text-[var(--success)]",
    warning: "bg-[var(--warning-bg)] text-[var(--warning)]",
    error: "bg-[var(--error-bg)] text-[var(--error)]",
    accent: "bg-[var(--surface-2)] text-[var(--accent)]",
  };
  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${tones[tone]}`}
    >
      {children}
    </span>
  );
}

export function RiskBadge({ risk }: { risk: Risk }) {
  const { t } = useI18n();
  const map = {
    Safe: { tone: "success" as const, key: "common.risk.safe" as const },
    Caution: { tone: "warning" as const, key: "common.risk.caution" as const },
    Dangerous: { tone: "error" as const, key: "common.risk.dangerous" as const },
  };
  const entry = map[risk];
  return <Pill tone={entry.tone}>{t(entry.key)}</Pill>;
}

export function StatusPill({ status, confidence }: { status: DepStatus; confidence: Confidence }) {
  const { t } = useI18n();
  const tones: Record<DepStatus, "success" | "accent" | "warning" | "error" | "neutral"> = {
    Installed: "success",
    Available: "accent",
    AurOnly: "warning",
    Missing: "error",
    Ignored: "neutral",
  };
  return (
    <span className="inline-flex items-center gap-2">
      <Pill tone={tones[status]}>{t(`common.status.${status}` as const)}</Pill>
      <span className="text-xs text-[var(--muted)]">
        {t(`common.confidence.${confidence}` as const)}
      </span>
    </span>
  );
}

export function Spinner() {
  return (
    <span
      aria-hidden
      className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-[var(--border)] border-t-[var(--accent)]"
    />
  );
}

export function Tabs<T extends string>({
  tabs,
  active,
  onChange,
}: {
  tabs: { id: T; label: string; count?: number }[];
  active: T;
  onChange: (id: T) => void;
}) {
  return (
    <div role="tablist" className="flex gap-1 border-b border-[var(--border)]">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={active === tab.id}
          onClick={() => onChange(tab.id)}
          className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium transition-colors ${
            active === tab.id
              ? "border-[var(--accent)] text-[var(--fg)]"
              : "border-transparent text-[var(--muted)] hover:text-[var(--fg)]"
          }`}
        >
          {tab.label}
          {typeof tab.count === "number" ? (
            <span className="ml-1.5 rounded-full bg-[var(--surface-2)] px-1.5 py-0.5 text-xs text-[var(--muted)]">
              {tab.count}
            </span>
          ) : null}
        </button>
      ))}
    </div>
  );
}
