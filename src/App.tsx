import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { api, type Analysis, type Settings } from "./lib/api";
import { I18nContext, translate } from "./lib/i18n";
import { Banner, Button } from "./components/ui";
import { Home } from "./pages/Home";
import { Review } from "./pages/Review";
import { Progress } from "./pages/Progress";
import { Installed } from "./pages/Installed";
import { SettingsPage } from "./pages/Settings";

type Screen =
  | { kind: "home" }
  | { kind: "review"; analysis: Analysis }
  | { kind: "progress"; analysis: Analysis }
  | { kind: "installed" }
  | { kind: "settings" };

const DEFAULT_SETTINGS: Settings = {
  language: "id",
  theme: "system",
  output_dir: null,
  keep_packages: true,
  check_aur: true,
  custom_mapping: "",
};

export default function App() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [ready, setReady] = useState(false);
  const [screen, setScreen] = useState<Screen>({ kind: "home" });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then((loaded) => setSettings({ ...DEFAULT_SETTINGS, ...loaded }))
      .catch(() => undefined)
      .finally(() => setReady(true));
  }, []);

  // Asosiasi berkas / `deb-installer foo.deb` langsung membuka tinjauan.
  useEffect(() => {
    if (!ready) return;
    invoke<string | null>("initial_file")
      .then(async (path) => {
        if (!path) return;
        const analysis = await api.analyze(path);
        setScreen({ kind: "review", analysis });
      })
      .catch((e) => setError(String(e)));
  }, [ready]);

  useEffect(() => {
    const root = document.documentElement;
    if (settings.theme === "system") {
      delete root.dataset.theme;
    } else {
      root.dataset.theme = settings.theme;
    }
  }, [settings.theme]);

  const i18n = useMemo(
    () => ({
      lang: settings.language,
      t: (key: Parameters<typeof translate>[1]) => translate(settings.language, key),
    }),
    [settings.language],
  );

  const navItems = [
    { id: "home" as const, label: i18n.t("nav.home") },
    { id: "installed" as const, label: i18n.t("nav.installed") },
    { id: "settings" as const, label: i18n.t("nav.settings") },
  ];

  const activeNav =
    screen.kind === "review" || screen.kind === "progress" ? "home" : screen.kind;

  return (
    <I18nContext.Provider value={i18n}>
      <div className="flex h-full flex-col">
        <header className="flex items-center justify-between border-b border-[var(--border)] bg-[var(--surface)] px-6 py-3">
          <div className="flex items-center gap-2">
            <span className="grid h-7 w-7 place-items-center rounded-lg bg-[var(--accent)] text-sm font-bold text-[var(--accent-fg)]">
              D
            </span>
            <span className="font-semibold tracking-tight">{i18n.t("app.name")}</span>
          </div>
          <nav className="flex gap-1" aria-label="Navigasi utama">
            {navItems.map((item) => (
              <button
                key={item.id}
                onClick={() => {
                  setError(null);
                  setScreen({ kind: item.id } as Screen);
                }}
                aria-current={activeNav === item.id ? "page" : undefined}
                className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
                  activeNav === item.id
                    ? "bg-[var(--surface-2)] text-[var(--fg)]"
                    : "text-[var(--muted)] hover:text-[var(--fg)]"
                }`}
              >
                {item.label}
              </button>
            ))}
          </nav>
        </header>

        <main className="flex-1 overflow-auto px-6 py-6">
          {!ready ? (
            <p className="text-[var(--muted)]">…</p>
          ) : (
            <>
              {error ? (
                <div className="mx-auto mb-4 flex w-full max-w-5xl items-start gap-3">
                  <div className="flex-1">
                    <Banner tone="error" title={i18n.t("common.error")}>
                      <span className="break-words">{error}</span>
                    </Banner>
                  </div>
                  <Button variant="ghost" onClick={() => setError(null)}>
                    {i18n.t("common.close")}
                  </Button>
                </div>
              ) : null}

              {screen.kind === "home" ? (
                <Home
                  onAnalyzed={(analysis) => {
                    setError(null);
                    setScreen({ kind: "review", analysis });
                  }}
                  onError={setError}
                />
              ) : null}

              {screen.kind === "review" ? (
                <Review
                  analysis={screen.analysis}
                  onBack={() => setScreen({ kind: "home" })}
                  onInstall={() => setScreen({ kind: "progress", analysis: screen.analysis })}
                />
              ) : null}

              {screen.kind === "progress" ? (
                <Progress
                  analysis={screen.analysis}
                  settings={settings}
                  onDone={() => setScreen({ kind: "home" })}
                  onRetry={() => setScreen({ kind: "review", analysis: screen.analysis })}
                />
              ) : null}

              {screen.kind === "installed" ? <Installed onError={setError} /> : null}

              {screen.kind === "settings" ? (
                <SettingsPage settings={settings} onChange={setSettings} onError={setError} />
              ) : null}
            </>
          )}
        </main>
      </div>
    </I18nContext.Provider>
  );
}
