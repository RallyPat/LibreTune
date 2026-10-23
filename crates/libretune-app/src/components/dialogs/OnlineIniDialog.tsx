import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { RefreshCw, FileUp } from "lucide-react";
import { Dialog, Button } from "../common";
import type { IniEntry } from "../../types/app";

interface OnlineIniEntry {
  source: string;
  name: string;
  signature: string | null;
  download_url: string;
  repo_path: string;
  size: number | null;
}

interface OnlineIniListResponse {
  entries: OnlineIniEntry[];
  last_updated: string | null;
  refreshed: boolean;
}

interface OnlineIniDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called after a user-provided .ini is imported, so the parent can refresh
   *  its INI list (the same callback New Project uses). */
  onIniImported?: (entry: IniEntry) => void;
}

/**
 * On-demand "Search for INI Online" dialog.
 *
 * The signature-mismatch flow already offers online INI search when connecting
 * to an ECU, but only reactively. This exposes the same capability from the
 * menu so a user can browse and download a definition at any time (e.g. before
 * connecting, or in demo mode).
 *
 * The listing is cached on disk by the backend: opening the dialog serves the
 * cache instantly (refreshing in the background only when stale), and the
 * Refresh button forces a network re-scan via `refresh_online_inis`.
 */
export default function OnlineIniDialog({ isOpen, onClose, onIniImported }: OnlineIniDialogProps) {
  const [results, setResults] = useState<OnlineIniEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [searching, setSearching] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [done, setDone] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen && results.length === 0 && !searching) {
      void search();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  async function search() {
    setSearching(true);
    setError(null);
    try {
      // No signature → the backend returns every known online definition.
      // Serves the persisted cache when fresh; a full scan only happens when
      // the cache is missing or older than its TTL.
      const found = await invoke<OnlineIniListResponse>("search_online_inis", {
        signature: null,
      });
      setResults(found.entries);
      setLastUpdated(found.last_updated);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSearching(false);
    }
  }

  async function refresh() {
    setRefreshing(true);
    setError(null);
    setNotice(null);
    try {
      const fresh = await invoke<OnlineIniListResponse>("refresh_online_inis");
      setResults(fresh.entries);
      setLastUpdated(fresh.last_updated);
      if (!fresh.refreshed) {
        setNotice("Refresh failed — showing the cached listing.");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRefreshing(false);
    }
  }

  async function download(entry: OnlineIniEntry) {
    setDownloading(entry.download_url);
    setError(null);
    setNotice(null);
    setDone(null);
    try {
      const path = await invoke<string>("download_ini", {
        downloadUrl: entry.download_url,
        name: entry.name,
        source: entry.source,
      });
      await invoke("update_project_ini", { iniPath: path, forceResync: true });
      setDone(`Loaded ${entry.name} (${entry.source}).`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDownloading(null);
    }
  }

  /** Use an .ini file from the user's computer: import it into the local
   *  repository, then make it the project's INI — the local-file counterpart
   *  of download(). Serves users whose ECU/firmware isn't in the online
   *  sources (custom firmware, vendor-supplied definitions). */
  async function importLocal() {
    setError(null);
    setNotice(null);
    setDone(null);
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: "INI Definition", extensions: ["ini"] }],
    });
    if (!selected || typeof selected !== "string") return; // cancelled
    setImporting(true);
    try {
      const entry = await invoke<IniEntry>("import_ini", { sourcePath: selected });
      await invoke("update_project_ini", { iniPath: entry.path, forceResync: true });
      onIniImported?.(entry);
      setDone(`Loaded ${entry.name} (local file).`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setImporting(false);
    }
  }

  const shown = results.filter((r) => {
    if (!filter.trim()) return true;
    const q = filter.toLowerCase();
    return (
      r.name.toLowerCase().includes(q) ||
      r.source.toLowerCase().includes(q) ||
      (r.signature?.toLowerCase().includes(q) ?? false)
    );
  });

  return (
    <Dialog open={isOpen} onClose={onClose} title="Search for INI Online" size="lg">
      <Dialog.Body>
        <p style={{ marginTop: 0, opacity: 0.8 }}>
          Browse and download ECU definitions from the official online sources
          (Speeduino, rusEFI, epicEFI, FOME), or provide your own .ini file —
          the selected file becomes the project's INI.
        </p>

        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center", marginBottom: "0.25rem" }}>
          <input
            type="text"
            value={filter}
            placeholder="Filter by name, source or signature…"
            onChange={(e) => setFilter(e.target.value)}
            style={{ flex: 1 }}
          />
          <Button
            variant="secondary"
            onClick={importLocal}
            disabled={importing || refreshing || searching || downloading !== null}
            title="Use an .ini definition file from your computer"
          >
            <FileUp size={14} style={{ marginRight: "0.35rem", verticalAlign: "-2px" }} />
            {importing ? "Importing…" : "Use local file…"}
          </Button>
          <Button
            variant="secondary"
            onClick={refresh}
            disabled={refreshing || searching || downloading !== null || importing}
            title="Re-scan all online sources now, ignoring the cache"
          >
            <RefreshCw size={14} style={{ marginRight: "0.35rem", verticalAlign: "-2px" }} className={refreshing ? "icon-spinning" : undefined} />
            {refreshing ? "Refreshing…" : "Refresh"}
          </Button>
        </div>
        <div style={{ opacity: 0.65, fontSize: "0.85em", marginBottom: "0.75rem" }}>
          {lastUpdated
            ? `Listing last updated: ${new Date(lastUpdated).toLocaleString()}`
            : "Listing not cached yet — first load scans all sources."}
        </div>

        {error && (
          <div style={{ color: "var(--color-error, #d33)", marginBottom: "0.5rem" }}>{error}</div>
        )}
        {notice && (
          <div style={{ color: "var(--warning, #b80)", marginBottom: "0.5rem" }}>{notice}</div>
        )}
        {done && (
          <div style={{ color: "var(--color-success, #2a2)", marginBottom: "0.5rem" }}>{done}</div>
        )}

        <div style={{ maxHeight: "50vh", overflowY: "auto" }}>
          {searching && shown.length === 0 ? (
            <div style={{ opacity: 0.7, padding: "1rem" }}>
              Searching online repositories, please wait…
            </div>
          ) : shown.length === 0 ? (
            <div style={{ opacity: 0.7, padding: "1rem" }}>No definitions found.</div>
          ) : (
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr style={{ textAlign: "left", opacity: 0.7 }}>
                  <th style={{ padding: "0.25rem 0.5rem" }}>Name</th>
                  <th style={{ padding: "0.25rem 0.5rem" }}>Source</th>
                  <th style={{ padding: "0.25rem 0.5rem" }} />
                </tr>
              </thead>
              <tbody>
                {shown.map((entry) => (
                  <tr key={entry.download_url} style={{ borderTop: "1px solid var(--color-border, #4443)" }}>
                    <td style={{ padding: "0.25rem 0.5rem", wordBreak: "break-all" }}>{entry.name}</td>
                    <td style={{ padding: "0.25rem 0.5rem" }}>{entry.source}</td>
                    <td style={{ padding: "0.25rem 0.5rem", textAlign: "right" }}>
                      <Button
                        variant="primary"
                        onClick={() => download(entry)}
                        disabled={downloading !== null}
                      >
                        {downloading === entry.download_url ? "Downloading…" : "Download & Use"}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="secondary" onClick={onClose}>
          Close
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
