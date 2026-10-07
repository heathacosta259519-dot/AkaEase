# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-10-07

### Added
- **Selectable Audio Quality**:
  - Full pipeline support for standard (128k), higher (192k), exhigh (320k), lossless (FLAC SQ), and Hi-Res (24bit master) audio streaming.
  - Position-preserving stream reloading: switching audio quality maintains the exact playback timestamp with seamless GStreamer pipeline reconnection.
  - Interactive quality badge with React Portal-mounted floating menu in both bottom player bar and fullscreen lyrics theater.
  - Accurate bitrate (kbps) and audio format (MP3/FLAC) indicator with graceful degradation notification for non-VIP accounts or standard-only songs.
  - Persistent default playback quality setting in System Preferences.
- **Playlist & Library Management**:
  - In-app liked music toggle with real-time UI synchronization across all views.
  - Complete playlist operations: create playlist, delete playlist, add/remove tracks, and subscribe/unsubscribe.

### Fixed
- **UI Menu Clipping Fix**:
  - Re-architected song action popovers with top-level `createPortal` mounting, completely resolving layout clipping caused by CSS `contain: paint` / `content-visibility: auto` on virtualized song lists.

## [0.1.0] - 2026-10-06

### Added
- **Initial Release of AkaEase**: A native Linux music client built with Tauri 2, Rust, and React.
- **Audio Engine**: GStreamer high-performance playback engine with gapless/resilient playback.
- **System Integration**: Native Linux MPRIS D-Bus bridge for media controls and hardware shortcuts.
- **UI & Controls**:
  - Immersive full-width playback deck and lyrics theater.
  - Smooth vinyl rotation animations with natural pause dynamics.
  - Apple Music-style dual-language synchronized lyrics with zero-jitter typography.
  - Ultra-high framerate scrolling pipeline targeting 150fps+ on high-refresh monitors.
  - Borderless frameless window with client-side drag regions.

### Fixed
- Fixed an issue where opening the lyrics view occasionally shifted the parent viewport and clipped top navigation controls.

