import { useEffect, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type { SignatureMismatchInfo } from "../components/dialogs/SignatureMismatchDialog";
import type { TuneMismatchInfo } from "../components/dialogs/TuneMismatchDialog";

export interface BackendEventListenerDeps {
  setSignatureMismatchInfo: (info: SignatureMismatchInfo) => void;
  setSignatureMismatchOpen: (open: boolean) => void;
  setMigrationReportOpen: (open: boolean) => void;
  setTuneMismatchInfo: (info: TuneMismatchInfo) => void;
  setTuneMismatchOpen: (open: boolean) => void;
  checkStatus: () => void | Promise<void>;
}

/**
 * Registers simple backend event listeners that have minimal/no dependencies on
 * mutable component state. Each listener is registered once on mount and
 * unregistered on unmount.
 */
export function useBackendEventListeners(deps: BackendEventListenerDeps): void {
  const {
    setSignatureMismatchInfo,
    setSignatureMismatchOpen,
    setMigrationReportOpen,
    setTuneMismatchInfo,
    setTuneMismatchOpen,
    checkStatus,
  } = deps;

  // `checkStatus` is a plain (non-memoized) function on the App side that is
  // recreated every render. Hold the latest one in a ref (mirroring
  // useAutoConnect.ts) so the definition:loaded listener effect below doesn't
  // need it in its deps and isn't torn down/re-registered on every render.
  const checkStatusRef = useRef(checkStatus);
  checkStatusRef.current = checkStatus;

  // Listen for signature mismatch events
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<SignatureMismatchInfo>("signature:mismatch", (event) => {
          console.log("Signature mismatch detected:", event.payload);
          setSignatureMismatchInfo(event.payload);
          setSignatureMismatchOpen(true);
        });
      } catch (e) {
        console.error("Failed to listen for signature:mismatch events:", e);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [setSignatureMismatchInfo, setSignatureMismatchOpen]);

  // Listen for migration needed events from backend
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen("tune:migration_needed", () => {
          console.log("Tune migration needed - opening dialog");
          setMigrationReportOpen(true);
        });
      } catch (e) {
        console.error("Failed to listen for tune:migration_needed events:", e);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [setMigrationReportOpen]);

  // Listen for definition:loaded event to ensure INI is ready before table operations
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<{
          signature: string;
          tables: number;
          curves: number;
          dialogs: number;
          constants: number;
        }>("definition:loaded", (event) => {
          console.log("[App] definition:loaded event:", event.payload);
          checkStatusRef.current();
        });
      } catch (e) {
        console.error("Failed to listen for definition:loaded events:", e);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Listen for tune mismatch events (after ECU sync)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<TuneMismatchInfo>("tune:mismatch", (event) => {
          console.log("Tune mismatch detected:", event.payload);
          setTuneMismatchInfo(event.payload);
          setTuneMismatchOpen(true);
        });
      } catch (e) {
        console.error("Failed to listen for tune:mismatch events:", e);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [setTuneMismatchInfo, setTuneMismatchOpen]);
}
