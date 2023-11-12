use std::collections::VecDeque;

use musicdb_lib::{
    data::{
        database::Database,
        queue::{Queue, QueueContent, QueueDuration, ShuffleState},
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
    gui::{Dragging, DrawInfo, GuiAction, GuiElem, GuiElemCfg},
    gui_base::{Panel, ScrollBox},
    gui_text::{self, AdvancedLabel, Label},
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
    c_control_flow_elements: Panel<(QueueLoop, QueueLoop, QueueRandom, QueueShuffle)>,
    c_duration: AdvancedLabel,
    queue_updated: bool,
}
const QP_QUEUE1: f32 = 0.0;
const QP_QUEUE2: f32 = 0.95;
const QP_INV1: f32 = QP_QUEUE2;
const QP_INV2: f32 = 1.0;
impl QueueViewer {
    pub fn new(config: GuiElemCfg) -> Self {
        let control_flow_elements = (
            QueueLoop::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 0.5))).w_mouse(),
                vec![],
                QueueContent::Loop(
                    0,
                    0,
                    Box::new(QueueContent::Folder(0, vec![], "in loop".to_string()).into()),
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
                    Box::new(QueueContent::Folder(0, vec![], "in loop".to_string()).into()),
                )
                .into(),
                false,
            )
            .alwayscopy(),
            QueueRandom::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (1.0, 0.5))).w_mouse(),
                vec![],
                QueueContent::Random(VecDeque::new()).into(),
                false,
            )
            .alwayscopy(),
            QueueShuffle::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.5), (1.0, 1.0))).w_mouse(),
                vec![],
                QueueContent::Shuffle {
                    inner: Box::new(QueueContent::Folder(0, vec![], String::new()).into()),
                    state: ShuffleState::NotShuffled,
                }
                .into(),
                false,
            )
            .alwayscopy(),
        );
        Self {
            config,
            c_scroll_box: ScrollBox::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2))),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                vec![],
                vec![],
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
                    gui_text::Content::new(format!("Total: {dt}"), Color::GRAY),
                    1.0,
                    1.0,
                )],
                vec![(
                    gui_text::Content::new(format!("Remaining: {dr}"), Color::GRAY),
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
        QueueContent::Folder(ia, q, _) => {
            if !skip_folder {
                target.push(Box::new(QueueFolder::new(
                    cfg.clone(),
                    path.clone(),
                    queue.clone(),
                    current,
                )));
                target_h.push(line_height * 0.8);
            }
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
        QueueContent::Random(q) => {
            target.push(Box::new(QueueRandom::new(
                cfg.clone(),
                path.clone(),
                queue.clone(),
                current,
            )));
            target_h.push(line_height);
            for (i, inner) in q.iter().enumerate() {
                let mut p = path.clone();
                p.push(i);
                queue_gui(
                    inner,
                    db,
                    depth + depth_inc_by,
                    depth_inc_by,
                    line_height,
                    target,
                    target_h,
                    p,
                    current && i == q.len().saturating_sub(2),
                    false,
                );
            }
            let mut p1 = path.clone();
            let p2 = p1.pop().unwrap_or(0) + 1;
            target.push(Box::new(QueueIndentEnd::new(cfg, (p1, p2))));
            target_h.push(line_height * 0.4);
        }
        QueueContent::Shuffle { inner, state: _ } => {
            target.push(Box::new(QueueShuffle::new(
                cfg.clone(),
                path.clone(),
                queue.clone(),
                current,
            )));
            target_h.push(line_height * 0.8);
            let mut p = path.clone();
            p.push(0);
            queue_gui(
                inner,
                db,
                depth + depth_inc_by,
                depth_inc_by,
                line_height,
                target,
                target_h,
                p,
                current,
                true,
            );
            let mut p1 = path.clone();
            let p2 = p1.pop().unwrap_or(0) + 1;
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
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        dragged_add_to_queue(dragged, |q, _| Command::QueueAdd(vec![], q))
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
                            gui_text::Content::new(
                                song.title.clone(),
                                if current {
                                    Color::from_int_rgb(194, 76, 178)
                                } else {
                                    Color::from_int_rgb(120, 76, 194)
                                },
                            ),
                            1.0,
                            1.0,
                        ),
                        (
                            gui_text::Content::new(
                                {
                                    let duration = song.duration_millis / 1000;
                                    format!("  {}:{:0>2}", duration / 60, duration % 60)
                                },
                                if current {
                                    Color::GRAY
                                } else {
                                    Color::DARK_GRAY
                                },
                            ),
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
            if !self.always_copy {
                vec![GuiAction::SendToServer(Command::QueueGoto(
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
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(QueueContent::Song(self.song.id).into()),
                None,
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
        self.copy = self.always_copy || modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            let mut p = self.path.clone();
            let insert_below = self.insert_below;
            dragged_add_to_queue(dragged, move |q, i| {
                if let Some(j) = p.pop() {
                    Command::QueueInsert(p.clone(), if insert_below { j + 1 } else { j } + i, q)
                } else {
                    Command::QueueAdd(p.clone(), q)
                }
            })
        } else {
            vec![]
        }
    }
}

struct QueueFolder {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElem>>,
    path: Vec<usize>,
    queue: Queue,
    current: bool,
    insert_into: bool,
    mouse: bool,
    mouse_pos: Vec2,
    copy: bool,
    always_copy: bool,
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
            children: vec![Box::new(Label::new(
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
            insert_into: false,
            mouse: false,
            mouse_pos: Vec2::ZERO,
            copy: false,
            always_copy: false,
            copy_on_mouse_down: false,
        }
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
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(self.queue.clone()),
                None,
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
            if !self.always_copy {
                vec![GuiAction::SendToServer(Command::QueueGoto(
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
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            if self.insert_into {
                let p = self.path.clone();
                dragged_add_to_queue(dragged, move |q, _| Command::QueueAdd(p.clone(), q))
            } else {
                let mut p = self.path.clone();
                let j = p.pop().unwrap_or(0);
                dragged_add_to_queue(dragged, move |q, i| {
                    Command::QueueInsert(p.clone(), j + i, q)
                })
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
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        let (p, j) = self.path_insert.clone();
        dragged_add_to_queue(dragged, move |q, i| {
            Command::QueueInsert(p.clone(), j + i, q)
        })
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
    fn mouse_wheel(&mut self, diff: f32) -> Vec<GuiAction> {
        if self.always_copy {
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
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(self.queue.clone()),
                None,
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
            if !self.always_copy {
                vec![GuiAction::SendToServer(Command::QueueGoto(
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
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            let mut p = self.path.clone();
            p.push(0);
            dragged_add_to_queue(dragged, move |q, _| Command::QueueAdd(p.clone(), q))
        } else {
            vec![]
        }
    }
}

struct QueueRandom {
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
impl QueueRandom {
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
                match queue.content() {
                    QueueContent::Random(_) => {
                        format!("random")
                    }
                    _ => "[???]".to_string(),
                },
                Color::from_int_rgb(32, 27, 179),
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
        self
    }
}
impl GuiElem for QueueRandom {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(self.queue.clone()),
                None,
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
            if !self.always_copy {
                vec![GuiAction::SendToServer(Command::QueueGoto(
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
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            let p = self.path.clone();
            dragged_add_to_queue(dragged, move |q, _| Command::QueueAdd(p.clone(), q))
        } else {
            vec![]
        }
    }
}

struct QueueShuffle {
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
impl QueueShuffle {
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
                match queue.content() {
                    QueueContent::Shuffle { .. } => {
                        format!("shuffle")
                    }
                    _ => "[???]".to_string(),
                },
                Color::from_int_rgb(92, 52, 194),
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
        self
    }
}
impl GuiElem for QueueShuffle {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if !self.mouse {
            self.mouse_pos = Vec2::new(
                info.mouse_pos.x - self.config.pixel_pos.top_left().x,
                info.mouse_pos.y - self.config.pixel_pos.top_left().y,
            );
        }
        if generic_queue_draw(info, &self.path, &mut self.mouse, self.copy_on_mouse_down) {
            info.actions.push(GuiAction::SetDragging(Some((
                Dragging::Queue(self.queue.clone()),
                None,
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
            if !self.always_copy {
                vec![GuiAction::SendToServer(Command::QueueGoto(
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
        modifiers: ModifiersState,
        _down: bool,
        _key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.copy = modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            let mut p = self.path.clone();
            p.push(0);
            dragged_add_to_queue(dragged, move |q, _| Command::QueueAdd(p.clone(), q))
        } else {
            vec![]
        }
    }
}

fn dragged_add_to_queue<F: FnMut(Queue, usize) -> Command + 'static>(
    dragged: Dragging,
    mut f: F,
) -> Vec<GuiAction> {
    match dragged {
        Dragging::Artist(id) => {
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(q) = add_to_queue_artist_by_id(id, db) {
                    vec![GuiAction::SendToServer(f(q, 0))]
                } else {
                    vec![]
                }
            }))]
        }
        Dragging::Album(id) => {
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(q) = add_to_queue_album_by_id(id, db) {
                    vec![GuiAction::SendToServer(f(q, 0))]
                } else {
                    vec![]
                }
            }))]
        }
        Dragging::Song(id) => {
            let q = QueueContent::Song(id).into();
            vec![GuiAction::SendToServer(f(q, 0))]
        }
        Dragging::Queue(q) => {
            vec![GuiAction::SendToServer(f(q, 0))]
        }
        Dragging::Queues(q) => q
            .into_iter()
            .enumerate()
            .map(|(i, q)| GuiAction::SendToServer(f(q, i)))
            .collect(),
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
                artist.name.clone(),
            )
            .into(),
        )
    } else {
        None
    }
}
