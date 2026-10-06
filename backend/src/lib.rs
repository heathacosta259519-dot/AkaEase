#![forbid(unsafe_code)]

pub mod account;
pub mod api;
#[cfg(feature = "audio")]
pub mod audio;
pub mod cache;
mod covers;
mod credentials;
pub mod diagnostics;
pub mod error;
pub mod lyrics;
pub mod model;
#[cfg(feature = "mpris")]
pub mod mpris;
mod personal;
#[cfg(feature = "audio")]
pub mod player;
pub mod queue;
mod session;
pub mod storage;
mod weapi;
