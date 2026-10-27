pub(super) fn should_dispatch(first_review: bool, auto_open: bool, notifications: bool) -> bool {
    first_review && (auto_open || notifications)
}
pub(super) fn launch_allowed(
    enabled: bool,
    auto_open: bool,
    notifications: bool,
    authorized: bool,
) -> bool {
    enabled && authorized && (auto_open || notifications)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_reviews_are_not_lost_in_a_burst() {
        for _ in 0..20 {
            assert!(should_dispatch(true, true, false));
        }
    }
    #[test]
    fn duplicate_reviews_are_silent_and_notifications_work_without_auto_focus() {
        assert!(!should_dispatch(false, true, true));
        assert!(should_dispatch(true, false, true));
        assert!(should_dispatch(true, true, false));
        assert!(!should_dispatch(true, false, false));
    }
    #[test]
    fn background_launch_requires_an_active_authorized_connection_and_an_alert_option() {
        assert!(launch_allowed(true, false, true, true));
        assert!(launch_allowed(true, true, false, true));
        assert!(!launch_allowed(true, false, false, true));
        assert!(!launch_allowed(false, true, true, true));
        assert!(!launch_allowed(true, true, true, false));
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::super::{Journal, Native};
    use serde_json::json;
    #[test]
    fn existing_profiles_default_to_notifications_and_the_toggle_is_a_native_setting() {
        let journal: Journal = serde_json::from_value(json!({"schemaVersion":3,"enabled":true,"clients":[],"plans":[]})).unwrap();
        assert!(journal.notifications);
        assert!(journal.auto_open);
        assert!(matches!(serde_json::from_value::<Native>(json!({"op":"notifications","enabled":false})).unwrap(), Native::Notifications {enabled:false}));
    }
}
