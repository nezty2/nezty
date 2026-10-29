import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { CrosshairOverlay } from "./CrosshairOverlay";
import "./index.css";

// Rechtermuisknop-menu van de webview uit, zoals bij een echte desktop-app.
document.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement | null;
  if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
  e.preventDefault();
});

// Het crosshair-venster laadt dezelfde pagina met #crosshair en toont alleen de overlay.
const isOverlay = window.location.hash === "#crosshair";

ReactDOM.createRoot(document.getElementById("root")!).render(<React.StrictMode>{isOverlay ? <CrosshairOverlay /> : <App />}</React.StrictMode>);
