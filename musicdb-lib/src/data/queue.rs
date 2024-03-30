use std::ops::AddAssign;

use crate::load::ToFromBytes;

use super::{database::Database, SongId};

#[derive(Clone, Debug)]
pub struct Queue {
    enabled: bool,
    content: QueueContent,
}
#[derive(Clone, Debug)]
pub enum QueueContent {
    Song(SongId),
    Folder(QueueFolder),
    Loop(usize, usize, Box<Queue>),
}
#[derive(Clone, Debug, Default)]
pub struct QueueFolder {
    pub index: usize,
    pub content: Vec<Queue>,
    pub name: String,
    pub order: Option<Vec<usize>>,
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

    pub fn add_to_end(&mut self, v: Vec<Self>) -> Option<usize> {
        match &mut self.content {
            QueueContent::Song(_) => None,
            QueueContent::Folder(folder) => folder.add_to_end(v),
            QueueContent::Loop(..) => None,
        }
    }
    pub fn insert(&mut self, v: Vec<Self>, index: usize) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(folder) => folder.insert(v, index),
            QueueContent::Loop(..) => false,
        }
    }

    pub fn len(&self) -> usize {
        if !self.enabled {
            return 0;
        }
        match &self.content {
            QueueContent::Song(_) => 1,
            QueueContent::Folder(folder) => folder.len(),
            QueueContent::Loop(total, _done, inner) => {
                if *total == 0 {
                    inner.len()
                } else {
                    total.saturating_mul(inner.len())
                }
            }
        }
    }
    pub fn duration_total(&self, db: &Database) -> QueueDuration {
        let mut dur = QueueDuration::new_total();
        self.add_duration(&mut dur, db);
        dur
    }
    // remaining time, including current song
    pub fn duration_remaining(&self, db: &Database) -> QueueDuration {
        let mut dur = QueueDuration::new_remaining();
        self.add_duration(&mut dur, db);
        dur
    }
    pub fn add_duration(&self, dur: &mut QueueDuration, db: &Database) {
        if self.enabled {
            match &self.content {
                QueueContent::Song(v) => {
                    dur.millis += db.get_song(v).map(|s| s.duration_millis).unwrap_or(0)
                }
                QueueContent::Folder(QueueFolder {
                    index,
                    content,
                    name: _,
                    order: _,
                }) => {
                    for (i, inner) in content.iter().enumerate() {
                        if dur.include_past || i >= *index {
                            inner.add_duration(dur, db);
                        }
                    }
                }
                QueueContent::Loop(total, done, inner) => {
                    if *total == 0 {
                        dur.infinite = true;
                    } else if dur.include_past {
                        // <total duration> * <total iterations>
                        let dt = inner.duration_total(db);
                        for _ in 0..*total {
                            *dur += dt;
                        }
                    } else {
                        // <remaining duration> + <total duration> * <remaining iterations>
                        inner.add_duration(dur, db);
                        let dt = inner.duration_total(db);
                        for _ in 0..(total.saturating_sub(*done + 1)) {
                            *dur += dt;
                        }
                    }
                }
            }
        }
    }

    /// recursively descends the queue until the current active element is found, then returns it.
    pub fn get_current(&self) -> Option<&Self> {
        match &self.content {
            QueueContent::Song(_) => Some(self),
            QueueContent::Folder(folder) => folder.get_current_immut()?.get_current(),
            QueueContent::Loop(_, _, inner) => inner.get_current(),
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
            QueueContent::Folder(folder) => folder.get_next(),
            QueueContent::Loop(total, current, inner) => {
                if let Some(v) = inner.get_next() {
                    Some(v)
                } else if *total == 0 || current < total {
                    inner.get_first()
                } else {
                    None
                }
            }
        }
    }
    pub fn get_first(&self) -> Option<&Self> {
        match &self.content {
            QueueContent::Song(..) => Some(self),
            QueueContent::Folder(folder) => folder.get_first(),
            QueueContent::Loop(_, _, q) => q.get_first(),
        }
    }

    pub fn advance_index_db(db: &mut Database) -> bool {
        let o = db.queue.advance_index_inner();
        o
    }
    pub fn init(&mut self) {
        match &mut self.content {
            QueueContent::Song(..) => {}
            QueueContent::Folder(folder) => {
                folder.index = 0;
                for v in &mut folder.content {
                    v.init();
                }
            }
            QueueContent::Loop(_, _, inner) => inner.init(),
        }
    }
    pub fn advance_index_inner(&mut self) -> bool {
        match &mut self.content {
            QueueContent::Song(_) => false,
            QueueContent::Folder(folder) => folder.advance_index_inner(),
            QueueContent::Loop(total, current, inner) => {
                if inner.advance_index_inner() {
                    true
                } else {
                    *current += 1;
                    if *total == 0 || *current < *total {
                        inner.init();
                        true
                    } else {
                        *current = 0;
                        false
                    }
                }
            }
        }
    }

    pub fn set_index_db(db: &mut Database, index: &Vec<usize>) {
        db.queue.reset_index();
        db.queue.set_index_inner(index, 0, vec![]);
    }
    pub fn set_index_inner(
        &mut self,
        index: &Vec<usize>,
        depth: usize,
        mut build_index: Vec<usize>,
    ) {
        let i = if let Some(i) = index.get(depth) {
            *i
        } else {
            return;
        };
        build_index.push(i);
        match &mut self.content {
            QueueContent::Song(_) => {}
            QueueContent::Folder(folder) => {
                folder.index = i;
                if let Some(c) = folder.get_current_mut() {
                    c.init();
                    c.set_index_inner(index, depth + 1, build_index);
                }
            }
            QueueContent::Loop(_, _, inner) => {
                inner.init();
                inner.set_index_inner(index, depth + 1, build_index)
            }
        }
    }
    pub fn reset_index(&mut self) {
        match self.content_mut() {
            QueueContent::Song(_) => {}
            QueueContent::Folder(folder) => {
                folder.index = 0;
                for v in &mut folder.content {
                    v.reset_index();
                }
            }
            QueueContent::Loop(_, done, i) => {
                *done = 0;
                i.reset_index();
            }
        }
    }

    pub fn get_item_at_index(&self, index: &Vec<usize>, depth: usize) -> Option<&Self> {
        if let Some(i) = index.get(depth) {
            match &self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(folder) => {
                    if let Some(v) = folder.get_at(*i) {
                        v.get_item_at_index(index, depth + 1)
                    } else {
                        None
                    }
                }
                QueueContent::Loop(_, _, inner) => inner.get_item_at_index(index, depth + 1),
            }
        } else {
            Some(self)
        }
    }
    pub fn get_item_at_index_mut(&mut self, index: &Vec<usize>, depth: usize) -> Option<&mut Self> {
        if let Some(i) = index.get(depth) {
            match &mut self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(folder) => {
                    if let Some(v) = folder.get_mut_at(*i) {
                        v.get_item_at_index_mut(index, depth + 1)
                    } else {
                        None
                    }
                }
                QueueContent::Loop(_, _, inner) => inner.get_item_at_index_mut(index, depth + 1),
            }
        } else {
            Some(self)
        }
    }

    pub fn remove_by_index(&mut self, index: &Vec<usize>, depth: usize) -> Option<Self> {
        if let Some(i) = index.get(depth) {
            match &mut self.content {
                QueueContent::Song(_) => None,
                QueueContent::Folder(folder) => {
                    if depth + 1 < index.len() {
                        if let Some(v) = folder.get_mut_at(*i) {
                            v.remove_by_index(index, depth + 1)
                        } else {
                            None
                        }
                    } else {
                        if *i < folder.content.len() {
                            // if current playback is past this point,
                            // reduce the index by 1 so that it still points to the same element
                            if folder.index > *i {
                                folder.index -= 1;
                            }
                            let idx = if let Some(order) = &mut folder.order {
                                let idx = order.remove(*i);
                                // compensate for removal of element from .content
                                for o in order {
                                    if *o > idx {
                                        *o -= 1;
                                    }
                                }
                                idx
                            } else {
                                *i
                            };
                            Some(folder.content.remove(idx))
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
            }
        } else {
            None
        }
    }
}

impl QueueFolder {
    pub fn iter(&self) -> QueueFolderIter {
        QueueFolderIter {
            folder: self,
            index: 0,
        }
    }
    pub fn add_to_end(&mut self, v: Vec<Queue>) -> Option<usize> {
        let add_len = v.len();
        let len = self.content.len();
        for mut v in v.into_iter() {
            v.init();
            self.content.push(v);
        }
        if let Some(order) = &mut self.order {
            for i in 0..add_len {
                order.push(len + i);
            }
        }
        Some(len)
    }
    pub fn insert(&mut self, v: Vec<Queue>, index: usize) -> bool {
        if index <= self.content.len() {
            if self.index >= index {
                self.index += v.len();
            }
            fn insert_multiple<T>(index: usize, vec: &mut Vec<T>, v: impl IntoIterator<Item = T>) {
                // remove the elements starting at the insertion point
                let end = vec.split_off(index);
                // insert new elements
                for v in v {
                    vec.push(v);
                }
                // re-add previously removed elements
                vec.extend(end);
            }
            let mapfunc = |mut v: Queue| {
                v.init();
                v
            };
            if let Some(order) = &mut self.order {
                insert_multiple(index, order, (0..v.len()).map(|i| self.content.len() + i));
                self.content.extend(v.into_iter().map(mapfunc));
            } else {
                insert_multiple(index, &mut self.content, v.into_iter().map(mapfunc));
            }
            true
        } else {
            false
        }
    }
    pub fn len(&self) -> usize {
        self.content.iter().map(|v| v.len()).sum()
    }
    pub fn get_at(&self, mut i: usize) -> Option<&Queue> {
        if let Some(order) = &self.order {
            i = *order.get(i)?;
        }
        self.content.get(i)
    }
    pub fn get_mut_at(&mut self, mut i: usize) -> Option<&mut Queue> {
        if let Some(order) = &self.order {
            i = *order.get(i)?;
        }
        self.content.get_mut(i)
    }
    pub fn get_current_immut(&self) -> Option<&Queue> {
        self.get_at(self.index)
    }
    pub fn get_current_mut(&mut self) -> Option<&mut Queue> {
        self.get_mut_at(self.index)
    }
    pub fn get_next(&self) -> Option<&Queue> {
        if let Some(v) = self.get_current_immut() {
            if let Some(v) = v.get_next() {
                Some(v)
            } else {
                if let Some(v) = self.get_at(self.index + 1) {
                    v.get_current()
                } else {
                    None
                }
            }
        } else {
            None
        }
    }
    pub fn get_first(&self) -> Option<&Queue> {
        if let Some(order) = &self.order {
            self.content.get(*order.first()?)
        } else {
            self.content.first()
        }
    }
    pub fn advance_index_inner(&mut self) -> bool {
        if let Some(c) = self.get_current_mut() {
            if c.advance_index_inner() {
                // inner value could advance index, do nothing.
                true
            } else {
                loop {
                    if self.index + 1 < self.content.len() {
                        // can advance
                        self.index += 1;
                        if self.content[self.index].enabled {
                            self.content[self.index].init();
                            break true;
                        }
                    } else {
                        // can't advance: index would be out of bounds
                        self.index = 0;
                        break false;
                    }
                }
            }
        } else {
            self.index = 0;
            false
        }
    }
}
pub struct QueueFolderIter<'a> {
    folder: &'a QueueFolder,
    index: usize,
}
impl<'a> Iterator for QueueFolderIter<'a> {
    type Item = &'a Queue;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(v) = self.folder.get_at(self.index) {
            self.index += 1;
            Some(v)
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
            Self::Folder(folder) => {
                s.write_all(&[0b00000000])?;
                ToFromBytes::to_bytes(folder, s)?;
            }
            Self::Loop(total, current, inner) => {
                s.write_all(&[0b11000000])?;
                total.to_bytes(s)?;
                current.to_bytes(s)?;
                inner.to_bytes(s)?;
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
            0b00000000 => Self::Folder(ToFromBytes::from_bytes(s)?),
            0b11000000 => Self::Loop(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                Box::new(ToFromBytes::from_bytes(s)?),
            ),
            _ => Self::Folder(QueueFolder {
                index: 0,
                content: vec![],
                name: "<invalid byte received>".to_string(),
                order: None,
            }),
        })
    }
}
impl ToFromBytes for QueueFolder {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: std::io::prelude::Write,
    {
        ToFromBytes::to_bytes(&self.index, s)?;
        ToFromBytes::to_bytes(&self.content, s)?;
        ToFromBytes::to_bytes(&self.name, s)?;
        ToFromBytes::to_bytes(&self.order, s)?;
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::prelude::Read,
    {
        let v = Self {
            index: ToFromBytes::from_bytes(s)?,
            content: ToFromBytes::from_bytes(s)?,
            name: ToFromBytes::from_bytes(s)?,
            order: ToFromBytes::from_bytes(s)?,
        };
        Ok(v)
    }
}

#[derive(Clone, Copy)]
pub struct QueueDuration {
    pub include_past: bool,
    pub infinite: bool,
    /// number of milliseconds (that we know of)
    pub millis: u64,
    /// number of milliseconds from the <random> element - only accurate the first time it is reached in queue.
    pub random_known_millis: u64,
    /// number of <random> elements, which could have pretty much any duration.
    pub random_counter: u64,
}
impl QueueDuration {
    fn new_total() -> Self {
        Self::new(true)
    }
    fn new_remaining() -> Self {
        Self::new(false)
    }
    fn new(include_past: bool) -> Self {
        QueueDuration {
            include_past,
            infinite: false,
            millis: 0,
            random_known_millis: 0,
            random_counter: 0,
        }
    }
}
impl AddAssign<QueueDuration> for QueueDuration {
    fn add_assign(&mut self, rhs: QueueDuration) {
        if rhs.infinite {
            self.infinite = true;
        }
        self.millis += rhs.millis;
        self.random_known_millis += rhs.random_known_millis;
        self.random_counter += rhs.random_counter;
    }
}
