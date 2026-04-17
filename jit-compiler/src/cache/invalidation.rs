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
