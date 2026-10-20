import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { rpc, subscribe } from "./api";
import type { Caption, Monitor, Overlay } from "./types";
function useEvent<T>(name: string, fn: (payload: T) => void) {
  const ref = useRef(fn);
  ref.current = fn;
  useEffect(() => {
    const result = subscribe<T>(name, (v) => ref.current(v));
    return () => {
      void result.then((f) => f());
    };
  }, [name]);
}
export function OverlayWindow() {
  const [state, setState] = useState<Caption | null>(null);
  const [style, setStyle] = useState<Overlay | null>(null);
  const [adjust, setAdjust] = useState(false);
  const [flash, setFlash] = useState(false);
  const [error, setError] = useState("");
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => {
    void rpc<{ caption: Caption; overlay: Overlay; adjusting: boolean }>(
      "caption",
    ).then((s) => {
      setState(s.caption);
      setStyle(s.overlay);
      setAdjust(s.adjusting);
    });
  }, []);
  useEffect(() => {
    content.current?.scrollTo({ top: 0 });
  }, [state?.request_id]);
  useEvent<Caption>("caption", (s) => {
    setState(s);
    setError("");
  });
  useEvent<Overlay>("overlay-settings", setStyle);
  useEvent<boolean>("overlay-mode", setAdjust);
  useEvent<string>("notice", setError);
  useEvent("capture-feedback", () => {
    setError("");
    setFlash(false);
    requestAnimationFrame(() => setFlash(true));
  });
  return (
    <div
      className={`caption-window ${adjust ? "adjust" : ""} ${flash ? "flash" : ""}`}
      onAnimationEnd={() => setFlash(false)}
      style={{
        fontSize: style?.font_size ?? 28,
        fontFamily: style?.font_family ?? "Segoe UI",
      }}
    >
      {adjust && (
        <div
          className="caption-tools"
          onPointerDown={(e) => {
            if ((e.target as HTMLElement).tagName !== "BUTTON")
              void getCurrentWindow().startDragging();
          }}
        >
          <span>拖动此处移动 · 拖动窗口边缘调整大小</span>
          <button
            onClick={() =>
              void rpc("finish_adjusting").catch((e) => setError(String(e)))
            }
          >
            完成调整
          </button>
        </div>
      )}
      {error && <p className="caption-notice">{error}</p>}
      <div className="caption-content" ref={content}>
        <div className="caption-question">
          {state?.question || "按识别快捷键读取问题"}
        </div>
        {state?.answer ? (
          <div className="caption-answer">
            {state.answer.split("|").map((t, i) => (
              <span key={i} className={i ? "optional-answer" : ""}>
                {i > 0 && <span className="separator"> | </span>}
                {t}
              </span>
            ))}
          </div>
        ) : (
          <div className="caption-wait">
            {state?.detail || "题目原文与回答将在这里显示"}
          </div>
        )}
      </div>
    </div>
  );
}
export function SelectorWindow() {
  const [monitor, setMonitor] = useState<Monitor | null>(null);
  const [start, setStart] = useState<{ x: number; y: number } | null>(null);
  const [end, setEnd] = useState<{ x: number; y: number } | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    document.body.classList.add("selecting");
    void rpc<Monitor>("selector_info").then(setMonitor);
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") void rpc("cancel_selection");
    };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, []);
  const rect =
    start && end
      ? {
          left: Math.min(start.x, end.x),
          top: Math.min(start.y, end.y),
          width: Math.abs(end.x - start.x),
          height: Math.abs(end.y - start.y),
        }
      : null;
  return (
    <div
      className="selection-screen"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        setStart({ x: e.clientX, y: e.clientY });
        setEnd({ x: e.clientX, y: e.clientY });
      }}
      onPointerMove={(e) => {
        if (start) setEnd({ x: e.clientX, y: e.clientY });
      }}
      onPointerUp={(e) => {
        if (!monitor || !start) return;
        const scale = monitor.scale;
        const x = Math.round(Math.min(start.x, e.clientX) * scale),
          y = Math.round(Math.min(start.y, e.clientY) * scale);
        const width = Math.round(Math.abs(e.clientX - start.x) * scale),
          height = Math.round(Math.abs(e.clientY - start.y) * scale);
        if (width < 10 || height < 10) {
          setError("区域太小，请重新拖动框选");
          setStart(null);
          setEnd(null);
          return;
        }
        void rpc("finish_selection", {
          region: {
            monitor,
            x,
            y,
            width: Math.min(width, monitor.width - x),
            height: Math.min(height, monitor.height - y),
          },
        }).catch((err) => setError(String(err)));
      }}
    >
      <div className="selection-hint">
        拖动框选问题文字区域 · Esc 取消{error && <p>{error}</p>}
      </div>
      {rect && (
        <div className="selection-box" style={rect}>
          <span>
            {Math.round(rect.width * (monitor?.scale ?? 1))} ×{" "}
            {Math.round(rect.height * (monitor?.scale ?? 1))}
          </span>
        </div>
      )}
    </div>
  );
}
