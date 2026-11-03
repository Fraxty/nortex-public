//! Icon-only activity bar plus the workspace state it drives.

use eframe::egui::{
    self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};

use super::branding::BrandTextures;
use crate::i18n::tr;

pub const WIDTH: f32 = 50.0;
const ITEM_HEIGHT: f32 = 46.0;
pub const BACKGROUND: Color32 = Color32::from_rgb(0x11, 0x13, 0x15);
pub const INACTIVE: Color32 = Color32::from_rgb(0x85, 0x8B, 0x91);
/// Active editor and IDE functions.
pub const ACTIVE: Color32 = Color32::from_rgb(0x96, 0x70, 0xE8);
/// Toolchain and status indicators.
pub const STATUS: Color32 = Color32::from_rgb(0x39, 0x8B, 0xFF);
const SEPARATOR: Color32 = Color32::from_rgb(0x2A, 0x2E, 0x33);

/// What the central area shows. Switching never discards documents or editor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Editor,
    Dashboard,
    Settings,
    Extensions,
}

/// Content of the left side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideView {
    #[default]
    Explorer,
    Search,
    Git,
    Debugger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityAction {
    Home,
    Explorer,
    Editor,
    Search,
    Git,
    Debugger,
    Settings,
    Extensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workspace {
    pub view_mode: ViewMode,
    pub side_view: SideView,
    pub side_open: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Editor,
            side_view: SideView::Explorer,
            side_open: true,
        }
    }
}

impl Workspace {
    pub fn apply(&mut self, action: ActivityAction) {
        let side = match action {
            ActivityAction::Explorer => Some(SideView::Explorer),
            ActivityAction::Search => Some(SideView::Search),
            ActivityAction::Git => Some(SideView::Git),
            ActivityAction::Debugger => Some(SideView::Debugger),
            _ => None,
        };
        if let Some(view) = side {
            let collapse =
                self.side_open && self.side_view == view && self.view_mode == ViewMode::Editor;
            self.side_open = !collapse;
            self.side_view = view;
            self.view_mode = ViewMode::Editor;
            return;
        }
        self.view_mode = match action {
            ActivityAction::Home => ViewMode::Dashboard,
            ActivityAction::Settings if self.view_mode == ViewMode::Settings => ViewMode::Editor,
            ActivityAction::Settings => ViewMode::Settings,
            ActivityAction::Extensions if self.view_mode == ViewMode::Extensions => {
                ViewMode::Editor
            }
            ActivityAction::Extensions => ViewMode::Extensions,
            _ => ViewMode::Editor,
        };
    }

    fn is_active(&self, action: ActivityAction) -> bool {
        match action {
            ActivityAction::Home => self.view_mode == ViewMode::Dashboard,
            ActivityAction::Editor => self.view_mode == ViewMode::Editor,
            ActivityAction::Settings => self.view_mode == ViewMode::Settings,
            ActivityAction::Extensions => self.view_mode == ViewMode::Extensions,
            ActivityAction::Explorer => self.side_open && self.side_view == SideView::Explorer,
            ActivityAction::Search => self.side_open && self.side_view == SideView::Search,
            ActivityAction::Git => self.side_open && self.side_view == SideView::Git,
            ActivityAction::Debugger => self.side_open && self.side_view == SideView::Debugger,
        }
    }
}

pub const ITEMS: [(ActivityAction, &str); 6] = [
    (ActivityAction::Explorer, "Explorer"),
    (ActivityAction::Editor, "Editor"),
    (ActivityAction::Search, "Suche"),
    (ActivityAction::Git, "Git"),
    (ActivityAction::Debugger, "Debugger"),
    (ActivityAction::Extensions, "Erweiterungen"),
];

fn active_color(action: ActivityAction) -> Color32 {
    match action {
        ActivityAction::Git | ActivityAction::Debugger => STATUS,
        ActivityAction::Extensions => ACTIVE,
        _ => ACTIVE,
    }
}

/// Draws the bar and returns the clicked action, if any.
pub fn show(
    context: &egui::Context,
    workspace: &Workspace,
    textures: &BrandTextures,
) -> Option<ActivityAction> {
    let mut clicked = None;
    egui::SidePanel::left("activity-bar")
        .default_width(WIDTH)
        .width_range(40.0..=90.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(BACKGROUND))
        .show(context, |ui| {
            let panel = ui.max_rect();
            ui.painter().vline(
                panel.right() - 0.5,
                panel.y_range(),
                Stroke::new(1.0, SEPARATOR),
            );
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            let width = ui.available_width().max(1.0);
            if home_button(ui, workspace, textures, width) {
                clicked = Some(ActivityAction::Home);
            }
            ui.add_space(6.0);
            for (action, tooltip) in ITEMS {
                if item(ui, workspace, action, tr(tooltip), width) {
                    clicked = Some(action);
                }
            }
            ui.add_space((ui.available_height() - ITEM_HEIGHT).max(0.0));
            if item(
                ui,
                workspace,
                ActivityAction::Settings,
                tr("Einstellungen"),
                width,
            ) {
                clicked = Some(ActivityAction::Settings);
            }
        });
    clicked
}

fn marker(ui: &egui::Ui, rect: Rect, color: Color32) {
    let strip = Rect::from_min_size(rect.min, vec2(2.0, rect.height()));
    ui.painter().rect_filled(strip, CornerRadius::ZERO, color);
}

fn home_button(
    ui: &mut egui::Ui,
    workspace: &Workspace,
    textures: &BrandTextures,
    width: f32,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, ITEM_HEIGHT * width / WIDTH), Sense::click());
    let response = response.on_hover_text(tr("Dashboard (NORTEX)"));
    let active = workspace.is_active(ActivityAction::Home);
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgb(0x1A, 0x1D, 0x21),
        );
    }
    if active {
        marker(ui, rect, ACTIVE);
    }
    let logo = Rect::from_center_size(rect.center(), vec2(26.0, 26.0) * (width / WIDTH));
    let tint = if active || response.hovered() {
        Color32::WHITE
    } else {
        Color32::from_gray(0xC8)
    };
    match textures.symbol() {
        Some(texture) => {
            ui.painter().image(
                texture.id(),
                logo,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                tint,
            );
        }
        None => {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "N",
                egui::FontId::proportional(20.0 * width / WIDTH),
                tint,
            );
        }
    }
    response.clicked()
}

fn item(
    ui: &mut egui::Ui,
    workspace: &Workspace,
    action: ActivityAction,
    tooltip: &str,
    width: f32,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, ITEM_HEIGHT * width / WIDTH), Sense::click());
    let response = response.on_hover_text(tooltip);
    let active = workspace.is_active(action);
    let color = if active {
        active_color(action)
    } else if response.hovered() {
        Color32::from_gray(0xC4)
    } else {
        INACTIVE
    };
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgb(0x1A, 0x1D, 0x21),
        );
    }
    if active {
        marker(ui, rect, active_color(action));
    }
    draw_icon(ui.painter(), action, rect.center(), color, width / WIDTH);
    response.clicked()
}

fn draw_icon(painter: &egui::Painter, action: ActivityAction, c: Pos2, color: Color32, s: f32) {
    // Local shadow: every offset and size below scales with the bar width.
    let vec2 = |x: f32, y: f32| egui::vec2(x * s, y * s);
    let stroke = Stroke::new(1.6 * s.max(0.8), color);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
    match action {
        ActivityAction::Explorer => {
            let back = Rect::from_min_size(c + vec2(-6.0, -10.0), vec2(12.0, 15.0));
            let front = Rect::from_min_size(c + vec2(-10.0, -6.0), vec2(12.0, 15.0));
            painter.rect_stroke(back, CornerRadius::ZERO, stroke, StrokeKind::Inside);
            painter.rect_filled(front, CornerRadius::ZERO, BACKGROUND);
            painter.rect_stroke(front, CornerRadius::ZERO, stroke, StrokeKind::Inside);
        }
        ActivityAction::Editor => {
            painter.add(egui::Shape::line(
                vec![
                    c + vec2(-4.0, -7.0),
                    c + vec2(-11.0, 0.0),
                    c + vec2(-4.0, 7.0),
                ],
                stroke,
            ));
            painter.add(egui::Shape::line(
                vec![c + vec2(4.0, -7.0), c + vec2(11.0, 0.0), c + vec2(4.0, 7.0)],
                stroke,
            ));
            line(c + vec2(2.0, -9.0), c + vec2(-2.0, 9.0));
        }
        ActivityAction::Search => {
            painter.circle_stroke(c + vec2(-2.0, -2.0), 7.0 * s, stroke);
            line(c + vec2(3.0, 3.0), c + vec2(10.0, 10.0));
        }
        ActivityAction::Git => {
            let top = c + vec2(0.0, -9.0);
            let bottom = c + vec2(0.0, 9.0);
            let branch = c + vec2(8.0, -2.0);
            line(top, bottom);
            line(c + vec2(0.0, 3.0), branch);
            for node in [top, bottom, branch] {
                painter.circle_filled(node, 3.0 * s, BACKGROUND);
                painter.circle_stroke(node, 3.0 * s, stroke);
            }
        }
        ActivityAction::Debugger => {
            painter.circle_stroke(c + vec2(0.0, 2.0), 6.0 * s, stroke);
            painter.circle_stroke(c + vec2(0.0, -6.0), 3.0 * s, stroke);
            for side in [-1.0_f32, 1.0] {
                line(c + vec2(side * 6.0, 0.0), c + vec2(side * 11.0, -3.0));
                line(c + vec2(side * 6.0, 4.0), c + vec2(side * 11.0, 7.0));
            }
        }
        ActivityAction::Settings => {
            painter.circle_stroke(c, 6.0 * s, stroke);
            painter.circle_filled(c, 2.0 * s, color);
            for step in 0..8 {
                let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                let direction = egui::vec2(angle.cos(), angle.sin());
                line(c + direction * 8.0 * s, c + direction * 11.0 * s);
            }
        }
        ActivityAction::Extensions => {
            let body = Rect::from_min_size(c + vec2(-7.0, -7.0), vec2(14.0, 14.0));
            painter.rect_stroke(body, CornerRadius::ZERO, stroke, StrokeKind::Inside);
            painter.circle_stroke(c + vec2(0.0, -9.0), 2.5 * s, stroke);
            painter.circle_stroke(c + vec2(9.0, 0.0), 2.5 * s, stroke);
            painter.circle_stroke(c + vec2(0.0, 9.0), 2.5 * s, stroke);
            painter.circle_stroke(c + vec2(-9.0, 0.0), 2.5 * s, stroke);
        }
        ActivityAction::Home => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_shows_dashboard_and_keeps_side_panel_state() {
        let mut workspace = Workspace::default();
        workspace.apply(ActivityAction::Search);
        workspace.apply(ActivityAction::Home);
        assert_eq!(workspace.view_mode, ViewMode::Dashboard);
        assert_eq!(
            (workspace.side_view, workspace.side_open),
            (SideView::Search, true)
        );
    }

    #[test]
    fn dashboard_and_editor_toggle_via_home_and_editor() {
        let mut workspace = Workspace::default();
        workspace.apply(ActivityAction::Home);
        workspace.apply(ActivityAction::Editor);
        assert_eq!(workspace.view_mode, ViewMode::Editor);
    }

    #[test]
    fn clicking_active_side_view_collapses_it_and_reopens_on_second_click() {
        let mut workspace = Workspace::default();
        workspace.apply(ActivityAction::Explorer);
        assert!(!workspace.side_open);
        workspace.apply(ActivityAction::Explorer);
        assert!(workspace.side_open);
    }

    #[test]
    fn side_view_click_from_dashboard_returns_to_editor_and_opens_panel() {
        let mut workspace = Workspace {
            side_open: false,
            ..Workspace::default()
        };
        workspace.apply(ActivityAction::Home);
        workspace.apply(ActivityAction::Git);
        assert_eq!(workspace.view_mode, ViewMode::Editor);
        assert_eq!(
            (workspace.side_view, workspace.side_open),
            (SideView::Git, true)
        );
    }

    #[test]
    fn settings_toggles_and_leaves_side_panel_untouched() {
        let mut workspace = Workspace::default();
        workspace.apply(ActivityAction::Settings);
        assert_eq!(workspace.view_mode, ViewMode::Settings);
        assert!(workspace.side_open);
        workspace.apply(ActivityAction::Settings);
        assert_eq!(workspace.view_mode, ViewMode::Editor);
    }

    #[test]
    fn extensions_view_toggles_without_changing_side_panel_state() {
        let mut workspace = Workspace::default();
        workspace.apply(ActivityAction::Extensions);
        assert_eq!(workspace.view_mode, ViewMode::Extensions);
        assert!(workspace.side_open);
        workspace.apply(ActivityAction::Extensions);
        assert_eq!(workspace.view_mode, ViewMode::Editor);
    }

    #[test]
    fn activity_bar_width_stays_within_spec() {
        assert!((48.0..=52.0).contains(&WIDTH));
    }

    #[test]
    fn bar_renders_headless_and_reports_no_click_without_input() {
        let context = egui::Context::default();
        let mut textures = BrandTextures::default();
        textures.load(&context);
        let workspace = Workspace::default();
        for size in [vec2(400.0, 300.0), vec2(1280.0, 800.0)] {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            };
            let mut result = Some(ActivityAction::Home);
            let _ = context.run(input, |context| {
                result = show(context, &workspace, &textures)
            });
            assert_eq!(result, None);
        }
    }
}
