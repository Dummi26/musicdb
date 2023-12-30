use std::sync::{atomic::AtomicBool, Arc};

use musicdb_lib::server::Command;
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg},
    gui_base::{Button, Panel},
};

pub struct PlayPause {
    config: GuiElemCfg,
    set_fav: Button<[FavIcon; 1]>,
    to_zero: Button<[Panel<()>; 1]>,
    play_pause: Button<[PlayPauseDisplay; 1]>,
    to_end: Button<[NextSongShape; 1]>,
}

impl PlayPause {
    pub fn new(config: GuiElemCfg, is_fav: Arc<AtomicBool>) -> Self {
        Self {
            config,
            set_fav: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.01, 0.01), (0.24, 0.99))),
                |_| {
                    vec![GuiAction::Build(Box::new(|db| {
                        if let Some(song_id) = db.queue.get_current_song() {
                            if let Some(song) = db.get_song(song_id) {
                                vec![GuiAction::SendToServer(
                                    if song.general.tags.iter().any(|v| v == "Fav") {
                                        Command::TagSongFlagUnset(*song_id, "Fav".to_owned())
                                    } else {
                                        Command::TagSongFlagSet(*song_id, "Fav".to_owned())
                                    },
                                )]
                            } else {
                                vec![]
                            }
                        } else {
                            vec![]
                        }
                    }))]
                },
                [FavIcon::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.2, 0.2), (0.8, 0.8))),
                    is_fav,
                )],
            ),
            to_zero: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.26, 0.01), (0.49, 0.99))),
                |_| vec![GuiAction::SendToServer(Command::Stop)],
                [Panel::with_background(
                    GuiElemCfg::at(Rectangle::from_tuples((0.2, 0.2), (0.8, 0.8))),
                    (),
                    Color::MAGENTA,
                )],
            ),
            play_pause: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.51, 0.01), (0.74, 0.99))),
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
                GuiElemCfg::at(Rectangle::from_tuples((0.76, 0.01), (0.99, 0.99))),
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

struct FavIcon {
    config: GuiElemCfg,
    is_fav: Arc<AtomicBool>,
}
impl FavIcon {
    pub fn new(config: GuiElemCfg, is_fav: Arc<AtomicBool>) -> Self {
        Self { config, is_fav }
    }
}
impl GuiElem for FavIcon {
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        let clr = if self.is_fav.load(std::sync::atomic::Ordering::Relaxed) {
            Color::from_rgb(0.7, 0.1, 0.1)
        } else {
            Color::from_rgb(0.3, 0.2, 0.2)
        };
        let pos = if info.pos.width() > info.pos.height() {
            let c = info.pos.top_left().x + info.pos.width() * 0.5;
            let d = info.pos.height() * 0.5;
            Rectangle::from_tuples(
                (c - d, info.pos.top_left().y),
                (c + d, info.pos.bottom_right().y),
            )
        } else if info.pos.height() > info.pos.width() {
            let c = info.pos.top_left().y + info.pos.height() * 0.5;
            let d = info.pos.width() * 0.5;
            Rectangle::from_tuples(
                (info.pos.top_left().x, c - d),
                (info.pos.bottom_right().x, c + d),
            )
        } else {
            info.pos.clone()
        };
        let circle_radius = 0.25;
        let out_dist = pos.height() * circle_radius * std::f32::consts::SQRT_2 * 0.5;
        let x_cntr = pos.top_left().x + pos.width() * 0.5;
        let left_circle_cntr = Vec2::new(
            pos.top_left().x + pos.width() * circle_radius,
            pos.top_left().y + pos.height() * circle_radius,
        );
        let right_circle_cntr = Vec2::new(
            pos.bottom_right().x - pos.width() * circle_radius,
            pos.top_left().y + pos.height() * circle_radius,
        );
        let circle_radius = circle_radius * pos.height();
        let x1 = x_cntr - circle_radius - out_dist;
        let x2 = x_cntr + circle_radius + out_dist;
        let h1 = pos.top_left().y + circle_radius;
        let h2 = pos.top_left().y + circle_radius + out_dist;
        g.draw_circle(left_circle_cntr, circle_radius, clr);
        g.draw_circle(right_circle_cntr, circle_radius, clr);
        g.draw_rectangle(Rectangle::from_tuples((x1, h1), (x2, h2)), clr);
        g.draw_triangle(
            [
                Vec2::new(x1, h2),
                Vec2::new(x2, h2),
                Vec2::new(x_cntr, pos.bottom_right().y),
            ],
            clr,
        )
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
                self.set_fav.elem_mut(),
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
