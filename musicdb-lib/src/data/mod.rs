use std::{
    io::{Read, Write},
    path::PathBuf,
};

use crate::load::ToFromBytes;

pub mod album;
pub mod artist;
pub mod database;
pub mod queue;
pub mod song;

pub type SongId = u64;
pub type AlbumId = u64;
pub type ArtistId = u64;
pub type CoverId = u64;

#[derive(Clone, Default, Debug)]
/// general data for songs, albums and artists
pub struct GeneralData {
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
/// the location of a file relative to the lib directory, often Artist/Album/Song.ext or similar
pub struct DatabaseLocation {
    pub rel_path: PathBuf,
}

impl ToFromBytes for DatabaseLocation {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.rel_path.to_bytes(s)
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(Self {
            rel_path: ToFromBytes::from_bytes(s)?,
        })
    }
}

impl<P> From<P> for DatabaseLocation
where
    P: Into<PathBuf>,
{
    fn from(value: P) -> Self {
        Self {
            rel_path: value.into(),
        }
    }
}

impl ToFromBytes for GeneralData {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.tags.to_bytes(s)?;
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(Self {
            tags: ToFromBytes::from_bytes(s)?,
        })
    }
}
