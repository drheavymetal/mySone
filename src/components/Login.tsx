import { useState, useRef, useEffect } from "react";
import { useAudioContext } from "../contexts/AudioContext";
import {
  Loader2,
  ExternalLink,
  ClipboardPaste,
  KeyRound,
  Eye,
  EyeOff,
  Terminal,
  Check,
  HelpCircle,
  X,
  Info,
  Zap,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

// ==================== How-To Modal ====================

function HowToModal({ onClose }: { onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-[#1a1a1a] border border-white/[0.08] rounded-2xl shadow-2xl max-w-lg w-full mx-4 max-h-[85vh] overflow-y-auto">
        <div className="sticky top-0 bg-[#1a1a1a] border-b border-white/[0.06] px-6 py-4 flex items-center justify-between rounded-t-2xl">
          <h2 className="text-[16px] font-bold text-white">
            How to Get Your Credentials
          </h2>
          <button
            onClick={onClose}
            className="text-[#666] hover:text-white transition-colors p-1"
          >
            <X size={18} />
          </button>
        </div>

        <div className="px-6 py-5 space-y-5 text-[13px]">
          {/* Step 1 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">1</span>
            </div>
            <div>
              <p className="text-white font-medium">
                Open the Tidal Web Player
              </p>
              <p className="text-[#808080] mt-1">
                Go to{" "}
                <code className="text-[#00FFFF]/80 bg-white/[0.05] px-1 rounded">
                  listen.tidal.com
                </code>{" "}
                in Chrome, Firefox, or Edge.
              </p>
            </div>
          </div>

          {/* Step 2 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">2</span>
            </div>
            <div>
              <p className="text-white font-medium">Open Developer Tools</p>
              <p className="text-[#808080] mt-1">
                Press{" "}
                <kbd className="px-1.5 py-0.5 bg-white/[0.08] rounded text-[#ccc] text-[11px] font-mono">
                  F12
                </kbd>{" "}
                on your keyboard, or right-click anywhere and select{" "}
                <span className="text-[#ccc]">Inspect</span>.
              </p>
            </div>
          </div>

          {/* Step 3 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">3</span>
            </div>
            <div>
              <p className="text-white font-medium">Go to the Network Tab</p>
              <p className="text-[#808080] mt-1">
                Click on the{" "}
                <span className="text-[#ccc]">Network</span> tab in the
                Developer Tools panel.
              </p>
              <div className="mt-2 p-2.5 bg-amber-900/20 border border-amber-700/30 rounded-lg text-amber-400 text-[12px]">
                Pro tip: Check the{" "}
                <span className="text-amber-300 font-medium">
                  "Preserve log"
                </span>{" "}
                checkbox to prevent logs from clearing if the page redirects.
              </div>
            </div>
          </div>

          {/* Step 4 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">4</span>
            </div>
            <div>
              <p className="text-white font-medium">Filter for "token"</p>
              <p className="text-[#808080] mt-1">
                In the filter box at the top of the Network tab, type{" "}
                <code className="text-[#00FFFF]/80 bg-white/[0.05] px-1 rounded">
                  token
                </code>
                .
              </p>
            </div>
          </div>

          {/* Step 5 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">5</span>
            </div>
            <div>
              <p className="text-white font-medium">
                Trigger the Login or Refresh
              </p>
              <p className="text-[#808080] mt-1">
                If you're logged out, log in normally. If you're already logged
                in, just refresh the page with{" "}
                <kbd className="px-1.5 py-0.5 bg-white/[0.08] rounded text-[#ccc] text-[11px] font-mono">
                  F5
                </kbd>
                . Tidal refreshes its auth token on load.
              </p>
            </div>
          </div>

          {/* Step 6 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">6</span>
            </div>
            <div>
              <p className="text-white font-medium">Find the Token Request</p>
              <p className="text-[#808080] mt-1">
                Look for a request named{" "}
                <code className="text-[#00FFFF]/80 bg-white/[0.05] px-1 rounded">
                  token
                </code>{" "}
                or{" "}
                <code className="text-[#00FFFF]/80 bg-white/[0.05] px-1 rounded">
                  oauth2/token
                </code>
                . Click on it.
              </p>
            </div>
          </div>

          {/* Step 7 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">7</span>
            </div>
            <div>
              <p className="text-white font-medium">Copy as cURL</p>
              <p className="text-[#808080] mt-1">
                Right-click the{" "}
                <code className="text-[#00FFFF]/80 bg-white/[0.05] px-1 rounded">
                  token
                </code>{" "}
                request, hover over{" "}
                <span className="text-[#ccc]">Copy</span>, then select{" "}
                <span className="text-[#ccc] font-medium">
                  Copy as cURL (bash)
                </span>
                .
              </p>
            </div>
          </div>

          {/* Step 8 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">8</span>
            </div>
            <div>
              <p className="text-white font-medium">
                Paste the cURL into Tide Vibe
              </p>
              <p className="text-[#808080] mt-1">
                Paste into the{" "}
                <span className="text-[#ccc]">Quick Import</span> text area and
                click{" "}
                <span className="text-[#ccc]">Extract Credentials</span>. This
                gives us your Client ID.
              </p>
            </div>
          </div>

          {/* Step 9 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
              <span className="text-[12px] font-bold text-[#00FFFF]">9</span>
            </div>
            <div>
              <p className="text-white font-medium">
                Copy the Response body
              </p>
              <p className="text-[#808080] mt-1">
                Back in DevTools, click on the same{" "}
                <code className="text-[#00FFFF]/80 bg-white/[0.05] px-1 rounded">
                  token
                </code>{" "}
                request. Switch to the{" "}
                <span className="text-[#ccc] font-medium">Response</span> tab
                (or <span className="text-[#ccc]">Preview</span>). Select all
                the JSON text and copy it.
              </p>
            </div>
          </div>

          {/* Step 10 */}
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-emerald-500/30 flex items-center justify-center shrink-0 mt-0.5">
              <Check size={12} className="text-emerald-400" />
            </div>
            <div>
              <p className="text-white font-medium">
                Paste the Response and log in
              </p>
              <p className="text-[#808080] mt-1">
                Paste the JSON response into Quick Import and click{" "}
                <span className="text-[#ccc]">Extract Credentials</span> again.
                The app will find your session tokens and show a{" "}
                <span className="text-[#ccc]">Log In Instantly</span> button.
              </p>
            </div>
          </div>

          {/* Note about quality */}
          <div className="mt-4 p-3 bg-[#0a0a0a] border border-white/[0.06] rounded-lg text-[12px] text-[#808080]">
            <p className="text-[#ccc] font-medium mb-1">
              About audio quality:
            </p>
            <p>
              The web player uses public PKCE credentials which typically
              include a{" "}
              <span className="text-white">Client ID</span> but{" "}
              <span className="text-amber-400">no Client Secret</span>. Without
              a secret, streaming is limited to{" "}
              <span className="text-white">
                Lossless (CD quality, 16-bit/44.1kHz)
              </span>
              . For Hi-Res (24-bit/192kHz), you need credentials that include
              a Client Secret (typically from a native app).
            </p>
          </div>
        </div>

        <div className="sticky bottom-0 bg-[#1a1a1a] border-t border-white/[0.06] px-6 py-4 rounded-b-2xl">
          <button
            onClick={onClose}
            className="w-full px-4 py-2.5 bg-white/[0.08] hover:bg-white/[0.12] rounded-full text-[13px] text-white font-medium transition-colors"
          >
            Got it
          </button>
        </div>
      </div>
    </div>
  );
}

// ==================== Login Component ====================

export default function Login() {
  const {
    startPkceAuth,
    completePkceAuth,
    importSession,
    getUserPlaylists,
    getSavedCredentials,
    parseCredentials,
  } = useAudioContext();
  const [step, setStep] = useState<
    "idle" | "waiting" | "exchanging" | "importing"
  >("idle");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [authorizeUrl, setAuthorizeUrl] = useState("");
  const [pasteUrl, setPasteUrl] = useState("");
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [showSecret, setShowSecret] = useState(false);
  const [credentialsLoaded, setCredentialsLoaded] = useState(false);
  const [showHowTo, setShowHowTo] = useState(false);

  // Quick Import state — two separate fields
  const [curlText, setCurlText] = useState("");
  const [responseText, setResponseText] = useState("");
  const [curlMasked, setCurlMasked] = useState(false);
  const [responseMasked, setResponseMasked] = useState(false);
  const [importFeedback, setImportFeedback] = useState<{
    type: "success" | "error" | "session";
    message: string;
  } | null>(null);
  const [secretHighlight, setSecretHighlight] = useState(false);
  const [extracting, setExtracting] = useState(false);

  const pkceRef = useRef<{
    codeVerifier: string;
    clientUniqueKey: string;
  } | null>(null);

  // Load saved credentials on mount
  useEffect(() => {
    const loadCreds = async () => {
      const { clientId: savedId, clientSecret: savedSecret } =
        await getSavedCredentials();
      if (savedId) setClientId(savedId);
      if (savedSecret) setClientSecret(savedSecret);
      setCredentialsLoaded(true);
    };
    loadCreds();
  }, []);

  // Auto-mask when text is long (likely contains tokens/secrets)
  useEffect(() => {
    if (curlText.length > 200) setCurlMasked(true);
  }, [curlText]);
  useEffect(() => {
    if (responseText.length > 200) setResponseMasked(true);
  }, [responseText]);

  const handleImportSession = async () => {
    if (!curlText.trim() && !responseText.trim()) return;

    setExtracting(true);
    setImportFeedback(null);

    try {
      // Parse both fields — merge results
      let parsedClientId: string | undefined;
      let parsedClientSecret: string | undefined;
      let parsedRefreshToken: string | undefined;
      let parsedAccessToken: string | undefined;

      // Parse cURL (gets client_id, maybe client_secret, maybe refresh_token)
      if (curlText.trim()) {
        const curlResult = await parseCredentials(curlText.trim());
        parsedClientId = curlResult.clientId || undefined;
        parsedClientSecret = curlResult.clientSecret || undefined;
        // cURL for grant_type=refresh_token may contain refresh_token
        if (curlResult.refreshToken) parsedRefreshToken = curlResult.refreshToken;
        if (curlResult.accessToken) parsedAccessToken = curlResult.accessToken;
      }

      // Parse response JSON (gets access_token, refresh_token)
      if (responseText.trim()) {
        const respResult = await parseCredentials(responseText.trim());
        if (respResult.accessToken) parsedAccessToken = respResult.accessToken;
        if (respResult.refreshToken) parsedRefreshToken = respResult.refreshToken;
        // Response JSON may also have client_id in some cases
        if (respResult.clientId && !parsedClientId)
          parsedClientId = respResult.clientId;
      }

      if (!parsedClientId) {
        setImportFeedback({
          type: "error",
          message:
            "Could not find a Client ID. Make sure you pasted the cURL command in the first field.",
        });
        setExtracting(false);
        return;
      }

      if (!parsedRefreshToken && !parsedAccessToken) {
        setImportFeedback({
          type: "error",
          message:
            "No session tokens found. Make sure you pasted the Response body (JSON) from the token request in the second field.",
        });
        setExtracting(false);
        return;
      }

      // We have everything — import the session
      setStep("importing");
      setStatus("Importing session...");
      setError(null);

      const tokens = await importSession(
        parsedClientId,
        parsedClientSecret,
        parsedRefreshToken || "",
        parsedAccessToken
      );

      setStatus("Loading your library...");
      if (tokens.user_id) {
        await getUserPlaylists(tokens.user_id);
      }

      setStatus("");
      setStep("idle");
    } catch (err: any) {
      setError(`Session import failed: ${err?.message || err}`);
      setStep("idle");
      setStatus("");
    } finally {
      setExtracting(false);
    }
  };

  const handlePasteCurl = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        setCurlText(text);
        setImportFeedback(null);
      }
    } catch {
      // Clipboard access denied
    }
  };

  const handlePasteResponse = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        setResponseText(text);
        setImportFeedback(null);
      }
    } catch {
      // Clipboard access denied
    }
  };

  const handleLogin = async () => {
    if (!clientId.trim()) {
      setError("Client ID is required to log in.");
      return;
    }
    if (!clientSecret.trim()) {
      setError(
        "Client Secret is required for the manual login flow. If you only have a Client ID from the web player, use Quick Import with the cURL command instead."
      );
      return;
    }

    try {
      setError(null);
      setStep("waiting");
      setStatus("Preparing login...");

      const params = await startPkceAuth(clientId.trim());
      pkceRef.current = {
        codeVerifier: params.codeVerifier,
        clientUniqueKey: params.clientUniqueKey,
      };
      setAuthorizeUrl(params.authorizeUrl);
      setStatus("");

      // Open browser
      try {
        await openUrl(params.authorizeUrl);
      } catch {
        window.open(params.authorizeUrl, "_blank");
      }
    } catch (err: any) {
      setError(`Failed to start login: ${err?.message || err}`);
      setStep("idle");
    }
  };

  const handleSubmitUrl = async () => {
    if (!pasteUrl.trim() || !pkceRef.current) return;

    try {
      // Extract code from the pasted URL
      let code: string | null = null;
      try {
        const url = new URL(pasteUrl.trim());
        code = url.searchParams.get("code");
      } catch {
        // Maybe they pasted just the code
        if (pasteUrl.trim().length > 10 && !pasteUrl.includes(" ")) {
          code = pasteUrl.trim();
        }
      }

      if (!code) {
        setError(
          "Could not find authorization code in the URL. Make sure you copied the full URL from the browser."
        );
        return;
      }

      setStep("exchanging");
      setStatus("Completing login...");

      const tokens = await completePkceAuth(
        code,
        pkceRef.current.codeVerifier,
        pkceRef.current.clientUniqueKey,
        clientId.trim(),
        clientSecret.trim()
      );

      setStatus("Loading your library...");
      if (tokens.user_id) {
        await getUserPlaylists(tokens.user_id);
      }

      setStep("idle");
      setStatus("");
    } catch (err: any) {
      setError(`Authentication failed: ${err?.message || err}`);
      setStep("waiting");
      setStatus("");
    }
  };

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        setPasteUrl(text);
      }
    } catch {
      // Clipboard access denied, user can paste manually
    }
  };

  const reset = () => {
    setError(null);
    setStep("idle");
    setStatus("");
    setPasteUrl("");
    setAuthorizeUrl("");
    pkceRef.current = null;
  };

  if (!credentialsLoaded) {
    return (
      <div className="flex items-center justify-center h-screen w-screen bg-gradient-to-br from-[#0a0a0a] via-[#121212] to-[#0a0a0a]">
        <Loader2 className="animate-spin text-[#00FFFF]" size={32} />
      </div>
    );
  }

  return (
    <div className="flex items-center justify-center h-screen w-screen bg-gradient-to-br from-[#0a0a0a] via-[#121212] to-[#0a0a0a]">
      {showHowTo && <HowToModal onClose={() => setShowHowTo(false)} />}

      <div className="text-center p-10 bg-[#1a1a1a]/60 backdrop-blur-sm rounded-2xl shadow-2xl border border-white/[0.06] max-w-lg w-full mx-4 max-h-[90vh] overflow-y-auto">
        <div className="mb-6 flex items-center justify-center gap-3">
          <div className="w-12 h-12 bg-white text-black font-extrabold flex items-center justify-center rounded-md text-2xl">
            T
          </div>
          <h1 className="text-4xl font-bold tracking-tight text-white">
            TIDE VIBE
          </h1>
        </div>

        {step === "idle" && (
          <>
            <p className="text-[#a6a6a6] mb-2 text-lg">
              Connect your Tidal account to start streaming
            </p>
            <button
              onClick={() => setShowHowTo(true)}
              className="inline-flex items-center gap-1.5 text-[12px] text-[#00FFFF]/70 hover:text-[#00FFFF] transition-colors mb-6"
            >
              <HelpCircle size={13} />
              How do I get my credentials?
            </button>

            {/* ==================== Quick Import ==================== */}
            <div className="text-left bg-[#0f0f0f] rounded-xl p-5 border border-white/[0.06] mb-4">
              <div className="flex items-center gap-2 mb-2">
                <Terminal size={16} className="text-[#00FFFF]" />
                <span className="text-[14px] text-white font-medium">
                  Quick Import
                </span>
                <span className="text-[10px] text-[#555] ml-auto">
                  Recommended
                </span>
              </div>
              <p className="text-[12px] text-[#666] mb-4">
                From your browser's Network tab, find the{" "}
                <code className="text-[#888] bg-white/[0.05] px-1 rounded">
                  token
                </code>{" "}
                request and paste both the cURL command and the response body
                below.
              </p>

              {/* Field 1: cURL */}
              <div className="mb-3">
                <div className="flex items-center justify-between mb-1.5">
                  <label className="text-[12px] text-[#999] font-medium">
                    1. cURL Command
                  </label>
                  <button
                    onClick={handlePasteCurl}
                    className="flex items-center gap-1 px-2 py-0.5 text-[11px] text-[#666] hover:text-white transition-colors"
                    title="Paste from clipboard"
                  >
                    <ClipboardPaste size={11} />
                    Paste
                  </button>
                </div>
                <div className="relative">
                  <textarea
                    value={curlText}
                    onChange={(e) => {
                      setCurlText(e.target.value);
                      setImportFeedback(null);
                    }}
                    placeholder="Right-click the token request → Copy → Copy as cURL (bash)"
                    rows={2}
                    className={`w-full bg-[#1a1a1a] border border-white/[0.1] rounded-lg px-3 py-2 text-[12px] text-white placeholder-[#444] outline-none focus:border-[#00FFFF]/50 font-mono resize-none ${
                      curlMasked && curlText.length > 0
                        ? "[-webkit-text-security:disc]"
                        : ""
                    }`}
                  />
                  {curlText.length > 0 && (
                    <button
                      type="button"
                      onClick={() => setCurlMasked(!curlMasked)}
                      className="absolute right-2 top-2 text-[#555] hover:text-[#999] transition-colors"
                      title={curlMasked ? "Reveal" : "Mask"}
                    >
                      {curlMasked ? <Eye size={12} /> : <EyeOff size={12} />}
                    </button>
                  )}
                </div>
                <p className="text-[10px] text-[#444] mt-1">
                  Provides your Client ID (and Client Secret if available).
                </p>
              </div>

              {/* Field 2: Response body */}
              <div className="mb-3">
                <div className="flex items-center justify-between mb-1.5">
                  <label className="text-[12px] text-[#999] font-medium">
                    2. Response Body
                  </label>
                  <button
                    onClick={handlePasteResponse}
                    className="flex items-center gap-1 px-2 py-0.5 text-[11px] text-[#666] hover:text-white transition-colors"
                    title="Paste from clipboard"
                  >
                    <ClipboardPaste size={11} />
                    Paste
                  </button>
                </div>
                <div className="relative">
                  <textarea
                    value={responseText}
                    onChange={(e) => {
                      setResponseText(e.target.value);
                      setImportFeedback(null);
                    }}
                    placeholder='Click the same token request → Response tab → copy the JSON&#10;{"access_token":"...","refresh_token":"..."}'
                    rows={2}
                    className={`w-full bg-[#1a1a1a] border border-white/[0.1] rounded-lg px-3 py-2 text-[12px] text-white placeholder-[#444] outline-none focus:border-[#00FFFF]/50 font-mono resize-none ${
                      responseMasked && responseText.length > 0
                        ? "[-webkit-text-security:disc]"
                        : ""
                    }`}
                  />
                  {responseText.length > 0 && (
                    <button
                      type="button"
                      onClick={() => setResponseMasked(!responseMasked)}
                      className="absolute right-2 top-2 text-[#555] hover:text-[#999] transition-colors"
                      title={responseMasked ? "Reveal" : "Mask"}
                    >
                      {responseMasked ? (
                        <Eye size={12} />
                      ) : (
                        <EyeOff size={12} />
                      )}
                    </button>
                  )}
                </div>
                <p className="text-[10px] text-[#444] mt-1">
                  Provides your session tokens (access_token, refresh_token).
                </p>
              </div>

              {/* Import button */}
              <button
                onClick={handleImportSession}
                disabled={
                  (!curlText.trim() && !responseText.trim()) || extracting
                }
                className="w-full flex items-center justify-center gap-2 px-4 py-3 bg-[#00FFFF] text-black rounded-lg font-bold text-[13px] hover:brightness-110 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              >
                {extracting ? (
                  <Loader2 size={15} className="animate-spin" />
                ) : (
                  <Zap size={15} />
                )}
                Import Session
              </button>

              {/* Feedback */}
              {importFeedback && (
                <div
                  className={`mt-3 flex items-start gap-2 p-2.5 rounded-lg text-[12px] ${
                    importFeedback.type === "success"
                      ? "bg-emerald-900/30 border border-emerald-700/40 text-emerald-400"
                      : importFeedback.type === "session"
                        ? "bg-[#00FFFF]/10 border border-[#00FFFF]/30 text-[#00FFFF]"
                        : "bg-red-900/30 border border-red-700/40 text-red-400"
                  }`}
                >
                  {importFeedback.type === "success" && (
                    <Check size={14} className="shrink-0 mt-0.5" />
                  )}
                  {importFeedback.type === "session" && (
                    <Zap size={14} className="shrink-0 mt-0.5" />
                  )}
                  <span>{importFeedback.message}</span>
                </div>
              )}
            </div>

            {/* ==================== Manual PKCE Login ==================== */}
            <div className="text-left bg-[#0f0f0f] rounded-xl p-5 border border-white/[0.06] mb-6">
              <div className="flex items-center gap-2 mb-3">
                <KeyRound size={16} className="text-[#555]" />
                <span className="text-[14px] text-[#999] font-medium">
                  Manual Login
                </span>
                <span className="text-[10px] text-[#444] ml-auto">
                  Native app credentials only
                </span>
              </div>
              <p className="text-[12px] text-[#555] mb-4">
                For advanced users with native app (Android) credentials.
                Requires <span className="text-[#999]">both</span> a Client ID
                and Client Secret.{" "}
                <span className="text-amber-400/70">
                  Web player credentials will not work here
                </span>{" "}
                -- use Quick Import above instead.
              </p>

              <div className="space-y-3">
                {/* Client ID */}
                <div>
                  <label className="block text-[12px] text-[#666] mb-1">
                    Client ID
                  </label>
                  <input
                    type="text"
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    placeholder="Native app Client ID"
                    className="w-full bg-[#1a1a1a] border border-white/[0.1] rounded-lg px-3 py-2.5 text-[13px] text-white placeholder-[#444] outline-none focus:border-[#00FFFF]/50 font-mono"
                  />
                </div>

                {/* Client Secret */}
                <div>
                  <label className="block text-[12px] text-[#666] mb-1">
                    Client Secret
                  </label>
                  <div className="relative">
                    <input
                      type={showSecret ? "text" : "password"}
                      value={clientSecret}
                      onChange={(e) => {
                        setClientSecret(e.target.value);
                        setSecretHighlight(false);
                      }}
                      placeholder="Required for this login method"
                      className={`w-full bg-[#1a1a1a] border rounded-lg px-3 py-2.5 pr-10 text-[13px] text-white placeholder-[#444] outline-none font-mono transition-colors ${
                        secretHighlight
                          ? "border-amber-500/60 focus:border-amber-500/80"
                          : "border-white/[0.1] focus:border-[#00FFFF]/50"
                      }`}
                    />
                    <button
                      type="button"
                      onClick={() => setShowSecret(!showSecret)}
                      className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[#666] hover:text-[#999] transition-colors"
                    >
                      {showSecret ? <EyeOff size={15} /> : <Eye size={15} />}
                    </button>
                  </div>
                  {secretHighlight && (
                    <p className="text-[11px] text-amber-400/80 mt-1.5">
                      A Client Secret is required for the manual PKCE login
                      flow.
                    </p>
                  )}
                </div>
              </div>

              {/* Quality indicator */}
              {clientId.trim() && clientSecret.trim() && (
                <div className="mt-4 flex items-start gap-2 p-2.5 rounded-lg text-[12px] bg-emerald-900/20 border border-emerald-700/30 text-emerald-400">
                  <Info size={14} className="shrink-0 mt-0.5" />
                  <span>
                    <span className="font-medium">Hi-Res ready:</span>{" "}
                    Streaming up to 24-bit/192kHz with automatic token refresh.
                  </span>
                </div>
              )}

              {/* Warning when only client ID is provided */}
              {clientId.trim() && !clientSecret.trim() && (
                <div className="mt-4 flex items-start gap-2 p-2.5 rounded-lg text-[12px] bg-amber-900/20 border border-amber-700/30 text-amber-400">
                  <Info size={14} className="shrink-0 mt-0.5" />
                  <span>
                    A Client Secret is required for manual login. If you only
                    have a Client ID (e.g. from the web player), use{" "}
                    <span className="font-medium">Quick Import</span> above
                    with the cURL command instead.
                  </span>
                </div>
              )}

              <button
                onClick={handleLogin}
                disabled={!clientId.trim() || !clientSecret.trim()}
                className="mt-4 w-full px-6 py-3 bg-white/[0.1] text-white font-bold rounded-full hover:bg-white/[0.15] transition-all text-[15px] disabled:opacity-30 disabled:cursor-not-allowed"
              >
                Login with Tidal
              </button>

              <p className="text-[11px] text-[#444] text-center mt-2">
                Opens Tidal's login page via the Android redirect URI. Your
                credentials are stored locally.
              </p>
            </div>
          </>
        )}

        {step === "importing" && (
          <div className="flex flex-col items-center gap-4 py-8">
            <Loader2 className="animate-spin text-[#00FFFF]" size={32} />
            <p className="text-[#a6a6a6]">
              {status || "Importing session..."}
            </p>
          </div>
        )}

        {step === "waiting" && (
          <div className="flex flex-col gap-5">
            <div className="text-left bg-[#0f0f0f] rounded-xl p-5 border border-white/[0.06]">
              <div className="flex items-start gap-3 mb-4">
                <div className="w-7 h-7 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
                  <span className="text-[13px] font-bold text-[#00FFFF]">
                    1
                  </span>
                </div>
                <div>
                  <p className="text-[14px] text-white font-medium">
                    Log in to Tidal in your browser
                  </p>
                  <p className="text-[12px] text-[#808080] mt-1">
                    A browser window should have opened. If not, click the
                    button below.
                  </p>
                </div>
              </div>

              <div className="flex items-start gap-3 mb-4">
                <div className="w-7 h-7 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
                  <span className="text-[13px] font-bold text-[#00FFFF]">
                    2
                  </span>
                </div>
                <div>
                  <p className="text-[14px] text-white font-medium">
                    Copy the URL from the redirect page
                  </p>
                  <p className="text-[12px] text-[#808080] mt-1">
                    After login you'll see an "Oops" page. Copy the full URL
                    from the browser address bar.
                  </p>
                </div>
              </div>

              <div className="flex items-start gap-3">
                <div className="w-7 h-7 rounded-full bg-[#00FFFF]/20 flex items-center justify-center shrink-0 mt-0.5">
                  <span className="text-[13px] font-bold text-[#00FFFF]">
                    3
                  </span>
                </div>
                <div className="flex-1">
                  <p className="text-[14px] text-white font-medium mb-2">
                    Paste the URL here
                  </p>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={pasteUrl}
                      onChange={(e) => setPasteUrl(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleSubmitUrl();
                      }}
                      placeholder="https://tidal.com/android/login/auth?code=..."
                      className="flex-1 bg-[#1a1a1a] border border-white/[0.1] rounded-lg px-3 py-2 text-[13px] text-white placeholder-[#555] outline-none focus:border-[#00FFFF]/50 min-w-0"
                    />
                    <button
                      onClick={handlePaste}
                      className="px-3 py-2 bg-white/[0.08] hover:bg-white/[0.12] rounded-lg text-[#a6a6a6] hover:text-white transition-colors shrink-0"
                      title="Paste from clipboard"
                    >
                      <ClipboardPaste size={16} />
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <div className="flex gap-3">
              <button
                onClick={() => {
                  try {
                    openUrl(authorizeUrl);
                  } catch {
                    window.open(authorizeUrl, "_blank");
                  }
                }}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-white/[0.08] hover:bg-white/[0.12] rounded-full text-[13px] text-white font-medium transition-colors"
              >
                <ExternalLink size={14} />
                Open Tidal Login
              </button>
              <button
                onClick={handleSubmitUrl}
                disabled={!pasteUrl.trim()}
                className="flex-1 px-4 py-2.5 bg-[#00FFFF] text-black rounded-full text-[13px] font-bold hover:brightness-110 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Complete Login
              </button>
            </div>

            <button
              onClick={reset}
              className="text-[12px] text-[#666] hover:text-[#999] transition-colors"
            >
              Cancel
            </button>
          </div>
        )}

        {step === "exchanging" && (
          <div className="flex flex-col items-center gap-4 py-8">
            <Loader2 className="animate-spin text-[#00FFFF]" size={32} />
            <p className="text-[#a6a6a6]">{status || "Completing login..."}</p>
          </div>
        )}

        {error && (
          <div className="mt-5 p-4 bg-red-900/30 border border-red-700/50 rounded-lg text-red-400 text-sm">
            {error}
            <button
              onClick={() => setError(null)}
              className="mt-2 block w-full text-center underline text-red-300 hover:text-red-200"
            >
              Dismiss
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
