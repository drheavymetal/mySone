import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import {
  Sparkles,
  X,
  Send,
  Loader2,
  ListPlus,
  ListRestart,
  Settings as SettingsIcon,
} from "lucide-react";
import type { Track, SearchResults } from "../types";
import { queueAtom, manualQueueAtom } from "../atoms/playback";

const SYSTEM_PROMPT = `Eres un curador musical para SONE, un cliente nativo de TIDAL.
Cuando el usuario describa la música que quiere, devuelve un objeto JSON con el campo "items":
un array de objetos { "artist": string, "track": string } representando la cola sugerida.

Reglas estrictas:
- Devuelve SOLO el JSON, sin comentarios ni texto extra.
- Cada artist/track debe corresponder a una canción real publicada en TIDAL.
- Tamaño orientativo: ~1 pista por cada 3-4 minutos solicitados; si la duración es ambigua, devuelve 15-25 pistas.
- Respeta exclusiones explícitas del usuario (artistas, géneros).
- Si el usuario pide cambios sobre la cola previa, ajusta sólo lo que indique y devuelve la cola completa nueva.`;

interface ChatMsg {
  role: "user" | "assistant";
  content: string;
}

interface SuggestedItem {
  artist: string;
  track: string;
}

interface ResolvedItem {
  suggestion: SuggestedItem;
  resolved: Track | null;
}

interface Props {
  open: boolean;
  onClose: () => void;
  onOpenSettings?: () => void;
}

export default function QueueChatPanel({
  open,
  onClose,
  onOpenSettings,
}: Props) {
  const [messages, setMessages] = useState<ChatMsg[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resolvedItems, setResolvedItems] = useState<ResolvedItem[] | null>(
    null,
  );
  const setQueue = useSetAtom(queueAtom);
  const setManualQueue = useSetAtom(manualQueueAtom);
  const panelRef = useRef<HTMLDivElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Esc + click-outside close
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

  // Autoscroll messages
  useEffect(() => {
    if (messagesRef.current) {
      messagesRef.current.scrollTop = messagesRef.current.scrollHeight;
    }
  }, [messages, resolvedItems, loading]);

  // Focus input on open
  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const handleSend = async () => {
    const text = input.trim();
    if (!text || loading) return;
    setInput("");
    setError(null);
    const newMessages: ChatMsg[] = [
      ...messages,
      { role: "user", content: text },
    ];
    setMessages(newMessages);
    setLoading(true);
    setResolvedItems(null);

    try {
      const raw = await invoke<string>("llm_chat", {
        system: SYSTEM_PROMPT,
        messages: newMessages.map((m) => ({
          role: m.role,
          content: m.content,
        })),
      });
      let items: SuggestedItem[] = [];
      try {
        const parsed = JSON.parse(raw);
        items = Array.isArray(parsed.items)
          ? parsed.items.filter(
              (it: unknown): it is SuggestedItem =>
                typeof it === "object" &&
                it !== null &&
                typeof (it as SuggestedItem).artist === "string" &&
                typeof (it as SuggestedItem).track === "string",
            )
          : [];
      } catch {
        // LLM returned non-JSON despite the directive — fall through
      }

      setMessages([
        ...newMessages,
        {
          role: "assistant",
          content:
            items.length > 0
              ? `Sugiero ${items.length} pistas. Resolviendo en TIDAL…`
              : "No entendí la respuesta del modelo.",
        },
      ]);

      if (items.length > 0) {
        const resolved = await Promise.all(
          items.map(async (it) => {
            const query = `${it.artist} ${it.track}`;
            try {
              const res = await invoke<SearchResults>("search_tidal", {
                query,
                limit: 3,
              });
              const top = res.tracks?.[0] ?? null;
              return { suggestion: it, resolved: top } satisfies ResolvedItem;
            } catch {
              return { suggestion: it, resolved: null } satisfies ResolvedItem;
            }
          }),
        );
        setResolvedItems(resolved);
      }
    } catch (e) {
      setError(typeof e === "string" ? e : ((e as { message?: string })?.message ?? "Error"));
    } finally {
      setLoading(false);
    }
  };

  const onInputKey = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const replaceQueue = () => {
    if (!resolvedItems) return;
    const tracks = resolvedItems
      .map((r) => r.resolved)
      .filter((t): t is Track => t !== null);
    if (tracks.length === 0) return;
    setQueue(tracks);
    setManualQueue([]);
    onClose();
  };

  const appendQueue = () => {
    if (!resolvedItems) return;
    const tracks = resolvedItems
      .map((r) => r.resolved)
      .filter((t): t is Track => t !== null);
    if (tracks.length === 0) return;
    setManualQueue((q) => [...q, ...tracks]);
    onClose();
  };

  if (!open) return null;

  const matchedCount =
    resolvedItems?.filter((r) => r.resolved !== null).length ?? 0;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fadeIn">
      <div
        ref={panelRef}
        className="bg-th-elevated rounded-xl shadow-2xl w-[560px] max-h-[80vh] flex flex-col overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 pt-5 pb-3 border-b border-th-border-subtle">
          <div className="flex items-center gap-2">
            <Sparkles size={16} className="text-th-accent" />
            <h2 className="text-[15px] font-semibold text-th-text-primary">
              Construir cola con IA
            </h2>
          </div>
          <div className="flex items-center gap-1">
            {onOpenSettings && (
              <button
                onClick={() => {
                  onClose();
                  onOpenSettings();
                }}
                title="Configurar backend de IA"
                className="w-7 h-7 rounded-full flex items-center justify-center hover:bg-th-inset transition-colors text-th-text-muted hover:text-th-text-primary"
              >
                <SettingsIcon size={14} />
              </button>
            )}
            <button
              onClick={onClose}
              className="w-7 h-7 rounded-full flex items-center justify-center hover:bg-th-inset transition-colors text-th-text-muted hover:text-th-text-primary"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Messages */}
        <div ref={messagesRef} className="flex-1 overflow-y-auto px-5 py-4 space-y-3">
          {messages.length === 0 && !loading && (
            <div className="text-center py-8 text-th-text-muted">
              <p className="text-[13px] mb-2">
                Describe la música que quieres y te armo la cola.
              </p>
              <p className="text-[11px] text-th-text-faint">
                Ejemplos: "música para cocinar 1h de jazz fusion", "set chill
                de bossa, sin Gilberto", "energía running 45 min", "rock
                progresivo de los 70 que no sea Pink Floyd ni Yes"
              </p>
            </div>
          )}

          {messages.map((m, i) => (
            <div
              key={i}
              className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}
            >
              <div
                className={`max-w-[80%] px-3 py-2 rounded-lg text-[13px] ${
                  m.role === "user"
                    ? "bg-th-accent text-th-bg"
                    : "bg-th-bg-secondary text-th-text-secondary"
                }`}
              >
                {m.content}
              </div>
            </div>
          ))}

          {loading && (
            <div className="flex items-center gap-2 text-th-text-muted">
              <Loader2 size={14} className="animate-spin" />
              <span className="text-[12px]">Pensando…</span>
            </div>
          )}

          {resolvedItems && resolvedItems.length > 0 && (
            <div className="mt-2 border border-th-border-subtle rounded-md overflow-hidden">
              <div className="px-3 py-2 bg-th-bg-secondary text-[11px] text-th-text-muted">
                {matchedCount} de {resolvedItems.length} pistas encontradas en
                TIDAL
              </div>
              <div className="divide-y divide-th-border-subtle max-h-[280px] overflow-y-auto">
                {resolvedItems.map((r, i) => (
                  <div
                    key={i}
                    className="flex items-center justify-between px-3 py-1.5 text-[12px]"
                  >
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-th-text-primary">
                        {r.resolved?.title ?? r.suggestion.track}
                      </p>
                      <p className="truncate text-[10px] text-th-text-muted">
                        {r.resolved?.artist?.name ?? r.suggestion.artist}
                      </p>
                    </div>
                    {r.resolved ? (
                      <span className="text-[10px] text-th-accent">✓</span>
                    ) : (
                      <span className="text-[10px] text-red-400">no match</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {error && <p className="text-[12px] text-red-400">{error}</p>}
        </div>

        {/* Action bar (when results) */}
        {resolvedItems && matchedCount > 0 && (
          <div className="flex items-center justify-end gap-2 px-5 py-2 border-t border-th-border-subtle">
            <button
              onClick={appendQueue}
              className="px-3 py-1.5 text-[12px] flex items-center gap-1.5 border border-th-border-subtle rounded-md text-th-text-secondary hover:text-th-text-primary hover:border-th-accent/50 transition-colors"
            >
              <ListPlus size={13} /> Añadir
            </button>
            <button
              onClick={replaceQueue}
              className="px-3 py-1.5 text-[12px] flex items-center gap-1.5 bg-th-accent text-th-bg rounded-md hover:opacity-90 transition-opacity"
            >
              <ListRestart size={13} /> Reemplazar cola
            </button>
          </div>
        )}

        {/* Input */}
        <div className="border-t border-th-border-subtle p-3">
          <div className="flex items-end gap-2">
            <textarea
              ref={inputRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={onInputKey}
              placeholder="Describe lo que quieres oír…"
              rows={2}
              className="flex-1 px-3 py-2 bg-th-bg-secondary border border-th-border-subtle rounded-md text-[13px] text-th-text-primary placeholder:text-th-text-muted resize-none outline-none focus:border-th-accent/50 transition-colors"
            />
            <button
              onClick={handleSend}
              disabled={loading || !input.trim()}
              className="px-3 py-2 bg-th-accent text-th-bg rounded-md hover:opacity-90 disabled:opacity-40 transition-opacity"
            >
              <Send size={14} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
