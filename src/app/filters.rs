use std::collections::{BTreeSet, HashSet};

use crate::{
    app::state::context::{App, FilterMenu, FilterMenuOption, UnitSelectionKey},
    models::{UnitActiveState, UnitEnablementState, UnitInfo, UnitLoadState, UnitScope, UnitType},
};

impl FilterMenu {
    pub fn unit_value(self, unit: &UnitInfo) -> String {
        match self {
            Self::Type => UnitType::from_unit_name(&unit.name).as_str().to_string(),
            Self::Scope => unit.scope.as_str().to_string(),
            Self::Active => unit.active_state.as_str().to_string(),
            Self::Enablement => unit.enablement_state.as_str().to_string(),
            Self::Load => unit.load_state.as_str().to_string(),
        }
    }

    pub fn selected_value(self, app: &App) -> Option<String> {
        match self {
            Self::Type => app.unit_list.type_filter.clone(),
            Self::Scope => app
                .unit_list
                .scope_filter
                .map(|value| value.as_str().to_string()),
            Self::Active => app
                .unit_list
                .active_filter
                .map(|value| value.as_str().to_string()),
            Self::Enablement => app
                .unit_list
                .enablement_filter
                .map(|value| value.as_str().to_string()),
            Self::Load => app
                .unit_list
                .load_filter
                .map(|value| value.as_str().to_string()),
        }
    }

    pub fn set_selected_value(self, app: &mut App, value: Option<String>) {
        match self {
            Self::Type => app.unit_list.type_filter = value,
            Self::Scope => {
                app.unit_list.scope_filter = value.and_then(|value| value.parse::<UnitScope>().ok())
            }
            Self::Active => {
                app.unit_list.active_filter =
                    value.and_then(|value| value.parse::<UnitActiveState>().ok())
            }
            Self::Enablement => {
                app.unit_list.enablement_filter =
                    value.and_then(|value| value.parse::<UnitEnablementState>().ok())
            }
            Self::Load => {
                app.unit_list.load_filter =
                    value.and_then(|value| value.parse::<UnitLoadState>().ok())
            }
        }
    }

    pub fn preferred_order(self) -> &'static [&'static str] {
        match self {
            Self::Type => &[
                "service",
                "socket",
                "target",
                "device",
                "mount",
                "automount",
                "timer",
                "path",
                "slice",
                "scope",
                "swap",
            ],
            Self::Scope => &["global", "session"],
            Self::Active => &[
                "active",
                "inactive",
                "failed",
                "activating",
                "deactivating",
                "maintenance",
                "reloading",
            ],
            Self::Enablement => &[
                "enabled",
                "enabled-runtime",
                "linked",
                "linked-runtime",
                "masked",
                "masked-runtime",
                "static",
                "disabled",
                "invalid",
                "indirect",
                "alias",
                "generated",
                "transient",
                "unknown",
            ],
            Self::Load => &["loaded", "not-found", "bad-setting", "error", "masked"],
        }
    }

    pub fn preferred_hotkeys(self, value: &str) -> Vec<char> {
        match self {
            Self::Type => match value {
                "service" => vec!['s'],
                "socket" => vec!['o'],
                "target" => vec!['t'],
                "device" => vec!['d'],
                "mount" => vec!['m'],
                "automount" => vec!['u'],
                "timer" => vec!['i'],
                "path" => vec!['p'],
                "slice" => vec!['l'],
                "scope" => vec!['c'],
                "swap" => vec!['w'],
                _ => Vec::new(),
            },
            Self::Scope => match value {
                "global" => vec!['g'],
                "session" => vec!['s'],
                _ => Vec::new(),
            },
            Self::Active => match value {
                "active" => vec!['t', 'v', 'c'],
                "inactive" => vec!['i'],
                "failed" => vec!['f'],
                "activating" => vec!['g'],
                "deactivating" => vec!['d'],
                "maintenance" => vec!['m'],
                "reloading" => vec!['r'],
                "unknown" => vec!['u'],
                _ => Vec::new(),
            },
            Self::Enablement => match value {
                "enabled" => vec!['e'],
                "disabled" => vec!['d'],
                "static" => vec!['s'],
                "masked" => vec!['m'],
                "indirect" => vec!['i'],
                "alias" => vec!['l'],
                "generated" => vec!['g'],
                "linked" => vec!['k'],
                "enabled-runtime" => vec!['r'],
                "disabled-runtime" => vec!['u'],
                "masked-runtime" => vec!['x'],
                "linked-runtime" => vec!['y'],
                "transient" => vec!['t'],
                "unknown" => vec!['w'],
                _ => Vec::new(),
            },
            Self::Load => match value {
                "loaded" => vec!['l'],
                "not-found" => vec!['n'],
                "masked" => vec!['m'],
                "error" => vec!['e'],
                "bad-setting" => vec!['b'],
                "merged" => vec!['g'],
                "stub" => vec!['s'],
                "unknown" => vec!['u'],
                _ => Vec::new(),
            },
        }
    }
}

impl App {
    pub fn reset_unit_filters(&mut self) {
        self.search.query.clear();
        self.unit_list.type_filter = None;
        self.unit_list.active_filter = None;
        self.unit_list.enablement_filter = None;
        self.unit_list.load_filter = None;
        self.unit_list.scope_filter = None;
        self.search.is_active = false;
        self.unit_list.open_filter_menu = None;
        self.search.cursor = 0;
        self.update_filter();
    }

    pub fn update_filter(&mut self) {
        let selected_unit_key = if self.unit_list.selected_key == UnitSelectionKey::default() {
            None
        } else {
            Some(self.unit_list.selected_key.clone())
        };

        if self.search.query.is_empty() {
            self.unit_list.filtered_indices = self
                .unit_list
                .units
                .iter()
                .enumerate()
                .filter(|(_, unit)| self.unit_matches_state_filters(unit))
                .map(|(index, _)| index)
                .collect();
            self.unit_list
                .filtered_indices
                .sort_by_key(|&index| self.unit_list.units[index].name.to_ascii_lowercase());
        } else {
            let mut scored: Vec<(usize, u32)> = self
                .unit_list
                .units
                .iter()
                .enumerate()
                .filter(|(_, unit)| self.unit_matches_state_filters(unit))
                .filter_map(|(index, unit)| self.search_score(unit).map(|score| (index, score)))
                .collect();
            scored.sort_by(|(left_index, left_score), (right_index, right_score)| {
                right_score.cmp(left_score).then_with(|| {
                    self.unit_list.units[*left_index]
                        .name
                        .to_ascii_lowercase()
                        .cmp(&self.unit_list.units[*right_index].name.to_ascii_lowercase())
                })
            });
            self.unit_list.filtered_indices = scored.into_iter().map(|(index, _)| index).collect();
        }

        self.restore_selection(selected_unit_key.as_ref());
    }

    pub fn filter_summary(&self, menu: FilterMenu) -> String {
        menu.selected_value(self)
            .unwrap_or_else(|| "all".to_string())
    }

    pub fn filter_menu_options(&self, menu: FilterMenu) -> Vec<FilterMenuOption> {
        let values = self.comprehensive_filter_values(menu);

        let mut options = vec![FilterMenuOption {
            hotkey: 'a',
            label: "all".to_string(),
            value: None,
            selected: menu.selected_value(self).is_none(),
            count: self
                .unit_list
                .units
                .iter()
                .filter(|u| self.unit_matches_scope_for_menu(u, menu))
                .count(),
        }];
        let mut used_hotkeys = HashSet::from(['a']);

        for label in self.sort_filter_values(menu, values) {
            let hotkey = self.assign_filter_hotkey(menu, &label, &mut used_hotkeys);
            let count = self
                .unit_list
                .units
                .iter()
                .filter(|u| {
                    self.unit_matches_scope_for_menu(u, menu)
                        && menu.unit_value(u) == label.as_str()
                })
                .count();

            options.push(FilterMenuOption {
                hotkey,
                selected: menu.selected_value(self).as_deref() == Some(label.as_str()),
                value: Some(label.clone()),
                label,
                count,
            });
        }

        options
    }

    pub fn unit_matches_state_filters(&self, unit: &UnitInfo) -> bool {
        Self::matches_filter_value(
            self.unit_list.type_filter.as_deref(),
            UnitType::from_unit_name(&unit.name).as_str(),
        ) && (self.unit_list.scope_filter.is_none()
            || self.unit_list.scope_filter == Some(unit.scope))
            && (self.unit_list.active_filter.is_none()
                || self.unit_list.active_filter == Some(unit.active_state))
            && (self.unit_list.enablement_filter.is_none()
                || self.unit_list.enablement_filter == Some(unit.enablement_state))
            && (self.unit_list.load_filter.is_none()
                || self.unit_list.load_filter == Some(unit.load_state))
    }

    pub fn unit_matches_scope_for_menu(&self, unit: &UnitInfo, menu: FilterMenu) -> bool {
        self.unit_matches_search(unit)
            && (menu == FilterMenu::Type
                || Self::matches_filter_value(
                    self.unit_list.type_filter.as_deref(),
                    UnitType::from_unit_name(&unit.name).as_str(),
                ))
            && (menu == FilterMenu::Scope
                || self.unit_list.scope_filter.is_none()
                || self.unit_list.scope_filter == Some(unit.scope))
            && (menu == FilterMenu::Active
                || self.unit_list.active_filter.is_none()
                || self.unit_list.active_filter == Some(unit.active_state))
            && (menu == FilterMenu::Enablement
                || self.unit_list.enablement_filter.is_none()
                || self.unit_list.enablement_filter == Some(unit.enablement_state))
            && (menu == FilterMenu::Load
                || self.unit_list.load_filter.is_none()
                || self.unit_list.load_filter == Some(unit.load_state))
    }

    fn available_filter_values(&self, menu: FilterMenu, scoped: bool) -> BTreeSet<String> {
        self.unit_list
            .units
            .iter()
            .filter(|unit| !scoped || self.unit_matches_scope_for_menu(unit, menu))
            .map(|unit| menu.unit_value(unit))
            .filter(|value| !value.is_empty())
            .collect()
    }

    fn comprehensive_filter_values(&self, menu: FilterMenu) -> BTreeSet<String> {
        let mut values = self.available_filter_values(menu, false);
        values.extend(
            menu.preferred_order()
                .iter()
                .map(|value| (*value).to_string()),
        );
        values
    }

    fn sort_filter_values(&self, menu: FilterMenu, values: BTreeSet<String>) -> Vec<String> {
        let mut remaining: Vec<String> = values.into_iter().collect();
        let mut ordered = Vec::with_capacity(remaining.len());

        for preferred in menu.preferred_order() {
            if let Some(index) = remaining.iter().position(|value| value == preferred) {
                ordered.push(remaining.remove(index));
            }
        }

        ordered.extend(remaining);
        ordered
    }

    fn assign_filter_hotkey(
        &self,
        menu: FilterMenu,
        label: &str,
        used_hotkeys: &mut HashSet<char>,
    ) -> char {
        let fallbacks = label
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase());

        for candidate in menu
            .preferred_hotkeys(label)
            .into_iter()
            .chain(fallbacks)
            .chain('0'..='9')
        {
            let normalized = candidate.to_ascii_lowercase();
            if used_hotkeys.insert(normalized) {
                return normalized;
            }
        }

        '?'
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::UnitInfo;

    fn test_app(units: Vec<UnitInfo>) -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.unit_list.units = units;
        app.is_loading = false;
        app.update_filter();
        app
    }

    fn unit(
        name: &str,
        description: &str,
        load_state: UnitLoadState,
        active_state: UnitActiveState,
        enablement_state: UnitEnablementState,
        path: &str,
    ) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: description.to_string(),
            scope: UnitScope::Global,
            load_state,
            active_state,
            enablement_state,
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
            fragment_path: format!("/etc/systemd/system/{name}"),
        }
    }

    fn filtered_names(app: &App) -> Vec<&str> {
        app.unit_list
            .filtered_indices
            .iter()
            .map(|&index| app.unit_list.units[index].name.as_str())
            .collect()
    }

    fn unit_with_scope(
        name: &str,
        scope: UnitScope,
        load_state: UnitLoadState,
        active_state: UnitActiveState,
        enablement_state: UnitEnablementState,
        path: &str,
    ) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: name.to_string(),
            scope,
            load_state,
            active_state,
            enablement_state,
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
            fragment_path: format!("/etc/systemd/system/{name}"),
        }
    }

    #[test]
    fn update_filter_combines_search_and_state_filters() {
        let mut app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/ssh",
            ),
            unit(
                "broken.service",
                "Broken worker",
                UnitLoadState::Loaded,
                UnitActiveState::Failed,
                UnitEnablementState::Static,
                "/test/unit/broken",
            ),
            unit(
                "db.service",
                "Database",
                UnitLoadState::Loaded,
                UnitActiveState::Failed,
                UnitEnablementState::Disabled,
                "/test/unit/db",
            ),
        ]);

        app.unit_list.active_filter = Some(UnitActiveState::Failed);
        app.unit_list.enablement_filter = Some(UnitEnablementState::Static);
        app.search.query = "broken".to_string();
        app.update_filter();

        assert_eq!(filtered_names(&app), vec!["broken.service"]);
    }

    #[test]
    fn update_filter_sorts_units_by_name() {
        let app = test_app(vec![
            unit(
                "Zebra.service",
                "Zebra",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/zebra_upper",
            ),
            unit(
                "zeta.service",
                "Zeta",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/zeta",
            ),
            unit(
                "Alpha.service",
                "Alpha",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/alpha_upper",
            ),
            unit(
                "beta.service",
                "Beta",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/beta",
            ),
        ]);

        assert_eq!(
            filtered_names(&app),
            vec![
                "Alpha.service",
                "beta.service",
                "Zebra.service",
                "zeta.service"
            ]
        );
    }

    #[test]
    fn update_filter_restores_selection_by_unit_name_after_resort() {
        let mut app = test_app(vec![
            unit(
                "zeta.service",
                "Zeta",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/zeta",
            ),
            unit(
                "alpha.service",
                "Alpha",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/alpha",
            ),
            unit(
                "beta.service",
                "Beta",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/beta",
            ),
        ]);

        app.unit_list.select_index(Some(2));
        assert_eq!(app.unit_list.selected_key.name, "zeta.service");

        app.search.query = "zeta".to_string();
        app.update_filter();

        assert_eq!(filtered_names(&app), vec!["zeta.service"]);
        assert_eq!(app.unit_list.selected_key.name, "zeta.service");
        assert_eq!(app.selected_unit_index(), Some(0));
    }

    #[test]
    fn update_filter_uses_stored_unit_name_when_list_state_is_stale() {
        let mut app = test_app(vec![
            unit(
                "alpha.service",
                "Alpha",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/alpha",
            ),
            unit(
                "beta.service",
                "Beta",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/beta",
            ),
            unit(
                "gamma.service",
                "Gamma",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/gamma",
            ),
        ]);

        app.unit_list.selected_key = UnitSelectionKey {
            name: "gamma.service".to_string(),
            scope: UnitScope::Global,
            path: "/test/unit/gamma".to_string(),
        };
        app.unit_list.state.select(Some(0));

        app.update_filter();

        assert_eq!(app.unit_list.selected_key.name, "gamma.service");
        assert_eq!(app.selected_unit_index(), Some(2));
    }

    #[test]
    fn update_filter_restores_duplicate_units_by_composite_key() {
        let mut app = test_app(vec![
            unit(
                "dup.service",
                "First",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/dup_a",
            ),
            unit(
                "dup.service",
                "Second",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/dup_b",
            ),
            unit(
                "other.service",
                "Other",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/other",
            ),
        ]);

        app.unit_list.select_index(Some(1));
        assert_eq!(app.unit_list.selected_key.path, "/test/unit/dup_b");

        app.update_filter();

        assert_eq!(app.unit_list.selected_key.path, "/test/unit/dup_b");
        assert_eq!(
            app.get_selected_unit().map(|unit| unit.path.to_string()),
            Some("/test/unit/dup_b".to_string())
        );
    }

    #[test]
    fn reset_unit_filters_clears_state_filters_and_search_phrase() {
        let mut app = test_app(vec![
            unit(
                "zeta.service",
                "Zeta",
                UnitLoadState::Loaded,
                UnitActiveState::Failed,
                UnitEnablementState::Masked,
                "/test/unit/zeta",
            ),
            unit(
                "alpha.service",
                "Alpha",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/alpha",
            ),
        ]);

        app.unit_list.active_filter = Some(UnitActiveState::Failed);
        app.unit_list.type_filter = Some("service".to_string());
        app.unit_list.enablement_filter = Some(UnitEnablementState::Masked);
        app.unit_list.load_filter = Some(UnitLoadState::Loaded);
        app.unit_list.scope_filter = Some(UnitScope::Global);
        app.search.query = "zeta".to_string();
        app.search.is_active = true;
        app.unit_list.open_filter_menu = Some(FilterMenu::Active);

        app.reset_unit_filters();

        assert!(app.search.query.is_empty());
        assert!(app.unit_list.type_filter.is_none());
        assert!(app.unit_list.active_filter.is_none());
        assert!(app.unit_list.enablement_filter.is_none());
        assert!(app.unit_list.load_filter.is_none());
        assert!(app.unit_list.scope_filter.is_none());
        assert!(!app.search.is_active);
        assert!(app.unit_list.open_filter_menu.is_none());
        assert_eq!(filtered_names(&app), vec!["alpha.service", "zeta.service"]);
    }

    #[test]
    fn type_filter_menu_includes_common_unit_types_and_filters_units() {
        let mut app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/ssh",
            ),
            unit(
                "ssh.socket",
                "Secure Shell socket",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/ssh_socket",
            ),
        ]);

        let options = app.filter_menu_options(FilterMenu::Type);

        assert_eq!(options[0].label, "all");
        assert!(
            options
                .iter()
                .any(|option| option.label == "service" && option.hotkey == 's')
        );
        assert!(
            options
                .iter()
                .any(|option| option.label == "socket" && option.hotkey == 'o')
        );
        assert!(options.iter().any(|option| option.label == "target"));

        app.unit_list.type_filter = Some("socket".to_string());
        app.update_filter();

        assert_eq!(filtered_names(&app), vec!["ssh.socket"]);
    }

    #[test]
    fn filter_menu_options_include_all_and_expected_hotkeys() {
        let app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/ssh",
            ),
            unit(
                "broken.service",
                "Broken worker",
                UnitLoadState::Masked,
                UnitActiveState::Inactive,
                UnitEnablementState::Static,
                "/test/unit/broken",
            ),
        ]);

        let options = app.filter_menu_options(FilterMenu::Active);

        assert_eq!(options[0].label, "all");
        assert!(options[0].selected);
        assert!(
            options
                .iter()
                .any(|option| option.label == "inactive" && option.hotkey == 'i')
        );
        assert!(options.iter().any(|option| option.label == "active"));
    }

    #[test]
    fn active_and_load_filters_include_documented_states_even_when_absent() {
        let app = test_app(vec![unit(
            "ssh.service",
            "Secure Shell",
            UnitLoadState::Loaded,
            UnitActiveState::Active,
            UnitEnablementState::Enabled,
            "/test/unit/ssh",
        )]);

        let active_options = app.filter_menu_options(FilterMenu::Active);
        let load_options = app.filter_menu_options(FilterMenu::Load);

        assert!(active_options.iter().any(|option| option.label == "failed"));
        assert!(
            active_options
                .iter()
                .any(|option| option.label == "reloading")
        );
        assert!(
            load_options
                .iter()
                .any(|option| option.label == "not-found")
        );
        assert!(
            load_options
                .iter()
                .any(|option| option.label == "bad-setting")
        );
    }

    #[test]
    fn unit_value_and_scope_filters_treat_type_and_scope_independently() {
        let mut app = test_app(vec![
            unit_with_scope(
                "ssh.service",
                UnitScope::Global,
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/ssh",
            ),
            unit_with_scope(
                "ssh.socket",
                UnitScope::Session,
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/ssh_socket",
            ),
        ]);

        assert_eq!(
            FilterMenu::Type.unit_value(&app.unit_list.units[0]),
            "service"
        );
        assert_eq!(
            FilterMenu::Type.unit_value(&app.unit_list.units[1]),
            "socket"
        );
        assert_eq!(
            FilterMenu::Scope.unit_value(&app.unit_list.units[1]),
            "session"
        );

        app.unit_list.type_filter = Some("service".to_string());
        app.unit_list.scope_filter = Some(UnitScope::Session);
        app.update_filter();

        assert!(filtered_names(&app).is_empty());
    }

    #[test]
    fn unit_matches_state_filters_rejects_single_mismatch() {
        let mut app = test_app(vec![unit(
            "ssh.service",
            "Secure Shell",
            UnitLoadState::Loaded,
            UnitActiveState::Active,
            UnitEnablementState::Enabled,
            "/test/unit/ssh",
        )]);

        let unit = &app.unit_list.units[0];

        app.unit_list.type_filter = Some("socket".to_string());
        assert!(!app.unit_matches_state_filters(unit));

        app.unit_list.type_filter = Some("service".to_string());
        app.unit_list.active_filter = Some(UnitActiveState::Failed);
        assert!(!app.unit_matches_state_filters(unit));

        app.unit_list.active_filter = Some(UnitActiveState::Active);
        assert!(app.unit_matches_state_filters(unit));
    }
}
