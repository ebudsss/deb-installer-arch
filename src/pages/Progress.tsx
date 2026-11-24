import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, formatBytes, type Analysis, type BuildResult, type InstallOutcome, type Progress as ProgressEvent, type Settings } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Banner, Button, Card, Spinner } from "../components/ui";

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
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-4">
      <div className="flex items-center gap-3">
        {busy ? <Spinner /> : null}
        <h1 className="text-xl font-semibold tracking-tight">
          {analysis.package} {analysis.version}
        </h1>
      </div>

      <Card className="p-4">
        <ol className="flex flex-wrap gap-2">
          {STAGES.map((stage, index) => {
            const state =
              phase === "error" && index === activeIndex
                ? "error"
                : index < activeIndex || phase === "done"
                  ? "done"
                  : index === activeIndex
                    ? "active"
                    : "pending";
            const color =
              state === "error"
                ? "border-[var(--error)] text-[var(--error)]"
                : state === "done"
                  ? "border-[var(--success)] text-[var(--success)]"
                  : state === "active"
                    ? "border-[var(--accent)] text-[var(--accent)]"
                    : "border-[var(--border)] text-[var(--muted)]";
            return (
              <li
                key={stage.id}
                className={`flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${color}`}
              >
                <span className="grid h-4 w-4 place-items-center rounded-full border border-current text-[10px]">
                  {index + 1}
                </span>
                {t(stage.labelKey)}
              </li>
            );
          })}
        </ol>
        <p className="mt-3 text-sm text-[var(--muted)]">{message}</p>
      </Card>

      {error ? (
        <Banner tone="error" title={t("progress.failed")}>
          {error}
        </Banner>
      ) : null}

      {phase === "done" && result ? (
        <Banner tone="success" title={t("progress.success")}>
          <p>
            {result.pkgname} {result.pkgver}-{result.pkgrel} · {formatBytes(result.installed_size)} ·{" "}
            {result.file_count} {t("progress.files")}
          </p>
        </Banner>
      ) : null}

      <Card className="p-0">
        <div className="flex items-center justify-between border-b border-[var(--border)] px-4 py-2">
          <span className="text-xs font-semibold uppercase tracking-wide text-[var(--muted)]">
            {t("progress.log")}
          </span>
          {result ? (
            <Button variant="ghost" onClick={() => void revealItemInDir(result.package_path)}>
              {t("progress.openFolder")}
            </Button>
          ) : null}
        </div>
        <pre ref={logRef} className="log max-h-72 overflow-auto p-4">
          {lines.length === 0 ? "…" : lines.join("\n")}
        </pre>
      </Card>

      <div className="flex justify-end gap-2">
        {phase === "done" ? <Button onClick={onDone}>{t("progress.done")}</Button> : null}
        {phase === "error" ? (
          <>
            <Button variant="ghost" onClick={onDone}>
              {t("common.close")}
            </Button>
            <Button variant="ghost" onClick={onRetry}>
              {t("review.back")}
            </Button>
            <Button onClick={retry}>{t("progress.retry")}</Button>
          </>
        ) : null}
      </div>

      {outcome && !outcome.success ? (
        <p className="text-xs text-[var(--muted)]">
          {outcome.diagnosis ? `${t("progress.diagnosis")}: ${outcome.diagnosis}` : null}
        </p>
      ) : null}
    </div>
  );
}
