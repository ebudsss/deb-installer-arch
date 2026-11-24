import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Settings as SettingsModel } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Banner, Button, Card } from "../components/ui";

export function SettingsPage({
  settings,
  onChange,
  onError,
}: {
  settings: SettingsModel;
  onChange: (settings: SettingsModel) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<SettingsModel>(settings);
  const [saved, setSaved] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshLog, setRefreshLog] = useState<string[]>([]);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);

  /// Bahasa dan tema berlaku langsung; sisanya menunggu tombol Simpan.
  function applyNow(next: SettingsModel) {
    setDraft(next);
    onChange(next);
    void api.saveSettings(next).catch((e) => onError(String(e)));
  }

  async function save() {
    try {
      await api.saveSettings(draft);
      onChange(draft);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      onError(String(e));
    }
  }

  async function pickDir() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setDraft((prev) => ({ ...prev, output_dir: selected }));
    }
  }

  async function refreshDb() {
    setRefreshing(true);
    setRefreshLog([]);
    try {
      const ok = await api.refreshFileDb();
      setRefreshLog([ok ? t("settings.refreshOk") : t("settings.refreshFailed")]);
    } catch (e) {
      onError(String(e));
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
      <h1 className="text-2xl font-semibold tracking-tight">{t("settings.title")}</h1>

      <Card className="divide-y divide-[var(--border)]">
        <div className="grid grid-cols-[12rem_1fr] items-center gap-4 px-4 py-3">
          <span id="language-label" className="text-sm text-[var(--muted)]">
            {t("settings.language")}
          </span>
          <div id="language" role="group" aria-labelledby="language-label" className="flex gap-1">
            {(
              [
                ["id", "Bahasa Indonesia"],
                ["en", "English"],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                onClick={() => applyNow({ ...draft, language: value })}
                aria-pressed={draft.language === value}
                className={`rounded-lg border px-3 py-1.5 text-sm ${
                  draft.language === value
                    ? "border-[var(--accent)] text-[var(--accent)]"
                    : "border-[var(--border)] text-[var(--muted)] hover:text-[var(--fg)]"
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-[12rem_1fr] items-center gap-4 px-4 py-3">
          <span id="theme-label" className="text-sm text-[var(--muted)]">
            {t("settings.theme")}
          </span>
          <div id="theme" role="group" aria-labelledby="theme-label" className="flex gap-1">
            {(
              [
                ["system", t("settings.theme.system")],
                ["light", t("settings.theme.light")],
                ["dark", t("settings.theme.dark")],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                onClick={() => applyNow({ ...draft, theme: value })}
                aria-pressed={draft.theme === value}
                className={`rounded-lg border px-3 py-1.5 text-sm ${
                  draft.theme === value
                    ? "border-[var(--accent)] text-[var(--accent)]"
                    : "border-[var(--border)] text-[var(--muted)] hover:text-[var(--fg)]"
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-[12rem_1fr] items-center gap-4 px-4 py-3">
          <span className="text-sm text-[var(--muted)]">{t("settings.output")}</span>
          <div className="flex min-w-0 items-center gap-2">
            <code className="min-w-0 flex-1 truncate rounded-lg bg-[var(--surface-2)] px-3 py-1.5 text-xs">
              {draft.output_dir ?? t("settings.default")}
            </code>
            <Button variant="ghost" onClick={pickDir}>
              {t("settings.choose")}
            </Button>
            {draft.output_dir ? (
              <Button variant="ghost" onClick={() => setDraft({ ...draft, output_dir: null })}>
                {t("settings.default")}
              </Button>
            ) : null}
          </div>
        </div>

        <label className="flex items-center justify-between gap-4 px-4 py-3">
          <span className="text-sm text-[var(--muted)]">{t("settings.keep")}</span>
          <input
            type="checkbox"
            checked={draft.keep_packages}
            onChange={(e) => setDraft({ ...draft, keep_packages: e.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
        </label>

        <label className="flex items-center justify-between gap-4 px-4 py-3">
          <span className="text-sm text-[var(--muted)]">{t("settings.aur")}</span>
          <input
            type="checkbox"
            checked={draft.check_aur}
            onChange={(e) => setDraft({ ...draft, check_aur: e.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
        </label>

        <div className="px-4 py-3">
          <label className="mb-2 block text-sm text-[var(--muted)]" htmlFor="mapping">
            {t("settings.mapping")}
          </label>
          <textarea
            id="mapping"
            value={draft.custom_mapping}
            onChange={(e) => setDraft({ ...draft, custom_mapping: e.target.value })}
            placeholder={t("settings.mappingHint")}
            spellCheck={false}
            rows={8}
            className="log w-full rounded-lg border border-[var(--border)] bg-[var(--surface-2)] p-3"
          />
        </div>
      </Card>

      <div className="flex items-center gap-3">
        <Button onClick={save} disabled={!dirty}>
          {t("settings.save")}
        </Button>
        {saved ? <span className="text-sm text-[var(--success)]">{t("settings.saved")}</span> : null}
      </div>

      <Card className="p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-sm font-medium">{t("settings.refreshDb")}</p>
            <p className="text-xs text-[var(--muted)]">{t("settings.refreshHint")}</p>
          </div>
          <Button variant="ghost" onClick={refreshDb} disabled={refreshing}>
            {refreshing ? t("settings.refreshing") : t("settings.refreshDb")}
          </Button>
        </div>
        {refreshLog.length > 0 ? (
          <pre className="log mt-3 rounded bg-[var(--surface-2)] p-2">{refreshLog.join("\n")}</pre>
        ) : null}
      </Card>

      <Banner tone="info">{t("settings.mappingAbout")}</Banner>
    </div>
  );
}
