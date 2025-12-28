use groq_parser::ast::Expr;

fn probe(e: Expr) {
    match e {
        Expr::NonExistent => {}
    }
}

fn main() {}
