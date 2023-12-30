use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

use musicdb_lib::data::ArtistId;
use speedy2d::{color::Color, dimen::Vec2, image::ImageHandle, shape::Rectangle};

use crate::{
    gui::{rect_from_rel, DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiServerImage},
    gui_anim::AnimationController,
    gui_base::Button,
    gui_playback::{get_right_x, image_display, CurrentInfo},
    gui_playpause::PlayPause,
    gui_text::{AdvancedLabel, Label},
};

pub struct IdleDisplay {
    pub config: GuiElemCfg,
    pub idle_mode: f32,
    pub current_info: CurrentInfo,
    pub current_artist_image: Option<(ArtistId, Option<(String, Option<Option<ImageHandle>>)>)>,
    pub c_idle_exit_hint: Button<[Label; 1]>,
    pub c_top_label: AdvancedLabel,
    pub c_side1_label: AdvancedLabel,
    pub c_side2_label: AdvancedLabel,
    pub c_buttons: PlayPause,
    pub c_buttons_custom_pos: bool,

    pub cover_aspect_ratio: AnimationController<f32>,
    pub artist_image_aspect_ratio: AnimationController<f32>,

    pub cover_pos: Option<Rectangle>,
    pub cover_left: f32,
    pub cover_top: f32,
    pub cover_bottom: f32,

    pub artist_image_pos: Option<Rectangle>,
    /// 0.0 -> same height as cover,
    /// 0.5 -> lower half of cover
    pub artist_image_top: f32,
    pub artist_image_to_cover_margin: f32,

    pub force_reset_texts: bool,

    is_fav: (bool, Arc<AtomicBool>),
}

impl IdleDisplay {
    pub fn new(config: GuiElemCfg) -> Self {
        let cover_bottom = 0.79;
        let is_fav = Arc::new(AtomicBool::new(false));
        Self {
            config,
            idle_mode: 0.0,
            current_info: CurrentInfo::new(),
            current_artist_image: None,
            c_idle_exit_hint: Button::new(
                GuiElemCfg::default().disabled(),
                |_| vec![GuiAction::EndIdle(true)],
                [Label::new(
                    GuiElemCfg::default(),
                    "Back".to_owned(),
                    Color::GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
            ),
            c_top_label: AdvancedLabel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.05, 0.02), (0.95, 0.18))),
                Vec2::new(0.5, 0.5),
                vec![],
            ),
            c_side1_label: AdvancedLabel::new(GuiElemCfg::default(), Vec2::new(0.0, 0.5), vec![]),
            c_side2_label: AdvancedLabel::new(GuiElemCfg::default(), Vec2::new(0.0, 0.5), vec![]),
            is_fav: (false, Arc::clone(&is_fav)),
            c_buttons: PlayPause::new(GuiElemCfg::default(), is_fav),
            c_buttons_custom_pos: false,
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
            cover_pos: None,
            cover_left: 0.01,
            cover_top: 0.21,
            cover_bottom,
            artist_image_pos: None,
            artist_image_top: 0.5,
            artist_image_to_cover_margin: 0.01,
            force_reset_texts: false,
        }
    }
}

impl GuiElem for IdleDisplay {
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_idle_exit_hint.elem_mut(),
                self.c_top_label.elem_mut(),
                self.c_side1_label.elem_mut(),
                self.c_side2_label.elem_mut(),
                self.c_buttons.elem_mut(),
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
        if self.current_info.new_song || self.force_reset_texts {
            self.current_info.new_song = false;
            self.force_reset_texts = false;
            let is_fav = self
                .current_info
                .current_song
                .and_then(|id| info.database.get_song(&id))
                .map(|song| song.general.tags.iter().any(|v| v == "Fav"))
                .unwrap_or(false);
            if self.is_fav.0 != is_fav {
                self.is_fav.0 = is_fav;
                self.is_fav
                    .1
                    .store(is_fav, std::sync::atomic::Ordering::Relaxed);
            }
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
                self.cover_pos.as_ref().map(|v| rect_from_rel(v, &info.pos)),
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
                self.artist_image_pos
                    .as_ref()
                    .map(|v| rect_from_rel(v, &info.pos)),
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
            .update(info.time.clone(), info.high_performance)
            | self
                .artist_image_aspect_ratio
                .update(info.time.clone(), info.high_performance);
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
            // limit width of c_buttons
            let buttons_right_pos = 1.0;
            let buttons_width_max = info.pos.height() * 0.08 * 4.0 / info.pos.width();
            // buttons use at most half the width (set to 0.2 later, when screen space is used for other things)
            let buttons_width = buttons_width_max.min(0.5);
            if !self.c_buttons_custom_pos {
                self.c_buttons.config_mut().pos = Rectangle::from_tuples(
                    (buttons_right_pos - buttons_width, 0.86),
                    (buttons_right_pos, 0.94),
                );
            }
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
