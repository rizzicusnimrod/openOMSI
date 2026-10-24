//! Says in the log when the game's window stops answering, and where the main thread was.
//!
//! Windows' own reports of a hang (AppHangB1, "Not responding") say nothing of what the game
//! was doing. A thread asks Windows the same question it asks itself (`IsHungAppWindow`: no
//! message taken for 5 s) twice a second; the main thread names the part of the frame it is
//! in (`at`), so a stop is logged with that part, again if it lasts, and with its length
//! once the window answers again.
// (elsewhere than on Windows nothing asks: the watch is only tested there)
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::Duration;

/// A stop still going on is said again after this long.
const AGAIN_AFTER: Duration = Duration::from_secs(30);
/// Windows calls a window hung this long after the last message it took.
const HUNG_AFTER: f64 = 5.0;
/// How often the watcher asks.
const EVERY: Duration = Duration::from_millis(500);

/// The window watched (its handle; 0 none yet).
static WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Watch `window` from a thread of its own (started once; a later window takes its place).
/// Windows only: elsewhere nothing asks.
pub(crate) fn watch(window: &winit::window::Window) {
    let Some(hwnd) = crate::controllers::window_handle(window) else { return };
    if WINDOW.swap(hwnd, std::sync::atomic::Ordering::Relaxed) != 0 {
        return;
    }
    #[cfg(windows)]
    let _ = std::thread::Builder::new().name("watchdog".into()).spawn(|| {
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsHungAppWindow};
        let started = std::time::Instant::now();
        let mut watch = Watch::default();
        loop {
            std::thread::sleep(EVERY);
            let part = *PART.lock();
            // (until the first frame names its part: the window was just shown after the
            // shaders were compiled, and Windows still counts those seconds)
            if part == "start" {
                continue;
            }
            let h = WINDOW.load(std::sync::atomic::Ordering::Relaxed);
            let hung = unsafe { IsHungAppWindow(HWND(h as *mut _)) }.as_bool();
            if let Some(said) = watch.check(hung, started.elapsed().as_secs_f64(), part) {
                log::warn!("{said}");
            }
        }
    });
}

/// The part of the frame the main thread is in (`at`), for the log of a stop.
static PART: parking_lot::Mutex<&'static str> = parking_lot::const_mutex("start");

/// The main thread is now in `part` of its frame (the profile's stage names).
pub(crate) fn at(part: &'static str) {
    *PART.lock() = part;
}

/// What the watcher has seen of the window so far.
#[derive(Default)]
struct Watch {
    /// Since when (seconds of the watcher's clock) and in which part the window has not
    /// answered, and whether that was said again.
    stopped: Option<(f64, &'static str, bool)>,
}

impl Watch {
    /// The window `hung` or not at `now` (seconds), the main thread in `part`: what to log.
    fn check(&mut self, hung: bool, now: f64, part: &'static str) -> Option<String> {
        match (&mut self.stopped, hung) {
            (None, false) => None,
            (None, true) => {
                self.stopped = Some((now, part, false));
                Some(format!("the game's window is not answering (\"Not responding\"): the main thread has been in {part} for {HUNG_AFTER:.0} s or more"))
            }
            (Some((since, _, again)), true) => {
                if *again || now - *since < AGAIN_AFTER.as_secs_f64() {
                    return None;
                }
                *again = true;
                Some(format!("the game's window is still not answering after {:.0} s, the main thread in {part}", now - *since + HUNG_AFTER))
            }
            (Some((since, at, _)), false) => {
                let said = format!("the game's window answers again: it stopped for {:.1} s in {at}", now - *since + HUNG_AFTER);
                self.stopped = None;
                Some(said)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answering_window_says_nothing() {
        let mut w = Watch::default();
        for t in 0..100 {
            assert_eq!(w.check(false, t as f64 * 0.5, "render"), None);
        }
    }

    #[test]
    fn a_stop_is_told_with_its_part_and_how_long_it_lasted() {
        let mut w = Watch::default();
        assert_eq!(w.check(false, 0.0, "render"), None);
        let told = w.check(true, 10.0, "traffic").expect("the stop is told");
        assert!(told.contains("traffic") && told.contains("Not responding"), "{told}");
        // (said once, not every half second)
        assert_eq!(w.check(true, 10.5, "traffic"), None);
        assert_eq!(w.check(true, 12.0, "traffic"), None);
        let back = w.check(false, 14.0, "render").expect("the end is told");
        // (Windows calls it hung 5 s after the last message it took)
        assert!(back.contains("traffic") && back.contains("9.0 s"), "{back}");
        assert_eq!(w.check(false, 14.5, "render"), None);
    }

    #[test]
    fn a_long_stop_is_told_again_once() {
        let mut w = Watch::default();
        assert!(w.check(true, 1.0, "loading").is_some());
        assert_eq!(w.check(true, 20.0, "loading"), None);
        let again = w.check(true, 31.5, "loading").expect("still stopped");
        assert!(again.contains("still") && again.contains("loading"), "{again}");
        assert_eq!(w.check(true, 90.0, "loading"), None);
        assert!(w.check(false, 91.0, "events").is_some());
    }
}
