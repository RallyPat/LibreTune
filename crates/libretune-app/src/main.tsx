import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LoadingProvider } from "./components/LoadingContext";
import { ToastProvider } from "./components/ToastContext";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LoadingProvider>
      <ToastProvider>
        <App />
      </ToastProvider>
    </LoadingProvider>
  </React.StrictMode>,
);
