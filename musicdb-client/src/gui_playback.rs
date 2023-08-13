use musicdb_lib::{
    data::{queue::QueueContent, SongId},
    server::Command,
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, window::MouseButton};

use crate::{
    gui::{adjust_area, adjust_pos, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_text::Label,
};

#[derive(Clone)]
pub struct CurrentSong {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    prev_song: Option<SongId>,
}
impl CurrentSong {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config,
            children: vec![
                GuiElem::new(Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.5))),
                    "".to_owned(),
                    Color::from_int_rgb(180, 180, 210),
                    None,
                    Vec2::new(0.1, 1.0),
                )),
                GuiElem::new(Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.5), (0.5, 1.0))),
                    "".to_owned(),
                    Color::from_int_rgb(120, 120, 120),
                    None,
                    Vec2::new(0.3, 0.0),
                )),
            ],

            prev_song: None,
        }
    }
}
impl GuiElemTrait for CurrentSong {
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
        let song = if let Some(v) = info.database.queue.get_current() {
            if let QueueContent::Song(song) = v.content() {
                if Some(*song) == self.prev_song {
                    // same song as before
                    return;
                } else {
                    Some(*song)
                }
            } else if self.prev_song.is_none() {
                // no song, nothing in queue
                return;
            } else {
                None
            }
        } else if self.prev_song.is_none() {
            // no song, nothing in queue
            return;
        } else {
            None
        };
        if self.prev_song != song {
            self.config.redraw = true;
            self.prev_song = song;
        }
        if self.config.redraw {
            self.config.redraw = false;
            let (name, subtext) = if let Some(song) = song {
                if let Some(song) = info.database.get_song(&song) {
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
}
