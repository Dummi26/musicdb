use std::{collections::VecDeque, sync::Arc, time::Instant};

use musicdb_lib::{
    data::{CoverId, SongId},
    server::Command,
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, window::MouseButton};

use crate::{
    gui::{
        adjust_area, adjust_pos, morph_rect, GuiAction, GuiCover, GuiElem, GuiElemCfg, GuiElemTrait,
    },
    gui_text::AdvancedLabel,
};

/*

Components for the StatusBar.
This file could probably have a better name.

*/

#[derive(Clone)]
pub struct CurrentSong {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    prev_song: Option<SongId>,
    cover_pos: Rectangle,
    covers: VecDeque<(CoverId, Option<(bool, Instant)>)>,
    idle_changed: bool,
    idle: f32,
    text_updated: Option<Instant>,
    text_pos_s: Rectangle,
    text_pos_l: Rectangle,
    cover_pos_s: Rectangle,
    cover_pos_l: Rectangle,
}
impl CurrentSong {
    pub fn new(config: GuiElemCfg) -> Self {
        let text_pos_s = Rectangle::from_tuples((0.4, 0.0), (1.0, 1.0));
        let text_pos_l = Rectangle::from_tuples((0.05, 0.0), (0.95, 0.25));
        let cover_pos_s = Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0));
        let cover_pos_l = Rectangle::from_tuples((0.0, 0.26), (0.4, 0.80));
        Self {
            config,
            children: vec![GuiElem::new(AdvancedLabel::new(
                GuiElemCfg::at(text_pos_s.clone()),
                Vec2::new(0.0, 0.5),
                vec![],
            ))],
            cover_pos: Rectangle::new(Vec2::ZERO, Vec2::ZERO),
            covers: VecDeque::new(),
            prev_song: None,
            idle_changed: false,
            idle: 0.0,
            text_updated: None,
            text_pos_s,
            text_pos_l,
            cover_pos_s,
            cover_pos_l,
        }
    }
    pub fn set_idle_mode(&mut self, idle_mode: f32) {
        self.idle = idle_mode;
        self.idle_changed = true;
        let label = self.children[0].try_as_mut::<AdvancedLabel>().unwrap();
        label.config_mut().pos = morph_rect(&self.text_pos_s, &self.text_pos_l, idle_mode);
        label.align = Vec2::new(0.5 * idle_mode, 0.5);
    }
    fn color_title(a: f32) -> Color {
        Self::color_with_alpha(&Color::WHITE, a)
    }
    fn color_artist(a: f32) -> Color {
        Color::from_rgba(0.32, 0.20, 0.49, a)
    }
    fn color_album(a: f32) -> Color {
        Color::from_rgba(0.03, 0.24, 0.18, a)
    }
    fn color_by(a: f32) -> Color {
        Self::color_with_alpha(&Color::DARK_GRAY, a)
    }
    fn color_on(a: f32) -> Color {
        Self::color_with_alpha(&Color::DARK_GRAY, a)
    }
    fn color_with_alpha(c: &Color, a: f32) -> Color {
        Color::from_rgba(c.r(), c.g(), c.b(), a)
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
            // end of last song
            Some(None)
        };
        if let Some(new_song) = new_song {
            // if there is a new song:
            if self.prev_song != new_song {
                self.config.redraw = true;
                self.prev_song = new_song;
            }
            // get cover
            let get_cover = |song: Option<u64>| crate::get_cover(song?, info.database);
            let cover = get_cover(new_song);
            // fade out all covers
            for (_, t) in &mut self.covers {
                if !t.is_some_and(|t| t.0) {
                    // not fading out yet, so make it start
                    *t = Some((true, Instant::now()));
                }
            }
            // cover fades in now.
            if let Some(cover) = cover {
                self.covers
                    .push_back((cover, Some((false, Instant::now()))));
            }
            if let Some(next_cover) = get_cover(info.database.queue.get_next_song().cloned()) {
                if !info.covers.contains_key(&next_cover) {
                    info.covers.insert(
                        next_cover,
                        GuiCover::new(next_cover, Arc::clone(&info.get_con)),
                    );
                }
            }
            // redraw
            if self.config.redraw {
                self.config.redraw = false;
                if let Some(song) = new_song {
                    let status_bar_text = info
                        .gui_config
                        .status_bar_text
                        .gen(&info.database, info.database.get_song(&song));
                    self.children[0]
                        .try_as_mut::<AdvancedLabel>()
                        .unwrap()
                        .content = status_bar_text;
                    self.text_updated = Some(Instant::now());
                }
            }
        }
        if let Some(updated) = &self.text_updated {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            let mut prog = updated.elapsed().as_secs_f32();
            if prog >= 1.0 {
                prog = 1.0;
                self.text_updated = None;
            }
            for c in self.children[0]
                .try_as_mut::<AdvancedLabel>()
                .unwrap()
                .content
                .iter_mut()
            {
                for c in c {
                    *c.0.color() = Self::color_with_alpha(c.0.color(), prog);
                }
            }
        }
        // calculate cover pos
        if self.idle_changed || self.config.pixel_pos.size() != info.pos.size() {
            let cw = (info.pos.height() / info.pos.width()).min(0.5);
            let padl = 0.1;
            let padr = 1.0 - padl;
            let padp = (self.idle * 1.5).min(1.0);
            self.cover_pos_s =
                Rectangle::from_tuples((cw * padl, padl + 0.7 * padp), (cw * padr, padr));
            self.text_pos_s = Rectangle::from_tuples((cw, 0.0), (1.0, 1.0));
            // to resize the text
            self.set_idle_mode(self.idle);
            self.idle_changed = false;
            // cover pos
            let pixel_pos = adjust_area(
                &info.pos,
                &morph_rect(&self.cover_pos_s, &self.cover_pos_l, self.idle),
            );
            let pad = 0.5 * (pixel_pos.width() - pixel_pos.height());
            self.cover_pos = if pad >= 0.0 {
                Rectangle::from_tuples(
                    (
                        pixel_pos.top_left().x + pad - info.pos.top_left().x,
                        pixel_pos.top_left().y - info.pos.top_left().y,
                    ),
                    (
                        pixel_pos.bottom_right().x - pad - info.pos.top_left().x,
                        pixel_pos.bottom_right().y - info.pos.top_left().y,
                    ),
                )
            } else {
                Rectangle::from_tuples(
                    (
                        pixel_pos.top_left().x - info.pos.top_left().x,
                        pixel_pos.top_left().y - pad - info.pos.top_left().y,
                    ),
                    (
                        pixel_pos.bottom_right().x - info.pos.top_left().x,
                        pixel_pos.bottom_right().y + pad - info.pos.top_left().y,
                    ),
                )
            };
        }
        let mut cover_to_remove = None;
        for (cover_index, (cover_id, time)) in self.covers.iter_mut().enumerate() {
            let pos = match time {
                None => 1.0,
                Some((false, t)) => {
                    let el = t.elapsed().as_secs_f32();
                    if el >= 1.0 {
                        *time = None;
                        1.0
                    } else {
                        if let Some(h) = &info.helper {
                            h.request_redraw();
                        }
                        el
                    }
                }
                Some((true, t)) => {
                    let el = t.elapsed().as_secs_f32();
                    if el >= 1.0 {
                        cover_to_remove = Some(cover_index);
                        2.0
                    } else {
                        if let Some(h) = &info.helper {
                            h.request_redraw();
                        }
                        1.0 + el
                    }
                }
            };
            if let Some(cover) = info.covers.get_mut(cover_id) {
                if let Some(cover) = cover.get_init(g) {
                    let rect = Rectangle::new(
                        Vec2::new(
                            info.pos.top_left().x + self.cover_pos.top_left().x,
                            info.pos.top_left().y + self.cover_pos.top_left().y,
                        ),
                        Vec2::new(
                            info.pos.top_left().x + self.cover_pos.bottom_right().x,
                            info.pos.top_left().y + self.cover_pos.bottom_right().y,
                        ),
                    );
                    if pos == 1.0 {
                        g.draw_rectangle_image(rect, &cover);
                    } else {
                        let prog = (pos - 1.0).abs();
                        // shrink to half (0.5x0.5) size while moving left and fading out
                        let lx = rect.top_left().x + rect.width() * prog * 0.25;
                        let rx = rect.bottom_right().x - rect.width() * prog * 0.25;
                        let ty = rect.top_left().y + rect.height() * prog * 0.25;
                        let by = rect.bottom_right().y - rect.height() * prog * 0.25;
                        let mut moved = rect.width() * prog * prog;
                        if pos > 1.0 {
                            moved = -moved;
                        }
                        g.draw_rectangle_image_tinted(
                            Rectangle::from_tuples((lx + moved, ty), (rx + moved, by)),
                            Color::from_rgba(
                                1.0,
                                1.0,
                                1.0,
                                if pos > 1.0 { 2.0 - pos } else { pos },
                            ),
                            &cover,
                        );
                    }
                } else {
                    // cover still loading, just wait
                }
            } else {
                // cover not loading or loaded, start loading!
                info.covers
                    .insert(*cover_id, GuiCover::new(*cover_id, info.get_con.clone()));
            }
        }
        // removing one cover per frame is good enough
        if let Some(index) = cover_to_remove {
            self.covers.remove(index);
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
