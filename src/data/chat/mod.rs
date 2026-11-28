//! Chat client: one background worker owns the keys, talks to the server and publishes
//! snapshots for the UI. Nothing here runs on the render thread.

pub mod crypto;

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use anyhow::Context;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};

use crate::app::i18n::Language;
use crate::app::text::sanitize;
use crypto::{KEY_LEN, from_hex, to_hex};

const THREAD_POLL: Duration = Duration::from_secs(10);
const IDLE_POLL: Duration = Duration::from_secs(60);
const PAGE: usize = 100;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Conversation {
    pub id: i64,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    pub role: String,
    pub members: Vec<String>,
    #[serde(default)]
    pub unread: i64,
}

impl Conversation {
    pub fn is_group(&self) -> bool {
        self.kind == "group"
    }

    /// Group title, or the other person's name.
    pub fn name<'a>(&'a self, me: &'a str) -> &'a str {
        match &self.title {
            Some(title) => title,
            None => self.members.iter().find(|m| *m != me).map_or(me, String::as_str),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    pub id: i64,
    pub sender: String,
    pub mine: bool,
    /// `None` while this console has no key for it.
    pub text: Option<String>,
    pub edited: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Thread {
    pub conversation: Conversation,
    pub lines: Vec<Line>,
    /// (username, key fingerprint) of the other members.
    pub fingerprints: Vec<(String, String)>,
    /// Someone's key differs from the one this console saw before.
    pub key_changed: Option<String>,
    pub waiting_for_key: bool,
}

#[derive(Debug, Clone)]
pub enum Notice {
    Error(String),
    InviteCode(String),
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub loaded: bool,
    pub inbox: Vec<Conversation>,
    pub thread: Option<Thread>,
    pub notice: Option<Notice>,
    pub my_fingerprint: String,
}

#[derive(Debug)]
pub enum Command {
    /// Chat screen opened or closed; closed means only the unread counter is polled.
    Visible(bool),
    Open(i64),
    CloseThread,
    Send(String),
    Edit(i64, String),
    Direct(String),
    NewGroup(String),
    Join(String),
    Invite,
    AddMember(String),
    RemoveMember(String),
    Rename(String),
    Leave,
    Block(String),
    Unblock(String),
}

pub struct Client {
    pub username: String,
    commands: mpsc::UnboundedSender<Command>,
    snapshots: watch::Receiver<Snapshot>,
}

impl Client {
    /// The worker stops by itself once this handle is dropped.
    pub fn start(username: String, lang: Language, notify: bool) -> Self {
        let (commands, rx) = mpsc::unbounded_channel();
        let (tx, snapshots) = watch::channel(Snapshot::default());
        let worker = Worker::new(username.clone(), lang, notify, tx);
        tokio::spawn(worker.run(rx));
        Self { username, commands, snapshots }
    }

    /// A handle with a fixed snapshot and no worker.
    #[cfg(test)]
    pub fn fixed(username: &str, snapshot: Snapshot) -> Self {
        let (commands, _) = mpsc::unbounded_channel();
        let (_, snapshots) = watch::channel(snapshot);
        Self { username: username.into(), commands, snapshots }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn snapshot(&self) -> watch::Ref<'_, Snapshot> {
        self.snapshots.borrow()
    }
}

#[derive(Deserialize)]
struct RawMessage {
    id: i64,
    sender_id: Option<i64>,
    sender_username: Option<String>,
    ciphertext: String,
    nonce: String,
    key_version: i64,
    #[serde(default)]
    edited_at: Option<String>,
    change_seq: i64,
}

#[derive(Deserialize)]
struct KeyInfo {
    version: i64,
    rekey_needed: bool,
    keys: Vec<MyKey>,
    members: Vec<Member>,
}

#[derive(Deserialize)]
struct MyKey {
    version: i64,
    wrapped_key: String,
}

#[derive(Deserialize)]
struct Member {
    user_id: i64,
    username: String,
    public_key: Option<String>,
    has_current_key: bool,
}

impl Member {
    fn key(&self) -> Option<[u8; KEY_LEN]> {
        from_hex(self.public_key.as_deref()?)?.try_into().ok()
    }
}

#[derive(Default)]
struct OpenThread {
    id: i64,
    cursor: i64,
    messages: BTreeMap<i64, RawMessage>,
    keys: HashMap<i64, [u8; KEY_LEN]>,
    fingerprints: Vec<(String, String)>,
    key_changed: Option<String>,
    waiting_for_key: bool,
}

#[derive(Debug)]
struct Status(u16, String);

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}
impl std::error::Error for Status {}

async fn call(method: Method, path: &str, body: Option<Value>) -> anyhow::Result<Value> {
    let mut request = crate::data::api::request(method, path).timeout(Duration::from_secs(20));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body: Value = response.json().await.unwrap_or_default();
        let message = body["error"].as_str().map_or_else(|| format!("HTTP {status}"), str::to_owned);
        return Err(Status(status.as_u16(), message).into());
    }
    Ok(response.json().await?)
}

fn encode(segment: &str) -> String {
    percent_encoding::utf8_percent_encode(segment.trim(), percent_encoding::NON_ALPHANUMERIC).to_string()
}

fn data_file(name: &str, username: &str) -> std::path::PathBuf {
    let user = to_hex(username.to_lowercase().as_bytes());
    std::path::Path::new("ux0:data/vitaforge").join(format!("chat_{name}_{user}"))
}

/// The private key never leaves the console. It is only regenerated when no file exists:
/// replacing a key that merely failed to read would orphan the whole history.
fn load_secret(username: &str) -> anyhow::Result<[u8; KEY_LEN]> {
    let path = data_file("key", username);
    match std::fs::read(&path) {
        Ok(bytes) => bytes.try_into().map_err(|_| anyhow::anyhow!("chat key file is damaged")),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let secret = crypto::random::<KEY_LEN>()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, secret).context("couldn't save the chat key")?;
            Ok(secret)
        }
        Err(err) => Err(err).context("couldn't read the chat key"),
    }
}

struct Worker {
    me: String,
    my_id: i64,
    secret: Option<[u8; KEY_LEN]>,
    lang: Language,
    notify: bool,
    visible: bool,
    unread: i64,
    pins: HashMap<String, String>,
    thread: Option<OpenThread>,
    state: Snapshot,
    out: watch::Sender<Snapshot>,
}

impl Worker {
    fn new(me: String, lang: Language, notify: bool, out: watch::Sender<Snapshot>) -> Self {
        Self {
            me,
            my_id: 0,
            secret: None,
            lang,
            notify,
            visible: false,
            unread: 0,
            pins: HashMap::new(),
            thread: None,
            state: Snapshot::default(),
            out,
        }
    }

    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        if let Ok(bytes) = std::fs::read(data_file("pins", &self.me)) {
            self.pins = serde_json::from_slice(&bytes).unwrap_or_default();
        }
        let mut next = Instant::now() + Duration::from_secs(5);
        loop {
            let wait = next.saturating_duration_since(Instant::now());
            match tokio::time::timeout(wait, commands.recv()).await {
                Ok(None) => return,
                Ok(Some(command)) => {
                    self.state.notice = None;
                    let result = match self.handle(command).await {
                        Ok(()) if self.visible => self.refresh().await,
                        other => other,
                    };
                    if let Err(err) = result {
                        self.state.notice = Some(Notice::Error(format!("{err:#}")));
                    }
                }
                Err(_) => {
                    let result = if self.visible { self.refresh().await } else { self.check_unread().await };
                    if let Err(err) = result {
                        crate::install::log_file(&format!("chat poll failed: {err:#}"));
                    }
                }
            }
            let interval = if self.visible && self.thread.is_some() { THREAD_POLL } else { IDLE_POLL };
            next = Instant::now() + interval;
            let _ = self.out.send(self.state.clone());
        }
    }

    async fn check_unread(&mut self) -> anyhow::Result<()> {
        let total = call(Method::GET, "/me/unread", None).await?["total"].as_i64().unwrap_or(0);
        if total > self.unread && self.notify {
            // Generic on purpose: message text never goes to the system notification list.
            crate::install::notify::send(&self.lang.tr("chat-notify"));
        }
        self.unread = total;
        Ok(())
    }

    /// Publishes this console's public key before anything that needs it.
    async fn ensure_key(&mut self) -> anyhow::Result<[u8; KEY_LEN]> {
        if let Some(secret) = self.secret {
            return Ok(secret);
        }
        let secret = load_secret(&self.me)?;
        let public = crypto::public_key(&secret);
        call(Method::PUT, "/me/key", Some(json!({ "public_key": to_hex(&public) }))).await?;
        self.state.my_fingerprint = crypto::fingerprint(&public);
        self.secret = Some(secret);
        Ok(secret)
    }

    async fn handle(&mut self, command: Command) -> anyhow::Result<()> {
        match command {
            Command::Visible(visible) => {
                self.visible = visible;
                if !visible {
                    self.thread = None;
                    self.state.thread = None;
                }
            }
            Command::Open(id) => self.open(id),
            Command::CloseThread => {
                self.thread = None;
                self.state.thread = None;
            }
            Command::Send(text) => self.publish(None, &text).await?,
            Command::Edit(id, text) => self.publish(Some(id), &text).await?,
            Command::Direct(user) => {
                self.ensure_key().await?;
                let reply = call(Method::POST, &format!("/me/conversations/direct/{}", encode(&user)), None).await?;
                self.open(reply["id"].as_i64().context("bad server reply")?);
            }
            Command::NewGroup(title) => {
                self.ensure_key().await?;
                let reply = call(Method::POST, "/me/conversations/group", Some(json!({ "title": title }))).await?;
                self.open(reply["id"].as_i64().context("bad server reply")?);
            }
            Command::Join(code) => {
                self.ensure_key().await?;
                let reply = call(Method::POST, &format!("/me/invites/{}/join", encode(&code)), None).await?;
                self.open(reply["id"].as_i64().context("bad server reply")?);
            }
            Command::Invite => {
                let id = self.thread_id()?;
                let reply = call(Method::POST, &format!("/me/conversations/{id}/invites"), None).await?;
                let code = reply["code"].as_str().context("bad server reply")?.to_owned();
                self.state.notice = Some(Notice::InviteCode(code));
            }
            Command::AddMember(user) => {
                let id = self.thread_id()?;
                let body = json!({ "usernames": [user.trim()] });
                call(Method::POST, &format!("/me/conversations/{id}/members"), Some(body)).await?;
            }
            Command::RemoveMember(user) => {
                let id = self.thread_id()?;
                call(Method::DELETE, &format!("/me/conversations/{id}/members/{}", encode(&user)), None).await?;
            }
            Command::Rename(title) => {
                let id = self.thread_id()?;
                call(Method::PATCH, &format!("/me/conversations/{id}"), Some(json!({ "title": title }))).await?;
            }
            Command::Leave => {
                let id = self.thread_id()?;
                call(Method::DELETE, &format!("/me/conversations/{id}/members/{}", encode(&self.me)), None).await?;
                self.thread = None;
                self.state.thread = None;
            }
            Command::Block(user) => {
                call(Method::POST, &format!("/me/blocks/{}", encode(&user)), None).await?;
            }
            Command::Unblock(user) => {
                call(Method::DELETE, &format!("/me/blocks/{}", encode(&user)), None).await?;
            }
        }
        Ok(())
    }

    fn open(&mut self, id: i64) {
        self.thread = Some(OpenThread { id, ..Default::default() });
        self.state.thread = None;
    }

    fn thread_id(&self) -> anyhow::Result<i64> {
        Ok(self.thread.as_ref().context("no conversation open")?.id)
    }

    async fn refresh(&mut self) -> anyhow::Result<()> {
        self.ensure_key().await?;
        if self.thread.is_some() {
            if let Err(err) = self.sync_thread().await {
                // Removed from the group (or it no longer exists): back to the inbox.
                if err.downcast_ref::<Status>().is_some_and(|s| s.0 == 404) {
                    self.thread = None;
                    self.state.thread = None;
                }
                return Err(err);
            }
        }
        let mut inbox: Vec<Conversation> = serde_json::from_value(call(Method::GET, "/me/conversations", None).await?)?;
        for conversation in &mut inbox {
            conversation.members.iter_mut().for_each(sanitize);
            if let Some(title) = &mut conversation.title {
                sanitize(title);
            }
        }
        self.unread = inbox.iter().map(|c| c.unread).sum();
        self.state.inbox = inbox;
        self.state.loaded = true;
        self.publish_thread();
        Ok(())
    }

    /// Fetches keys, rotating or sharing them as the server asks, and stores mine.
    async fn sync_keys(&mut self) -> anyhow::Result<KeyInfo> {
        let secret = self.ensure_key().await?;
        let id = self.thread_id()?;
        let path = format!("/me/conversations/{id}/keys");
        let mut info: KeyInfo = serde_json::from_value(call(Method::GET, &path, None).await?)?;
        self.my_id = info.members.iter().find(|m| m.username.eq_ignore_ascii_case(&self.me)).context("not a member")?.user_id;

        let thread = self.thread.as_mut().context("no conversation open")?;
        for key in &info.keys {
            if let Some(plain) = from_hex(&key.wrapped_key).and_then(|w| crypto::unwrap_key(&secret, &w, id, key.version)) {
                thread.keys.insert(key.version, plain);
            }
        }

        // A new key goes to everyone; the current one only to members still waiting for it.
        let share = if info.rekey_needed {
            Some((info.version + 1, crypto::random::<KEY_LEN>()?, true))
        } else {
            thread.keys.get(&info.version).map(|key| (info.version, *key, false))
        };
        if let Some((version, key, rotating)) = share {
            let mut wraps = Vec::new();
            for member in info.members.iter().filter(|m| rotating || !m.has_current_key) {
                // A member without a usable key can't be wrapped for; they're picked up later.
                let Some(public) = member.key() else { continue };
                let wrapped = crypto::wrap_key(&key, &public, id, version)?;
                wraps.push(json!({ "username": member.username, "wrapped_key": to_hex(&wrapped) }));
            }
            if !wraps.is_empty() {
                call(Method::POST, &path, Some(json!({ "version": version, "keys": wraps }))).await?;
                if rotating {
                    thread.keys.insert(version, key);
                    info.version = version;
                    info.rekey_needed = false;
                }
            }
        }
        thread.waiting_for_key = !thread.keys.contains_key(&info.version);

        let mut pins_changed = false;
        thread.fingerprints.clear();
        for member in info.members.iter().filter(|m| m.user_id != self.my_id) {
            let Some(public) = member.key() else { continue };
            let fingerprint = crypto::fingerprint(&public);
            let mut name = member.username.clone();
            sanitize(&mut name);
            match self.pins.insert(member.username.clone(), fingerprint.clone()) {
                Some(old) if old == fingerprint => {}
                old => {
                    pins_changed = true;
                    if old.is_some() {
                        thread.key_changed = Some(name.clone());
                    }
                }
            }
            thread.fingerprints.push((name, fingerprint));
        }
        if pins_changed {
            if let Err(err) = std::fs::write(data_file("pins", &self.me), serde_json::to_vec(&self.pins)?) {
                crate::install::log_file(&format!("couldn't save chat key pins: {err}"));
            }
        }
        Ok(info)
    }

    async fn sync_thread(&mut self) -> anyhow::Result<()> {
        // Another member rotating at the same moment answers 409; the next round sees their key.
        match self.sync_keys().await {
            Err(err) if err.downcast_ref::<Status>().is_some_and(|s| s.0 == 409) => {
                self.sync_keys().await?;
            }
            other => {
                other?;
            }
        }
        let thread = self.thread.as_mut().context("no conversation open")?;
        // shortcut: opening a thread pages through its whole history; add a "latest N" query when threads get long.
        loop {
            let path = format!("/me/conversations/{}/messages?after={}&limit={PAGE}", thread.id, thread.cursor);
            let page: Vec<RawMessage> = serde_json::from_value(call(Method::GET, &path, None).await?)?;
            let full = page.len() == PAGE;
            for message in page {
                thread.cursor = thread.cursor.max(message.change_seq);
                thread.messages.insert(message.id, message);
            }
            if !full {
                return Ok(());
            }
        }
    }

    async fn publish(&mut self, edit: Option<i64>, text: &str) -> anyhow::Result<()> {
        let mut attempts = 0;
        loop {
            let info = self.sync_keys().await?;
            let thread = self.thread.as_ref().context("no conversation open")?;
            let key = thread.keys.get(&info.version).with_context(|| self.lang.tr("chat-waiting-key"))?;
            let (nonce, sealed) = crypto::seal_message(key, thread.id, self.my_id, info.version, text)?;
            let body = json!({ "ciphertext": to_hex(&sealed), "nonce": to_hex(&nonce), "key_version": info.version });
            let base = format!("/me/conversations/{}/messages", thread.id);
            let result = match edit {
                Some(id) => call(Method::PATCH, &format!("{base}/{id}"), Some(body)).await,
                None => call(Method::POST, &base, Some(body)).await,
            };
            match result {
                // The key rotated between reading it and sending.
                Err(err) if attempts == 0 && err.downcast_ref::<Status>().is_some_and(|s| s.0 == 409) => attempts = 1,
                other => return other.map(|_| ()),
            }
        }
    }

    fn publish_thread(&mut self) {
        let Some(thread) = &self.thread else {
            self.state.thread = None;
            return;
        };
        let Some(conversation) = self.state.inbox.iter().find(|c| c.id == thread.id).cloned() else {
            return;
        };
        let lines = thread
            .messages
            .values()
            .map(|m| {
                let text = m.sender_id.and_then(|sender| {
                    let key = thread.keys.get(&m.key_version)?;
                    let mut text = crypto::open_message(
                        key,
                        thread.id,
                        sender,
                        m.key_version,
                        &from_hex(&m.nonce)?,
                        &from_hex(&m.ciphertext)?,
                    )?;
                    sanitize(&mut text);
                    Some(text)
                });
                let mut sender = m.sender_username.clone().unwrap_or_else(|| "?".into());
                sanitize(&mut sender);
                Line {
                    id: m.id,
                    sender,
                    mine: m.sender_id == Some(self.my_id),
                    text,
                    edited: m.edited_at.is_some(),
                }
            })
            .collect();
        self.state.thread = Some(Thread {
            conversation,
            lines,
            fingerprints: thread.fingerprints.clone(),
            key_changed: thread.key_changed.clone(),
            waiting_for_key: thread.waiting_for_key,
        });
    }
}
