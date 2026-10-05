use crate::{
    error::{BackendError, Result},
    model::Track,
};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

/// Order contains indices, never a second copy of track metadata.
#[derive(Debug, Default)]
pub struct Queue {
    tracks: Vec<Track>,
    order: Vec<usize>,
    cursor: Option<usize>,
    repeat: Repeat,
    shuffle: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot<'a> {
    pub tracks: &'a [Track],
    pub current_index: Option<usize>,
    pub repeat: Repeat,
    pub shuffle: bool,
}

impl Queue {
    pub fn can_next(&self) -> bool {
        self.cursor
            .is_some_and(|cursor| cursor + 1 < self.order.len() || self.repeat == Repeat::All)
    }
    pub fn can_previous(&self) -> bool {
        self.cursor.is_some()
    }
    pub fn snapshot(&self) -> QueueSnapshot<'_> {
        QueueSnapshot {
            tracks: &self.tracks,
            current_index: self.current_index(),
            repeat: self.repeat,
            shuffle: self.shuffle,
        }
    }
    pub fn current_index(&self) -> Option<usize> {
        self.cursor.and_then(|i| self.order.get(i).copied())
    }
    pub fn current(&self) -> Option<&Track> {
        self.current_index().and_then(|i| self.tracks.get(i))
    }
    pub fn replace(&mut self, tracks: Vec<Track>, selected: usize) -> Result<()> {
        if (!tracks.is_empty() && selected >= tracks.len()) || (tracks.is_empty() && selected != 0)
        {
            return Err(BackendError::InvalidInput(
                "queue index out of range".into(),
            ));
        }
        self.tracks = tracks;
        self.order = (0..self.tracks.len()).collect();
        self.cursor = (!self.tracks.is_empty()).then_some(selected);
        self.reorder();
        Ok(())
    }
    pub fn select(&mut self, index: usize) -> Result<()> {
        let cursor = self
            .order
            .iter()
            .position(|&i| i == index)
            .ok_or_else(|| BackendError::InvalidInput("queue index out of range".into()))?;
        self.cursor = Some(cursor);
        Ok(())
    }
    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }
    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle != enabled {
            self.shuffle = enabled;
            self.reorder();
        }
    }
    fn reorder(&mut self) {
        let current = self.current_index();
        self.order = (0..self.tracks.len()).collect();
        if self.shuffle {
            self.order.shuffle(&mut rand::rng());
            if let Some(current) = current {
                let pos = self.order.iter().position(|&i| i == current).unwrap();
                self.order.swap(0, pos);
            }
        }
        self.cursor = current.and_then(|index| self.order.iter().position(|&i| i == index));
    }
    /// `automatic` means EOS; manual next skips repeat-one.
    pub fn advance(&mut self, automatic: bool) -> Option<&Track> {
        let cursor = self.cursor?;
        if automatic && self.repeat == Repeat::One {
            return self.current();
        }
        if cursor + 1 < self.order.len() {
            self.cursor = Some(cursor + 1);
        } else if self.repeat == Repeat::All {
            self.cursor = Some(0);
        } else {
            return None;
        }
        self.current()
    }
    pub fn previous(&mut self) -> Option<&Track> {
        let cursor = self.cursor?;
        self.cursor = Some(if cursor > 0 {
            cursor - 1
        } else if self.repeat == Repeat::All {
            self.order.len() - 1
        } else {
            0
        });
        self.current()
    }
    pub fn remove(&mut self, index: usize) -> Result<()> {
        let pos = self
            .order
            .iter()
            .position(|&i| i == index)
            .ok_or_else(|| BackendError::InvalidInput("queue index out of range".into()))?;
        self.tracks.remove(index);
        self.order.remove(pos);
        for entry in &mut self.order {
            if *entry > index {
                *entry -= 1;
            }
        }
        self.cursor = self.cursor.and_then(|cursor| {
            if self.order.is_empty() {
                None
            } else {
                Some(if pos < cursor {
                    cursor - 1
                } else {
                    cursor.min(self.order.len() - 1)
                })
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            artists: vec![],
            album: crate::model::Album {
                id: "0".into(),
                name: String::new(),
                cover_url: None,
            },
            duration_ms: 1000,
        }
    }
    #[test]
    fn duplicate_entries_and_shuffle_preserve_selection() {
        let mut queue = Queue::default();
        queue
            .replace(vec![track("1"), track("1"), track("2")], 1)
            .unwrap();
        queue.set_shuffle(true);
        assert_eq!(queue.current_index(), Some(1));
        queue.set_shuffle(false);
        assert_eq!(queue.current_index(), Some(1));
        queue.remove(0).unwrap();
        assert_eq!(queue.current_index(), Some(0));
        queue.remove(0).unwrap();
        assert_eq!(queue.current().unwrap().id, "2");
    }
    #[test]
    fn repeat_empty_and_invalid_replacement() {
        let mut queue = Queue::default();
        queue.set_repeat(Repeat::All);
        assert!(queue.previous().is_none());
        queue.replace(vec![track("1"), track("2")], 0).unwrap();
        assert!(queue.replace(vec![], 2).is_err());
        queue.set_repeat(Repeat::One);
        assert_eq!(queue.advance(true).unwrap().id, "1");
        assert_eq!(queue.advance(false).unwrap().id, "2");
        assert!(queue.advance(false).is_none());
        queue.set_repeat(Repeat::All);
        assert_eq!(queue.advance(true).unwrap().id, "1");
        queue.remove(1).unwrap();
        queue.remove(0).unwrap();
        assert!(queue.advance(true).is_none());
    }
}
