import { useEffect, useRef, useState } from "react";
import { X, Mic2, Loader2 } from "lucide-react";
import { useLyrics } from "../hooks/useLyrics";
import { findActiveLineIndex } from "../lib/lyrics";
import { getInterpolatedPosition } from "../lib/playbackPosition";

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function LyricsPanel({ open, onClose }: Props) {
  const { loading, parsed, error, rtl } = useLyrics();
  const [activeIndex, setActiveIndex] = useState<number>(-1);
  const linesRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<(HTMLParagraphElement | null)[]>([]);
  const panelRef = useRef<HTMLDivElement>(null);

  // Drive active-line tracking from rAF while panel is open.
  useEffect(() => {
    if (!open || !parsed?.synced.length) return;
    let raf = 0;
    const tick = () => {
      const t = getInterpolatedPosition();
      const idx = findActiveLineIndex(parsed.synced, t);
      setActiveIndex((prev) => (prev === idx ? prev : idx));
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [open, parsed]);

  // Scroll active line into view (centered).
  useEffect(() => {
    if (activeIndex < 0) return;
    const el = lineRefs.current[activeIndex];
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [activeIndex]);

  // Esc + click-outside close.
  useEffect(() => {
    if (!open) return;
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("keydown", onEsc);
    document.addEventListener("mousedown", onClick);
    return () => {
      window.removeEventListener("keydown", onEsc);
      document.removeEventListener("mousedown", onClick);
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fadeIn">
      <div
        ref={panelRef}
        className="bg-th-elevated rounded-xl shadow-2xl w-[520px] max-h-[80vh] flex flex-col overflow-hidden"
        dir={rtl ? "rtl" : "ltr"}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 pt-5 pb-3 border-b border-th-border-subtle">
          <div className="flex items-center gap-2">
            <Mic2 size={16} className="text-th-text-muted" />
            <h2 className="text-[15px] font-semibold text-th-text-primary">
              Letras
            </h2>
            {parsed?.synced.length ? (
              <span className="text-[10px] uppercase tracking-wider text-th-accent ml-1">
                synced
              </span>
            ) : null}
          </div>
          <button
            onClick={onClose}
            className="w-7 h-7 rounded-full flex items-center justify-center hover:bg-th-inset transition-colors text-th-text-muted hover:text-th-text-primary"
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div
          ref={linesRef}
          className="overflow-y-auto px-6 py-8 flex-1 scroll-smooth"
        >
          {loading ? (
            <div className="flex items-center justify-center text-th-text-muted py-10 gap-2">
              <Loader2 size={14} className="animate-spin" />
              <span className="text-[12px]">Cargando letras…</span>
            </div>
          ) : error ? (
            <p className="text-[12px] text-th-text-muted text-center py-10">
              No hay letras disponibles para esta pista.
            </p>
          ) : parsed?.synced.length ? (
            <div className="space-y-1.5">
              {parsed.synced.map((line, i) => (
                <p
                  key={i}
                  ref={(el) => {
                    lineRefs.current[i] = el;
                  }}
                  className={`text-[18px] leading-snug transition-all duration-150 ${
                    i === activeIndex
                      ? "text-th-accent font-semibold scale-[1.02]"
                      : i < activeIndex
                      ? "text-th-text-muted"
                      : "text-th-text-secondary"
                  } ${line.text ? "" : "h-[1em]"}`}
                >
                  {line.text || " "}
                </p>
              ))}
            </div>
          ) : parsed?.plain ? (
            <pre className="whitespace-pre-wrap text-[14px] leading-relaxed text-th-text-secondary font-sans">
              {parsed.plain}
            </pre>
          ) : (
            <p className="text-[12px] text-th-text-muted text-center py-10">
              Sin letras.
            </p>
          )}
        </div>

        {parsed?.synced.length ? (
          <div className="px-5 py-2 border-t border-th-border-subtle text-[10px] text-th-text-muted text-center">
            Las letras se sincronizan con el playback automáticamente.
          </div>
        ) : null}
      </div>
    </div>
  );
}
