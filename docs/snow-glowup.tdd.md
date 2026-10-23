# Snow glow-up: TDD evidence

**Source plan:** the `/plan` of 2026-10-23 (snow glow-up + tyre marks), phases 0, 1, 2, 4 and 5;
phase 3 (dynamic tyre marks from the real wheels) skipped at the user's request.
**Interpretation recorded:** "snow looks ugly" was read as the falling flakes, the snow cover and
the roads together; "especially on TW" as TH_Wald's own `WinterSnowfall` road pictures (382 files,
`Splines/TH_Wald_Splines/texture/WinterSnowfall`), confirmed by offscreen renders before any change.
**Checkpoint commits:** none per stage - the fork is committed on the user's request only; the
work went into one commit after the full runs below.

## User journeys

- As a driver in a snow weather with snow on the road, I want the roads to look like winter roads
  (ruts, packed snow, slush), not a blurry picture.
- As a driver on a snowy day with cleared roads, I want dark wet or dry asphalt with snow at the
  kerb, not a white road.
- As a driver in falling snow, I want the flakes to streak past as I drive, not to hang as blobs.
- As a player who liked a map's own snowy pictures, I want a switch back to them.

## Task report

| Task | Test target | RED | GREEN |
|---|---|---|---|
| Snow state from the weather (`weather_setup::snow_lying`) | `cargo test -p omsi-app --lib -- weather_setup` | compile: `snow_lying`/`SnowLying` missing | 7 passed |
| Road takes the road's snow, melt water (`snow_cover::surface_snow`, `melt_wet`, `snow_row`) | `-p omsi-render -- snow_cover` | compile: functions and `Lighting.road_snow/slush` missing | 3 passed |
| Road textures skip `WinterSnowfall` (`omsi_texture::find_texture_seasoned`) | `-p omsi-texture` | compile: `find_texture_seasoned` missing | 49 passed |
| Lane wear map (`road_wear::raster`, `at`, `origin_for`, `needs_raster`) | `-p omsi-render -- road_wear` | compile: 22 missing items | 6 passed |
| Ruts and road layers (`rut`, `road_layers`, `rut_seen`, `streaked`) | `-p omsi-render -- snow_cover` | compile each time (`road_layers`/`rut`, `rut_seen`, `streaked`) | 9, 10, 11 passed |
| Wear lanes from the traffic (`tracks::wear_lanes_near`, `wants_new_map`) | `-p omsi-app -- tracks` | compile; then runtime: a 1.2 m AI path gave a 0.6 m half lane | 3 passed |
| Snow material (`patchy`, `snow_white`, `sparkle`) | `-p omsi-render -- snow_cover` | compile: 3 functions missing | 14 passed |
| Falling flakes (`snowfall::flake_streak`, `near_fade`, `camera_velocity`) | `-p omsi-render -- snowfall` | compile: 4 items missing | 5 passed |
| `SnowParams` grows `motion` | `shaders_validate_and_match_the_uniforms` | runtime: uniform size mismatch | passed |
| Setting `own_snowy_roads` (game) | `-p omsi-app -- settings::tests::the_looks` | compile: field missing | passed |
| Setting `own_snowy_roads` (launcher) | `-p omsi-launcher-core -- phase_two` | runtime assertion | 45 passed |

Every shader change was validated by naga in `shaders_validate_and_match_the_uniforms`, with
both scene variants (with and without the wear map texture).

One test changed after it was written: the two-lane wear test now puts the line between the lanes
on a texel's middle - the bilinear map reads up to half a texel short across that line, which
`road_wear.rs` documents and the shader's soft thresholds take in.

## Test specification

| # | What is guaranteed | Test | Type |
|---|---|---|---|
| 1 | A snow weather without `[snowOnRoad]` leaves the roads clear, with it covers them | `a_snow_weather_without_snow_on_the_road_leaves_the_roads_clear`, `snow_on_the_road_lies_on_the_road_too` | unit |
| 2 | Slush follows the temperature (none at -6 °C, all at +1.5 °C) | `the_slush_comes_with_the_thaw` | unit |
| 3 | The weather model lays thin snow thinly and the roads' last | `the_model_lays_thin_snow_thinly_and_the_road_last` | unit |
| 4 | A road texture keeps its bare picture under `WinterSnowfall`, other textures as OMSI 2 | `own_snowy_roads_keep_a_roads_bare_picture_in_the_snowfall` | unit |
| 5 | The wear map measures lane distance, traffic, width and the next lane; duplicate lanes are one | `road_wear::tests::*` | unit |
| 6 | Ruts lie at car and bus wheel lines; far off they fade to the lane's mean | `the_ruts_lie_where_car_and_bus_wheels_run`, `far_off_the_ruts_fade_to_the_lanes_mean_and_none_beside_it` | unit |
| 7 | Snowed-in roads wear in their ruts, cleared roads keep kerb snow and a line between lanes | `road_layers` tests | unit |
| 8 | Rut breakup moves snow between layers, never makes it; quiet lanes never wear through | `the_ruts_come_and_go_along_the_road_and_bare_the_asphalt_on_a_busy_one` | unit |
| 9 | Thin snow covers its share in patches; glints are sharp and fade with the pixel footprint | `thin_snow_lies_in_patches_covering_its_share`, `a_glint_is_sharp_and_fades_where_a_pixel_takes_in_many_crystals` | unit |
| 10 | Flakes streak across the line of sight only, at most 1.5 m, light spread along | `snowfall::tests::*` | unit |
| 11 | Camera velocity ignores pauses and jumps | `the_cameras_speed_comes_from_where_it_was_a_frame_ago` | unit |
| 12 | The new setting defaults on and survives game and launcher saves | `the_looks_of_phase_two_round_trip_with_their_defaults`, `the_looks_of_phase_two_survive_a_save` | unit |

## Visual checks (offscreen, TH_Wald, 2004-01-15)

`D:\dev\vistest\sn_*` (before), `sn1_*` (cleared roads), `sn2_*`/`sn3_*` (ruts, material),
`sn4_*` (final: ruts with streaks, heavy snow streaking in the wind, night drive with
`OMSI_SNOW_MOTION=0,16,0`). The lavender bushes were traced to TH_Wald's own frosted
`WinterSnow` pictures (unchanged with seasonal foliage off), not changed.

## Coverage and known gaps

- No coverage tool is installed for this workspace (cargo-llvm-cov absent); coverage was not
  measured. The new pure functions each have tests; the GPU paths are covered by naga validation
  and offscreen renders, not by GPU tests.
- Not tested: a live session (sound, LAN, a weather change mid-session reloading the season), the
  wear map's worker thread under a long drive, Vanilla+ beyond compile/validation.
- Phase 3 (real tyre marks) not built.
- Follow-up: sharper road snow and fewer glints, `docs/snow-sharpness.tdd.md`.
