use crate::cache::cache_scope::CacheScope;

#[derive(Debug, Clone, Copy)]
pub enum InvalidationAction {
    None,
    SoftInvalidate,
    HardInvalidate,
    WholeBucketDrop,
}

#[derive(Debug, Clone, Copy)]
pub enum InvalidationTrigger {
    SourceChanged,
    DepChanged,
    CompileFlagsChanged,
    IrEpochChanged,
    ToolchainChanged,
    PermissionContextChanged,
}

pub fn invalidation_action(trigger: InvalidationTrigger, scope: CacheScope) -> InvalidationAction {
    match (trigger, scope) {
        (InvalidationTrigger::SourceChanged, CacheScope::Project) => InvalidationAction::HardInvalidate,
        (InvalidationTrigger::SourceChanged, CacheScope::UserExt) => InvalidationAction::HardInvalidate,
        (InvalidationTrigger::SourceChanged, CacheScope::GlobalStd) => InvalidationAction::WholeBucketDrop,
        (InvalidationTrigger::DepChanged, _) => InvalidationAction::HardInvalidate,
        (InvalidationTrigger::CompileFlagsChanged, _) => InvalidationAction::SoftInvalidate,
        (InvalidationTrigger::IrEpochChanged, _) => InvalidationAction::WholeBucketDrop,
        (InvalidationTrigger::ToolchainChanged, _) => InvalidationAction::WholeBucketDrop,
        (InvalidationTrigger::PermissionContextChanged, _) => InvalidationAction::HardInvalidate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: all scopes
    const ALL_SCOPES: [CacheScope; 3] = [
        CacheScope::GlobalStd,
        CacheScope::UserExt,
        CacheScope::Project,
    ];

    #[test]
    fn test_source_changed_project_and_user_hard_invalidate() {
        for scope in [CacheScope::Project, CacheScope::UserExt] {
            let action = invalidation_action(InvalidationTrigger::SourceChanged, scope);
            assert!(matches!(action, InvalidationAction::HardInvalidate),
                "SourceChanged for {scope:?} should be HardInvalidate, got {action:?}");
        }
    }

    #[test]
    fn test_source_changed_global_std_whole_bucket_drop() {
        let action = invalidation_action(InvalidationTrigger::SourceChanged, CacheScope::GlobalStd);
        assert!(matches!(action, InvalidationAction::WholeBucketDrop));
    }

    #[test]
    fn test_dep_changed_always_hard_invalidate() {
        for scope in ALL_SCOPES {
            let action = invalidation_action(InvalidationTrigger::DepChanged, scope);
            assert!(matches!(action, InvalidationAction::HardInvalidate),
                "DepChanged for {scope:?} should be HardInvalidate");
        }
    }

    #[test]
    fn test_compile_flags_changed_always_soft_invalidate() {
        for scope in ALL_SCOPES {
            let action = invalidation_action(InvalidationTrigger::CompileFlagsChanged, scope);
            assert!(matches!(action, InvalidationAction::SoftInvalidate),
                "CompileFlagsChanged for {scope:?} should be SoftInvalidate");
        }
    }

    #[test]
    fn test_ir_epoch_changed_always_whole_bucket_drop() {
        for scope in ALL_SCOPES {
            let action = invalidation_action(InvalidationTrigger::IrEpochChanged, scope);
            assert!(matches!(action, InvalidationAction::WholeBucketDrop),
                "IrEpochChanged for {scope:?} should be WholeBucketDrop");
        }
    }

    #[test]
    fn test_toolchain_changed_always_whole_bucket_drop() {
        for scope in ALL_SCOPES {
            let action = invalidation_action(InvalidationTrigger::ToolchainChanged, scope);
            assert!(matches!(action, InvalidationAction::WholeBucketDrop),
                "ToolchainChanged for {scope:?} should be WholeBucketDrop");
        }
    }

    #[test]
    fn test_permission_context_changed_always_hard_invalidate() {
        for scope in ALL_SCOPES {
            let action = invalidation_action(InvalidationTrigger::PermissionContextChanged, scope);
            assert!(matches!(action, InvalidationAction::HardInvalidate),
                "PermissionContextChanged for {scope:?} should be HardInvalidate");
        }
    }

    #[test]
    fn test_invalidation_action_exhaustive_coverage() {
        // Every trigger × scope combo must return a valid action
        let triggers = [
            InvalidationTrigger::SourceChanged,
            InvalidationTrigger::DepChanged,
            InvalidationTrigger::CompileFlagsChanged,
            InvalidationTrigger::IrEpochChanged,
            InvalidationTrigger::ToolchainChanged,
            InvalidationTrigger::PermissionContextChanged,
        ];
        for trigger in triggers {
            for scope in ALL_SCOPES {
                let action = invalidation_action(trigger, scope);
                // All actions are valid — just ensure no panic
                let _ = format!("{action:?}");
            }
        }
    }
}
