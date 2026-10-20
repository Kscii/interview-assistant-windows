import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { OverlayWindow, SelectorWindow } from "./windows";
import "./style.css";
const mode = new URLSearchParams(location.search).get("window");
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {mode === "overlay" ? (
      <OverlayWindow />
    ) : mode === "selector" ? (
      <SelectorWindow />
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
