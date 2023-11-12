use std::time::Instant;

use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    gui::{DrawInfo, GuiElem, GuiElemCfg},
    gui_anim::AnimationController,
    gui_playback::{image_display, CurrentInfo},
    gui_text::AdvancedLabel,
};

pub struct StatusBar {
    config: GuiElemCfg,
    pub idle_mode: f32,
    current_info: CurrentInfo,
    cover_aspect_ratio: AnimationController<f32>,
    c_song_label: AdvancedLabel,
}

impl StatusBar {
    pub fn new(config: GuiElemCfg) -> Self {
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
        }
    }
}

impl GuiElem for StatusBar {
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_song_label.elem_mut()].into_iter())
    }
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        self.current_info.update(info, g);
        if self.current_info.new_song {
            self.current_info.new_song = false;
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
            .update(info.time.clone(), info.no_animations);
        if ar_updated || info.pos.size() != self.config.pixel_pos.size() {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            self.c_song_label.config_mut().pos = Rectangle::from_tuples(
                (
                    self.cover_aspect_ratio.value * info.pos.height() / info.pos.width(),
                    0.0,
                ),
                (1.0, 1.0),
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
    }
    fn updated_queue(&mut self) {
        self.current_info.update = true;
    }
}
