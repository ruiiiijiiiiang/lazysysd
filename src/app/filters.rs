use std::collections::{BTreeSet, HashSet};

use crate::{
    app::state::{App, FilterMenu, FilterMenuOption, UnitSelectionKey},
    models::UnitInfo,
};

impl FilterMenu {
    pub fn title(self) -> &'static str {
        match self {
            Self::Active => "Active State",
            Self::Enablement => "Enablement State",
            Self::Load => "Load State",
            Self::Scope => "Scope",
        }
    }

    pub fn segment_title(self, show_hotkey: bool) -> &'static str {
        match self {
            Self::Active => {
                if show_hotkey {
                    " Active (a) "
                } else {
                    " Active "
                }
            }
            Self::Enablement => {
                if show_hotkey {
                    " Enablement (n) "
                } else {
                    " Enablement "
                }
            }
            Self::Load => {
                if show_hotkey {
                    " Load (o) "
                } else {
                    " Load "
                }
            }
            Self::Scope => {
                if show_hotkey {
                    " Scope (p) "
                } else {
                    " Scope "
                }
            }
        }
    }

    pub fn unit_value(self, unit: &UnitInfo) -> &str {
        match self {
            Self::Active => &unit.active_state,
            Self::Enablement => &unit.enablement_state,
            Self::Load => &unit.load_state,
            Self::Scope => &unit.scope,
        }
    }

    pub fn selected_value(self, app: &App) -> Option<&str> {
        match self {
            Self::Active => app.active_state_filter.as_deref(),
            Self::Enablement => app.enablement_state_filter.as_deref(),
            Self::Load => app.load_state_filter.as_deref(),
            Self::Scope => app.scope_filter.as_deref(),
        }
    }

    pub fn set_selected_value(self, app: &mut App, value: Option<String>) {
        match self {
            Self::Active => app.active_state_filter = value,
            Self::Enablement => app.enablement_state_filter = value,
            Self::Load => app.load_state_filter = value,
            Self::Scope => app.scope_filter = value,
        }
    }

    pub fn preferred_order(self) -> &'static [&'static str] {
        match self {
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
            Self::Scope => &["global", "session"],
        }
    }

    pub fn preferred_hotkeys(self, value: &str) -> Vec<char> {
        match self {
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
            Self::Scope => match value {
                "global" => vec!['g'],
                "session" => vec!['s'],
                _ => Vec::new(),
            },
        }
    }
}

impl App {
    pub fn update_filter(&mut self) {
        let selected_unit_key = if self.selected_unit_key == UnitSelectionKey::default() {
            None
        } else {
            Some(self.selected_unit_key.clone())
        };

        if self.search_query.is_empty() {
            self.filtered_units = self
                .units
                .iter()
                .enumerate()
                .filter(|(_, unit)| self.unit_matches_state_filters(unit))
                .map(|(index, _)| index)
                .collect();
            self.filtered_units
                .sort_by_key(|&index| self.units[index].name.to_ascii_lowercase());
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .units
                .iter()
                .enumerate()
                .filter(|(_, unit)| self.unit_matches_state_filters(unit))
                .filter_map(|(index, unit)| self.search_score(unit).map(|score| (index, score)))
                .collect();
            scored.sort_by(|(left_index, left_score), (right_index, right_score)| {
                right_score.cmp(left_score).then_with(|| {
                    self.units[*left_index]
                        .name
                        .to_ascii_lowercase()
                        .cmp(&self.units[*right_index].name.to_ascii_lowercase())
                })
            });
            self.filtered_units = scored.into_iter().map(|(index, _)| index).collect();
        }

        self.restore_selection(selected_unit_key.as_ref());
    }

    pub fn filter_summary(&self, menu: FilterMenu) -> &str {
        menu.selected_value(self).unwrap_or("all")
    }

    pub fn filter_menu_options(&self, menu: FilterMenu) -> Vec<FilterMenuOption> {
        let values = self.comprehensive_filter_values(menu);

        let mut options = vec![FilterMenuOption {
            hotkey: 'a',
            label: "all".to_string(),
            value: None,
            selected: menu.selected_value(self).is_none(),
            count: self
                .units
                .iter()
                .filter(|u| self.unit_matches_scope_for_menu(u, menu))
                .count(),
        }];
        let mut used_hotkeys = HashSet::from(['a']);

        for label in self.sort_filter_values(menu, values) {
            let hotkey = self.assign_filter_hotkey(menu, &label, &mut used_hotkeys);
            let count = self
                .units
                .iter()
                .filter(|u| {
                    self.unit_matches_scope_for_menu(u, menu)
                        && menu.unit_value(u) == label.as_str()
                })
                .count();

            options.push(FilterMenuOption {
                hotkey,
                selected: menu.selected_value(self) == Some(label.as_str()),
                value: Some(label.clone()),
                label,
                count,
            });
        }

        options
    }

    pub fn unit_matches_state_filters(&self, unit: &UnitInfo) -> bool {
        Self::matches_filter_value(self.active_state_filter.as_deref(), &unit.active_state)
            && Self::matches_filter_value(
                self.enablement_state_filter.as_deref(),
                &unit.enablement_state,
            )
            && Self::matches_filter_value(self.load_state_filter.as_deref(), &unit.load_state)
            && Self::matches_filter_value(self.scope_filter.as_deref(), &unit.scope)
    }

    pub fn unit_matches_scope_for_menu(&self, unit: &UnitInfo, menu: FilterMenu) -> bool {
        self.unit_matches_search(unit)
            && (menu == FilterMenu::Active
                || Self::matches_filter_value(
                    self.active_state_filter.as_deref(),
                    &unit.active_state,
                ))
            && (menu == FilterMenu::Enablement
                || Self::matches_filter_value(
                    self.enablement_state_filter.as_deref(),
                    &unit.enablement_state,
                ))
            && (menu == FilterMenu::Load
                || Self::matches_filter_value(self.load_state_filter.as_deref(), &unit.load_state))
            && (menu == FilterMenu::Scope
                || Self::matches_filter_value(self.scope_filter.as_deref(), &unit.scope))
    }

    fn available_filter_values(&self, menu: FilterMenu, scoped: bool) -> BTreeSet<String> {
        self.units
            .iter()
            .filter(|unit| !scoped || self.unit_matches_scope_for_menu(unit, menu))
            .map(|unit| menu.unit_value(unit).to_string())
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
        app.units = units;
        app.is_loading = false;
        app.update_filter();
        app
    }

    fn unit(
        name: &str,
        description: &str,
        load_state: &str,
        active_state: &str,
        enablement_state: &str,
        path: &str,
    ) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: description.to_string(),
            scope: "global".to_string(),
            load_state: load_state.to_string(),
            active_state: active_state.to_string(),
            enablement_state: enablement_state.to_string(),
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
        }
    }

    fn filtered_names(app: &App) -> Vec<&str> {
        app.filtered_units
            .iter()
            .map(|&index| app.units[index].name.as_str())
            .collect()
    }

    #[test]
    fn update_filter_combines_search_and_state_filters() {
        let mut app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                "loaded",
                "active",
                "enabled",
                "/test/unit/ssh",
            ),
            unit(
                "broken.service",
                "Broken worker",
                "loaded",
                "failed",
                "static",
                "/test/unit/broken",
            ),
            unit(
                "db.service",
                "Database",
                "loaded",
                "failed",
                "disabled",
                "/test/unit/db",
            ),
        ]);

        app.active_state_filter = Some("failed".to_string());
        app.enablement_state_filter = Some("static".to_string());
        app.search_query = "broken".to_string();
        app.update_filter();

        assert_eq!(filtered_names(&app), vec!["broken.service"]);
    }

    #[test]
    fn update_filter_sorts_units_by_name() {
        let app = test_app(vec![
            unit(
                "Zebra.service",
                "Zebra",
                "loaded",
                "active",
                "enabled",
                "/test/unit/zebra_upper",
            ),
            unit(
                "zeta.service",
                "Zeta",
                "loaded",
                "active",
                "enabled",
                "/test/unit/zeta",
            ),
            unit(
                "Alpha.service",
                "Alpha",
                "loaded",
                "active",
                "enabled",
                "/test/unit/alpha_upper",
            ),
            unit(
                "beta.service",
                "Beta",
                "loaded",
                "active",
                "enabled",
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
                "loaded",
                "active",
                "enabled",
                "/test/unit/zeta",
            ),
            unit(
                "alpha.service",
                "Alpha",
                "loaded",
                "active",
                "enabled",
                "/test/unit/alpha",
            ),
            unit(
                "beta.service",
                "Beta",
                "loaded",
                "active",
                "enabled",
                "/test/unit/beta",
            ),
        ]);

        app.select_filtered_unit_index(Some(2));
        assert_eq!(app.selected_unit_key.name, "zeta.service");

        app.search_query = "zeta".to_string();
        app.update_filter();

        assert_eq!(filtered_names(&app), vec!["zeta.service"]);
        assert_eq!(app.selected_unit_key.name, "zeta.service");
        assert_eq!(app.selected_unit_index(), Some(0));
    }

    #[test]
    fn update_filter_uses_stored_unit_name_when_list_state_is_stale() {
        let mut app = test_app(vec![
            unit(
                "alpha.service",
                "Alpha",
                "loaded",
                "active",
                "enabled",
                "/test/unit/alpha",
            ),
            unit(
                "beta.service",
                "Beta",
                "loaded",
                "active",
                "enabled",
                "/test/unit/beta",
            ),
            unit(
                "gamma.service",
                "Gamma",
                "loaded",
                "active",
                "enabled",
                "/test/unit/gamma",
            ),
        ]);

        app.selected_unit_key = UnitSelectionKey {
            name: "gamma.service".to_string(),
            scope: "global".to_string(),
            path: "/test/unit/gamma".to_string(),
        };
        app.list_state.select(Some(0));

        app.update_filter();

        assert_eq!(app.selected_unit_key.name, "gamma.service");
        assert_eq!(app.selected_unit_index(), Some(2));
    }

    #[test]
    fn update_filter_restores_duplicate_units_by_composite_key() {
        let mut app = test_app(vec![
            unit(
                "dup.service",
                "First",
                "loaded",
                "active",
                "enabled",
                "/test/unit/dup_a",
            ),
            unit(
                "dup.service",
                "Second",
                "loaded",
                "active",
                "enabled",
                "/test/unit/dup_b",
            ),
            unit(
                "other.service",
                "Other",
                "loaded",
                "active",
                "enabled",
                "/test/unit/other",
            ),
        ]);

        app.select_filtered_unit_index(Some(1));
        assert_eq!(app.selected_unit_key.path, "/test/unit/dup_b");

        app.update_filter();

        assert_eq!(app.selected_unit_key.path, "/test/unit/dup_b");
        assert_eq!(
            app.get_selected_unit().map(|unit| unit.path.to_string()),
            Some("/test/unit/dup_b".to_string())
        );
    }

    #[test]
    fn filter_menu_options_include_all_and_expected_hotkeys() {
        let app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                "loaded",
                "active",
                "enabled",
                "/test/unit/ssh",
            ),
            unit(
                "broken.service",
                "Broken worker",
                "masked",
                "inactive",
                "static",
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
            "loaded",
            "active",
            "enabled",
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
}
