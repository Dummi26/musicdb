use std::io::{Read, Write};

use crate::load::ToFromBytes;

use super::{AlbumId, ArtistId, CoverId, GeneralData, SongId};

#[derive(Clone, Debug, PartialEq)]
pub struct Album {
    pub id: AlbumId,
    pub name: String,
    pub artist: ArtistId,
    pub cover: Option<CoverId>,
    pub songs: Vec<SongId>,
    pub general: GeneralData,
}

impl ToFromBytes for Album {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.id.to_bytes(s)?;
        self.name.to_bytes(s)?;
        self.artist.to_bytes(s)?;
        self.songs.to_bytes(s)?;
        self.cover.to_bytes(s)?;
        self.general.to_bytes(s)?;
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(Self {
            id: ToFromBytes::from_bytes(s)?,
            name: ToFromBytes::from_bytes(s)?,
            artist: ToFromBytes::from_bytes(s)?,
            songs: ToFromBytes::from_bytes(s)?,
            cover: ToFromBytes::from_bytes(s)?,
            general: ToFromBytes::from_bytes(s)?,
        })
    }
}
