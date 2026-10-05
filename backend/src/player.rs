//! Serialized playback owner shared by IPC and MPRIS. Source URLs never enter snapshots.
use crate::{
    account::AccountService,
    api::{NeteaseClient, validate_id},
    audio::{AudioEngine, AudioEvent, PlaybackSnapshot, PlaybackState},
    error::{BackendError, Result},
    model::{StreamSource, Track},
    queue::{Queue, Repeat},
};
use serde::Serialize;
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::Instant,
};

pub type ResolveFuture<'a> = Pin<Box<dyn Future<Output = Result<StreamSource>> + Send + 'a>>;
pub trait SourceResolver: Send + Sync + 'static {
    fn resolve<'a>(&'a self, track: &'a Track) -> ResolveFuture<'a>;
}

pub struct NeteaseResolver {
    pub account: Arc<AccountService>,
    pub anonymous: NeteaseClient,
}
impl SourceResolver for NeteaseResolver {
    fn resolve<'a>(&'a self, track: &'a Track) -> ResolveFuture<'a> {
        Box::pin(async move {
            if self.account.snapshot().await.profile.is_some() {
                self.account.stream(&track.id).await
            } else {
                self.anonymous.stream(&track.id).await
            }
        })
    }
}

pub trait AudioDriver: Send + 'static {
    fn load(&mut self, uri: &str) -> Result<()>;
    fn play(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn seek(&mut self, ms: u64) -> Result<()>;
    fn volume(&mut self, value: f64) -> Result<()>;
    fn snapshot(&self) -> PlaybackSnapshot;
    fn events(&mut self) -> Result<Vec<AudioEvent>>;
    fn can_seek(&self) -> bool;
}
impl AudioDriver for AudioEngine {
    fn load(&mut self, uri: &str) -> Result<()> {
        self.load(uri)
    }
    fn play(&mut self) -> Result<()> {
        self.play()
    }
    fn pause(&mut self) -> Result<()> {
        self.pause()
    }
    fn stop(&mut self) -> Result<()> {
        self.stop()
    }
    fn seek(&mut self, ms: u64) -> Result<()> {
        self.seek(ms)
    }
    fn volume(&mut self, v: f64) -> Result<()> {
        self.set_volume(v)
    }
    fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot()
    }
    fn events(&mut self) -> Result<Vec<AudioEvent>> {
        self.drain_events()
    }
    fn can_seek(&self) -> bool {
        self.can_seek()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub sequence: String,
    pub queue_revision: String,
    pub selection_id: String,
    pub current: Option<Track>,
    pub current_index: Option<usize>,
    pub queue_length: usize,
    pub playback: PlaybackSnapshot,
    pub play_when_ready: bool,
    pub resolving: bool,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub can_seek: bool,
    pub buffering_percent: Option<u8>,
    pub is_preview: bool,
    pub last_error: Option<BackendError>,
    pub seek_serial: String,
    pub seek_position_ms: u64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerQueue {
    pub revision: String,
    pub tracks: Vec<Track>,
    pub current_index: Option<usize>,
}

pub enum PlayerCommand {
    Restore(crate::storage::SavedPlayer),
    Replace {
        tracks: Vec<Track>,
        selected: usize,
        autoplay: bool,
    },
    Select {
        index: usize,
        revision: String,
    },
    Remove {
        index: usize,
        revision: String,
    },
    Play,
    Pause,
    Toggle,
    Stop,
    /// Cancel session-bound audio while retaining the resume position.
    Suspend,
    Next,
    Previous,
    Seek {
        position_ms: u64,
        selection_id: String,
    },
    SeekRelative(i64),
    Volume(f64),
    Repeat(Repeat),
    Shuffle(bool),
    Shutdown,
}
enum Request {
    Save(oneshot::Sender<crate::storage::SavedPlayer>),
    Control(PlayerCommand, oneshot::Sender<Result<PlayerSnapshot>>),
    Queue(oneshot::Sender<PlayerQueue>),
}
#[derive(Clone)]
pub struct PlayerHandle {
    tx: mpsc::Sender<Request>,
    state: watch::Receiver<PlayerSnapshot>,
}
impl PlayerHandle {
    pub fn spawn(driver: impl AudioDriver, resolver: Arc<dyn SourceResolver>) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let (resolved_tx, resolved_rx) = mpsc::channel(4);
        let mut actor = Actor {
            driver: Box::new(driver),
            resolver,
            queue: Queue::default(),
            revision: 0,
            selection: 0,
            sequence: 0,
            desired: false,
            loaded: false,
            resolving: None,
            resolved_tx,
            resolve_task: None,
            failures: 0,
            retried: false,
            retry_position: None,
            restored_position: None,
            buffering: None,
            preview: false,
            error: None,
            seek_serial: 0,
            seek_position: 0,
            deadline: None,
        };
        let (publisher, state) = watch::channel(actor.snapshot());
        tokio::spawn(async move {
            actor.run(rx, resolved_rx, publisher).await;
        });
        Self { tx, state }
    }
    pub fn snapshot(&self) -> PlayerSnapshot {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<PlayerSnapshot> {
        self.state.clone()
    }
    pub async fn command(&self, command: PlayerCommand) -> Result<PlayerSnapshot> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Request::Control(command, tx))
            .await
            .map_err(|_| BackendError::ServiceClosed)?;
        rx.await.map_err(|_| BackendError::ServiceClosed)?
    }
    pub async fn queue(&self) -> Result<PlayerQueue> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Request::Queue(tx))
            .await
            .map_err(|_| BackendError::ServiceClosed)?;
        rx.await.map_err(|_| BackendError::ServiceClosed)
    }
    pub async fn checkpoint(&self) -> Result<crate::storage::SavedPlayer> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Request::Save(tx))
            .await
            .map_err(|_| BackendError::ServiceClosed)?;
        rx.await.map_err(|_| BackendError::ServiceClosed)
    }
}
struct Actor {
    driver: Box<dyn AudioDriver>,
    resolver: Arc<dyn SourceResolver>,
    queue: Queue,
    revision: u64,
    selection: u64,
    sequence: u64,
    desired: bool,
    loaded: bool,
    resolving: Option<u64>,
    resolved_tx: mpsc::Sender<(u64, Result<StreamSource>)>,
    resolve_task: Option<JoinHandle<()>>,
    failures: usize,
    retried: bool,
    retry_position: Option<u64>,
    restored_position: Option<u64>,
    buffering: Option<u8>,
    preview: bool,
    error: Option<BackendError>,
    seek_serial: u64,
    seek_position: u64,
    deadline: Option<Instant>,
}
impl Actor {
    fn snapshot(&self) -> PlayerSnapshot {
        let queue = self.queue.snapshot();
        let mut playback = self.driver.snapshot();
        if self.resolving.is_some() {
            playback.state = PlaybackState::Loading;
            playback.position_ms = 0;
            playback.duration_ms = None;
        }
        PlayerSnapshot {
            sequence: self.sequence.to_string(),
            queue_revision: self.revision.to_string(),
            selection_id: self.selection.to_string(),
            current: self.queue.current().cloned(),
            current_index: queue.current_index,
            queue_length: queue.tracks.len(),
            playback,
            play_when_ready: self.desired,
            resolving: self.resolving.is_some(),
            repeat: queue.repeat,
            shuffle: queue.shuffle,
            can_next: self.queue.can_next(),
            can_previous: self.queue.can_previous(),
            can_seek: self.loaded && self.driver.can_seek(),
            buffering_percent: self.buffering,
            is_preview: self.preview,
            last_error: self.error.clone(),
            seek_serial: self.seek_serial.to_string(),
            seek_position_ms: self.seek_position,
        }
    }
    fn publish(&mut self, publisher: &watch::Sender<PlayerSnapshot>) {
        let mut state = self.snapshot();
        if *publisher.borrow() != state {
            self.sequence += 1;
            state.sequence = self.sequence.to_string();
            publisher.send_replace(state);
        }
    }
    fn cancel(&mut self) -> Result<()> {
        if let Some(task) = self.resolve_task.take() {
            task.abort();
        }
        self.selection += 1;
        self.resolving = None;
        self.loaded = false;
        self.buffering = None;
        self.preview = false;
        self.deadline = None;
        self.retry_position = None;
        self.restored_position = None;
        self.driver.stop()
    }
    fn start(&mut self, retry: bool) -> Result<()> {
        let position = self.driver.snapshot().position_ms;
        self.cancel()?;
        let Some(track) = self.queue.current().cloned() else {
            self.desired = false;
            return Ok(());
        };
        self.retried = retry;
        if retry {
            self.retry_position = Some(position);
        }
        let generation = self.selection;
        self.resolving = Some(generation);
        self.deadline = Some(Instant::now() + Duration::from_secs(30));
        let resolver = self.resolver.clone();
        let tx = self.resolved_tx.clone();
        self.resolve_task = Some(tokio::spawn(async move {
            let result = tokio::time::timeout(Duration::from_secs(20), resolver.resolve(&track))
                .await
                .unwrap_or(Err(BackendError::Timeout));
            let _ = tx.send((generation, result)).await;
        }));
        Ok(())
    }
    fn manual(&mut self) {
        self.failures = 0;
        self.error = None;
        self.retried = false;
    }
    fn revision(&self, revision: &str) -> Result<()> {
        if revision != self.revision.to_string() {
            Err(BackendError::StaleOperation)
        } else {
            Ok(())
        }
    }
    fn seek(&mut self, ms: u64) -> Result<()> {
        if !self.loaded || !self.driver.can_seek() {
            return Err(BackendError::Audio("current source cannot seek".into()));
        }
        self.driver.seek(ms)?;
        self.seek_serial += 1;
        self.seek_position = ms;
        Ok(())
    }
    fn control(&mut self, cmd: PlayerCommand) -> Result<()> {
        match cmd {
            PlayerCommand::Restore(saved) => {
                saved.validate()?;
                self.control(PlayerCommand::Replace {
                    tracks: saved.tracks,
                    selected: saved.current_index.unwrap_or(0),
                    autoplay: false,
                })?;
                self.queue.set_repeat(saved.repeat);
                self.queue.set_shuffle(saved.shuffle);
                self.driver.volume(saved.volume)?;
                self.restored_position = Some(saved.position_ms);
            }
            PlayerCommand::Replace {
                tracks,
                selected,
                autoplay,
            } => {
                if tracks.len() > 10_000 {
                    return Err(BackendError::InvalidInput("queue limit is 10000".into()));
                }
                for track in &tracks {
                    validate_id(&track.id)?;
                }
                self.queue.replace(tracks, selected)?;
                self.revision += 1;
                self.manual();
                self.desired = autoplay;
                if autoplay {
                    self.start(false)?;
                } else {
                    self.cancel()?;
                }
            }
            PlayerCommand::Select { index, revision } => {
                self.revision(&revision)?;
                self.queue.select(index)?;
                self.manual();
                self.desired = true;
                self.start(false)?;
            }
            PlayerCommand::Remove { index, revision } => {
                self.revision(&revision)?;
                let current = self.queue.current_index() == Some(index);
                self.queue.remove(index)?;
                self.revision += 1;
                if current {
                    self.manual();
                    if self.desired || self.loaded || self.resolving.is_some() {
                        self.start(false)?;
                    } else {
                        self.cancel()?;
                    }
                }
            }
            PlayerCommand::Play => {
                self.manual();
                self.desired = true;
                if self.loaded {
                    self.driver.play()?;
                } else if self.resolving.is_none() {
                    let resume = self.restored_position.take();
                    self.start(false)?;
                    self.retry_position = resume;
                }
            }
            PlayerCommand::Pause => {
                self.desired = false;
                if self.loaded {
                    self.driver.pause()?;
                }
            }
            PlayerCommand::Toggle => {
                if self.desired {
                    self.control(PlayerCommand::Pause)?;
                } else {
                    self.control(PlayerCommand::Play)?;
                }
            }
            PlayerCommand::Stop | PlayerCommand::Shutdown => {
                self.desired = false;
                self.cancel()?;
            }
            PlayerCommand::Suspend => {
                let position = self
                    .restored_position
                    .or(self.retry_position)
                    .unwrap_or_else(|| self.driver.snapshot().position_ms);
                self.desired = false;
                self.cancel()?;
                self.restored_position = Some(position);
            }
            PlayerCommand::Next => {
                self.manual();
                if self.queue.advance(false).is_some() {
                    self.desired = true;
                    self.start(false)?;
                } else {
                    self.desired = false;
                    self.cancel()?;
                }
            }
            PlayerCommand::Previous => {
                self.manual();
                if self.queue.previous().is_some() {
                    self.desired = true;
                    self.start(false)?;
                }
            }
            PlayerCommand::Seek {
                position_ms,
                selection_id,
            } => {
                if selection_id != self.selection.to_string() {
                    return Err(BackendError::StaleOperation);
                }
                self.seek(position_ms)?;
            }
            PlayerCommand::SeekRelative(offset) => {
                let now = self.driver.snapshot();
                let end = now.duration_ms.unwrap_or(u64::MAX / 1_000_000);
                self.seek(now.position_ms.saturating_add_signed(offset).min(end))?;
            }
            PlayerCommand::Volume(volume) => {
                if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
                    return Err(BackendError::InvalidInput("volume must be 0..1".into()));
                }
                self.driver.volume(volume)?;
            }
            PlayerCommand::Repeat(repeat) => self.queue.set_repeat(repeat),
            PlayerCommand::Shuffle(shuffle) => self.queue.set_shuffle(shuffle),
        }
        Ok(())
    }
    fn fail(&mut self, error: BackendError, refresh: bool) {
        self.error = Some(error);
        if refresh && !self.retried && self.desired && self.start(true).is_ok() {
            return;
        }
        self.failures += 1;
        let limit = self.queue.snapshot().tracks.len().min(10);
        if self.desired
            && self.failures < limit
            && self.queue.advance(false).is_some()
            && self.start(false).is_ok()
        {
            return;
        }
        self.desired = false;
        let _ = self.cancel();
    }
    fn resolved(&mut self, generation: u64, result: Result<StreamSource>) {
        if self.resolving != Some(generation) {
            return;
        }
        self.resolving = None;
        self.resolve_task = None;
        match result {
            Ok(source) => {
                if self
                    .queue
                    .current()
                    .is_none_or(|track| track.id != source.track_id)
                    || source.expires_in_seconds == Some(0)
                {
                    self.fail(
                        BackendError::Protocol("invalid resolved source".into()),
                        false,
                    );
                    return;
                }
                self.preview = source.is_preview;
                let result = self.driver.load(&source.url).and_then(|_| {
                    if self.desired {
                        self.driver.play()
                    } else {
                        self.driver.pause()
                    }
                });
                if let Err(error) = result {
                    self.fail(error, true);
                } else {
                    self.loaded = true;
                }
            }
            Err(error) => self.fail(error, false),
        }
    }
    fn tick(&mut self) {
        let before_events_position = self.driver.snapshot().position_ms;
        let events = match self.driver.events() {
            Ok(events) => events,
            Err(error) => {
                self.fail(error, true);
                return;
            }
        };
        for event in events {
            match event {
                AudioEvent::Ended if self.loaded => {
                    self.loaded = false;
                    self.failures = 0;
                    if self.desired && self.queue.advance(true).is_some() {
                        if let Err(error) = self.start(false) {
                            self.fail(error, false);
                        }
                    } else {
                        self.desired = false;
                        let _ = self.cancel();
                    }
                    return;
                }
                AudioEvent::Error if self.loaded => {
                    self.loaded = false;
                    self.fail(BackendError::Audio("playback failed".into()), true);
                    if self.retried && self.resolving.is_some() {
                        self.retry_position = Some(before_events_position);
                    }
                    return;
                }
                AudioEvent::Buffering(percent) => {
                    self.buffering = Some(percent);
                }
                _ => {}
            }
        }
        let state = self.driver.snapshot();
        if self.loaded && matches!(state.state, PlaybackState::Playing | PlaybackState::Paused) {
            self.deadline = None;
            if let Some(position) = self.retry_position.take().filter(|&p| p > 0)
                && let Err(error) = self.seek(position.min(state.duration_ms.unwrap_or(position)))
            {
                self.error = Some(error);
            }
        } else {
            if self.loaded {
                self.deadline
                    .get_or_insert_with(|| Instant::now() + Duration::from_secs(30));
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                self.fail(BackendError::Timeout, false);
            }
        }
    }
    async fn run(
        &mut self,
        mut rx: mpsc::Receiver<Request>,
        mut resolved: mpsc::Receiver<(u64, Result<StreamSource>)>,
        publisher: watch::Sender<PlayerSnapshot>,
    ) {
        let mut ticks = tokio::time::interval(Duration::from_millis(100));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                request=rx.recv()=>match request {
                    Some(Request::Save(reply))=>{
                        let queue = self.queue.snapshot();
                        let playback = self.driver.snapshot();
                        let _ = reply.send(crate::storage::SavedPlayer { version: 1, tracks: queue.tracks.to_vec(), current_index: queue.current_index,
                            repeat: queue.repeat, shuffle: queue.shuffle, volume: playback.volume,
                            position_ms: self.restored_position.or(self.retry_position).unwrap_or(playback.position_ms) });
                    }
                    Some(Request::Control(command,reply))=>{
                        let shutdown=matches!(command,PlayerCommand::Shutdown);
                        let result=self.control(command);self.publish(&publisher);let _=reply.send(result.map(|_|self.snapshot()));
                        if shutdown {break;}
                    }
                    Some(Request::Queue(reply))=>{let _=reply.send(PlayerQueue {revision:self.revision.to_string(),tracks:self.queue.snapshot().tracks.to_vec(),current_index:self.queue.current_index()});}
                    None=>break,
                },
                Some((generation,result))=resolved.recv()=>{self.resolved(generation,result);self.publish(&publisher);}
                _=ticks.tick()=>{self.tick();self.publish(&publisher);}
            }
        }
        let _ = self.cancel();
    }
}

#[cfg(test)]
#[path = "player_tests.rs"]
mod tests;
