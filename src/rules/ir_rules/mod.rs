//! IR-based lint rules.
//!
//! These rules operate on the semantic IR graph rather than the raw AST,
//! making them easier to write and more maintainable.

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

pub use computed_value_in_filter::IrComputedValueInFilter;
pub use count_in_correlated_subquery::IrCountInCorrelatedSubquery;
pub use join_in_filter::IrJoinInFilter;
pub use join_to_get_id::IrJoinToGetId;
pub use many_joins::IrManyJoins;
pub use match_on_id::IrMatchOnId;
pub use non_literal_comparison::IrNonLiteralComparison;
pub use order_on_expr::IrOrderOnExpr;
pub use pagination::{IrDeepPagination, IrDeepPaginationParam, IrLargePages};
pub use query_size::{IrExtremelyLargeQuery, IrVeryLargeQuery};
pub use repeated_dereference::IrRepeatedDereference;
