import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Share2, Copy, X, Check, Users } from "lucide-react";

type ShareStatus = {
  active: boolean;
  token: string | null;
  url: string | null;
  listener_count: number;
};

export default function ShareLinkButton() {
  const [status, setStatus] = useState<ShareStatus | null>(null);
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<ShareStatus>("share_status");
      setStatus(s);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    refresh();
    const t = window.setInterval(refresh, 2500);
    return () => window.clearInterval(t);
  }, [refresh]);

  // Close on click outside / Escape
  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onClick);
    window.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onClick);
      window.removeEventListener("keydown", onEsc);
    };
  }, [open]);

  const handleClick = async () => {
    setError(null);
    if (status?.active) {
      // already sharing → just open the panel
      setOpen(true);
      return;
    }
    setBusy(true);
    try {
      const s = await invoke<ShareStatus>("share_start");
      setStatus(s);
      setOpen(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleStop = async () => {
    setError(null);
    setBusy(true);
    try {
      const s = await invoke<ShareStatus>("share_stop");
      setStatus(s);
      setOpen(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async () => {
    if (!status?.url) return;
    try {
      await navigator.clipboard.writeText(status.url);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };

  const active = status?.active ?? false;

  return (
    <div className="relative">
      <button
        onClick={handleClick}
        disabled={busy}
        title={active ? "Sharing live (click to manage)" : "Share live audio link"}
        className={`p-1.5 rounded-md transition-colors ${
          active
            ? "text-th-accent hover:bg-th-inset"
            : "text-th-text-muted hover:text-th-text-primary hover:bg-th-inset"
        } ${busy ? "opacity-50" : ""}`}
      >
        <Share2 size={18} />
      </button>

      {active && (
        <span
          className="absolute top-0 right-0 w-2 h-2 rounded-full bg-th-accent pointer-events-none"
          aria-hidden
        />
      )}

      {open && active && status?.url && (
        <div
          ref={panelRef}
          className="absolute bottom-full right-0 mb-2 w-[360px] bg-th-elevated rounded-xl shadow-2xl border border-th-border-subtle overflow-hidden z-50"
        >
          <div className="flex items-center justify-between px-4 pt-4 pb-2">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-th-accent animate-pulse" />
              <h3 className="text-[13px] font-semibold text-th-text-primary">
                Sharing live
              </h3>
            </div>
            <button
              onClick={() => setOpen(false)}
              className="text-th-text-muted hover:text-th-text-primary"
            >
              <X size={16} />
            </button>
          </div>

          <div className="px-4 pb-3">
            <p className="text-[11px] text-th-text-muted mb-2">
              Comparte este enlace. Cualquiera con él lo abre en el navegador y
              escucha lo que reproduces.
            </p>

            <div className="flex items-center gap-2 p-2 bg-th-bg-secondary rounded-md border border-th-border-subtle">
              <input
                readOnly
                value={status.url}
                className="flex-1 bg-transparent text-[12px] text-th-text-primary outline-none truncate"
                onFocus={(e) => e.target.select()}
              />
              <button
                onClick={handleCopy}
                className="px-2 py-1 text-[11px] flex items-center gap-1 text-th-text-secondary hover:text-th-text-primary transition-colors shrink-0"
              >
                {copied ? (
                  <>
                    <Check size={12} /> Copiado
                  </>
                ) : (
                  <>
                    <Copy size={12} /> Copiar
                  </>
                )}
              </button>
            </div>

            <div className="flex items-center justify-between mt-3">
              <div className="flex items-center gap-1.5 text-[11px] text-th-text-muted">
                <Users size={12} />
                <span>
                  {status.listener_count}{" "}
                  {status.listener_count === 1 ? "oyente" : "oyentes"}
                </span>
              </div>
              <button
                onClick={handleStop}
                disabled={busy}
                className="px-3 py-1 text-[11px] border border-th-border-subtle rounded-md text-th-text-secondary hover:text-red-400 hover:border-red-400/40 transition-colors disabled:opacity-50"
              >
                Stop sharing
              </button>
            </div>

            {error && (
              <p className="mt-2 text-[11px] text-red-400">{error}</p>
            )}

            <p className="mt-3 text-[10px] text-th-text-muted leading-relaxed">
              Re-codificado en Opus 256 kbps (no bit-perfect en el navegador del
              oyente). Solo en contexto personal.
            </p>
          </div>
        </div>
      )}

      {error && !open && (
        <p className="absolute top-full right-0 mt-1 text-[10px] text-red-400 whitespace-nowrap">
          {error}
        </p>
      )}
    </div>
  );
}
