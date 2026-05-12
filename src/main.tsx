import ReactDOM from "react-dom/client";
import App from "./App";

// Note: the previous TEMP DebugBoundary that lived here was replaced in
// F8.6 by the permanent `ErrorBoundary` mounted inside `App.tsx`. The
// permanent boundary uses the app theme tokens, exposes a "Try again"
// button, and offers a "Copy diagnostics" affordance for bug reports.

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <App />,
);
