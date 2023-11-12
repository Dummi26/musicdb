use std::{sync::Arc, time::Instant};

use musicdb_lib::data::ArtistId;
use speedy2d::{color::Color, dimen::Vec2, image::ImageHandle, shape::Rectangle};

use crate::{
    gui::{DrawInfo, GuiElem, GuiElemCfg, GuiServerImage},
    gui_anim::AnimationController,
    gui_playback::{get_right_x, image_display, CurrentInfo},
    gui_text::AdvancedLabel,
};

pub struct IdleDisplay {
    config: GuiElemCfg,
    pub idle_mode: f32,
    current_info: CurrentInfo,
    current_artist_image: Option<(ArtistId, Option<(String, Option<Option<ImageHandle>>)>)>,
    c_top_label: AdvancedLabel,
    c_side1_label: AdvancedLabel,
    c_side2_label: AdvancedLabel,
    cover_aspect_ratio: AnimationController<f32>,
    artist_image_aspect_ratio: AnimationController<f32>,
    cover_left: f32,
    cover_top: f32,
    cover_bottom: f32,
    /// 0.0 -> same height as cover,
    /// 0.5 -> lower half of cover
    artist_image_top: f32,
    artist_image_to_cover_margin: f32,
}

impl IdleDisplay {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config,
            idle_mode: 0.0,
            current_info: CurrentInfo::new(),
            current_artist_image: None,
            c_top_label: AdvancedLabel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.05, 0.02), (0.95, 0.18))),
                Vec2::new(0.5, 0.5),
                vec![],
            ),
            c_side1_label: AdvancedLabel::new(GuiElemCfg::default(), Vec2::new(0.0, 0.5), vec![]),
            c_side2_label: AdvancedLabel::new(GuiElemCfg::default(), Vec2::new(0.0, 0.5), vec![]),
            cover_aspect_ratio: AnimationController::new(
                1.0,
                1.0,
                0.01,
                1.0,
                0.8,
                0.6,
                Instant::now(),
            ),
            artist_image_aspect_ratio: AnimationController::new(
                0.0,
                0.0,
                0.01,
                1.0,
                0.8,
                0.6,
                Instant::now(),
            ),
            cover_left: 0.01,
            cover_top: 0.21,
            cover_bottom: 0.79,
            artist_image_top: 0.5,
            artist_image_to_cover_margin: 0.01,
        }
    }
}

impl GuiElem for IdleDisplay {
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_top_label.elem_mut(),
                self.c_side1_label.elem_mut(),
                self.c_side2_label.elem_mut(),
            ]
            .into_iter(),
        )
    }
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        // draw background
        g.draw_rectangle(
            info.pos.clone(),
            Color::from_rgba(0.0, 0.0, 0.0, 0.5 + 0.5 * self.idle_mode),
        );
        // update current_info
        self.current_info.update(info, g);
        if self.current_info.new_song {
            self.current_info.new_song = false;
            self.c_top_label.content = if let Some(song) = self.current_info.current_song {
                info.gui_config
                    .idle_top_text
                    .gen(&info.database, info.database.get_song(&song))
            } else {
                vec![]
            };
            self.c_top_label.config_mut().redraw = true;
            self.c_side1_label.content = if let Some(song) = self.current_info.current_song {
                info.gui_config
                    .idle_side1_text
                    .gen(&info.database, info.database.get_song(&song))
            } else {
                vec![]
            };
            self.c_side1_label.config_mut().redraw = true;
            self.c_side2_label.content = if let Some(song) = self.current_info.current_song {
                info.gui_config
                    .idle_side2_text
                    .gen(&info.database, info.database.get_song(&song))
            } else {
                vec![]
            };
            self.c_side2_label.config_mut().redraw = true;
            // check artist
            if let Some(artist_id) = self
                .current_info
                .current_song
                .as_ref()
                .and_then(|id| info.database.songs().get(id))
                .map(|song| song.artist)
            {
                if self.current_artist_image.is_none()
                    || self
                        .current_artist_image
                        .as_ref()
                        .is_some_and(|(a, _)| *a != artist_id)
                {
                    self.current_artist_image = Some((artist_id, None));
                    self.artist_image_aspect_ratio.target = 0.0;
                    if let Some(artist) = info.database.artists().get(&artist_id) {
                        for tag in &artist.general.tags {
                            if tag.starts_with("ImageExt=") {
                                let filename = format!("{}.{}", artist.name, &tag[9..]);
                                self.current_artist_image =
                                    Some((artist_id, Some((filename.clone(), None))));
                                if !info.custom_images.contains_key(&filename) {
                                    info.custom_images.insert(
                                        filename.clone(),
                                        GuiServerImage::new_custom_file(
                                            filename,
                                            Arc::clone(&info.get_con),
                                        ),
                                    );
                                }
                                break;
                            }
                        }
                    }
                }
            } else {
                if self.current_artist_image.is_some() {
                    self.current_artist_image = None;
                    self.artist_image_aspect_ratio.target = 0.0;
                }
            }
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
        if let Some((_, Some((img, h)))) = &mut self.current_artist_image {
            if h.is_none() {
                if let Some(img) = info.custom_images.get_mut(img) {
                    if let Some(img) = img.get_init(g) {
                        *h = Some(Some(img));
                    } else if img.is_err() {
                        *h = Some(None);
                    }
                }
            }
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
                info.pos.top_left().x + info.pos.height() * self.cover_left,
                info.pos.top_left().y + info.pos.height() * self.cover_top,
                info.pos.top_left().y + info.pos.height() * self.cover_bottom,
                &mut self.cover_aspect_ratio,
            );
        }
        // draw artist image
        if let Some((_, Some((_, Some(img))))) = &self.current_artist_image {
            let top = info.pos.top_left().y + info.pos.height() * self.cover_top;
            let bottom = info.pos.top_left().y + info.pos.height() * self.cover_bottom;
            image_display(
                g,
                img.as_ref(),
                get_right_x(
                    info.pos.top_left().x + info.pos.height() * self.cover_left,
                    top,
                    bottom,
                    self.cover_aspect_ratio.value,
                ) + info.pos.height() * self.artist_image_to_cover_margin,
                top + (bottom - top) * self.artist_image_top,
                bottom,
                &mut self.artist_image_aspect_ratio,
            );
        }
        // move children to make space for cover
        let ar_updated = self
            .cover_aspect_ratio
            .update(info.time.clone(), info.no_animations)
            | self
                .artist_image_aspect_ratio
                .update(info.time.clone(), info.no_animations);
        if ar_updated || info.pos.size() != self.config.pixel_pos.size() {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            // make thing be relative to width instead of to height by multiplying with this
            let top = self.cover_top;
            let bottom = self.cover_bottom;
            let left = (get_right_x(self.cover_left, top, bottom, self.cover_aspect_ratio.value)
                + self.artist_image_to_cover_margin)
                * info.pos.height()
                / info.pos.width();
            let ai_top = top + (bottom - top) * self.artist_image_top;
            let max_right = 1.0 - self.cover_left * info.pos.height() / info.pos.width();
            self.c_side1_label.config_mut().pos =
                Rectangle::from_tuples((left, top), (max_right, ai_top));
            let left = get_right_x(
                left,
                ai_top * info.pos.height() / info.pos.width(),
                bottom * info.pos.height() / info.pos.width(),
                self.artist_image_aspect_ratio.value,
            );
            self.c_side2_label.config_mut().pos =
                Rectangle::from_tuples((left, ai_top), (max_right, bottom));
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