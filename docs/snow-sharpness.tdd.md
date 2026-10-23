# Sharper road snow, fewer glints: TDD evidence

**Source plan:** the `/plan` of 2026-10-23 "road snow looks low resolution", phases 1-3 and 5,
plus the user's addition "the snow glitters/shines way too much than it should, lower it".
Phase 4 (the asphalt showing through pressed snow) was left out: the user approved the plan with
"go ahead" without choosing it.
**Interpretation recorded:** "low resolution" was read as the procedural snow's lack of detail
below about half a metre (no texture is involved: crops of `D:\dev\vistest\sn4_light_road.png`
showed smooth bands and a flat white); "shines" as the ruts' polished sheen against a low sun
(`sn5b_sun_back.png`: gold streaks in the ruts) as well as the glints themselves.
**Checkpoint commits:** none per stage - the fork is committed on the user's request only.

## User journeys

- As a driver on a snowed-in road, I want the ruts, the snow between them and the kerb to look
  sharp close to the bus at 2560x1440, not like a blurred low-resolution picture.
- As a driver on a cleared road, I want the snow bank at the kerb and the line between the lanes
  to have crisp, ragged edges.
- As a driver facing a low winter sun, I want the snow to glint only now and then in tiny points,
  and the ruts not to shine like polished metal.
- As a driver looking far down the road, I want the sharper detail not to shimmer or stair-step.

## Task report

| Task | Test target | RED | GREEN |
|---|---|---|---|
| Wear map in half floats, metres (`road_wear::raster`, `at`, `Wear::texel`) | `-p omsi-render --lib -- road_wear` | runtime: `the_distance_between_texels_is_held_to_a_millimetre` read 0.010818958 m off (limit 0.001) | 7 passed |
| Half floats read back (`atmosphere::f16_value`) | `-- half_floats` | compile: `f16_value` missing | passed |
| Sharp ruts through the pixel (`rut(d, soft)`, `box_band`, `RUT_AREA` 0.4325) | `-- snow_cover` | compile: `rut` took 1 argument, `RUT_EDGE` missing | passed |
| Rut walls tilt the snow (`rut_tilt`) | `-- snow_cover` | compile: `rut_tilt`, `RUT_TILT_MAX` missing | passed |
| Sharp kerb bank and middle line (`road_layers(.., soft)`, `ramp`, `KERB_IN`, `BETWEEN_SEP`) | `-- snow_cover` | compile: 6-argument call, constants missing | passed |
| Fine grain, tread lines (`fine_fade`, `snow_grain`, `tread`) | `-- snow_cover` | compile: functions missing | passed |
| Fewer, pinpoint, dimmer glints (`GLINT_SHARE` 0.04 -> 0.015, `GLINT_PEAK` 6 -> 2, `glint_spot`, `facet_dot`) | `-- snow_cover` | compile: functions and constants missing | passed |
| Shader twins keep the Rust constants | `the_shaders_constants_are_these` | runtime: `RUT_AREA` not in snow_cover.wgsl | passed |
| Every shader still validates (both wear-map variants) | `shaders_validate_and_match_the_uniforms` | - | passed |

Full run after the shader work: `cargo test --release --target x86_64-pc-windows-msvc -p omsi-render --lib`
-> `test result: ok. 112 passed; 0 failed; 27 ignored`. The two compiler warnings it prints
(`lib.rs:15653` unused `mut` in a test, `relief.rs:14` `RELIEF_DEPTH`) were there before.
App: `cargo test --release --target x86_64-pc-windows-msvc -p omsi-app --lib` -> `685 passed`
(the wear map's worker, `tracks.rs`, takes the new texels unchanged).

### Rounds after the first renders (what the pictures showed, what changed)

Every round: `cargo test ... -p omsi-render --lib` green, release build, renders at 2560x1440
with `D:\dev\vistest\sn5.sh` / `sn5sun.sh` (`sn5b_*` = v0.2.78 as released, `sn5n_*`, `sn6_*` ...
`sn11_*` the rounds below; the sunny views use a test-only weather,
`testbin\Weather\Test Sunny Snow Road.owt` = Crisp Winter Sun with `[snowOnRoad]`).

| Round | Seen | Change | Test |
|---|---|---|---|
| sn5n | Ruts crisp; a 2x2-pixel checker on the loose snow in a low sun; no glints left; no tyre lines in the ruts | - | - |
| sn6 | Grain relief from the screen's derivatives (shared per 2x2 pixels) replaced by the noise's own slope (`bilerp_slope`); tyre lines through their own pixel width (`tread_soft` = `fwidth(lane_d)`: through `RUT_EDGE`'s 3.5 cm they always faded out) and on the lane's distance before the ragged edges; slush takes lines and grain; pressed snow roughness 0.45-0.7 -> 0.6-0.8 | tyre lines visible; checker still there | RED (compile): `bilerp_slope` missing; GREEN |
| sn7 | The checker is the value noise's slope itself (naught along every cell's edge: a grid under grazing light). Grain relief removed - the grain stays in the snow's white; `bilerp_slope`, `vnoise_slope` and their test removed with it | checker gone | suite green |
| sn8-sn10 | Glints: 0 glint pixels where v0.2.78 had 17 (margin 40) - spot dimmed by the pixel, then FXAA | `glint_spot` pixel-sized, `GLINT_PEAK` 5, `GLINT_R` 1 cm | test updated with the constants (no separate RED: tuning) |
| sn11 | Single-pixel glints are wiped out by FXAA (it takes a lone bright pixel for a jagged edge) | spot radius one pixel (some two pixels across) | test updated with the code (no separate RED: tuning) |

## Visual checks (offscreen, TH_Wald, 2004-01-15 12:00, 2560x1440)

- `sn5b_light_road` / `sn8_light_road` (Light Snow): the ruts' walls a few centimetres wide and
  ragged instead of a quarter of a metre of blur; the snow grainy instead of a flat white; far off
  the ruts stay one calm grey (no stair steps).
- `sn5b_light_top` / `sn6_light_top`: tyre lines along the ruts.
- `sn5b_crisp_road` / `sn8_crisp_road` (Crisp Winter Sun, cleared roads): the line of salty snow
  between the lanes and the kerb bank sharp-edged.
- `sn5b_sun_back` / `sn11_sun_back` (into a low sun): the ruts' gold sheen gone; glints counted in
  the region x 800-1500, y 950-1300 by the scratchpad's `glints.ps1` (pixels standing out of their
  5x5 neighbourhood):

| Build | margin 25 | margin 40 |
|---|---|---|
| v0.2.78 (`sn5b`) | 183 | 17 |
| final (`sn11`) | 49 | 7 |

Each shot gets a random script seed, so the bus settles a little differently from run to run:
before and after are framed alike, not pixel for pixel.

## Test specification

| # | What is guaranteed | Test | Type |
|---|---|---|---|
| 1 | The wear map holds the lane distance to a millimetre between texels | `road_wear::tests::the_distance_between_texels_is_held_to_a_millimetre` | unit |
| 2 | Half floats read back exactly | `atmosphere::tests::half_floats` | unit |
| 3 | A rut's walls go from snow to rut within 6 cm near the eye | `a_ruts_walls_are_sharp_near_the_eye` | unit |
| 4 | Through any pixel width the ruts keep their share (`RUT_AREA`) and stay 0..1 | `a_wider_pixel_softens_the_walls_and_keeps_the_ruts_share` | unit |
| 5 | Rut walls tilt the snow the right way, the floor is flat, never beyond `RUT_TILT_MAX`, gentle far off, none without snow | `a_ruts_walls_tilt_the_snow_and_its_floor_lies_flat` | unit |
| 6 | The kerb bank and the line between lanes have edges a few centimetres wide, spread far off | `a_cleared_roads_kerb_bank_and_middle_line_have_sharp_edges` | unit |
| 7 | Fine detail fades out once a pixel spans its cells; the grain's finest octave first | `fine_detail_fades_once_a_pixel_spans_its_cells`, `the_snows_grain_shows_near_and_fades_out_finest_first` | unit |
| 8 | Tyre lines show only where a pixel resolves them | `the_tyres_lines_show_only_where_a_pixel_resolves_them` | unit |
| 9 | Glints: at most 2 % of cells, below the old peak, some two pixels across, as much light through a wider pixel, on average under 1 % of the snow's light | `glints_are_rare_pinpoints_not_a_blinding_sheet` | unit |
| 10 | The WGSL twin's constants are the Rust ones | `the_shaders_constants_are_these` | unit |
| 11 | Every shader validates, both wear-map variants | `shaders_validate_and_match_the_uniforms` | unit (naga) |
| 12 | Earlier guarantees (ruts at the wheel lines, far mean, road layers, streaks, patches, white) | the rest of `snow_cover::tests` | unit |

## Performance

Not measured cleanly: the PC was in use during the windowed benchmark (`D:\dev\perftest\benchw.sh`,
Light Snow, terminus, 2560x1440, `OMSI_GPU_TIMERS=1`), and the same build's main pass ranged from
3.0 to 11.4 ms between runs (the undisturbed runs of 2026-10-23 noon held it at 7.8-7.95 ms).
Raw runs, old = v0.2.78, new = this work:

| Run | old main pass | new main pass |
|---|---|---|
| 1 | 9.40 ms (52.9 fps) | 3.02 ms (37.2 fps) - window not drawing |
| 2 | 7.21 ms (39.0 fps) | 7.53 ms (37.3 fps) |
| 3 | 9.06 ms (55.8 fps) | 11.41 ms (47.8 fps) |

What the change adds per pixel: the grain (up to three value-noise octaves, only within about 90 m
and only those a pixel resolves), the tyre lines (two octaves in the ruts within 90 m), the ragged
edges (two octaves), two `fwidth`s, and the rut walls' arithmetic; it drops the five-tap fine-line
noise of v0.2.78. The wear map is twice the memory (8 MB) and upload. To repeat on an idle PC:
`WEATHER="Light Snow.owt" DATE=2004-01-15 TIME=12:00 GPUT=1 ./benchw.sh <name> <exe> enhanced`.

## Coverage and known gaps

- No coverage tool is installed for this workspace (cargo-llvm-cov absent); coverage was not
  measured. The new pure functions each have tests; the shader paths are covered by naga
  validation and offscreen renders, not by GPU tests.
- The tyre lines' direction along the lane, the ragged edges and the grain's look are shader
  geometry, checked in renders only.
- Not tested: a live drive (motion shimmer of the sharp edges was judged from still frames
  only), Vanilla+ beyond validation, other maps than TH_Wald.
- Pre-existing flake seen once in the full suite and not in three reruns:
  `the_scene_groups_fit_the_sixteen_texture_units_of_opengl` counts the wear map's texture when
  another test has opened a GPU device first (`WEAR_MAP` is process-wide) - since v0.2.78, not
  touched here.
- Phase 4 (asphalt showing through pressed snow) not built.
