use musicdb_lib::data::{AlbumId, ArtistId};
use speedy2d::{color::Color, dimen::Vec2, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiElem, GuiElemCfg},
    gui_base::{Button, Panel, ScrollBox},
    gui_text::Label,
};

pub struct SongAdder {
    pub config: GuiElemCfg,
    state: u8,
    c_loading: Option<Label>,
    c_scroll_box: ScrollBox<Vec<AddableSong>>,
    c_background: Panel<()>,
    data: Option<Vec<AddSong>>,
}
struct AddSong {
    path: String,
    path_broken: bool,
    artist: Option<ArtistId>,
    album: Option<AlbumId>,
}
impl SongAdder {
    pub fn new(
        mut config: GuiElemCfg,
        no_animations: bool,
        line_height: f32,
        scroll_sensitivity_pixels: f64,
        scroll_sensitivity_lines: f64,
        scroll_sensitivity_pages: f64,
    ) -> Self {
        config.redraw = true;
        Self {
            config,
            state: 0,
            c_loading: Some(Label::new(
                GuiElemCfg::default(),
                format!("Loading..."),
                Color::GRAY,
                None,
                Vec2::new(0.5, 0.5),
            )),
            c_scroll_box: ScrollBox::new(
                GuiElemCfg::default(),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                vec![],
                vec![],
                line_height,
            ),
            c_background: Panel::with_background(GuiElemCfg::default().w_mouse(), (), Color::BLACK),
            data: None,
        }
    }
}

impl GuiElem for SongAdder {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_loading.as_mut().map(|v| v.elem_mut()),
                Some(self.c_scroll_box.elem_mut()),
                Some(self.c_background.elem_mut()),
            ]
            .into_iter()
            .flatten(),
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut Graphics2D) {
        if self.state < 10 {
            self.state += 1;
            if self.state == 2 {
                self.c_loading = None;
                eprintln!("Locking GetCon...");
                let mut get_con = info.get_con.lock().unwrap();
                eprintln!("Requesting list of unused songs...");
                match get_con.find_unused_song_files(None).unwrap() {
                    Ok(data) => {
                        eprintln!("Got list of songs.");
                        self.c_scroll_box.children = data
                            .iter()
                            .map(|(path, is_bad)| AddableSong::new(path.to_owned(), *is_bad))
                            .collect();
                        self.c_scroll_box.config_mut().redraw = true;
                        self.data = Some(
                            data.into_iter()
                                .map(|(p, b)| AddSong {
                                    path: p,
                                    path_broken: b,
                                    artist: None,
                                    album: None,
                                })
                                .collect(),
                        );
                    }
                    Err(e) => {
                        eprintln!("Got error: {e}");
                        self.c_loading = Some(Label::new(
                            GuiElemCfg::default(),
                            format!("Error:\n{e}"),
                            Color::RED,
                            None,
                            Vec2::new(0.5, 0.5),
                        ));
                    }
                }
            }
        }

        if self.config.redraw {
            self.config.redraw = false;
            self.c_scroll_box.config_mut().redraw = true;
        }
    }
}

struct AddableSong {
    pub config: GuiElemCfg,
    pub c_button: Button<[Label; 1]>,
    pub path: String,
    pub is_bad: bool,
}
impl AddableSong {
    pub fn new(path: String, is_bad: bool) -> Self {
        Self {
            config: GuiElemCfg::default(),
            c_button: Button::new(
                GuiElemCfg::default(),
                |_| vec![],
                [Label::new(
                    GuiElemCfg::default(),
                    format!("{path}"),
                    if is_bad {
                        Color::LIGHT_GRAY
                    } else {
                        Color::WHITE
                    },
                    None,
                    Vec2::new(0.0, 0.5),
                )],
            ),
            path,
            is_bad,
        }
    }
}
impl GuiElem for AddableSong {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_button.elem_mut()].into_iter())
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
