import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, formatBytes, type Analysis, type BuildResult, type InstallOutcome, type Progress as ProgressEvent, type Settings } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Banner, Button, Card, Spinner } from "../components/ui";
import {
  IconAlert,
  IconCheck,
  IconFolder,
  IconRefresh,
  IconX,
} from "../components/icons";

type Phase = "convert" | "install" | "done" | "error";

const STAGES: { id: Phase | "analyze"; labelKey: "progress.stage.analyze" | "progress.stage.convert" | "progress.stage.install" | "progress.stage.done" }[] = [
  { id: "analyze", labelKey: "progress.stage.analyze" },
  { id: "convert", labelKey: "progress.stage.convert" },
  { id: "install", labelKey: "progress.stage.install" },
  { id: "done", labelKey: "progress.stage.done" },
];

export function Progress({
  analysis,
  settings,
  onDone,
  onRetry,
}: {
  analysis: Analysis;
  settings: Settings;
  onDone: () => void;
  onRetry: () => void;
}) {
  const { t } = useI18n();
  const [phase, setPhase] = useState<Phase>("convert");
  const [message, setMessage] = useState(t("progress.converting"));
  const [lines, setLines] = useState<string[]>([]);
  const [result, setResult] = useState<BuildResult | null>(null);
  const [outcome, setOutcome] = useState<InstallOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const startedAttempt = useRef(-1);
  const logRef = useRef<HTMLPreElement | null>(null);

  useEffect(() => {
    let mounted = true;
    const unlisteners: (() => void)[] = [];

    (async () => {
      unlisteners.push(
        await listen<ProgressEvent>("progress", (event) => {
          if (mounted) setMessage(event.payload.message);
        }),
      );
      unlisteners.push(
        await listen<string>("install-log", (event) => {
          if (mounted) setLines((prev) => [...prev, event.payload]);
        }),
      );

      // StrictMode menjalankan efek dua kali; alur konversi hanya dimulai
      // sekali per percobaan.
      if (startedAttempt.current === attempt) return;
      startedAttempt.current = attempt;

      try {
        setPhase("convert");
        setMessage(t("progress.converting"));
        const built = await api.build({
          path: analysis.path,
          output_dir: settings.output_dir,
          include_install_script: true,
          compression_level: 10,
          keep_staging: false,
        });
        setResult(built);
        setPhase("install");
        setMessage(t("progress.installing"));

        const installed = await api.install(built.package_path, settings.keep_packages);
        setOutcome(installed);
        // Event `install-log` mengisi log saat berjalan; bila tidak ada yang
        // diterima (mis. kegagalan sebelum pacman dijalankan), pakai salinan
        // dari hasil agar pengguna tetap melihat penyebabnya.
        setLines((prev) => (prev.length > 0 ? prev : installed.log));
        if (installed.success) {
          setPhase("done");
          setMessage(t("progress.success"));
        } else {
          const reason = installed.diagnosis_message ?? t("progress.failed");
          setPhase("error");
          setMessage(reason);
          setError(reason);
        }
      } catch (e) {
        const reason = String(e);
        setPhase("error");
        setMessage(reason);
        setError(reason);
      }
    })();

    return () => {
      mounted = false;
      for (const fn of unlisteners) fn();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attempt]);

  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines, error]);

  const activeIndex = STAGES.findIndex((stage) => stage.id === phase);
  const busy = phase === "convert" || phase === "install";

  function retry() {
    setLines([]);
    setResult(null);
    setOutcome(null);
    setError(null);
    setPhase("convert");
    setAttempt((n) => n + 1);
  }

  return (
    <div className="mx-auto flex w-full max-w-[46rem] flex-col gap-4">
      <header className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          {busy ? (
            <Spinner size={16} />
          ) : phase === "done" ? (
            <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[var(--success-soft)] text-[var(--success)]">
              <IconCheck size={13} />
            </span>
          ) : (
            <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[var(--error-soft)] text-[var(--error)]">
              <IconAlert size={13} />
            </span>
          )}
          <div className="min-w-0">
            <h1 className="truncate text-[18px] font-semibold leading-tight tracking-tight">
              {analysis.package}
            </h1>
            <p className="truncate text-[11.5px] text-[var(--faint)]">
              {analysis.version} · {analysis.arch}
            </p>
          </div>
        </div>
      </header>

      <Card className="overflow-hidden">
        {/* Garis kemajuan: tak tentu selama bekerja, penuh saat selesai. */}
        <div className="h-0.5 w-full overflow-hidden bg-[var(--surface-2)]">
          {busy ? (
            <div className="h-full w-1/3 animate-[shimmer_1.4s_ease-in-out_infinite] bg-[var(--accent)]" />
          ) : phase === "done" ? (
            <div className="h-full w-full bg-[var(--success)]" />
          ) : null}
        </div>

        <div className="p-3.5">
          <ol className="flex flex-wrap items-center gap-1.5">
            {STAGES.map((stage, index) => {
              const state =
                phase === "error" && index === activeIndex
                  ? "error"
                  : index < activeIndex || phase === "done"
                    ? "done"
                    : index === activeIndex
                      ? "active"
                      : "pending";
              const styles =
                state === "error"
                  ? "border-[var(--error)]/50 bg-[var(--error-soft)] text-[var(--error)]"
                  : state === "done"
                    ? "border-[var(--success)]/40 bg-[var(--success-soft)] text-[var(--success)]"
                    : state === "active"
                      ? "border-[var(--accent)]/50 bg-[var(--accent-soft)] text-[var(--accent)]"
                      : "border-[var(--border)] text-[var(--faint)]";
              return (
                <li key={stage.id} className="flex items-center gap-1.5">
                  <span
                    className={`flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-[11.5px] font-medium ${styles}`}
                  >
                    <span className="grid h-3.5 w-3.5 place-items-center rounded-full border border-current text-[8.5px] font-semibold">
                      {state === "done" ? "✓" : index + 1}
                    </span>
                    {t(stage.labelKey)}
                  </span>
                  {index < STAGES.length - 1 ? (
                    <span aria-hidden className="h-px w-2 bg-[var(--border)]" />
                  ) : null}
                </li>
              );
            })}
          </ol>
          {busy ? <p className="mt-2.5 text-[12.5px] text-[var(--muted)]">{message}</p> : null}
        </div>
      </Card>

      {error ? (
        <Banner tone="error" title={t("progress.failed")}>
          {error}
        </Banner>
      ) : null}

      {phase === "done" && result ? (
        <Banner tone="success" title={t("progress.success")}>
          <p className="tabular-nums">
            {result.pkgname} {result.pkgver}-{result.pkgrel} · {formatBytes(result.installed_size)} ·{" "}
            {result.file_count} {t("progress.files")}
          </p>
        </Banner>
      ) : null}

      <Card className="overflow-hidden p-0">
        <div className="flex items-center justify-between gap-2 border-b border-[var(--border)] px-3.5 py-2">
          <span className="text-[10.5px] font-semibold uppercase tracking-wider text-[var(--faint)]">
            {t("progress.log")}
          </span>
          {result ? (
            <Button variant="subtle" size="sm" onClick={() => void revealItemInDir(result.package_path)}>
              <IconFolder size={13} />
              {t("progress.openFolder")}
            </Button>
          ) : null}
        </div>
        <pre ref={logRef} className="log max-h-64 overflow-auto bg-[var(--bg-2)] p-3.5">
          {lines.length === 0 ? t("progress.logEmpty") : lines.join("\n")}
        </pre>
      </Card>

      <div className="flex justify-end gap-2">
        {phase === "done" ? (
          <Button onClick={onDone}>
            <IconCheck size={14} />
            {t("progress.done")}
          </Button>
        ) : null}
        {phase === "error" ? (
          <>
            <Button variant="subtle" onClick={onDone}>
              <IconX size={14} />
              {t("common.close")}
            </Button>
            <Button variant="ghost" onClick={onRetry}>
              {t("review.back")}
            </Button>
            <Button onClick={retry}>
              <IconRefresh size={14} />
              {t("progress.retry")}
            </Button>
          </>
        ) : null}
      </div>

      {outcome && !outcome.success ? (
        <p className="text-[11.5px] text-[var(--faint)]">
          {outcome.diagnosis ? `${t("progress.diagnosis")}: ${outcome.diagnosis}` : null}
        </p>
      ) : null}
    </div>
  );
}
