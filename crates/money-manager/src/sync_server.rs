//! Connecting a ledger through a Tresse server's sign-in page.
//!
//! The app opens the server's page in the browser ([`SyncConnect::begin`]);
//! the user signs in there and the page sends the repository's config string
//! back through a `moneymanager://tresse-connect` link, which Android delivers
//! as a deep link and `install` parks in [`LINKS`]. A poller matches the link
//! against the pending attempt; only a matching `state` and server are
//! accepted, so a crafted link cannot hand credentials to the app.
//!
//! The returned string never carries an encryption key: a new repository gets
//! one generated on this device, an existing one keeps the key of the remote
//! it replaces. The server never sees the key.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui_kit::{App, Global};
use money_core::remote::{
    KeyChoice, RemoteConfig, StorageType, choose_key, connect_url, generate_encryption_key,
    marks_new_repository,
};

use crate::settings::Settings;

/// Where the server page sends the config string back. The host tells a
/// connect link apart from any other use of the scheme.
pub const CONNECT_ACTION: &str = "tresse-connect";
const CONNECT_SCHEME: &str = "moneymanager";

/// How long a started connection waits for the browser to come back.
const ATTEMPT_TTL: Duration = Duration::from_secs(30 * 60);

/// How often the pending attempt is matched against links received meanwhile.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// The Tresse server this build offers first, baked in at build time from
/// `MONEY_MANAGER_TRESSE_SERVER_URL`. None when the build names none: the
/// server URL field is then the only way to one.
pub fn default_server_url() -> Option<&'static str> {
    option_env!("MONEY_MANAGER_TRESSE_SERVER_URL")
        .map(str::trim)
        .filter(|url| !url.is_empty())
}

/// A connection the user started from the app; only its own return is
/// accepted. Kept while the app runs and in the settings across restarts:
/// Android may kill the app while the browser is in front.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectAttempt {
    pub server_url: String,
    pub state: String,
    expires: Instant,
}

impl ConnectAttempt {
    fn new(server_url: String) -> Self {
        // 32 hex chars, like the Obsidian plugin's state.
        let state = uuid::Uuid::new_v4().simple().to_string();
        ConnectAttempt {
            server_url,
            state,
            expires: Instant::now() + ATTEMPT_TTL,
        }
    }

    pub(crate) fn restore(server_url: String, state: String, expires_millis: u64) -> Option<Self> {
        let left = expires_millis as i128 - now_millis() as i128;
        if left <= 0 {
            return None;
        }
        Some(ConnectAttempt {
            server_url,
            state,
            expires: Instant::now() + Duration::from_millis(left as u64),
        })
    }

    pub fn expired(&self) -> bool {
        self.expires <= Instant::now()
    }

    /// What [`restore`] reads back, persisted across restarts.
    pub fn expires_millis(&self) -> u64 {
        now_millis() + self.expires.saturating_duration_since(Instant::now()).as_millis() as u64
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// The pending attempt and its outcome, shared between the settings form, the
/// first-run form and the poller. UI thread only; the raw links live in
/// [`LINKS`] because the deep-link callback is not on it.
pub struct SyncConnect {
    attempt: Option<ConnectAttempt>,
    outcome: Option<Result<RemoteConfig, String>>,
}

impl Global for SyncConnect {}

impl SyncConnect {
    /// Installs the global with the attempt the settings kept across a
    /// restart, and starts the poller that matches links against it.
    pub fn install(cx: &mut App) {
        let attempt = Settings::global(cx).pending_connect();
        cx.set_global(SyncConnect {
            attempt,
            outcome: None,
        });
        spawn_poller(cx);
    }

    pub fn attempt(cx: &App) -> Option<ConnectAttempt> {
        cx.global::<Self>().attempt.clone()
    }

    pub fn waiting(cx: &App) -> bool {
        let state = cx.global::<Self>();
        state.attempt.is_some() && state.outcome.is_none()
    }

    /// Starts an attempt and answers the page to open in the browser.
    pub fn begin(cx: &mut App, server_url: &str) -> Result<String, String> {
        let server = normalize_server_url(server_url)?;
        let attempt = ConnectAttempt::new(server);
        let return_to = format!("{CONNECT_SCHEME}://{CONNECT_ACTION}?state={}", attempt.state);
        let page = connect_url(&attempt.server_url, Some(&return_to));
        let state = cx.global_mut::<Self>();
        state.attempt = Some(attempt.clone());
        state.outcome = None;
        Settings::update(cx, |settings| settings.set_pending_connect(Some(&attempt)));
        Ok(page)
    }

    /// Drops a started attempt: the browser tab is left, its return is not
    /// accepted any more.
    pub fn cancel(cx: &mut App) {
        Self::clear(cx, None);
    }

    /// The validated remote (or the refusal), for the form that waits; taken
    /// once. Reads before writing: `global_mut` notifies this global's
    /// observers, and they call back here — an unconditional `take` would
    /// notify forever.
    pub fn take_outcome(cx: &mut App) -> Option<Result<RemoteConfig, String>> {
        if cx.global::<Self>().outcome.is_none() {
            return None;
        }
        cx.global_mut::<Self>().outcome.take()
    }

    fn clear(cx: &mut App, outcome: Option<Result<RemoteConfig, String>>) {
        let state = cx.global_mut::<Self>();
        state.attempt = None;
        state.outcome = outcome;
        Settings::update(cx, |settings| settings.set_pending_connect(None));
    }
}

fn spawn_poller(cx: &mut App) {
    // The loop ends with the app: the executor drops it at shutdown.
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            cx.update(|cx| {
                let Some(attempt) = SyncConnect::attempt(cx) else {
                    return;
                };
                if attempt.expired() {
                    SyncConnect::clear(
                        cx,
                        Some(Err(
                            "The connection timed out. Start again from the app.".into(),
                        )),
                    );
                    return;
                }
                let Some(link) = take_link_for(&attempt) else {
                    return;
                };
                let previous = Settings::global(cx).remote().ok().flatten();
                let outcome = accept(&link, &attempt, previous.as_ref());
                SyncConnect::clear(cx, Some(outcome));
            });
        }
    })
    .detach();
}

/// Links the deep-link handler parked, waiting for an attempt to match.
/// Android's handler fills these; on a desktop the queue stays empty and the
/// server's page is used through its Copy button and a pasted string instead.
static LINKS: Mutex<VecDeque<(Instant, String)>> = Mutex::new(VecDeque::new());

#[allow(dead_code)]
fn push_link(url: &str) {
    if !url.starts_with(CONNECT_SCHEME) {
        return;
    }
    let mut links = LINKS.lock().unwrap();
    if links.len() >= 8 {
        links.pop_front();
    }
    links.push_back((Instant::now(), url.to_string()));
}

/// The first received link that belongs to `attempt`; anything stale or from
/// another attempt is dropped, so the queue never grows and a foreign link
/// cannot sit waiting for a later attempt.
fn take_link_for(attempt: &ConnectAttempt) -> Option<String> {
    let mut links = LINKS.lock().unwrap();
    let mut found: Option<String> = None;
    while let Some((at, url)) = links.pop_front() {
        if at.elapsed() > ATTEMPT_TTL {
            continue;
        }
        if found.is_none() && link_belongs_to(&url, attempt) {
            found = Some(url);
        } else {
            links.push_back((at, url));
        }
    }
    found
}

fn link_belongs_to(link: &str, attempt: &ConnectAttempt) -> bool {
    link_params(link)
        .map(|params| {
            params
                .get("state")
                .is_some_and(|state| state == &attempt.state)
        })
        .unwrap_or(false)
}

/// Applies a returned or pasted config string: the key it carries, or one
/// generated here for a new repository, or the key of the remote it
/// replaces — and a check of the whole remote.
fn finish_import(
    text: &str,
    previous: Option<&RemoteConfig>,
    parse_error: &str,
) -> Result<RemoteConfig, String> {
    let mut remote = RemoteConfig::from_share_string(text).map_err(|_| parse_error.to_string())?;
    match choose_key(&remote, marks_new_repository(text), previous)
        .map_err(|error| error.to_string())?
    {
        KeyChoice::Carried => {}
        KeyChoice::Keep(key) => remote.encryption_key = key,
        KeyChoice::Generate => remote.encryption_key = generate_encryption_key()?,
    }
    remote.validate()?;
    Ok(remote)
}

/// Validates a link the server page sent back against the attempt that opened
/// it, and answers the remote it describes.
pub fn accept(
    link: &str,
    attempt: &ConnectAttempt,
    previous: Option<&RemoteConfig>,
) -> Result<RemoteConfig, String> {
    if attempt.expired() {
        return Err("The connection timed out. Start again from the app.".into());
    }
    let params = link_params(link)?;
    if params.get("state") != Some(&attempt.state) {
        return Err("This link does not belong to a connection started here.".into());
    }
    let config = params
        .get("config")
        .ok_or_else(|| "The server link carried no configuration.".to_string())?;
    let remote = finish_import(config, previous, "The server link carried no Tresse config string.")?;
    if remote.storage_type != StorageType::Http
        || normalize_server_url(&remote.url).ok().as_deref() != Some(attempt.server_url.as_str())
    {
        return Err("The link points at another server than the one opened.".into());
    }
    Ok(remote)
}

/// Imports a config string pasted into the app, from the server page's Copy
/// button or another device.
pub fn import_share_string(text: &str, previous: Option<&RemoteConfig>) -> Result<RemoteConfig, String> {
    finish_import(
        text,
        previous,
        "That is not a config string. It starts with tresse://",
    )
}

fn normalize_server_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Server URL is required.".into());
    }
    if trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    if trimmed.starts_with("http://") {
        let host = trimmed["http://".len()..]
            .split(['/', ':'])
            .next()
            .unwrap_or_default();
        if matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
            return Ok(trimmed.to_string());
        }
    }
    Err("The server must use https (http only for localhost).".into())
}

/// Splits `moneymanager://tresse-connect?state=…&config=…` into its
/// percent-decoded parameters.
fn link_params(link: &str) -> Result<HashMap<String, String>, String> {
    let body = link
        .strip_prefix(CONNECT_SCHEME)
        .and_then(|rest| rest.strip_prefix("://"))
        .ok_or_else(|| "Not a Money Manager link.".to_string())?;
    let (host, query) = body
        .split_once('?')
        .ok_or_else(|| "The link carries no parameters.".to_string())?;
    if host != CONNECT_ACTION {
        return Err("Not a server-connect link.".to_string());
    }
    let mut params = HashMap::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair
            .split_once('=')
            .filter(|(key, _)| !key.is_empty())
            .ok_or_else(|| "The link is malformed.".to_string())?;
        params.insert(key.to_string(), percent_decode(value)?);
    }
    Ok(params)
}

/// Decodes what the page's `encodeURIComponent` produced. `+` stays literal:
/// that encoding never produces it, and reading it as space would corrupt a
/// token that legitimately holds one.
fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| "The link is malformed.".to_string())?;
            out.push(hex_value(hex[0])? << 4 | hex_value(hex[1])?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "The link is not valid UTF-8.".to_string())
}

fn hex_value(byte: u8) -> Result<u8, String> {
    (byte as char)
        .to_digit(16)
        .map(|digit| digit as u8)
        .ok_or_else(|| "The link is malformed.".to_string())
}

#[cfg(target_os = "android")]
pub fn install_deep_links() {
    gpui_mobile::packages::deeplink::set_deep_link_handler(push_link);
    if let Ok(Some(url)) = gpui_mobile::packages::deeplink::get_initial_link() {
        push_link(&url);
    }
}

#[cfg(test)]
pub(crate) fn push_link_for_test(url: &str) {
    push_link(url);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt() -> ConnectAttempt {
        ConnectAttempt {
            server_url: "https://sync.example".into(),
            state: "abc123".into(),
            expires: Instant::now() + Duration::from_secs(60),
        }
    }

    fn returned(state: &str, config: &str) -> String {
        format!(
            "moneymanager://tresse-connect?state={state}&config={}",
            encode_component(config)
        )
    }

    fn encode_component(value: &str) -> String {
        let mut out = String::new();
        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
                out.push(byte as char);
            } else {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
        out
    }

    #[test]
    fn a_returned_link_opens_the_repository_it_describes() {
        let attempt = attempt();
        let link = returned(
            "abc123",
            "tresse://http?url=https://sync.example&repo_id=money&token=trs_a&new=1",
        );

        let remote = accept(&link, &attempt, None).unwrap();

        assert_eq!(remote.url, "https://sync.example");
        assert_eq!(remote.repo_id, "money");
        assert_eq!(remote.token, "trs_a");
        // A new repository: the key is generated here, never carried.
        assert!(!remote.encryption_key.is_empty());
    }

    #[test]
    fn an_existing_repository_keeps_the_key_of_the_remote_it_replaces() {
        let attempt = attempt();
        let mut previous = RemoteConfig {
            url: "https://sync.example".into(),
            repo_id: "money".into(),
            encryption_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
            ..Default::default()
        };
        previous.storage_type = StorageType::Http;
        let link = returned(
            "abc123",
            "tresse://http?url=https://sync.example&repo_id=money&token=trs_b",
        );

        let remote = accept(&link, &attempt, Some(&previous)).unwrap();

        assert_eq!(remote.token, "trs_b");
        assert_eq!(
            remote.encryption_key,
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        );
    }

    #[test]
    fn a_link_from_another_start_or_server_is_refused() {
        let attempt = attempt();
        let wrong_state = returned(
            "zzz",
            "tresse://http?url=https://sync.example&repo_id=money&token=trs_a&new=1",
        );
        assert!(accept(&wrong_state, &attempt, None).is_err());

        let other_server = returned(
            "abc123",
            "tresse://http?url=https://evil.example&repo_id=money&token=trs_a&new=1",
        );
        assert!(accept(&other_server, &attempt, None).is_err());
    }

    #[test]
    fn only_this_apps_connect_links_are_matched() {
        let attempt = attempt();
        assert!(!link_belongs_to("moneymanager://other?state=abc123", &attempt));
        assert!(!link_belongs_to("https://sync.example/?state=abc123", &attempt));
    }

    #[test]
    fn link_params_decode_what_the_page_encoded() {
        let link = format!(
            "moneymanager://tresse-connect?state=abc123&config={}",
            encode_component("tresse://http?url=https://sync.example/&token=a%b&repo_id=мой")
        );

        let params = link_params(&link).unwrap();

        assert_eq!(
            params["config"],
            "tresse://http?url=https://sync.example/&token=a%b&repo_id=мой"
        );
    }

    #[test]
    fn a_pasted_new_repository_string_gets_a_generated_key() {
        let remote =
            import_share_string("tresse://http?url=https://sync.example&repo_id=money&token=trs_a&new=1", None)
                .unwrap();

        assert_eq!(remote.storage_type, StorageType::Http);
        assert!(!remote.encryption_key.is_empty());
    }

    #[test]
    fn url_normalization_accepts_https_and_loopback_http_only() {
        assert_eq!(normalize_server_url(" https://sync.example/ ").unwrap(), "https://sync.example");
        assert_eq!(normalize_server_url("http://127.0.0.1:9000/").unwrap(), "http://127.0.0.1:9000");
        assert!(normalize_server_url("http://sync.example").is_err());
        assert!(normalize_server_url("").is_err());
    }

    #[test]
    fn a_restored_attempt_keeps_its_expiry() {
        let fresh = ConnectAttempt::restore("https://sync.example".into(), "s".into(), now_millis() + 1000);
        assert!(fresh.is_some_and(|attempt| !attempt.expired()));
        assert!(ConnectAttempt::restore("https://sync.example".into(), "s".into(), now_millis() - 1).is_none());
    }
}
