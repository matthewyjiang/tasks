pub fn needs_onboarding(platform: &dyn taskmanager_core::Platform) -> bool {
    platform
        .load_key(taskmanager_core::ACCOUNT_DATA_KEY_ID)
        .is_err()
        || platform
            .load_key(taskmanager_core::DEVICE_PRIVATE_KEY_ID)
            .is_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskmanager_core::{init_account, MockPlatform};

    #[test]
    fn onboarding_needed_until_account_is_initialized() {
        let platform = MockPlatform::new();
        assert!(needs_onboarding(&platform));
        init_account(&platform).unwrap();
        assert!(!needs_onboarding(&platform));
    }
}
