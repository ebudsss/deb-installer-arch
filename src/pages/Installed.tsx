import { useCallback, useEffect, useState } from "react";
import { api, type InstallRecord } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Banner, Button, Card, Pill, Spinner } from "../components/ui";

export function Installed({ onError }: { onError: (message: string) => void }) {
  const { t } = useI18n();
  const [records, setRecords] = useState<InstallRecord[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
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

  async function uninstall(record: InstallRecord) {
    setBusy(record.pkgname);
    setLog([]);
    try {
      const outcome = await api.uninstall(record.pkgname);
      setLog(outcome.log);
      if (!outcome.success) {
        onError(outcome.diagnosis_message ?? t("installed.removeFailed"));
      }
      reload();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(null);
      setConfirming(null);
    }
  }

  async function forget(record: InstallRecord) {
    try {
      await api.deleteRecord(record.pkgname);
      reload();
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold tracking-tight">{t("installed.title")}</h1>
        <Button variant="ghost" onClick={reload}>
          {t("installed.refresh")}
        </Button>
      </div>

      {records === null ? (
        <div className="flex items-center gap-2 text-[var(--muted)]">
          <Spinner /> {t("common.loading")}
        </div>
      ) : records.length === 0 ? (
        <Banner tone="info">{t("installed.empty")}</Banner>
      ) : (
        <Card>
          <table className="w-full text-left text-sm">
            <thead className="border-b border-[var(--border)] text-xs uppercase tracking-wide text-[var(--muted)]">
              <tr>
                <th className="px-4 py-2 font-medium">{t("installed.package")}</th>
                <th className="px-4 py-2 font-medium">{t("installed.version")}</th>
                <th className="px-4 py-2 font-medium">{t("installed.status")}</th>
                <th className="px-4 py-2 font-medium">{t("installed.source")}</th>
                <th className="px-4 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--border)]">
              {records.map((record) => {
                const installed = record.installed_at !== null && record.uninstalled_at === null;
                const removed = record.uninstalled_at !== null;
                return (
                  <tr key={record.id} className="align-middle">
                    <td className="px-4 py-2">
                      <code>{record.pkgname}</code>
                    </td>
                    <td className="px-4 py-2 text-[var(--muted)]">{record.pkgver}</td>
                    <td className="px-4 py-2">
                      {installed ? (
                        <Pill tone="success">{t("installed.installed")}</Pill>
                      ) : removed ? (
                        <Pill tone="neutral">{t("installed.removed")}</Pill>
                      ) : (
                        <Pill tone="accent">{t("installed.converted")}</Pill>
                      )}
                    </td>
                    <td className="max-w-[18rem] px-4 py-2">
                      <span className="block truncate text-xs text-[var(--muted)]" title={record.deb_path}>
                        {record.deb_name} {record.deb_version}
                      </span>
                    </td>
                    <td className="px-4 py-2 text-right">
                      {confirming === record.pkgname ? (
                        <span className="inline-flex gap-1">
                          <Button variant="danger" disabled={busy === record.pkgname} onClick={() => void uninstall(record)}>
                            {t("installed.yes")}
                          </Button>
                          <Button variant="ghost" onClick={() => setConfirming(null)}>
                            {t("installed.cancel")}
                          </Button>
                        </span>
                      ) : (
                        <span className="inline-flex gap-1">
                          {installed ? (
                            <Button variant="danger" onClick={() => setConfirming(record.pkgname)}>
                              {t("installed.uninstall")}
                            </Button>
                          ) : (
                            <Button variant="ghost" onClick={() => void forget(record)}>
                              {t("installed.forget")}
                            </Button>
                          )}
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </Card>
      )}

      {log.length > 0 ? (
        <Card className="p-0">
          <pre className="log max-h-56 overflow-auto p-4">{log.join("\n")}</pre>
        </Card>
      ) : null}

    </div>
  );
}
