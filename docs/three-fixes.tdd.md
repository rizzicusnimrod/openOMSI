# Third brake light, windscreen obstruction, cinematic headlamps: TDD evidence

**Source plan:** the `/plan` of 2026-10-24 ("On all vehicles, including trucks and buses, the
third brake light is popping out of nowhere, the rain/snow on windshield isnt obstructing
enough, and when in cinematic the headlights wont get dimmed down"), approved with "go ahead".
The plan's recommended windscreen slider (50-150 %) was built; the user did not answer the
question, "go ahead" was taken as approval of the plan as written.
**Interpretation recorded:** the "third brake light" was reproduced before any code was changed:
the Enhanced-only red road glow behind a braking vehicle (`lights.rs rear_glow`) lit the *cabin*
of the vehicle behind it, which then glowed red through its rear window
(`D:\dev\vistest\tb_bus_94.0.png` Enhanced against `tb_busvp_94.0.png` Vanilla+, same frame).
The cinematic report was traced with the user's own `game.log` (night front shots in Light Snow,
`cinematic_headlamps` already at 0.1): in clear weather the dim works, in snow and fog the
beams' light in the air was untouched (`cin_clear_50.0.png` against `cin_0.1_50.0.png`).
**Checkpoint commits:** RED and GREEN per fix on `custom`, local only (not pushed):
`ccd63d4d`/`563054a6` brake glow, `f21908e9`/`6c283b4a` cinematic dim, `a7147394`,
`5f952780`/`9cbd54e8` windscreen and its setting, `8cdd975e`/`98aa1bbd` snow on the glass and
offscreen snapshots.

## User journeys

- As a driver behind a braking car at night, I want only its own brake lamps to light up, not a
  red glow inside the car or bus behind it.
- As a driver in rain or snow, I want the water and snow on the windscreen to hide the road
  until the wipers pass, and to set how much (`glass_obstruction`).
- As a video maker filming the bus at night in fog or snow, I want the cinematic camera's
  headlamp dim to take down the beams' glow in the air too.

## Task report

| Task | Test target | RED | GREEN |
|---|---|---|---|
| Vehicle lights stop at other vehicles; a small light never gets inside one (`blockers.wgsl lamp_blocked`, Rust copy `light_past_vehicles`) | `-p omsi-render --lib a_brake_glow_leaves_the_cabin_behind_it_dark` | compile: `light_past_vehicles` missing | passed |
| The small lights carry their vehicle mask (`gpu_light`, `pos.w` below 0) | `-- a_vehicle_lamps_light_takes_its_vehicles_along` | compile (same target) | passed |
| The rear glow's cone stays under its lamps (`rear_glow_aim`, 34 deg down, 18/30 deg) | `-p omsi-app --lib the_rear_glow_never_shines_above_its_lamps` | compile: `rear_glow_aim` missing | passed |
| Every Enhanced vehicle light takes blockers (`stops_at_vehicles`) | `-- a_vehicles_lights_stop_at_other_vehicles` | compile: `stops_at_vehicles` missing | passed |
| Cinematic dim on the beams in fog and on snow and rain (`Enhanced.lens`, `lens_kept_light`) | GPU `a_film_cameras_dim_takes_the_beams_in_the_fog_down` (`--ignored`) | runtime: the beam's light in the fog 147.35 as it is, 147.36 at dim 0.9 | fog 35.61 -> 11.45, snowfall 32.10 -> 7.10 |
| The shader's dim curve is the Rust one | `the_shaders_lens_steps_are_these` | - (new twin test) | passed |
| A soaked pane hides most of the street; the sheet blurs it the wetter the more; snow (flakes, clumps, melt water) hides most of the view; all scaled by the setting | `-p omsi-render --lib glass_film` | compile: strength argument, `sheet_blur`, `RAIN_SHEET`, `SHEET_MAX`, `SHEET_BLUR`, `GLASS_SNOW_MELT` missing | 4 passed |
| The setting is read, clamped, kept and saved (`glass_obstruction`) | `-p omsi-app --lib the_windscreen_setting_round_trips`, `-p omsi-launcher-core the_games_options_survive_a_save` | app compile (no field), launcher runtime (no default) | passed |
| The picture behind the glass in snow too (`glass_weather`) | `-p omsi-render --lib the_glass_has_its_picture_behind_it_while_it_snows` | compile: `glass_weather` missing | passed |

Mutation check for the GPU test (the measure was refined after the first RED, from the picture's
mean to the lamp's share of it, the lamp weak enough not to burn out): with the shader's dim
forced off the test fails again (68.12 at dim 0.9 against 35.61 as it is).

## Test specification

| # | What is guaranteed | Test | Type | Result |
|---|---|---|---|---|
| 1 | A braking car's glow leaves the cabin of the car behind dark, lights its front face and the road between, and the car behind shadows the road past it | `a_brake_glow_leaves_the_cabin_behind_it_dark` | unit (Rust copy of the WGSL) | PASS |
| 2 | A vehicle's small light carries its vehicles in `pos.w` (below 0), keeps its inner cone; beams and classic lights unchanged | `a_vehicle_lamps_light_takes_its_vehicles_along` | unit | PASS |
| 3 | The rear glow's cone tops out 2-8 deg under its lamps and still reaches the road 8 m back | `the_rear_glow_never_shines_above_its_lamps` | unit | PASS |
| 4 | Beams and Enhanced small lights stop at vehicles; classic stand-ins do not | `a_vehicles_lights_stop_at_other_vehicles` | unit | PASS |
| 5 | A film camera's dim takes a headlamp facing it down in fog and snowfall (beam's light to under 40 %) | `a_film_cameras_dim_takes_the_beams_in_the_fog_down` | GPU | PASS |
| 6 | lamp_air.wgsl's dim steps equal the Rust ones | `the_shaders_lens_steps_are_these` | unit | PASS |
| 7 | A soaked pane hides >= 70 % between its drops (35 % at the setting's 50 %), never more than 97 %; a fresh wipe < 15 % | `a_soaked_pane_hides_most_of_the_street_between_its_drops` | unit | PASS |
| 8 | The sheet's blur grows with wetness and the setting, nothing when dry | `the_sheet_blurs_the_street_the_more_the_wetter` | unit | PASS |
| 9 | A snowed pane hides >= 70 % between its clumps, a clump 100 %, a little snow < 30 % | `a_snowed_pane_hides_most_of_the_view` | unit | PASS |
| 10 | shader.wgsl's film constants equal glass_film.rs's | `the_shaders_constants_are_these` | unit | PASS |
| 11 | `glass_obstruction` defaults to 100, clamps to 50-150, survives a save (game and launcher) | `the_windscreen_setting_round_trips`, `the_games_options_survive_a_save` | unit | PASS |
| 12 | The panes have their picture of the street in rain, on wet roads, in snowfall, with snow on the glass | `the_glass_has_its_picture_behind_it_while_it_snows` | unit | PASS |
| 13 | All shaders validate and translate to GLSL with the new functions and the new uniform field | `shaders_validate_and_match_the_uniforms`, `the_*_translates_to_glsl` | unit | PASS |

Full suites after the last change (`cargo test --release --target x86_64-pc-windows-msvc`):
omsi-render 124 passed (GPU `--ignored`: 26 passed, the 2 known failures `spv_dump` and the
smoke puff test fail as before), omsi-app 708, omsi-launcher-core 45, omsi-sim 281.

## Pictures (offscreen, `D:\dev\vistest`)

- Brake glow, the same frame (`OMSI_AUTOPILOT_TEST`, Rennsteig duty, 2004-12-17 22:00, camera
  26 m behind the bus): `tb_bus_94.0.png` before, `tb_busfix_94.0.png` after, `tb_busvp_94.0.png`
  Vanilla+. Red excess (R - (G+B)/2) in the rear window of the car behind: 12.6 before, 7.6
  after (warm street-lamp white, the red gone), -4.7 Vanilla+.
- Windscreen, driver's eye, Heavy Rain / Heavy Snowfall at 12:00 and 21:30: `ws_*_45.0.png`
  before (0.2.84), `wsfix5_*_45.0.png` after. Rain: the street behind is a soft blur, trees
  faint shapes, lamps spread into halos at night; mean brightness unchanged (day 120.8 ->
  121.7, night 54.5 -> 55.2). Snow: clumps and melt water hide most of the road by day; at night
  the snow on the glass shows dark against the lit road (the flakes did before too).
- Cinematic: `cin_clear_*.png` (clear, dim works), `cinfix_*.png` (fog, snow). The front shots
  are not repeatable frame for frame (the director cuts on the GPU's depth probes), so they show
  no clean before/after; the GPU test above is the measurement.

## Findings on the way

- Offscreen snapshots drew the panes of the vehicle the camera sits in with a picture of long
  ago (that vehicle is drawn last and takes the street from the frame before): every drop a
  dark speck - the "offscreen drops render dark" noted since the cinematic work. Snapshots in
  rain or snow now render a frame first, as the final picture always did. In the window every
  frame has the last one, so the game itself was not affected.
- Light Snow at night, camera in front of the bus: what stays bright in the picture is the snowy
  road between the camera and the bus, lit by the low beams - the road itself, which no dim of
  the lamps should take away (`fx_base_20.0.png` and the variants without coronas, glare or
  snowfall all show it).

## Coverage and known gaps

- No coverage tool is set up for this workspace; the guarantees are the tests above and the
  pictures.
- Not tested in the game window: how the dim cinematic shot looks to the user in their own
  weather; how the windscreen feels with the wipers running at speed (the wipe map drives the
  sheet as it drove the drops); the night snow on the glass being dark against lit roads.
- Not built (the plan's optional part): snow banks at the edges of the wiped arcs.
