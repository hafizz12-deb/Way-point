# Waypoints (hmod)

A port of Nirokx's native Horizon mod [Waypoints](https://github.com/Nirokx/horizon-waypoints) (MIT) to a sandboxed
`.hmod` (WebAssembly, mod API 0.1), so it can run where `.hnmod` source mods can't, such as the Android port.

## Use

| | |
|---|---|
| **N** | A waypoint where you stand, named `Waypoint 1`, `Waypoint 2`... |
| **M** | The list: filter by dimension, pick one to hide, recolour or delete it (click Delete twice) |
| `/wp add [name]` | A waypoint here, with a name |
| `/wp at X Y Z [name]` | A waypoint anywhere, in the dimension you are in |
| `/wp rename N name` | Rename; `N` is the `#` in the list |
| `/wp del N` · `hide N` · `show N` · `color N [0-15]` | The rest |
| `/wp list` | Opens the list |

Each visible waypoint of your dimension shows a dot, its name and its distance in the world. When you die, a red
`Death 1`, `Death 2`... marks the place; the newest 5 stay and renaming one keeps it.

The keys can be rebound in Options → Mods → Waypoints Hmod. On a phone, map a touch button to the key.
Settings (same page): names on or off, death waypoints, field of view, and whether names follow every frame.

## What differs from the native mod

- **Names are typed in chat**, not in a form: widgets have no text field. N makes a numbered waypoint; `/wp add`,
  `/wp rename` name it.
- **One list for every world and server.** A mod can't tell which world it is in, so waypoints carry their dimension
  only. Delete the ones of another world.
- **Field of view is a setting** (Options → Mods): a mod can't read the game's. Set it to your FOV, or names drift
  from their blocks when you turn. Names are placed by projecting the waypoint with the player's position and view
  angles, so they can lag a tick behind unless "every frame" is on.
- **Deaths are detected, not told**: the API has no death event. A waypoint is made when your health reads 0, or
  when you vanish at low health and come back at full health and food somewhere else. It can miss one.
- No 3D marker in the world, only the dot and the name over it.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo install --git https://github.com/horizon-rust/horizon --rev 56f968e11c7627536cb6a83420caddce7e3acb8f cargo-horizon-mod
cargo horizon-mod build        # target/horizon-mod/waypoints_hmod.hmod
cargo test                     # the logic's tests, on the host
```

The newest `cargo-horizon-mod` refuses WASM mods ("code mods are native now"), so the tool is pinned to the commit
the SDK is. `.github/workflows/build-hmod.yml` does it all: Actions → Run workflow.

## License

[MIT](LICENSE). The projection, the palette and the file format follow Nirokx's Waypoints.
