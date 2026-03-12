use clap::Parser;
use indexmap::IndexMap;
use whilers::{
    extended_to_core::num_to_core,
    interpret::input,
    lang::{Expression, Prog, ProgName},
    niltree::NilTree,
    output::parse_num,
    parser::expression,
};

#[derive(Parser, Debug)]
#[command(
    name = "niltree-values",
    about = "Parse a nil-tree expression, show canonical While AST, and print all valid numeric/list interpretations"
)]
struct Args {
    #[arg(help = "While expression (for example: <nil.<nil.nil>>, 3, [1,2], [[1],[2,3]])")]
    expr: String,
}

fn niltree_to_core_expr(tree: &NilTree) -> Expression {
    match tree {
        NilTree::Nil => Expression::Nil,
        NilTree::Num(n) => num_to_core(*n),
        NilTree::List(values) => values
            .iter()
            .cloned()
            .fold(Expression::Nil, |tail, head| {
                Expression::Cons(
                    Box::new(niltree_to_core_expr(&head)),
                    Box::new(tail),
                )
            }),
    }
}

fn as_list_elements(tree: &NilTree) -> Vec<NilTree> {
    match tree {
        NilTree::Nil => vec![],
        NilTree::Num(n) => vec![NilTree::Nil; *n],
        NilTree::List(v) => v.iter().rev().cloned().collect(),
    }
}

fn as_list_of_numbers(tree: &NilTree) -> Option<Vec<usize>> {
    let elems = as_list_elements(tree);
    elems.into_iter()
        .map(|elem| parse_num(&elem).ok())
        .collect::<Option<Vec<_>>>()
}

fn as_list_of_lists_of_numbers(tree: &NilTree) -> Option<Vec<Vec<usize>>> {
    let elems = as_list_elements(tree);
    elems.into_iter()
        .map(|elem| as_list_of_numbers(&elem))
        .collect::<Option<Vec<_>>>()
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let parsed_expr = expression(&args.expr)
        .map(|(rest, expr)| (rest.to_string(), expr))
        .map_err(|err| anyhow::anyhow!("Failed to parse expression: {err:?}"))?;

    if !parsed_expr.0.trim().is_empty() {
        println!("warning: unparsed tail: {:?}", parsed_expr.0);
    }

    let progs: IndexMap<ProgName, Prog> = Default::default();
    let tree = input(&args.expr, &progs)?;
    let core_ast = niltree_to_core_expr(&tree);

    println!("input expr: {}", args.expr);
    println!("parsed ast: {}", parsed_expr.1);
    println!("normalized nil tree: {}", tree);
    println!("while ast (core cons/nil): {}", core_ast);
    println!();

    match parse_num(&tree) {
        Ok(n) => println!("as number: {}", n),
        Err(_) => println!("as number: not representable"),
    }

    match as_list_of_numbers(&tree) {
        Some(values) => println!("as list of numbers: {:?}", values),
        None => println!("as list of numbers: not representable"),
    }

    match as_list_of_lists_of_numbers(&tree) {
        Some(values) => println!("as list of lists of numbers: {:?}", values),
        None => println!("as list of lists of numbers: not representable"),
    }

    Ok(())
}
