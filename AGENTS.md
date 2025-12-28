# Instructions for LLM Agents: Generating GROQ Lint Rules

This document provides instructions for generating Rust code for new GROQ lint rules based on the definitions in `rules.yaml`.

## Goal

To implement a new lint rule, create a new Rust module in `src/rules/ir_rules/` that implements the `IrRule` trait and register it in `src/lib.rs`.

## Architecture Overview

Rules operate on a semantic IR (Intermediate Representation) graph rather than the raw AST. The IR provides:
- Structural scope tracking (filter, projection, function args)
- Query helpers like `graph.joins()`, `graph.in_filter(node)`
- Simplified node types (~12 vs 35+ AST variants)

Rules return `Hit` objects containing only the span and optional detail. The linter fills in the rest (rule_id, severity, message) from `rules.yaml` metadata.

## Workflow

1. **Read `rules.yaml`:** Identify the rule to implement. Note its `id`, `advice`, `severity`, and `context`.
2. **Create Rule File:** Create `src/rules/ir_rules/<rule_id>.rs`.
3. **Implement IrRule Trait:** Return `Vec<Hit>` from the `check` method.
4. **Register Rule:** Add the module to `src/rules/ir_rules/mod.rs` and register in `src/lib.rs`.

## Implementation Template

```rust
use crate::ir::IrGraph;
use crate::rules::{Hit, IrRule};

pub struct MyNewRule;

impl IrRule for MyNewRule {
    fn id(&self) -> &'static str {
        "my_new_rule" // Must match ID in rules.yaml
    }

    fn name(&self) -> &'static str {
        "My New Rule"
    }

    fn description(&self) -> &'static str {
        "Fallback description if rules.yaml advice is missing."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        // Return hits for each violation found
        graph
            .joins()
            .filter(|node| graph.in_filter(node))
            .map(|node| Hit::at(node.span))
            .collect()
    }
}
```

## Hit API

Rules return `Hit` objects instead of full `Finding` structs:

```rust
// Node-scoped hit (most common)
Hit::at(node.span)

// Global-scoped hit (for whole-query issues)
Hit::global(node.span)

// Add extra detail to append to the advice message
Hit::at(node.span).with_detail("Found 11 joins.")
Hit::global(graph.root().span).with_detail(format!("Query size: {} bytes.", size))
```

The linter automatically fills in:
- `rule_id` from `self.id()`
- `severity` from `rules.yaml` (falls back to Medium)
- `message` from `rules.yaml` advice + optional detail

## IR Graph API

### Node Iterators

```rust
graph.nodes()           // All nodes
graph.joins()           // NodeKind::Join nodes (-> operator)
graph.filters()         // NodeKind::Filter nodes
graph.projections()     // NodeKind::Projection nodes
graph.binary_ops()      // NodeKind::Binary nodes
graph.function_calls()  // NodeKind::FunctionCall nodes
```

### Scope Queries

```rust
graph.in_filter(node)      // Is this node inside a filter constraint?
graph.in_projection(node)  // Is this node inside a projection?
graph.ancestors(node)      // Iterator over parent nodes
graph.descendants(node_id) // Iterator over child nodes
```

### Node Access

```rust
graph.node(node_id)  // Get node by ID
graph.root()         // Get root node
graph.query_len      // Original query length in bytes
```

## NodeKind Variants

The IR collapses 35+ AST variants into ~12 semantic types:

```rust
NodeKind::Literal(LiteralValue)     // Strings, ints, floats, bools, null
NodeKind::Param { name }            // $param references
NodeKind::This                      // @ (current scope)
NodeKind::Parent { depth }          // ^ or ^.^.^ (parent scope)
NodeKind::Access { base, attribute } // field.access
NodeKind::Join { base }             // -> dereference
NodeKind::Filter { base, predicate, inner_scope }
NodeKind::Projection { base, fields }
NodeKind::Slice { base, start, end, inclusive }
NodeKind::Range { start, end, inclusive }
NodeKind::Binary { op, lhs, rhs }   // Operators (+, ==, &&, etc.)
NodeKind::FunctionCall { namespace, name, args, pipe_arg }
```

### BinaryOp Helpers

```rust
op.is_comparison()  // ==, !=, <, >, <=, >=, in, match
op.is_arithmetic()  // +, -, *, /, %, **
op.is_logical()     // &&, ||
```

## Example: Simple Rule

Detect joins inside filters:

```rust
use crate::ir::IrGraph;
use crate::rules::{Hit, IrRule};

pub struct IrJoinInFilter;

impl IrRule for IrJoinInFilter {
    fn id(&self) -> &'static str { "join_in_filter" }
    fn name(&self) -> &'static str { "Join in Filter" }
    fn description(&self) -> &'static str { "Avoid -> inside filters." }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .joins()
            .filter(|join| graph.in_filter(join))
            .map(|join| Hit::at(join.span))
            .collect()
    }
}
```

## Example: Rule with Pattern Matching

Detect `_id match "*pattern*"`:

```rust
use crate::ir::{BinaryOp, LiteralValue, NodeKind, IrGraph};
use crate::rules::{Hit, IrRule};

pub struct IrMatchOnId;

impl IrRule for IrMatchOnId {
    fn id(&self) -> &'static str { "match_on_id" }
    fn name(&self) -> &'static str { "Match on ID" }
    fn description(&self) -> &'static str { "match may not work as expected on _id." }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .binary_ops()
            .filter_map(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    if *op == BinaryOp::Match {
                        let lhs_node = graph.node(*lhs);
                        if matches!(&lhs_node.kind, NodeKind::Access { attribute, .. } if attribute == "_id") {
                            let rhs_node = graph.node(*rhs);
                            if let NodeKind::Literal(LiteralValue::String(s)) = &rhs_node.kind {
                                if s.contains('*') {
                                    return Some(Hit::at(node.span));
                                }
                            }
                        }
                    }
                }
                None
            })
            .collect()
    }
}
```

## Example: Global Rule with Detail

Detect queries with many joins:

```rust
use crate::ir::IrGraph;
use crate::rules::{Hit, IrRule};

pub struct IrManyJoins;

impl IrRule for IrManyJoins {
    fn id(&self) -> &'static str { "many_joins" }
    fn name(&self) -> &'static str { "Many Joins" }
    fn description(&self) -> &'static str { "Too many joins may cause performance issues." }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let count = graph.joins().count();
        if count > 10 {
            vec![Hit::global(graph.root().span)
                .with_detail(format!("Found {} joins.", count))]
        } else {
            vec![]
        }
    }
}
```

## Rule Supercession

If one rule should suppress another when both fire:

```rust
impl IrRule for IrExtremelyLargeQuery {
    // ... id, name, description, check ...

    fn supercedes(&self) -> &'static [&'static str] {
        &["very_large_query"]  // Don't show very_large if extremely_large fires
    }
}
```

## Registration Steps

1. **`src/rules/ir_rules/mod.rs`:** Add module and re-export:
   ```rust
   pub mod my_new_rule;
   pub use my_new_rule::IrMyNewRule;
   ```

2. **`src/lib.rs`:** Register in `with_ir_rules()`:
   ```rust
   use crate::rules::ir_rules::IrMyNewRule;

   // Inside Linter::with_ir_rules():
   linter.add_ir_rule(Box::new(IrMyNewRule));
   ```

## rules.yaml Format

```yaml
rules:
  - id: my_new_rule
    pattern: "description of what triggers this rule"
    advice: "Message shown to users. Use `backticks` for code."
    category: performance|correctness|style
    severity: high|medium|low
    context: filter|projection|whole_query
```

The `advice` field is used as the message. Severity is read automatically by `rule.severity()`.
