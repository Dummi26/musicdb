use crate::load::ToFromBytes;

use super::SongId;

#[derive(Clone, Debug)]
pub struct Queue {
    enabled: bool,
    content: QueueContent,
}
#[derive(Clone, Debug)]
pub enum QueueContent {
    Song(SongId),
    Folder(usize, Vec<Queue>, String),
}

impl Queue {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn content(&self) -> &QueueContent {
        &self.content
    }

    pub fn add_to_end(&mut self, v: Self) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(_, vec, _) => {
                vec.push(v);
                true
            }
        }
    }
    pub fn insert(&mut self, v: Self, index: usize) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(_, vec, _) => {
                if index <= vec.len() {
                    vec.insert(index, v);
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        if !self.enabled {
            return 0;
        }
        match &self.content {
            QueueContent::Song(_) => 1,
            QueueContent::Folder(_, v, _) => v.iter().map(|v| v.len()).sum(),
        }
    }

    /// recursively descends the queue until the current active element is found, then returns it.
    pub fn get_current(&self) -> Option<&Self> {
        match &self.content {
            QueueContent::Folder(i, v, _) => {
                let i = *i;
                if let Some(v) = v.get(i) {
                    v.get_current()
                } else {
                    None
                }
            }
            QueueContent::Song(_) => Some(self),
        }
    }
    pub fn get_current_song(&self) -> Option<&SongId> {
        if let QueueContent::Song(id) = self.get_current()?.content() {
            Some(id)
        } else {
            None
        }
    }
    pub fn get_next_song(&self) -> Option<&SongId> {
        if let QueueContent::Song(id) = self.get_next()?.content() {
            Some(id)
        } else {
            None
        }
    }
    pub fn get_next(&self) -> Option<&Self> {
        match &self.content {
            QueueContent::Folder(i, vec, _) => {
                let i = *i;
                if let Some(v) = vec.get(i) {
                    if let Some(v) = v.get_next() {
                        Some(v)
                    } else {
                        if let Some(v) = vec.get(i + 1) {
                            v.get_current()
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            }
            QueueContent::Song(_) => None,
        }
    }

    pub fn advance_index(&mut self) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(index, contents, _) => {
                if let Some(c) = contents.get_mut(*index) {
                    // inner value could advance index, do nothing.
                    if c.advance_index() {
                        true
                    } else {
                        loop {
                            if *index + 1 < contents.len() {
                                // can advance
                                *index += 1;
                                if contents[*index].enabled {
                                    break true;
                                }
                            } else {
                                // can't advance: index would be out of bounds
                                *index = 0;
                                break false;
                            }
                        }
                    }
                } else {
                    *index = 0;
                    false
                }
            }
        }
    }

    pub fn set_index(&mut self, index: &Vec<usize>, depth: usize) {
        let i = index.get(depth).map(|v| *v).unwrap_or(0);
        match &mut self.content {
            QueueContent::Song(_) => {}
            QueueContent::Folder(idx, contents, _) => {
                *idx = i;
                for (i2, c) in contents.iter_mut().enumerate() {
                    if i2 != i {
                        c.set_index(&vec![], 0)
                    }
                }
                if let Some(c) = contents.get_mut(i) {
                    c.set_index(index, depth + 1);
                }
            }
        }
    }

    pub fn get_item_at_index(&self, index: &Vec<usize>, depth: usize) -> Option<&Self> {
        if let Some(i) = index.get(depth) {
            match &self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(_, v, _) => {
                    if let Some(v) = v.get(*i) {
                        v.get_item_at_index(index, depth + 1)
                    } else {
                        None
                    }
                }
            }
        } else {
            Some(self)
        }
    }
    pub fn get_item_at_index_mut(&mut self, index: &Vec<usize>, depth: usize) -> Option<&mut Self> {
        if let Some(i) = index.get(depth) {
            match &mut self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(_, v, _) => {
                    if let Some(v) = v.get_mut(*i) {
                        v.get_item_at_index_mut(index, depth + 1)
                    } else {
                        None
                    }
                }
            }
        } else {
            Some(self)
        }
    }

    pub fn remove_by_index(&mut self, index: &Vec<usize>, depth: usize) -> Option<Self> {
        if let Some(i) = index.get(depth) {
            match &mut self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(ci, v, _) => {
                    if depth + 1 < index.len() {
                        if let Some(v) = v.get_mut(*i) {
                            v.remove_by_index(index, depth + 1)
                        } else {
                            None
                        }
                    } else {
                        if *i < v.len() {
                            // if current playback is past this point,
                            // reduce the index by 1 so that it still points to the same element
                            if *ci > *i {
                                *ci -= 1;
                            }
                            Some(v.remove(*i))
                        } else {
                            None
                        }
                    }
                }
            }
        } else {
            None
        }
    }
}

impl From<QueueContent> for Queue {
    fn from(value: QueueContent) -> Self {
        Self {
            enabled: true,
            content: value,
        }
    }
}

impl ToFromBytes for Queue {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: std::io::Write,
    {
        s.write_all(&[if self.enabled { 0b11111111 } else { 0b00000000 }])?;
        self.content.to_bytes(s)?;
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::Read,
    {
        let mut enabled = [0];
        s.read_exact(&mut enabled)?;
        Ok(Self {
            enabled: enabled[0].count_ones() >= 4,
            content: ToFromBytes::from_bytes(s)?,
        })
    }
}

impl ToFromBytes for QueueContent {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: std::io::Write,
    {
        match self {
            Self::Song(id) => {
                s.write_all(&[0b11111111])?;
                id.to_bytes(s)?;
            }
            Self::Folder(index, contents, name) => {
                s.write_all(&[0b00000000])?;
                index.to_bytes(s)?;
                contents.to_bytes(s)?;
                name.to_bytes(s)?;
            }
        }
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::Read,
    {
        let mut switch_on = [0];
        s.read_exact(&mut switch_on)?;
        Ok(if switch_on[0].count_ones() > 4 {
            Self::Song(ToFromBytes::from_bytes(s)?)
        } else {
            Self::Folder(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
            )
        })
    }
}
