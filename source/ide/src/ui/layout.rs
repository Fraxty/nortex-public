//! Pure panel-size policy: keeps the editor dominant at every window size.

use super::activity_bar;

pub const EXPLORER_MIN: f32 = 180.0;
pub const EXPLORER_MAX: f32 = 520.0;
pub const SIDECAR_MIN: f32 = 220.0;
pub const SIDECAR_MAX: f32 = 520.0;
pub const CONSOLE_MIN: f32 = 80.0;
pub const CONSOLE_MAX: f32 = 600.0;
/// Editor never gets less than this many points of width ...
pub const EDITOR_MIN_WIDTH: f32 = 500.0;
/// ... nor less than this share of the window width.
pub const EDITOR_MIN_SHARE: f32 = 0.5;
/// Estimated height of menu, tab/toolbar rows, status lines and the bottom status bar.
pub const CHROME_HEIGHT: f32 = 280.0;
/// Separators, frames and window borders that eat width beyond the panel sizes.
pub const PANEL_FRAME_ALLOWANCE: f32 = 24.0;
/// Rows of code that should stay visible next to the console.
pub const MIN_VISIBLE_LINES: f32 = 15.0;
pub const LINE_HEIGHT: f32 = 21.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelSize {
    pub default: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutInput {
    pub window: (f32, f32),
    pub zen: bool,
    pub explorer_open: bool,
    pub sidecar_visible: bool,
    /// The user explicitly asked for the sidecar (shortcut/menu); it then wins over the explorer.
    pub sidecar_forced: bool,
    pub console_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutPlan {
    pub activity_bar: bool,
    pub explorer: Option<PanelSize>,
    pub sidecar: Option<PanelSize>,
    pub console: Option<PanelSize>,
    /// True when the sidecar was requested but hidden to protect the editor width.
    pub sidecar_auto_collapsed: bool,
}

impl LayoutPlan {
    /// Editor width when side panels sit at their default sizes.
    pub fn editor_width(&self, window_width: f32) -> f32 {
        let bar = if self.activity_bar {
            activity_bar::WIDTH
        } else {
            0.0
        };
        window_width
            - bar
            - self.explorer.map_or(0.0, |panel| panel.default)
            - self.sidecar.map_or(0.0, |panel| panel.default)
    }

    pub fn visible_lines(&self, window_height: f32) -> f32 {
        let console = self.console.map_or(0.0, |panel| panel.default);
        ((window_height - CHROME_HEIGHT - console) / LINE_HEIGHT).max(0.0)
    }
}

fn sized(default: f32, min: f32, max: f32) -> PanelSize {
    let max = max.max(min);
    PanelSize {
        default: default.clamp(min, max),
        min,
        max,
    }
}

pub fn compute(input: LayoutInput) -> LayoutPlan {
    let (width, height) = (input.window.0.max(1.0), input.window.1.max(1.0));
    if input.zen {
        return LayoutPlan {
            activity_bar: false,
            explorer: None,
            sidecar: None,
            console: None,
            sidecar_auto_collapsed: false,
        };
    }

    let editor_min = EDITOR_MIN_WIDTH.max(width * EDITOR_MIN_SHARE);
    let budget = (width - activity_bar::WIDTH - editor_min - PANEL_FRAME_ALLOWANCE).max(0.0);

    let explorer_own_max = (width * 0.22).clamp(EXPLORER_MIN, EXPLORER_MAX);
    let sidecar_own_max = (width * 0.25).clamp(SIDECAR_MIN, SIDECAR_MAX);
    let explorer_floor = if input.explorer_open {
        EXPLORER_MIN
    } else {
        0.0
    };

    let fits_beside_explorer = budget - explorer_floor >= SIDECAR_MIN;
    let fits_alone = budget >= SIDECAR_MIN;
    let wants_sidecar = input.sidecar_visible;
    let sidecar_shown =
        wants_sidecar && (fits_beside_explorer || (input.sidecar_forced && fits_alone));
    let explorer_shown = input.explorer_open && (!sidecar_shown || fits_beside_explorer);

    let sidecar = sidecar_shown.then(|| {
        let explorer_max = explorer_own_max.min((budget - SIDECAR_MIN).max(EXPLORER_MIN));
        let taken = if explorer_shown { explorer_max } else { 0.0 };
        sized(
            width * 0.22,
            SIDECAR_MIN,
            sidecar_own_max.min(budget - taken),
        )
    });
    let explorer = explorer_shown.then(|| {
        let room = if sidecar_shown {
            budget - SIDECAR_MIN
        } else {
            budget
        };
        sized(width * 0.20, EXPLORER_MIN, explorer_own_max.min(room))
    });

    let console = input.console_open.then(|| {
        let by_share = (height * 0.30).clamp(CONSOLE_MIN, CONSOLE_MAX);
        let by_lines = height - CHROME_HEIGHT - MIN_VISIBLE_LINES * LINE_HEIGHT;
        let max = by_share.min(by_lines).max(60.0);
        sized(height * 0.25, CONSOLE_MIN.min(max), max)
    });

    LayoutPlan {
        activity_bar: true,
        explorer,
        sidecar,
        console,
        sidecar_auto_collapsed: wants_sidecar && !sidecar_shown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(width: f32, height: f32) -> LayoutInput {
        LayoutInput {
            window: (width, height),
            zen: false,
            explorer_open: true,
            sidecar_visible: true,
            sidecar_forced: false,
            console_open: true,
        }
    }

    fn max_editor_shrink(plan: &LayoutPlan, width: f32) -> f32 {
        // Worst case: the user drags both side panels to their maximum.
        width
            - activity_bar::WIDTH
            - plan.explorer.map_or(0.0, |panel| panel.max)
            - plan.sidecar.map_or(0.0, |panel| panel.max)
    }

    #[test]
    fn editor_keeps_half_the_width_at_standard_resolutions() {
        for (width, height) in [(1920.0, 1080.0), (1600.0, 900.0), (1280.0, 720.0)] {
            let plan = compute(input(width, height));
            assert!(plan.explorer.is_some() && plan.sidecar.is_some());
            assert!(plan.editor_width(width) >= width * 0.5, "default {width}");
            assert!(
                max_editor_shrink(&plan, width) >= width * 0.5,
                "max {width}"
            );
        }
    }

    #[test]
    fn panel_limits_match_the_specification() {
        let plan = compute(input(1920.0, 1080.0));
        let explorer = plan.explorer.unwrap();
        let sidecar = plan.sidecar.unwrap();
        let console = plan.console.unwrap();
        assert_eq!(explorer.min, EXPLORER_MIN);
        assert_eq!(sidecar.min, SIDECAR_MIN);
        assert_eq!(console.min, CONSOLE_MIN);
        assert!(explorer.max <= EXPLORER_MAX && explorer.max > 300.0);
        assert!(sidecar.max <= SIDECAR_MAX && sidecar.max > 350.0);
        assert!(console.max <= CONSOLE_MAX && console.max > 280.0);
        assert!(explorer.default <= 0.22 * 1920.0 && sidecar.default <= 0.25 * 1920.0);
    }

    #[test]
    fn sidecar_collapses_when_the_editor_would_drop_below_500() {
        let narrow = compute(input(800.0, 600.0));
        assert!(narrow.sidecar.is_none() && narrow.sidecar_auto_collapsed);
        assert!(narrow.editor_width(800.0) >= 500.0);
        let wide = compute(input(1280.0, 720.0));
        assert!(wide.sidecar.is_some() && !wide.sidecar_auto_collapsed);
    }

    #[test]
    fn forced_sidecar_replaces_the_explorer_on_narrow_windows() {
        let mut request = input(700.0, 600.0);
        request.sidecar_forced = true;
        let plan = compute(request);
        assert!(
            plan.sidecar.is_none(),
            "150 px budget cannot host a sidecar"
        );

        let mut request = input(900.0, 600.0);
        request.sidecar_forced = true;
        let plan = compute(request);
        assert!(plan.sidecar.is_some());
        assert!(plan.explorer.is_none());
        assert!(plan.editor_width(900.0) >= 500.0);
    }

    #[test]
    fn zen_mode_collapses_everything() {
        let mut request = input(1280.0, 720.0);
        request.zen = true;
        let plan = compute(request);
        assert!(plan.explorer.is_none() && plan.sidecar.is_none() && plan.console.is_none());
        assert!(!plan.activity_bar);
        assert_eq!(plan.editor_width(1280.0), 1280.0);
    }

    #[test]
    fn console_leaves_at_least_fifteen_lines_from_720p_upwards() {
        for height in [720.0, 900.0, 1080.0] {
            let plan = compute(input(1280.0, height));
            let console = plan.console.unwrap();
            assert!(
                plan.visible_lines(height) >= MIN_VISIBLE_LINES,
                "default {height}"
            );
            let at_max = (height - CHROME_HEIGHT - console.max) / LINE_HEIGHT;
            assert!(at_max >= MIN_VISIBLE_LINES, "max {height}");
        }
    }

    #[test]
    fn console_never_exceeds_its_cap_nor_vanishes_the_editor() {
        for height in [300.0, 400.0, 600.0, 1440.0] {
            let console = compute(input(1280.0, height)).console.unwrap();
            assert!(console.max <= CONSOLE_MAX);
            assert!(console.min <= console.default && console.default <= console.max);
            assert!(console.max < height);
        }
    }

    #[test]
    fn degenerate_window_sizes_do_not_panic() {
        for (width, height) in [
            (0.0, 0.0),
            (400.0, 300.0),
            (f32::NAN, 100.0),
            (50.0, 5000.0),
        ] {
            let plan = compute(input(width, height));
            for panel in [plan.explorer, plan.sidecar, plan.console]
                .into_iter()
                .flatten()
            {
                assert!(panel.min <= panel.default && panel.default <= panel.max);
            }
        }
    }
}
