import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Analysis, type InstallRecord } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { Banner, Button, Card, Spinner } from "../components/ui";

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
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // analyzePath stabil untuk tujuan drag-drop.
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
    .slice(0, 5);

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">{t("home.title")}</h1>
        <p className="mt-1 text-[var(--muted)]">{t("home.subtitle")}</p>
      </div>

      <div
        className={`flex flex-col items-center justify-center gap-4 rounded-2xl border-2 border-dashed px-6 py-14 text-center transition-colors ${
          dragging
            ? "border-[var(--accent)] bg-[var(--surface-2)]"
            : "border-[var(--border)] bg-[var(--surface)]"
        }`}
        aria-busy={busy}
      >
        {busy ? (
          <>
            <Spinner />
            <p className="text-[var(--muted)]">{t("home.analyzing")}</p>
          </>
        ) : (
          <>
            <svg width="44" height="44" viewBox="0 0 24 24" fill="none" aria-hidden>
              <path
                d="M12 16V4m0 0L7.5 8.5M12 4l4.5 4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3"
                stroke="var(--muted)"
                strokeWidth="1.6"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            <p className="text-base font-medium">{t("home.drop")}</p>
            <Button onClick={chooseFile}>{t("home.choose")}</Button>
            <p className="max-w-md text-xs leading-relaxed text-[var(--muted)]">{t("home.hint")}</p>
          </>
        )}
      </div>

      {recentDebs.length > 0 ? (
        <section>
          <h2 className="mb-2 text-sm font-semibold text-[var(--muted)]">{t("home.recent")}</h2>
          <Card>
            <ul className="divide-y divide-[var(--border)]">
              {recentDebs.map((record) => (
                <li key={record.id}>
                  <button
                    onClick={() => analyzePath(record.deb_path)}
                    className="flex w-full items-center justify-between gap-4 px-4 py-3 text-left hover:bg-[var(--surface-2)]"
                  >
                    <span className="min-w-0">
                      <span className="block truncate font-medium">{record.deb_name}</span>
                      <span className="block truncate text-xs text-[var(--muted)]">
                        {record.deb_path}
                      </span>
                    </span>
                    <span className="shrink-0 text-xs text-[var(--muted)]">{record.deb_version}</span>
                  </button>
                </li>
              ))}
            </ul>
          </Card>
        </section>
      ) : null}

      <Banner tone="info">{t("home.about")}</Banner>
    </div>
  );
}
