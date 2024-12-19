use musicdb_lib::{
    data::{
        database::Database,
        queue::{Queue, QueueContent, QueueDuration},
        song::Song,
        AlbumId, ArtistId,
    },
    server::{Action, Req},
};
use speedy2d::{
    color::Color,
    dimen::Vec2,
    shape::Rectangle,
    window::{ModifiersState, MouseButton, VirtualKeyCode},
};

use crate::{
    gui::{Dragging, DrawInfo, EventInfo, GuiAction, GuiElem, GuiElemCfg},
    gui_base::{Panel, ScrollBox},
    gui_text::{self, AdvancedLabel, Label, TextField},
};

/*


This is responsible for showing the current queue,
with drag-n-drop only if the mouse leaves the element before it is released,
because simple clicks have to be GoTo events.

*/

pub struct QueueViewer {
    config: GuiElemCfg,
    c_scroll_box: ScrollBox<Vec<Box<dyn GuiElem>>>,
    c_empty_space_drag_handler: QueueEmptySpaceDragHandler,
    c_control_flow_elements: Panel<(QueueLoop, QueueLoop, QueueFolder, TextField)>,
    c_duration: AdvancedLabel,
    recv: std::sync::mpsc::Receiver<QVMsg>,
    queue_updated: bool,
}
pub enum QVMsg {
    ControlFlowElementsSetFolderName(String),
}
const QP_QUEUE1: f32 = 0.0;
const QP_QUEUE2: f32 = 0.95;
const QP_INV1: f32 = QP_QUEUE2;
const QP_INV2: f32 = 1.0;
impl QueueViewer {
    pub fn new(config: GuiElemCfg) -> Self {
        let (sender, recv) = std::sync::mpsc::channel();
        let control_flow_elements = (
            QueueLoop::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 0.5))).w_mouse(),
                vec![],
                QueueContent::Loop(
                    0,
                    0,
                    Box::new(
                        QueueContent::Folder(musicdb_lib::data::queue::QueueFolder {
                            index: 0,
                            content: vec![],
                            name: "in loop".to_string(),
                            order: None,
                        })
                        .into(),
                    ),
                )
                .into(),
                false,
            )
            .alwayscopy(),
            QueueLoop::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.5), (0.5, 1.0))).w_mouse(),
                vec![],
                QueueContent::Loop(
                    2,
                    0,
                    Box::new(
                        QueueContent::Folder(musicdb_lib::data::queue::QueueFolder {
                            index: 0,
                            content: vec![],
                            name: "in loop".to_string(),
                            order: None,
                        })
                        .into(),
                    ),
                )
                .into(),
                false,
            )
            .alwayscopy(),
            QueueFolder::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (1.0, 0.5))).w_mouse(),
                vec![],
                musicdb_lib::data::queue::QueueFolder {
                    index: 0,
                    content: vec![],
                    name: format!("folder name"),
                    order: None,
                },
                false,
            )
            .alwayscopy(),
            {
                let mut tf = TextField::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.5), (1.0, 1.0))),
                    format!("folder name"),
                    Color::from_rgb(0.0, 0.33, 0.0),
                    Color::from_rgb(0.0, 0.67, 0.0),
                );
                tf.on_changed = Some(Box::new(move |folder_name| {
                    _ = sender.send(QVMsg::ControlFlowElementsSetFolderName(
                        folder_name.to_owned(),
                    ));
                }));
                tf
            },
        );
        Self {
            config,
            c_scroll_box: ScrollBox::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2))),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                vec![],
                vec![],
                0.0,
            ),
            c_empty_space_drag_handler: QueueEmptySpaceDragHandler::new(GuiElemCfg::at(
                Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2)),
            )),
            c_control_flow_elements: Panel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_INV1), (0.5, QP_INV2))),
                control_flow_elements,
            ),
            c_duration: AdvancedLabel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.5, QP_INV1), (1.0, QP_INV2))),
                Vec2::new(0.0, 0.5),
                vec![],
            ),
            queue_updated: false,
            recv,
        }
    }
}
impl GuiElem for QueueViewer {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_scroll_box.elem_mut(),
                self.c_empty_space_drag_handler.elem_mut(),
                self.c_control_flow_elements.elem_mut(),
                self.c_duration.elem_mut(),
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        while let Ok(msg) = self.recv.try_recv() {
            match msg {
                QVMsg::ControlFlowElementsSetFolderName(name) => {
                    *self
                        .c_control_flow_elements
                        .children
                        .2
                        .c_name
                        .content
                        .text() = name.clone();
                    self.c_control_flow_elements.children.2.queue.name = name;
                }
            }
        }
        if self.queue_updated {
            self.queue_updated = false;
            let label = &mut self.c_duration;
            fn fmt_dur(dur: QueueDuration) -> String {
                if dur.infinite {
                    "∞".to_owned()
                } else {
                    let seconds = dur.millis / 1000;
                    let minutes = seconds / 60;
                    let h = minutes / 60;
                    let m = minutes % 60;
                    let s = seconds % 60;
                    if dur.random_counter == 0 {
                        if h > 0 {
                            format!("{h}:{m:0>2}:{s:0>2}")
                        } else {
                            format!("{m:0>2}:{s:0>2}")
                        }
                    } else {
                        let r = dur.random_counter;
                        if dur.millis > 0 {
                            if h > 0 {
                                format!("{h}:{m:0>2}:{s:0>2} + {r} random songs")
                            } else {
                                format!("{m:0>2}:{s:0>2} + {r} random songs")
                            }
                        } else {
                            format!("{r} random songs")
                        }
                    }
                }
            }
            let dt = fmt_dur(info.database.queue.duration_total(&info.database));
            let dr = fmt_dur(info.database.queue.duration_remaining(&info.database));
            label.content = vec![
                vec![(
                    gui_text::AdvancedContent::Text(gui_text::Content::new(
                        format!("Total: {dt}"),
                        Color::GRAY,
                    )),
                    1.0,
                    1.0,
                )],
                vec![(
                    gui_text::AdvancedContent::Text(gui_text::Content::new(
                        format!("Remaining: {dr}"),
                        Color::GRAY,
                    )),
                    1.0,
                    1.0,
                )],
            ];
            label.config_mut().redraw = true;
        }
        if self.config.redraw || info.pos.size() != self.config.pixel_pos.size() {
            self.config.redraw = false;
            let mut c = vec![];
            let mut h = vec![];
            queue_gui(
                &info.database.queue,
                &info.database,
                0.0,
                0.02,
                info.line_height,
                &mut c,
                &mut h,
                vec![],
                true,
                true,
            );
            let scroll_box = &mut self.c_scroll_box;
            scroll_box.children = c;
            scroll_box.children_heights = h;
            scroll_box.config_mut().redraw = true;
        }
    }
    fn updated_queue(&mut self) {
        self.queue_updated = true;
        self.config.redraw = true;
    }
}

fn queue_gui(
    queue: &Queue,
    db: &Database,
    depth: f32,
    depth_inc_by: f32,
    line_height: f32,
    target: &mut Vec<Box<dyn GuiElem>>,
    target_h: &mut Vec<f32>,
    path: Vec<usize>,
    current: bool,
    skip_folder: bool,
) {
    let cfg = GuiElemCfg::at(Rectangle::from_tuples((depth, 0.0), (1.0, 1.0)));
    match queue.content() {
        QueueContent::Song(id) => {
            if let Some(s) = db.songs().get(id) {
                target.push(Box::new(QueueSong::new(
                    cfg,
                    path,
                    s.clone(),
                    current,
                    db,
                    depth_inc_by * 0.33,
                )));
                target_h.push(line_height * 1.75);
            }
        }
        QueueContent::Folder(qf) => {
            let musicdb_lib::data::queue::QueueFolder {
                index: ia,
                content: _,
                name: _,
                order: _,
            } = qf;
            if !skip_folder {
                target.push(Box::new(QueueFolder::new(
                    cfg.clone(),
                    path.clone(),
                    qf.clone(),
                    current,
                )));
                target_h.push(line_height * 0.8);
            }
            for (i, q) in qf.iter().enumerate() {
                let mut p = path.clone();
                p.push(i);
                queue_gui(
                    q,
                    db,
                    depth + depth_inc_by,
                    depth_inc_by,
                    line_height,
                    target,
                    target_h,
                    p,
                    current && *ia == i,
                    false,
                );
            }
            if !skip_folder {
                let mut p1 = path;
                let p2 = p1.pop().unwrap_or(0) + 1;
                target.push(Box::new(QueueIndentEnd::new(cfg, (p1, p2))));
                target_h.push(line_height * 0.4);
            }
        }
        QueueContent::Loop(_, _, inner) => {
            let mut p = path.clone();
            p.push(0);
            let mut p1 = path.clone();
            let p2 = p1.pop().unwrap_or(0) + 1;
            target.push(Box::new(QueueLoop::new(
                cfg.clone(),
                path,
                queue.clone(),
                current,
            )));
            target_h.push(line_height * 0.8);
            queue_gui(
                &inner,
                db,
                depth,
                depth_inc_by,
                line_height,
                target,
                target_h,
                p,
                current,
                true,
            );
            target.push(Box::new(QueueIndentEnd::new(cfg, (p1, p2))));
            target_h.push(line_height * 0.4);
        }
    }
}

struct QueueEmptySpaceDragHandler {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElem>>,
}
impl QueueEmptySpaceDragHandler {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config: config.w_drag_target(),
            children: vec![],
        }
    }
}
impl GuiElem for QueueEmptySpaceDragHandler {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        e.take();
        dragged_add_to_queue(
            dragged,
            (),
            |_, q| Action::QueueAdd(vec![], q, Req::none()),
            |_, q| Action::QueueMoveInto(q, vec![]),
        )
    }
}

fn generic_queue_draw(
    info: &mut DrawInfo,
    path: &Vec<usize>,
    queue: impl FnOnce() -> Queue,
    mouse: &mut bool,
    copy_on_mouse_down: bool,
) {
    if *mouse && !info.pos.contains(info.mouse_pos) {
        // mouse left our element
        *mouse = false;
        info.actions.push(GuiAction::SetDragging(Some((
            Dragging::Queue(if copy_on_mouse_down {
                Ok(queue())
            } else {
                Err(path.clone())
            }),
            None,
        ))));
    }
}

struct QueueSong {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElem>>,
    path: Vec<usize>,
    song: Song,
    current: bool,
    insert_below: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    always_copy: bool,
    copy_on_mouse_down: bool,
}
impl QueueSong {
    pub fn new(
        config: GuiElemCfg,
        path: Vec<usize>,
        song: Song,
        current: bool,
        db: &Database,
        sub_offset: f32,
    ) -> Self {
        Self {
            config: config.w_mouse().w_keyboard_watch().w_drag_target(),
            children: vec![
                Box::new(AdvancedLabel::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.57))),
                    Vec2::new(0.0, 0.5),
                    vec![vec![
                        (
                            gui_text::AdvancedContent::Text(gui_text::Content::new(
                                song.title.clone(),
                                if current {
                                    Color::from_int_rgb(194, 76, 178)
                                } else {
                                    Color::from_int_rgb(120, 76, 194)
                                },
                            )),
                            1.0,
                            1.0,
                        ),
                        (
                            gui_text::AdvancedContent::Text(gui_text::Content::new(
                                {
                                    let duration = song.duration_millis / 1000;
                                    format!("  {}:{:0>2}", duration / 60, duration % 60)
                                },
                                if current {
                                    Color::GRAY
                                } else {
                                    Color::DARK_GRAY
                                },
                            )),
                            0.6,
                            1.0,
                        ),
                    ]],
                )),
                Box::new(Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((sub_offset, 0.57), (1.0, 1.0))),
                    match (
                        db.artists().get(&song.artist),
                        song.album.as_ref().and_then(|id| db.albums().get(id)),
                    ) {
                        (None, None) => String::new(),
                        (Some(artist), None) => format!("by {}", artist.name),
                        (None, Some(album)) => {
                            if let Some(artist) = db.artists().get(&album.artist) {
                                format!("on {} by {}", album.name, artist.name)
                            } else {
                                format!("on {}", album.name)
                            }
                        }
                        (Some(artist), Some(album)) => {
                            format!("by {} on {}", artist.name, album.name)
                        }
                    },
                    if current {
                        Color::from_int_rgb(97, 38, 89)
                    } else {
                        Color::from_int_rgb(60, 38, 97)
                    },
                    None,
                    Vec2::new(0.0, 0.5),
                )),
            ],
            path,
            song,
            current,
            insert_below: false,
            mouse: false,
            mouse_pos: Vec2::ZERO,
            copy: false,
            always_copy: false,
            copy_on_mouse_down: false,
        }
    }
}

impl GuiElem for QueueSong {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn mouse_down(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left && e.take() {
            self.mouse = true;
            self.copy_on_mouse_down = self.copy;
        }
        vec![]
    }
    fn mouse_up(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            if e.take() && !self.always_copy {
                vec![GuiAction::SendToServer(Action::QueueGoto(
                    self.path.clone(),
                ))]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        self.insert_below = info.mouse_pos.y > info.pos.top_left().y + info.pos.height() * 0.5;
        if !self.always_copy && info.dragging.is_some() && info.pos.contains(info.mouse_pos) {
            g.draw_rectangle(
                if self.insert_below {
                    Rectangle::new(
                        Vec2::new(
                            info.pos.top_left().x,
                            info.pos.top_left().y + info.pos.height() * 0.75,
                        ),
                        *info.pos.bottom_right(),
                    )
                } else {
                    Rectangle::new(
                        *info.pos.top_left(),
                        Vec2::new(
                            info.pos.bottom_right().x,
                            info.pos.top_left().y + info.pos.height() * 0.25,
                        ),
                    )
                },
                Color::from_rgba(1.0, 1.0, 1.0, 0.25),
            );
        }
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        generic_queue_draw(
            info,
            &self.path,
            || QueueContent::Song(self.song.id).into(),
            &mut self.mouse,
            self.copy_on_mouse_down,
        );
    }
    fn key_watch(
        &mut self,
        _e: &mut EventInfo,
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = self.always_copy || modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            e.take();
            let insert_below = self.insert_below;
            dragged_add_to_queue(
                dragged,
                self.path.clone(),
                move |mut p: Vec<usize>, q| {
                    if let Some(j) = p.pop() {
                        Action::QueueInsert(p, if insert_below { j + 1 } else { j }, q, Req::none())
                    } else {
                        Action::QueueAdd(p, q, Req::none())
                    }
                },
                move |mut p, q| {
                    if insert_below {
                        if let Some(l) = p.last_mut() {
                            *l += 1;
                        }
                    }
                    Action::QueueMove(q, p)
                },
            )
        } else {
            vec![]
        }
    }
}

struct QueueFolder {
    config: GuiElemCfg,
    c_name: Label,
    path: Vec<usize>,
    queue: musicdb_lib::data::queue::QueueFolder,
    current: bool,
    insert_into: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    always_copy: bool,
    copy_on_mouse_down: bool,
}
impl QueueFolder {
    pub fn new(
        config: GuiElemCfg,
        path: Vec<usize>,
        queue: musicdb_lib::data::queue::QueueFolder,
        current: bool,
    ) -> Self {
        let musicdb_lib::data::queue::QueueFolder {
            index: _,
            content,
            name,
            order,
        } = &queue;
        Self {
            config: if path.is_empty() {
                config
            } else {
                config.w_mouse().w_keyboard_watch()
            }
            .w_drag_target(),
            c_name: Label::new(
                GuiElemCfg::default(),
                format!(
                    "{}  ({}){}",
                    if path.is_empty() && name.is_empty() {
                        "Queue"
                    } else {
                        name
                    },
                    content.len(),
                    if order.is_some() { " [shuffled]" } else { "" },
                ),
                Color::from_int_rgb(52, 132, 50),
                None,
                Vec2::new(0.0, 0.5),
            ),
            path,
            queue,
            current,
            insert_into: false,
            mouse: false,
            mouse_pos: Vec2::ZERO,
            copy: false,
            always_copy: false,
            copy_on_mouse_down: false,
        }
    }
    fn alwayscopy(mut self) -> Self {
        self.always_copy = true;
        self.copy = true;
        self.config.scroll_events = true;
        self
    }
}
impl GuiElem for QueueFolder {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_name.elem_mut()].into_iter())
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
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        self.insert_into = info.mouse_pos.y > info.pos.top_left().y + info.pos.height() * 0.5;
        if !self.always_copy && info.dragging.is_some() && info.pos.contains(info.mouse_pos) {
            g.draw_rectangle(
                if self.insert_into {
                    Rectangle::new(
                        Vec2::new(
                            info.pos.top_left().x,
                            info.pos.top_left().y + info.pos.height() * 0.5,
                        ),
                        *info.pos.bottom_right(),
                    )
                } else {
                    Rectangle::new(
                        *info.pos.top_left(),
                        Vec2::new(
                            info.pos.bottom_right().x,
                            info.pos.top_left().y + info.pos.height() * 0.25,
                        ),
                    )
                },
                Color::from_rgba(1.0, 1.0, 1.0, 0.25),
            );
        }
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        generic_queue_draw(
            info,
            &self.path,
            || QueueContent::Folder(self.queue.clone()).into(),
            &mut self.mouse,
            self.copy_on_mouse_down,
        );
    }
    fn mouse_down(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left && e.take() {
            self.mouse = true;
            self.copy_on_mouse_down = self.copy;
        } else if button == MouseButton::Right && e.take() {
            // return vec![GuiAction::ContextMenu(Some(vec![Box::new(
            //     Panel::with_background(GuiElemCfg::default(), (), Color::DARK_GRAY),
            // )]))];
            return vec![GuiAction::SendToServer(if self.queue.order.is_some() {
                Action::QueueUnshuffle(self.path.clone())
            } else {
                Action::QueueShuffle(self.path.clone())
            })];
        }
        vec![]
    }
    fn mouse_up(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            if e.take() && !self.always_copy {
                vec![GuiAction::SendToServer(Action::QueueGoto(
                    self.path.clone(),
                ))]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn key_watch(
        &mut self,
        _e: &mut EventInfo,
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            e.take();
            if self.insert_into {
                dragged_add_to_queue(
                    dragged,
                    self.path.clone(),
                    |p, q| Action::QueueAdd(p, q, Req::none()),
                    |p, q| Action::QueueMoveInto(q, p),
                )
            } else {
                dragged_add_to_queue(
                    dragged,
                    self.path.clone(),
                    |mut p, q| {
                        let j = p.pop().unwrap_or(0);
                        Action::QueueInsert(p, j, q, Req::none())
                    },
                    |p, q| Action::QueueMove(q, p),
                )
            }
        } else {
            vec![]
        }
    }
}
pub struct QueueIndentEnd {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElem>>,
    path_insert: (Vec<usize>, usize),
}
impl QueueIndentEnd {
    pub fn new(config: GuiElemCfg, path_insert: (Vec<usize>, usize)) -> Self {
        Self {
            config: config.w_drag_target(),
            children: vec![],
            path_insert,
        }
    }
}
impl GuiElem for QueueIndentEnd {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        if info.dragging.is_some() {
            g.draw_rectangle(
                info.pos.clone(),
                Color::from_rgba(
                    1.0,
                    1.0,
                    1.0,
                    if info.pos.contains(info.mouse_pos) {
                        0.3
                    } else {
                        0.2
                    },
                ),
            );
        }
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        e.take();
        dragged_add_to_queue(
            dragged,
            self.path_insert.clone(),
            |(p, j), q| Action::QueueInsert(p, j, q, Req::none()),
            |(mut p, j), q| {
                p.push(j);
                Action::QueueMove(q, p)
            },
        )
    }
}

struct QueueLoop {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElem>>,
    path: Vec<usize>,
    queue: Queue,
    current: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    always_copy: bool,
    copy_on_mouse_down: bool,
}
impl QueueLoop {
    pub fn new(config: GuiElemCfg, path: Vec<usize>, queue: Queue, current: bool) -> Self {
        Self {
            config: if path.is_empty() {
                config
            } else {
                config.w_mouse().w_keyboard_watch()
            }
            .w_drag_target(),
            children: vec![Box::new(Label::new(
                GuiElemCfg::default(),
                Self::get_label_text(&queue),
                Color::from_int_rgb(217, 197, 65),
                None,
                Vec2::new(0.0, 0.5),
            ))],
            path,
            queue,
            current,
            mouse: false,
            mouse_pos: Vec2::ZERO,
            copy: false,
            always_copy: false,
            copy_on_mouse_down: false,
        }
    }
    fn alwayscopy(mut self) -> Self {
        self.always_copy = true;
        self.copy = true;
        self.config.scroll_events = true;
        self
    }
    fn get_label_text(queue: &Queue) -> String {
        match queue.content() {
            QueueContent::Loop(total, _current, _) => {
                if *total == 0 {
                    format!("repeat forever")
                } else if *total == 1 {
                    format!("repeat 1 time")
                } else {
                    format!("repeat {total} times")
                }
            }
            _ => "[???]".to_string(),
        }
    }
}
impl GuiElem for QueueLoop {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn mouse_wheel(&mut self, e: &mut EventInfo, diff: f32) -> Vec<GuiAction> {
        if self.always_copy && e.take() {
            if let QueueContent::Loop(total, _, _) = self.queue.content_mut() {
                if diff > 0.0 {
                    *total += 1;
                } else if diff < 0.0 && *total > 0 {
                    *total -= 1;
                }
            }
            *self.children[0]
                .any_mut()
                .downcast_mut::<Label>()
                .unwrap()
                .content
                .text() = Self::get_label_text(&self.queue);
        }
        vec![]
    }
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        generic_queue_draw(
            info,
            &self.path,
            || self.queue.clone(),
            &mut self.mouse,
            self.copy_on_mouse_down,
        );
    }
    fn mouse_down(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left && e.take() {
            self.mouse = true;
            self.copy_on_mouse_down = self.copy;
        }
        vec![]
    }
    fn mouse_up(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            if e.take() && !self.always_copy {
                vec![GuiAction::SendToServer(Action::QueueGoto(
                    self.path.clone(),
                ))]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn key_watch(
        &mut self,
        _e: &mut EventInfo,
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            e.take();
            let mut p = self.path.clone();
            p.push(0);
            dragged_add_to_queue(
                dragged,
                p,
                |p, q| Action::QueueAdd(p, q, Req::none()),
                |p, q| Action::QueueMoveInto(q, p),
            )
        } else {
            vec![]
        }
    }
}

fn dragged_add_to_queue<T: 'static>(
    dragged: Dragging,
    data: T,
    f_queues: impl FnOnce(T, Vec<Queue>) -> Action + 'static,
    f_queue_by_path: impl FnOnce(T, Vec<usize>) -> Action + 'static,
) -> Vec<GuiAction> {
    match dragged {
        Dragging::Artist(id) => {
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(q) = add_to_queue_artist_by_id(id, db) {
                    vec![GuiAction::SendToServer(f_queues(data, vec![q]))]
                } else {
                    vec![]
                }
            }))]
        }
        Dragging::Album(id) => {
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(q) = add_to_queue_album_by_id(id, db) {
                    vec![GuiAction::SendToServer(f_queues(data, vec![q]))]
                } else {
                    vec![]
                }
            }))]
        }
        Dragging::Song(id) => {
            let q = QueueContent::Song(id).into();
            vec![GuiAction::SendToServer(f_queues(data, vec![q]))]
        }
        Dragging::Queue(q) => vec![GuiAction::SendToServer(match q {
            Ok(q) => f_queues(data, vec![q]),
            Err(p) => f_queue_by_path(data, p),
        })],
        Dragging::Queues(q) => vec![GuiAction::SendToServer(f_queues(data, q))],
    }
}

fn add_to_queue_album_by_id(id: AlbumId, db: &Database) -> Option<Queue> {
    if let Some(album) = db.albums().get(&id) {
        Some(
            QueueContent::Folder(musicdb_lib::data::queue::QueueFolder {
                index: 0,
                content: album
                    .songs
                    .iter()
                    .map(|id| QueueContent::Song(*id).into())
                    .collect(),
                name: album.name.clone(),
                order: None,
            })
            .into(),
        )
    } else {
        None
    }
}
fn add_to_queue_artist_by_id(id: ArtistId, db: &Database) -> Option<Queue> {
    if let Some(artist) = db.artists().get(&id) {
        Some(
            QueueContent::Folder(musicdb_lib::data::queue::QueueFolder {
                index: 0,
                content: artist
                    .singles
                    .iter()
                    .map(|id| QueueContent::Song(*id).into())
                    .chain(
                        artist
                            .albums
                            .iter()
                            .filter_map(|id| add_to_queue_album_by_id(*id, db)),
                    )
                    .collect(),
                name: artist.name.clone(),
                order: None,
            })
            .into(),
        )
    } else {
        None
    }
}
