//! The game window's size and fullscreen mode, changed while the game runs (the game
//! menu's *Window size* and *Fullscreen*): a window is resized and put in the middle of its
//! screen; in fullscreen a chosen size is the screen's own resolution (an exclusive video
//! mode of it), *Automatic* the desktop's (borderless). The renderer follows the window's
//! new size as it follows any resize.

use winit::window::{Fullscreen, Window};

/// The size a window gets for the setting `want` (None: *Automatic*) on a screen of
/// `screen` pixels: what was asked, no larger than the screen; else 1600 x 900, no larger
/// than nine tenths of it.
pub(crate) fn window_size(want: Option<(u32, u32)>, screen: (u32, u32)) -> (u32, u32) {
    match want {
        Some((w, h)) => (w.min(screen.0).max(320), h.min(screen.1).max(240)),
        None => (1600.min(screen.0 * 9 / 10), 900.min(screen.1 * 9 / 10)),
    }
}

/// Which of a screen's video modes (width, height, refresh rate in mHz) shows `want`: the
/// one of that size with the highest refresh rate.
pub(crate) fn pick_mode(want: (u32, u32), modes: &[(u32, u32, u32)]) -> Option<usize> {
    modes.iter().enumerate().filter(|(_, m)| (m.0, m.1) == want).max_by_key(|(_, m)| m.2).map(|(i, _)| i)
}

/// Put `window` in the mode the settings ask for: fullscreen or not, at `want` (None:
/// *Automatic*). A fullscreen size the screen has no mode for is drawn borderless at the
/// desktop's.
pub(crate) fn apply(window: &Window, fullscreen: bool, want: Option<(u32, u32)>) {
    let monitor = window.current_monitor().or_else(|| window.primary_monitor());
    if fullscreen {
        let mode = want.zip(monitor.as_ref()).and_then(|(want, m)| {
            let modes: Vec<_> = m.video_modes().collect();
            let list: Vec<(u32, u32, u32)> = modes.iter().map(|v| (v.size().width, v.size().height, v.refresh_rate_millihertz())).collect();
            pick_mode(want, &list).map(|i| modes[i].clone())
        });
        match mode {
            Some(mode) => {
                log::info!("display: fullscreen at {}x{} ({:.0} Hz)", mode.size().width, mode.size().height, mode.refresh_rate_millihertz() as f32 / 1000.0);
                window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
            }
            None => {
                if let Some((w, h)) = want {
                    log::info!("display: the screen has no {w}x{h} mode; fullscreen at the desktop's");
                }
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
        return;
    }
    window.set_fullscreen(None);
    let screen = monitor.as_ref().map_or((1920, 1080), |m| (m.size().width, m.size().height));
    let (w, h) = window_size(want, screen);
    let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
    if let Some(m) = monitor {
        let (x, y) = (m.position().x + ((screen.0 as i32 - w as i32) / 2).max(0), m.position().y + ((screen.1 as i32 - h as i32) * 2 / 5).max(0));
        window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }
    log::info!("display: a window of {w}x{h}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_is_the_size_asked_and_never_larger_than_its_screen() {
        assert_eq!(window_size(Some((1280, 720)), (2560, 1440)), (1280, 720));
        assert_eq!(window_size(Some((3840, 2160)), (2560, 1440)), (2560, 1440));
        // automatic: 1600 x 900, or nine tenths of a small screen
        assert_eq!(window_size(None, (2560, 1440)), (1600, 900));
        assert_eq!(window_size(None, (1366, 768)), (1229, 691));
    }

    #[test]
    fn fullscreen_takes_the_screens_mode_of_that_size_at_its_fastest() {
        let modes = [(1920, 1080, 60_000), (2560, 1440, 59_951), (2560, 1440, 143_912), (1280, 720, 60_000)];
        assert_eq!(pick_mode((2560, 1440), &modes), Some(2));
        assert_eq!(pick_mode((1280, 720), &modes), Some(3));
        assert_eq!(pick_mode((1600, 900), &modes), None);
    }
}
