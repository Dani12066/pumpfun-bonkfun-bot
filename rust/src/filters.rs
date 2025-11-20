use std::time::Duration;

use crate::{config::Config, events::TokenEvent, state::SniperState};

#[derive(Debug)]
pub enum FilterDecision {
    Allowed,
    Blacklisted,
    NotWhitelisted,
    RateLimited,
    Duplicate,
}

pub fn apply_filters(event: &TokenEvent, config: &Config, state: &SniperState) -> FilterDecision {
    if state.seen_mints.contains(&event.mint) {
        return FilterDecision::Duplicate;
    }

    if state.filters.is_blacklisted(&event.developer) {
        return FilterDecision::Blacklisted;
    }

    if !state.filters.is_whitelisted(&event.developer) {
        return FilterDecision::NotWhitelisted;
    }

    let max_per_minute = config.dev_filters.dev_max_tokens_per_min.unwrap_or(10);
    let allowed =
        state
            .rate_limiter
            .is_allowed(&event.developer, max_per_minute, Duration::from_secs(60));
    if !allowed {
        return FilterDecision::RateLimited;
    }

    FilterDecision::Allowed
}
