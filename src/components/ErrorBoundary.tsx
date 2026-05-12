import { Component, type ReactNode } from "react";

/**
 * Permanent root-level error boundary (F8.6).
 *
 * Replaces the temporary `DebugBoundary` previously inlined in
 * `main.tsx` while diagnosing the post-Phase 7 black-screen bug. The
 * permanent version:
 *
 *  - styles itself with the app theme tokens (no jarring red overlay).
 *  - logs to `console.error` so dev-tools / `/tmp/sone-dev.log` capture
 *    the full stack + componentStack.
 *  - exposes a "Try again" button that resets the boundary AND a "Copy
 *    diagnostics" button so a user can paste the crash into a bug report.
 *  - does NOT swallow the error — it remembers it, lets the user reset.
 *
 * Failure is local to the React subtree; Tauri-side state and the audio
 * path are untouched. The boundary is mounted inside `App.tsx` so it
 * sits ABOVE the routing logic and catches crashes from any page
 * (Explore, Classical Hub, Stats, Settings, Player).
 */

interface Props {
  children: ReactNode;
}

interface State {
  err?: Error;
  componentStack?: string;
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = {};

  static getDerivedStateFromError(err: Error): State {
    return { err };
  }

  componentDidCatch(err: Error, info: { componentStack: string }): void {
    // Surface to console so dev-tools + log redirection captures it.
    console.error(
      "[ErrorBoundary] React render crash:",
      err,
      "\n--- componentStack ---\n",
      info.componentStack,
    );
    this.setState({ componentStack: info.componentStack });
  }

  reset = (): void => {
    this.setState({ err: undefined, componentStack: undefined });
  };

  copyDiagnostics = async (): Promise<void> => {
    const { err, componentStack } = this.state;
    if (err === undefined) {
      return;
    }
    const blob = [
      `Sone — UI render crash`,
      `Time: ${new Date().toISOString()}`,
      `Message: ${err.message}`,
      ``,
      `Stack:`,
      err.stack ?? "(no stack)",
      ``,
      `Component stack:`,
      componentStack ?? "(no component stack)",
    ].join("\n");
    try {
      await navigator.clipboard.writeText(blob);
    } catch (clipErr) {
      console.error("[ErrorBoundary] clipboard write failed:", clipErr);
    }
  };

  render(): ReactNode {
    const { err, componentStack } = this.state;
    if (err === undefined) {
      return this.props.children;
    }
    return (
      <div className="min-h-screen w-full bg-th-bg text-th-text-primary flex items-center justify-center p-8">
        <div className="max-w-2xl w-full rounded-lg border border-th-divider bg-th-surface/40 p-6 shadow-lg">
          <h1 className="text-lg font-semibold mb-2">
            Something went wrong
          </h1>
          <p className="text-sm text-th-text-secondary mb-4">
            The interface hit an unexpected error. The audio engine and
            your settings are unaffected. You can try resuming, or copy
            the diagnostics below for a bug report.
          </p>
          <pre className="text-xs font-mono bg-black/40 text-amber-200/90 p-3 rounded border border-th-divider overflow-auto max-h-48 mb-4 whitespace-pre-wrap break-words">
            {err.message}
          </pre>
          {componentStack !== undefined ? (
            <details className="mb-4 text-xs text-th-text-faint">
              <summary className="cursor-pointer select-none hover:text-th-text-secondary">
                Component stack
              </summary>
              <pre className="mt-2 font-mono bg-black/30 p-2 rounded overflow-auto max-h-48 whitespace-pre-wrap break-words">
                {componentStack.trim()}
              </pre>
            </details>
          ) : null}
          <div className="flex gap-2">
            <button
              type="button"
              onClick={this.reset}
              className="px-4 py-2 rounded bg-th-accent text-th-bg font-semibold text-sm hover:opacity-90 transition-opacity"
            >
              Try again
            </button>
            <button
              type="button"
              onClick={this.copyDiagnostics}
              className="px-4 py-2 rounded border border-th-divider bg-th-surface-hover text-th-text-primary text-sm hover:bg-th-surface transition-colors"
            >
              Copy diagnostics
            </button>
          </div>
        </div>
      </div>
    );
  }
}
