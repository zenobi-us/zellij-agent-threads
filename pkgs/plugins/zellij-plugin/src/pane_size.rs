//! Synchronizes sidebar width across plugin instances.
//!
//! Zellij resizes panes relatively, so this module keeps the resize protocol in
//! one place and nudges each plugin pane toward configured target widths.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;

const SIZE_PIPE_NAME: &str = "zellij-agent-threads-layout";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneSizeConfig {
    pub(crate) collapsed_cols: usize,
    pub(crate) expanded_cols: usize,
}

#[derive(Default)]
pub(crate) struct PaneSizeService {
    config: PaneSizeConfig,
    peers: Vec<u32>,
    own_plugin_url: Option<String>,
    own_cols: usize,
    resize_direction: Option<Direction>,
    revision: u64,
    seen_revisions: HashMap<u32, u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PaneSizeMessage {
    kind: String,
    collapsed: bool,
    target_cols: usize,
    revision: u64,
    source_plugin_id: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PaneSizeEffect {
    ResizePluginPane {
        pane_id: PaneId,
        current_cols: usize,
        target_cols: usize,
        direction: Option<Direction>,
    },
    SendControlMessage {
        peer: u32,
        payload: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PaneSizeUpdate {
    pub(crate) collapsed: bool,
    pub(crate) effects: Vec<PaneSizeEffect>,
}

pub(crate) struct ZellijLayoutAdapter;

impl ZellijLayoutAdapter {
    pub(crate) fn apply(effects: Vec<PaneSizeEffect>) {
        for effect in effects {
            match effect {
                PaneSizeEffect::ResizePluginPane {
                    pane_id,
                    current_cols,
                    target_cols,
                    direction,
                } => resize_plugin_pane_to(pane_id, current_cols, target_cols, direction),
                PaneSizeEffect::SendControlMessage { peer, payload } => {
                    send_control_message(peer, payload)
                }
            }
        }
    }
}

impl Default for PaneSizeConfig {
    fn default() -> Self {
        Self {
            collapsed_cols: 8,
            expanded_cols: 16,
        }
    }
}

impl PaneSizeService {
    pub(crate) fn new(config: PaneSizeConfig) -> Self {
        Self {
            config,
            peers: Vec::new(),
            own_plugin_url: None,
            own_cols: 0,
            resize_direction: None,
            revision: 0,
            seen_revisions: HashMap::new(),
        }
    }

    pub(crate) fn set_current_cols(&mut self, cols: usize) {
        if cols > 0 {
            self.own_cols = cols;
        }
    }

    pub(crate) fn sync_peers(&mut self, self_id: Option<u32>, manifest: PaneManifest) {
        let Some(self_id) = self_id else { return };
        let all_panes: Vec<_> = manifest
            .panes
            .values()
            .flat_map(|panes| panes.iter())
            .collect();
        let all_plugins: Vec<_> = all_panes
            .iter()
            .copied()
            .filter(|pane| pane.is_plugin)
            .collect();

        if let Some(own) = all_plugins.iter().find(|pane| pane.id == self_id) {
            self.own_plugin_url = own.plugin_url.clone();
            self.own_cols = own.pane_content_columns;
            self.resize_direction = resize_border_direction(own, &all_panes);
        }

        let Some(own_plugin_url) = self.own_plugin_url.as_deref() else {
            self.peers.clear();
            return;
        };

        self.peers = all_plugins
            .iter()
            .filter(|pane| pane.id != self_id)
            .filter(|pane| pane.plugin_url.as_deref() == Some(own_plugin_url))
            .map(|pane| pane.id)
            .collect();
        self.peers.sort_unstable();
        self.peers.dedup();
    }

    pub(crate) fn local_toggle(
        &mut self,
        self_id: Option<u32>,
        should_be_collapsed: bool,
        current_cols: usize,
    ) -> Vec<PaneSizeEffect> {
        let Some(self_id) = self_id else {
            return Vec::new();
        };
        self.set_current_cols(current_cols);
        let target_cols = self.target_cols_for_state(should_be_collapsed);
        let mut effects = vec![PaneSizeEffect::ResizePluginPane {
            pane_id: PaneId::Plugin(self_id),
            current_cols: self.own_cols,
            target_cols,
            direction: self.resize_direction,
        }];
        self.own_cols = target_cols;
        self.revision += 1;

        let message = PaneSizeMessage {
            kind: "pane_size".into(),
            collapsed: should_be_collapsed,
            target_cols,
            revision: self.revision,
            source_plugin_id: self_id,
        };
        let Ok(payload) = serde_json::to_string(&message) else {
            return effects;
        };

        effects.extend(
            self.peers
                .iter()
                .map(|peer| PaneSizeEffect::SendControlMessage {
                    peer: *peer,
                    payload: payload.clone(),
                }),
        );
        effects
    }

    pub(crate) fn handle_pipe(
        &mut self,
        pipe_message: &PipeMessage,
        self_id: Option<u32>,
        current_cols: usize,
    ) -> Option<PaneSizeUpdate> {
        if pipe_message.name != SIZE_PIPE_NAME {
            return None;
        }
        let payload = pipe_message.payload.as_deref()?;
        let message: PaneSizeMessage = serde_json::from_str(payload).ok()?;
        if message.kind != "pane_size" || Some(message.source_plugin_id) == self_id {
            return None;
        }

        let last_seen = self
            .seen_revisions
            .entry(message.source_plugin_id)
            .or_insert(0);
        if message.revision <= *last_seen {
            return None;
        }
        *last_seen = message.revision;

        self.set_current_cols(current_cols);
        let mut effects = Vec::new();
        if let Some(self_id) = self_id {
            effects.push(PaneSizeEffect::ResizePluginPane {
                pane_id: PaneId::Plugin(self_id),
                current_cols: self.own_cols,
                target_cols: message.target_cols,
                direction: self.resize_direction,
            });
            self.own_cols = message.target_cols;
        }
        Some(PaneSizeUpdate {
            collapsed: message.collapsed,
            effects,
        })
    }

    fn target_cols_for_state(&self, should_be_collapsed: bool) -> usize {
        if should_be_collapsed {
            self.config.collapsed_cols
        } else {
            self.config.expanded_cols
        }
    }
}

fn resize_border_direction(own: &PaneInfo, panes: &[&PaneInfo]) -> Option<Direction> {
    let min_x = panes.iter().map(|pane| pane.pane_x).min()?;
    let max_right = panes
        .iter()
        .map(|pane| pane.pane_x + pane.pane_columns)
        .max()?;
    let own_right = own.pane_x + own.pane_columns;

    if own.pane_x == min_x && own_right < max_right {
        Some(Direction::Right)
    } else if own_right == max_right && own.pane_x > min_x {
        Some(Direction::Left)
    } else {
        None
    }
}

#[cfg(not(test))]
fn resize_plugin_pane_to(
    pane_id: PaneId,
    current_cols: usize,
    target_cols: usize,
    direction: Option<Direction>,
) {
    if current_cols == 0 || current_cols == target_cols {
        return;
    }
    let resize = if current_cols > target_cols {
        Resize::Decrease
    } else {
        Resize::Increase
    };
    for _ in 0..current_cols.abs_diff(target_cols) {
        resize_pane_with_id(ResizeStrategy::new(resize, direction), pane_id);
    }
}

#[cfg(test)]
fn resize_plugin_pane_to(
    _pane_id: PaneId,
    _current_cols: usize,
    _target_cols: usize,
    _direction: Option<Direction>,
) {
}
#[cfg(not(test))]
fn send_control_message(peer: u32, payload: String) {
    pipe_message_to_plugin(MessageToPlugin {
        destination_plugin_id: Some(peer),
        message_name: SIZE_PIPE_NAME.into(),
        message_payload: Some(payload),
        plugin_config: BTreeMap::new(),
        ..Default::default()
    });
}

#[cfg(test)]
fn send_control_message(_peer: u32, _payload: String) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_old_revisions_from_same_source() {
        let mut service = PaneSizeService::new(PaneSizeConfig::default());
        let payload = serde_json::to_string(&PaneSizeMessage {
            kind: "pane_size".into(),
            collapsed: true,
            target_cols: 8,
            revision: 1,
            source_plugin_id: 7,
        })
        .unwrap();
        let message = PipeMessage {
            source: PipeSource::Plugin(7),
            name: SIZE_PIPE_NAME.into(),
            payload: Some(payload),
            args: BTreeMap::new(),
            is_private: false,
        };

        assert_eq!(
            service.handle_pipe(&message, None, 0).unwrap().collapsed,
            true
        );
        assert_eq!(service.handle_pipe(&message, None, 0), None);
    }

    #[test]
    fn local_toggle_returns_resize_and_peer_messages() {
        let mut service = PaneSizeService::new(PaneSizeConfig::default());
        service.peers = vec![2, 3];

        let effects = service.local_toggle(Some(1), true, 16);

        assert_eq!(effects.len(), 3);
        assert!(matches!(
            effects[0],
            PaneSizeEffect::ResizePluginPane {
                pane_id: PaneId::Plugin(1),
                current_cols: 16,
                target_cols: 8,
                direction: None,
            }
        ));
        assert!(matches!(
            effects[1],
            PaneSizeEffect::SendControlMessage { peer: 2, .. }
        ));
        assert!(matches!(
            effects[2],
            PaneSizeEffect::SendControlMessage { peer: 3, .. }
        ));
    }

    #[test]
    fn chooses_right_border_for_left_sidebar() {
        let own = pane(1, 0, 8);
        let main = pane(2, 8, 72);

        assert_eq!(
            resize_border_direction(&own, &[&own, &main]),
            Some(Direction::Right)
        );
    }

    #[test]
    fn chooses_left_border_for_right_sidebar() {
        let main = pane(1, 0, 72);
        let own = pane(2, 72, 8);

        assert_eq!(
            resize_border_direction(&own, &[&main, &own]),
            Some(Direction::Left)
        );
    }

    #[test]
    fn chooses_border_against_terminal_neighbor() {
        let own = pane(1, 0, 8);
        let terminal = PaneInfo {
            id: 2,
            is_plugin: false,
            pane_x: 8,
            pane_columns: 72,
            ..Default::default()
        };

        assert_eq!(
            resize_border_direction(&own, &[&own, &terminal]),
            Some(Direction::Right)
        );
    }

    fn pane(id: u32, pane_x: usize, pane_columns: usize) -> PaneInfo {
        PaneInfo {
            id,
            is_plugin: true,
            pane_x,
            pane_columns,
            ..Default::default()
        }
    }
}
