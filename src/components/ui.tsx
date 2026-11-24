import { useEffect, useRef, type ReactNode } from "react";
import type { Confidence, DepStatus, Risk } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { IconAlert, IconCheck, IconInfo, IconX } from "./icons";

/** Permukaan dasar dengan garis tipis dan bayangan halus. */
export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={`rounded-xl border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-sm)] ${className}`}
    >
      {children}
    </div>
  );
}

type ButtonVariant = "primary" | "ghost" | "danger" | "subtle";
type ButtonSize = "sm" | "md";

const BUTTON_VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "bg-[var(--accent)] text-[var(--accent-fg)] shadow-[var(--shadow-sm)] hover:bg-[var(--accent-hover)] active:translate-y-px disabled:bg-[var(--surface-2)] disabled:text-[var(--faint)] disabled:shadow-none disabled:active:translate-y-0",
  ghost:
    "border border-[var(--border)] bg-[var(--surface)] text-[var(--fg)] hover:border-[var(--border-strong)] hover:bg-[var(--surface-2)] active:translate-y-px disabled:text-[var(--faint)] disabled:active:translate-y-0",
  danger:
    "border border-[var(--error)]/50 bg-transparent text-[var(--error)] hover:border-[var(--error)] hover:bg-[var(--error-soft)] active:translate-y-px disabled:opacity-50 disabled:active:translate-y-0",
  subtle:
    "bg-transparent text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--fg)] active:translate-y-px disabled:text-[var(--faint)] disabled:active:translate-y-0",
};

const BUTTON_SIZES: Record<ButtonSize, string> = {
  sm: "h-8 gap-1.5 px-3 text-[13px]",
  md: "h-9 gap-2 px-4 text-[13.5px]",
};

export function Button({
  children,
  onClick,
  variant = "primary",
  size = "md",
  disabled = false,
  className = "",
  type = "button",
  title,
  ariaLabel,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  className?: string;
  type?: "button" | "submit";
  title?: string;
  ariaLabel?: string;
}) {
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={ariaLabel}
      className={`inline-flex shrink-0 items-center justify-center rounded-lg font-medium transition-[background-color,border-color,color,transform,box-shadow] duration-150 disabled:cursor-not-allowed ${BUTTON_SIZES[size]} ${BUTTON_VARIANTS[variant]} ${className}`}
    >
      {children}
    </button>
  );
}

/** Tombol ikon persegi, dipakai di toolbar. */
export function IconButton({
  children,
  onClick,
  label,
  disabled = false,
  className = "",
}: {
  children: ReactNode;
  onClick?: () => void;
  label: string;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
      className={`inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-[var(--border)] text-[var(--muted)] transition-colors hover:border-[var(--border-strong)] hover:bg-[var(--surface-2)] hover:text-[var(--fg)] disabled:cursor-not-allowed disabled:text-[var(--faint)] ${className}`}
    >
      {children}
    </button>
  );
}

export type BannerTone = "info" | "success" | "warning" | "error";

const BANNER_TONES: Record<BannerTone, { box: string; icon: ReactNode }> = {
  info: {
    box: "border-[var(--border)] bg-[var(--surface-2)] text-[var(--fg)]",
    icon: <IconInfo size={15} />,
  },
  success: {
    box: "border-[var(--success)]/40 bg-[var(--success-soft)] text-[var(--success)]",
    icon: <IconCheck size={15} />,
  },
  warning: {
    box: "border-[var(--warning)]/40 bg-[var(--warning-soft)] text-[var(--warning)]",
    icon: <IconAlert size={15} />,
  },
  error: {
    box: "border-[var(--error)]/40 bg-[var(--error-soft)] text-[var(--error)]",
    icon: <IconAlert size={15} />,
  },
};

export function Banner({
  tone,
  title,
  children,
  className = "",
}: {
  tone: BannerTone;
  title?: ReactNode;
  children?: ReactNode;
  className?: string;
}) {
  const style = BANNER_TONES[tone];
  return (
    <div
      className={`flex items-start gap-2.5 rounded-xl border px-3.5 py-3 text-[13px] leading-relaxed ${style.box} ${className}`}
      role={tone === "error" ? "alert" : "status"}
    >
      <span className="mt-px shrink-0">{style.icon}</span>
      <div className="min-w-0 flex-1">
        {title ? <div className="font-semibold">{title}</div> : null}
        {children ? <div className={title ? "mt-1" : undefined}>{children}</div> : null}
      </div>
    </div>
  );
}

export type PillTone = "neutral" | "success" | "warning" | "error" | "accent";

const PILL_TONES: Record<PillTone, string> = {
  neutral: "bg-[var(--surface-2)] text-[var(--muted)]",
  success: "bg-[var(--success-soft)] text-[var(--success)]",
  warning: "bg-[var(--warning-soft)] text-[var(--warning)]",
  error: "bg-[var(--error-soft)] text-[var(--error)]",
  accent: "bg-[var(--accent-soft)] text-[var(--accent)]",
};

export function Pill({
  tone = "neutral",
  children,
  className = "",
}: {
  tone?: PillTone;
  children: ReactNode;
  className?: string;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[11.5px] font-medium leading-5 ${PILL_TONES[tone]} ${className}`}
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
  const tones: Record<DepStatus, PillTone> = {
    Installed: "success",
    Available: "accent",
    AurOnly: "warning",
    Missing: "error",
    Ignored: "neutral",
  };
  return (
    <span className="inline-flex items-center gap-2">
      <Pill tone={tones[status]}>{t(`common.status.${status}` as const)}</Pill>
      <span className="text-[11.5px] text-[var(--faint)]">
        {t(`common.confidence.${confidence}` as const)}
      </span>
    </span>
  );
}

export function Spinner({ size = 14 }: { size?: number }) {
  return (
    <span
      aria-hidden
      className="inline-block animate-spin rounded-full border-2 border-[var(--border-strong)] border-t-[var(--accent)]"
      style={{ width: size, height: size }}
    />
  );
}

/** Kerangka muat dengan kilau; `width` dalam persen. */
export function Skeleton({ width = "100%", height = 12 }: { width?: string; height?: number }) {
  return <span className="skeleton block" style={{ width, height }} aria-hidden />;
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
    <div role="tablist" className="flex gap-0.5 border-b border-[var(--border)] px-2">
      {tabs.map((tab) => {
        const selected = active === tab.id;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={selected}
            onClick={() => onChange(tab.id)}
            className={`relative -mb-px flex items-center gap-1.5 border-b-2 px-3 py-2.5 text-[13px] font-medium transition-colors ${
              selected
                ? "border-[var(--accent)] text-[var(--fg)]"
                : "border-transparent text-[var(--muted)] hover:text-[var(--fg)]"
            }`}
          >
            {tab.label}
            {typeof tab.count === "number" ? (
              <span
                className={`rounded px-1.5 py-px text-[11px] font-semibold ${
                  selected
                    ? "bg-[var(--accent-soft)] text-[var(--accent)]"
                    : "bg-[var(--surface-2)] text-[var(--faint)]"
                }`}
              >
                {tab.count}
              </span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}

/** Kartu angka ringkas untuk halaman Terpasang. */
export function StatCard({
  label,
  value,
  hint,
  icon,
  tone = "neutral",
}: {
  label: string;
  value: ReactNode;
  hint?: string;
  icon?: ReactNode;
  tone?: PillTone;
}) {
  const accent =
    tone === "success"
      ? "text-[var(--success)]"
      : tone === "accent"
        ? "text-[var(--accent)]"
        : tone === "warning"
          ? "text-[var(--warning)]"
          : "text-[var(--muted)]";
  return (
    <div className="rounded-xl border border-[var(--border)] bg-[var(--surface)] px-3.5 py-3 shadow-[var(--shadow-sm)]">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11.5px] font-medium uppercase tracking-wide text-[var(--faint)]">
          {label}
        </span>
        {icon ? <span className={accent}>{icon}</span> : null}
      </div>
      <div className="mt-1.5 text-[22px] font-semibold leading-none tracking-tight">{value}</div>
      {hint ? <p className="mt-1.5 text-[11.5px] text-[var(--faint)]">{hint}</p> : null}
    </div>
  );
}

/** Dialog modal dengan latar gelap dan transisi masuk yang halus. */
export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  footer,
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
}) {
  const panel = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey);
    // Fokuskan panel agar tombol Escape dan Tab bekerja di dalam dialog.
    panel.current?.focus();
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center p-4">
      <div
        className="animate-fade absolute inset-0 bg-black/55 backdrop-blur-[2px]"
        onClick={onClose}
        aria-hidden
      />
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        tabIndex={-1}
        className="animate-rise relative w-full max-w-md rounded-xl border border-[var(--border-strong)] bg-[var(--surface)] p-5 shadow-[var(--shadow-lg)] outline-none"
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h2 className="text-[15px] font-semibold tracking-tight">{title}</h2>
            {description ? (
              <p className="mt-1 text-[13px] leading-relaxed text-[var(--muted)]">{description}</p>
            ) : null}
          </div>
          <button
            onClick={onClose}
            aria-label="Tutup"
            className="-mr-1 -mt-1 shrink-0 rounded-lg p-1.5 text-[var(--faint)] transition-colors hover:bg-[var(--surface-2)] hover:text-[var(--fg)]"
          >
            <IconX size={15} />
          </button>
        </div>
        {children ? <div className="mt-3.5">{children}</div> : null}
        {footer ? <div className="mt-5 flex justify-end gap-2">{footer}</div> : null}
      </div>
    </div>
  );
}

/** Keadaan kosong dengan ilustrasi, pesan, dan aksi. */
export function EmptyState({
  illustration,
  title,
  description,
  action,
}: {
  illustration: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-[var(--border)] bg-[var(--surface)] px-6 py-12 text-center">
      <div className="opacity-90">{illustration}</div>
      <h3 className="mt-4 text-[14.5px] font-semibold">{title}</h3>
      {description ? (
        <p className="mt-1 max-w-sm text-[13px] leading-relaxed text-[var(--muted)]">
          {description}
        </p>
      ) : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}

/** Judul bagian dengan aksi di kanan. */
export function SectionHeader({
  title,
  action,
}: {
  title: string;
  action?: ReactNode;
}) {
  return (
    <div className="mb-2 flex items-center justify-between gap-3">
      <h2 className="text-[12px] font-semibold uppercase tracking-wider text-[var(--faint)]">
        {title}
      </h2>
      {action}
    </div>
  );
}
