import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { api, type Analysis, type Settings } from "./lib/api";
import { I18nContext, translate, type StringKey } from "./lib/i18n";
import { ToastProvider, useToast } from "./components/toast";
import { IconBox, IconHome, IconSettings, IconShield } from "./components/icons";
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

type NavId = "home" | "installed" | "settings";

const DEFAULT_SETTINGS: Settings = {
  language: "en",
  theme: "system",
  output_dir: null,
  keep_packages: true,
  check_aur: true,
  custom_mapping: "",
};

const NAV: { id: NavId; label: StringKey; icon: typeof IconHome }[] = [
  { id: "home", label: "nav.home", icon: IconHome },
  { id: "installed", label: "nav.installed", icon: IconBox },
  { id: "settings", label: "nav.settings", icon: IconSettings },
];

/** Mengikuti perubahan tema sistem selama pengguna memilih "system". */
function useResolvedTheme(theme: Settings["theme"]): "dark" | "light" {
  const [systemTheme, setSystemTheme] = useState<"dark" | "light">(() =>
    window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark",
  );

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: light)");
    const onChange = (event: MediaQueryListEvent) => {
      setSystemTheme(event.matches ? "light" : "dark");
    };
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  return theme === "system" ? systemTheme : theme;
}

function Shell() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [ready, setReady] = useState(false);
  const [screen, setScreen] = useState<Screen>({ kind: "home" });
  const toast = useToast();

  const reportError = useCallback((message: string) => toast.error(message), [toast]);

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
      .catch((e) => reportError(String(e)));
  }, [ready, reportError]);

  const resolvedTheme = useResolvedTheme(settings.theme);
  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
  }, [resolvedTheme]);

  const i18n = useMemo(
    () => ({
      lang: settings.language,
      t: (key: StringKey) => translate(settings.language, key),
    }),
    [settings.language],
  );

  const activeNav: NavId =
    screen.kind === "review" || screen.kind === "progress" ? "home" : screen.kind;

  function navigate(id: NavId) {
    setScreen({ kind: id });
  }

  return (
    <I18nContext.Provider value={i18n}>
      <div className="flex h-full flex-col bg-[var(--bg)]">
        <header className="sticky top-0 z-30 flex h-[52px] shrink-0 items-center justify-between gap-4 border-b border-[var(--border)] bg-[var(--bg-2)] px-4">
          <div className="flex min-w-0 items-center gap-2.5">
            <span className="grid h-7 w-7 shrink-0 place-items-center rounded-lg bg-[var(--accent)] text-[13px] font-bold text-[var(--accent-fg)] shadow-[var(--shadow-sm)]">
              D
            </span>
            <div className="min-w-0">
              <div className="truncate text-[13.5px] font-semibold leading-tight tracking-tight">
                {i18n.t("app.name")}
              </div>
              <div className="hidden truncate text-[10.5px] leading-tight text-[var(--faint)] sm:block">
                Arch Linux
              </div>
            </div>
          </div>

          <nav className="flex items-center gap-0.5" aria-label="Navigasi utama">
            {NAV.map((item) => {
              const active = activeNav === item.id;
              const Icon = item.icon;
              return (
                <button
                  key={item.id}
                  onClick={() => navigate(item.id)}
                  aria-current={active ? "page" : undefined}
                  className={`relative flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-[13px] font-medium transition-colors duration-150 ${
                    active
                      ? "bg-[var(--surface-2)] text-[var(--fg)]"
                      : "text-[var(--muted)] hover:bg-[var(--surface)] hover:text-[var(--fg)]"
                  }`}
                >
                  <Icon size={15} className={active ? "text-[var(--accent)]" : undefined} />
                  <span className="hidden sm:inline">{i18n.t(item.label)}</span>
                  {active ? (
                    <span
                      aria-hidden
                      className="absolute inset-x-2.5 -bottom-px h-px bg-[var(--accent)]"
                    />
                  ) : null}
                </button>
              );
            })}
          </nav>
        </header>

        <main className="min-h-0 flex-1 overflow-y-auto">
          {!ready ? (
            <div className="mx-auto flex w-full max-w-5xl items-center gap-2 px-6 py-10 text-[13px] text-[var(--muted)]">
              {i18n.t("common.loading")}
            </div>
          ) : (
            <div key={screen.kind} className="animate-fade px-5 py-6">
              {screen.kind === "home" ? (
                <Home
                  onAnalyzed={(analysis) => setScreen({ kind: "review", analysis })}
                  onError={reportError}
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

              {screen.kind === "installed" ? (
                <Installed onError={reportError} onGoHome={() => setScreen({ kind: "home" })} />
              ) : null}

              {screen.kind === "settings" ? (
                <SettingsPage
                  settings={settings}
                  onChange={setSettings}
                  onError={reportError}
                />
              ) : null}
            </div>
          )}
        </main>

        <footer className="flex h-7 shrink-0 items-center justify-between border-t border-[var(--border)] bg-[var(--bg-2)] px-4 text-[10.5px] text-[var(--faint)]">
          <span className="flex items-center gap-1.5">
            <IconShield size={11} />
            {i18n.t("footer.note")}
          </span>
          <span className="tabular-nums">v0.1.0</span>
        </footer>
      </div>
    </I18nContext.Provider>
  );
}

export default function App() {
  return (
    <ToastProvider>
      <Shell />
    </ToastProvider>
  );
}
