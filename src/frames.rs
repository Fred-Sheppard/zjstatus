use zellij_tile::prelude::*;

pub struct FrameConfig {
    pub hide_frames_for_single_pane: bool,
    pub hide_frames_except_for_search: bool,
    pub hide_frames_except_for_fullscreen: bool,
    pub hide_frames_except_for_scroll: bool,
    pub frame_style: PaneFrameStyle,
}

impl FrameConfig {
    pub fn new(
        hide_frames_for_single_pane: bool,
        hide_frames_except_for_search: bool,
        hide_frames_except_for_fullscreen: bool,
        hide_frames_except_for_scroll: bool,
        pane_frame_style: &str,
    ) -> Self {
        let frame_style = match pane_frame_style {
            "full" => PaneFrameStyle::Full,
            "titles" => PaneFrameStyle::Titles,
            "none" => PaneFrameStyle::None,
            _ => PaneFrameStyle::Titles,
        };

        Self {
            hide_frames_for_single_pane,
            hide_frames_except_for_search,
            hide_frames_except_for_fullscreen,
            hide_frames_except_for_scroll,
            frame_style,
        }
    }

    pub fn is_disabled(&self) -> bool {
        !self.hide_frames_for_single_pane
            && !self.hide_frames_except_for_search
            && !self.hide_frames_except_for_fullscreen
            && !self.hide_frames_except_for_scroll
    }
}

#[tracing::instrument(skip_all)]
pub fn hide_frames_conditionally(
    config: &FrameConfig,
    tabs: &[TabInfo],
    pane_info: &PaneManifest,
    mode_info: &ModeInfo,
    plugin_pane_id: PluginIds,
    is_zjframes: bool,
) {
    if config.is_disabled() {
        return;
    }

    let panes = match get_current_panes(tabs, pane_info) {
        Some(panes) => panes,
        None => return,
    };

    // check if we are running for the current tab since one plugin will run for
    // each tab. If we do not prevent execution, the screen will start to flicker
    // 'cause every plugin will try to toggle the frames.
    //
    // This is only relevant for zjstatus, not zjframes; as zjframes only
    // runs once per session.
    if !is_plugin_for_current_tab(&panes, plugin_pane_id) && !is_zjframes {
        return;
    }

    let panes: Vec<&PaneInfo> = panes
        .iter()
        .filter(|p| !p.is_plugin && !p.is_floating)
        .collect();

    tracing::debug!("panes: {:?}", panes);
    let current_style = mode_info
        .pane_frame_style
        .unwrap_or_else(|| detect_frame_style(&panes));

    let frames_for_search =
        config.hide_frames_except_for_search && should_show_frames_for_search(mode_info);
    let frames_for_fullscreen =
        config.hide_frames_except_for_fullscreen && should_show_frames_for_fullscreen(&panes);
    let frames_for_single_pane = config.hide_frames_for_single_pane
        && should_show_frames_for_multiple_panes(mode_info, &panes);
    let frames_for_scroll =
        config.hide_frames_except_for_scroll && should_show_frames_for_scroll(mode_info);

    tracing::debug!(
        "search {:?} fullscreen {:?} single {:?} scroll {:?} actual {:?}",
        frames_for_search,
        frames_for_fullscreen,
        frames_for_single_pane,
        frames_for_scroll,
        current_style,
    );

    let target_style = if frames_for_search
        || frames_for_fullscreen
        || frames_for_single_pane
        || frames_for_scroll
    {
        config.frame_style
    } else {
        PaneFrameStyle::None
    };

    if current_style != target_style {
        tracing::debug!("style: {:?}", target_style);
        set_pane_frame_style(target_style);
    }
}

// Fallback for hosts that do not report the frame style in ModeInfo
fn detect_frame_style(panes: &[&PaneInfo]) -> PaneFrameStyle {
    let visible = || panes.iter().filter(|&&p| !p.is_suppressed);

    if visible().any(|p| p.pane_content_x > p.pane_x) {
        PaneFrameStyle::Full
    } else if visible().any(|p| p.pane_content_y > p.pane_y) {
        PaneFrameStyle::Titles
    } else {
        PaneFrameStyle::None
    }
}

pub fn should_show_frames_for_scroll(mode_info: &ModeInfo) -> bool {
    mode_info.mode == InputMode::Scroll
}

pub fn should_show_frames_for_search(mode_info: &ModeInfo) -> bool {
    mode_info.mode == InputMode::EnterSearch || mode_info.mode == InputMode::Search
}

pub fn should_show_frames_for_fullscreen(panes: &[&PaneInfo]) -> bool {
    let active_pane = match panes.iter().find(|p| p.is_focused) {
        Some(p) => p,
        None => return false,
    };

    active_pane.is_fullscreen
}

#[tracing::instrument(skip_all)]
pub fn should_show_frames_for_multiple_panes(mode_info: &ModeInfo, panes: &[&PaneInfo]) -> bool {
    tracing::debug!("mode: {:?}", mode_info.mode);
    if mode_info.mode == InputMode::RenamePane
        || mode_info.mode == InputMode::Search
        || mode_info.mode == InputMode::EnterSearch
    {
        return true;
    }

    panes.len() > 1
}

fn is_plugin_for_current_tab(panes: &[PaneInfo], plugin_pane_id: PluginIds) -> bool {
    panes
        .iter()
        .any(|p| p.is_plugin && p.id == plugin_pane_id.plugin_id)
}

fn get_current_panes(tabs: &[TabInfo], pane_info: &PaneManifest) -> Option<Vec<PaneInfo>> {
    let active_tab = tabs.iter().find(|t| t.active);
    let active_tab = active_tab.as_ref()?;

    pane_info.panes.get(&active_tab.position).cloned()
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    fn pane(content_x: usize, content_y: usize, is_suppressed: bool) -> PaneInfo {
        PaneInfo {
            pane_x: 0,
            pane_y: 0,
            pane_content_x: content_x,
            pane_content_y: content_y,
            is_suppressed,
            ..PaneInfo::default()
        }
    }

    #[rstest]
    #[case(vec![pane(1, 1, false)], PaneFrameStyle::Full)]
    #[case(vec![pane(0, 1, false)], PaneFrameStyle::Titles)]
    #[case(vec![pane(0, 0, false)], PaneFrameStyle::None)]
    #[case(vec![pane(0, 0, false), pane(0, 1, false)], PaneFrameStyle::Titles)]
    #[case(vec![pane(0, 0, false), pane(1, 1, true)], PaneFrameStyle::None)]
    #[case(vec![], PaneFrameStyle::None)]
    pub fn test_detect_frame_style(#[case] panes: Vec<PaneInfo>, #[case] expected: PaneFrameStyle) {
        let panes: Vec<&PaneInfo> = panes.iter().collect();
        assert_eq!(detect_frame_style(&panes), expected);
    }
}
