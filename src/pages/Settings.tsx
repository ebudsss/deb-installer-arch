import { useEffect, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Settings as SettingsModel } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { useToast } from "../components/toast";
import { Banner, Button, Card, Pill, Spinner } from "../components/ui";
import { IconCheck, IconFolder, IconGlobe, IconRefresh, IconShield } from "../components/icons";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--faint)]">
        {title}
      </h2>
      <Card className="divide-y divide-[var(--border)] overflow-hidden">{children}</Card>
    </section>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 px-3.5 py-3">
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium">{label}</p>
        {hint ? <p className="mt-0.5 text-[11.5px] leading-relaxed text-[var(--muted)]">{hint}</p> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/** Deretan tombol pilihan tunggal; dipakai untuk bahasa dan tema. */
function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: readonly (readonly [T, string])[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div role="group" aria-label={label} className="flex gap-0.5 rounded-lg bg-[var(--surface-2)] p-0.5">
      {options.map(([id, text]) => (
        <button
          key={id}
          onClick={() => onChange(id)}
          aria-pressed={value === id}
          className={`rounded-md px-2.5 py-1 text-[12px] font-medium transition-colors ${
            value === id
              ? "bg-[var(--surface)] text-[var(--fg)] shadow-[var(--shadow-sm)]"
              : "text-[var(--muted)] hover:text-[var(--fg)]"
          }`}
        >
          {text}
        </button>
      ))}
    </div>
  );
}

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
  const toast = useToast();
  const [draft, setDraft] = useState<SettingsModel>(settings);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);

  /** Bahasa dan tema berlaku langsung; sisanya menunggu tombol Simpan. */
  function applyNow(next: SettingsModel) {
    setDraft(next);
    onChange(next);
    api.saveSettings(next).catch((e) => onError(String(e)));
  }

  async function save() {
    try {
      await api.saveSettings(draft);
      onChange(draft);
      toast.success(t("settings.saved"));
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
    try {
      const ok = await api.refreshFileDb();
      if (ok) toast.success(t("settings.refreshOk"));
      else onError(t("settings.refreshFailed"));
    } catch (e) {
      onError(String(e));
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-[46rem] flex-col gap-5">
      <header>
        <h1 className="text-[22px] font-semibold leading-tight tracking-tight">
          {t("settings.title")}
        </h1>
        <p className="mt-1 text-[13px] leading-relaxed text-[var(--muted)]">
          {t("settings.subtitle")}
        </p>
      </header>

      <Section title={t("settings.section.appearance")}>
        <Row label={t("settings.language")}>
          <Segmented
            label={t("settings.language")}
            value={draft.language}
            onChange={(language) => applyNow({ ...draft, language })}
            options={[
              ["en", "English"],
              ["id", "Indonesia"],
            ] as const}
          />
        </Row>
        <Row label={t("settings.theme")}>
          <Segmented
            label={t("settings.theme")}
            value={draft.theme}
            onChange={(theme) => applyNow({ ...draft, theme })}
            options={[
              ["system", t("settings.theme.system")],
              ["light", t("settings.theme.light")],
              ["dark", t("settings.theme.dark")],
            ] as const}
          />
        </Row>
      </Section>

      <Section title={t("settings.section.packages")}>
        <Row label={t("settings.output")}>
          <div className="flex items-center gap-1.5">
            <code className="max-w-[14rem] truncate rounded-md bg-[var(--surface-2)] px-2 py-1 text-[11px] text-[var(--muted)]">
              {draft.output_dir ?? t("settings.default")}
            </code>
            <Button variant="ghost" size="sm" onClick={pickDir}>
              <IconFolder size={13} />
              {t("settings.choose")}
            </Button>
            {draft.output_dir ? (
              <Button
                variant="subtle"
                size="sm"
                onClick={() => setDraft({ ...draft, output_dir: null })}
              >
                {t("settings.default")}
              </Button>
            ) : null}
          </div>
        </Row>
        <Row label={t("settings.keep")}>
          <Toggle
            checked={draft.keep_packages}
            onChange={(keep_packages) => setDraft({ ...draft, keep_packages })}
            label={t("settings.keep")}
          />
        </Row>
        <Row label={t("settings.aur")}>
          <Toggle
            checked={draft.check_aur}
            onChange={(check_aur) => setDraft({ ...draft, check_aur })}
            label={t("settings.aur")}
          />
        </Row>
        <Row label={t("settings.refreshDb")} hint={t("settings.refreshHint")}>
          <Button variant="ghost" size="sm" onClick={refreshDb} disabled={refreshing}>
            {refreshing ? <Spinner size={12} /> : <IconRefresh size={13} />}
            {refreshing ? t("settings.refreshing") : t("settings.runNow")}
          </Button>
        </Row>
      </Section>

      <Section title={t("settings.section.mapping")}>
        <div className="px-3.5 py-3">
          <textarea
            id="mapping"
            value={draft.custom_mapping}
            onChange={(e) => setDraft({ ...draft, custom_mapping: e.target.value })}
            placeholder={t("settings.mappingHint")}
            spellCheck={false}
            rows={7}
            className="log w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--bg-2)] p-3 text-[var(--fg)] placeholder:text-[var(--faint)] focus:border-[var(--accent)] focus:outline-none"
          />
          <p className="mt-2 text-[11.5px] leading-relaxed text-[var(--muted)]">
            {t("settings.mappingAbout")}
          </p>
        </div>
      </Section>

      <Section title={t("settings.section.about")}>
        <Row label={t("settings.about.version")}>
          <span className="tabular-nums text-[12.5px] text-[var(--muted)]">0.1.0</span>
        </Row>
        <Row label={t("settings.about.stack")}>
          <span className="text-[12.5px] text-[var(--muted)]">Tauri 2 · React 19 · Rust</span>
        </Row>
        <Row label={t("settings.about.license")}>
          <Pill tone="neutral">MIT</Pill>
        </Row>
        <Row label={t("settings.about.repo")}>
          <a
            href="https://github.com/ebudsss/deb-installer-arch"
            target="_blank"
            rel="noreferrer"
            className="text-[12.5px] text-[var(--accent)] hover:underline"
          >
            github.com/ebudsss/deb-installer-arch
          </a>
        </Row>
      </Section>

      <div className="flex items-center justify-between gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface)] px-3.5 py-2.5">
        <span className="flex items-center gap-1.5 text-[11.5px] text-[var(--faint)]">
          <IconShield size={12} />
          {t("settings.localOnly")}
        </span>
        <div className="flex items-center gap-2">
          {dirty ? (
            <span className="flex items-center gap-1.5 text-[11.5px] text-[var(--warning)]">
              <IconGlobe size={12} />
              {t("settings.unsaved")}
            </span>
          ) : null}
          <Button onClick={save} disabled={!dirty} size="sm">
            <IconCheck size={13} />
            {t("settings.save")}
          </Button>
        </div>
      </div>

      <Banner tone="info">{t("settings.privacy")}</Banner>
    </div>
  );
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-10 rounded-full transition-colors duration-150 ${
        checked ? "bg-[var(--accent)]" : "bg-[var(--border-strong)]"
      }`}
    >
      <span
        aria-hidden
        className={`absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-white shadow-sm transition-transform duration-150 ${
          checked ? "translate-x-[1.25rem]" : "translate-x-0"
        }`}
      />
    </button>
  );
}
