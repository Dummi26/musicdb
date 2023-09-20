use std::collections::VecDeque;

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
    gui_base::{Panel, ScrollBox},
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
const QP_QUEUE1: f32 = 0.0;
const QP_QUEUE2: f32 = 0.95;
const QP_INV1: f32 = QP_QUEUE2;
const QP_INV2: f32 = 1.0;
impl QueueViewer {
    pub fn new(config: GuiElemCfg) -> Self {
        Self {
            config,
            children: vec![
                GuiElem::new(ScrollBox::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2))),
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
                )),
                GuiElem::new(QueueEmptySpaceDragHandler::new(GuiElemCfg::at(
                    Rectangle::from_tuples((0.0, QP_QUEUE1), (1.0, QP_QUEUE2)),
                ))),
                GuiElem::new(Panel::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, QP_INV1), (1.0, QP_INV2))),
                    vec![
                        GuiElem::new(
                            QueueLoop::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 0.5)))
                                    .w_mouse(),
                                vec![],
                                QueueContent::Loop(
                                    0,
                                    0,
                                    Box::new(
                                        QueueContent::Folder(0, vec![], "in loop".to_string())
                                            .into(),
                                    ),
                                )
                                .into(),
                                false,
                            )
                            .alwayscopy(),
                        ),
                        GuiElem::new(
                            QueueLoop::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.5), (0.5, 1.0)))
                                    .w_mouse(),
                                vec![],
                                QueueContent::Loop(
                                    2,
                                    0,
                                    Box::new(
                                        QueueContent::Folder(0, vec![], "in loop".to_string())
                                            .into(),
                                    ),
                                )
                                .into(),
                                false,
                            )
                            .alwayscopy(),
                        ),
                        GuiElem::new(
                            QueueRandom::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (1.0, 0.5)))
                                    .w_mouse(),
                                vec![],
                                QueueContent::Random(VecDeque::new()).into(),
                                false,
                            )
                            .alwayscopy(),
                        ),
                        GuiElem::new(
                            QueueShuffle::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.5), (1.0, 1.0)))
                                    .w_mouse(),
                                vec![],
                                QueueContent::Shuffle(0, vec![], vec![], 0).into(),
                                false,
                            )
                            .alwayscopy(),
                        ),
                    ],
                )),
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
    skip_folder: bool,
) {
    let cfg = GuiElemCfg::at(Rectangle::from_tuples((depth, 0.0), (1.0, 1.0)));
    match queue.content() {
        QueueContent::Song(id) => {
            if let Some(s) = db.songs().get(id) {
                target.push((
                    GuiElem::new(QueueSong::new(
                        cfg,
                        path,
                        s.clone(),
                        current,
                        db,
                        depth_inc_by * 0.33,
                    )),
                    line_height * 1.75,
                ));
            }
        }
        QueueContent::Folder(ia, q, _) => {
            if !skip_folder {
                target.push((
                    GuiElem::new(QueueFolder::new(
                        cfg.clone(),
                        path.clone(),
                        queue.clone(),
                        current,
                    )),
                    line_height * 0.8,
                ));
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
                    p,
                    current && *ia == i,
                    false,
                );
            }
            if !skip_folder {
                let mut p1 = path;
                let p2 = p1.pop().unwrap_or(0) + 1;
                target.push((
                    GuiElem::new(QueueIndentEnd::new(cfg, (p1, p2))),
                    line_height * 0.4,
                ));
            }
        }
        QueueContent::Loop(_, _, inner) => {
            let mut p = path.clone();
            let mut p1 = path.clone();
            let p2 = p1.pop().unwrap_or(0) + 1;
            p.push(0);
            target.push((
                GuiElem::new(QueueLoop::new(cfg.clone(), path, queue.clone(), current)),
                line_height * 0.8,
            ));
            queue_gui(
                &inner,
                db,
                depth,
                depth_inc_by,
                line_height,
                target,
                p,
                current,
                true,
            );
            target.push((
                GuiElem::new(QueueIndentEnd::new(cfg, (p1, p2))),
                line_height * 0.4,
            ));
        }
        QueueContent::Random(q) => {
            target.push((
                GuiElem::new(QueueRandom::new(cfg, path.clone(), queue.clone(), current)),
                line_height,
            ));
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
                    p,
                    current && i == q.len().saturating_sub(2),
                    false,
                );
            }
        }
        QueueContent::Shuffle(c, map, elems, _) => {
            target.push((
                GuiElem::new(QueueShuffle::new(cfg, path.clone(), queue.clone(), current)),
                line_height * 0.8,
            ));
            for (i, inner) in map.iter().enumerate() {
                if let Some(inner) = elems.get(*inner) {
                    let mut p = path.clone();
                    p.push(i);
                    queue_gui(
                        inner,
                        db,
                        depth + depth_inc_by,
                        depth_inc_by,
                        line_height,
                        target,
                        p,
                        current && i == *c,
                        false,
                    );
                }
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
                GuiElem::new(Label::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.57))),
                    song.title.clone(),
                    if current {
                        Color::from_int_rgb(194, 76, 178)
                    } else {
                        Color::from_int_rgb(120, 76, 194)
                    },
                    None,
                    Vec2::new(0.0, 0.5),
                )),
                GuiElem::new(Label::new(
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
                        Color::from_int_rgb(146, 57, 133)
                    } else {
                        Color::from_int_rgb(95, 57, 146)
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
        self
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
        self.copy = self.always_copy || modifiers.ctrl();
        vec![]
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        if !self.always_copy {
            let mut p = self.path.clone();
            let insert_below = self.insert_below;
            dragged_add_to_queue(dragged, move |q| {
                if let Some(i) = p.pop() {
                    Command::QueueInsert(p, if insert_below { i + 1 } else { i }, q)
                } else {
                    Command::QueueAdd(p, q)
                }
            })
        } else {
            vec![]
        }
    }
}

#[derive(Clone)]
struct QueueFolder {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
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
        self
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
        if !self.always_copy {
            if self.insert_into {
                let p = self.path.clone();
                dragged_add_to_queue(dragged, move |q| Command::QueueAdd(p, q))
            } else {
                let mut p = self.path.clone();
                let i = p.pop();
                dragged_add_to_queue(dragged, move |q| Command::QueueInsert(p, i.unwrap_or(0), q))
            }
        } else {
            vec![]
        }
    }
}
#[derive(Clone)]
pub struct QueueIndentEnd {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
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
impl GuiElemTrait for QueueIndentEnd {
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
        let (p, i) = self.path_insert.clone();
        dragged_add_to_queue(dragged, move |q| Command::QueueInsert(p, i, q))
    }
}

#[derive(Clone)]
struct QueueLoop {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
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
            children: vec![GuiElem::new(Label::new(
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
impl GuiElemTrait for QueueLoop {
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
                .inner
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
        if !self.always_copy {
            let mut p = self.path.clone();
            p.push(0);
            dragged_add_to_queue(dragged, move |q| Command::QueueAdd(p, q))
        } else {
            vec![]
        }
    }
}

#[derive(Clone)]
struct QueueRandom {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
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
            children: vec![GuiElem::new(Label::new(
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
impl GuiElemTrait for QueueRandom {
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
        if !self.always_copy {
            let p = self.path.clone();
            dragged_add_to_queue(dragged, move |q| Command::QueueAdd(p, q))
        } else {
            vec![]
        }
    }
}

#[derive(Clone)]
struct QueueShuffle {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
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
            children: vec![GuiElem::new(Label::new(
                GuiElemCfg::default(),
                match queue.content() {
                    QueueContent::Shuffle(..) => {
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
impl GuiElemTrait for QueueShuffle {
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
        if !self.always_copy {
            let p = self.path.clone();
            dragged_add_to_queue(dragged, move |q| Command::QueueAdd(p, q))
        } else {
            vec![]
        }
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
