use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

use speedy2d::{dimen::Vec2, shape::Rectangle};

use crate::{
    gui::{DrawInfo, GuiElem, GuiElemCfg},
    gui_anim::AnimationController,
    gui_playback::{image_display, CurrentInfo},
    gui_playpause::PlayPause,
    gui_text::AdvancedLabel,
};

pub struct StatusBar {
    config: GuiElemCfg,
    pub idle_mode: f32,
    current_info: CurrentInfo,
    cover_aspect_ratio: AnimationController<f32>,
    c_song_label: AdvancedLabel,
    pub force_reset_texts: bool,
    c_buttons: PlayPause,
    is_fav: (bool, Arc<AtomicBool>),
}

impl StatusBar {
    pub fn new(config: GuiElemCfg) -> Self {
        let is_fav = Arc::new(AtomicBool::new(false));
        Self {
            config,
            idle_mode: 0.0,
            current_info: CurrentInfo::new(),
            cover_aspect_ratio: AnimationController::new(
                0.0,
                0.0,
                0.01,
                1.0,
                0.8,
                0.6,
                Instant::now(),
            ),
            c_song_label: AdvancedLabel::new(GuiElemCfg::default(), Vec2::new(0.0, 0.5), vec![]),
            force_reset_texts: false,
            is_fav: (false, Arc::clone(&is_fav)),
            c_buttons: PlayPause::new(GuiElemCfg::default(), is_fav),
        }
    }
}

impl GuiElem for StatusBar {
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_song_label.elem_mut(), self.c_buttons.elem_mut()].into_iter())
    }
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        self.current_info.update(info, g);
        if self.current_info.new_song || self.force_reset_texts {
            self.current_info.new_song = false;
            self.force_reset_texts = false;
            let is_fav = self
                .current_info
                .current_song
                .and_then(|id| info.database.get_song(&id))
                .map(|song| song.general.tags.iter().any(|v| v == "Fav"))
                .unwrap_or(false);
            eprintln!("is_fav: {is_fav}");
            if self.is_fav.0 != is_fav {
                self.is_fav.0 = is_fav;
                self.is_fav
                    .1
                    .store(is_fav, std::sync::atomic::Ordering::Relaxed);
            }
            self.c_song_label.content = if let Some(song) = self.current_info.current_song {
                info.gui_config
                    .status_bar_text
                    .gen(&info.database, info.database.get_song(&song))
            } else {
                vec![]
            };
            self.c_song_label.config_mut().redraw = true;
        }
        if self.current_info.new_cover {
            self.current_info.new_cover = false;
            match self.current_info.current_cover {
                None | Some((_, Some(None))) => {
                    self.cover_aspect_ratio.target = 0.0;
                }
                Some((_, None)) | Some((_, Some(Some(_)))) => {}
            }
        }
        // move children to make space for cover
        let ar_updated = self
            .cover_aspect_ratio
            .update(info.time.clone(), info.high_performance);
        if ar_updated || info.pos.size() != self.config.pixel_pos.size() {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            // limit width of c_buttons
            let buttons_right_pos = 0.99;
            let buttons_width_max = info.pos.height() * 0.7 * 4.0 / info.pos.width();
            let buttons_width = buttons_width_max.min(0.2);
            self.c_buttons.config_mut().pos = Rectangle::from_tuples(
                (buttons_right_pos - buttons_width, 0.15),
                (buttons_right_pos, 0.85),
            );
            self.c_song_label.config_mut().pos = Rectangle::from_tuples(
                (
                    self.cover_aspect_ratio.value * info.pos.height() / info.pos.width(),
                    0.0,
                ),
                (buttons_right_pos - buttons_width, 1.0),
            );
        }
        // draw cover
        if let Some(Some(cover)) = self
            .current_info
            .current_cover
            .as_ref()
            .map(|v| v.1.as_ref())
        {
            image_display(
                g,
                cover.as_ref(),
                None,
                info.pos.top_left().x + info.pos.height() * 0.05,
                info.pos.top_left().y + info.pos.height() * 0.05,
                info.pos.top_left().y + info.pos.height() * 0.95,
                &mut self.cover_aspect_ratio,
            );
        }
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
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
    fn updated_library(&mut self) {
        self.current_info.update = true;
        self.force_reset_texts = true;
    }
    fn updated_queue(&mut self) {
        self.current_info.update = true;
    }
}
