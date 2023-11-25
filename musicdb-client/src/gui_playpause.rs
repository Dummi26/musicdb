use musicdb_lib::server::Command;
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg},
    gui_base::{Button, Panel},
};

pub struct PlayPause {
    config: GuiElemCfg,
    to_zero: Button<[Panel<()>; 1]>,
    play_pause: Button<[PlayPauseDisplay; 1]>,
    to_end: Button<[NextSongShape; 1]>,
}

impl PlayPause {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config,
            to_zero: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.3, 1.0))),
                |_| vec![GuiAction::SendToServer(Command::Stop)],
                [Panel::with_background(
                    GuiElemCfg::at(Rectangle::from_tuples((0.2, 0.2), (0.8, 0.8))),
                    (),
                    Color::MAGENTA,
                )],
            ),
            play_pause: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.35, 0.0), (0.65, 1.0))),
                |btn| {
                    vec![GuiAction::SendToServer(if btn.children[0].is_playing {
                        Command::Pause
                    } else {
                        Command::Resume
                    })]
                },
                [PlayPauseDisplay::new(GuiElemCfg::at(
                    Rectangle::from_tuples((0.2, 0.2), (0.8, 0.8)),
                ))],
            ),
            to_end: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.7, 0.0), (1.0, 1.0))),
                |_| vec![GuiAction::SendToServer(Command::NextSong)],
                [NextSongShape::new(GuiElemCfg::at(Rectangle::from_tuples(
                    (0.2, 0.2),
                    (0.8, 0.8),
                )))],
            ),
        }
    }
}

struct PlayPauseDisplay {
    config: GuiElemCfg,
    is_playing: bool,
}
impl PlayPauseDisplay {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config,
            is_playing: false,
        }
    }
}
impl GuiElem for PlayPauseDisplay {
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        self.is_playing = info.database.playing;
        if info.database.playing {
            g.draw_rectangle(
                Rectangle::from_tuples(
                    (
                        info.pos.top_left().x + info.pos.width() * 0.2,
                        info.pos.top_left().y,
                    ),
                    (
                        info.pos.top_left().x + info.pos.width() * 0.4,
                        info.pos.bottom_right().y,
                    ),
                ),
                Color::BLUE,
            );
            g.draw_rectangle(
                Rectangle::from_tuples(
                    (
                        info.pos.bottom_right().x - info.pos.width() * 0.4,
                        info.pos.top_left().y,
                    ),
                    (
                        info.pos.bottom_right().x - info.pos.width() * 0.2,
                        info.pos.bottom_right().y,
                    ),
                ),
                Color::BLUE,
            );
        } else {
            g.draw_triangle(
                [
                    *info.pos.top_left(),
                    Vec2::new(
                        info.pos.bottom_right().x,
                        (info.pos.top_left().y + info.pos.bottom_right().y) / 2.0,
                    ),
                    info.pos.bottom_left(),
                ],
                Color::GREEN,
            );
        }
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([].into_iter())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn elem(&self) -> &dyn GuiElem {
        self
    }
    fn elem_mut(&mut self) -> &mut dyn GuiElem {
        self
    }
}

struct NextSongShape {
    config: GuiElemCfg,
}
impl NextSongShape {
    pub fn new(config: GuiElemCfg) -> Self {
        Self { config }
    }
}
impl GuiElem for NextSongShape {
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        let top = *info.pos.top_left();
        let bottom = info.pos.bottom_left();
        let right = Vec2::new(info.pos.bottom_right().x, (top.y + bottom.y) / 2.0);
        g.draw_triangle([top, right, bottom], Color::CYAN);
        let half_width = info.pos.width() * 0.04;
        let top_right = Vec2::new(info.pos.top_right().x - half_width, info.pos.top_left().y);
        let bottom_right = Vec2::new(
            info.pos.top_right().x - half_width,
            info.pos.bottom_right().y,
        );
        g.draw_line(top_right, bottom_right, 2.0 * half_width, Color::CYAN);
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([].into_iter())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn elem(&self) -> &dyn GuiElem {
        self
    }
    fn elem_mut(&mut self) -> &mut dyn GuiElem {
        self
    }
}

impl GuiElem for PlayPause {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.to_zero.elem_mut(),
                self.play_pause.elem_mut(),
                self.to_end.elem_mut(),
            ]
            .into_iter(),
        )
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn elem(&self) -> &dyn GuiElem {
        self
    }
    fn elem_mut(&mut self) -> &mut dyn GuiElem {
        self
    }
}
