use crate::error::{BackendError, Result};
use gstreamer::{self as gst, prelude::*};
use serde::Serialize;

fn failure(message: &str) -> BackendError {
    BackendError::Audio(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Stopped,
    Loading,
    Playing,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f64,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AudioEvent {
    Ended,
    Error,
    Buffering(u8),
    StateChanged(PlaybackState),
}

/// The application service owns this engine; widgets never own playback state.
pub struct AudioEngine {
    pipeline: gst::Element,
    bus: gst::Bus,
    loaded: bool,
    wants_play: bool,
    buffering: bool,
    pending_seek: Option<u64>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        Self::create(false)
    }
    pub fn configured(proxy: &crate::storage::ProxyConfig) -> Result<Self> {
        Self::create_configured(false, proxy)
    }
    fn create_configured(silent: bool, proxy: &crate::storage::ProxyConfig) -> Result<Self> {
        proxy.validate()?;
        let engine = Self::create(silent)?;
        let proxy = proxy.clone();
        engine
            .pipeline
            .connect("source-setup", false, move |values| {
                if let Ok(source) = values[1].get::<gst::Element>()
                    && source.find_property("proxy").is_some()
                {
                    if source.find_property("http-log-level").is_some() {
                        source.set_property_from_str("http-log-level", "none");
                    }
                    if source.find_property("retries").is_some() {
                        source.set_property("retries", 0i32);
                    }
                    if source.find_property("timeout").is_some() {
                        source.set_property("timeout", 20u32);
                    }
                    match &proxy {
                        crate::storage::ProxyConfig::System => {}
                        crate::storage::ProxyConfig::Direct => source.set_property("proxy", ""),
                        crate::storage::ProxyConfig::Http(url) => {
                            source.set_property("proxy", url.as_str())
                        }
                    }
                }
                None
            });
        Ok(engine)
    }
    /// Decode against a synchronized null sink, for hardware-independent acceptance.
    pub fn silent() -> Result<Self> {
        Self::create(true)
    }
    pub fn can_seek(&self) -> bool {
        let mut query = gst::query::Seeking::new(gst::Format::Time);
        self.loaded && self.pipeline.query(&mut query) && query.result().0
    }
    fn create(silent: bool) -> Result<Self> {
        gst::init().map_err(|_| failure("GStreamer initialization"))?;
        let pipeline = gst::ElementFactory::make("playbin")
            .build()
            .map_err(|_| failure("playbin plugin missing"))?;
        // This is an audio client: video decoding must never open an unmanaged window.
        let video = gst::ElementFactory::make("fakesink")
            .build()
            .map_err(|_| failure("fakesink plugin missing"))?;
        pipeline.set_property("video-sink", &video);
        if silent {
            let sink = gst::ElementFactory::make("fakesink")
                .property("sync", true)
                .build()
                .map_err(|_| failure("fakesink plugin missing"))?;
            pipeline.set_property("audio-sink", &sink);
        }
        let bus = pipeline
            .bus()
            .ok_or_else(|| failure("pipeline bus missing"))?;
        Ok(Self {
            pipeline,
            bus,
            loaded: false,
            wants_play: false,
            buffering: false,
            pending_seek: None,
        })
    }

    pub fn load(&mut self, uri: &str) -> Result<()> {
        let url = reqwest::Url::parse(uri)
            .map_err(|_| BackendError::InvalidInput("invalid audio URI".into()))?;
        if !matches!(url.scheme(), "https" | "http" | "file") {
            return Err(BackendError::InvalidInput(
                "unsupported audio URI scheme".into(),
            ));
        }
        self.stop()?;
        self.pipeline.set_property("uri", uri);
        self.loaded = true;
        Ok(())
    }
    pub fn play(&mut self) -> Result<()> {
        if !self.loaded {
            return Err(failure("no source loaded"));
        }
        self.pipeline
            .set_state(if self.buffering {
                gst::State::Paused
            } else {
                gst::State::Playing
            })
            .map_err(|_| failure("start playback"))?;
        self.wants_play = true;
        Ok(())
    }
    pub fn pause(&mut self) -> Result<()> {
        if !self.loaded {
            return Err(failure("no source loaded"));
        }
        self.pipeline
            .set_state(gst::State::Paused)
            .map_err(|_| failure("pause playback"))?;
        self.wants_play = false;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Null)
            .map_err(|_| failure("stop playback"))?;
        self.loaded = false;
        self.wants_play = false;
        self.buffering = false;
        self.pending_seek = None;
        self.bus.set_flushing(true);
        self.bus.set_flushing(false);
        Ok(())
    }
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err(BackendError::InvalidInput(
                "volume must be finite and between 0 and 1".into(),
            ));
        }
        self.pipeline.set_property("volume", volume);
        Ok(())
    }
    pub fn seek(&mut self, position_ms: u64) -> Result<()> {
        if !self.loaded || position_ms > u64::MAX / 1_000_000 {
            return Err(BackendError::InvalidInput(
                "invalid seek position or no source".into(),
            ));
        }
        if self
            .pipeline
            .query_duration::<gst::ClockTime>()
            .is_some_and(|duration| position_ms > duration.mseconds())
        {
            return Err(BackendError::InvalidInput("seek beyond duration".into()));
        }
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::ClockTime::from_mseconds(position_ms),
            )
            .map_err(|_| failure("source does not support seek"))?;
        // FLUSH seeks complete asynchronously; preserve the accepted position meanwhile.
        self.pending_seek = Some(position_ms);
        Ok(())
    }
    pub fn snapshot(&self) -> PlaybackSnapshot {
        let state = match self.pipeline.current_state() {
            _ if !self.loaded => PlaybackState::Stopped,
            gst::State::Playing => PlaybackState::Playing,
            gst::State::Paused if !self.wants_play => PlaybackState::Paused,
            _ => PlaybackState::Loading,
        };
        PlaybackSnapshot {
            state,
            position_ms: self.pending_seek.unwrap_or_else(|| {
                self.pipeline
                    .query_position::<gst::ClockTime>()
                    .map(|t| t.mseconds())
                    .unwrap_or(0)
            }),
            duration_ms: self
                .pipeline
                .query_duration::<gst::ClockTime>()
                .map(|t| t.mseconds()),
            volume: self.pipeline.property("volume"),
        }
    }
    /// Call regularly (e.g. every 100 ms) from the owner to consume engine events.
    pub fn drain_events(&mut self) -> Result<Vec<AudioEvent>> {
        let mut events = Vec::new();
        while let Some(message) = self.bus.pop() {
            match message.view() {
                gst::MessageView::AsyncDone(_) => self.pending_seek = None,
                gst::MessageView::Eos(_) => {
                    self.stop()?;
                    events.push(AudioEvent::Ended);
                }
                gst::MessageView::Error(_) => {
                    self.stop()?;
                    events.push(AudioEvent::Error);
                }
                gst::MessageView::Buffering(buffer) => {
                    let percent = buffer.percent().clamp(0, 100) as u8;
                    self.buffering = percent < 100;
                    if self.wants_play {
                        self.pipeline
                            .set_state(if self.buffering {
                                gst::State::Paused
                            } else {
                                gst::State::Playing
                            })
                            .map_err(|_| failure("buffering state change"))?;
                    }
                    events.push(AudioEvent::Buffering(percent));
                }
                gst::MessageView::StateChanged(_)
                    if message.src().is_some_and(|source| {
                        source == self.pipeline.upcast_ref::<gst::Object>()
                    }) =>
                {
                    events.push(AudioEvent::StateChanged(self.snapshot().state))
                }
                _ => {}
            }
        }
        Ok(events)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn wait(engine: &mut AudioEngine, state: PlaybackState) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while engine.snapshot().state != state {
            assert!(Instant::now() < deadline, "state transition timed out");
            assert!(!engine.drain_events().unwrap().contains(&AudioEvent::Error));
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn decodes_generated_wav_pauses_seeks_and_reaches_eos() {
        let mut engine = AudioEngine::create(true).unwrap();
        assert!(engine.set_volume(f64::NAN).is_err());
        assert!(engine.play().is_err());
        let samples = 16_000u32;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + samples * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(8000u32.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((samples * 2).to_le_bytes());
        wav.resize(44 + samples as usize * 2, 0);
        let file = std::env::temp_dir().join(format!(
            "aka-audio-test-{}-{}.wav",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::write(&file, wav).unwrap();
        let uri = reqwest::Url::from_file_path(&file).unwrap();
        engine.load(uri.as_str()).unwrap();
        engine.set_volume(0.25).unwrap();
        engine.play().unwrap();
        wait(&mut engine, PlaybackState::Playing);
        engine.pause().unwrap();
        wait(&mut engine, PlaybackState::Paused);
        assert_eq!(engine.snapshot().duration_ms, Some(2000));
        assert_eq!(engine.snapshot().volume, 0.25);
        engine.seek(1700).unwrap();
        engine.play().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let events = engine.drain_events().unwrap();
            assert!(!events.contains(&AudioEvent::Error));
            if events.contains(&AudioEvent::Ended) {
                break;
            }
            assert!(Instant::now() < deadline, "EOS timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(engine.snapshot().state, PlaybackState::Stopped);
        drop(engine);
        std::fs::remove_file(file).unwrap();
    }
    #[tokio::test]
    async fn http_audio_source_uses_explicit_proxy() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy =
            crate::storage::ProxyConfig::Http(format!("http://{}", listener.local_addr().unwrap()));
        let mut engine = AudioEngine::create_configured(true, &proxy).unwrap();
        engine
            .load("http://unresolved.invalid/synthetic.wav")
            .unwrap();
        engine.play().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 4096];
            let n = socket.read(&mut bytes).await.unwrap();
            assert!(
                String::from_utf8_lossy(&bytes[..n])
                    .starts_with("GET http://unresolved.invalid/synthetic.wav")
            );
            socket
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        })
        .await
        .unwrap();
        engine.stop().unwrap();
    }
}
