//! Online INI Repository
//!
//! Provides functionality to search for and download INI files from the
//! official per-platform definition sources:
//!
//! - **Speeduino** — GitHub `reference/` (tracks master) plus the `.ini`
//!   assets attached to each tagged firmware release on GitHub Releases
//!   (what released firmware actually matches).
//! - **rusEFI** — `https://rusefi.com/online/ini/rusefi/`, the same Apache
//!   directory index the rusEFI console downloads from. Every published
//!   bundle is addressable as
//!   `{branch}/{year}/{month}/{day}/{board}/{hash}.ini`, and the firmware
//!   signature encodes exactly those components.
//! - **epicEFI** — `https://content.epicefi.com/firmware/ini/`, a rusEFI
//!   white-label using the identical layout.
//! - **FOME** — GitHub `firmware/tunerstudio/generated/` (the per-board
//!   generated definitions; the parent directory only holds fragments).
//!
//! ## Usage
//!
//! ```ignore
//! let online = OnlineIniRepository::new();
//! let results = online.search(Some("rusEFI master.2026.06.07.proteus_f4.753206531")).await?;
//! for entry in results {
//!     println!("{}: {:?}", entry.name, entry.signature);
//! }
//! online.download(&results[0], "./definitions/").await?;
//! ```

use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// Information about an online INI file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineIniEntry {
    /// Source repository (speeduino, rusefi, etc.)
    pub source: IniSource,
    /// Display name
    pub name: String,
    /// Firmware signature (if known). For autoindex bundles this is
    /// reconstructed from the path components; for release assets it is the
    /// release tag.
    pub signature: Option<String>,
    /// Direct download URL
    pub download_url: String,
    /// Path within the source repository / bundle tree
    pub repo_path: String,
    /// File size in bytes (if known)
    pub size: Option<u64>,
}

/// Known INI sources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IniSource {
    Speeduino,
    RusEFI,
    /// FOME — a rusEFI fork; its per-board TunerStudio INIs are generated
    /// under `FOME-Tech/fome-fw/firmware/tunerstudio/generated`.
    Fome,
    /// epicEFI — a rusEFI white-label for epicECU boards. Publishes its
    /// TunerStudio INIs on `content.epicefi.com` in the same
    /// `{branch}/{year}/{month}/{day}/{board}/{hash}.ini` layout as rusEFI,
    /// with `epicEFI`-branded signatures.
    EpicEFI,
    Custom,
}

impl IniSource {
    /// Every source the online search fetches from, in priority order.
    ///
    /// `Custom` is intentionally excluded — it has no upstream URL and is only
    /// used to tag user-imported files. Add new upstream platforms here (and
    /// give them a listing endpoint below) to widen auto-discovery coverage.
    pub fn online_sources() -> &'static [IniSource] {
        &[
            IniSource::Speeduino,
            IniSource::RusEFI,
            IniSource::EpicEFI,
            IniSource::Fome,
        ]
    }

    /// Root of the Apache autoindex tree for platforms that publish their
    /// TunerStudio INIs as
    /// `{root}/{branch}/{year}/{month}/{day}/{board}/{hash}.ini`.
    ///
    /// - rusEFI: the same server the rusEFI console itself downloads from
    ///   (their `RealIniFileProvider` resolves this exact URL shape).
    /// - epicEFI: `content.epicefi.com`, identical layout.
    pub fn autoindex_root(&self) -> Option<&'static str> {
        match self {
            IniSource::RusEFI => Some("https://rusefi.com/online/ini/rusefi"),
            IniSource::EpicEFI => Some("https://content.epicefi.com/firmware/ini"),
            _ => None,
        }
    }

    /// GitHub contents-API URL listing a flat directory of `.ini` files.
    pub fn github_contents_url(&self) -> Option<&'static str> {
        match self {
            // The .ini used to live under reference/tunerstudio/, which no
            // longer exists; the single canonical definition is
            // reference/speeduino.ini (master-tracking).
            IniSource::Speeduino => {
                Some("https://api.github.com/repos/speeduino/speeduino/contents/reference")
            }
            // The real per-board definitions are generated into this
            // subdirectory; the parent only holds fragment INIs (menus,
            // gauges) that are not loadable definitions.
            IniSource::Fome => Some(
                "https://api.github.com/repos/FOME-Tech/fome-fw/contents/firmware/tunerstudio/generated",
            ),
            _ => None,
        }
    }

    /// GitHub releases API URL for platforms that attach `.ini` assets to
    /// firmware releases. Speeduino tags every release (e.g. `202501.7`)
    /// with the matching `speeduino.ini` — that is the definition users
    /// running released firmware need, since `reference/` tracks master.
    pub fn github_releases_url(&self) -> Option<&'static str> {
        match self {
            IniSource::Speeduino => {
                Some("https://api.github.com/repos/speeduino/speeduino/releases?per_page=30")
            }
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            IniSource::Speeduino => "Speeduino",
            IniSource::RusEFI => "rusEFI",
            IniSource::Fome => "FOME",
            IniSource::EpicEFI => "epicEFI",
            IniSource::Custom => "Custom",
        }
    }
}

/// A parsed `rusEFI`-style firmware signature:
/// `{brand} {branch}.{year}.{month}.{day}.{board}.{hash}`.
///
/// This is the same format rusEFI's own tooling parses (`SignatureHelper`
/// and `upload_ini.sh`), and the online-INI URL is derivable directly from
/// it as `{autoindex_root}/{branch}/{year}/{month}/{day}/{board}/{hash}.ini`.
/// epicEFI is a white-label rusEFI build and uses the identical layout with
/// the `epicEFI` brand word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSignature {
    pub branch: String,
    pub year: String,
    pub month: String,
    pub day: String,
    pub board: String,
    pub hash: String,
}

impl BundleSignature {
    /// Parse the dot-separated signature body after the brand word.
    /// Returns `None` for signatures that don't follow the 6-part format
    /// (Speeduino, MegaSquirt, ...).
    pub fn parse(signature: &str) -> Option<Self> {
        let (_, rest) = signature.split_once(' ')?;
        let parts: Vec<&str> = rest.trim().split('.').collect();
        if parts.len() != 6 {
            return None;
        }
        let [branch, year, month, day, board, hash] =
            [parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]];
        // Upstream (upload_ini.sh) allows alnum, '-' and '_' in the free-form
        // components, and zero-padded digits for the date components.
        let token = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        };
        let numeric = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit());
        if !token(branch) || !token(board) || !token(hash) {
            return None;
        }
        if !numeric(year, 4) || !numeric(month, 2) || !numeric(day, 2) {
            return None;
        }
        Some(BundleSignature {
            branch: branch.to_string(),
            year: year.to_string(),
            month: month.to_string(),
            day: day.to_string(),
            board: board.to_string(),
            hash: hash.to_string(),
        })
    }

    /// URL of this bundle's INI under an autoindex root.
    pub fn ini_url(&self, root: &str) -> String {
        format!(
            "{}/{}/{}/{}/{}/{}/{}.ini",
            root, self.branch, self.year, self.month, self.day, self.board, self.hash
        )
    }
}

/// GitHub API response for directory listing
#[derive(Debug, Deserialize)]
struct GitHubFile {
    name: String,
    path: String,
    size: Option<u64>,
    download_url: Option<String>,
    #[serde(rename = "type")]
    file_type: String,
}

/// GitHub API response for a release
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

/// GitHub API response for a release asset
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: Option<u64>,
}

/// Render a reqwest error together with its full `source()` chain — the
/// outer Display ("error sending request for url (...)") hides whether the
/// cause was DNS, connect, TLS or a timeout.
fn error_chain(e: &reqwest::Error) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = write!(out, "{e}");
    let mut cur = std::error::Error::source(e);
    while let Some(c) = cur {
        let _ = write!(out, ": {c}");
        cur = c.source();
    }
    out
}

/// One link extracted from an Apache `mod_autoindex` directory listing.
#[derive(Debug, Clone)]
struct AutoindexLink {
    /// href exactly as it appeared in the HTML (`board/`, `1234567890.ini`)
    href: String,
    is_dir: bool,
}

/// Extract relative directory and `.ini` file links from an Apache
/// autoindex page. Drops parent links, sort links (`?C=N;O=D`), icons and
/// off-site URLs.
fn parse_autoindex_links(html: &str) -> Vec<AutoindexLink> {
    let marker = "href=\"";
    let mut out = Vec::new();
    let mut cursor = 0;
    while let Some(found) = html[cursor..].find(marker) {
        let start = cursor + found + marker.len();
        let Some(end) = html[start..].find('"') else {
            break;
        };
        let href = html[start..start + end].to_string();
        cursor = start + end;
        if href.starts_with("..") || href.contains("://") || href.contains('?') {
            continue;
        }
        if let Some(name) = href.strip_suffix('/') {
            if !name.is_empty() {
                out.push(AutoindexLink { href, is_dir: true });
            }
        } else if href.to_ascii_lowercase().ends_with(".ini") {
            out.push(AutoindexLink {
                href,
                is_dir: false,
            });
        }
    }
    out
}

/// The lexicographically-greatest sub-directory link. Autoindex date
/// directories are zero-padded, so this is the newest date available.
fn newest_dir(links: &[AutoindexLink]) -> Option<String> {
    links
        .iter()
        .filter(|l| l.is_dir)
        .map(|l| l.href.clone())
        .max()
}

/// Join an autoindex href onto a base URL. Relative hrefs (`board/`,
/// `123.ini`) append to the base; root-relative hrefs (`/icons/...`)
/// resolve against the host root.
fn join_url(base: &str, href: &str) -> String {
    let base = base.trim_end_matches('/');
    if href.starts_with('/') {
        // Root-relative: keep only scheme://host from the base.
        let root = match (
            base.find("://"),
            base[base.find("://").unwrap() + 3..].find('/'),
        ) {
            (Some(scheme_end), Some(host_slash)) => &base[..scheme_end + 3 + host_slash],
            _ => base,
        };
        format!("{root}{href}")
    } else {
        format!("{base}/{href}")
    }
}

/// Lowercased alphanumeric tokens of length >= 2 — the identity vocabulary
/// used to match online file names against ECU signatures.
fn identity_tokens(s: &str) -> std::collections::HashSet<String> {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// Online listings can't carry the INI's real signature without downloading
/// every file, so an entry matches when all of its file-name identity tokens
/// (board, hash, version — e.g. `proteus_f4-1739931529.ini`) appear in the
/// ECU signature's tokens.
fn entry_matches_signature(
    entry: &OnlineIniEntry,
    sig_tokens: &std::collections::HashSet<String>,
) -> bool {
    let stem = entry.name.strip_suffix(".ini").unwrap_or(&entry.name);
    let stem_tokens = identity_tokens(stem);
    !stem_tokens.is_empty() && stem_tokens.iter().all(|t| sig_tokens.contains(t))
}

/// Build an entry for a bundle described by a parsed path / signature. The
/// file name inside a board directory is the bundle hash.
fn autoindex_entry(
    source: IniSource,
    root: &str,
    bundle: &BundleSignature,
) -> Option<OnlineIniEntry> {
    if bundle.hash.is_empty() {
        return None;
    }
    let repo_path = format!(
        "{}/{}/{}/{}/{}/{}.ini",
        bundle.branch, bundle.year, bundle.month, bundle.day, bundle.board, bundle.hash
    );
    Some(OnlineIniEntry {
        source,
        name: format!("{}-{}.ini", bundle.board, bundle.hash),
        signature: Some(format!(
            "{} {}.{}.{}.{}.{}.{}",
            source.display_name(),
            bundle.branch,
            bundle.year,
            bundle.month,
            bundle.day,
            bundle.board,
            bundle.hash
        )),
        download_url: format!("{root}/{repo_path}"),
        repo_path,
        size: None,
    })
}

/// How long (seconds) a cached online-INI listing stays fresh before the
/// next search refreshes it from the network. rusEFI and epicEFI publish new
/// bundles daily, so a day matches the fastest-moving upstream; longer TTLs
/// would leave brand-new firmware undiscoverable.
pub const ONLINE_INI_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// On-disk representation of the cached listing (`online_ini_cache.json`).
#[derive(Debug, Serialize, Deserialize)]
pub struct OnlineIniCacheFile {
    /// Bump when the format changes; loaders reject other versions.
    pub version: u32,
    /// RFC 3339 timestamp of the last successful network refresh.
    pub last_updated: String,
    /// The cached listing entries.
    pub entries: Vec<OnlineIniEntry>,
}

impl OnlineIniCacheFile {
    /// Version of the cache format this build writes and reads.
    pub const CURRENT_VERSION: u32 = 1;
}

/// Online INI repository client
pub struct OnlineIniRepository {
    /// HTTP client for API requests
    client: reqwest::Client,
    /// Cache of known INI entries (signature -> entry)
    cache: Vec<OnlineIniEntry>,
    /// When the cache was last refreshed from the network (None = never).
    last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl OnlineIniRepository {
    /// Create a new online repository client
    pub fn new() -> Self {
        // Ignore any inherited proxy configuration and pin HTTP/1.1: some
        // GUI-process environments and CDNs reject the request otherwise
        // (observed as a spurious 400 on rusEFI's online-INI host, while the
        // exact same request over HTTP/1.1 with no proxy returns 200).
        let client = reqwest::Client::builder()
            .user_agent("LibreTune/0.1")
            .no_proxy()
            .http1_only()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        OnlineIniRepository {
            client,
            cache: Vec::new(),
            last_updated: None,
        }
    }

    /// Search for INI files matching a signature
    ///
    /// If signature is `None`, returns every INI found in the newest
    /// published bundles of every source. With a signature, returns
    /// (a) cache entries whose file-name identity tokens (board, hash,
    /// version) all appear in the signature, plus (b) for `rusEFI`-style
    /// 6-part signatures, the exact bundle INI derived directly from the
    /// signature's own path components — older firmware included, no
    /// directory walk needed.
    pub async fn search(
        &mut self,
        signature: Option<&str>,
    ) -> Result<Vec<OnlineIniEntry>, io::Error> {
        if self.cache.is_empty() {
            self.refresh_cache().await?;
        }

        let Some(sig) = signature else {
            return Ok(self.cache.clone());
        };

        let sig_tokens = identity_tokens(sig);
        let mut results: Vec<OnlineIniEntry> = self
            .cache
            .iter()
            .filter(|e| entry_matches_signature(e, &sig_tokens))
            .cloned()
            .collect();

        for entry in self.fetch_signature_derived_entries(sig).await {
            if !results.iter().any(|e| e.download_url == entry.download_url) {
                results.push(entry);
            }
        }

        Ok(results)
    }

    /// Refresh the cache by fetching INI lists from all sources.
    ///
    /// Builds the new listing off to the side and only swaps it in when at
    /// least one source answered — a total failure (e.g. offline) keeps the
    /// previous cache and its timestamp intact instead of wiping it.
    async fn refresh_cache(&mut self) -> Result<(), io::Error> {
        let mut fresh = Vec::new();
        let mut failures = Vec::new();

        for &source in IniSource::online_sources() {
            match self.list_source(source).await {
                Ok(entries) => fresh.extend(entries),
                Err(e) => {
                    eprintln!("Warning: Failed to fetch INIs from {source:?}: {e}");
                    failures.push((source, e));
                }
            }
        }

        if fresh.is_empty() {
            let detail = failures
                .iter()
                .map(|(s, e)| format!("{s:?}: {e}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(io::Error::other(format!(
                "No online INI source responded ({detail})"
            )));
        }
        if !failures.is_empty() {
            eprintln!(
                "Warning: online INI refresh incomplete, {} of {} sources failed",
                failures.len(),
                IniSource::online_sources().len()
            );
        }

        self.cache = fresh;
        self.last_updated = Some(chrono::Utc::now());
        Ok(())
    }

    /// Force a network refresh of the cached listing (see [`Self::refresh_cache`]).
    pub async fn refresh(&mut self) -> Result<(), io::Error> {
        self.refresh_cache().await
    }

    /// The cached listing entries.
    pub fn entries(&self) -> &[OnlineIniEntry] {
        &self.cache
    }

    /// When the cache was last refreshed (None = never refreshed).
    pub fn last_updated(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_updated
    }

    /// `last_updated` formatted as RFC 3339, for display in the frontend.
    pub fn last_updated_rfc3339(&self) -> Option<String> {
        self.last_updated.map(|t| t.to_rfc3339())
    }

    /// True when the cache is empty or older than [`ONLINE_INI_CACHE_TTL_SECS`].
    pub fn is_stale(&self) -> bool {
        match self.last_updated {
            None => true,
            Some(t) => {
                self.cache.is_empty()
                    || (chrono::Utc::now() - t).num_seconds() > ONLINE_INI_CACHE_TTL_SECS as i64
            }
        }
    }

    /// Load a previously saved cache file. Returns `Ok(false)` when the file
    /// does not exist yet (first run); a corrupt or unsupported file is an
    /// error. Loading a stale-but-present cache is fine — callers decide
    /// whether to refresh.
    pub fn load_cache(&mut self, path: &Path) -> io::Result<bool> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };
        let file: OnlineIniCacheFile = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if file.version != OnlineIniCacheFile::CURRENT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported cache version {} (expected {})",
                    file.version,
                    OnlineIniCacheFile::CURRENT_VERSION
                ),
            ));
        }
        let parsed = chrono::DateTime::parse_from_rfc3339(&file.last_updated)
            .map(|t| t.with_timezone(&chrono::Utc))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.cache = file.entries;
        self.last_updated = Some(parsed);
        Ok(true)
    }

    /// Persist the cache so the next session starts without a network scan.
    pub fn save_cache(&self, path: &Path) -> io::Result<()> {
        let file = OnlineIniCacheFile {
            version: OnlineIniCacheFile::CURRENT_VERSION,
            last_updated: self
                .last_updated
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
            entries: self.cache.clone(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, bytes)
    }

    /// Test hook: set the cache timestamp without sleeping.
    #[cfg(test)]
    fn set_last_updated_for_test(&mut self, t: chrono::DateTime<chrono::Utc>) {
        self.last_updated = Some(t);
    }

    /// Fetch the current INI list from a single source (no caching). Exposed
    /// for targeted refresh and diagnostics.
    pub async fn list_source(&self, source: IniSource) -> io::Result<Vec<OnlineIniEntry>> {
        match source {
            IniSource::Speeduino => self.fetch_speeduino().await,
            IniSource::RusEFI | IniSource::EpicEFI => self.fetch_autoindex_latest(source).await,
            IniSource::Fome => self.fetch_github_contents(source).await,
            IniSource::Custom => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Custom source has no upstream listing",
            )),
        }
    }

    /// Speeduino lists both the master-tracking `reference/` INI and the INI
    /// assets attached to tagged firmware releases.
    async fn fetch_speeduino(&self) -> io::Result<Vec<OnlineIniEntry>> {
        let mut entries = self.fetch_github_contents(IniSource::Speeduino).await?;
        entries.extend(
            self.fetch_github_release_assets(IniSource::Speeduino)
                .await?,
        );
        Ok(entries)
    }

    /// Fetch the INI list from a GitHub contents-API directory
    async fn fetch_github_contents(&self, source: IniSource) -> io::Result<Vec<OnlineIniEntry>> {
        let api_url = source.github_contents_url().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "No GitHub contents URL for source",
            )
        })?;

        let response = self
            .client
            .get(api_url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if !response.status().is_success() {
            return Err(io::Error::other(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        let files: Vec<GitHubFile> = response
            .json()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut entries = Vec::new();
        for file in files {
            // Only include .ini files
            if file.file_type == "file" && file.name.to_ascii_lowercase().ends_with(".ini") {
                if let Some(download_url) = file.download_url {
                    entries.push(OnlineIniEntry {
                        source,
                        name: file.name,
                        signature: None, // Would need to download to get signature
                        download_url,
                        repo_path: file.path,
                        size: file.size,
                    });
                }
            }
        }

        Ok(entries)
    }

    /// Fetch `.ini` assets attached to GitHub releases.
    async fn fetch_github_release_assets(
        &self,
        source: IniSource,
    ) -> io::Result<Vec<OnlineIniEntry>> {
        let api_url = source.github_releases_url().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "No GitHub releases URL for source",
            )
        })?;

        let response = self
            .client
            .get(api_url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        if !response.status().is_success() {
            return Err(io::Error::other(format!(
                "GitHub API error: {}",
                response.status()
            )));
        }

        let releases: Vec<GitHubRelease> = response
            .json()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut entries = Vec::new();
        for release in releases {
            if release.draft {
                continue;
            }
            for asset in release.assets {
                if !asset.name.to_ascii_lowercase().ends_with(".ini") {
                    continue;
                }
                entries.push(OnlineIniEntry {
                    source,
                    name: format!(
                        "{}-{}.ini",
                        source.display_name().to_lowercase(),
                        release.tag_name
                    ),
                    // Release-tagged INIs match firmware of the same tag.
                    signature: Some(format!("{} {}", source.display_name(), release.tag_name)),
                    download_url: asset.browser_download_url,
                    repo_path: format!("releases/{}", release.tag_name),
                    size: asset.size,
                });
            }
        }

        Ok(entries)
    }

    /// Fetch the link list of an Apache autoindex page.
    async fn fetch_autoindex_links(&self, url: &str) -> io::Result<Vec<AutoindexLink>> {
        // Always fetch directory indexes with a trailing slash: some hosts
        // answer the slashless form with a 301 whose Location is an
        // *internal* address (epicEFI's nginx behind Docker redirects to
        // http://172.18.0.60/...), which the client then follows into an
        // unroutable black hole until the timeout fires.
        let url = if url.ends_with('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };
        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| io::Error::other(error_chain(&e)))?;
        if !response.status().is_success() {
            return Err(io::Error::other(format!(
                "Autoindex error: {} for {url}",
                response.status()
            )));
        }
        let html = response
            .text()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(parse_autoindex_links(&html))
    }

    /// Walk an autoindex tree to its newest day directory and list every
    /// board's INI: `{root}/{branch}/{year}/{month}/{day}/{board}/{hash}.ini`.
    /// Directory names are zero-padded dates, so the lexicographically newest
    /// link is the chronologically newest.
    async fn fetch_autoindex_latest(&self, source: IniSource) -> io::Result<Vec<OnlineIniEntry>> {
        let root = source.autoindex_root().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Not an autoindex source")
        })?;

        let root_links = self.fetch_autoindex_links(root).await?;
        let branch = root_links
            .iter()
            .filter(|l| l.is_dir)
            .find(|l| l.href == "master/")
            .map(|l| l.href.clone())
            .or_else(|| newest_dir(&root_links))
            .ok_or_else(|| io::Error::other(format!("No branch directory under {root}")))?;
        let branch_url = join_url(root, &branch);

        let year = newest_dir(&self.fetch_autoindex_links(&branch_url).await?)
            .ok_or_else(|| io::Error::other(format!("No year directory under {branch_url}")))?;
        let year_url = join_url(&branch_url, &year);

        let month = newest_dir(&self.fetch_autoindex_links(&year_url).await?)
            .ok_or_else(|| io::Error::other(format!("No month directory under {year_url}")))?;
        let month_url = join_url(&year_url, &month);

        let day = newest_dir(&self.fetch_autoindex_links(&month_url).await?)
            .ok_or_else(|| io::Error::other(format!("No day directory under {month_url}")))?;
        let day_url = join_url(&month_url, &day);

        let day_links = self.fetch_autoindex_links(&day_url).await?;
        let boards: Vec<String> = day_links
            .iter()
            .filter(|l| l.is_dir)
            .map(|l| l.href.clone())
            .collect();

        // One request per board (~40-50 per day); run them concurrently but
        // capped, to keep the browse dialog fast without hammering the host.
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
        let client = self.client.clone();
        let mut set = tokio::task::JoinSet::new();
        for board in boards {
            let client = client.clone();
            let semaphore = semaphore.clone();
            let board_url = join_url(&day_url, &board);
            let board_name = board.trim_end_matches('/').to_string();
            let root = root.to_string();
            let branch = branch.trim_end_matches('/').to_string();
            let year = year.trim_end_matches('/').to_string();
            let month = month.trim_end_matches('/').to_string();
            let day = day.trim_end_matches('/').to_string();
            set.spawn(async move {
                let _permit = semaphore.acquire_owned().await;
                let mut entries = Vec::new();
                if let Ok(resp) = client
                    .get(&board_url)
                    .timeout(std::time::Duration::from_secs(15))
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(html) = resp.text().await {
                            for link in parse_autoindex_links(&html) {
                                if link.is_dir {
                                    continue;
                                }
                                let Some(hash) = link.href.strip_suffix(".ini") else {
                                    continue;
                                };
                                let bundle = BundleSignature {
                                    branch: branch.clone(),
                                    year: year.clone(),
                                    month: month.clone(),
                                    day: day.clone(),
                                    board: board_name.clone(),
                                    hash: hash.to_string(),
                                };
                                if let Some(entry) = autoindex_entry(source, &root, &bundle) {
                                    entries.push(entry);
                                }
                            }
                        }
                    }
                }
                entries
            });
        }

        let mut out = Vec::new();
        while let Some(joined) = set.join_next().await {
            if let Ok(entries) = joined {
                out.extend(entries);
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Derive download entries directly from a `rusEFI`-style signature.
    ///
    /// The signature encodes the exact bundle path, so any firmware — not
    /// just the newest day the browse walk covers — resolves with a single
    /// existence probe per platform root.
    async fn fetch_signature_derived_entries(&self, signature: &str) -> Vec<OnlineIniEntry> {
        let Some(bundle) = BundleSignature::parse(signature) else {
            return Vec::new();
        };
        // The brand word usually identifies the platform, but the same board
        // can exist on both, so probe the other root too when the first miss.
        let lower = signature.to_ascii_lowercase();
        let ordered = if lower.starts_with("epicefi") {
            [IniSource::EpicEFI, IniSource::RusEFI]
        } else {
            [IniSource::RusEFI, IniSource::EpicEFI]
        };
        for source in ordered {
            let Some(root) = source.autoindex_root() else {
                continue;
            };
            let url = bundle.ini_url(root);
            if self.url_exists(&url).await {
                return autoindex_entry(source, root, &bundle).into_iter().collect();
            }
        }
        Vec::new()
    }

    /// Probe whether a URL exists (HEAD, short timeout).
    async fn url_exists(&self, url: &str) -> bool {
        match self
            .client
            .head(url)
            .timeout(std::time::Duration::from_secs(8))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Download an INI file to the specified directory
    ///
    /// Returns the path to the downloaded file.
    pub async fn download(
        &self,
        entry: &OnlineIniEntry,
        target_dir: &Path,
    ) -> Result<std::path::PathBuf, io::Error> {
        eprintln!("[online-ini] downloading {}", entry.download_url);
        let response = self
            .client
            .get(&entry.download_url)
            .header(reqwest::header::ACCEPT, "*/*")
            .send()
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            eprintln!(
                "[online-ini] {} -> {} body: {}",
                entry.download_url,
                status,
                body.chars().take(300).collect::<String>()
            );
            return Err(io::Error::other(format!("Download failed: {}", status)));
        }

        let content = response
            .bytes()
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Create target directory if it doesn't exist
        std::fs::create_dir_all(target_dir)?;

        // Generate unique filename
        let filename = format!(
            "{}_{}",
            entry.source.display_name().to_lowercase(),
            entry.name
        );
        let target_path = target_dir.join(&filename);

        std::fs::write(&target_path, &content)?;

        Ok(target_path)
    }

    /// Check if we have internet connectivity
    pub async fn check_connectivity(&self) -> bool {
        // Try to reach GitHub
        match self
            .client
            .head("https://api.github.com")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 403, // 403 = rate limited but reachable
            Err(_) => false,
        }
    }
}

impl Default for OnlineIniRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ini_source_endpoints() {
        assert!(IniSource::Speeduino.github_contents_url().is_some());
        assert!(IniSource::Speeduino.github_releases_url().is_some());
        assert!(IniSource::Fome.github_contents_url().is_some());
        assert!(IniSource::RusEFI.autoindex_root().is_some());
        assert!(IniSource::EpicEFI.autoindex_root().is_some());
        assert!(IniSource::Custom.autoindex_root().is_none());
        assert!(IniSource::Custom.github_contents_url().is_none());
        assert!(IniSource::Custom.github_releases_url().is_none());
    }

    #[test]
    fn test_online_sources_have_a_listing_endpoint() {
        // Every source the search iterates over must have at least one
        // listing endpoint and must not be the Custom (no-upstream) tag.
        for &source in IniSource::online_sources() {
            assert_ne!(source, IniSource::Custom);
            let endpoints = source.autoindex_root().is_some() as usize
                + source.github_contents_url().is_some() as usize
                + source.github_releases_url().is_some() as usize;
            assert!(endpoints >= 1, "{source:?} has no listing endpoint");
        }
    }

    #[test]
    fn test_epicefi_is_an_online_source() {
        assert!(IniSource::online_sources().contains(&IniSource::EpicEFI));
        assert_eq!(IniSource::EpicEFI.display_name(), "epicEFI");
    }

    #[test]
    fn test_fome_is_an_online_source() {
        assert!(IniSource::online_sources().contains(&IniSource::Fome));
        assert_eq!(IniSource::Fome.display_name(), "FOME");
    }

    fn sample_entry() -> OnlineIniEntry {
        OnlineIniEntry {
            source: IniSource::Speeduino,
            name: "speeduino.ini".to_string(),
            signature: None,
            download_url: "https://example.com/speeduino.ini".to_string(),
            repo_path: "reference/speeduino.ini".to_string(),
            size: Some(1234),
        }
    }

    #[test]
    fn test_cache_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("online_ini_cache.json");

        let mut repo = OnlineIniRepository::new();
        // Missing file: Ok(false), repo stays empty and stale.
        assert!(!repo.load_cache(&path).unwrap());
        assert!(repo.is_stale());
        assert!(repo.entries().is_empty());

        repo.cache = vec![sample_entry()];
        repo.last_updated = Some(chrono::Utc::now());
        repo.save_cache(&path).unwrap();

        let mut loaded = OnlineIniRepository::new();
        assert!(loaded.load_cache(&path).unwrap());
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].name, "speeduino.ini");
        assert!(!loaded.is_stale());
        assert!(loaded.last_updated().is_some());

        // A corrupt file must be an error, not silently ignored.
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(loaded.load_cache(&path).is_err());
    }

    #[test]
    fn test_cache_staleness_by_age() {
        let mut repo = OnlineIniRepository::new();
        repo.cache = vec![sample_entry()];

        // Older than the TTL -> stale.
        let old =
            chrono::Utc::now() - chrono::Duration::seconds(ONLINE_INI_CACHE_TTL_SECS as i64 + 60);
        repo.set_last_updated_for_test(old);
        assert!(repo.is_stale());

        // Fresh timestamp with non-empty entries -> not stale.
        repo.set_last_updated_for_test(chrono::Utc::now());
        assert!(!repo.is_stale());

        // An empty cache is always stale, whatever the timestamp says.
        repo.cache.clear();
        assert!(repo.is_stale());
    }

    #[test]
    fn test_bundle_signature_parse_and_url() {
        let sig =
            BundleSignature::parse("rusEFI master.2026.06.07.purple-gateway.753206531").unwrap();
        assert_eq!(sig.branch, "master");
        assert_eq!(sig.year, "2026");
        assert_eq!(sig.month, "06");
        assert_eq!(sig.day, "07");
        assert_eq!(sig.board, "purple-gateway");
        assert_eq!(sig.hash, "753206531");
        assert_eq!(
            sig.ini_url("https://rusefi.com/online/ini/rusefi"),
            "https://rusefi.com/online/ini/rusefi/master/2026/06/07/purple-gateway/753206531.ini"
        );

        // epicEFI white-label signatures use the same layout.
        let epic = BundleSignature::parse("epicEFI master.2026.08.26.epicECU.4128885531").unwrap();
        assert_eq!(epic.board, "epicECU");
        assert_eq!(
            epic.ini_url("https://content.epicefi.com/firmware/ini"),
            "https://content.epicefi.com/firmware/ini/master/2026/08/26/epicECU/4128885531.ini"
        );

        // Non-bundle signatures must not parse.
        assert!(BundleSignature::parse("Speeduino 202501.7").is_none());
        assert!(BundleSignature::parse("rusEFI master.2026.06").is_none());
        assert!(BundleSignature::parse("rusEFI master.202X.06.07.board.123").is_none());
        assert!(BundleSignature::parse("nospaces").is_none());
    }

    #[test]
    fn test_autoindex_link_parsing() {
        let html = concat!(
            "<html><body><h1>Index of /firmware/ini/master/2026/08/26</h1>\n",
            "<pre><img src=\"/icons/blank.gif\" alt=\"[ICO]\">",
            "<a href=\"../\">Parent Directory</a>",
            "<a href=\"2025/\">2025/</a>",
            "<a href=\"2026/\">2026/</a>",
            "<a href=\"epicECU/\">epicECU/</a>",
            "<a href=\"4128885531.ini\">4128885531.ini</a>",
            "<a href=\"/spicons/folder.gif\">icon</a>",
            "<a href=\"?C=N;O=D\">Name</a>",
            "<a href=\"readme.md\">readme.md</a></pre></body></html>"
        );
        let links = parse_autoindex_links(html);
        let dirs: Vec<&str> = links
            .iter()
            .filter(|l| l.is_dir)
            .map(|l| l.href.as_str())
            .collect();
        assert_eq!(dirs, vec!["2025/", "2026/", "epicECU/"]);
        let files: Vec<&str> = links
            .iter()
            .filter(|l| !l.is_dir)
            .map(|l| l.href.as_str())
            .collect();
        assert_eq!(files, vec!["4128885531.ini"]);
        // Zero-padded dates: lexicographically newest == chronologically newest.
        assert_eq!(newest_dir(&links).as_deref(), Some("epicECU/"));
    }

    #[test]
    fn test_join_url() {
        assert_eq!(
            join_url("https://example.com/ini", "master/"),
            "https://example.com/ini/master/"
        );
        assert_eq!(
            join_url("https://example.com/ini/", "123.ini"),
            "https://example.com/ini/123.ini"
        );
        assert_eq!(
            join_url("https://example.com/ini", "/abs/path.ini"),
            "https://example.com/abs/path.ini"
        );
    }

    #[test]
    fn test_entry_matches_signature_by_name_tokens() {
        let entry = |name: &str| OnlineIniEntry {
            source: IniSource::RusEFI,
            name: name.to_string(),
            signature: None,
            download_url: String::new(),
            repo_path: String::new(),
            size: None,
        };

        let proteus_sig = identity_tokens("rusEFI master.2026.08.26.proteus_f4.1739931529");
        assert!(entry_matches_signature(
            &entry("proteus_f4-1739931529.ini"),
            &proteus_sig
        ));
        // Wrong board or wrong hash must not match.
        assert!(!entry_matches_signature(
            &entry("mre_f4-1739931529.ini"),
            &proteus_sig
        ));
        assert!(!entry_matches_signature(
            &entry("proteus_f4-999999999.ini"),
            &proteus_sig
        ));

        let speeduino_sig = identity_tokens("Speeduino 202501.7");
        assert!(entry_matches_signature(
            &entry("speeduino-202501.7.ini"),
            &speeduino_sig
        ));
        // The master-tracking definition matches any Speeduino (dev INI).
        assert!(entry_matches_signature(
            &entry("speeduino.ini"),
            &speeduino_sig
        ));
        assert!(!entry_matches_signature(
            &entry("fome_proteus_f4.ini"),
            &speeduino_sig
        ));
    }

    #[tokio::test]
    #[ignore = "live network call"]
    async fn live_search_derives_rusefi_bundle_url() {
        let mut repo = OnlineIniRepository::new();
        // Real bundle published on rusefi.com (verified 2026-08-26).
        let results = repo
            .search(Some("rusEFI master.2026.08.26.proteus_f4.1739931529"))
            .await
            .unwrap();
        assert!(
            results.iter().any(|e| e
                .download_url
                .ends_with("/master/2026/08/26/proteus_f4/1739931529.ini")),
            "{results:?}"
        );
    }

    #[tokio::test]
    #[ignore = "live network call"]
    async fn live_browse_lists_all_sources() {
        let mut repo = OnlineIniRepository::new();
        let results = repo.search(None).await.unwrap();
        assert!(results.iter().any(|e| e.source == IniSource::RusEFI));
        assert!(results.iter().any(|e| e.source == IniSource::EpicEFI));
        assert!(results.iter().any(|e| e.source == IniSource::Speeduino));
        assert!(results.iter().any(|e| e.source == IniSource::Fome));
    }

    #[tokio::test]
    #[ignore = "live network call"]
    async fn live_refresh_then_cache_roundtrip_serves_next_instance() {
        // Mirrors the app's ensure-cache flow: refresh from the network,
        // persist, then a fresh repository instance loads the cache and is
        // not stale (no second scan needed).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("online_ini_cache.json");

        let mut first = OnlineIniRepository::new();
        assert!(first.is_stale());
        first.refresh().await.unwrap();
        assert!(!first.is_stale());
        assert!(!first.entries().is_empty());
        assert!(first.last_updated().is_some());
        first.save_cache(&path).unwrap();

        let mut second = OnlineIniRepository::new();
        assert!(second.load_cache(&path).unwrap());
        assert!(!second.is_stale());
        assert_eq!(second.entries().len(), first.entries().len());
    }
}
