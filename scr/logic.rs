//! Waypoints, without the game: the data, its text format, the projection of
//! a waypoint onto the screen, the death heuristic and the `/wp` commands.
//! Nothing here touches the mod API, so `cargo test` runs it on the host.
//!
//! The projection, the palette and the storage format follow Nirokx's native
//! Horizon mod "Waypoints" (MIT, https://github.com/Nirokx/horizon-waypoints).

pub const OVERWORLD: &str = "minecraft:overworld";
pub const NETHER: &str = "minecraft:the_nether";
pub const END: &str = "minecraft:the_end";

/// The colours a waypoint can have: Minecraft's text colours and a brown.
pub const PALETTE: [u32; 16] = [
    0xFF5555, 0xFFAA00, 0xFFFF55, 0x55FF55, 0x00AA00, 0x55FFFF, 0x00AAAA, 0x5555FF, 0x0000AA,
    0xAA00AA, 0xFF55FF, 0xFFFFFF, 0xAAAAAA, 0x555555, 0xAA5500, 0x000000,
];
pub const COLOR_NAMES: [&str; 16] = [
    "Red", "Gold", "Yellow", "Green", "Dark green", "Aqua", "Dark aqua", "Blue", "Dark blue",
    "Purple", "Pink", "White", "Gray", "Dark gray", "Brown", "Black",
];
/// The vertical fields of view the settings page cycles through.
pub const FOVS: [f32; 6] = [60.0, 70.0, 80.0, 90.0, 100.0, 110.0];
pub const DEFAULT_FOV: usize = 1;

pub const DEATH_COLOR: u32 = 0xFF5555;
pub const DEATH_MAX: usize = 5;
pub const NAME_MAX: usize = 32;
/// Labels drawn at once: the nearest ones.
pub const MAX_LABELS: usize = 32;
/// A standing player's eyes above their feet.
pub const EYE_HEIGHT: f64 = 1.62;

const INK: u32 = 0xFFFFFF;
const LABEL_BG: u32 = 0x9000_0000;

// ---- Waypoints and their file ------------------------------------------------

/// One waypoint. `pos` is a block position; `color` is `0xRRGGBB`.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub pos: [i32; 3],
    pub dimension: String,
    pub color: u32,
    pub hidden: bool,
    /// Made by a death: the oldest of these go, unless renamed.
    pub death: bool,
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// One line per waypoint: name, x, y, z, dimension, colour, hidden, death.
pub fn encode(list: &[Waypoint]) -> String {
    let mut out = String::from("# waypoints 1\n");
    for w in list {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:06X}\t{}\t{}\n",
            escape(&w.name),
            w.pos[0],
            w.pos[1],
            w.pos[2],
            w.dimension,
            w.color & 0xFF_FFFF,
            u8::from(w.hidden),
            u8::from(w.death)
        ));
    }
    out
}

/// What `encode` wrote; broken lines are skipped.
pub fn decode(text: &str) -> Vec<Waypoint> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                return None;
            }
            Some(Waypoint {
                name: unescape(f[0]),
                pos: [f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?],
                dimension: f[4].to_owned(),
                color: u32::from_str_radix(f[5], 16).ok()? & 0xFF_FFFF,
                hidden: f[6].trim() == "1",
                death: f.get(7).is_some_and(|v| v.trim() == "1"),
            })
        })
        .collect()
}

// ---- Settings ----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Names and distances in the world.
    pub labels: bool,
    /// A waypoint where the player dies.
    pub deaths: bool,
    /// Index into [`FOVS`]: the game's field of view, which a mod can't read.
    pub fov: usize,
    /// Labels follow every frame (smoother) instead of every tick.
    pub smooth: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { labels: true, deaths: true, fov: DEFAULT_FOV, smooth: false }
    }
}

impl Settings {
    pub fn fov_degrees(&self) -> f32 {
        FOVS[self.fov.min(FOVS.len() - 1)]
    }
    pub fn encode(&self) -> String {
        format!(
            "labels={}\ndeaths={}\nfov={}\nsmooth={}\n",
            u8::from(self.labels),
            u8::from(self.deaths),
            self.fov,
            u8::from(self.smooth)
        )
    }
    pub fn decode(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "labels" => s.labels = v != "0",
                "deaths" => s.deaths = v != "0",
                "smooth" => s.smooth = v == "1",
                "fov" => {
                    if let Ok(i) = v.parse::<usize>() {
                        s.fov = i.min(FOVS.len() - 1);
                    }
                }
                _ => {}
            }
        }
        s
    }
}

// ---- Words ---------------------------------------------------------------------

pub fn dimension_label(id: &str) -> String {
    match id {
        OVERWORLD => "Overworld".into(),
        NETHER => "Nether".into(),
        END => "The End".into(),
        other => other.rsplit(':').next().unwrap_or(other).replace('_', " "),
    }
}

/// `12 m`, then `1.2 km` from a kilometre on.
pub fn distance_text(d: f64) -> String {
    if d < 999.5 {
        format!("{} m", d.round() as i64)
    } else {
        format!("{:.1} km", d / 1000.0)
    }
}

pub fn color_name(rgb: u32) -> &'static str {
    PALETTE
        .iter()
        .position(|c| *c == rgb & 0xFF_FFFF)
        .map_or("Custom", |i| COLOR_NAMES[i])
}

fn next_color(rgb: u32) -> u32 {
    let at = PALETTE.iter().position(|c| *c == rgb & 0xFF_FFFF);
    PALETTE[at.map_or(0, |i| (i + 1) % PALETTE.len())]
}

/// A name of at most [`NAME_MAX`] characters, no control characters.
pub fn clean_name(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_control()).take(NAME_MAX).collect::<String>().trim().to_owned()
}

// ---- Filter --------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    All,
    Overworld,
    Nether,
    End,
}

impl Filter {
    pub fn next(self) -> Filter {
        match self {
            Filter::All => Filter::Overworld,
            Filter::Overworld => Filter::Nether,
            Filter::Nether => Filter::End,
            Filter::End => Filter::All,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Overworld => "Overworld",
            Filter::Nether => "Nether",
            Filter::End => "End",
        }
    }
    pub fn matches(self, dimension: &str) -> bool {
        match self {
            Filter::All => true,
            Filter::Overworld => dimension == OVERWORLD,
            Filter::Nether => dimension == NETHER,
            Filter::End => dimension == END,
        }
    }
}

// ---- The list --------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct List {
    pub waypoints: Vec<Waypoint>,
}

impl List {
    pub fn add(&mut self, w: Waypoint) -> usize {
        self.waypoints.push(w);
        self.waypoints.len() - 1
    }

    /// `Waypoint 1`, `Waypoint 2`...: the first number no waypoint has.
    pub fn default_name(&self) -> String {
        let mut n = 1;
        while self.waypoints.iter().any(|w| w.name == format!("Waypoint {n}")) {
            n += 1;
        }
        format!("Waypoint {n}")
    }

    fn death_name(&self) -> String {
        let top = self
            .waypoints
            .iter()
            .filter_map(|w| w.name.strip_prefix("Death ")?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("Death {}", top + 1)
    }

    /// A red `Death n` where the player died; the oldest of them go beyond
    /// [`DEATH_MAX`]. Renaming one (see [`List::rename`]) keeps it for good.
    pub fn add_death(&mut self, dimension: String, pos: [i32; 3]) {
        let name = self.death_name();
        self.waypoints.push(Waypoint {
            name,
            pos,
            dimension,
            color: DEATH_COLOR,
            hidden: false,
            death: true,
        });
        let deaths = self.waypoints.iter().filter(|w| w.death).count();
        let mut extra = deaths.saturating_sub(DEATH_MAX);
        self.waypoints.retain(|w| {
            let gone = w.death && extra > 0;
            extra -= usize::from(gone);
            !gone
        });
    }

    pub fn rename(&mut self, i: usize, name: &str) -> Result<(), String> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err("A waypoint needs a name".into());
        }
        let w = self.waypoints.get_mut(i).ok_or("No such waypoint")?;
        w.name = name;
        w.death = false;
        Ok(())
    }

    pub fn remove(&mut self, i: usize) -> bool {
        if i < self.waypoints.len() {
            self.waypoints.remove(i);
            true
        } else {
            false
        }
    }

    pub fn toggle_hidden(&mut self, i: usize, hidden: Option<bool>) -> bool {
        match self.waypoints.get_mut(i) {
            Some(w) => {
                w.hidden = hidden.unwrap_or(!w.hidden);
                true
            }
            None => false,
        }
    }

    /// Sets a colour by palette index, or moves to the next one.
    pub fn recolor(&mut self, i: usize, palette: Option<usize>) -> Result<u32, String> {
        let w = self.waypoints.get_mut(i).ok_or("No such waypoint")?;
        w.color = match palette {
            Some(p) => *PALETTE.get(p).ok_or("Colours go from 0 to 15")?,
            None => next_color(w.color),
        };
        Ok(w.color)
    }

    /// The waypoints a filter lets through, as indexes into the list.
    pub fn indexes(&self, filter: Filter) -> Vec<usize> {
        (0..self.waypoints.len()).filter(|i| filter.matches(&self.waypoints[*i].dimension)).collect()
    }
}

/// Where the player is: feet position and dimension.
pub struct Here<'a> {
    pub pos: [f64; 3],
    pub dimension: &'a str,
}

fn centre(w: &Waypoint) -> [f64; 3] {
    [f64::from(w.pos[0]) + 0.5, f64::from(w.pos[1]) + 0.5, f64::from(w.pos[2]) + 0.5]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// A row of the list screen: `#3 Home  100 64 -20  Overworld  25 m`. The
/// number is the waypoint's place in the whole list, which `/wp` takes.
pub fn row_text(i: usize, w: &Waypoint, here: Option<&Here<'_>>) -> String {
    let far = here
        .filter(|h| h.dimension == w.dimension)
        .map_or(String::new(), |h| format!("  {}", distance_text(distance(h.pos, centre(w)))));
    format!(
        "#{} {}{}  {} {} {}  {}{}",
        i + 1,
        w.name,
        if w.hidden { " (hidden)" } else { "" },
        w.pos[0],
        w.pos[1],
        w.pos[2],
        dimension_label(&w.dimension),
        far
    )
}

// ---- Projection -------------------------------------------------------------------

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Where the player looks: yaw 0 faces south (+Z), 90 west; pitch 90 down.
pub fn forward(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

type Basis = ([f64; 3], [f64; 3], [f64; 3]);

/// The camera's forward, right and up. Right comes from the yaw alone, so
/// looking straight up or down still has one.
fn basis(yaw: f32, pitch: f32) -> Basis {
    let f = forward(yaw, pitch);
    let y = f64::from(yaw).to_radians();
    let r = [-y.cos(), 0.0, -y.sin()];
    (f, r, cross(r, f))
}

/// `point` on a `size` screen as the world's perspective projection draws it
/// (vertical `fov`, the screen's aspect); None behind the camera.
pub fn project(camera: [f64; 3], view: Basis, fov: f32, point: [f64; 3], size: [f32; 2]) -> Option<[f32; 2]> {
    let (f, r, u) = view;
    let d = sub(point, camera);
    let z = dot(d, f);
    if z < 0.05 || size[1] <= 0.0 {
        return None;
    }
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0]) / f64::from(size[1]);
    let nx = dot(d, r) / (z * t * aspect);
    let ny = dot(d, u) / (z * t);
    Some([((nx + 1.0) / 2.0 * f64::from(size[0])) as f32, ((1.0 - ny) / 2.0 * f64::from(size[1])) as f32])
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

/// A thing to draw, in GUI pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// x, y, width, height; `0xAARRGGBB`.
    Fill([f32; 4], u32),
    /// text, x, y; `0xRRGGBB`; shadow.
    Text(String, [f32; 2], u32, bool),
}

/// The name and distance of each visible waypoint of `dimension` that is on
/// screen, farthest first so the near ones are drawn over them.
pub fn labels(
    waypoints: &[Waypoint],
    camera: Camera,
    player: [f64; 3],
    dimension: &str,
    size: [f32; 2],
    measure: &mut dyn FnMut(&str) -> f32,
) -> Vec<Draw> {
    let view = basis(camera.yaw, camera.pitch);
    let mut shown: Vec<(f64, [f32; 2], &Waypoint)> = waypoints
        .iter()
        .filter(|w| !w.hidden && w.dimension == dimension)
        .filter_map(|w| {
            let at = project(camera.pos, view, camera.fov, centre(w), size)?;
            let on_screen = at[0] >= 0.0 && at[0] <= size[0] && at[1] >= 0.0 && at[1] <= size[1];
            on_screen.then(|| (distance(player, centre(w)), at, w))
        })
        .collect();
    shown.sort_by(|a, b| a.0.total_cmp(&b.0));
    shown.truncate(MAX_LABELS);
    shown.reverse();
    let mut draws = Vec::new();
    for (d, [x, y], w) in shown {
        let dist = distance_text(d);
        let (name_w, dist_w) = (measure(&w.name), measure(&dist));
        let width = name_w + 6.0 + dist_w;
        let left = (x - width / 2.0).round();
        let top = (y - 16.0).round();
        draws.push(Draw::Fill([left - 3.0, top - 2.0, width + 6.0, 12.0], LABEL_BG));
        draws.push(Draw::Text(w.name.clone(), [left, top], w.color, true));
        draws.push(Draw::Text(dist, [left + name_w + 6.0, top], INK, true));
        // The marker: a dot in the waypoint's colour, ringed in black.
        draws.push(Draw::Fill([x - 3.0, y - 3.0, 6.0, 6.0], 0xFF00_0000));
        draws.push(Draw::Fill([x - 2.0, y - 2.0, 4.0, 4.0], 0xFF00_0000 | w.color));
    }
    draws
}

// ---- Deaths ------------------------------------------------------------------------

/// What the mod sees of the player each tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub dimension: String,
    pub pos: [f64; 3],
    pub health: f32,
    pub food: u32,
}

/// Finds a death from what `game.get-player` shows. The API has no death
/// event, so two signs are used: the player's health reads 0, or the player
/// vanishes at low health and comes back at full health and food somewhere
/// else (a respawn). A gap in the tick counter (another world) forgets it.
#[derive(Default)]
pub struct DeathTracker {
    last: Option<Snapshot>,
    gone: Option<Snapshot>,
    dead: bool,
    last_tick: Option<u64>,
}

fn spot(s: &Snapshot) -> (String, [i32; 3]) {
    (s.dimension.clone(), s.pos.map(|v| v.floor() as i32))
}

impl DeathTracker {
    pub fn step(&mut self, tick: u64, now: Option<Snapshot>) -> Option<(String, [i32; 3])> {
        let continuous = self.last_tick.is_some_and(|t| t.wrapping_add(1) == tick);
        self.last_tick = Some(tick);
        if !continuous {
            self.gone = None;
            self.dead = false;
            self.last = None;
        }
        let Some(s) = now else {
            if !self.dead {
                self.gone = self.last.clone();
            }
            return None;
        };
        let mut found = None;
        if s.health <= 0.0 {
            if !self.dead {
                self.dead = true;
                found = Some(spot(&s));
            }
        } else if self.dead {
            self.dead = false;
        } else if let Some(g) = self.gone.take() {
            let moved = g.dimension != s.dimension || distance(g.pos, s.pos) > 6.0;
            if g.health <= 8.0 && s.health >= 20.0 && s.food >= 20 && moved {
                found = Some(spot(&g));
            }
        }
        self.gone = None;
        self.last = Some(s);
        found
    }
}

// ---- /wp ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Help,
    List,
    Add(Option<String>),
    At([i32; 3], Option<String>),
    Rename(usize, String),
    Del(usize),
    Hide(usize),
    Show(usize),
    Color(usize, Option<usize>),
}

pub const HELP: &str = "/wp add [name] | at X Y Z [name] | rename N name | del N | hide N | show N | color N [0-15] | list. N is the # in the list.";

fn number(word: Option<&str>) -> Result<usize, String> {
    let n: usize = word.and_then(|w| w.trim_start_matches('#').parse().ok()).ok_or("Give the waypoint's number (the # in the list)")?;
    n.checked_sub(1).ok_or_else(|| "Numbers start at 1".to_string())
}

pub fn parse(args: &str) -> Result<Cmd, String> {
    let args = args.trim();
    let (word, rest) = args.split_once(char::is_whitespace).map_or((args, ""), |(w, r)| (w, r.trim()));
    let name = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_owned());
    match word {
        "" | "help" => Ok(Cmd::Help),
        "list" | "l" => Ok(Cmd::List),
        "add" | "new" => Ok(Cmd::Add(name(rest))),
        "at" => {
            let mut it = rest.splitn(4, char::is_whitespace);
            let mut coord = || -> Result<i32, String> {
                it.next().and_then(|w| w.parse().ok()).ok_or_else(|| "/wp at X Y Z [name]".to_string())
            };
            let pos = [coord()?, coord()?, coord()?];
            Ok(Cmd::At(pos, it.next().and_then(name)))
        }
        "rename" | "name" => {
            let (n, new) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Ok(Cmd::Rename(number(Some(n))?, new.trim().to_owned()))
        }
        "del" | "delete" | "remove" => Ok(Cmd::Del(number(rest.split_whitespace().next())?)),
        "hide" => Ok(Cmd::Hide(number(rest.split_whitespace().next())?)),
        "show" => Ok(Cmd::Show(number(rest.split_whitespace().next())?)),
        "color" | "colour" => {
            let mut it = rest.split_whitespace();
            let n = number(it.next())?;
            let c = match it.next() {
                Some(c) => Some(c.parse::<usize>().map_err(|_| "Colours go from 0 to 15".to_string())?),
                None => None,
            };
            Ok(Cmd::Color(n, c))
        }
        other => Err(format!("Unknown: {other}. {HELP}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wp(name: &str, pos: [i32; 3]) -> Waypoint {
        Waypoint { name: name.into(), pos, dimension: OVERWORLD.into(), color: 0xFF5555, hidden: false, death: false }
    }

    #[test]
    fn the_file_round_trips() {
        let mut list = vec![wp("Home\tbase", [1, -64, 300])];
        list.push(Waypoint { hidden: true, death: true, dimension: NETHER.into(), ..wp("Death 1", [0, 70, 0]) });
        assert_eq!(decode(&encode(&list)), list);
        assert!(decode("# waypoints 1\nbroken line\n").is_empty());
    }

    #[test]
    fn settings_round_trip_and_clamp() {
        let s = Settings { labels: false, deaths: true, fov: 3, smooth: true };
        assert_eq!(Settings::decode(&s.encode()), s);
        assert_eq!(Settings::decode("fov=99").fov, FOVS.len() - 1);
        assert_eq!(Settings::decode(""), Settings::default());
    }

    #[test]
    fn straight_ahead_is_the_middle_of_the_screen() {
        let cam = Camera { pos: [0.5, 0.5, 0.5], yaw: 0.0, pitch: 0.0, fov: 70.0 };
        let view = basis(cam.yaw, cam.pitch);
        let at = project(cam.pos, view, cam.fov, centre(&wp("a", [0, 0, 10])), [400.0, 200.0]).unwrap();
        assert!((at[0] - 200.0).abs() < 1e-3 && (at[1] - 100.0).abs() < 1e-3, "{at:?}");
        // Behind the camera.
        assert!(project(cam.pos, view, cam.fov, centre(&wp("a", [0, 0, -10])), [400.0, 200.0]).is_none());
    }

    #[test]
    fn yaw_90_faces_west_and_the_right_hand_is_north() {
        // Yaw 0 faces south (+Z): east (+X) is on the left, so west is on the right.
        let size = [400.0, 200.0];
        let cam = [0.5, 0.5, 0.5];
        let view = basis(0.0, 0.0);
        let west = project(cam, view, 70.0, centre(&wp("w", [-10, 0, 10])), size).unwrap();
        let east = project(cam, view, 70.0, centre(&wp("e", [10, 0, 10])), size).unwrap();
        assert!(west[0] > 200.0 && east[0] < 200.0);
        // Turned to face west (yaw 90), a waypoint to the west is dead ahead.
        let ahead = project(cam, basis(90.0, 0.0), 70.0, centre(&wp("w", [-10, 0, 0])), size).unwrap();
        assert!((ahead[0] - 200.0).abs() < 1e-3, "{ahead:?}");
        // Higher is up the screen.
        let up = project(cam, view, 70.0, centre(&wp("u", [0, 8, 10])), size).unwrap();
        assert!(up[1] < 100.0);
    }

    #[test]
    fn labels_show_only_visible_waypoints_of_this_dimension() {
        let mut hidden = wp("hidden", [0, 0, 10]);
        hidden.hidden = true;
        let mut nether = wp("nether", [0, 0, 11]);
        nether.dimension = NETHER.into();
        let list = vec![wp("near", [0, 0, 5]), wp("far", [0, 0, 50]), hidden, nether, wp("behind", [0, 0, -5])];
        let cam = Camera { pos: [0.5, 1.62, 0.5], yaw: 0.0, pitch: 0.0, fov: 70.0 };
        let mut width = |t: &str| t.len() as f32 * 6.0;
        let draws = labels(&list, cam, [0.5, 0.0, 0.5], OVERWORLD, [400.0, 200.0], &mut width);
        let names: Vec<&str> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text(t, _, c, _) if *c != INK => Some(t.as_str()),
                _ => None,
            })
            .collect();
        // Farthest first, so the near one is drawn over it.
        assert_eq!(names, ["far", "near"]);
        assert!(draws.contains(&Draw::Text("5 m".into(), [draws_x(&draws, "5 m"), draws_y(&draws, "5 m")], INK, true)));
    }

    fn draws_x(d: &[Draw], t: &str) -> f32 {
        d.iter().find_map(|d| if let Draw::Text(s, at, ..) = d { (s == t).then_some(at[0]) } else { None }).unwrap()
    }
    fn draws_y(d: &[Draw], t: &str) -> f32 {
        d.iter().find_map(|d| if let Draw::Text(s, at, ..) = d { (s == t).then_some(at[1]) } else { None }).unwrap()
    }

    #[test]
    fn distance_words() {
        assert_eq!(distance_text(12.4), "12 m");
        assert_eq!(distance_text(999.4), "999 m");
        assert_eq!(distance_text(1234.0), "1.2 km");
    }

    #[test]
    fn names_numbers_and_edits() {
        let mut l = List::default();
        assert_eq!(l.default_name(), "Waypoint 1");
        l.add(wp("Waypoint 1", [0, 0, 0]));
        l.add(wp("Waypoint 3", [0, 0, 0]));
        assert_eq!(l.default_name(), "Waypoint 2");
        assert!(l.rename(0, "  ").is_err());
        assert!(l.rename(9, "x").is_err());
        l.rename(0, "Home").unwrap();
        assert_eq!(l.waypoints[0].name, "Home");
        assert_eq!(clean_name(&"x".repeat(80)).len(), NAME_MAX);
        assert_eq!(l.recolor(0, None).unwrap(), PALETTE[1]);
        assert_eq!(l.recolor(0, Some(15)).unwrap(), 0x000000);
        assert!(l.recolor(0, Some(16)).is_err());
        assert!(l.toggle_hidden(0, None) && l.waypoints[0].hidden);
        assert!(l.remove(1) && !l.remove(5));
    }

    #[test]
    fn only_the_newest_deaths_stay_and_a_rename_keeps_one() {
        let mut l = List::default();
        for i in 0..7 {
            l.add_death(OVERWORLD.into(), [i, 64, 0]);
            if i == 0 {
                l.rename(0, "Treasure").unwrap();
            }
        }
        let names: Vec<&str> = l.waypoints.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["Treasure", "Death 2", "Death 3", "Death 4", "Death 5", "Death 6"]);
    }

    #[test]
    fn rows_say_where_and_how_far() {
        let w = wp("Home", [100, 64, -20]);
        let here = Here { pos: [100.5, 64.0, 5.5], dimension: OVERWORLD };
        assert_eq!(row_text(2, &w, Some(&here)), "#3 Home  100 64 -20  Overworld  25 m");
        let nether = Here { pos: [0.0, 0.0, 0.0], dimension: NETHER };
        assert_eq!(row_text(0, &w, Some(&nether)), "#1 Home  100 64 -20  Overworld");
    }

    #[test]
    fn filters_pick_dimensions() {
        let mut l = List::default();
        l.add(wp("a", [0; 3]));
        l.add(Waypoint { dimension: NETHER.into(), ..wp("b", [0; 3]) });
        assert_eq!(l.indexes(Filter::All), [0, 1]);
        assert_eq!(l.indexes(Filter::Nether), [1]);
        assert_eq!(Filter::End.next(), Filter::All);
    }

    fn snap(health: f32, food: u32, pos: [f64; 3]) -> Snapshot {
        Snapshot { dimension: OVERWORLD.into(), pos, health, food }
    }

    #[test]
    fn a_death_is_found_once_when_health_reads_zero() {
        let mut t = DeathTracker::default();
        assert_eq!(t.step(1, Some(snap(20.0, 20, [0.5, 64.0, 0.5]))), None);
        assert_eq!(t.step(2, Some(snap(0.0, 20, [3.7, 64.0, -2.2]))), Some((OVERWORLD.into(), [3, 64, -3])));
        assert_eq!(t.step(3, Some(snap(0.0, 20, [3.7, 64.0, -2.2]))), None);
        // The respawn is not a death.
        assert_eq!(t.step(4, Some(snap(20.0, 20, [100.0, 70.0, 100.0]))), None);
        assert_eq!(t.step(5, Some(snap(20.0, 20, [100.0, 70.0, 100.0]))), None);
    }

    #[test]
    fn a_respawn_without_a_zero_reading_is_found_from_the_last_place() {
        let mut t = DeathTracker::default();
        t.step(1, Some(snap(2.0, 18, [10.5, 64.0, 10.5])));
        assert_eq!(t.step(2, None), None);
        assert_eq!(t.step(3, None), None);
        assert_eq!(t.step(4, Some(snap(20.0, 20, [0.5, 80.0, 0.5]))), Some((OVERWORLD.into(), [10, 64, 10])));
    }

    #[test]
    fn healing_leaving_or_joining_another_world_is_not_a_death() {
        // Back in the same place with full health: not a respawn.
        let mut t = DeathTracker::default();
        t.step(1, Some(snap(2.0, 18, [10.5, 64.0, 10.5])));
        t.step(2, None);
        assert_eq!(t.step(3, Some(snap(20.0, 20, [10.5, 64.0, 10.5]))), None);
        // A gap in the tick counter (another world) forgets what was seen.
        let mut t = DeathTracker::default();
        t.step(1, Some(snap(2.0, 18, [10.5, 64.0, 10.5])));
        t.step(2, None);
        assert_eq!(t.step(900, Some(snap(20.0, 20, [500.0, 64.0, 500.0]))), None);
        // Low health that never recovers is not one either.
        let mut t = DeathTracker::default();
        t.step(1, Some(snap(2.0, 18, [10.5, 64.0, 10.5])));
        t.step(2, None);
        assert_eq!(t.step(3, Some(snap(2.0, 18, [10.5, 64.0, 10.5]))), None);
    }

    #[test]
    fn commands_parse() {
        assert_eq!(parse(""), Ok(Cmd::Help));
        assert_eq!(parse("add"), Ok(Cmd::Add(None)));
        assert_eq!(parse("add Iron  farm"), Ok(Cmd::Add(Some("Iron  farm".into()))));
        assert_eq!(parse("at 1 -2 3 Base camp"), Ok(Cmd::At([1, -2, 3], Some("Base camp".into()))));
        assert_eq!(parse("at 1 -2 3"), Ok(Cmd::At([1, -2, 3], None)));
        assert!(parse("at 1 x 3").is_err());
        assert_eq!(parse("rename 2 New name"), Ok(Cmd::Rename(1, "New name".into())));
        assert_eq!(parse("del #3"), Ok(Cmd::Del(2)));
        assert_eq!(parse("color 1 5"), Ok(Cmd::Color(0, Some(5))));
        assert_eq!(parse("color 1"), Ok(Cmd::Color(0, None)));
        assert!(parse("del 0").is_err());
        assert!(parse("del").is_err());
        assert!(parse("fly").unwrap_err().starts_with("Unknown: fly."));
    }
}
