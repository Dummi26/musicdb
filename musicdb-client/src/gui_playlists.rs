use musicdb_lib::data::database::Database;
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg},
    gui_base::{Button, ScrollBox},
    gui_queue::QueueViewer,
    gui_text::{Label, TextField},
};

/*

A list of saved queues (playlists)
using the queue viewer.

*/

fn queue_viewer_pos() -> Rectangle {
    Rectangle::from_tuples((0.0, 0.2), (1.0, 1.0))
}

pub struct PlaylistView {
    config: GuiElemCfg,
    pub c_search: TextField,
    pub c_search_clear: Button<[Label; 1]>,
    pub c_playlists: ScrollBox<Vec<Button<[Label; 1]>>>,
    pub c_queue_viewer: QueueViewer,
    // - - -
    search_string: String,
}
impl PlaylistView {
    pub fn new(config: GuiElemCfg) -> Self {
        let c_search = TextField::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.01, 0.01), (0.91, 0.05))),
            "playlist name".to_string(),
            Color::GRAY,
            Color::WHITE,
        );
        let c_search_clear = Button::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.93, 0.01), (0.99, 0.05))),
            |_| vec![GuiAction::SetShowingSavedQueue(String::new())],
            [Label::new(
                GuiElemCfg::default(),
                "×".to_owned(),
                Color::from_int_rgb(150, 70, 70),
                None,
                Vec2::new(0.5, 0.5),
            )],
        );
        let playlists_scroll_box = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.06), (1.0, 0.19))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![],
            vec![],
            0.0,
        );
        Self {
            config: config.w_keyboard_watch(),
            c_search,
            c_search_clear,
            c_playlists: playlists_scroll_box,
            c_queue_viewer: QueueViewer::new_saved_queue(
                GuiElemCfg::at(queue_viewer_pos()),
                String::new(),
            ),
            search_string: String::new(),
        }
    }
}
impl GuiElem for PlaylistView {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_search.elem_mut(),
                self.c_search_clear.elem_mut(),
                self.c_playlists.elem_mut(),
                self.c_queue_viewer.elem_mut(),
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
    fn draw_rev(&self) -> bool {
        false
    }
    fn updated_library(&mut self) {
        self.config.redraw_once();
    }
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        let search = &mut self.c_search.c_input.content;
        if search.will_redraw() && self.search_string != *search.get_text() {
            self.search_string = search.get_text().clone();
            self.config.redraw_once();
        }
        if self.config.redraw() || info.pos.size() != self.config.pixel_pos.size() {
            self.config.redrawn();
            self.update_ui(info.database, info.line_height);
        }
    }
}
impl PlaylistView {
    /// Sets the contents of the `ScrollBox` based on `self.search_string`.
    fn update_ui(&mut self, db: &Database, line_height: f32) {
        let mut elems = vec![];
        let mut elemh = vec![];
        let search = self.search_string.to_lowercase();
        for name in db.queues.keys() {
            if name.to_lowercase().contains(&search) {
                let (e, h) = self.build_ui_element(name, db, line_height);
                elems.push(e);
                elemh.push(h);
            }
        }
        let playlists_scroll_box = &mut self.c_playlists;
        playlists_scroll_box.children = elems;
        playlists_scroll_box.children_heights = elemh;
        playlists_scroll_box.config_mut().redraw_once();
        self.c_queue_viewer.saved = Some(self.search_string.clone());
        self.c_queue_viewer.updated_queue();
    }
    fn build_ui_element(&self, name: &str, db: &Database, h: f32) -> (Button<[Label; 1]>, f32) {
        let name = name.to_owned();
        let name2 = name.clone();
        (
            Button::new(
                GuiElemCfg::default(),
                move |_| vec![GuiAction::SetShowingSavedQueue(name2.clone())],
                [Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.02, 0.0), (0.98, 1.0))),
                    name,
                    Color::WHITE,
                    None,
                    Vec2::new(0.0, 0.5),
                )],
            ),
            h,
        )
    }
}
