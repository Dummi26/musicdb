use std::{
    io::{Cursor, Read, Write},
    thread::{self, JoinHandle},
};

use musicdb_lib::{
    data::{queue::QueueContent, CoverId, SongId},
    server::{get, Command},
};
use speedy2d::{
    color::Color, dimen::Vec2, image::ImageHandle, shape::Rectangle, window::MouseButton,
};

use crate::{
    gui::{adjust_area, adjust_pos, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_text::Label,
};

/*

Components for the StatusBar.
This file could probably have a better name.

*/

pub struct CurrentSong<T: Read + Write> {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    get_con: Option<get::Client<T>>,
    prev_song: Option<SongId>,
    cover_pos: Rectangle,
    cover_id: Option<CoverId>,
    cover: Option<ImageHandle>,
    new_cover: Option<JoinHandle<(get::Client<T>, Option<Vec<u8>>)>>,
}
impl<T: Read + Write> Clone for CurrentSong<T> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            children: self.children.clone(),
            get_con: None,
            prev_song: None,
            cover_pos: self.cover_pos.clone(),
            cover_id: None,
            cover: None,
            new_cover: None,
        }
    }
}
impl<T: Read + Write + 'static + Sync + Send> CurrentSong<T> {
    pub fn new(config: GuiElemCfg, get_con: get::Client<T>) -> Self {
        Self {
            config,
            children: vec![
                GuiElem::new(Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.4, 0.0), (1.0, 0.5))),
                    "".to_owned(),
                    Color::from_int_rgb(180, 180, 210),
                    None,
                    Vec2::new(0.0, 1.0),
                )),
                GuiElem::new(Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.4, 0.5), (1.0, 1.0))),
                    "".to_owned(),
                    Color::from_int_rgb(120, 120, 120),
                    None,
                    Vec2::new(0.0, 0.0),
                )),
            ],
            get_con: Some(get_con),
            cover_pos: Rectangle::new(Vec2::ZERO, Vec2::ZERO),
            cover_id: None,
            prev_song: None,
            cover: None,
            new_cover: None,
        }
    }
}
impl<T: Read + Write + 'static + Sync + Send> GuiElemTrait for CurrentSong<T> {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut GuiElem> + '_> {
        Box::new(self.children.iter_mut())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn clone_gui(&self) -> Box<dyn GuiElemTrait> {
        Box::new(self.clone())
    }
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        // check if there is a new song
        let new_song = if let Some(song) = info.database.queue.get_current_song() {
            if Some(*song) == self.prev_song {
                // same song as before
                None
            } else {
                Some(Some(*song))
            }
        } else if self.prev_song.is_none() {
            // no song, nothing in queue
            None
        } else {
            self.cover = None;
            Some(None)
        };
        // drawing stuff
        if self.config.pixel_pos.size() != info.pos.size() {
            let leftright = 0.05;
            let topbottom = 0.05;
            let mut width = 0.3;
            let mut height = 1.0 - topbottom * 2.0;
            if width * info.pos.width() < height * info.pos.height() {
                height = width * info.pos.width() / info.pos.height();
            } else {
                width = height * info.pos.height() / info.pos.width();
            }
            let right = leftright + width + leftright;
            self.cover_pos = Rectangle::from_tuples(
                (leftright, 0.5 - 0.5 * height),
                (leftright + width, 0.5 + 0.5 * height),
            );
            for el in self.children.iter_mut().take(2) {
                let pos = &mut el.inner.config_mut().pos;
                *pos = Rectangle::new(Vec2::new(right, pos.top_left().y), *pos.bottom_right());
            }
        }
        if self.new_cover.as_ref().is_some_and(|v| v.is_finished()) {
            let (get_con, cover) = self.new_cover.take().unwrap().join().unwrap();
            self.get_con = Some(get_con);
            if let Some(cover) = cover {
                self.cover = g
                    .create_image_from_file_bytes(
                        None,
                        speedy2d::image::ImageSmoothingMode::Linear,
                        Cursor::new(cover),
                    )
                    .ok();
            }
        }
        if let Some(cover) = &self.cover {
            g.draw_rectangle_image(
                Rectangle::new(
                    Vec2::new(
                        info.pos.top_left().x + info.pos.width() * self.cover_pos.top_left().x,
                        info.pos.top_left().y + info.pos.height() * self.cover_pos.top_left().y,
                    ),
                    Vec2::new(
                        info.pos.top_left().x + info.pos.width() * self.cover_pos.bottom_right().x,
                        info.pos.top_left().y + info.pos.height() * self.cover_pos.bottom_right().y,
                    ),
                ),
                cover,
            );
        }
        if let Some(new_song) = new_song {
            // if there is a new song:
            if self.prev_song != new_song {
                self.config.redraw = true;
                self.prev_song = new_song;
            }
            if self.config.redraw {
                self.config.redraw = false;
                let (name, subtext) = if let Some(song) = new_song {
                    if let Some(song) = info.database.get_song(&song) {
                        let cover = if let Some(v) = song.cover {
                            Some(v)
                        } else if let Some(v) = song
                            .album
                            .as_ref()
                            .and_then(|id| info.database.albums().get(id))
                            .and_then(|album| album.cover)
                        {
                            Some(v)
                        } else {
                            None
                        };
                        if cover != self.cover_id {
                            self.cover = None;
                            if let Some(cover) = cover {
                                if let Some(mut get_con) = self.get_con.take() {
                                    self.new_cover = Some(thread::spawn(move || {
                                        match get_con.cover_bytes(cover).unwrap() {
                                            Ok(v) => (get_con, Some(v)),
                                            Err(e) => {
                                                eprintln!("couldn't get cover (response: {e})");
                                                (get_con, None)
                                            }
                                        }
                                    }));
                                }
                            }
                            self.cover_id = cover;
                        }
                        let sub = match (
                            song.artist
                                .as_ref()
                                .and_then(|id| info.database.artists().get(id)),
                            song.album
                                .as_ref()
                                .and_then(|id| info.database.albums().get(id)),
                        ) {
                            (None, None) => String::new(),
                            (Some(artist), None) => format!("by {}", artist.name),
                            (None, Some(album)) => {
                                if let Some(artist) = album
                                    .artist
                                    .as_ref()
                                    .and_then(|id| info.database.artists().get(id))
                                {
                                    format!("on {} by {}", album.name, artist.name)
                                } else {
                                    format!("on {}", album.name)
                                }
                            }
                            (Some(artist), Some(album)) => {
                                format!("by {} on {}", artist.name, album.name)
                            }
                        };
                        (song.title.clone(), sub)
                    } else {
                        (
                            "< song not in db >".to_owned(),
                            "maybe restart the client to resync the database?".to_owned(),
                        )
                    }
                } else {
                    (String::new(), String::new())
                };
                *self.children[0]
                    .try_as_mut::<Label>()
                    .unwrap()
                    .content
                    .text() = name;
                *self.children[1]
                    .try_as_mut::<Label>()
                    .unwrap()
                    .content
                    .text() = subtext;
            }
        }
    }
}

#[derive(Clone)]
pub struct PlayPauseToggle {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    playing_target: bool,
    playing_waiting_for_change: bool,
}
impl PlayPauseToggle {
    /// automatically adds w_mouse to config
    pub fn new(config: GuiElemCfg, playing: bool) -> Self {
        Self {
            config: config.w_mouse(),
            children: vec![],
            playing_target: playing,
            playing_waiting_for_change: false,
        }
    }
}
impl GuiElemTrait for PlayPauseToggle {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut GuiElem> + '_> {
        Box::new(self.children.iter_mut())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn clone_gui(&self) -> Box<dyn GuiElemTrait> {
        Box::new(self.clone())
    }
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        if self.playing_waiting_for_change {
            if info.database.playing == self.playing_target {
                self.playing_waiting_for_change = false;
            }
        } else {
            // not waiting for change, update if the value changes
            self.playing_target = info.database.playing;
        }
        let pos = if info.pos.width() > info.pos.height() {
            let a = 0.5 * info.pos.height();
            let m = 0.5 * (info.pos.top_left().x + info.pos.bottom_right().x);
            Rectangle::new(
                Vec2::new(m - a, info.pos.top_left().y),
                Vec2::new(m + a, info.pos.bottom_right().y),
            )
        } else {
            let a = 0.5 * info.pos.width();
            let m = 0.5 * (info.pos.top_left().y + info.pos.bottom_right().y);
            Rectangle::new(
                Vec2::new(info.pos.top_left().x, m - a),
                Vec2::new(info.pos.bottom_right().x, m + a),
            )
        };
        if self.playing_target {
            g.draw_triangle(
                [
                    adjust_pos(&pos, &Vec2::new(0.25, 0.25)),
                    adjust_pos(&pos, &Vec2::new(0.75, 0.5)),
                    adjust_pos(&pos, &Vec2::new(0.25, 0.75)),
                ],
                if self.playing_waiting_for_change {
                    Color::GRAY
                } else {
                    Color::GREEN
                },
            )
        } else {
            g.draw_rectangle(
                adjust_area(&pos, &Rectangle::from_tuples((0.25, 0.25), (0.75, 0.75))),
                if self.playing_waiting_for_change {
                    Color::RED
                } else {
                    Color::GRAY
                },
            );
        }
    }
    fn mouse_pressed(&mut self, button: MouseButton) -> Vec<GuiAction> {
        match button {
            MouseButton::Left => {
                if !self.playing_waiting_for_change {
                    self.playing_target = !self.playing_target;
                    self.playing_waiting_for_change = true;
                    vec![GuiAction::SendToServer(if self.playing_target {
                        Command::Resume
                    } else {
                        Command::Pause
                    })]
                } else {
                    vec![]
                }
            }
            MouseButton::Right => vec![GuiAction::SendToServer(Command::NextSong)],
            _ => vec![],
        }
    }
}
