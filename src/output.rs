// This module contains functions for generating output strings (e.g. nil trees, encodings of nil trees, programs-as-data etc.)

use std::fmt::Display;

use anyhow::bail;
use indexmap::IndexMap;
use regex::{Captures, Regex};

use crate::{
    atoms::Atom,
    extended_to_core::prog_to_core,
    interpret::{interpret, ExecState},
    lang::{Prog, ProgName},
    niltree::NilTree,
    prog_as_data::unparse_prog,
};

#[derive(PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize, Copy)]
pub enum OutputFormat {
    NilTree,
    Integer,
    ListOfIntegers,
    NestedListOfIntegers,
    NestedListOfAtoms,
    ProgramAsData,
    CoreWhile,
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub enum Output {
    Text(String),
    Error(String),
    #[default]
    None,
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::NilTree => "Nil Tree",
            OutputFormat::Integer => "Integer",
            OutputFormat::ListOfIntegers => "List of integers",
            OutputFormat::NestedListOfIntegers => "Nested list of integers",
            OutputFormat::NestedListOfAtoms => "Nested list of atoms",
            OutputFormat::ProgramAsData => "Program as data",
            OutputFormat::CoreWhile => "Core While",
        }
        .fmt(f)
    }
}

pub fn generate_output(
    main_prog: &Prog,
    input: &NilTree,
    progs: &IndexMap<ProgName, Prog>,
    format: &OutputFormat,
    debug: bool,
) -> Output {
    let res = interpret(main_prog, input, progs);

    match res {
        Ok((output_tree, store)) => match format {
            OutputFormat::NilTree => {
                generate_output_with_debug(&output_tree, &store, debug, |x| x.to_string())
            }
            OutputFormat::Integer => {
                generate_output_with_debug(&output_tree, &store, debug, parse_num_str)
            }
            OutputFormat::ListOfIntegers => {
                generate_output_with_debug(&output_tree, &store, debug, format_list_ints)
            }
            OutputFormat::NestedListOfIntegers => {
                generate_output_with_debug(&output_tree, &store, debug, format_nest_list_ints)
            }
            OutputFormat::NestedListOfAtoms => {
                generate_output_with_debug(&output_tree, &store, debug, format_nest_list_atoms)
            }
            OutputFormat::ProgramAsData => match unparse_prog(main_prog, progs) {
                Ok(prog_as_data) => Output::Text(prog_as_data),
                Err(e) => Output::Error(e.to_string()),
            },
            OutputFormat::CoreWhile => match prog_to_core(main_prog, progs) {
                Ok(prog) => Output::Text(prog.to_string()),
                Err(e) => Output::Error(e.to_string()),
            },
        },
        Err(e) => Output::Error(format!("Program failed to run!\n{e}")),
    }
}

fn generate_output_with_debug(
    output_tree: &NilTree,
    store: &ExecState,
    debug: bool,
    f: impl Fn(&NilTree) -> String,
) -> Output {
    let mut res = vec![];
    if debug {
        for (prog_name, var, val) in store.get_history() {
            res.push(format!("{prog_name} {var} = {}", f(val)));
        }
        res.push("".into());
    }

    res.push(f(output_tree));
    Output::Text(res.join("\n"))
}

pub fn parse_num(tree: &NilTree) -> anyhow::Result<usize> {
    Ok(match tree {
        NilTree::Nil => 0,
        NilTree::List(v) => {
            if v.iter().all(|x| matches!(x, NilTree::Nil)) {
                v.len()
            } else {
                bail!("NaN")
            }
        }
        NilTree::Num(n) => *n,
    })
}

pub fn parse_num_str(tree: &NilTree) -> String {
    match parse_num(tree) {
        Ok(n) => n.to_string(),
        Err(e) => e.to_string(),
    }
}

pub fn parse_num_or_atom_str(tree: &NilTree) -> String {
    match parse_num(tree) {
        Ok(n) => num_to_num_or_atom_str(n),
        Err(e) => e.to_string(),
    }
}

pub fn num_to_num_or_atom_str(n: usize) -> String {
    if n < u8::MAX as usize {
        if let Ok(atom) = Atom::try_from(n as u8) {
            return atom.to_string();
        }
    }

    n.to_string()
}

// there is probably a better way to do these functions with iterators
pub fn format_list_f(tree: &NilTree, f: impl Fn(&NilTree) -> String) -> String {
    let mut res = vec![];
    if let NilTree::List(v) = tree {
        v.iter().rev().for_each(|nt| res.push(f(&nt)))
    } else if let NilTree::Num(n) = tree {
        (0..*n).for_each(|_| res.push(f(&NilTree::Nil)))
    }
    format!("[{}]", res.join(","))
}

pub fn format_list_ints(tree: &NilTree) -> String {
    format_list_f(tree, parse_num_str)
}

pub fn format_nest_list_ints(tree: &NilTree) -> String {
    format_list_f(tree, |tree| {
        if let Ok(_) = parse_num(tree) {
            parse_num_str(tree)
        } else {
            format_nest_list_ints(tree)
        }
    })
}

pub fn format_nest_list_atoms(tree: &NilTree) -> String {
    let s = format_nest_list_ints(tree);
    let re = Regex::new(r"\[\s*(\d*)\s*,").unwrap();
    re.replace_all(&s, |x: &Captures<'_>| {
        format!(
            "[{},",
            num_to_num_or_atom_str(x[1].to_string().parse().unwrap())
        )
    })
    .to_string()
}

/// Returns true if `tree` can be interpreted as a flat list of natural numbers.
///
/// - `Nil` is the empty list `[]` — valid.
/// - `Num(n)` is the shorthand for a list of *n* zeros `[0,0,…,0]` — valid.
/// - `List(v)` is valid when every element passes [`parse_num`].
pub fn is_list_of_nums(tree: &NilTree) -> bool {
    match tree {
        NilTree::Nil => true,
        NilTree::Num(_) => true,
        NilTree::List(v) => v.iter().all(|elem| parse_num(elem).is_ok()),
    }
}

/// Returns true if `tree` can be interpreted as a list of lists of natural numbers.
///
/// - `Nil` is the empty list of lists `[]` — valid.
/// - `Num(n)` is the shorthand for `[Nil; n]`; each `Nil` is `[]` — valid.
/// - `List(v)` is valid when every element satisfies [`is_list_of_nums`].
pub fn is_list_of_list_of_nums(tree: &NilTree) -> bool {
    match tree {
        NilTree::Nil => true,
        NilTree::Num(_) => true,
        NilTree::List(v) => v.iter().all(|elem| is_list_of_nums(elem)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{interpret::input, niltree::cons};
    use indexmap::IndexMap;

    fn inp(s: &str) -> NilTree {
        input(s, &IndexMap::default()).expect(s)
    }

    // ── is_list_of_nums ────────────────────────────────────────────────────

    #[test]
    fn list_of_nums_nil_is_empty_list() {
        assert!(is_list_of_nums(&NilTree::Nil));
    }

    #[test]
    fn list_of_nums_num_zero_is_nil() {
        // Num(0) == Nil == []
        assert!(is_list_of_nums(&NilTree::Num(0)));
    }

    #[test]
    fn list_of_nums_num_shorthand() {
        // Num(n) is shorthand for [0,0,...,0] with n elements
        assert!(is_list_of_nums(&NilTree::Num(3)));
        assert!(is_list_of_nums(&NilTree::Num(100)));
    }

    #[test]
    fn list_of_nums_empty_list_variant() {
        assert!(is_list_of_nums(&NilTree::list(vec![])));
    }

    #[test]
    fn list_of_nums_all_nil_elements() {
        // [nil, nil] = [0, 0]
        let tree = NilTree::list(vec![NilTree::Nil, NilTree::Nil]);
        assert!(is_list_of_nums(&tree));
    }

    #[test]
    fn list_of_nums_all_num_elements() {
        // [Num(1), Num(2), Num(3)] — each element is a plain number
        let tree = NilTree::list(vec![NilTree::Num(3), NilTree::Num(2), NilTree::Num(1)]);
        assert!(is_list_of_nums(&tree));
    }

    #[test]
    fn list_of_nums_cons_built() {
        // cons-build [0] and [1, 2, 3]
        let zero_list = cons(&NilTree::Nil, &NilTree::Nil); // = [0]
        assert!(is_list_of_nums(&zero_list));

        let three = inp("[1,2,3]");
        assert!(is_list_of_nums(&three));
    }

    #[test]
    fn list_of_nums_parsed_integers() {
        assert!(is_list_of_nums(&inp("[0]")));
        assert!(is_list_of_nums(&inp("[0,1,2,3]")));
        assert!(is_list_of_nums(&inp("[5,10,15]")));
    }

    #[test]
    fn list_of_nums_rejects_nested_list() {
        // [[1,2],[3,4]] — top-level elements are lists, not numbers
        assert!(!is_list_of_nums(&inp("[[1,2],[3,4]]")));
    }

    #[test]
    fn list_of_nums_rejects_doubly_nested() {
        // [[[1]]] — deeply nested
        assert!(!is_list_of_nums(&inp("[[[1]]]")));
    }

    #[test]
    fn list_of_nums_rejects_list_with_non_num_element() {
        // List containing [1] as an element: [1] = List([Num(1)]),
        // but parse_num(List([Num(1)])) fails because Num(1) ≠ Nil
        let inner = inp("[1]"); // = List([Num(1)])
        let outer = NilTree::list(vec![inner]);
        assert!(!is_list_of_nums(&outer));
    }

    // ── is_list_of_list_of_nums ────────────────────────────────────────────

    #[test]
    fn list_of_list_of_nums_nil_is_empty_outer_list() {
        assert!(is_list_of_list_of_nums(&NilTree::Nil));
    }

    #[test]
    fn list_of_list_of_nums_num_shorthand() {
        // Num(n) ≡ [Nil; n], and each Nil is a valid (empty) list of numbers
        assert!(is_list_of_list_of_nums(&NilTree::Num(0)));
        assert!(is_list_of_list_of_nums(&NilTree::Num(3)));
    }

    #[test]
    fn list_of_list_of_nums_empty_list_variant() {
        assert!(is_list_of_list_of_nums(&NilTree::list(vec![])));
    }

    #[test]
    fn list_of_list_of_nums_typical() {
        // [[1,2],[3,4]]
        assert!(is_list_of_list_of_nums(&inp("[[1,2],[3,4]]")));
    }

    #[test]
    fn list_of_list_of_nums_with_empty_sublists() {
        // [[], [0], [1,2,3]]
        assert!(is_list_of_list_of_nums(&inp("[[],[0],[1,2,3]]")));
    }

    #[test]
    fn list_of_list_of_nums_single_sublist() {
        // [[5]]
        assert!(is_list_of_list_of_nums(&inp("[[5]]")));
    }

    #[test]
    fn list_of_list_of_nums_integers_are_valid_sublists() {
        // [1,2,3] — each integer Num(n) is also a valid list of numbers [0,0,...,0]
        // so [1,2,3] parses as BOTH a list of numbers AND a list of lists of numbers
        assert!(is_list_of_list_of_nums(&inp("[1,2,3]")));
    }

    #[test]
    fn list_of_list_of_nums_rejects_triple_nesting() {
        // [[[1]]] — the element [[1]] contains [1], and [1] ≠ a number
        assert!(!is_list_of_list_of_nums(&inp("[[[1]]]")));
    }

    #[test]
    fn list_of_list_of_nums_rejects_non_num_grandchild() {
        // [[1,[2]]] — [2] is not a number → inner list is not a list-of-nums
        assert!(!is_list_of_list_of_nums(&inp("[[1,[2]]]")));
    }

    // ── overlap: values that satisfy both predicates ────────────────────────

    #[test]
    fn overlap_nil_satisfies_both() {
        assert!(is_list_of_nums(&NilTree::Nil));
        assert!(is_list_of_list_of_nums(&NilTree::Nil));
    }

    #[test]
    fn overlap_flat_int_list_satisfies_both() {
        // A flat integer list like [1,2,3] is a list-of-nums AND a list-of-lists-of-nums
        // because each integer Num(n) can be read as the list [0,...,0]
        let t = inp("[1,2,3]");
        assert!(is_list_of_nums(&t));
        assert!(is_list_of_list_of_nums(&t));
    }

    #[test]
    fn exclusive_nested_list_only_list_of_list() {
        // [[1,2],[3]] is a list-of-lists-of-nums but NOT a list-of-nums
        let t = inp("[[1,2],[3]]");
        assert!(!is_list_of_nums(&t));
        assert!(is_list_of_list_of_nums(&t));
    }
}
