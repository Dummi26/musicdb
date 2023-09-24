use std::collections::VecDeque;

use rand::seq::{IteratorRandom, SliceRandom};

use crate::{load::ToFromBytes, server::Command};

use super::{database::Database, SongId};

#[derive(Clone, Debug)]
pub struct Queue {
    enabled: bool,
    content: QueueContent,
}
#[derive(Clone, Debug)]
pub enum QueueContent {
    Song(SongId),
    Folder(usize, Vec<Queue>, String),
    Loop(usize, usize, Box<Queue>),
    Random(VecDeque<Queue>),
    Shuffle {
        inner: Box<Queue>,
        state: ShuffleState,
    },
}
#[derive(Clone, Copy, Debug)]
pub enum ShuffleState {
    NotShuffled,
    Modified,
    Shuffled,
}

pub enum QueueAction {
    AddRandomSong(Vec<usize>),
    SetShuffle(Vec<usize>, bool),
}

impl Queue {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn content(&self) -> &QueueContent {
        &self.content
    }
    pub fn content_mut(&mut self) -> &mut QueueContent {
        &mut self.content
    }

    pub fn add_to_end(&mut self, v: Self) -> Option<usize> {
        match &mut self.content {
            QueueContent::Song(_) => None,
            QueueContent::Folder(_, vec, _) => {
                vec.push(v);
                Some(vec.len() - 1)
            }
            QueueContent::Loop(..) => None,
            QueueContent::Random(q) => {
                q.push_back(v);
                Some(q.len() - 1)
            }
            QueueContent::Shuffle { .. } => None,
        }
    }
    pub fn insert(&mut self, v: Self, index: usize) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(current, vec, _) => {
                if index <= vec.len() {
                    if *current >= index {
                        *current += 1;
                    }
                    vec.insert(index, v);
                    true
                } else {
                    false
                }
            }
            QueueContent::Loop(..) | QueueContent::Random(..) | QueueContent::Shuffle { .. } => {
                false
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
            QueueContent::Random(v) => v.iter().map(|v| v.len()).sum(),
            QueueContent::Loop(total, _done, inner) => {
                if *total == 0 {
                    inner.len()
                } else {
                    *total * inner.len()
                }
            }
            QueueContent::Shuffle { inner, state: _ } => inner.len(),
        }
    }

    /// recursively descends the queue until the current active element is found, then returns it.
    pub fn get_current(&self) -> Option<&Self> {
        match &self.content {
            QueueContent::Song(_) => Some(self),
            QueueContent::Folder(i, v, _) => {
                let i = *i;
                if let Some(v) = v.get(i) {
                    v.get_current()
                } else {
                    None
                }
            }
            QueueContent::Loop(_, _, inner) => inner.get_current(),
            QueueContent::Random(v) => v.get(v.len().saturating_sub(2))?.get_current(),
            QueueContent::Shuffle { inner, state: _ } => inner.get_current(),
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
            QueueContent::Song(_) => None,
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
            QueueContent::Loop(total, current, inner) => {
                if let Some(v) = inner.get_next() {
                    Some(v)
                } else if *total == 0 || current < total {
                    inner.get_first()
                } else {
                    None
                }
            }
            QueueContent::Random(v) => v.get(v.len().saturating_sub(1))?.get_current(),
            QueueContent::Shuffle { inner, state: _ } => inner.get_next(),
        }
    }
    pub fn get_first(&self) -> Option<&Self> {
        match &self.content {
            QueueContent::Song(..) => Some(self),
            QueueContent::Folder(_, v, _) => v.first(),
            QueueContent::Loop(_, _, q) => q.get_first(),
            QueueContent::Random(q) => q.front(),
            QueueContent::Shuffle { inner, state: _ } => inner.get_first(),
        }
    }

    pub fn advance_index_db(db: &mut Database) -> bool {
        let mut actions = vec![];
        let o = db.queue.advance_index_inner(vec![], &mut actions);
        Self::handle_actions(db, actions);
        o
    }
    pub fn init(&mut self, path: Vec<usize>, actions: &mut Vec<QueueAction>) {
        match &mut self.content {
            QueueContent::Song(..) => {}
            QueueContent::Folder(i, v, _) => {
                *i = 0;
                if let Some(v) = v.first_mut() {
                    v.init(
                        {
                            let mut p = path.clone();
                            p.push(0);
                            p
                        },
                        actions,
                    );
                }
            }
            QueueContent::Loop(_, _, inner) => inner.init(
                {
                    let mut p = path.clone();
                    p.push(0);
                    p
                },
                actions,
            ),
            QueueContent::Random(q) => {
                if q.len() == 0 {
                    actions.push(QueueAction::AddRandomSong(path.clone()));
                    actions.push(QueueAction::AddRandomSong(path.clone()));
                }
                if let Some(q) = q.get_mut(q.len().saturating_sub(2)) {
                    q.init(path, actions)
                }
            }
            QueueContent::Shuffle { inner, state } => {
                let mut p = path.clone();
                p.push(0);
                if matches!(state, ShuffleState::NotShuffled | ShuffleState::Modified) {
                    actions.push(QueueAction::SetShuffle(
                        path,
                        matches!(state, ShuffleState::Modified),
                    ));
                    *state = ShuffleState::Shuffled;
                }
                inner.init(p, actions);
            }
        }
    }
    pub fn handle_actions(db: &mut Database, actions: Vec<QueueAction>) {
        for action in actions {
            match action {
                QueueAction::AddRandomSong(path) => {
                    if !db.is_client() {
                        if let Some(song) = db.songs().keys().choose(&mut rand::thread_rng()) {
                            db.apply_command(Command::QueueAdd(
                                path,
                                QueueContent::Song(*song).into(),
                            ));
                        }
                    }
                }
                QueueAction::SetShuffle(path, partial) => {
                    if !db.is_client() {
                        let mut actions = vec![];
                        if let Some(QueueContent::Shuffle { inner, state: _ }) = db
                            .queue
                            .get_item_at_index_mut(&path, 0, &mut actions)
                            .map(|v| v.content_mut())
                        {
                            if let QueueContent::Folder(i, v, _) = inner.content_mut() {
                                let mut order = (0..v.len()).collect::<Vec<usize>>();
                                if partial && *i + 1 < v.len() {
                                    // shuffle only elements after the current one
                                    order[*i + 1..].shuffle(&mut rand::thread_rng());
                                } else {
                                    order.shuffle(&mut rand::thread_rng());
                                }
                                db.apply_command(Command::QueueSetShuffle(path, order));
                            }
                        }
                        Queue::handle_actions(db, actions);
                    }
                }
            }
        }
    }
    fn advance_index_inner(
        &mut self,
        mut path: Vec<usize>,
        actions: &mut Vec<QueueAction>,
    ) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(index, contents, _) => {
                if let Some(c) = contents.get_mut(*index) {
                    let mut p = path.clone();
                    p.push(*index);
                    if c.advance_index_inner(p, actions) {
                        // inner value could advance index, do nothing.
                        true
                    } else {
                        loop {
                            if *index + 1 < contents.len() {
                                // can advance
                                *index += 1;
                                if contents[*index].enabled {
                                    contents[*index].init(path, actions);
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
            QueueContent::Loop(total, current, inner) => {
                path.push(0);
                if inner.advance_index_inner(path.clone(), actions) {
                    true
                } else {
                    *current += 1;
                    if *total == 0 || *current < *total {
                        inner.init(path, actions);
                        true
                    } else {
                        *current = 0;
                        false
                    }
                }
            }
            QueueContent::Random(q) => {
                let i = q.len().saturating_sub(2);
                let mut p = path.clone();
                p.push(i);
                if q.get_mut(i)
                    .is_some_and(|inner| inner.advance_index_inner(p, actions))
                {
                    true
                } else {
                    if q.len() >= 2 {
                        q.pop_front();
                    }
                    // only sub 1 here because this is before the next random song is added
                    let i2 = q.len().saturating_sub(1);
                    if let Some(q) = q.get_mut(i2) {
                        let mut p = path.clone();
                        p.push(i2);
                        q.init(p, actions);
                    }
                    actions.push(QueueAction::AddRandomSong(path));
                    false
                }
            }
            QueueContent::Shuffle { inner, state } => {
                let mut p = path.clone();
                p.push(0);
                if !inner.advance_index_inner(p, actions) {
                    *state = ShuffleState::Shuffled;
                    actions.push(QueueAction::SetShuffle(path, false));
                    false
                } else {
                    true
                }
            }
        }
    }

    pub fn set_index_db(db: &mut Database, index: &Vec<usize>) {
        let mut actions = vec![];
        db.queue.reset_index();
        db.queue.set_index_inner(index, 0, vec![], &mut actions);
        Self::handle_actions(db, actions);
    }
    pub fn set_index_inner(
        &mut self,
        index: &Vec<usize>,
        depth: usize,
        mut build_index: Vec<usize>,
        actions: &mut Vec<QueueAction>,
    ) {
        let i = if let Some(i) = index.get(depth) {
            *i
        } else {
            return;
        };
        build_index.push(i);
        match &mut self.content {
            QueueContent::Song(_) => {}
            QueueContent::Folder(idx, contents, _) => {
                if i != *idx {
                    *idx = i;
                }
                if let Some(c) = contents.get_mut(i) {
                    c.init(build_index.clone(), actions);
                    c.set_index_inner(index, depth + 1, build_index, actions);
                }
            }
            QueueContent::Loop(_, _, inner) => {
                inner.init(build_index.clone(), actions);
                inner.set_index_inner(index, depth + 1, build_index, actions)
            }
            QueueContent::Random(_) => {}
            QueueContent::Shuffle { inner, state: _ } => {
                inner.init(build_index.clone(), actions);
                inner.set_index_inner(index, depth + 1, build_index, actions)
            }
        }
    }
    pub fn reset_index(&mut self) {
        match self.content_mut() {
            QueueContent::Song(_) => {}
            QueueContent::Folder(i, v, _) => {
                *i = 0;
                for v in v {
                    v.reset_index();
                }
            }
            QueueContent::Loop(_, done, i) => {
                *done = 0;
                i.reset_index();
            }
            QueueContent::Random(_) => {}
            QueueContent::Shuffle { inner, state: _ } => {
                inner.reset_index();
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
                QueueContent::Loop(_, _, inner) => inner.get_item_at_index(index, depth + 1),
                QueueContent::Random(vec) => vec.get(*i)?.get_item_at_index(index, depth + 1),
                QueueContent::Shuffle { inner, state: _ } => {
                    inner.get_item_at_index(index, depth + 1)
                }
            }
        } else {
            Some(self)
        }
    }
    pub fn get_item_at_index_mut(
        &mut self,
        index: &Vec<usize>,
        depth: usize,
        actions: &mut Vec<QueueAction>,
    ) -> Option<&mut Self> {
        if let Some(i) = index.get(depth) {
            match &mut self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(_, v, _) => {
                    if let Some(v) = v.get_mut(*i) {
                        v.get_item_at_index_mut(index, depth + 1, actions)
                    } else {
                        None
                    }
                }
                QueueContent::Loop(_, _, inner) => {
                    inner.get_item_at_index_mut(index, depth + 1, actions)
                }
                QueueContent::Random(vec) => {
                    vec.get_mut(*i)?
                        .get_item_at_index_mut(index, depth + 1, actions)
                }
                QueueContent::Shuffle { inner, state } => {
                    // if getting a mutable reference to the Folder that holds our songs,
                    // it may have been modified
                    if depth + 1 == index.len() && matches!(state, ShuffleState::Shuffled) {
                        *state = ShuffleState::Modified;
                    }
                    if matches!(state, ShuffleState::NotShuffled | ShuffleState::Modified) {
                        actions.push(QueueAction::SetShuffle(
                            index[0..depth].to_vec(),
                            matches!(state, ShuffleState::Modified),
                        ));
                        *state = ShuffleState::Shuffled;
                    }
                    inner.get_item_at_index_mut(index, depth + 1, actions)
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
                QueueContent::Loop(_, _, inner) => {
                    if depth + 1 < index.len() {
                        inner.remove_by_index(index, depth + 1)
                    } else {
                        None
                    }
                }
                QueueContent::Random(v) => v.remove(*i),
                QueueContent::Shuffle { inner, state: _ } => {
                    inner.remove_by_index(index, depth + 1)
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
            Self::Loop(total, current, inner) => {
                s.write_all(&[0b11000000])?;
                total.to_bytes(s)?;
                current.to_bytes(s)?;
                inner.to_bytes(s)?;
            }
            Self::Random(q) => {
                s.write_all(&[0b00110000])?;
                q.to_bytes(s)?;
            }
            Self::Shuffle { inner, state } => {
                s.write_all(&[0b00001100])?;
                inner.to_bytes(s)?;
                state.to_bytes(s)?;
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
        Ok(match switch_on[0] {
            0b11111111 => Self::Song(ToFromBytes::from_bytes(s)?),
            0b00000000 => Self::Folder(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
            ),
            0b11000000 => Self::Loop(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                Box::new(ToFromBytes::from_bytes(s)?),
            ),
            0b00110000 => Self::Random(ToFromBytes::from_bytes(s)?),
            0b00001100 => Self::Shuffle {
                inner: Box::new(ToFromBytes::from_bytes(s)?),
                state: ToFromBytes::from_bytes(s)?,
            },
            _ => Self::Folder(0, vec![], "<invalid byte received>".to_string()),
        })
    }
}
impl ToFromBytes for ShuffleState {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: std::io::Write,
    {
        s.write_all(&[match self {
            Self::NotShuffled => 1,
            Self::Modified => 2,
            Self::Shuffled => 4,
        }])
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::Read,
    {
        let mut b = [0];
        s.read_exact(&mut b)?;
        Ok(match b[0] {
            1 => Self::NotShuffled,
            2 => Self::Modified,
            4 => Self::Shuffled,
            _ => {
                eprintln!(
                    "[warn] received {} as ShuffleState, which is invalid. defaulting to Shuffled.",
                    b[0]
                );
                Self::Shuffled
            }
        })
    }
}
