import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import { Dialog, Button } from "../common";

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
export default function OnlineIniDialog({ isOpen, onClose }: OnlineIniDialogProps) {
  const [results, setResults] = useState<OnlineIniEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [searching, setSearching] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<string | null>(null);
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
          (Speeduino, rusEFI, epicEFI, FOME). The selected file becomes the
          project's INI.
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
            onClick={refresh}
            disabled={refreshing || searching || downloading !== null}
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
