# Bug and performance fixes (2026-10-24): TDD evidence

**Source plan:** the `/plan` of 2026-10-24 ("openomsi; focus on bug fixes and performance
fixes"), phases 1-4 approved by the user ("do phases 1-4, no github issues"), run with the
`tdd-workflow` skill. Commits on `custom`: `0d3f9d71`, `1de5ab31`, `3e5a710a`, `c3427a2a`.

## User journeys

- As a player, I start a session from the launcher and get to drive sooner.
- As a player, the game window never greys out as "Not responding" while it starts.
- As a player, driving through TH_Wald with traffic has no hitches when cars appear.
- As a player whose game crashed, I can copy the report and send it to the fork's author
  (the fork takes no GitHub issues).
- As the maintainer, the log tells me which build ran, and where the game was if it hung.

## Task report

### Phase 1 - load time (`0d3f9d71`)

Measured first, then fixed what the measurement showed; the planned texture disk cache
turned out to be a small part.

| Step | Finding | Change | Result |
|---|---|---|---|
| texture disk cache | non-DDS textures BC-compressed on every load | `omsi_texture::disk_cache` (`~/.openomsi/cache/textures`, 4 GB cap, keyed by path, size, time, options, format version) | only 116 of 815 textures of the first tiles needed CPU work; little time saved, kept |
| cut-step timers (`OMSI_PROFILE`) | "cut/textures" was the texture *list*: 6.2 s of lookups on one thread | lookups in parallel (`par_iter_mut` / `into_par_iter`) | list 6.2 -> 3.4 s |
| texture index | a texture missing from its folders walked the whole tree | parallel walk, built in the background at the start | 0.1-0.3 s, off the loading path |
| shader compiling | 168 scene pipelines one after another (15-31 s a start) | parallel, first one per distinct shader (each compiled once into wgpu's cache), then the rest; puddles/TAA/grass side by side | renderer 31 -> 21 s (launcher path, MSAA 4) |

A first "all 28 at once" version made the start slower (25-31 s): the duplicates compiled
the same shader side by side. Kept: the two-round version.

Launcher-path start (windowed, TH_Wald, `hangcheck.ps1` logs): to drivable ~48 s -> ~29 s;
first area (13 of 14 tiles) 13.8 s -> 6.4 s.

Tests: `disk_cache::tests` (7), `gpu::tests::a_second_load_of_a_bmp_comes_from_the_disk_cache`,
`a_picture_too_noisy_to_compress_is_not_tried_again`, `a_dds_that_carries_its_chain_is_not_kept_twice`,
`the_index_of_pictures_elsewhere_is_built_ahead`, `the_texture_index_holds_every_picture_of_the_tree`.
RED: 8 failed against stubs (`cargo test -p omsi-texture --lib`: "8 failed"), the index
test failed with the late file found. GREEN: 59, then 61 passed.

### Phase 2 - stutter (`1de5ab31` lane cells, `3e5a710a`, `c3427a2a`)

| Step | Finding | Change | Evidence |
|---|---|---|---|
| long AI paths (upstream ad2022fe, ported by hand: upstream moved the file) | `lane_cells` searched its list per cell | a set beside the list | `lane_cells_once_each_in_the_order_reached`: RED 189.8 ms > 50 ms, GREEN |
| road-ahead fill | 8.7-13 ms pass every 2 s | `populate_ahead_step` every 0.25 s, 1 lane / 1 car a step | `a_step_of_the_road_ahead_looks_at_one_lane_and_puts_one_car`: RED (6, MAX) != (1, 1), GREEN; offscreen first minute sight 2.59 -> 0.43 ms, pass 2.68 -> 1.47 ms, same cars ahead |
| waking sleeping cars | all woken in one pass | `WAKE_PER_PASS` 3, next pass after 0.5 s | `a_pass_wakes_a_few_sleeping_cars_and_comes_back_soon_for_the_rest`: RED MAX != 3, GREEN |
| 28-98 ms creations | timers showed the per-car blank text textures (texture + zero upload + mips + submit; ~30 ms when GPU memory grew), and the first car of a vehicle set / trailer | `GpuCache::text_pool` (16 per size, cleared on the GPU), `set_on_gpu` background set reads incl. trailers | `a_text_texture_of_a_vehicle_gone_is_taken_by_the_next`: RED, GREEN; `a_new_car_takes_one_of_the_first_paint_schemes_by_its_seed`: RED, GREEN |

Offscreen 20-minute Rennsteig drive (`offdrive.sh`, `OMSI_DEBUG_POPULATION`): slowest
population pass 28-98 ms (every run before) -> 6.9 ms.

### Phase 3 - frame time (no commit)

Offscreen `OMSI_BENCH` (TH_Wald terminus, 2560x1440, Enhanced, MSAA 8, AF 16; `bench3.sh`):
CPU 8.5-9.9 ms, GPU 23.6-25.6 ms a frame - GPU-bound. GPU: main pass 13.4-14.5, shadow
near 3.3, MSAA prepass 2.9, depth prepass 1.0, one matched mirror 3.0 ms.

| Item | Result |
|---|---|
| 3d light grid reuse | "lights" stage 0.1 ms a view: not worth it, dropped |
| 3b spline batching | blended splines (~900 draws) left out on purpose (draw order, existing test); CPU-side, no fps while GPU-bound: not done |
| 3e instance list | CPU-side; its growth was not the hitch (no "room made" log): not done |
| 3a mirror depth prepass | mirror passes 3.2 -> 2.0 ms, but the frame mean did not improve (34.22/34.30 off vs 34.19/34.79 on, same build, interleaved): reverted, kept as `D:/dev/wip-2026-10-24/mirror-prepass-tried.patch` |
| 3c shared depth prepass | at most ~1 ms GPU, within the ±1 ms run-to-run noise: not done |

The bigger lever is a setting: MSAA 8 -> 4 (measured 42 -> 49 fps by day on 2026-10-18).

### Phase 4 - bugs (`1de5ab31`, `3e5a710a`)

| Bug | Fix | Evidence |
|---|---|---|
| 4 AppHang reports (10-17..10-20) | window created hidden, shown once it can draw | `hangcheck.ps1` (IsHungAppWindow every 250 ms): old build "Not responding" ~28 s of the start; new 3 samples (<1 s) at the moment it shows |
| crash dialog opened GitHub issues (disabled on the fork) | no GitHub button, report copied for a message, 20 translations | `the_crash_dialog_sends_nobody_to_github`: RED, GREEN |
| log spam | settings / controllers / animation variable once | `the_settings_are_logged_again_only_when_they_change`, `the_controllers_are_logged_only_when_they_change`, `a_missing_variable_is_warned_about_once`: RED, GREEN |
| version 0.2.0 / 0.1.0.0 | `:/VERSION`, FileVersion | exe FileVersion 0.1.0 -> 0.2.80.0, log `0.2.80-custom` without `OPENOMSI_VERSION` |
| hangs say nothing | `watchdog.rs` | `a_stop_is_told_with_its_part_and_how_long_it_lasted`, `a_long_stop_is_told_again_once`: RED, GREEN |
| dead-code warnings | `glass_film` allow outside tests | release build without them |

## Test specification

| # | What is guaranteed | Test | Type | Result |
|---|---|---|---|---|
| 1 | a compressed texture comes back from the disk cache unchanged | `disk_cache::tests::a_compressed_texture_comes_back_from_the_disk` | unit | PASS |
| 2 | a changed file, other GPU options or a damaged entry is not used | `disk_cache::tests::*` | unit | PASS |
| 3 | a BMP's second load comes from the cache with identical levels | `gpu::tests::a_second_load_of_a_bmp_comes_from_the_disk_cache` | unit | PASS |
| 4 | the texture index is ready before a lookup needs it | `tests::the_index_of_pictures_elsewhere_is_built_ahead` | unit | PASS |
| 5 | a 1000 km path's cells in < 50 ms | `extend_tests::lane_cells_once_each_in_the_order_reached` | unit | PASS |
| 6 | the road ahead is filled 1 lane / 1 car a step | `ahead_step_tests::a_step_of_the_road_ahead_looks_at_one_lane_and_puts_one_car` | unit | PASS |
| 7 | 3 sleeping cars a pass, the rest within 4 s | `ahead_step_tests::a_pass_wakes_a_few_sleeping_cars_and_comes_back_soon_for_the_rest` | unit | PASS |
| 8 | a gone vehicle's text texture is taken by the next of its size | `scene::tests::a_text_texture_of_a_vehicle_gone_is_taken_by_the_next` (noop GPU) | integration | PASS |
| 9 | the crash dialog never names GitHub | `hint_tests::the_crash_dialog_sends_nobody_to_github` | unit | PASS |
| 10 | a hang is logged with its part and length | `watchdog::tests::*` | unit | PASS |

Full suites after the last change: `cargo test --release --target x86_64-pc-windows-msvc`
omsi-app 705 passed (6 ignored), omsi-render 119 (GPU `--ignored`: 25 pass, 2 known
failures also on the old code: `spv_dump` needs `SPV_DUMP`, the smoke puff test), omsi-sim
281, omsi-texture 61.

## Coverage and known gaps

- No coverage tool is set up for this Rust workspace; the guarantees are the tests above
  plus the measurements.
- Not unit-testable here: the hidden window (checked with `hangcheck.ps1`), parallel shader
  compiling (startup timing), background vehicle-set reads (offscreen drive logs).
- The slowest creations left are an S315's 16 text textures at a spawn (11-20 ms) before
  the pool has any of their sizes.
- Memory (the RADAR_PRE_LEAK reports of long test runs): a real-time windowed drive of
  the final build (Rennsteig duty, autopilot, traffic 60, 1280x720), private bytes sampled
  every 30 s, stopped at the user's word after 13.5 min: 5728 MB at 1 min, 5741-5760 MB
  from 5 to 13.5 min - flat within 30 MB, no growth. (Working set 2.43-2.48 GB.) The
  offscreen `--drive` is no soak: it does not draw in real time (20 game minutes in 80 s).
