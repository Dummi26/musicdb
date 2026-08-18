use std::{borrow::Cow, rc::Rc};

use musicdb_lib::{
    data::{
        AlbumId, ArtistId,
        database::Database,
        queue::{Queue, QueueContent, QueueDuration},
        song::Song,
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
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{self, AdvancedLabel, Label, TextField},
};

/*


This is responsible for showing the current queue,
with drag-n-drop only if the mouse leaves the element before it is released,
because simple clicks have to be GoTo events.

*/

const EMPTY_QUEUE: &Queue = &Queue::empty_folder();

fn q<'a>(db: &'a Database, saved: &Option<String>) -> &'a Queue {
    if let Some(name) = saved {
        db.queues.get(name).unwrap_or(EMPTY_QUEUE)
    } else {
        &db.queue
    }
}
trait HasQueue {
    fn queue<'a>(&'a self, saved: &Option<String>) -> &'a Queue;
}
impl<'a> HasQueue for DrawInfo<'a> {
    fn queue<'b>(&'b self, saved: &Option<String>) -> &'b Queue {
        q(self.database, saved)
    }
}
fn wrap(saved: &Option<Rc<String>>, action: Action) -> Action {
    if let Some(queue) = saved {
        Action::SavedQueue(queue.as_ref().to_owned(), vec![action])
    } else {
        action
    }
}

pub struct QueueViewer {
    config: GuiElemCfg,
    /// If `Some(_)`, `self` shows a saved queue (playlist)
    /// instead of the active, playing queue.
    /// After changing this, call `updated_queue()`.
    pub saved: Option<String>,
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
    pub fn new_active_queue(config: GuiElemCfg) -> Self {
        Self::new_impl(config, None)
    }
    pub fn new_saved_queue(config: GuiElemCfg, queue: String) -> Self {
        Self::new_impl(config, Some(queue))
    }
    fn new_impl(config: GuiElemCfg, saved: Option<String>) -> Self {
        let (sender, recv) = std::sync::mpsc::channel();
        let s1 = saved.as_ref().map(|v| Rc::new(v.to_owned()));
        let control_flow_elements = (
            QueueLoop::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 0.5))).w_mouse(),
                s1.clone(),
                vec![],
                QueueContent::Loop(
                    0,
                    0,
                    Box::new(
                        QueueContent::Folder(musicdb_lib::data::queue::QueueFolder {
                            index: 0,
                            content: vec![],
                            name: String::new(),
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
                s1.clone(),
                vec![],
                QueueContent::Loop(
                    2,
                    0,
                    Box::new(
                        QueueContent::Folder(musicdb_lib::data::queue::QueueFolder {
                            index: 0,
                            content: vec![],
                            name: String::new(),
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
                s1.clone(),
                vec![],
                musicdb_lib::data::queue::QueueFolder {
                    index: 0,
                    content: vec![],
                    name: "folder name".to_owned(),
                    order: None,
                },
                false,
            )
            .alwayscopy(),
            {
                let mut tf = TextField::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.5), (1.0, 1.0))),
                    "folder name".to_owned(),
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
            saved,
            c_scroll_box: ScrollBox::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2))),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                vec![],
                vec![],
                0.0,
            ),
            c_empty_space_drag_handler: QueueEmptySpaceDragHandler::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2))),
                s1.clone(),
            ),
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
            let s1 = self.saved.as_ref().map(|v| Rc::new(v.to_owned()));
            self.c_empty_space_drag_handler.saved = s1;
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
            let dt = fmt_dur(info.queue(&self.saved).duration_total(info.database));
            let dr = fmt_dur(info.queue(&self.saved).duration_remaining(info.database));
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
            label.config_mut().redraw_once();
        }
        if self.config.redraw() || info.pos.size() != self.config.pixel_pos.size() {
            self.config.redrawn();
            let s1 = self.saved.as_ref().map(|v| Rc::new(v.to_owned()));
            let mut c = vec![];
            let mut h = vec![];
            queue_gui(
                info.queue(&self.saved),
                &s1,
                info.database,
                0.0,
                0.02,
                info.line_height,
                &mut c,
                &mut h,
                vec![],
                true,
                false,
            );
            let scroll_box = &mut self.c_scroll_box;
            scroll_box.children = c;
            scroll_box.children_heights = h;
            scroll_box.config_mut().redraw_once();
        }
    }
    fn updated_library(&mut self) {
        self.updated_queue();
    }
    fn updated_queue(&mut self) {
        self.queue_updated = true;
        self.config.redraw_once();
    }
}

fn queue_gui(
    queue: &Queue,
    saved: &Option<Rc<String>>,
    db: &Database,
    depth: f32,
    depth_inc_by: f32,
    line_height: f32,
    target: &mut Vec<Box<dyn GuiElem>>,
    target_h: &mut Vec<f32>,
    path: Vec<usize>,
    current: bool,
    skip_first: bool,
) -> Option<Box<dyn GuiElem>> {
    let mut out = None;
    let mut push = |target: &mut Vec<_>, e| {
        if skip_first && out.is_none() {
            out = Some(e);
        } else {
            target.push(e);
        }
    };
    let is_root = path.is_empty();
    let cfg = GuiElemCfg::at(Rectangle::from_tuples((depth, 0.0), (1.0, 1.0)));
    match queue.content() {
        QueueContent::Song(id) => {
            if let Some(s) = db.songs().get(id) {
                push(
                    target,
                    Box::new(QueueSong::new(
                        cfg,
                        saved.clone(),
                        path,
                        s.clone(),
                        current,
                        db,
                        depth_inc_by * 0.33,
                    )),
                );
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
            let mut folder = QueueFolder::new(
                cfg.clone(),
                saved.clone(),
                path.clone(),
                qf.clone(),
                current,
            );
            if skip_first || is_root {
                folder.no_ins_before = true;
            }
            push(target, Box::new(folder));
            target_h.push(line_height * 0.8);
            for (i, q) in qf.iter().enumerate() {
                let mut p = path.clone();
                p.push(i);
                queue_gui(
                    q,
                    saved,
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
            if !is_root {
                let mut p1 = path;
                let p2 = p1.pop().unwrap_or(0) + 1;
                push(
                    target,
                    Box::new(QueueIndentEnd::new(cfg, saved.clone(), (p1, p2))),
                );
                target_h.push(line_height * 0.4);
            }
        }
        QueueContent::Loop(_, _, inner) => {
            let mut p = path.clone();
            p.push(0);
            let i = target.len();
            push(
                target,
                Box::new(QueueLoop::new(
                    cfg.clone(),
                    saved.clone(),
                    path,
                    queue.clone(),
                    current,
                )),
            );
            if let Some(mut inner) = queue_gui(
                inner,
                saved,
                db,
                depth,
                depth_inc_by,
                line_height,
                target,
                target_h,
                p,
                current,
                true,
            ) {
                inner.config_mut().pos = Rectangle::from_tuples((0.5, 0.0), (1.0, 1.0));
                target[i]
                    .any_mut()
                    .downcast_mut::<QueueLoop>()
                    .unwrap()
                    .inner = Some(inner);
            }
        }
    }
    out
}

struct QueueEmptySpaceDragHandler {
    config: GuiElemCfg,
    saved: Option<Rc<String>>,
    children: Vec<Box<dyn GuiElem>>,
}
impl QueueEmptySpaceDragHandler {
    pub fn new(config: GuiElemCfg, saved: Option<Rc<String>>) -> Self {
        Self {
            config: config.w_drag_target(),
            saved,
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
        let s1 = self.saved.clone();
        let s2 = self.saved.clone();
        dragged_add_to_queue(
            dragged,
            &self.saved,
            (),
            move |_, q| wrap(&s1, Action::QueueAdd(vec![], q, Req::none())),
            move |_, q| wrap(&s2, Action::QueueMoveInto(q, vec![])),
        )
    }
}

fn generic_queue_draw(
    info: &mut DrawInfo,
    saved: &Option<Rc<String>>,
    path: &[usize],
    queue: impl FnOnce() -> Queue,
    mouse: &mut bool,
    copy_on_mouse_down: bool,
) {
    if *mouse && !info.pos.contains(info.mouse_pos) {
        // mouse left our element
        *mouse = false;
        info.actions.push(GuiAction::SetDragging(Some((
            if copy_on_mouse_down {
                Dragging::Queue(queue(), None)
            } else {
                Dragging::Queue(
                    queue(),
                    Some((saved.as_ref().map(|v| v.as_ref().to_owned()), path.to_vec())),
                )
            },
            None,
        ))));
    }
}

struct QueueSong {
    config: GuiElemCfg,
    saved: Option<Rc<String>>,
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
        saved: Option<Rc<String>>,
        path: Vec<usize>,
        song: Song,
        current: bool,
        db: &Database,
        sub_offset: f32,
    ) -> Self {
        Self {
            config: config.w_mouse().w_keyboard_watch().w_drag_target(),
            saved,
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
    fn alwayscopy(mut self) -> Self {
        self.always_copy = true;
        self.copy = true;
        self.config.scroll_events = true;
        self
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
            vec![]
        } else if button == MouseButton::Right && e.take() {
            let me = self.song.clone();
            let menu_actions: Vec<Box<dyn GuiElem + 'static>> = vec![Box::new(Button::new(
                GuiElemCfg::default(),
                move |_| vec![GuiAction::EditSongs(vec![me.clone()])],
                [Label::new(
                    GuiElemCfg::default(),
                    "Edit this song".to_owned(),
                    Color::WHITE,
                    None,
                    Vec2::new_y(0.5),
                )],
            ))];
            vec![GuiAction::ContextMenu(Some(menu_actions))]
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        let s1 = self.saved.clone();
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            if e.take() && !self.always_copy {
                vec![GuiAction::SendToServer(wrap(
                    &s1,
                    Action::QueueGoto(self.path.clone()),
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
            &self.saved,
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
        down: bool,
        key: Option<VirtualKeyCode>,
        scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = self.always_copy || key_watch_ctrl(&modifiers, down, key);
        vec![]
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            e.take();
            let insert_below = self.insert_below;
            let s1 = self.saved.clone();
            let s2 = self.saved.clone();
            dragged_add_to_queue(
                dragged,
                &self.saved,
                self.path.clone(),
                move |mut p: Vec<usize>, q| {
                    wrap(
                        &s1,
                        if let Some(j) = p.pop() {
                            Action::QueueInsert(
                                p,
                                if insert_below { j + 1 } else { j },
                                q,
                                Req::none(),
                            )
                        } else {
                            Action::QueueAdd(p, q, Req::none())
                        },
                    )
                },
                move |mut p, q| {
                    if insert_below && let Some(l) = p.last_mut() {
                        *l += 1;
                    }
                    wrap(&s2, Action::QueueMove(q, p))
                },
            )
        } else {
            vec![]
        }
    }
}

fn key_watch_ctrl(modifiers: &ModifiersState, down: bool, key: Option<VirtualKeyCode>) -> bool {
    if let Some(key) = key
        && matches!(key, VirtualKeyCode::LControl | VirtualKeyCode::RControl)
    {
        down
    } else {
        modifiers.ctrl()
    }
}

struct QueueFolder {
    config: GuiElemCfg,
    saved: Option<Rc<String>>,
    c_name: Label,
    path: Vec<usize>,
    queue: musicdb_lib::data::queue::QueueFolder,
    current: bool,
    insert_into: bool,
    no_ins_before: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    always_copy: bool,
    copy_on_mouse_down: bool,
}
impl QueueFolder {
    pub fn new(
        config: GuiElemCfg,
        saved: Option<Rc<String>>,
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
            config: config.w_mouse().w_keyboard_watch().w_drag_target(),
            c_name: Label::new(
                GuiElemCfg::default(),
                format!(
                    "{}  ({}){}",
                    if path.is_empty() && name.is_empty() {
                        if let Some(saved) = &saved {
                            if saved.is_empty() {
                                Cow::Borrowed("Unnamed playlist")
                            } else {
                                Cow::Owned(format!("Playlist \"{saved}\""))
                            }
                        } else {
                            Cow::Borrowed("Queue")
                        }
                    } else {
                        Cow::Borrowed(name.as_str())
                    },
                    content.len(),
                    if order.is_some() { " [shuffled]" } else { "" },
                ),
                Color::from_int_rgb(52, 132, 50),
                None,
                Vec2::new(0.0, 0.5),
            ),
            saved,
            path,
            queue,
            current,
            insert_into: false,
            no_ins_before: false,
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
        self.insert_into = self.no_ins_before
            || info.mouse_pos.y > info.pos.top_left().y + info.pos.height() * 0.5;
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
        let name = self.path.is_empty().then(|| self.saved.clone());
        generic_queue_draw(
            info,
            &self.saved,
            &self.path,
            || {
                let mut folder = self.queue.clone();
                if folder.name.is_empty()
                    && let Some(name) = name
                {
                    folder.name = if let Some(name) = name {
                        name.as_ref().to_owned()
                    } else {
                        "Queue".to_owned()
                    };
                }
                QueueContent::Folder(folder).into()
            },
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
            return vec![GuiAction::SendToServer(wrap(
                &self.saved,
                if self.queue.order.is_some() {
                    Action::QueueUnshuffle(self.path.clone())
                } else {
                    Action::QueueShuffle(self.path.clone(), 1)
                },
            ))];
        }
        vec![]
    }
    fn mouse_up(&mut self, e: &mut EventInfo, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            if e.take() && !self.always_copy {
                vec![GuiAction::SendToServer(wrap(
                    &self.saved,
                    Action::QueueGoto(self.path.clone()),
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
        down: bool,
        key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = self.always_copy || key_watch_ctrl(&modifiers, down, key);
        vec![]
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            e.take();
            if self.insert_into {
                let s1 = self.saved.clone();
                let s2 = self.saved.clone();
                dragged_add_to_queue(
                    dragged,
                    &self.saved,
                    self.path.clone(),
                    move |p, q| wrap(&s1, Action::QueueAdd(p, q, Req::none())),
                    move |p, q| wrap(&s2, Action::QueueMoveInto(q, p)),
                )
            } else {
                let s1 = self.saved.clone();
                let s2 = self.saved.clone();
                dragged_add_to_queue(
                    dragged,
                    &self.saved,
                    self.path.clone(),
                    move |mut p, q| {
                        let j = p.pop().unwrap_or(0);
                        wrap(&s1, Action::QueueInsert(p, j, q, Req::none()))
                    },
                    move |p, q| wrap(&s2, Action::QueueMove(q, p)),
                )
            }
        } else {
            vec![]
        }
    }
}
pub struct QueueIndentEnd {
    config: GuiElemCfg,
    saved: Option<Rc<String>>,
    children: Vec<Box<dyn GuiElem>>,
    path_insert: (Vec<usize>, usize),
}
impl QueueIndentEnd {
    pub fn new(
        config: GuiElemCfg,
        saved: Option<Rc<String>>,
        path_insert: (Vec<usize>, usize),
    ) -> Self {
        Self {
            config: config.w_drag_target(),
            saved,
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
        let s1 = self.saved.clone();
        let s2 = self.saved.clone();
        dragged_add_to_queue(
            dragged,
            &self.saved,
            self.path_insert.clone(),
            move |(p, j), q| wrap(&s1, Action::QueueInsert(p, j, q, Req::none())),
            move |(mut p, j), q| {
                p.push(j);
                wrap(&s2, Action::QueueMove(q, p))
            },
        )
    }
}

struct QueueLoop {
    config: GuiElemCfg,
    saved: Option<Rc<String>>,
    children: Vec<Box<dyn GuiElem>>,
    path: Vec<usize>,
    queue: Queue,
    current: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    always_copy: bool,
    copy_on_mouse_down: bool,
    inner: Option<Box<dyn GuiElem>>,
}
impl QueueLoop {
    pub fn new(
        config: GuiElemCfg,
        saved: Option<Rc<String>>,
        path: Vec<usize>,
        queue: Queue,
        current: bool,
    ) -> Self {
        Self {
            config: if path.is_empty() {
                config
            } else {
                config.w_mouse().w_keyboard_watch()
            }
            .w_drag_target(),
            saved,
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
            inner: None,
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
                    "repeat forever".to_owned()
                } else if *total == 1 {
                    "repeat 1 time".to_owned()
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
        if let Some(inner) = &mut self.inner {
            Box::new(
                [inner.elem_mut()]
                    .into_iter()
                    .chain(self.children.iter_mut().map(|v| v.elem_mut())),
            )
        } else {
            Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
        }
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
        let pos = Rectangle::new(
            *info.pos.top_left(),
            Vec2::new(
                (info.pos.top_left().x + info.pos.bottom_right().x) / 2.0,
                info.pos.bottom_right().y,
            ),
        );
        let ppos = std::mem::replace(&mut info.pos, pos);
        generic_queue_draw(
            info,
            &self.saved,
            &self.path,
            || self.queue.clone(),
            &mut self.mouse,
            self.copy_on_mouse_down,
        );
        info.pos = ppos;
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
                vec![GuiAction::SendToServer(wrap(
                    &self.saved,
                    Action::QueueGoto(self.path.clone()),
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
        down: bool,
        key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = self.always_copy || key_watch_ctrl(&modifiers, down, key);
        vec![]
    }
    fn dragged(&mut self, e: &mut EventInfo, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            e.take();
            let mut p = self.path.clone();
            p.push(0);
            let s1 = self.saved.clone();
            let s2 = self.saved.clone();
            dragged_add_to_queue(
                dragged,
                &self.saved,
                p,
                move |p, q| wrap(&s1, Action::QueueAdd(p, q, Req::none())),
                move |p, q| wrap(&s2, Action::QueueMoveInto(q, p)),
            )
        } else {
            vec![]
        }
    }
}

fn dragged_add_to_queue<T: 'static>(
    dragged: Dragging,
    saved: &Option<Rc<String>>,
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
        Dragging::Queue(q, src) => match src {
            None => vec![GuiAction::SendToServer(f_queues(data, vec![q]))],
            Some((queue, path)) => {
                if queue.as_deref() == saved.as_ref().map(|v| v.as_str()) {
                    // within one queue (active or playlist)
                    vec![GuiAction::SendToServer(f_queue_by_path(data, path))]
                } else {
                    // between different queues => always copy, never move
                    vec![
                        // GuiAction::SendToServer(if let Some(queue) = queue {
                        //     Action::SavedQueue(queue, vec![Action::QueueRemove(path)])
                        // } else {
                        //     Action::QueueRemove(path)
                        // }),
                        GuiAction::SendToServer(f_queues(data, vec![q])),
                    ]
                }
            }
        },
        Dragging::Queues(q) => vec![GuiAction::SendToServer(f_queues(data, q))],
    }
}

fn add_to_queue_album_by_id(id: AlbumId, db: &Database) -> Option<Queue> {
    db.albums().get(&id).map(|album| {
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
        .into()
    })
}
fn add_to_queue_artist_by_id(id: ArtistId, db: &Database) -> Option<Queue> {
    db.artists().get(&id).map(|artist| {
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
        .into()
    })
}
