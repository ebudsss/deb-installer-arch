import { useCallback, useEffect, useMemo, useState } from "react";
import brandSymbol from "../assets/brand-symbol.png";
import { api, type InstalledEntry } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { useToast } from "../components/toast";
import {
  Banner,
  Button,
  Card,
  EmptyState,
  IconButton,
  Modal,
  Pill,
  SectionHeader,
  Skeleton,
  StatCard,
} from "../components/ui";
import {
  IconBox,
  IconCheck,
  IconClock,
  IconFolder,
  IconPackage,
  IconRefresh,
  IconPlay,
  IconSearch,
  IconTrash,
  IconX,
} from "../components/icons";

type Filter = "all" | "installed" | "pending";

function isInstalled(record: InstalledEntry): boolean {
  return record.installed_at !== null && record.uninstalled_at === null;
}

export function Installed({
  onError,
  onGoHome,
}: {
  onError: (message: string) => void;
  onGoHome: () => void;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const [records, setRecords] = useState<InstalledEntry[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [target, setTarget] = useState<InstalledEntry | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [log, setLog] = useState<string[]>([]);

  const reload = useCallback(() => {
    api
      .listInstalled()
      .then(setRecords)
      .catch((e) => {
        setRecords([]);
        onError(String(e));
      });
  }, [onError]);

  useEffect(() => {
    reload();
  }, [reload]);

  const stats = useMemo(() => {
    const list = records ?? [];
    const installed = list.filter(isInstalled).length;
    return {
      total: list.length,
      installed,
      pending: list.length - installed,
    };
  }, [records]);

  const visible = useMemo(() => {
    const list = records ?? [];
    const needle = query.trim().toLowerCase();
    return list.filter((record) => {
      if (filter === "installed" && !isInstalled(record)) return false;
      if (filter === "pending" && isInstalled(record)) return false;
      if (!needle) return true;
      return (
        record.pkgname.toLowerCase().includes(needle) ||
        record.deb_name.toLowerCase().includes(needle) ||
        record.deb_version.toLowerCase().includes(needle)
      );
    });
  }, [records, query, filter]);

  async function open(record: InstalledEntry) {
    try {
      await api.launch(record.pkgname);
    } catch (e) {
      onError(String(e));
    }
  }

  async function uninstall(record: InstalledEntry) {
    setBusy(record.pkgname);
    setLog([]);
    try {
      const outcome = await api.uninstall(record.pkgname);
      setLog(outcome.log);
      if (outcome.success) {
        toast.success(t("installed.uninstallDone"));
      } else {
        onError(outcome.diagnosis_message ?? t("installed.removeFailed"));
      }
      reload();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(null);
      setTarget(null);
    }
  }

  async function forget(record: InstalledEntry) {
    try {
      await api.deleteRecord(record.pkgname);
      reload();
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-[64rem] flex-col gap-5">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight">
            {t("installed.title")}
          </h1>
          <p className="mt-1 max-w-lg text-[13px] leading-relaxed text-[var(--muted)]">
            {t("installed.subtitle")}
          </p>
        </div>
        <Button variant="ghost" size="sm" onClick={reload}>
          <IconRefresh size={14} />
          {t("common.refresh")}
        </Button>
      </header>

      {records === null ? (
        <div className="grid gap-2.5 sm:grid-cols-3">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className="rounded-xl border border-[var(--border)] bg-[var(--surface)] px-3.5 py-3"
            >
              <Skeleton width="45%" height={10} />
              <div className="mt-2.5">
                <Skeleton width="30%" height={20} />
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="grid gap-2.5 sm:grid-cols-3">
          <StatCard
            label={t("installed.stat.total")}
            value={stats.total}
            hint={t("installed.stat.totalHint")}
            icon={<IconPackage size={14} />}
          />
          <StatCard
            label={t("installed.stat.installed")}
            value={stats.installed}
            hint={t("installed.stat.installedHint")}
            icon={<IconCheck size={14} />}
            tone="success"
          />
          <StatCard
            label={t("installed.stat.pending")}
            value={stats.pending}
            hint={t("installed.stat.pendingHint")}
            icon={<IconClock size={14} />}
            tone="accent"
          />
        </div>
      )}

      {records !== null && records.length === 0 ? (
        <EmptyState
          illustration={<img src={brandSymbol} alt="" className="h-20 w-20 object-contain" />}
          title={t("installed.empty")}
          description={t("installed.emptyBody")}
          action={<Button onClick={onGoHome}>{t("installed.emptyAction")}</Button>}
        />
      ) : records !== null ? (
        <Card className="overflow-hidden">
          {/* Toolbar */}
          <div className="flex flex-wrap items-center gap-2 border-b border-[var(--border)] px-3 py-2.5">
            <div className="relative min-w-[12rem] flex-1">
              <span className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-[var(--faint)]">
                <IconSearch size={14} />
              </span>
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("installed.search")}
                aria-label={t("installed.search")}
                className="h-8 w-full rounded-lg border border-[var(--border)] bg-[var(--bg-2)] pl-8 pr-8 text-[12.5px] text-[var(--fg)] placeholder:text-[var(--faint)] focus:border-[var(--accent)] focus:outline-none"
              />
              {query ? (
                <button
                  onClick={() => setQuery("")}
                  aria-label={t("common.cancel")}
                  className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 text-[var(--faint)] hover:text-[var(--fg)]"
                >
                  <IconX size={12} />
                </button>
              ) : null}
            </div>
            <div className="flex items-center gap-0.5 rounded-lg bg-[var(--surface-2)] p-0.5">
              {(
                [
                  ["all", t("installed.stat.total")],
                  ["installed", t("installed.stat.installed")],
                  ["pending", t("installed.stat.pending")],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  onClick={() => setFilter(id)}
                  aria-pressed={filter === id}
                  className={`rounded-md px-2 py-1 text-[11.5px] font-medium transition-colors ${
                    filter === id
                      ? "bg-[var(--surface)] text-[var(--fg)] shadow-[var(--shadow-sm)]"
                      : "text-[var(--muted)] hover:text-[var(--fg)]"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
            <IconButton label={t("common.refresh")} onClick={reload}>
              <IconRefresh size={14} />
            </IconButton>
          </div>

          {visible.length === 0 ? (
            <div className="px-4 py-10 text-center">
              <p className="text-[13px] font-medium">{t("installed.searchEmpty")}</p>
              <p className="mt-1 text-[12px] text-[var(--muted)]">{t("installed.searchEmptyBody")}</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full min-w-[40rem] border-collapse text-left">
                <thead>
                  <tr className="border-b border-[var(--border)] text-[10.5px] uppercase tracking-wider text-[var(--faint)]">
                    <th className="px-3.5 py-2 font-medium">{t("installed.package")}</th>
                    <th className="px-3.5 py-2 font-medium">{t("installed.version")}</th>
                    <th className="px-3.5 py-2 font-medium">{t("installed.status")}</th>
                    <th className="px-3.5 py-2 font-medium">{t("installed.source")}</th>
                    <th className="px-3.5 py-2 text-right font-medium">{t("installed.actions")}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[var(--border)]">
                  {visible.map((record) => {
                    const installed = isInstalled(record);
                    const removed = record.uninstalled_at !== null;
                    return (
                      <tr
                        key={record.id}
                        className="group transition-colors hover:bg-[var(--surface-2)]"
                      >
                        <td className="px-3.5 py-2.5">
                          <div className="flex items-center gap-2.5">
                            <span className="grid h-7 w-7 shrink-0 place-items-center rounded-lg bg-[var(--surface-2)] text-[var(--accent)]">
                              <IconBox size={13} />
                            </span>
                            <code className="truncate text-[12.5px] font-medium">
                              {record.pkgname}
                            </code>
                          </div>
                        </td>
                        <td className="whitespace-nowrap px-3.5 py-2.5 text-[12.5px] tabular-nums text-[var(--muted)]">
                          {record.pkgver}
                        </td>
                        <td className="px-3.5 py-2.5">
                          {installed ? (
                            <Pill tone="success">{t("installed.installed")}</Pill>
                          ) : removed ? (
                            <Pill tone="neutral">{t("installed.removed")}</Pill>
                          ) : (
                            <Pill tone="accent">{t("installed.converted")}</Pill>
                          )}
                        </td>
                        <td className="max-w-[16rem] px-3.5 py-2.5">
                          <div className="truncate text-[12px]">{record.deb_name}</div>
                          <div
                            className="mt-0.5 flex items-center gap-1 truncate text-[10.5px] text-[var(--faint)]"
                            title={record.deb_path}
                          >
                            <IconFolder size={9} className="shrink-0" />
                            <span className="truncate">{record.deb_path}</span>
                          </div>
                        </td>
                        <td className="whitespace-nowrap px-3.5 py-2.5">
                          <div className="flex items-center justify-end gap-1.5">
                            {installed && record.launcher ? (
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => void open(record)}
                                title={t("installed.openHint")}
                              >
                                <IconPlay size={12} />
                                {t("installed.open")}
                              </Button>
                            ) : null}
                            {installed ? (
                              <Button
                                variant="danger"
                                size="sm"
                                onClick={() => setTarget(record)}
                                disabled={busy === record.pkgname}
                              >
                                {busy === record.pkgname ? (
                                  <IconRefresh size={13} className="animate-spin" />
                                ) : (
                                  <IconTrash size={13} />
                                )}
                                {t("installed.uninstall")}
                              </Button>
                            ) : (
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => void forget(record)}
                                title={t("installed.forget")}
                              >
                                {t("installed.forget")}
                              </Button>
                            )}
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </Card>
      ) : null}

      {log.length > 0 ? (
        <section>
          <SectionHeader title={t("progress.log")} />
          <Card className="p-0">
            <pre className="log max-h-52 overflow-auto p-3.5">{log.join("\n")}</pre>
          </Card>
        </section>
      ) : null}

      <Modal
        open={target !== null}
        onClose={() => setTarget(null)}
        title={t("installed.confirmTitle")}
        description={t("installed.confirmBody")}
        footer={
          <>
            <Button variant="ghost" onClick={() => setTarget(null)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="danger"
              onClick={() => target && void uninstall(target)}
              disabled={busy !== null}
            >
              {busy !== null ? <IconRefresh size={13} className="animate-spin" /> : <IconTrash size={13} />}
              {t("installed.uninstall")}
            </Button>
          </>
        }
      >
        {target ? (
          <div className="rounded-lg border border-[var(--border)] bg-[var(--bg-2)] px-3 py-2.5">
            <div className="flex items-center gap-2">
              <code className="text-[13px] font-medium">{target.pkgname}</code>
              <span className="text-[12px] tabular-nums text-[var(--muted)]">{target.pkgver}</span>
            </div>
            <div className="mt-1 truncate text-[11px] text-[var(--faint)]">{target.deb_path}</div>
          </div>
        ) : null}
        <div className="mt-3">
          <Banner tone="warning">{t("installed.confirmWarning")}</Banner>
        </div>
      </Modal>
    </div>
  );
}
