use std::{sync::Arc, time::Duration};

use musicdb_lib::data::{CoverId, SongId};
use speedy2d::{color::Color, dimen::Vec2, image::ImageHandle, shape::Rectangle};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElemCfg, GuiServerImage},
    gui_anim::AnimationController,
    gui_base::Panel,
    gui_notif::NotifInfo,
    gui_text::Label,
};

pub struct CurrentInfo {
    pub update: bool,
    pub new_song: bool,
    pub new_cover: bool,
    pub current_song: Option<SongId>,
    pub current_cover: Option<(CoverId, Option<Option<ImageHandle>>)>,
}

impl CurrentInfo {
    pub fn new() -> Self {
        Self {
            update: true,
            new_song: false,
            new_cover: false,
            current_song: None,
            current_cover: None,
        }
    }
    pub fn update(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        if self.update {
            self.update = false;
            let current_song = info.database.queue.get_current_song().cloned();
            if current_song != self.current_song {
                self.current_song = current_song;
                self.new_song = true;
            }
            if let Some(current_song) = current_song {
                let current_songs_cover =
                    info.database.songs().get(&current_song).and_then(|song| {
                        song.cover
                            .or_else(|| {
                                song.album
                                    .and_then(|album| info.database.albums().get(&album))
                                    .and_then(|album| album.cover)
                            })
                            .or_else(|| {
                                info.database
                                    .artists()
                                    .get(&song.artist)
                                    .and_then(|artist| artist.cover)
                            })
                    });
                if let Some(current_songs_cover) = current_songs_cover {
                    if let Some(cover) = info.covers.get_mut(&current_songs_cover) {
                        if let Some(cover) = cover.get_init(g) {
                            // cover loaded
                            if self.current_cover.is_none()
                                || self.current_cover.as_ref().is_some_and(|(cc, h)| {
                                    *cc != current_songs_cover || !matches!(h, Some(Some(_)))
                                })
                            {
                                self.current_cover = Some((current_songs_cover, Some(Some(cover))));
                                self.new_cover = true;
                            }
                        } else if cover.is_err() {
                            // no cover with that ID
                            if self.current_cover.is_none()
                                || self.current_cover.as_ref().is_some_and(|(csc, h)| {
                                    *csc != current_songs_cover || !matches!(h, Some(None))
                                })
                            {
                                // is_err and `current` is old
                                self.current_cover = Some((current_songs_cover, Some(None)));
                                self.new_cover = true;
                                // show notification
                                info.actions.push(GuiAction::ShowNotification(Box::new(
                                    move |_| {
                                        (
                                            Box::new(Panel::with_background(
                                                GuiElemCfg::default(),
                                                [Label::new(
                                                    GuiElemCfg::default(),
                                                    format!("Couldn't load cover"),
                                                    Color::WHITE,
                                                    None,
                                                    Vec2::new(0.5, 0.5),
                                                )],
                                                Color::from_rgba(0.0, 0.0, 0.0, 0.8),
                                            )),
                                            NotifInfo::new(Duration::from_secs(1)),
                                        )
                                    },
                                )));
                            }
                        } else {
                            // Cover loading, check again later
                            if self.current_cover.is_none()
                                || self.current_cover.as_ref().is_some_and(|(cc, h)| {
                                    *cc != current_songs_cover || h.is_some()
                                })
                            {
                                self.current_cover = Some((current_songs_cover, None));
                                self.new_cover = true;
                            }
                            self.update = true;
                        }
                    } else {
                        info.covers.insert(
                            current_songs_cover,
                            GuiServerImage::new_cover(
                                current_songs_cover,
                                Arc::clone(&info.get_con),
                            ),
                        );
                        if self.current_cover.is_none()
                            || self
                                .current_cover
                                .as_ref()
                                .is_some_and(|(cc, h)| *cc != current_songs_cover || h.is_some())
                        {
                            self.current_cover = Some((current_songs_cover, None));
                            self.new_cover = true;
                        }
                        self.update = true;
                    }
                } else {
                    // no cover
                    if self.current_cover.is_some() {
                        self.current_cover = None;
                        self.new_cover = true;
                    }
                }
            } else {
                // no song
                if self.current_cover.is_some() {
                    self.current_cover = None;
                    self.new_cover = true;
                }
            }
        }
    }
}

pub fn image_display(
    g: &mut speedy2d::Graphics2D,
    img: Option<&ImageHandle>,
    left: f32,
    top: f32,
    bottom: f32,
    aspect_ratio: &mut AnimationController<f32>,
) {
    if let Some(cover) = &img {
        let cover_size = cover.size();
        aspect_ratio.target = if cover_size.x > 0 && cover_size.y > 0 {
            let right_x = get_right_x(left, top, bottom, aspect_ratio.value);
            let pos = Rectangle::from_tuples((left, top), (right_x, bottom));
            let aspect_ratio = cover_size.x as f32 / cover_size.y as f32;
            g.draw_rectangle_image(pos, cover);
            aspect_ratio
        } else {
            0.0
        };
    } else {
        aspect_ratio.target = 0.0;
    }
}
pub fn get_right_x(left: f32, top: f32, bottom: f32, aspect_ratio: f32) -> f32 {
    left + aspect_ratio * (bottom - top)
}
