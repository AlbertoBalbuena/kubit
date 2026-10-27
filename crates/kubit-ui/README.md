# kubit-ui

Desktop UI (layer 5): eframe + egui_dock over `kubit_engine::Session`. Every
document change is a `Session::execute` call (toolbar, shortcuts, popovers,
viewport clicks); commands never open dialogs, the UI opens file dialogs (rfd)
and passes the path as a parameter.

## Dependencies (checked on crates.io, 2026-10-27)

| Crate | Version | Licence | Why |
|---|---|---|---|
| `eframe`, `egui` | 0.36.2 | MIT OR Apache-2.0 | App shell and widgets. Not 0.37: `egui_dock` 0.21.1 (newest) requires egui `^0.36`. egui-wgpu 0.36 uses wgpu `^30`, the same wgpu as `kubit-render`. |
| `egui_dock` | 0.21.1 | MIT | Dockable tabs (viewport, features). |
| `rfd` | 0.17.2 | MIT | Native file dialogs. |
| `sys-locale` | 0.3.2 | MIT OR Apache-2.0 | System language for the default UI locale. |
| `pollster` | 1.0.1 | Apache-2.0 OR MIT | Blocks on device creation for offscreen screenshots. |

egui 0.36 needs Rust 1.95, so `kubit-ui` and `kubit` set `rust-version = "1.95"`.

## Pieces

- `theme`: every colour, spacing and radius token (dark and light); maps them
  onto egui's style and onto `kubit_render::RenderStyle`.
- `i18n`: es/en tables keyed like `cmd.solid.box` (registry labels) and `ui.*`;
  default from the system locale, fallback English, then the key itself.
- `viewport`: kubit-render on eframe's wgpu device, drawn offscreen into an
  sRGB texture that egui samples through an `Rgba8Unorm` view (egui-wgpu expects
  gamma-encoded native textures). Hover picks are asynchronous (submitted on one
  frame, resolved on a later one); a click uses the blocking native pick.
- `app`: dock, toolbar built from `CommandSpec::toolbar_group`, popovers for
  `solid.box` and `face.push_pull`, registry shortcuts (Ctrl means Cmd on macOS).
- `screenshot`: renders the same app offscreen on a headless egui-wgpu render
  state, no window needed (`kubit --screenshot out.png`).

Navigation: orbit with right or middle drag, pan with Shift + middle drag, zoom
to cursor with the wheel, `F` fits, standard views in the toolbar and View menu.
