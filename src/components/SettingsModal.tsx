import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAtom, useStore } from "jotai";
import {
  X,
  Infinity as InfinityIcon,
  Volume2,
  MessageSquare,
  AppWindow,
  MonitorDown,
  Globe,
  RefreshCw,
  ShieldAlert,
  Sparkles,
  CheckCircle2,
  XCircle,
  Loader2,
} from "lucide-react";
import {
  autoplayAtom,
  bitPerfectAtom,
  allowExplicitAtom,
  currentTrackAtom,
  isPlayingAtom,
  queueAtom,
  manualQueueAtom,
  originalQueueAtom,
  historyAtom,
  playbackSourceAtom,
  contextSourceAtom,
} from "../atoms/playback";

type LLMProvider = "off" | "deepseek" | "ollama";
type LLMSettings = {
  provider: LLMProvider;
  deepseekApiKey: string;
  deepseekModel: string;
  ollamaBaseUrl: string;
  ollamaModel: string;
};

const LLM_DEFAULT: LLMSettings = {
  provider: "off",
  deepseekApiKey: "",
  deepseekModel: "deepseek-chat",
  ollamaBaseUrl: "http://localhost:11434",
  ollamaModel: "llama3.1:8b",
};

function AIBackendSection() {
  const [settings, setSettings] = useState<LLMSettings>(LLM_DEFAULT);
  const [loaded, setLoaded] = useState(false);
  const [testStatus, setTestStatus] = useState<
    "idle" | "testing" | "ok" | "fail"
  >("idle");
  const [testMessage, setTestMessage] = useState("");
  const saveTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    invoke<LLMSettings>("get_llm_settings")
      .then((s) => {
        setSettings(s);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const update = (patch: Partial<LLMSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    setTestStatus("idle");
    if (saveTimer.current) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      invoke("set_llm_settings", { settings: next }).catch(() => {});
    }, 400);
  };

  const test = async () => {
    setTestStatus("testing");
    try {
      await invoke("llm_ping");
      setTestStatus("ok");
      setTestMessage("Conexión OK");
    } catch (e) {
      setTestStatus("fail");
      setTestMessage(typeof e === "string" ? e : ((e as { message?: string })?.message ?? "Error"));
    }
  };

  if (!loaded) return null;

  return (
    <div>
      <h3 className="text-[11px] uppercase tracking-wider text-th-text-muted mb-3">
        AI backend
      </h3>

      <div className="flex items-center justify-between py-3 border-b border-th-border-subtle">
        <div className="flex items-center gap-3 min-w-0">
          <Sparkles size={16} className="text-th-text-muted shrink-0" />
          <div>
            <p className="text-[13px] text-th-text-secondary">Proveedor</p>
            <p className="text-[11px] text-th-text-muted">
              Para construir colas chateando con IA (botón ✨ en la PlayerBar)
            </p>
          </div>
        </div>
        <select
          value={settings.provider}
          onChange={(e) =>
            update({ provider: e.target.value as LLMProvider })
          }
          className="px-2 py-1 text-[12px] bg-th-bg-secondary border border-th-border-subtle rounded text-th-text-primary"
        >
          <option value="off">Desactivado</option>
          <option value="deepseek">DeepSeek (API)</option>
          <option value="ollama">Ollama (local)</option>
        </select>
      </div>

      {settings.provider === "deepseek" && (
        <div className="py-3 space-y-2 border-b border-th-border-subtle">
          <input
            type="password"
            value={settings.deepseekApiKey}
            onChange={(e) => update({ deepseekApiKey: e.target.value })}
            placeholder="DeepSeek API key"
            className="w-full px-2 py-1.5 text-[12px] bg-th-bg-secondary border border-th-border-subtle rounded text-th-text-primary placeholder:text-th-text-muted"
          />
          <input
            type="text"
            value={settings.deepseekModel}
            onChange={(e) => update({ deepseekModel: e.target.value })}
            placeholder="modelo (deepseek-chat)"
            className="w-full px-2 py-1.5 text-[12px] bg-th-bg-secondary border border-th-border-subtle rounded text-th-text-primary placeholder:text-th-text-muted"
          />
        </div>
      )}

      {settings.provider === "ollama" && (
        <div className="py-3 space-y-2 border-b border-th-border-subtle">
          <input
            type="text"
            value={settings.ollamaBaseUrl}
            onChange={(e) => update({ ollamaBaseUrl: e.target.value })}
            placeholder="Ollama URL (http://localhost:11434)"
            className="w-full px-2 py-1.5 text-[12px] bg-th-bg-secondary border border-th-border-subtle rounded text-th-text-primary placeholder:text-th-text-muted"
          />
          <input
            type="text"
            value={settings.ollamaModel}
            onChange={(e) => update({ ollamaModel: e.target.value })}
            placeholder="modelo (llama3.1:8b)"
            className="w-full px-2 py-1.5 text-[12px] bg-th-bg-secondary border border-th-border-subtle rounded text-th-text-primary placeholder:text-th-text-muted"
          />
        </div>
      )}

      {settings.provider !== "off" && (
        <div className="flex items-center gap-2 py-2">
          <button
            onClick={test}
            disabled={testStatus === "testing"}
            className="px-3 py-1 text-[12px] border border-th-border-subtle rounded-md text-th-text-secondary hover:text-th-text-primary hover:border-th-accent/50 transition-colors disabled:opacity-50"
          >
            {testStatus === "testing" ? (
              <span className="flex items-center gap-1">
                <Loader2 size={12} className="animate-spin" /> Probando…
              </span>
            ) : (
              "Probar conexión"
            )}
          </button>
          {testStatus === "ok" && (
            <span className="flex items-center gap-1 text-[11px] text-green-400">
              <CheckCircle2 size={12} /> {testMessage}
            </span>
          )}
          {testStatus === "fail" && (
            <span className="flex items-center gap-1 text-[11px] text-red-400">
              <XCircle size={12} /> {testMessage}
            </span>
          )}
        </div>
      )}
    </div>
  );
}

function ExplicitContentToggle() {
  const [allowExplicit, setAllowExplicit] = useAtom(allowExplicitAtom);
  const store = useStore();

  return (
    <div className="flex items-center justify-between py-3">
      <div className="flex items-center gap-3 min-w-0">
        <ShieldAlert size={16} className="text-th-text-muted shrink-0" />
        <div>
          <p className="text-[13px] text-th-text-secondary">
            Allow explicit content
          </p>
          <p className="text-[11px] text-th-text-muted">
            Allow playing tracks marked as explicit
          </p>
        </div>
      </div>
      <button
        onClick={() => {
          setAllowExplicit(!allowExplicit);
          invoke("stop_track").catch(() => {});
          invoke("notify_track_stopped").catch(() => {});
          store.set(currentTrackAtom, null);
          store.set(isPlayingAtom, false);
          store.set(queueAtom, []);
          store.set(manualQueueAtom, []);
          store.set(originalQueueAtom, null);
          store.set(historyAtom, []);
          store.set(playbackSourceAtom, null);
          store.set(contextSourceAtom, null);
        }}
      >
        <Toggle on={allowExplicit} />
      </button>
    </div>
  );
}
import { proxySettingsAtom, type ProxySettings } from "../atoms/proxy";
import { useToast } from "../contexts/ToastContext";
import { clearAllCache } from "../api/tidal";
import Toggle from "./Toggle";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

export default function SettingsModal({ open, onClose }: SettingsModalProps) {
  const [autoplay, setAutoplay] = useAtom(autoplayAtom);
  const [bitPerfect] = useAtom(bitPerfectAtom);
  const [volumeNormalization, setVolumeNormalization] = useState(false);
  const [discordRpc, setDiscordRpc] = useState(false);
  const [decorations, setDecorations] = useState(true);
  const [minimizeToTray, setMinimizeToTray] = useState(false);
  const [proxySettings, setProxySettings] = useAtom(proxySettingsAtom);
  const [proxyTestStatus, setProxyTestStatus] = useState<
    "idle" | "testing" | "success" | "error"
  >("idle");
  const [proxyTestMessage, setProxyTestMessage] = useState("");
  const { showToast } = useToast();
  const panelRef = useRef<HTMLDivElement>(null);
  const proxySaveTimer = useRef<number | undefined>(undefined);

  // Load backend-synced preferences when modal opens
  useEffect(() => {
    if (!open) return;
    invoke<boolean>("get_volume_normalization")
      .then(setVolumeNormalization)
      .catch(() => {});
    invoke<boolean>("get_decorations")
      .then(setDecorations)
      .catch(() => {});
    invoke<boolean>("get_minimize_to_tray")
      .then(setMinimizeToTray)
      .catch(() => {});
    invoke<boolean>("get_discord_rpc")
      .then(setDiscordRpc)
      .catch(() => {});
  }, [open]);

  // Close on click outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open, onClose]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  const updateProxy = (patch: Partial<ProxySettings>) => {
    const next = { ...proxySettings, ...patch };
    setProxySettings(next);
    setProxyTestStatus("idle");
    clearTimeout(proxySaveTimer.current);
    proxySaveTimer.current = window.setTimeout(() => {
      invoke("set_proxy_settings", { settings: next }).catch(() => {});
    }, 500);
  };

  const testProxy = async () => {
    setProxyTestStatus("testing");
    try {
      const msg = await invoke<string>("test_proxy_connection", {
        settings: proxySettings,
      });
      setProxyTestStatus("success");
      setProxyTestMessage(msg);
    } catch (e: any) {
      setProxyTestStatus("error");
      setProxyTestMessage(typeof e === "string" ? e : e.message || "Failed");
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fadeIn">
        <div
          ref={panelRef}
          className="bg-th-elevated rounded-xl shadow-2xl w-[440px] max-h-[80vh] flex flex-col overflow-hidden"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-5 pt-5 pb-3 border-b border-th-border-subtle">
            <h2 className="text-[16px] font-bold text-th-text-primary">
              Settings
            </h2>
            <button
              onClick={onClose}
              className="w-8 h-8 rounded-full flex items-center justify-center hover:bg-th-inset transition-colors text-th-text-muted hover:text-th-text-primary"
            >
              <X size={18} />
            </button>
          </div>

          {/* Scrollable content */}
          <div className="overflow-y-auto px-5 py-4 space-y-6">
            {/* ── Playback ── */}
            <div>
              <h3 className="text-[11px] uppercase tracking-wider text-th-text-muted mb-3">
                Playback
              </h3>

              {/* Autoplay */}
              <div className="flex items-center justify-between py-3 border-b border-th-border-subtle">
                <div className="flex items-center gap-3 min-w-0">
                  <InfinityIcon
                    size={16}
                    className="text-th-text-muted shrink-0"
                  />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">
                      Autoplay
                    </p>
                    <p className="text-[11px] text-th-text-muted">
                      Automatically play similar tracks when queue ends
                    </p>
                  </div>
                </div>
                <button onClick={() => setAutoplay(!autoplay)}>
                  <Toggle on={autoplay} />
                </button>
              </div>

              {/* Normalize volume */}
              <div
                className={`flex items-center justify-between py-3 ${
                  bitPerfect ? "opacity-50" : ""
                }`}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <Volume2
                    size={16}
                    className="text-th-text-muted shrink-0"
                  />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">
                      Normalize volume
                    </p>
                    <p className="text-[11px] text-th-text-muted">
                      Even out volume differences between tracks
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    if (bitPerfect) return;
                    const next = !volumeNormalization;
                    setVolumeNormalization(next);
                    invoke("set_volume_normalization", { enabled: next }).catch(
                      () => {},
                    );
                  }}
                  disabled={bitPerfect}
                  className={bitPerfect ? "cursor-not-allowed" : ""}
                >
                  <Toggle on={volumeNormalization} />
                </button>
              </div>

              {/* Allow explicit content */}
              <ExplicitContentToggle />
            </div>

            {/* ── Integrations ── */}
            <div>
              <h3 className="text-[11px] uppercase tracking-wider text-th-text-muted mb-3">
                Integrations
              </h3>

              {/* Discord Rich Presence */}
              <div className="flex items-center justify-between py-3">
                <div className="flex items-center gap-3 min-w-0">
                  <MessageSquare
                    size={16}
                    className="text-th-text-muted shrink-0"
                  />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">
                      Discord Rich Presence
                    </p>
                    <p className="text-[11px] text-th-text-muted">
                      Show what you're listening to on Discord
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    const next = !discordRpc;
                    setDiscordRpc(next);
                    invoke("set_discord_rpc", { enabled: next }).catch(() => {
                      setDiscordRpc(!next);
                    });
                  }}
                >
                  <Toggle on={discordRpc} />
                </button>
              </div>
            </div>

            {/* ── General ── */}
            <div>
              <h3 className="text-[11px] uppercase tracking-wider text-th-text-muted mb-3">
                General
              </h3>

              {/* Window decorations */}
              <div className="flex items-center justify-between py-3 border-b border-th-border-subtle">
                <div className="flex items-center gap-3 min-w-0">
                  <AppWindow
                    size={16}
                    className="text-th-text-muted shrink-0"
                  />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">
                      Window decorations
                    </p>
                    <p className="text-[11px] text-th-text-muted">
                      Show native title bar and window controls
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    const next = !decorations;
                    setDecorations(next);
                    invoke("set_decorations", { enabled: next }).catch(() => {
                      setDecorations(!next);
                      showToast("Failed to update window decorations");
                    });
                  }}
                >
                  <Toggle on={decorations} />
                </button>
              </div>

              {/* Close to tray */}
              <div className="flex items-center justify-between py-3">
                <div className="flex items-center gap-3 min-w-0">
                  <MonitorDown
                    size={16}
                    className="text-th-text-muted shrink-0"
                  />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">
                      Close to tray
                    </p>
                    <p className="text-[11px] text-th-text-muted">
                      Minimize to system tray instead of quitting
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    const next = !minimizeToTray;
                    setMinimizeToTray(next);
                    invoke("set_minimize_to_tray", { enabled: next }).catch(
                      () => {},
                    );
                  }}
                >
                  <Toggle on={minimizeToTray} />
                </button>
              </div>
            </div>

            {/* ── Network ── */}
            <div>
              <h3 className="text-[11px] uppercase tracking-wider text-th-text-muted mb-3">
                Network
              </h3>

              {/* Proxy toggle */}
              <div className="flex items-center justify-between py-3">
                <div className="flex items-center gap-3 min-w-0">
                  <Globe size={16} className="text-th-text-muted shrink-0" />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">Proxy</p>
                  </div>
                </div>
                <button
                  onClick={() =>
                    updateProxy({ enabled: !proxySettings.enabled })
                  }
                >
                  <Toggle on={proxySettings.enabled} />
                </button>
              </div>

              {/* Proxy config (visible when enabled) */}
              {proxySettings.enabled && (
                <div className="pl-9 space-y-2 pb-2">
                  {/* Type selector */}
                  <div className="flex gap-2">
                    {(["http", "socks5"] as const).map((t) => (
                      <button
                        key={t}
                        onClick={() => updateProxy({ proxy_type: t })}
                        className={`flex-1 text-[12px] py-1.5 rounded-md border transition-colors ${
                          proxySettings.proxy_type === t
                            ? "border-th-accent text-th-accent bg-th-accent/10"
                            : "border-th-border-subtle text-th-text-muted hover:border-th-accent/50"
                        }`}
                      >
                        {t.toUpperCase()}
                      </button>
                    ))}
                  </div>

                  {/* Host + Port */}
                  <div className="flex gap-2">
                    <input
                      type="text"
                      placeholder="Host"
                      value={proxySettings.host}
                      onChange={(e) => updateProxy({ host: e.target.value })}
                      className="flex-1 min-w-0 px-2.5 py-1.5 rounded-md bg-th-inset border border-th-border-subtle text-[12px] text-th-text-primary placeholder:text-th-text-muted focus:border-th-accent/50 focus:outline-none"
                    />
                    <input
                      type="number"
                      placeholder="Port"
                      value={proxySettings.port || ""}
                      onChange={(e) =>
                        updateProxy({ port: parseInt(e.target.value) || 0 })
                      }
                      className="w-20 px-2.5 py-1.5 rounded-md bg-th-inset border border-th-border-subtle text-[12px] text-th-text-primary placeholder:text-th-text-muted focus:border-th-accent/50 focus:outline-none"
                    />
                  </div>

                  {/* Username + Password */}
                  <input
                    type="text"
                    placeholder="Username (optional)"
                    value={proxySettings.username || ""}
                    onChange={(e) =>
                      updateProxy({ username: e.target.value || null })
                    }
                    className="w-full px-2.5 py-1.5 rounded-md bg-th-inset border border-th-border-subtle text-[12px] text-th-text-primary placeholder:text-th-text-muted focus:border-th-accent/50 focus:outline-none"
                  />
                  <input
                    type="password"
                    placeholder="Password (optional)"
                    value={proxySettings.password || ""}
                    onChange={(e) =>
                      updateProxy({ password: e.target.value || null })
                    }
                    className="w-full px-2.5 py-1.5 rounded-md bg-th-inset border border-th-border-subtle text-[12px] text-th-text-primary placeholder:text-th-text-muted focus:border-th-accent/50 focus:outline-none"
                  />

                  {/* Test button */}
                  <button
                    onClick={testProxy}
                    disabled={
                      proxyTestStatus === "testing" ||
                      !proxySettings.host ||
                      !proxySettings.port
                    }
                    className="w-full py-1.5 rounded-md text-[12px] font-medium border border-th-border-subtle text-th-text-secondary hover:text-th-text-primary hover:border-th-accent/50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {proxyTestStatus === "testing"
                      ? "Testing..."
                      : "Test Connection"}
                  </button>

                  {/* Test result */}
                  {proxyTestStatus === "success" && (
                    <p className="text-[11px] text-green-400">
                      {proxyTestMessage}
                    </p>
                  )}
                  {proxyTestStatus === "error" && (
                    <p className="text-[11px] text-red-400">
                      {proxyTestMessage}
                    </p>
                  )}
                </div>
              )}

              {!proxySettings.enabled && (
                <p className="text-[11px] text-th-text-muted pl-9 -mt-1">
                  Enable proxy to configure connection settings
                </p>
              )}
            </div>

            {/* ── AI Backend ── */}
            <AIBackendSection />

            {/* ── Utilities ── */}
            <div>
              <h3 className="text-[11px] uppercase tracking-wider text-th-text-muted mb-3">
                Utilities
              </h3>

              {/* Refresh App */}
              <div className="flex items-center justify-between py-3">
                <div className="flex items-center gap-3 min-w-0">
                  <RefreshCw
                    size={16}
                    className="text-th-text-muted shrink-0"
                  />
                  <div>
                    <p className="text-[13px] text-th-text-secondary">
                      Refresh App
                    </p>
                    <p className="text-[11px] text-th-text-muted">
                      Clear cache and reload the application
                    </p>
                  </div>
                </div>
                <button
                  onClick={async () => {
                    await clearAllCache();
                    window.location.reload();
                  }}
                  className="px-3 py-1 text-[12px] border border-th-border-subtle rounded-md text-th-text-secondary hover:text-th-text-primary hover:border-th-accent/50 transition-colors shrink-0"
                >
                  Refresh
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
  );
}
