import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export function OverlayWindow() {
  const [currentUrl, setCurrentUrl] = useState<string>("");

  useEffect(() => {
    // Listen for url_loaded event if emitted
    const unlistenPromise = listen<{ url: string }>("url_loaded", (event) => {
      setCurrentUrl(event.payload.url);
    });

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        invoke("dismiss").catch(console.error);
      }
    };
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div
      style={{
        width: "100vw",
        height: "100vh",
        background: "#0f1117",
        color: "#f8fafc",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif",
      }}
    >
      <div
        style={{
          background: "rgba(255, 255, 255, 0.05)",
          border: "1px solid rgba(255, 255, 255, 0.1)",
          borderRadius: "12px",
          padding: "24px 32px",
          textAlign: "center",
          maxWidth: "420px",
        }}
      >
        <div style={{ fontSize: "36px", marginBottom: "12px" }}>🚀</div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: "18px", fontWeight: 600 }}>
          CursorLauncher Overlay
        </h2>
        <p style={{ margin: "0 0 16px 0", fontSize: "13px", color: "#94a3b8" }}>
          {currentUrl ? `Loading ${currentUrl}...` : "Waiting for keyword routing..."}
        </p>
        <div
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
            fontSize: "12px",
            background: "rgba(255, 255, 255, 0.08)",
            padding: "4px 10px",
            borderRadius: "6px",
            color: "#cbd5e1",
          }}
        >
          <span>Press</span>
          <kbd
            style={{
              background: "rgba(255, 255, 255, 0.15)",
              padding: "1px 5px",
              borderRadius: "4px",
              fontFamily: "monospace",
            }}
          >
            Esc
          </kbd>
          <span>to dismiss</span>
        </div>
      </div>
    </div>
  );
}
