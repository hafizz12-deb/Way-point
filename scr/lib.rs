//! Waypoints for Horizon, as a sandboxed `.hmod` (WebAssembly) mod.
//!
//! N makes a waypoint where you stand, M opens the list (hide, recolour,
//! delete), `/wp` does the rest by command (names can't be typed into a
//! widget, so `/wp add NAME` and `/wp rename N NAME` name them). Each visible
//! waypoint of your dimension shows its name and distance in the world, and a
//! death can leave a red one where you died. A port of Nirokx's native mod
//! "Waypoints" (MIT) to the mod API 0.1: see README.md for what differs.
mod logic;

use horizon_mod_sdk::{
    Mod, Registration, chat, game,
    input::{self, Event},
    render::{self, FillShape, Point, Rect, Shape, TextShape},
    storage,
    ui::{self, ButtonWidget, ListWidget, Screen, TextWidget, Widget},
};
use logic::{
    Camera, Cmd, DeathTracker, Draw, EYE_HEIGHT, FOVS, Filter, HELP, Here, List, PALETTE, Settings,
    Waypoint, clean_name, color_name, dimension_label, distance_text, row_text,
};
use std::collections::HashMap;

const WAYPOINTS: &str = "waypoints.v1";
const SETTINGS: &str = "settings.v1";

// The widgets' ids: the game echoes them back in `ui_select`.
const ID_TEXT: u32 = 1;
const ID_FILTER: u32 = 10;
const ID_NEW: u32 = 11;
const ID_LIST: u32 = 12;
const ID_TOGGLE: u32 = 20;
const ID_COLOR: u32 = 21;
const ID_DELETE: u32 = 22;
const ID_BACK: u32 = 23;
const ID_SET_LABELS: u32 = 100;
const ID_SET_DEATHS: u32 = 101;
const ID_SET_FOV: u32 = 102;
const ID_SET_SMOOTH: u32 = 103;

/// The most rows a `list` widget takes.
const MAX_ROWS: usize = 512;
/// Text widths are kept by text; a long session may see many distances.
const MAX_WIDTHS: usize = 512;
const MUTED: u32 = 0xAAAAAA;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Closed,
    List,
    /// A waypoint's page, by its place in the list.
    Detail(usize),
}

struct Waypoints {
    list: List,
    settings: Settings,
    filter: Filter,
    page: Page,
    /// The list screen's rows: places in `list`.
    rows: Vec<usize>,
    /// The waypoint whose Delete was clicked once.
    confirm: Option<usize>,
    widths: HashMap<String, f32>,
    /// What is on screen now: the HUD is only set again when it changes.
    drawn: Vec<Draw>,
    deaths: DeathTracker,
}

fn load(key: &str) -> Option<String> {
    storage::get(key).and_then(|v| String::from_utf8(v).ok())
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

fn block_of(p: &game::Player) -> [i32; 3] {
    [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]
}

fn shapes(draws: &[Draw]) -> Vec<Shape> {
    draws
        .iter()
        .map(|d| match d {
            Draw::Fill([x, y, w, h], color) => Shape::Fill(FillShape {
                rect: Rect { x: *x, y: *y, w: *w, h: *h },
                color: *color,
            }),
            Draw::Text(text, [x, y], color, shadow) => Shape::Text(TextShape {
                text: text.clone(),
                at: Point { x: *x, y: *y },
                color: *color,
                centered: false,
                shadow: *shadow,
            }),
        })
        .collect()
}

impl Waypoints {
    fn save(&self) {
        let _ = storage::set(WAYPOINTS, logic::encode(&self.list.waypoints).as_bytes());
    }

    fn save_settings(&self) {
        let _ = storage::set(SETTINGS, self.settings.encode().as_bytes());
    }

    // ---- In the world ------------------------------------------------------

    fn clear_labels(&mut self) {
        if !self.drawn.is_empty() {
            self.drawn.clear();
            render::set_hud(&[]);
        }
    }

    /// Projects every visible waypoint onto the screen and sets the HUD when
    /// what is drawn changed. The player's position, yaw and pitch are as of
    /// the call (a tick, or a frame in smooth mode); the field of view is the
    /// setting's, because a mod can't read the game's.
    fn refresh_labels(&mut self) {
        let Some(p) = game::get_player() else {
            return self.clear_labels();
        };
        if !self.settings.labels || self.list.waypoints.is_empty() {
            return self.clear_labels();
        }
        let (w, h) = render::gui_size();
        let camera = Camera {
            pos: [p.x, p.y + EYE_HEIGHT, p.z],
            yaw: p.yaw,
            pitch: p.pitch,
            fov: self.settings.fov_degrees(),
        };
        if self.widths.len() > MAX_WIDTHS {
            self.widths.clear();
        }
        let widths = &mut self.widths;
        let mut measure = |t: &str| *widths.entry(t.to_owned()).or_insert_with(|| render::text_width(t));
        let draws = logic::labels(
            &self.list.waypoints,
            camera,
            [p.x, p.y, p.z],
            &p.dimension,
            [w, h],
            &mut measure,
        );
        if draws != self.drawn {
            render::set_hud(&shapes(&draws));
            self.drawn = draws;
        }
    }

    // ---- Making and changing waypoints -----------------------------------------

    fn make(&mut self, name: Option<String>, pos: [i32; 3], dimension: String) -> String {
        let name = name
            .map(|n| clean_name(&n))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| self.list.default_name());
        let color = PALETTE[self.list.waypoints.len() % 11];
        self.list.add(Waypoint {
            name: name.clone(),
            pos,
            dimension,
            color,
            hidden: false,
            death: false,
        });
        self.save();
        self.refresh_labels();
        format!("Waypoint \"{name}\" at {} {} {}", pos[0], pos[1], pos[2])
    }

    fn make_here(&mut self, name: Option<String>) -> Result<String, String> {
        let p = game::get_player().ok_or("Join a world first")?;
        Ok(self.make(name, block_of(&p), p.dimension.clone()))
    }

    // ---- Screens -------------------------------------------------------------------

    fn open_list(&mut self) {
        self.page = Page::List;
        self.confirm = None;
        self.rows = self.list.indexes(self.filter);
        self.rows.truncate(MAX_ROWS);
        let player = game::get_player();
        let here = player.as_ref().map(|p| Here { pos: [p.x, p.y, p.z], dimension: &p.dimension });
        let rows: Vec<String> = self
            .rows
            .iter()
            .map(|&i| row_text(i, &self.list.waypoints[i], here.as_ref()))
            .collect();
        let mut widgets = vec![
            Widget::Button(ButtonWidget { id: ID_FILTER, label: format!("Filter: {}", self.filter.label()) }),
            Widget::Button(ButtonWidget { id: ID_NEW, label: "New waypoint here".into() }),
        ];
        if rows.is_empty() {
            widgets.push(Widget::Text(TextWidget {
                id: ID_TEXT,
                text: "No waypoints here yet.".into(),
                color: MUTED,
            }));
        } else {
            widgets.push(Widget::Text(TextWidget {
                id: ID_TEXT,
                text: "Pick one to hide, recolour or delete it.".into(),
                color: MUTED,
            }));
            widgets.push(Widget::List(ListWidget { id: ID_LIST, rows, selected: None }));
        }
        ui::open_screen(&Screen {
            title: format!("Waypoints ({})", self.list.waypoints.len()),
            widgets,
        });
    }

    fn open_detail(&mut self, i: usize) {
        let Some(w) = self.list.waypoints.get(i) else {
            return self.open_list();
        };
        self.page = Page::Detail(i);
        let player = game::get_player();
        let far = player
            .as_ref()
            .filter(|p| p.dimension == w.dimension)
            .map(|p| {
                let d = [
                    p.x - (f64::from(w.pos[0]) + 0.5),
                    p.y - (f64::from(w.pos[1]) + 0.5),
                    p.z - (f64::from(w.pos[2]) + 0.5),
                ];
                format!("  {}", distance_text((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()))
            })
            .unwrap_or_default();
        let text = |id, text: String, color| Widget::Text(TextWidget { id, text, color });
        let button = |id, label: String| Widget::Button(ButtonWidget { id, label });
        let widgets = vec![
            text(ID_TEXT, w.name.clone(), w.color),
            text(
                ID_TEXT + 1,
                format!("{} {} {}  {}{far}", w.pos[0], w.pos[1], w.pos[2], dimension_label(&w.dimension)),
                0,
            ),
            button(ID_TOGGLE, if w.hidden { "Show in the world".into() } else { "Hide in the world".into() }),
            button(ID_COLOR, format!("Colour: {}", color_name(w.color))),
            button(
                ID_DELETE,
                if self.confirm == Some(i) { "Click again to delete".into() } else { "Delete".into() },
            ),
            button(ID_BACK, "Back to the list".into()),
            text(ID_TEXT + 2, format!("To rename it: /wp rename {} NAME", i + 1), MUTED),
        ];
        ui::open_screen(&Screen { title: "Waypoint".into(), widgets });
    }

    fn page_settings(&self) {
        let s = &self.settings;
        let button = |id, label: String| Widget::Button(ButtonWidget { id, label });
        ui::set_settings(&[
            button(ID_SET_LABELS, format!("Names in the world: {}", on_off(s.labels))),
            button(ID_SET_DEATHS, format!("Death waypoints: {}", on_off(s.deaths))),
            button(ID_SET_FOV, format!("Field of view: {}", s.fov_degrees())),
            button(
                ID_SET_SMOOTH,
                format!("Names follow: {}", if s.smooth { "every frame" } else { "every tick" }),
            ),
        ]);
    }

    fn change_setting(&mut self, id: u32) {
        let s = &mut self.settings;
        match id {
            ID_SET_LABELS => s.labels = !s.labels,
            ID_SET_DEATHS => s.deaths = !s.deaths,
            ID_SET_FOV => s.fov = (s.fov + 1) % FOVS.len(),
            ID_SET_SMOOTH => s.smooth = !s.smooth,
            _ => return,
        }
        self.save_settings();
        self.page_settings();
        self.refresh_labels();
    }
}

impl Mod for Waypoints {
    fn init() -> Result<(Self, Registration), String> {
        let list = List { waypoints: load(WAYPOINTS).map(|t| logic::decode(&t)).unwrap_or_default() };
        let settings = load(SETTINGS).map(|t| Settings::decode(&t)).unwrap_or_default();
        // A key the player can rebind on the mod's page in Options > Mods.
        for (name, label, key) in [
            ("create", "New waypoint here", "key.keyboard.n"),
            ("list", "Waypoint list", "key.keyboard.m"),
        ] {
            let _ = input::register_key(name, label, key);
        }
        let me = Waypoints {
            list,
            settings,
            filter: Filter::All,
            page: Page::Closed,
            rows: Vec::new(),
            confirm: None,
            widths: HashMap::new(),
            drawn: Vec::new(),
            deaths: DeathTracker::default(),
        };
        me.page_settings();
        Ok((me, Registration::new().command("wp").on_tick().on_frame()))
    }

    fn tick(&mut self, tick: u64) {
        for event in input::poll() {
            if let Event::KeyDown(key) = event {
                match key.as_str() {
                    "create" => match self.make_here(None) {
                        Ok(text) | Err(text) => chat::show(&text),
                    },
                    "list" => self.open_list(),
                    _ => {}
                }
            }
        }
        let now = game::get_player().map(|p| logic::Snapshot {
            dimension: p.dimension.clone(),
            pos: [p.x, p.y, p.z],
            health: p.health,
            food: p.food,
        });
        if let Some((dimension, pos)) = self.deaths.step(tick, now) {
            if self.settings.deaths {
                self.list.add_death(dimension, pos);
                self.save();
            }
        }
        if !self.settings.smooth {
            self.refresh_labels();
        }
    }

    fn frame(&mut self, _seconds: f32) {
        if self.settings.smooth {
            self.refresh_labels();
        }
    }

    fn command(&mut self, _name: &str, args: &str) -> Result<Option<String>, String> {
        let said = match logic::parse(args)? {
            Cmd::Help => return Ok(Some(HELP.into())),
            Cmd::List => {
                self.open_list();
                return Ok(None);
            }
            Cmd::Add(name) => self.make_here(name)?,
            Cmd::At(pos, name) => {
                let dimension = game::get_player()
                    .map_or_else(|| logic::OVERWORLD.to_owned(), |p| p.dimension.clone());
                self.make(name, pos, dimension)
            }
            Cmd::Rename(i, name) => {
                self.list.rename(i, &name)?;
                self.save();
                format!("#{} is now \"{}\"", i + 1, clean_name(&name))
            }
            Cmd::Del(i) => {
                if !self.list.remove(i) {
                    return Err("No such waypoint".into());
                }
                self.save();
                format!("Deleted #{}", i + 1)
            }
            Cmd::Hide(i) | Cmd::Show(i) if i >= self.list.waypoints.len() => {
                return Err("No such waypoint".into());
            }
            Cmd::Hide(i) => {
                self.list.toggle_hidden(i, Some(true));
                self.save();
                format!("#{} is hidden", i + 1)
            }
            Cmd::Show(i) => {
                self.list.toggle_hidden(i, Some(false));
                self.save();
                format!("#{} is shown", i + 1)
            }
            Cmd::Color(i, palette) => {
                let rgb = self.list.recolor(i, palette)?;
                self.save();
                format!("#{} is {}", i + 1, color_name(rgb))
            }
        };
        self.refresh_labels();
        Ok(Some(said))
    }

    fn ui_select(&mut self, id: u32, row: u32) {
        match (id, self.page) {
            (ID_FILTER, _) => {
                self.filter = self.filter.next();
                self.open_list();
            }
            (ID_NEW, _) => {
                if let Err(text) = self.make_here(None) {
                    chat::show(&text);
                }
                self.open_list();
            }
            (ID_LIST, _) => {
                if let Some(&i) = self.rows.get(row as usize) {
                    self.confirm = None;
                    self.open_detail(i);
                }
            }
            (ID_TOGGLE, Page::Detail(i)) => {
                self.list.toggle_hidden(i, None);
                self.save();
                self.refresh_labels();
                self.open_detail(i);
            }
            (ID_COLOR, Page::Detail(i)) => {
                let _ = self.list.recolor(i, None);
                self.save();
                self.refresh_labels();
                self.open_detail(i);
            }
            (ID_DELETE, Page::Detail(i)) => {
                if self.confirm == Some(i) {
                    self.list.remove(i);
                    self.confirm = None;
                    self.save();
                    self.refresh_labels();
                    self.open_list();
                } else {
                    self.confirm = Some(i);
                    self.open_detail(i);
                }
            }
            (ID_BACK, _) => self.open_list(),
            (ID_SET_LABELS..=ID_SET_SMOOTH, _) => self.change_setting(id),
            _ => {}
        }
    }

    fn ui_closed(&mut self) {
        self.page = Page::Closed;
        self.confirm = None;
    }

    fn ui_settings_opened(&mut self) {
        self.page_settings();
    }
}

horizon_mod_sdk::export_mod!(Waypoints);
