import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { IconAlert, IconCheck, IconInfo, IconX } from "./icons";

export type ToastTone = "success" | "error" | "info";

interface ToastItem {
  id: number;
  tone: ToastTone;
  message: string;
}

export interface ToastApi {
  success: (message: string) => void;
  error: (message: string) => void;
  info: (message: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

const TOAST_TONES: Record<ToastTone, { color: string; icon: ReactNode }> = {
  success: { color: "var(--success)", icon: <IconCheck size={14} /> },
  error: { color: "var(--error)", icon: <IconAlert size={14} /> },
  info: { color: "var(--accent)", icon: <IconInfo size={14} /> },
};

/** Notifikasi ringkas di pojok kanan bawah, hilang sendiri. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const nextId = useRef(0);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());

  const dismiss = useCallback((id: number) => {
    setItems((prev) => prev.filter((item) => item.id !== id));
    const timer = timers.current.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.current.delete(id);
    }
  }, []);

  const push = useCallback(
    (tone: ToastTone, message: string) => {
      const id = nextId.current;
      nextId.current += 1;
      // Maksimal tiga notifikasi sekaligus agar tidak menutupi UI.
      setItems((prev) => [...prev.slice(-2), { id, tone, message }]);
      const timeout = window.setTimeout(() => dismiss(id), tone === "error" ? 7000 : 3500);
      timers.current.set(id, timeout);
    },
    [dismiss],
  );

  useEffect(() => {
    const pending = timers.current;
    return () => {
      pending.forEach((timer) => clearTimeout(timer));
      pending.clear();
    };
  }, []);

  const api = useMemo<ToastApi>(
    () => ({
      success: (message) => push("success", message),
      error: (message) => push("error", message),
      info: (message) => push("info", message),
    }),
    [push],
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div
        className="pointer-events-none fixed bottom-4 right-4 z-50 flex w-[min(23rem,calc(100vw-2rem))] flex-col gap-2"
        aria-live="polite"
      >
        {items.map((item) => {
          const tone = TOAST_TONES[item.tone];
          return (
            <div
              key={item.id}
              role="status"
              className="animate-rise pointer-events-auto flex items-start gap-2.5 rounded-lg border border-[var(--border-strong)] bg-[var(--surface-2)] px-3 py-2.5 shadow-[var(--shadow-md)]"
            >
              <span className="mt-0.5 shrink-0" style={{ color: tone.color }}>
                {tone.icon}
              </span>
              <p className="min-w-0 flex-1 text-[12.5px] leading-relaxed text-[var(--fg)]">
                {item.message}
              </p>
              <button
                onClick={() => dismiss(item.id)}
                aria-label="Tutup notifikasi"
                className="-mr-1 shrink-0 rounded p-1 text-[var(--faint)] transition-colors hover:text-[var(--fg)]"
              >
                <IconX size={12} />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) throw new Error("useToast harus dipakai di dalam ToastProvider");
  return api;
}
