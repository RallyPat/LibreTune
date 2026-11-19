//! Sign in with ChatGPT for open-source desktop clients.

use axum::{extract::Query, http::StatusCode, routing::get, Router};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use libretune_core::llm::providers::chatgpt::RESOURCE;
use reqwest::{Client, Url};
use ring::{digest, signature};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use tauri_plugin_opener::OpenerExt;
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

const DYNAMIC_CLIENT: &str = "dynamic_agent_client";
const ISSUER: &str = "https://auth.openai.com";
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const CALLBACK: &str = "/auth/callback";

#[derive(Serialize, Deserialize)]
struct Profile {
    client_id: String,
    subject: String,
    email: Option<String>,
    tokens: Option<Session>,
}

#[derive(Serialize, Deserialize)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    refresh_at: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
    token_type: String,
    scope: String,
    expires_in: i64,
    earliest_refresh_at: Option<i64>,
}

impl TokenResponse {
    fn into_session(self) -> Result<Session, String> {
        let now = chrono::Utc::now().timestamp();
        if !self.scope.split_whitespace().any(|s| s == PLAN_SCOPE) {
            return Err("Permission to use your ChatGPT plan was not granted.".into());
        }
        if !self.token_type.eq_ignore_ascii_case("bearer")
            || self.access_token.is_empty()
            || self.refresh_token.is_empty()
            || self.expires_in <= 0
        {
            return Err("OpenAI returned an invalid ChatGPT session.".into());
        }
        Ok(Session {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            expires_at: now.saturating_add(self.expires_in),
            refresh_at: self.earliest_refresh_at.unwrap_or(now),
        })
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Accounts {
    active: Option<String>,
    profiles: Vec<Profile>,
}

impl Accounts {
    fn active(&self) -> Option<&Profile> {
        let id = self.active.as_deref()?;
        self.profiles.iter().find(|profile| profile.client_id == id)
    }

    fn active_mut(&mut self) -> Option<&mut Profile> {
        let id = self.active.as_deref()?;
        self.profiles
            .iter_mut()
            .find(|profile| profile.client_id == id)
    }

    fn status(&self) -> ChatGptStatus {
        ChatGptStatus {
            active: self.active.clone(),
            accounts: self
                .profiles
                .iter()
                .enumerate()
                .map(|(i, profile)| AccountStatus {
                    id: profile.client_id.clone(),
                    label: format!(
                        "{} ({})",
                        profile.email.as_deref().unwrap_or("ChatGPT account"),
                        i + 1
                    ),
                    connected: profile.tokens.is_some(),
                })
                .collect(),
        }
    }
}

struct Store {
    dir: PathBuf,
}

impl Store {
    fn new(app: &tauri::AppHandle) -> Self {
        Self {
            dir: crate::paths::get_app_data_dir(app).join("chatgpt"),
        }
    }

    /// Keep the lock across the exchange and atomic save, including across app processes.
    fn lock(&self) -> Result<File, String> {
        fs::create_dir_all(&self.dir).map_err(storage_error)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.dir.join("session.lock"))
            .map_err(storage_error)?;
        file.try_lock().map_err(|e| format!("Could not lock the ChatGPT session. Retry after the current account operation finishes. ({e})"))?;
        Ok(file)
    }

    fn load(&self) -> Result<Accounts, String> {
        match fs::read(self.dir.join("profile.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(storage_error),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Accounts::default()),
            Err(err) => Err(storage_error(err)),
        }
    }

    fn save(&self, accounts: &Accounts) -> Result<(), String> {
        let bytes = serde_json::to_vec(accounts).map_err(storage_error)?;
        self.write("profile.json", &bytes)
    }

    fn host(&self) -> Result<String, String> {
        match fs::read_to_string(self.dir.join("host-id")) {
            Ok(id) => Ok(id),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let id = format!("urn:uuid:{}", Uuid::new_v4());
                self.write("host-id", id.as_bytes())?;
                Ok(id)
            }
            Err(err) => Err(storage_error(err)),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        // NamedTempFile uses mode 0600 on Unix. On Windows the app data directory
        // inherits the user's profile ACL.
        let mut file = tempfile::NamedTempFile::new_in(&self.dir).map_err(storage_error)?;
        file.write_all(bytes).map_err(storage_error)?;
        file.as_file().sync_all().map_err(storage_error)?;
        file.persist(self.dir.join(Path::new(name)))
            .map_err(storage_error)?;
        Ok(())
    }
}

fn storage_error(error: impl std::fmt::Display) -> String {
    format!("Could not access the local ChatGPT session: {error}")
}

fn oauth_client() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "Could not initialize the ChatGPT HTTP client.".into())
}

async fn exchange(client: &Client, form: &[(&str, &str)]) -> Result<TokenResponse, String> {
    let response = client
        .post(format!("{ISSUER}/api/accounts/oauth/token"))
        .form(form)
        .send()
        .await
        .map_err(|_| "Could not reach ChatGPT authorization. Try again.")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.json::<Value>().await.unwrap_or_default();
        return match body["error"].as_str() {
            Some(
                "invalid_grant"
                | "invalid_refresh_token"
                | "token_expired"
                | "refresh_token_expired"
                | "refresh_token_invalidated"
                | "refresh_token_reused",
            ) => Err("ChatGPT authorization expired or was revoked. Sign in again.".into()),
            _ => Err(format!("ChatGPT authorization failed (HTTP {status}).")),
        };
    }
    response
        .json()
        .await
        .map_err(|_| "OpenAI returned an invalid authorization response.".into())
}

pub(super) async fn access_token(app: &tauri::AppHandle) -> Result<String, String> {
    let store = Store::new(app);
    let _lock = store.lock()?;
    let mut accounts = store.load()?;
    let profile = accounts
        .active_mut()
        .ok_or("Sign in with ChatGPT in AI Assistant settings.")?;
    let session = profile
        .tokens
        .as_ref()
        .ok_or("Sign in with ChatGPT in AI Assistant settings.")?;
    let now = chrono::Utc::now().timestamp();
    if session.expires_at > now + 60 || (session.expires_at > now && session.refresh_at > now) {
        return Ok(session.access_token.clone());
    }
    let next = exchange(
        &oauth_client()?,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &profile.client_id),
            ("refresh_token", &session.refresh_token),
            ("resource", RESOURCE),
        ],
    )
    .await?
    .into_session()?;
    let token = next.access_token.clone();
    profile.tokens = Some(next);
    store.save(&accounts)?;
    Ok(token)
}

fn random_parameter() -> String {
    B64.encode(rand::random::<[u8; 32]>())
}

#[derive(Deserialize)]
struct Callback {
    state: String,
    code: Option<String>,
    client_id: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct Identity {
    iss: String,
    aud: Value,
    exp: i64,
    nonce: String,
    sub: String,
    email: Option<String>,
}

async fn verify_identity(
    client: &Client,
    jwt: &str,
    id: &str,
    expected_nonce: &str,
) -> Result<Identity, String> {
    const INVALID: &str = "OpenAI returned an invalid ChatGPT identity token.";
    let (signed, sig) = jwt.rsplit_once('.').ok_or(INVALID)?;
    let (header, payload) = signed.split_once('.').ok_or(INVALID)?;
    let decode = |s| B64.decode(s).map_err(|_| INVALID);
    let header: Value = serde_json::from_slice(&decode(header)?).map_err(|_| INVALID)?;
    let kid = header["kid"].as_str().ok_or(INVALID)?;
    if header["alg"] != "RS256" {
        return Err(INVALID.into());
    }
    let keys: Value = client
        .get(format!("{ISSUER}/.well-known/jwks.json"))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| "Could not fetch OpenAI signing keys.")?
        .json()
        .await
        .map_err(|_| "Could not read OpenAI signing keys.")?;
    let key = keys["keys"]
        .as_array()
        .and_then(|keys| {
            keys.iter()
                .find(|key| key["kid"] == kid && key["kty"] == "RSA")
        })
        .ok_or(INVALID)?;
    let n = decode(key["n"].as_str().ok_or(INVALID)?)?;
    let e = decode(key["e"].as_str().ok_or(INVALID)?)?;
    signature::RsaPublicKeyComponents { n, e }
        .verify(
            &signature::RSA_PKCS1_2048_8192_SHA256,
            signed.as_bytes(),
            &decode(sig)?,
        )
        .map_err(|_| INVALID)?;
    let claims: Identity = serde_json::from_slice(&decode(payload)?).map_err(|_| INVALID)?;
    let audience_matches = claims.aud == id
        || claims
            .aud
            .as_array()
            .is_some_and(|aud| aud.len() == 1 && aud[0] == id);
    if !audience_matches
        || claims.iss != ISSUER
        || claims.nonce != expected_nonce
        || claims.exp <= chrono::Utc::now().timestamp()
        || claims.sub.is_empty()
    {
        return Err(INVALID.into());
    }
    Ok(claims)
}

#[derive(Serialize)]
struct AccountStatus {
    id: String,
    label: String,
    connected: bool,
}

#[derive(Serialize)]
pub struct ChatGptStatus {
    active: Option<String>,
    accounts: Vec<AccountStatus>,
}

pub(super) fn connected(app: &tauri::AppHandle) -> bool {
    Store::new(app)
        .load()
        .is_ok_and(|accounts| accounts.active().is_some_and(|p| p.tokens.is_some()))
}

#[tauri::command]
pub async fn chatgpt_status(app: tauri::AppHandle) -> Result<ChatGptStatus, String> {
    Ok(Store::new(&app).load()?.status())
}

#[tauri::command]
pub async fn chatgpt_select_account(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<ChatGptStatus, String> {
    let store = Store::new(&app);
    let _lock = store.lock()?;
    let mut accounts = store.load()?;
    if !accounts.profiles.iter().any(|p| p.client_id == account_id) {
        return Err("Unknown ChatGPT account.".into());
    }
    accounts.active = Some(account_id);
    store.save(&accounts)?;
    Ok(accounts.status())
}

#[tauri::command]
pub async fn chatgpt_sign_in(
    app: tauri::AppHandle,
    account_id: Option<String>,
) -> Result<ChatGptStatus, String> {
    let store = Store::new(&app);
    let _lock = store.lock()?;
    let mut accounts = store.load()?;
    let existing = account_id
        .as_ref()
        .map(|id| {
            accounts
                .profiles
                .iter()
                .position(|p| &p.client_id == id)
                .ok_or("Unknown ChatGPT account.")
        })
        .transpose()?;
    let profile = existing.map(|index| &accounts.profiles[index]);
    let saved_id = profile.map(|p| p.client_id.as_str());
    let host = store.host()?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|e| format!("Could not bind the sign-in callback: {e}"))?;
    let redirect = format!(
        "http://{}{}",
        listener.local_addr().map_err(storage_error)?,
        CALLBACK
    );
    let state = random_parameter();
    let nonce = random_parameter();
    let verifier = random_parameter();
    let challenge = B64.encode(digest::digest(&digest::SHA256, verifier.as_bytes()));
    let mut authorize =
        Url::parse(&format!("{ISSUER}/api/accounts/authorize")).map_err(storage_error)?;
    authorize.query_pairs_mut().extend_pairs([
        ("client_id", saved_id.unwrap_or(DYNAMIC_CLIENT)),
        ("ext_agent_host_id", &host),
        ("response_type", "code"),
        ("redirect_uri", &redirect),
        (
            "scope",
            "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct",
        ),
        ("resource", RESOURCE),
        ("state", &state),
        ("nonce", &nonce),
        ("code_challenge_method", "S256"),
        ("code_challenge", &challenge),
    ]);
    if let Some(email) = profile.and_then(|p| p.email.as_deref()) {
        authorize.query_pairs_mut().append_pair("login_hint", email);
    }
    if saved_id.is_none() {
        authorize
            .query_pairs_mut()
            .append_pair("agent_name_hint", "LibreTune");
    }
    let (sender, receiver) = oneshot::channel();
    let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
    let router = Router::new().route(
        CALLBACK,
        get(move |Query(callback): Query<Callback>| {
            let state = state.clone();
            let sender = sender.clone();
            async move {
                if callback.state != state {
                    return (StatusCode::BAD_REQUEST, "Invalid ChatGPT sign-in state.");
                }
                if let Some(tx) = sender.lock().ok().and_then(|mut sender| sender.take()) {
                    let _ = tx.send(callback);
                }
                (
                    StatusCode::OK,
                    "Return to LibreTune to check your ChatGPT sign-in.",
                )
            }
        }),
    );
    app.opener()
        .open_url(authorize.as_str(), None::<&str>)
        .map_err(|_| "Could not open the ChatGPT sign-in page.")?;
    // Graceful shutdown lets the browser receive its response before closing the listener.
    let (shutdown, stop) = oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stop.await;
            })
            .await
    });
    let callback = tokio::time::timeout(Duration::from_secs(180), receiver).await;
    let _ = shutdown.send(());
    // Bound shutdown even when a loopback client is stuck.
    let mut server = server;
    if tokio::time::timeout(Duration::from_secs(1), &mut server)
        .await
        .is_err()
    {
        server.abort();
    }
    let callback = callback
        .map_err(|_| "ChatGPT sign-in timed out.")?
        .map_err(|_| "ChatGPT callback ended before sign-in.")?;
    if callback.error.is_some() {
        return Err("ChatGPT sign-in was declined or could not complete.".into());
    }
    let code = callback
        .code
        .filter(|code| !code.is_empty())
        .ok_or("Missing authorization code.")?;
    let id = match (saved_id, callback.client_id.as_deref()) {
        (Some(saved), None) => saved,
        (Some(saved), Some(returned)) if saved == returned => saved,
        (None, Some(issued)) if issued.starts_with("oaiapp_") => issued,
        _ => return Err("ChatGPT did not return the expected OAuth client ID.".into()),
    };
    let client = oauth_client()?;
    let response = exchange(
        &client,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", id),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    let identity = verify_identity(
        &client,
        response
            .id_token
            .as_deref()
            .ok_or("Missing ChatGPT identity token.")?,
        id,
        &nonce,
    )
    .await?;
    if profile.is_some_and(|p| p.subject != identity.sub) {
        return Err("Choose the previously connected ChatGPT account.".into());
    }
    let profile = Profile {
        client_id: id.into(),
        subject: identity.sub,
        email: identity.email,
        tokens: Some(response.into_session()?),
    };
    accounts.active = Some(profile.client_id.clone());
    if let Some(index) = existing {
        accounts.profiles[index] = profile;
    } else {
        if accounts
            .profiles
            .iter()
            .any(|saved| saved.client_id == profile.client_id)
        {
            return Err(
                "This ChatGPT connection is already registered. Select it in settings.".into(),
            );
        }
        accounts.profiles.push(profile);
    }
    store.save(&accounts)?;
    Ok(accounts.status())
}

#[tauri::command]
pub async fn chatgpt_sign_out(app: tauri::AppHandle) -> Result<bool, String> {
    let store = Store::new(&app);
    let _lock = store.lock()?;
    let mut accounts = store.load()?;
    let Some(profile) = accounts.active_mut() else {
        return Ok(true);
    };
    let Some(session) = profile.tokens.take() else {
        return Ok(true);
    };
    let client = oauth_client()?;
    let form = [
        ("token", session.refresh_token.as_str()),
        ("token_type_hint", "refresh_token"),
        ("client_id", profile.client_id.as_str()),
    ];
    let mut revoked = false;
    for attempt in 0..2 {
        let result = client
            .post(format!("{ISSUER}/api/accounts/oauth/revoke"))
            .form(&form)
            .send()
            .await;
        match result {
            Ok(response) if response.status() == StatusCode::OK => {
                revoked = true;
                break;
            }
            Ok(response) if !response.status().is_server_error() => break,
            _ if attempt == 0 => tokio::time::sleep(Duration::from_millis(300)).await,
            _ => {}
        }
    }
    store.save(&accounts)?;
    Ok(revoked)
}

#[derive(Deserialize, Serialize)]
pub struct PlanModel {
    slug: String,
    display_name: String,
    #[serde(skip_serializing)]
    visibility: String,
}

#[tauri::command]
pub async fn chatgpt_models(app: tauri::AppHandle) -> Result<Vec<PlanModel>, String> {
    #[derive(Deserialize)]
    struct Catalog {
        models: Vec<PlanModel>,
    }
    let token = access_token(&app).await?;
    let result = oauth_client()?
        .get(format!("{RESOURCE}/models"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Could not reach the ChatGPT model catalog.")?;
    if !result.status().is_success() {
        return Err(format!(
            "ChatGPT could not list models (HTTP {}). Check your connection and plan permissions.",
            result.status()
        ));
    }
    let catalog: Catalog = result
        .json()
        .await
        .map_err(|_| "OpenAI returned an invalid ChatGPT model catalog.")?;
    Ok(catalog
        .models
        .into_iter()
        .filter(|model| model.visibility == "list")
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_only_grant_cannot_be_used_for_plan_requests() {
        let response = TokenResponse {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            id_token: None,
            token_type: "Bearer".into(),
            scope: "openid profile resource.invoke".into(),
            expires_in: 3600,
            earliest_refresh_at: None,
        };
        assert!(response.into_session().is_err());
    }

    #[test]
    fn store_keeps_host_id_and_locks_session_operations() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            dir: dir.path().into(),
        };
        let lock = store.lock().unwrap();
        assert!(store.lock().is_err());
        let host = store.host().unwrap();
        store.save(&Accounts::default()).unwrap();
        assert_eq!(store.host().unwrap(), host);
        assert!(store.load().unwrap().profiles.is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.path().join("profile.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        drop(lock);
        assert!(store.lock().is_ok());
    }
}
