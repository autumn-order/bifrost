//! Tests for EntityRefreshTracker.
//!
//! This module verifies the entity refresh tracker behavior, including finding
//! entries that need updating based on cache expiration, scheduling jobs with
//! staggered execution times, batch limiting, and handling empty tables.

use bifrost::server::scheduler::{
    config::eve::alliance as alliance_config,
    entity_refresh::{EntityRefreshTracker, SchedulableEntity},
};
use bifrost_test_utils::prelude::*;
use chrono::{Duration, Utc};
use entity::prelude::EveAlliance;
use migration::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::util::redis::RedisTest;
use crate::worker::queue::setup_test_queue;

pub struct AllianceInfo;

impl SchedulableEntity for AllianceInfo {
    type Entity = entity::eve_alliance::Entity;

    fn updated_at_column() -> impl ColumnTrait + sea_orm::IntoSimpleExpr {
        entity::eve_alliance::Column::UpdatedAt
    }

    fn id_column() -> impl ColumnTrait + sea_orm::IntoSimpleExpr {
        entity::eve_alliance::Column::AllianceId
    }
}

mod find_entries_needing_update;
mod schedule_jobs;
