use std::{
    fmt::Display,
    str::{Chars, FromStr},
};

use musicdb_lib::data::{database::Database, song::Song, GeneralData};
use speedy2d::color::Color;

use crate::gui_text::Content;

#[derive(Debug)]
pub struct TextBuilder(pub Vec<TextPart>);
#[derive(Debug)]
pub enum TextPart {
    LineBreak,
    SetColor(Color),
    SetScale(f32),
    SetHeightAlign(f32),
    // - - - - -
    Literal(String),
    SongTitle,
    AlbumName,
    ArtistName,
    SongDuration(bool),
    /// Searches for a tag with exactly the provided value.
    /// Returns nothing or one of the following characters:
    /// `s` for Song, `a` for Album, and `A` for Artist.
    TagEq(String),
    /// Searches for a tag which starts with the provided string, then returns the end of it.
    /// If the search string is the entire tag, returns an empty string (which is not `nothing` because it is a TextPart::Literal, so it counts as `something` in an `if`).
    TagEnd(String),
    /// Searches for a tag which contains the provided string, then returns that tag's value.
    TagContains(String),
    /// If `1` is something, uses `2`.
    /// If `1` is nothing, uses `3`.
    If(TextBuilder, TextBuilder, TextBuilder),
}
impl TextBuilder {
    pub fn gen(&self, db: &Database, current_song: Option<&Song>) -> Vec<Vec<(Content, f32, f32)>> {
        let mut out = vec![];
        let mut line = vec![];
        self.gen_to(
            db,
            current_song,
            &mut out,
            &mut line,
            &mut 1.0,
            &mut 1.0,
            &mut Color::WHITE,
        );
        if !line.is_empty() {
            out.push(line)
        }
        out
    }
    pub fn gen_to(
        &self,
        db: &Database,
        current_song: Option<&Song>,
        out: &mut Vec<Vec<(Content, f32, f32)>>,
        line: &mut Vec<(Content, f32, f32)>,
        scale: &mut f32,
        align: &mut f32,
        color: &mut Color,
    ) {
        macro_rules! push {
            ($e:expr) => {
                line.push((Content::new($e, *color), *scale, *align))
            };
        }
        fn all_general<'a>(
            db: &'a Database,
            current_song: &'a Option<&'a Song>,
        ) -> [Option<&'a GeneralData>; 3] {
            if let Some(s) = current_song {
                if let Some(al) = s.album.and_then(|id| db.albums().get(&id)) {
                    if let Some(a) = db.artists().get(&s.artist) {
                        [Some(&s.general), Some(&al.general), Some(&a.general)]
                    } else {
                        [Some(&s.general), Some(&al.general), None]
                    }
                } else if let Some(a) = db.artists().get(&s.artist) {
                    [Some(&s.general), None, Some(&a.general)]
                } else {
                    [Some(&s.general), None, None]
                }
            } else {
                [None, None, None]
            }
        }
        for part in &self.0 {
            match part {
                TextPart::LineBreak => out.push(std::mem::replace(line, vec![])),
                TextPart::SetColor(c) => *color = *c,
                TextPart::SetScale(v) => *scale = *v,
                TextPart::SetHeightAlign(v) => *align = *v,
                TextPart::Literal(s) => push!(s.to_owned()),
                TextPart::SongTitle => {
                    if let Some(s) = current_song {
                        push!(s.title.to_owned());
                    }
                }
                TextPart::AlbumName => {
                    if let Some(s) = current_song {
                        if let Some(album) = s.album.and_then(|id| db.albums().get(&id)) {
                            push!(album.name.to_owned());
                        }
                    }
                }
                TextPart::ArtistName => {
                    if let Some(s) = current_song {
                        if let Some(artist) = db.artists().get(&s.artist) {
                            push!(artist.name.to_owned());
                        }
                    }
                }
                TextPart::SongDuration(show_millis) => {
                    if let Some(s) = current_song {
                        let seconds = s.duration_millis / 1000;
                        let minutes = seconds / 60;
                        let seconds = seconds % 60;
                        push!(if *show_millis {
                            let ms = s.duration_millis % 1000;
                            format!("{minutes}:{seconds:0>2}.{ms:0>4}")
                        } else {
                            format!("{minutes}:{seconds:0>2}")
                        });
                    }
                }
                TextPart::TagEq(p) => {
                    for (i, gen) in all_general(db, &current_song).into_iter().enumerate() {
                        if let Some(_) = gen.and_then(|gen| gen.tags.iter().find(|t| *t == p)) {
                            push!(match i {
                                0 => 's',
                                1 => 'a',
                                2 => 'A',
                                _ => unreachable!("array length should be 3"),
                            }
                            .to_string());
                            break;
                        }
                    }
                }
                TextPart::TagEnd(p) => {
                    for gen in all_general(db, &current_song) {
                        if let Some(t) =
                            gen.and_then(|gen| gen.tags.iter().find(|t| t.starts_with(p)))
                        {
                            push!(t[p.len()..].to_owned());
                            break;
                        }
                    }
                }
                TextPart::TagContains(p) => {
                    for gen in all_general(db, &current_song) {
                        if let Some(t) = gen.and_then(|gen| gen.tags.iter().find(|t| t.contains(p)))
                        {
                            push!(t.to_owned());
                            break;
                        }
                    }
                }
                TextPart::If(condition, yes, no) => {
                    if !condition.gen(db, current_song).is_empty() {
                        yes.gen_to(db, current_song, out, line, scale, align, color);
                    } else {
                        no.gen_to(db, current_song, out, line, scale, align, color);
                    }
                }
            }
        }
    }
}
impl FromStr for TextBuilder {
    type Err = TextBuilderParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_chars(&mut s.chars())
    }
}
impl TextBuilder {
    fn from_chars(chars: &mut Chars) -> Result<Self, TextBuilderParseError> {
        let mut vec = vec![];
        let mut current = String::new();
        macro_rules! done {
            () => {
                if !current.is_empty() {
                    // if it starts with at least one space, replace the first space with
                    // a No-Break space, as recommended in `https://github.com/QuantumBadger/Speedy2D/issues/45`,
                    // to avoid an issue where leading whitespaces are removed when drawing text.
                    if current.starts_with(' ') {
                        current = current.replacen(' ', "\u{00A0}", 1);
                    }
                    vec.push(TextPart::Literal(std::mem::replace(
                        &mut current,
                        String::new(),
                    )));
                }
            };
        }
        loop {
            if let Some(ch) = chars.next() {
                match ch {
                    '\n' => {
                        done!();
                        vec.push(TextPart::LineBreak);
                    }
                    '\\' => match chars.next() {
                        None => current.push('\\'),
                        Some('t') => {
                            done!();
                            vec.push(TextPart::SongTitle);
                        }
                        Some('a') => {
                            done!();
                            vec.push(TextPart::AlbumName);
                        }
                        Some('A') => {
                            done!();
                            vec.push(TextPart::ArtistName);
                        }
                        Some('d') => {
                            done!();
                            vec.push(TextPart::SongDuration(false));
                        }
                        Some('D') => {
                            done!();
                            vec.push(TextPart::SongDuration(true));
                        }
                        Some('s') => {
                            done!();
                            vec.push(TextPart::SetScale({
                                let mut str = String::new();
                                loop {
                                    match chars.next() {
                                        None | Some(';') => break,
                                        Some(c) => str.push(c),
                                    }
                                }
                                if let Ok(v) = str.parse() {
                                    v
                                } else {
                                    return Err(TextBuilderParseError::CouldntParse(
                                        str,
                                        "number (float)".to_string(),
                                    ));
                                }
                            }))
                        }
                        Some('h') => {
                            done!();
                            vec.push(TextPart::SetHeightAlign({
                                let mut str = String::new();
                                loop {
                                    match chars.next() {
                                        None | Some(';') => break,
                                        Some(c) => str.push(c),
                                    }
                                }
                                if let Ok(v) = str.parse() {
                                    v
                                } else {
                                    return Err(TextBuilderParseError::CouldntParse(
                                        str,
                                        "number (float)".to_string(),
                                    ));
                                }
                            }))
                        }
                        Some('c') => {
                            done!();
                            vec.push(TextPart::SetColor({
                                let mut str = String::new();
                                for _ in 0..6 {
                                    if let Some(ch) = chars.next() {
                                        str.push(ch);
                                    } else {
                                        return Err(TextBuilderParseError::TooFewCharsForColor);
                                    }
                                }
                                if let Ok(i) = u32::from_str_radix(&str, 16) {
                                    Color::from_hex_rgb(i)
                                } else {
                                    return Err(TextBuilderParseError::ColorNotHex);
                                }
                            }));
                        }
                        Some(ch) => current.push(ch),
                    },
                    '%' => {
                        done!();
                        let mode = if let Some(ch) = chars.next() {
                            ch
                        } else {
                            return Err(TextBuilderParseError::UnclosedPercent);
                        };
                        loop {
                            match chars.next() {
                                Some('%') => {
                                    let s = std::mem::replace(&mut current, String::new());
                                    vec.push(match mode {
                                        '=' => TextPart::TagEq(s),
                                        '>' => TextPart::TagEnd(s),
                                        '_' => TextPart::TagContains(s),
                                        c => return Err(TextBuilderParseError::TagModeUnknown(c)),
                                    });
                                    break;
                                }
                                Some(ch) => current.push(ch),
                                None => return Err(TextBuilderParseError::UnclosedPercent),
                            }
                        }
                    }
                    '?' => {
                        done!();
                        vec.push(TextPart::If(
                            Self::from_chars(chars)?,
                            Self::from_chars(chars)?,
                            Self::from_chars(chars)?,
                        ));
                    }
                    '#' => break,
                    ch => current.push(ch),
                }
            } else {
                break;
            }
        }
        done!();
        Ok(Self(vec))
    }
}
#[derive(Debug)]
pub enum TextBuilderParseError {
    UnclosedPercent,
    TagModeUnknown(char),
    TooFewCharsForColor,
    ColorNotHex,
    CouldntParse(String, String),
}
impl Display for TextBuilderParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnclosedPercent => write!(
                f,
                "Unclosed %: Syntax is %<mode><search>%, where <mode> is _, >, or =."
            ),
            Self::TagModeUnknown(mode) => {
                write!(f, "Unknown tag mode '{mode}': Allowed are only _, > or =.")
            }
            Self::TooFewCharsForColor => write!(f, "Too few chars for color: Syntax is \\cRRGGBB."),
            Self::ColorNotHex => write!(f, "Color value wasn't a hex number! Syntax is \\cRRGGBB, where R, G, and B are values from 0-9 and A-F (hex 0-F)."),
            Self::CouldntParse(v, t) => write!(f, "Couldn't parse value '{v}' to type '{t}'."),
        }
    }
}
