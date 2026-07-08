use crate::fullscreen::is_fullscreen_app_running;
use crate::ipc::run_transition_and_set;
use lw_core::config::Config;
use lw_core::traits::WallpaperManager;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Debug, Clone, Default)]
pub struct SchedulerState {
    pub active: bool,
    pub next_change_at: Option<Instant>,
}

/// Discovers wallpaper files in the directory.
#[must_use]
pub fn get_wallpaper_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ext_str == "png" || ext_str == "jpg" || ext_str == "jpeg" || ext_str == "bmp"
                    {
                        files.push(path);
                    }
                }
            }
        }
    }
    files.sort(); // Deterministic ordering
    files
}

/// Runs the background scheduler rotation loop.
pub async fn run_scheduler<W>(
    config: Arc<Mutex<Config>>,
    wallpaper_manager: Arc<W>,
    state: Arc<Mutex<SchedulerState>>,
) where
    W: WallpaperManager + 'static,
{
    info!("Starting background wallpaper rotation scheduler task...");
    let mut last_run = Instant::now();
    let mut first_run = true;

    loop {
        // Poll state every 1 second to stay responsive and precise
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Retrieve latest config values
        let (enabled, interval_mins, shuffle, wallpaper_dir, transition_default, change_on_startup) = {
            let cfg = config.lock().unwrap();
            (
                cfg.scheduler.enabled,
                cfg.scheduler.interval_mins,
                cfg.shuffle,
                cfg.wallpaper_dir.clone(),
                cfg.transition_default.clone(),
                cfg.scheduler.change_on_startup,
            )
        };

        // Update current active status
        {
            let mut st = state.lock().unwrap();
            st.active = enabled;
        }

        if !enabled {
            let mut st = state.lock().unwrap();
            st.next_change_at = None;
            continue;
        }

        // Validate directory
        if !wallpaper_dir.exists() || !wallpaper_dir.is_dir() {
            let mut st = state.lock().unwrap();
            st.next_change_at = None;
            continue;
        }

        // Read wallpaper list dynamically
        let wallpapers = get_wallpaper_files(&wallpaper_dir);
        if wallpapers.is_empty() {
            let mut st = state.lock().unwrap();
            st.next_change_at = None;
            continue;
        }

        let interval = Duration::from_secs(u64::from(interval_mins) * 60);

        // Determine if rotation should be triggered
        let should_change = if first_run {
            first_run = false;
            if change_on_startup {
                info!("Scheduler triggering change_on_startup wallpaper rotation...");
                true
            } else {
                false
            }
        } else {
            Instant::now().duration_since(last_run) >= interval
        };

        // Update next change calculation for IPC status command
        {
            let mut st = state.lock().unwrap();
            st.next_change_at = Some(last_run + interval);
        }

        if should_change {
            // Check for fullscreen games/apps to defer rotation
            if is_fullscreen_app_running() {
                // Delay checks by 10s intervals until app exits fullscreen
                last_run = Instant::now().checked_sub(interval).unwrap_or_else(Instant::now)
                    + Duration::from_secs(10);
                {
                    let mut st = state.lock().unwrap();
                    st.next_change_at = Some(last_run + interval);
                }
                info!("Fullscreen application detected. Deferring scheduled wallpaper change by 10 seconds.");
                continue;
            }

            // Select and execute transition
            let current_wp = wallpaper_manager.get_current_wallpaper().unwrap_or_default();
            if let Some(target_wp) = select_next_wallpaper(&wallpapers, &current_wp, shuffle) {
                info!("Scheduler rotating wallpaper: {:?}", target_wp);

                let params = lw_core::ipc::TransitionParams {
                    effect_type: transition_default.effect_type.clone(),
                    duration_ms: transition_default.duration_ms,
                    easing: transition_default.easing,
                };

                if let Err(e) =
                    run_transition_and_set(&target_wp, &params, wallpaper_manager.as_ref())
                {
                    error!("Scheduler failed to perform transition: {e:?}");
                }
            }

            last_run = Instant::now();
        }
    }
}

#[must_use]
pub fn select_next_wallpaper(
    wallpapers: &[PathBuf],
    current_wp: &Path,
    shuffle: bool,
) -> Option<PathBuf> {
    if wallpapers.is_empty() {
        return None;
    }
    if wallpapers.len() == 1 {
        return Some(wallpapers[0].clone());
    }

    if shuffle {
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seed = u64::try_from(seed & 0xFFFF_FFFF_FFFF_FFFF).unwrap_or(0);
        let mut rng = LcgRng::new(seed);

        let current_index = wallpapers.iter().position(|p| p == current_wp);
        let mut next_index = rng.next_range(0..=(wallpapers.len() - 1));

        if let Some(curr) = current_index {
            if next_index == curr {
                next_index = (next_index + 1) % wallpapers.len();
            }
        }
        Some(wallpapers[next_index].clone())
    } else {
        let current_index = wallpapers.iter().position(|p| p == current_wp);
        let next_index = match current_index {
            Some(idx) => (idx + 1) % wallpapers.len(),
            None => 0,
        };
        Some(wallpapers[next_index].clone())
    }
}

struct LcgRng {
    state: u64,
}

impl LcgRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1_442_695_040_888_963_407) }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    fn next_range(&mut self, range: std::ops::RangeInclusive<usize>) -> usize {
        let low = *range.start();
        let high = *range.end();
        if low >= high {
            return low;
        }
        let diff = (high - low + 1) as u64;
        let val = self.next_u64() % diff;
        low + usize::try_from(val).unwrap_or(0)
    }
}
