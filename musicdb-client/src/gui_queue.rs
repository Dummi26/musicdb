use musicdb_lib::{
    data::{
        database::Database,
        queue::{Queue, QueueContent},
        song::Song,
        AlbumId, ArtistId,
    },
    server::Command,
};
use speedy2d::{
    color::Color,
    dimen::Vec2,
    shape::Rectangle,
    window::{ModifiersState, MouseButton, VirtualKeyCode},
};

use crate::{
    gui::{Dragging, DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_base::ScrollBox,
    gui_text::Label,
};

/*


This is responsible for showing the current queue,
with drag-n-drop only if the mouse leaves the element before it is released,
because simple clicks have to be GoTo events.

*/

#[derive(Clone)]
pub struct QueueViewer {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
}
impl QueueViewer {
    pub fn new(config: GuiElemCfg) -> Self {
        let queue_scroll_box = ScrollBox::new(
            GuiElemCfg::default(),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![(
                GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    "loading...".to_string(),
                    Color::DARK_GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                )),
                1.0,
            )],
        );
        Self {
            config,
            children: vec![
                GuiElem::new(queue_scroll_box),
                GuiElem::new(QueueEmptySpaceDragHandler::new(GuiElemCfg::default())),
            ],
        }
    }
}
impl GuiElemTrait for QueueViewer {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.config.redraw || info.pos.size() != self.config.pixel_pos.size() {
            self.config.redraw = false;
            let mut c = vec![];
            queue_gui(
                &info.database.queue,
                &info.database,
                0.0,
                0.02,
                info.line_height,
                &mut c,
                vec![],
                true,
            );
            let mut scroll_box = self.children[0].try_as_mut::<ScrollBox>().unwrap();
            scroll_box.children = c;
            scroll_box.config_mut().redraw = true;
        }
    }
    fn updated_queue(&mut self) {
        self.config.redraw = true;
    }
}

fn queue_gui(
    queue: &Queue,
    db: &Database,
    depth: f32,
    depth_inc_by: f32,
    line_height: f32,
    target: &mut Vec<(GuiElem, f32)>,
    path: Vec<usize>,
    current: bool,
) {
    let cfg = GuiElemCfg::at(Rectangle::from_tuples((depth, 0.0), (1.0, 1.0)));
    match queue.content() {
        QueueContent::Song(id) => {
            if let Some(s) = db.songs().get(id) {
                target.push((
                    GuiElem::new(QueueSong::new(cfg, path, s.clone(), current)),
                    line_height,
                ));
            }
        }
        QueueContent::Folder(ia, q, _) => {
            target.push((
                GuiElem::new(QueueFolder::new(cfg, path.clone(), queue.clone(), current)),
                line_height * 0.67,
            ));
            for (i, q) in q.iter().enumerate() {
                let mut p = path.clone();
                p.push(i);
                queue_gui(
                    q,
                    db,
                    depth + depth_inc_by,
                    depth_inc_by,
                    line_height,
                    target,
                    p,
                    current && *ia == i,
                );
            }
        }
    }
}

#[derive(Clone)]
struct QueueEmptySpaceDragHandler {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
}
impl QueueEmptySpaceDragHandler {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config: config.w_drag_target(),
            children: vec![],
        }
    }
}
impl GuiElemTrait for QueueEmptySpaceDragHandler {
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
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        dragged_add_to_queue(dragged, |q| Command::QueueAdd(vec![], q))
    }
}

fn generic_queue_draw(
    info: &mut DrawInfo,
    path: &Vec<usize>,
    mouse: &mut bool,
    copy_on_mouse_down: bool,
) -> bool {
    if *mouse && !info.pos.contains(info.mouse_pos) {
        *mouse = false;
        if !copy_on_mouse_down {
            info.actions
                .push(GuiAction::SendToServer(Command::QueueRemove(path.clone())));
        }
        true
    } else {
        false
    }
}

#[derive(Clone)]
struct QueueSong {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    path: Vec<usize>,
    song: Song,
    current: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    copy_on_mouse_down: bool,
}
impl QueueSong {
    pub fn new(config: GuiElemCfg, path: Vec<usize>, song: Song, current: bool) -> Self {
        Self {
            config: config.w_mouse().w_keyboard_watch().w_drag_target(),
            children: vec![GuiElem::new(Label::new(
                GuiElemCfg::default(),
                song.title.clone(),
                if current {
                    Color::from_int_rgb(194, 76, 178)
                } else {
                    Color::from_int_rgb(120, 76, 194)
                },
                None,
                Vec2::new(0.0, 0.5),
            ))],
            path,
            song,
            current,
            mouse: false,
            mouse_pos: Vec2::ZERO,
            copy: false,
            copy_on_mouse_down: false,
        }
    }
}

impl GuiElemTrait for QueueSong {
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
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            self.copy_on_mouse_down = self.copy;
        }
        vec![]
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            vec![GuiAction::SendToServer(Command::QueueGoto(
                self.path.clone(),
            ))]
        } else {
            vec![]
        }
    }
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(QueueContent::Song(self.song.id).into()),
                Some(Box::new(move |i, g| {
                    let sw = i.pos.width();
                    let sh = i.pos.height();
                    let x = (i.mouse_pos.x - mouse_pos.x) / sw;
                    let y = (i.mouse_pos.y - mouse_pos.y) / sh;
                    el.inner.config_mut().pos =
                        Rectangle::from_tuples((x, y), (x + w / sw, y + h / sh));
                    el.draw(i, g)
                })),
            ))));
        }
    }
    fn key_watch(
        &mut self,
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        let mut p = self.path.clone();
        dragged_add_to_queue(dragged, move |q| {
            if let Some(i) = p.pop() {
                Command::QueueInsert(p, i, q)
            } else {
                Command::QueueAdd(p, q)
            }
        })
    }
}

#[derive(Clone)]
struct QueueFolder {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    path: Vec<usize>,
    queue: Queue,
    current: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    copy_on_mouse_down: bool,
}
impl QueueFolder {
    pub fn new(config: GuiElemCfg, path: Vec<usize>, queue: Queue, current: bool) -> Self {
        Self {
            config: if path.is_empty() {
                config
            } else {
                config.w_mouse().w_keyboard_watch()
            }
            .w_drag_target(),
            children: vec![GuiElem::new(Label::new(
                GuiElemCfg::default(),
                match queue.content() {
                    QueueContent::Folder(_, q, n) => format!(
                        "{}  ({})",
                        if path.is_empty() && n.is_empty() {
                            "Queue"
                        } else {
                            n
                        },
                        q.len()
                    ),
                    _ => "[???]".to_string(),
                },
                Color::from_int_rgb(52, 132, 50),
                None,
                Vec2::new(0.0, 0.5),
            ))],
            path,
            queue,
            current,
            mouse: false,
            mouse_pos: Vec2::ZERO,
            copy: false,
            copy_on_mouse_down: false,
        }
    }
}
impl GuiElemTrait for QueueFolder {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(self.queue.clone()),
                Some(Box::new(move |i, g| {
                    let sw = i.pos.width();
                    let sh = i.pos.height();
                    let x = (i.mouse_pos.x - mouse_pos.x) / sw;
                    let y = (i.mouse_pos.y - mouse_pos.y) / sh;
                    el.inner.config_mut().pos =
                        Rectangle::from_tuples((x, y), (x + w / sw, y + h / sh));
                    el.draw(i, g)
                })),
            ))));
        }
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            self.copy_on_mouse_down = self.copy;
        }
        vec![]
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            vec![GuiAction::SendToServer(Command::QueueGoto(
                self.path.clone(),
            ))]
        } else {
            vec![]
        }
    }
    fn key_watch(
        &mut self,
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        let p = self.path.clone();
        dragged_add_to_queue(dragged, move |q| Command::QueueAdd(p, q))
    }
}

fn dragged_add_to_queue<F: FnOnce(Queue) -> Command + 'static>(
    dragged: Dragging,
    f: F,
) -> Vec<GuiAction> {
    match dragged {
        Dragging::Artist(id) => {
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(q) = add_to_queue_artist_by_id(id, db) {
                    vec![GuiAction::SendToServer(f(q))]
                } else {
                    vec![]
                }
            }))]
        }
        Dragging::Album(id) => {
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(q) = add_to_queue_album_by_id(id, db) {
                    vec![GuiAction::SendToServer(f(q))]
                } else {
                    vec![]
                }
            }))]
        }
        Dragging::Song(id) => {
            let q = QueueContent::Song(id).into();
            vec![GuiAction::SendToServer(f(q))]
        }
        Dragging::Queue(q) => {
            vec![GuiAction::SendToServer(f(q))]
        }
    }
}

fn add_to_queue_album_by_id(id: AlbumId, db: &Database) -> Option<Queue> {
    if let Some(album) = db.albums().get(&id) {
        Some(
            QueueContent::Folder(
                0,
                album
                    .songs
                    .iter()
                    .map(|id| QueueContent::Song(*id).into())
                    .collect(),
                album.name.clone(),
            )
            .into(),
        )
    } else {
        None
    }
}
fn add_to_queue_artist_by_id(id: ArtistId, db: &Database) -> Option<Queue> {
    if let Some(artist) = db.artists().get(&id) {
        Some(
            QueueContent::Folder(
                0,
                artist
                    .albums
                    .iter()
                    .filter_map(|id| add_to_queue_album_by_id(*id, db))
                    .collect(),
                artist.name.clone(),
            )
            .into(),
        )
    } else {
        None
    }
}

// use musicdb_lib::{
//     data::{
//         database::Database,
//         queue::{Queue, QueueContent},
//         AlbumId, ArtistId,
//     },
//     server::Command,
// };
// use speedy2d::{
//     color::Color,
//     dimen::Vec2,
//     font::{TextLayout, TextOptions},
// };

// use crate::gui::{Dragging, DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait};

// pub struct QueueViewer {
//     config: GuiElemCfg,
//     children: Vec<GuiElem>,
//     /// 0.0 = bottom
//     scroll: f32,
// }
// impl QueueViewer {
//     pub fn new(config: GuiElemCfg) -> Self {
//         Self {
//             config: config.w_drag_target(),
//             children: vec![],
//             scroll: 0.0,
//         }
//     }
// }
// impl GuiElemTrait for QueueViewer {
//     fn config(&self) -> &GuiElemCfg {
//         &self.config
//     }
//     fn config_mut(&mut self) -> &mut GuiElemCfg {
//         &mut self.config
//     }
//     fn children(&mut self) -> Box<dyn Iterator<Item = &mut GuiElem> + '_> {
//         Box::new(self.children.iter_mut())
//     }
//     fn draw(&mut self, info: &DrawInfo, g: &mut speedy2d::Graphics2D) {
//         g.draw_rectangle(info.pos.clone(), Color::from_rgb(0.0, 0.1, 0.0));
//         let queue_height = info.database.queue.len();
//         let start_y_pos = info.pos.bottom_right().y
//             + (self.scroll - queue_height as f32) * info.queue_song_height;
//         let mut skip = 0;
//         let limit = queue_height.saturating_sub(self.scroll.floor() as usize + skip);
//         self.draw_queue(
//             &info.database.queue,
//             &mut skip,
//             &mut 0,
//             limit,
//             &mut Vec2::new(info.pos.top_left().x, start_y_pos),
//             info.pos.width(),
//             info,
//             g,
//         );
//     }
//     fn dragged(&mut self, dragged: Dragging) -> Vec<crate::gui::GuiAction> {
//         match dragged {
//             Dragging::Song(id) => vec![GuiAction::SendToServer(Command::QueueAdd(
//                 vec![],
//                 QueueContent::Song(id).into(),
//             ))],
//             Dragging::Album(id) => vec![GuiAction::Build(Box::new(move |db| {
//                 if let Some(q) = Self::add_to_queue_album_by_id(id, db) {
//                     vec![GuiAction::SendToServer(Command::QueueAdd(vec![], q))]
//                 } else {
//                     vec![]
//                 }
//             }))],
//             Dragging::Artist(id) => vec![GuiAction::Build(Box::new(move |db| {
//                 if let Some(q) = Self::add_to_queue_artist_by_id(id, db) {
//                     vec![GuiAction::SendToServer(Command::QueueAdd(vec![], q))]
//                 } else {
//                     vec![]
//                 }
//             }))],
//             _ => vec![],
//         }
//     }
// }
// impl QueueViewer {
//     fn add_to_queue_album_by_id(id: AlbumId, db: &Database) -> Option<Queue> {
//         if let Some(album) = db.albums().get(&id) {
//             Some(
//                 QueueContent::Folder(
//                     0,
//                     album
//                         .songs
//                         .iter()
//                         .map(|id| QueueContent::Song(*id).into())
//                         .collect(),
//                     album.name.clone(),
//                 )
//                 .into(),
//             )
//         } else {
//             None
//         }
//     }
//     fn add_to_queue_artist_by_id(id: ArtistId, db: &Database) -> Option<Queue> {
//         if let Some(artist) = db.artists().get(&id) {
//             Some(
//                 QueueContent::Folder(
//                     0,
//                     artist
//                         .albums
//                         .iter()
//                         .filter_map(|id| Self::add_to_queue_album_by_id(*id, db))
//                         .collect(),
//                     artist.name.clone(),
//                 )
//                 .into(),
//             )
//         } else {
//             None
//         }
//     }
// }

// const INDENT_PX: f32 = 8.0;

// impl QueueViewer {
//     fn draw_queue(
//         &mut self,
//         queue: &Queue,
//         skip: &mut usize,
//         drawn: &mut usize,
//         limit: usize,
//         top_left: &mut Vec2,
//         width: f32,
//         info: &DrawInfo,
//         g: &mut speedy2d::Graphics2D,
//     ) {
//         // eprintln!("[queue: {} : {}/{}]", *skip, *drawn, limit);
//         match queue.content() {
//             QueueContent::Song(id) => {
//                 if *skip == 0 {
//                     if *drawn < limit {
//                         *drawn += 1;
//                         let text = if let Some(song) = info.database.get_song(id) {
//                             song.title.clone()
//                         } else {
//                             format!("< {id} >")
//                         };
//                         let height = info
//                             .font
//                             .layout_text(&text, 1.0, TextOptions::new())
//                             .height();
//                         g.draw_text_cropped(
//                             top_left.clone(),
//                             info.pos.clone(),
//                             Color::from_int_rgb(112, 41, 99),
//                             &info.font.layout_text(
//                                 &text,
//                                 0.75 * info.queue_song_height / height,
//                                 TextOptions::new(),
//                             ),
//                         );
//                         top_left.y += info.queue_song_height;
//                     }
//                 } else {
//                     *skip -= 1;
//                 }
//             }
//             QueueContent::Folder(index, vec, _name) => {
//                 top_left.x += INDENT_PX;
//                 for v in vec {
//                     self.draw_queue(v, skip, drawn, limit, top_left, width - INDENT_PX, info, g);
//                 }
//                 top_left.x -= INDENT_PX;
//             }
//         }
//     }
// }
