import { useMemo, useState } from "react";
import { formatBytes, type Analysis } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Banner, Button, Card, Pill, RiskBadge, StatusPill, Tabs } from "../components/ui";
import { IconArrowLeft, IconCheck } from "../components/icons";

type TabId = "info" | "deps" | "files" | "scripts";

export function Review({
  analysis,
  onBack,
  onInstall,
}: {
  analysis: Analysis;
  onBack: () => void;
  onInstall: () => void;
}) {
  const { t } = useI18n();
  const [tab, setTab] = useState<TabId>("info");
  const [acknowledged, setAcknowledged] = useState(false);

  const blocking = analysis.dep_summary.aur + analysis.dep_summary.missing;
  const forbidden = analysis.sensitive.filter((s) => s.level === "Forbidden");
  const needsAck = analysis.max_risk === "Dangerous";
  const canInstall = !analysis.blocked && (!needsAck || acknowledged);

  return (
    <div className="mx-auto flex w-full max-w-[60rem] flex-col gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            <h1 className="truncate text-[22px] font-semibold leading-tight tracking-tight">
              {analysis.package}
            </h1>
            <Pill tone="neutral">{analysis.version}</Pill>
            <Pill tone="accent">{analysis.arch}</Pill>
            <RiskBadge risk={analysis.max_risk} />
          </div>
          <p className="mt-1 line-clamp-2 max-w-xl text-[13px] leading-relaxed text-[var(--muted)]">
            {analysis.description.split("\n")[0]}
          </p>
        </div>
        <div className="flex shrink-0 gap-2">
          <Button variant="ghost" onClick={onBack}>
            <IconArrowLeft size={14} />
            {t("review.back")}
          </Button>
          <Button onClick={onInstall} disabled={!canInstall}>
            <IconCheck size={14} />
            {t("review.install")}
          </Button>
        </div>
      </div>

      {analysis.blocked ? (
        <Banner tone="error" title={t("review.blocked")}>
          <ul className="mt-2 list-disc space-y-1 pl-5">
            {forbidden.map((item) => (
              <li key={item.path}>
                <code>{item.path}</code> — {item.reason}
              </li>
            ))}
          </ul>
        </Banner>
      ) : null}

      {needsAck && !analysis.blocked ? (
        <Banner tone="error" title={t("review.dangerTitle")}>
          <ul className="mt-1 space-y-1">
            {analysis.scripts.findings
              .filter((finding) => finding.risk === "Dangerous")
              .map((finding, index) => (
                <li key={`${finding.script}-${finding.line}-${index}`}>
                  <code className="text-xs">
                    {finding.script}:{finding.line}
                  </code>{" "}
                  <code className="text-xs">{finding.text}</code>
                </li>
              ))}
          </ul>
          <label className="mt-3 flex items-start gap-2 text-[12.5px]">
            <input
              type="checkbox"
              checked={acknowledged}
              onChange={(event) => setAcknowledged(event.target.checked)}
              className="mt-0.5 h-4 w-4 shrink-0 accent-[var(--error)]"
            />
            <span>{t("review.dangerAck")}</span>
          </label>
        </Banner>
      ) : null}

      {blocking > 0 && !analysis.blocked ? (
        <Banner tone="warning">
          <ul className="space-y-0.5">
            {analysis.dep_summary.aur > 0 ? <li>{t("deps.warnAur")}</li> : null}
            {analysis.dep_summary.missing > 0 ? <li>{t("deps.warnMissing")}</li> : null}
          </ul>
        </Banner>
      ) : null}

      {analysis.sensitive.length > 0 && !analysis.blocked ? (
        <Banner tone="warning" title={t("info.sensitive")}>
          <ul className="mt-1 list-disc space-y-1 pl-5">
            {analysis.sensitive.map((item) => (
              <li key={item.path}>
                <code>{item.path}</code> — {item.reason}
              </li>
            ))}
          </ul>
        </Banner>
      ) : null}

      <Card>
        <Tabs<TabId>
          active={tab}
          onChange={setTab}
          tabs={[
            { id: "info", label: t("review.tab.info") },
            { id: "deps", label: t("review.tab.deps"), count: analysis.dep_summary.total },
            { id: "files", label: t("review.tab.files"), count: analysis.file_count },
            {
              id: "scripts",
              label: t("review.tab.scripts"),
              count: analysis.scripts.findings.length,
            },
          ]}
        />
        <div className="p-3.5">
          {tab === "info" ? <InfoTab analysis={analysis} /> : null}
          {tab === "deps" ? <DepsTab analysis={analysis} /> : null}
          {tab === "files" ? <FilesTab analysis={analysis} /> : null}
          {tab === "scripts" ? <ScriptsTab analysis={analysis} /> : null}
        </div>
      </Card>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[9.5rem_1fr] items-baseline gap-3 py-1.5">
      <dt className="text-[12px] text-[var(--faint)]">{label}</dt>
      <dd className="min-w-0 break-words text-[12.5px]">{children}</dd>
    </div>
  );
}

function InfoTab({ analysis }: { analysis: Analysis }) {
  const { t } = useI18n();
  return (
    <div className="space-y-5">
      <dl className="divide-y divide-[var(--border)]/70">
        <Row label={t("info.package")}>{analysis.package}</Row>
        <Row label={t("info.version")}>{analysis.version}</Row>
        <Row label={t("info.arch")}>
          {analysis.arch} → {analysis.arch_mapped}
        </Row>
        <Row label={t("info.pkgname")}>
          <code>{analysis.suggested_pkgname}</code>
        </Row>
        <Row label={t("info.maintainer")}>{analysis.maintainer || "—"}</Row>
        {analysis.homepage ? (
          <Row label={t("info.homepage")}>
            <a className="text-[var(--accent)] hover:underline" href={analysis.homepage} target="_blank" rel="noreferrer">
              {analysis.homepage}
            </a>
          </Row>
        ) : null}
        <Row label={t("info.fileSize")}>{formatBytes(analysis.file_size)}</Row>
        <Row label={t("info.installedSize")}>{formatBytes(analysis.installed_size)}</Row>
        <Row label={t("info.fileCount")}>{analysis.file_count}</Row>
        <Row label={t("info.dataMember")}>
          <code>{analysis.data_member}</code>
        </Row>
        <Row label={t("info.sha256")}>
          <code className="break-all font-mono text-[11px] text-[var(--muted)]">{analysis.sha256}</code>
        </Row>
      </dl>

      {analysis.alternatives.length > 0 ? (
        <Banner tone="info" title={t("info.alternatives")}>
          <p className="mb-2">{t("info.alternativesHint")}</p>
          <ul className="space-y-1">
            {analysis.alternatives.map((alt) => (
              <li key={`${alt.source}-${alt.name}`} className="flex flex-wrap items-center gap-2">
                <code>{alt.name}</code>
                <Pill tone={alt.source === "Repo" ? "success" : "warning"}>
                  {alt.source === "Repo" ? "repo" : "AUR"}
                </Pill>
                {alt.version ? <span className="text-xs">{alt.version}</span> : null}
                {alt.description ? (
                  <span className="text-xs text-[var(--muted)]">{alt.description}</span>
                ) : null}
              </li>
            ))}
          </ul>
        </Banner>
      ) : null}
    </div>
  );
}

function DepsTab({ analysis }: { analysis: Analysis }) {
  const { t } = useI18n();
  const { dep_summary: summary } = analysis;
  if (analysis.deps.length === 0) {
    return <p className="text-[var(--muted)]">{t("deps.allClear")}</p>;
  }
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-1.5">
        {(
          [
            [summary.installed, t("common.status.Installed"), "success"],
            [summary.available, t("common.status.Available"), "accent"],
            [summary.aur, "AUR", "warning"],
            [summary.missing, t("common.status.Missing"), "error"],
            [summary.ignored, t("common.status.Ignored"), "neutral"],
          ] as const
        )
          .filter(([count]) => count > 0)
          .map(([count, label, tone]) => (
            <Pill key={label} tone={tone}>
              <span className="tabular-nums">{count}</span> {label}
            </Pill>
          ))}
        <span className="ml-1 text-[11.5px] text-[var(--faint)]">
          {summary.total} {t("deps.summary")}
        </span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-[var(--border)] text-xs uppercase tracking-wide text-[var(--muted)]">
            <tr>
              <th className="py-2 pr-3 font-medium">{t("deps.debian")}</th>
              <th className="py-2 pr-3 font-medium">{t("deps.arch")}</th>
              <th className="py-2 pr-3 font-medium">{t("deps.status")}</th>
              <th className="py-2 font-medium">{t("deps.note")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--border)]">
            {analysis.deps.map((dep, index) => (
              <tr key={`${dep.debian}-${index}`} className="align-top">
                <td className="py-2 pr-3">
                  <code>{dep.debian}</code>
                  {dep.constraint ? (
                    <span className="ml-1 text-xs text-[var(--muted)]">{dep.constraint}</span>
                  ) : null}
                  {dep.alternatives.length > 1 ? (
                    <div className="text-xs text-[var(--muted)]">
                      {dep.alternatives.slice(1).join(" | ")}
                    </div>
                  ) : null}
                </td>
                <td className="py-2 pr-3">
                  {dep.arch_name ? <code>{dep.arch_name}</code> : <span className="text-[var(--muted)]">—</span>}
                </td>
                <td className="py-2 pr-3">
                  <StatusPill status={dep.status} confidence={dep.confidence} />
                </td>
                <td className="py-2 text-xs text-[var(--muted)]">{dep.note ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function FilesTab({ analysis }: { analysis: Analysis }) {
  const { t } = useI18n();
  const [limit, setLimit] = useState(300);
  const shown = useMemo(() => analysis.entries.slice(0, limit), [analysis.entries, limit]);

  if (analysis.entries.length === 0) {
    return <p className="text-[var(--muted)]">{t("files.empty")}</p>;
  }

  return (
    <div className="space-y-3">
      <div className="max-h-[26rem] overflow-auto rounded-lg border border-[var(--border)]">
        <table className="w-full text-left text-xs">
          <tbody className="divide-y divide-[var(--border)]">
            {shown.map((entry) => (
              <tr key={entry.path}>
                <td className="w-14 py-1.5 pl-3 text-[var(--muted)]">{entry.kind}</td>
                <td className="py-1.5 pr-3">
                  <code className="break-all">{entry.path}</code>
                  {entry.link_target ? (
                    <span className="text-[var(--muted)]"> → {entry.link_target}</span>
                  ) : null}
                </td>
                <td className="w-20 py-1.5 pr-3 text-right text-[var(--muted)]">
                  {entry.kind === "file" ? formatBytes(entry.size) : ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {limit < analysis.entries.length ? (
        <Button variant="ghost" onClick={() => setLimit((n) => n + 500)}>
          {t("files.more")} ({analysis.entries.length - limit})
        </Button>
      ) : null}
      {analysis.entries_truncated ? (
        <p className="text-xs text-[var(--muted)]">{t("files.truncated")}</p>
      ) : null}
    </div>
  );
}

function ScriptsTab({ analysis }: { analysis: Analysis }) {
  const { t } = useI18n();
  const { findings } = analysis.scripts;

  return (
    <div className="space-y-5">
      {findings.length === 0 ? (
        <p className="text-[var(--muted)]">{t("scripts.none")}</p>
      ) : (
        <ul className="space-y-2">
          {findings.map((finding, index) => (
            <li
              key={`${finding.script}-${finding.line}-${index}`}
              className="rounded-lg border border-[var(--border)] p-3"
            >
              <div className="flex flex-wrap items-center gap-2">
                <RiskBadge risk={finding.risk} />
                <code className="text-xs">
                  {finding.script}:{finding.line}
                </code>
                <code className="text-xs text-[var(--muted)]">{finding.pattern}</code>
              </div>
              <pre className="log mt-2 overflow-x-auto rounded bg-[var(--surface-2)] p-2">
                {finding.text}
              </pre>
              <p className="mt-2 text-xs leading-relaxed text-[var(--muted)]">{finding.advice}</p>
            </li>
          ))}
        </ul>
      )}

      <div>
        <h3 className="mb-2 text-sm font-semibold">{t("scripts.translated")}</h3>
        {analysis.install_script ? (
          <pre className="log max-h-64 overflow-auto rounded-lg border border-[var(--border)] bg-[var(--surface-2)] p-3">
            {analysis.install_script}
          </pre>
        ) : (
          <p className="text-sm text-[var(--muted)]">{t("scripts.noTranslation")}</p>
        )}
      </div>
    </div>
  );
}
