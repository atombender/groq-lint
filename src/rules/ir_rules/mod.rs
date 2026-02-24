//! Lint rule implementations.
//!
//! Each rule operates on the semantic IR graph.

mod computed_value_in_filter;
mod count_in_correlated_subquery;
mod join_in_filter;
mod join_to_get_id;
mod many_joins;
mod match_on_id;
mod non_literal_comparison;
mod order_on_expr;
mod pagination;
mod query_size;
mod repeated_dereference;

pub use computed_value_in_filter::ComputedValueInFilter;
pub use count_in_correlated_subquery::CountInCorrelatedSubquery;
pub use join_in_filter::JoinInFilter;
pub use join_to_get_id::JoinToGetId;
pub use many_joins::ManyJoins;
pub use match_on_id::MatchOnId;
pub use non_literal_comparison::NonLiteralComparison;
pub use order_on_expr::OrderOnExpr;
pub use pagination::{DeepPagination, DeepPaginationParam, LargePages};
pub use query_size::{ExtremelyLargeQuery, VeryLargeQuery};
pub use repeated_dereference::RepeatedDereference;
