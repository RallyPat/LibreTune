import React, { useState, useEffect } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import PopOutWindow from "./PopOutWindow";
import { LoadingProvider } from "./components/LoadingContext";
import { ToastProvider } from "./components/ToastContext";

/**
 * Root component that determines whether to render App or PopOutWindow
 * based on the URL hash. Uses useEffect to ensure the hash is available
 * after the DOM is ready (fixes timing issue in Tauri WebviewWindow).
 */
function Root() {
  const [isPopOut, setIsPopOut] = useState<boolean | null>(null);
  const [debugInfo, setDebugInfo] = useState<string>('initializing...');

  useEffect(() => {
    const checkHash = () => {
      const hash = window.location.hash;
      const href = window.location.href;
      const info = `hash="${hash}" href="${href}"`;
      console.log('[main.tsx] hash check:', info);
      setDebugInfo(info);
      setIsPopOut(hash.startsWith('#/popout'));
    };

    // Check immediately on mount
    checkHash();

    // Also listen for hash changes (for robustness)
    window.addEventListener('hashchange', checkHash);
    return () => window.removeEventListener('hashchange', checkHash);
  }, []);

  // Show debug info while determining which component to render
  if (isPopOut === null) {
    return (
      <div style={{ padding: 20, color: 'white', background: '#333' }}>
        <h2>Loading... (isPopOut = null)</h2>
        <p>Debug: {debugInfo}</p>
      </div>
    );
  }

  // Show what we're rendering
  console.log('[main.tsx] Rendering:', isPopOut ? 'PopOutWindow' : 'App');

  return isPopOut ? <PopOutWindow /> : <App />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LoadingProvider>
      <ToastProvider>
        <Root />
      </ToastProvider>
    </LoadingProvider>
  </React.StrictMode>,
);
