import { useEffect, useState, type ReactNode } from "react";
import brandSymbol from "../assets/brand-symbol.png";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Analysis, type InstallRecord } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Button, SectionHeader, Spinner } from "../components/ui";
import {
  IconArrowLeft,
  IconBox,
  IconClock,
  IconFile,
  IconFolder,
  IconLayers,
  IconPackage,
  IconShield,
  IconTerminal,
  IconUpload,
} from "../components/icons";

const MAX_RECENT = 4;

export function Home({
  onAnalyzed,
  onError,
}: {
  onAnalyzed: (analysis: Analysis) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [recent, setRecent] = useState<InstallRecord[]>([]);

  useEffect(() => {
    api.listInstalled().then(setRecent).catch(() => setRecent([]));
  }, []);

  async function analyzePath(path: string) {
    if (!path.toLowerCase().endsWith(".deb")) {
      onError(t("home.onlyDeb"));
      return;
    }
    setBusy(true);
    try {
      const analysis = await api.analyze(path);
      onAnalyzed(analysis);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          setDragging(true);
        } else if (event.payload.type === "drop") {
          setDragging(false);
          const path = event.payload.paths.find((p) => p.toLowerCase().endsWith(".deb"));
          if (path) {
            void analyzePath(path);
          } else if (event.payload.paths.length > 0) {
            onError(t("home.onlyDeb"));
          }
        } else {
          setDragging(false);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // Handler drag-drop hanya perlu dipasang sekali; `t` dan `onError`
    // stabil untuk tujuan ini.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function chooseFile() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Debian package", extensions: ["deb"] }],
    });
    if (typeof selected === "string") {
      await analyzePath(selected);
    }
  }

  const recentDebs = recent
    .filter((record) => record.deb_path.endsWith(".deb"))
    .slice(0, MAX_RECENT);

  return (
    <div className="mx-auto flex w-full max-w-[52rem] flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-[22px] font-semibold leading-tight tracking-tight">{t("home.title")}</h1>
          <p className="mt-1 max-w-lg text-[13px] leading-relaxed text-[var(--muted)]">
            {t("home.subtitle")}
          </p>
        </div>
        <span className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-2.5 py-1.5 text-[11.5px] text-[var(--muted)]">
          <IconTerminal size={12} className="text-[var(--accent)]" />
          {t("home.archBadge")}
        </span>
      </header>

      {/* Area unggah utama */}
      <div
        onDragEnter={() => setDragging(true)}
        onDragLeave={() => setDragging(false)}
        className={`group relative flex flex-col items-center justify-center rounded-xl border-2 border-dashed px-6 py-7 text-center transition-[border-color,background-color,transform] duration-150 ${
          dragging
            ? "border-[var(--accent)] bg-[var(--accent-soft)]"
            : "border-[var(--border)] bg-[var(--surface)] hover:border-[var(--border-strong)]"
        }`}
        aria-busy={busy}
      >
        {busy ? (
          <>
            <div className="relative grid h-20 w-20 place-items-center">
              <img
                src={brandSymbol}
                alt=""
                className="h-full w-full object-contain opacity-30"
              />
              <span className="absolute inset-0 grid place-items-center">
                <Spinner size={22} />
              </span>
            </div>
            <p className="mt-4 text-[13.5px] font-medium">{t("home.analyzing")}</p>
          </>
        ) : (
          <>
            <img
              src={brandSymbol}
              alt=""
              className={`h-20 w-20 object-contain transition-transform duration-200 ${
                dragging ? "scale-105" : "group-hover:scale-[1.03]"
              }`}
            />
            <p className="mt-3 text-[14px] font-semibold">
              {dragging ? t("home.dropActive") : t("home.drop")}
            </p>
            <Button onClick={chooseFile} className="mt-3">
              <IconUpload size={15} />
              {t("home.choose")}
            </Button>
            <p className="mt-2.5 flex items-center gap-1.5 text-[11.5px] text-[var(--faint)]">
              <IconFile size={12} />
              <span>
                {t("home.formats")} <code className="text-[var(--muted)]">.deb</code>
              </span>
            </p>
          </>
        )}
      </div>

      {/* Berkas terakhir */}
      {recentDebs.length > 0 ? (
        <section>
          <SectionHeader
            title={t("home.recent")}
            action={
              <span className="text-[11.5px] text-[var(--faint)]">{t("home.recentHint")}</span>
            }
          />
          <div className="overflow-hidden rounded-xl border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-sm)]">
            <ul className="divide-y divide-[var(--border)]">
              {recentDebs.map((record) => (
                <li key={record.id}>
                  <button
                    onClick={() => void analyzePath(record.deb_path)}
                    className="group flex w-full items-center gap-3 px-3.5 py-2.5 text-left transition-colors hover:bg-[var(--surface-2)]"
                  >
                    <span className="grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-[var(--surface-2)] text-[var(--accent)]">
                      <IconPackage size={15} />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-baseline gap-2">
                        <span className="truncate text-[13px] font-medium">{record.deb_name}</span>
                        <span className="shrink-0 text-[11.5px] tabular-nums text-[var(--muted)]">
                          {record.deb_version}
                        </span>
                      </span>
                      <span className="mt-0.5 flex items-center gap-1 truncate text-[11px] text-[var(--faint)]">
                        <IconFolder size={10} className="shrink-0" />
                        <span className="truncate" title={record.deb_path}>
                          {record.deb_path}
                        </span>
                      </span>
                    </span>
                    <IconArrowLeft
                      size={13}
                      className="shrink-0 rotate-180 text-[var(--faint)] opacity-0 transition-opacity group-hover:opacity-100"
                    />
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </section>
      ) : null}

      {/* Informasi ringkas */}
      <section>
        <SectionHeader title={t("home.about.title")} />
        <div className="grid gap-2.5 sm:grid-cols-2">
          <InfoTile
            icon={<IconLayers size={14} />}
            title={t("home.about.format")}
            body={t("home.about.formatBody")}
          />
          <InfoTile
            icon={<IconBox size={14} />}
            title={t("home.about.convert")}
            body={t("home.about.convertBody")}
          />
          <InfoTile
            icon={<IconClock size={14} />}
            title={t("home.about.deps")}
            body={t("home.about.depsBody")}
          />
          <InfoTile
            icon={<IconShield size={14} />}
            title={t("home.about.native")}
            body={t("home.about.nativeBody")}
          />
        </div>
      </section>
    </div>
  );
}

function InfoTile({
  icon,
  title,
  body,
}: {
  icon: ReactNode;
  title: string;
  body: string;
}) {
  return (
    <div className="flex gap-2.5 rounded-xl border border-[var(--border)] bg-[var(--surface)] px-3.5 py-3">
      <span className="mt-px grid h-6 w-6 shrink-0 place-items-center rounded-md bg-[var(--accent-soft)] text-[var(--accent)]">
        {icon}
      </span>
      <div className="min-w-0">
        <p className="text-[12.5px] font-medium">{title}</p>
        <p className="mt-0.5 text-[11.5px] leading-relaxed text-[var(--muted)]">{body}</p>
      </div>
    </div>
  );
}
