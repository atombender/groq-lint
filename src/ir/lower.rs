//! AST to IR lowering.

use groq_parser::ast::{Expr, Literal, Token};

use super::node::{BinaryOp, LiteralValue, Node, NodeKind, ObjectField, ProjectionField, UnaryOp};
use super::provenance::Provenance;
use super::scope::{Scope, ScopeKind};
use super::{IrGraph, NodeId, ScopeId, Span};

/// Lower a GROQ AST expression into a semantic IR graph.
pub fn lower(expr: &Expr, query_len: usize) -> IrGraph {
    let mut ctx = LoweringContext::new();

    // Create placeholder for root scope's introducing node
    let placeholder_id = NodeId(0);

    // Create root scope
    let root_scope = Scope {
        id: ScopeId(0),
        kind: ScopeKind::Root,
        parent: None,
        introducing_node: placeholder_id,
    };
    ctx.scopes.push(root_scope);

    let root = ctx.lower_expr(expr);

    // Update root scope's introducing_node
    ctx.scopes[0].introducing_node = root;

    IrGraph {
        nodes: ctx.nodes,
        scopes: ctx.scopes,
        root,
        query_len,
    }
}

struct LoweringContext {
    nodes: Vec<Node>,
    scopes: Vec<Scope>,
    current_scope: ScopeId,
}

impl LoweringContext {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            scopes: Vec::new(),
            current_scope: ScopeId(0),
        }
    }

    fn alloc_node(&mut self, span: Span, kind: NodeKind, provenance: Provenance) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node {
            id,
            span,
            scope: self.current_scope,
            kind,
            provenance,
        });
        id
    }

    fn push_scope(&mut self, kind: ScopeKind, introducing_node: NodeId) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            id,
            kind,
            parent: Some(self.current_scope),
            introducing_node,
        });
        self.current_scope = id;
        id
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.scopes.get(self.current_scope.0 as usize) {
            if let Some(parent) = scope.parent {
                self.current_scope = parent;
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> NodeId {
        match expr {
            // --- Value Sources ---
            Expr::Everything(e) => {
                self.alloc_node(e.pos.into(), NodeKind::Dataset, Provenance::Dataset)
            }

            Expr::This(e) => self.alloc_node(e.pos.into(), NodeKind::This, Provenance::This),

            Expr::Parent(e) => {
                // Count consecutive ^ for depth (parser represents each ^ as separate node)
                let depth = 1;
                self.alloc_node(
                    e.pos.into(),
                    NodeKind::Parent { depth },
                    Provenance::Parent { depth },
                )
            }

            Expr::Param(p) => self.alloc_node(
                p.pos.into(),
                NodeKind::Param {
                    name: p.name.clone(),
                },
                Provenance::Param {
                    name: p.name.clone(),
                },
            ),

            Expr::Literal(lit) => self.lower_literal(lit),

            // Bare attribute is access on implicit this
            Expr::Attribute(attr) => {
                let this = self.alloc_node(attr.pos.into(), NodeKind::This, Provenance::This);
                self.alloc_node(
                    attr.pos.into(),
                    NodeKind::Access {
                        base: this,
                        attribute: attr.name.clone(),
                    },
                    Provenance::This,
                )
            }

            // --- Traversals ---
            Expr::Dot(dot) => self.lower_dot(dot),

            Expr::Filter(filter) => self.lower_filter(filter),

            Expr::Projection(proj) => self.lower_projection(proj),

            Expr::Postfix(postfix) => self.lower_postfix(postfix),

            Expr::Element(elem) => self.lower_element(elem),

            Expr::Slice(slice) => self.lower_slice(slice),

            Expr::ArrayTraversal(at) => {
                let base = self.lower_expr(&at.expr);
                self.alloc_node(
                    at.pos.into(),
                    NodeKind::ArrayCoerce { base },
                    Provenance::Unknown,
                )
            }

            // --- Operators ---
            Expr::Binary(bin) => {
                let lhs = self.lower_expr(&bin.lhs);
                let rhs = self.lower_expr(&bin.rhs);
                let op = token_to_binary_op(bin.operator);

                // Compute full span from lhs start to rhs end
                let lhs_span = self.nodes[lhs.0 as usize].span;
                let rhs_span = self.nodes[rhs.0 as usize].span;
                let full_span = Span {
                    start: lhs_span.start,
                    end: rhs_span.end,
                };

                // Try constant folding for arithmetic on literals
                if let Some(folded) = self.try_fold_binary(op, lhs, rhs) {
                    return self.alloc_node(
                        full_span,
                        NodeKind::Literal(folded),
                        Provenance::Literal,
                    );
                }

                let provenance = Provenance::Computed {
                    sources: vec![lhs, rhs],
                };
                self.alloc_node(full_span, NodeKind::Binary { op, lhs, rhs }, provenance)
            }

            Expr::Prefix(prefix) => {
                let operand = self.lower_expr(&prefix.rhs);
                let op = token_to_unary_op(prefix.operator);

                // Try constant folding for unary on literals
                if let Some(folded) = self.try_fold_unary(op, operand) {
                    return self.alloc_node(
                        prefix.pos.into(),
                        NodeKind::Literal(folded),
                        Provenance::Literal,
                    );
                }

                let provenance = Provenance::Computed {
                    sources: vec![operand],
                };
                self.alloc_node(
                    prefix.pos.into(),
                    NodeKind::Unary { op, operand },
                    provenance,
                )
            }

            // --- Function Calls ---
            Expr::FunctionCall(func) => {
                let args: Vec<NodeId> = func.arguments.iter().map(|a| self.lower_expr(a)).collect();
                let namespace = if func.namespace.is_empty() {
                    None
                } else {
                    Some(func.namespace.clone())
                };
                self.alloc_node(
                    func.pos.into(),
                    NodeKind::FunctionCall {
                        namespace,
                        name: func.name.clone(),
                        args,
                        pipe_base: None,
                    },
                    Provenance::Unknown,
                )
            }

            Expr::FunctionPipe(fp) => {
                let base = self.lower_expr(&fp.lhs);
                let args: Vec<NodeId> = fp
                    .func
                    .arguments
                    .iter()
                    .map(|a| self.lower_expr(a))
                    .collect();
                let namespace = if fp.func.namespace.is_empty() {
                    None
                } else {
                    Some(fp.func.namespace.clone())
                };
                self.alloc_node(
                    fp.pos.into(),
                    NodeKind::FunctionCall {
                        namespace,
                        name: fp.func.name.clone(),
                        args,
                        pipe_base: Some(base),
                    },
                    Provenance::Unknown,
                )
            }

            // --- Structural ---
            Expr::Array(arr) => {
                let elements: Vec<NodeId> =
                    arr.expressions.iter().map(|e| self.lower_expr(e)).collect();
                self.alloc_node(
                    arr.pos.into(),
                    NodeKind::ArrayLiteral { elements },
                    Provenance::Literal,
                )
            }

            Expr::Object(obj) => self.lower_object(obj),

            Expr::Range(range) => {
                let start = self.lower_expr(&range.start);
                let end = self.lower_expr(&range.end);
                self.alloc_node(
                    range.pos.into(),
                    NodeKind::Range {
                        start,
                        end,
                        inclusive: range.inclusive,
                    },
                    Provenance::Computed {
                        sources: vec![start, end],
                    },
                )
            }

            // --- Grouping/Unwrapping ---
            Expr::Group(g) => {
                // Groups are transparent in the IR - we just lower their contents
                self.lower_expr(&g.expression)
            }

            Expr::Constraint(c) => {
                // Constraints are transparent - just lower the inner expression
                self.lower_expr(&c.expression)
            }

            Expr::Subscript(s) => {
                // Subscripts are transparent - just lower the inner expression
                self.lower_expr(&s.value)
            }

            // --- Pipe ---
            Expr::Pipe(pipe) => {
                // Pipe is transparent - semantically it's just the RHS with LHS as implicit base
                // The pipe structure is already captured in FunctionPipe
                // For bare pipes (non-function RHS), we treat as transparent
                self.lower_expr(&pipe.rhs)
            }

            // --- Tuple ---
            Expr::Tuple(tuple) => {
                // Tuples are used for things like order(a, b) arguments
                // We lower them as array literals
                let elements: Vec<NodeId> =
                    tuple.members.iter().map(|e| self.lower_expr(e)).collect();
                self.alloc_node(
                    tuple.pos.into(),
                    NodeKind::ArrayLiteral { elements },
                    Provenance::Literal,
                )
            }

            // --- Ellipsis ---
            Expr::Ellipsis(e) => {
                // Spread operator - represented as a special marker in projections
                // When standalone, we represent it as This (spread current object)
                self.alloc_node(e.pos.into(), NodeKind::This, Provenance::This)
            }
        }
    }

    fn lower_literal(&mut self, lit: &Literal) -> NodeId {
        match lit {
            Literal::Null(n) => self.alloc_node(
                n.pos.into(),
                NodeKind::Literal(LiteralValue::Null),
                Provenance::Literal,
            ),
            Literal::Boolean(b) => self.alloc_node(
                b.pos.into(),
                NodeKind::Literal(LiteralValue::Bool(b.value)),
                Provenance::Literal,
            ),
            Literal::Integer(i) => self.alloc_node(
                i.pos.into(),
                NodeKind::Literal(LiteralValue::Int(i.value)),
                Provenance::Literal,
            ),
            Literal::Float(f) => self.alloc_node(
                f.pos.into(),
                NodeKind::Literal(LiteralValue::Float(f.value)),
                Provenance::Literal,
            ),
            Literal::String(s) => self.alloc_node(
                s.pos.into(),
                NodeKind::Literal(LiteralValue::String(s.value.clone())),
                Provenance::Literal,
            ),
        }
    }

    fn lower_dot(&mut self, dot: &groq_parser::ast::DotOperator) -> NodeId {
        let base = self.lower_expr(&dot.lhs);

        // The RHS of a dot can be:
        // - An attribute (most common): base.name
        // - A dereference result: base->name parses as Dot(Postfix(base, ->), Attribute(name))
        match &*dot.rhs {
            Expr::Attribute(attr) => self.alloc_node(
                dot.pos.into(),
                NodeKind::Access {
                    base,
                    attribute: attr.name.clone(),
                },
                Provenance::Unknown,
            ),
            // For other RHS expressions, we need to lower them and combine
            other => {
                let rhs = self.lower_expr(other);
                // This handles edge cases like computed property access
                // For now, treat as Access with dynamic key
                let provenance = Provenance::Computed {
                    sources: vec![base, rhs],
                };
                // We'll treat dynamic access as Element for now
                self.alloc_node(
                    dot.pos.into(),
                    NodeKind::Element { base, index: rhs },
                    provenance,
                )
            }
        }
    }

    fn lower_filter(&mut self, filter: &groq_parser::ast::Filter) -> NodeId {
        let base = self.lower_expr(&filter.lhs);

        // Create a placeholder for the filter node (for inner_scope's introducing_node)
        let filter_id = NodeId(self.nodes.len() as u32);

        // Push filter scope before lowering predicate
        let inner_scope = self.push_scope(ScopeKind::Filter, filter_id);

        let predicate = self.lower_expr(&filter.constraint.expression);

        self.pop_scope();

        self.alloc_node(
            filter.pos.into(),
            NodeKind::Filter {
                base,
                predicate,
                inner_scope,
            },
            Provenance::Unknown,
        )
    }

    fn lower_projection(&mut self, proj: &groq_parser::ast::Projection) -> NodeId {
        let base = self.lower_expr(&proj.lhs);

        // Create a placeholder for the projection node
        let proj_id = NodeId(self.nodes.len() as u32);

        // Push projection scope before lowering fields
        let inner_scope = self.push_scope(ScopeKind::Projection, proj_id);

        let fields = proj
            .object
            .expressions
            .iter()
            .map(|e| self.lower_projection_field(e))
            .collect();

        self.pop_scope();

        self.alloc_node(
            proj.pos.into(),
            NodeKind::Projection {
                base,
                fields,
                inner_scope,
            },
            Provenance::Unknown,
        )
    }

    fn lower_projection_field(&mut self, expr: &Expr) -> ProjectionField {
        match expr {
            // Spread: ...
            Expr::Ellipsis(_) => {
                let value = self.lower_expr(expr);
                ProjectionField {
                    key: None,
                    value,
                    is_spread: true,
                }
            }

            // Named field: "key": value or key: value
            Expr::Binary(bin) if bin.operator == Token::Colon => {
                let key_name = match &*bin.lhs {
                    Expr::Literal(Literal::String(s)) => Some(s.value.clone()),
                    Expr::Attribute(attr) => Some(attr.name.clone()),
                    _ => None,
                };
                let value = self.lower_expr(&bin.rhs);
                ProjectionField {
                    key: key_name,
                    value,
                    is_spread: false,
                }
            }

            // Conditional field: condition => value
            Expr::Binary(bin) if bin.operator == Token::Rocket => {
                // Lower as a binary expression with FatArrow operator
                let value = self.lower_expr(expr);
                ProjectionField {
                    key: None,
                    value,
                    is_spread: false,
                }
            }

            // Shorthand: just an attribute name (implies key = value)
            Expr::Attribute(attr) => {
                let value = self.lower_expr(expr);
                ProjectionField {
                    key: Some(attr.name.clone()),
                    value,
                    is_spread: false,
                }
            }

            // Other expression (computed field)
            _ => {
                let value = self.lower_expr(expr);
                ProjectionField {
                    key: None,
                    value,
                    is_spread: false,
                }
            }
        }
    }

    fn lower_postfix(&mut self, postfix: &groq_parser::ast::PostfixOperator) -> NodeId {
        let base = self.lower_expr(&postfix.lhs);
        let base_span = self.nodes[base.0 as usize].span;
        let postfix_span: Span = postfix.pos.into();
        // Full span from base start to postfix end
        let full_span = Span {
            start: base_span.start,
            end: postfix_span.end,
        };

        match postfix.operator {
            Token::Arrow => {
                // Dereference/join operation
                self.alloc_node(
                    full_span,
                    NodeKind::Join { base },
                    Provenance::Join { target: base },
                )
            }
            Token::AscOperator => {
                // Order modifier: expr asc
                self.alloc_node(
                    full_span,
                    NodeKind::Binary {
                        op: BinaryOp::Asc,
                        lhs: base,
                        rhs: base, // Dummy RHS for unary-like postfix
                    },
                    Provenance::Computed {
                        sources: vec![base],
                    },
                )
            }
            Token::DescOperator => {
                // Order modifier: expr desc
                self.alloc_node(
                    full_span,
                    NodeKind::Binary {
                        op: BinaryOp::Desc,
                        lhs: base,
                        rhs: base, // Dummy RHS for unary-like postfix
                    },
                    Provenance::Computed {
                        sources: vec![base],
                    },
                )
            }
            _ => {
                // Unknown postfix operator - preserve as-is
                self.alloc_node(full_span, NodeKind::This, Provenance::Unknown)
            }
        }
    }

    fn lower_element(&mut self, elem: &groq_parser::ast::Element) -> NodeId {
        let base = self.lower_expr(&elem.lhs);
        let index = self.lower_expr(&elem.idx.value);
        self.alloc_node(
            elem.pos.into(),
            NodeKind::Element { base, index },
            Provenance::Unknown,
        )
    }

    fn lower_slice(&mut self, slice: &groq_parser::ast::Slice) -> NodeId {
        let base = self.lower_expr(&slice.lhs);

        // The range is wrapped in a Subscript
        match &*slice.range.value {
            Expr::Range(range) => {
                let start = Some(self.lower_expr(&range.start));
                let end = Some(self.lower_expr(&range.end));
                self.alloc_node(
                    slice.pos.into(),
                    NodeKind::Slice {
                        base,
                        start,
                        end,
                        inclusive: range.inclusive,
                    },
                    Provenance::Unknown,
                )
            }
            // If not a range, treat as element access
            other => {
                let index = self.lower_expr(other);
                self.alloc_node(
                    slice.pos.into(),
                    NodeKind::Element { base, index },
                    Provenance::Unknown,
                )
            }
        }
    }

    fn lower_object(&mut self, obj: &groq_parser::ast::Object) -> NodeId {
        let fields: Vec<ObjectField> = obj
            .expressions
            .iter()
            .filter_map(|e| self.lower_object_field(e))
            .collect();

        self.alloc_node(
            obj.pos.into(),
            NodeKind::ObjectLiteral { fields },
            Provenance::Literal,
        )
    }

    fn lower_object_field(&mut self, expr: &Expr) -> Option<ObjectField> {
        match expr {
            // key: value
            Expr::Binary(bin) if bin.operator == Token::Colon => {
                let key = self.lower_expr(&bin.lhs);
                let value = self.lower_expr(&bin.rhs);
                Some(ObjectField { key, value })
            }
            // condition => value (conditional object entry)
            Expr::Binary(bin) if bin.operator == Token::Rocket => {
                // Lower as a Binary node with FatArrow operator
                // The key is the condition, the value is the result
                let key = self.lower_expr(&bin.lhs);
                let value = self.lower_expr(&bin.rhs);
                Some(ObjectField { key, value })
            }
            // Spread or other - skip for now
            _ => None,
        }
    }

    /// Try to fold a binary operation on two literal values.
    /// Returns Some(folded_value) if both operands are literals and the op is foldable.
    fn try_fold_binary(&self, op: BinaryOp, lhs: NodeId, rhs: NodeId) -> Option<LiteralValue> {
        let lhs_node = &self.nodes[lhs.0 as usize];
        let rhs_node = &self.nodes[rhs.0 as usize];

        let lhs_val = match &lhs_node.kind {
            NodeKind::Literal(v) => v,
            _ => return None,
        };
        let rhs_val = match &rhs_node.kind {
            NodeKind::Literal(v) => v,
            _ => return None,
        };

        match (lhs_val, rhs_val, op) {
            // Integer arithmetic
            (LiteralValue::Int(a), LiteralValue::Int(b), BinaryOp::Add) => {
                Some(LiteralValue::Int(a.checked_add(*b)?))
            }
            (LiteralValue::Int(a), LiteralValue::Int(b), BinaryOp::Sub) => {
                Some(LiteralValue::Int(a.checked_sub(*b)?))
            }
            (LiteralValue::Int(a), LiteralValue::Int(b), BinaryOp::Mul) => {
                Some(LiteralValue::Int(a.checked_mul(*b)?))
            }
            (LiteralValue::Int(a), LiteralValue::Int(b), BinaryOp::Div) if *b != 0 => {
                Some(LiteralValue::Int(a / b))
            }
            (LiteralValue::Int(a), LiteralValue::Int(b), BinaryOp::Mod) if *b != 0 => {
                Some(LiteralValue::Int(a % b))
            }
            (LiteralValue::Int(a), LiteralValue::Int(b), BinaryOp::Pow) if *b >= 0 => {
                Some(LiteralValue::Int(a.checked_pow(*b as u32)?))
            }

            // Float arithmetic
            (LiteralValue::Float(a), LiteralValue::Float(b), BinaryOp::Add) => {
                Some(LiteralValue::Float(a + b))
            }
            (LiteralValue::Float(a), LiteralValue::Float(b), BinaryOp::Sub) => {
                Some(LiteralValue::Float(a - b))
            }
            (LiteralValue::Float(a), LiteralValue::Float(b), BinaryOp::Mul) => {
                Some(LiteralValue::Float(a * b))
            }
            (LiteralValue::Float(a), LiteralValue::Float(b), BinaryOp::Div) if *b != 0.0 => {
                Some(LiteralValue::Float(a / b))
            }
            (LiteralValue::Float(a), LiteralValue::Float(b), BinaryOp::Pow) => {
                Some(LiteralValue::Float(a.powf(*b)))
            }

            // Mixed int/float - promote to float
            (LiteralValue::Int(a), LiteralValue::Float(b), BinaryOp::Add) => {
                Some(LiteralValue::Float(*a as f64 + b))
            }
            (LiteralValue::Float(a), LiteralValue::Int(b), BinaryOp::Add) => {
                Some(LiteralValue::Float(a + *b as f64))
            }
            (LiteralValue::Int(a), LiteralValue::Float(b), BinaryOp::Sub) => {
                Some(LiteralValue::Float(*a as f64 - b))
            }
            (LiteralValue::Float(a), LiteralValue::Int(b), BinaryOp::Sub) => {
                Some(LiteralValue::Float(a - *b as f64))
            }
            (LiteralValue::Int(a), LiteralValue::Float(b), BinaryOp::Mul) => {
                Some(LiteralValue::Float(*a as f64 * b))
            }
            (LiteralValue::Float(a), LiteralValue::Int(b), BinaryOp::Mul) => {
                Some(LiteralValue::Float(a * *b as f64))
            }
            (LiteralValue::Int(a), LiteralValue::Float(b), BinaryOp::Div) if *b != 0.0 => {
                Some(LiteralValue::Float(*a as f64 / b))
            }
            (LiteralValue::Float(a), LiteralValue::Int(b), BinaryOp::Div) if *b != 0 => {
                Some(LiteralValue::Float(a / *b as f64))
            }

            // String concatenation
            (LiteralValue::String(a), LiteralValue::String(b), BinaryOp::Add) => {
                Some(LiteralValue::String(format!("{}{}", a, b)))
            }

            // Boolean operations
            (LiteralValue::Bool(a), LiteralValue::Bool(b), BinaryOp::And) => {
                Some(LiteralValue::Bool(*a && *b))
            }
            (LiteralValue::Bool(a), LiteralValue::Bool(b), BinaryOp::Or) => {
                Some(LiteralValue::Bool(*a || *b))
            }

            _ => None,
        }
    }

    /// Try to fold a unary operation on a literal value.
    fn try_fold_unary(&self, op: UnaryOp, operand: NodeId) -> Option<LiteralValue> {
        let operand_node = &self.nodes[operand.0 as usize];

        let val = match &operand_node.kind {
            NodeKind::Literal(v) => v,
            _ => return None,
        };

        match (val, op) {
            (LiteralValue::Int(n), UnaryOp::Neg) => Some(LiteralValue::Int(-n)),
            (LiteralValue::Float(n), UnaryOp::Neg) => Some(LiteralValue::Float(-n)),
            (LiteralValue::Int(n), UnaryOp::Pos) => Some(LiteralValue::Int(*n)),
            (LiteralValue::Float(n), UnaryOp::Pos) => Some(LiteralValue::Float(*n)),
            (LiteralValue::Bool(b), UnaryOp::Not) => Some(LiteralValue::Bool(!b)),
            _ => None,
        }
    }
}

fn token_to_binary_op(token: Token) -> BinaryOp {
    match token {
        Token::And => BinaryOp::And,
        Token::Or => BinaryOp::Or,
        Token::Equals => BinaryOp::Eq,
        Token::NEQ => BinaryOp::Neq,
        Token::LT => BinaryOp::Lt,
        Token::LTE => BinaryOp::Lte,
        Token::GT => BinaryOp::Gt,
        Token::GTE => BinaryOp::Gte,
        Token::InOperator => BinaryOp::In,
        Token::MatchOperator => BinaryOp::Match,
        Token::Plus => BinaryOp::Add,
        Token::Minus => BinaryOp::Sub,
        Token::Asterisk => BinaryOp::Mul,
        Token::Slash => BinaryOp::Div,
        Token::Percent => BinaryOp::Mod,
        Token::Exponentiation => BinaryOp::Pow,
        Token::AscOperator => BinaryOp::Asc,
        Token::DescOperator => BinaryOp::Desc,
        Token::Rocket => BinaryOp::FatArrow,
        // Default for unknown tokens
        _ => BinaryOp::Eq,
    }
}

fn token_to_unary_op(token: Token) -> UnaryOp {
    match token {
        Token::Not => UnaryOp::Not,
        Token::Minus => UnaryOp::Neg,
        Token::Plus => UnaryOp::Pos,
        // Default for unknown tokens
        _ => UnaryOp::Pos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use groq_parser::parser::Parser;

    fn parse_and_lower(query: &str) -> IrGraph {
        let mut parser = Parser::new(query);
        let ast = parser.parse().expect("Failed to parse");
        lower(&ast, query.len())
    }

    #[test]
    fn test_lower_everything() {
        let graph = parse_and_lower("*");
        assert!(matches!(graph.root().kind, NodeKind::Dataset));
    }

    #[test]
    fn test_lower_filter() {
        let graph = parse_and_lower("*[_type == \"foo\"]");
        assert!(matches!(graph.root().kind, NodeKind::Filter { .. }));

        // Should have a Filter scope
        assert!(graph.scopes.iter().any(|s| s.kind == ScopeKind::Filter));
    }

    #[test]
    fn test_lower_join() {
        let graph = parse_and_lower("*[true]->author");
        // The join should be in the graph
        assert!(graph.joins().count() > 0);
    }

    #[test]
    fn test_lower_projection() {
        let graph = parse_and_lower("*{title, body}");
        assert!(matches!(graph.root().kind, NodeKind::Projection { .. }));
        assert!(graph.scopes.iter().any(|s| s.kind == ScopeKind::Projection));
    }

    #[test]
    fn test_in_filter_scope() {
        let graph = parse_and_lower("*[_type == \"foo\" && author->name == \"bar\"]");

        // The join inside the filter should report in_filter = true
        for join in graph.joins() {
            assert!(graph.in_filter(join));
        }
    }

    #[test]
    fn test_constant_folding_arithmetic() {
        let graph = parse_and_lower("*[count > 2 + 3]");
        // The 2 + 3 should be folded to 5, so no Binary node for that
        let binary_count = graph
            .nodes
            .iter()
            .filter(|n| matches!(&n.kind, NodeKind::Binary { op, .. } if op.is_arithmetic()))
            .count();
        assert_eq!(binary_count, 0, "Arithmetic should be folded");

        // Should have a literal 5
        let has_five = graph
            .nodes
            .iter()
            .any(|n| matches!(&n.kind, NodeKind::Literal(LiteralValue::Int(5))));
        assert!(has_five, "Should have folded literal 5");
    }

    #[test]
    fn test_constant_folding_string_concat() {
        let graph = parse_and_lower(r#"*[name == "hello" + " world"]"#);
        // The string concat should be folded
        let has_hello_world = graph.nodes.iter().any(
            |n| matches!(&n.kind, NodeKind::Literal(LiteralValue::String(s)) if s == "hello world"),
        );
        assert!(has_hello_world, "Should have folded string 'hello world'");
    }

    #[test]
    fn test_no_folding_with_field_ref() {
        let graph = parse_and_lower("*[count > price + 1]");
        // price + 1 cannot be folded (price is a field ref)
        let has_add = graph.nodes.iter().any(|n| {
            matches!(
                &n.kind,
                NodeKind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        });
        assert!(has_add, "Should have unfoldable Add binary op");
    }
}
