//! Re-export of Rig configuration types from `ragent_config`.
//!
//! The canonical definitions live in `ragent_config::config` so that
//! `ragent-config` does not depend on `ragent-rig` (which would create a
//! dependency cycle: ragent-config → ragent-rig → ragent-agent →
//! ragent-config).  This module exists to keep existing `ragent_rig::config`
//! imports working.

pub use ragent_config::config::{
    RigConfig, RigEmbeddingsConfig, RigMemoryConfig, RigProviderConfig, RigVectorStoreConfig,
};
