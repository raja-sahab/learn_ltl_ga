use crate::syntax::*;
use crate::trace::*;
use itertools::Itertools;

use std::sync::Arc;

/// A tree structure with unary and binary nodes, but containing no data.
#[derive(Debug, Clone)]
enum SkeletonTree {
    Leaf,
    UnaryNode(Arc<SkeletonTree>),
    BinaryNode(Arc<(SkeletonTree, SkeletonTree)>),
}

impl SkeletonTree {
    /// Generates all possible `SkeletonTree`s of the given size,
    /// where the size is given by the number of leaves.
    fn gen(size: usize) -> Vec<SkeletonTree> {
        match size {
            0 => panic!("No tree of size 0"),
            1 => vec![SkeletonTree::Leaf],
            size => {
                let smaller_skeletons = Self::gen(size - 1);
                let mut skeletons: Vec<SkeletonTree> = smaller_skeletons
                    .into_iter()
                    .map(|branch| SkeletonTree::UnaryNode(Arc::new(branch)))
                    .collect();
                for left_size in 1..(size - 1) {
                    let left_smaller_skeletons = Self::gen(left_size);
                    let right_smaller_skeletons = Self::gen(size - 1 - left_size);

                    skeletons.extend(
                        left_smaller_skeletons
                            .into_iter()
                            .cartesian_product(right_smaller_skeletons.into_iter())
                            .map(|branches| SkeletonTree::BinaryNode(Arc::new(branches))),
                    );
                }
                skeletons
            }
        }
    }

    /// Generates all possible LTL formulae whose structure fits that of the `SkeletonTree`,
    /// in the sense that leaves of the `SkeletonTree` correspond to propositional variables,
    /// unary nodes of the `SkeletonTree` correspond to unary operators of LTL,
    /// and binary nodes of the `SkeletonTree` correspond to binary operators of LTL.
    /// After being generated, a formula is checked under filtering criteria,
    /// and discarded if found to be equivalent to other formulae that have been or will included anyway.
    /// The const generic N represents the set of propositional variables which might appear in the generated formulae.
    fn gen_formulae<const N: usize>(&self) -> Vec<SyntaxTree> {
        match self {
            // Leaves of the `SkeletonTree` correspond to propositional variables
            SkeletonTree::Leaf => (0..N)
                .map(|n| SyntaxTree::Atom(n as Idx))
                .collect::<Vec<SyntaxTree>>(),
            // Unary nodes of the `SkeletonTree` correspond to unary operators of LTL
            SkeletonTree::UnaryNode(child) => {
                let children = child.gen_formulae::<N>();
                // Use known bounds to allocate just as much memory as needed and avoid reallocations.
                let mut trees = Vec::with_capacity(4 * children.len());

                for child in children {
                    let child = Arc::new(child);

                    if check_not(child.as_ref()) {
                        trees.push(SyntaxTree::Not(child.clone()));
                    }

                    if check_next(child.as_ref()) {
                        trees.push(SyntaxTree::Next(child.clone()));
                    }

                    if check_globally(child.as_ref()) {
                        trees.push(SyntaxTree::Globally(child.clone()));
                    }

                    if check_finally(child.as_ref()) {
                        trees.push(SyntaxTree::Finally(child));
                    }
                }

                trees.shrink_to_fit();

                trees
            }
            // Binary nodes of the `SkeletonTree` correspond to binary operators of LTL
            SkeletonTree::BinaryNode(child) => {
                let left_children = child.0.gen_formulae::<N>();
                let right_children = child.1.gen_formulae::<N>();
                // Use known bounds to allocate just as much memory as needed and avoid reallocations.
                let mut trees = Vec::with_capacity(4 * left_children.len() * right_children.len());
                let children = left_children
                    .into_iter()
                    .cartesian_product(right_children.into_iter());

                for (left_child, right_child) in children {
                    let children = Arc::new((left_child, right_child));

                    if check_and(children.as_ref()) {
                        trees.push(SyntaxTree::And(children.clone()));
                    }

                    if check_or(children.as_ref()) {
                        trees.push(SyntaxTree::Or(children.clone()));
                    }

                    if check_implies(children.as_ref()) {
                        trees.push(SyntaxTree::Implies(children.clone()));
                    }

                    if check_until(children.as_ref()) {
                        trees.push(SyntaxTree::Until(children));
                    }
                }

                trees.shrink_to_fit();

                trees
            }
        }
    }
}

/// Find a formula consistent with the given `Sample`.
/// Uses a fundamentally brute-force search algorithm.
// Parallel search is faster but less consistent then single-threaded search
pub fn par_brute_solve<const N: usize>(sample: &Sample<N>, log: bool) -> Option<SyntaxTree> {
    use rayon::prelude::*;

    (1..).into_iter().find_map(|size| {
        if log {
            println!("Searching formulae of size {}", size);
        }
        // At small size, the overhead for parallel iterators is not worth it.
        // At larger size, we use parallel iterators for speed.
        if size < 6 {
            SkeletonTree::gen(size)
                .into_iter()
                .flat_map(|skeleton| skeleton.gen_formulae::<N>())
                .find(|formula| sample.is_consistent(formula))
        } else {
            SkeletonTree::gen(size)
                .into_par_iter()
                .flat_map(|skeleton| skeleton.gen_formulae::<N>())
                .find_any(|formula| sample.is_consistent(formula))
        }
    })
}

fn check_not(child: &SyntaxTree) -> bool {
    match child {
        // ¬¬φ ≡ φ
        SyntaxTree::Not(_)
        // ¬(φ -> ψ) ≡ φ ∧ ¬ψ
        | SyntaxTree::Implies(_)
        // ¬ F φ ≡ G ¬ φ
        | SyntaxTree::Finally(_) => false,
        // ¬(¬φ ∨ ψ) ≡ φ ∧ ¬ψ
        SyntaxTree::Or(children)
        // ¬(¬φ ∧ ψ) ≡ φ ∨ ¬ψ
        | SyntaxTree::And(children) if matches!(children.0, SyntaxTree::Not(_)) => false,
        // ¬(φ ∨ ¬ψ) ≡ ¬φ ∧ ψ
        SyntaxTree::Or(children)
        // ¬(φ ∧ ¬ψ) ≡ ¬φ ∨ ψ
        | SyntaxTree::And(children) if matches!(children.1, SyntaxTree::Not(_)) => false,
        _ => true,
    }
}

fn check_next(child: &SyntaxTree) -> bool {
    !matches!(
        child,
        // X ¬ φ ≡ ¬ X φ
        // X G φ ≡ G X φ
        // X F φ ≡ F X φ
        SyntaxTree::Not(_) | SyntaxTree::Globally(_) | SyntaxTree::Finally(_)
    )
}

fn check_globally(child: &SyntaxTree) -> bool {
    !matches!(
        child,
        // G G φ ≡ G φ
        SyntaxTree::Globally(_) // // X G φ ≡ G X φ
                                // | SyntaxTree::Unary { op: UnaryOp::Next, .. }
                                // // G False ≡ False
                                // | SyntaxTree::Zeroary { op: ZeroaryOp::False }
    )
}

fn check_finally(child: &SyntaxTree) -> bool {
    !matches!(
        child,
        // F F φ ≡ F φ
        SyntaxTree::Finally(_) // // X F φ ≡ F X φ
                               // | SyntaxTree::Unary { op: UnaryOp::Next, .. }
                               // // F False ≡ False
                               // | SyntaxTree::Zeroary { op: ZeroaryOp::False }
    )
}

fn check_and((left_child, right_child): &(SyntaxTree, SyntaxTree)) -> bool {
    // Commutative law WARNING: CORRECTNESS OF COMM+ASSOC IS NOT PROVEN
    left_child < right_child
    // left_child != right_child
        && match (left_child, right_child) {
        //  Excluded middle
        (child, SyntaxTree::Not(neg_child ))
        |(SyntaxTree::Not(neg_child), child) if child == neg_child.as_ref() => false,
        // // Domination law
        // (.., SyntaxTree::Zeroary { op: ZeroaryOp::False })
        // | (SyntaxTree::Zeroary { op: ZeroaryOp::False }, ..)
        // Associative laws
        | (SyntaxTree::And(_), _)
        // De Morgan's laws
        | (SyntaxTree::Not(_), SyntaxTree::Not(_))
        // X (φ ∧ ψ) ≡ (X φ) ∧ (X ψ)
        | (SyntaxTree::Next(_), SyntaxTree::Next(_))
        // G (φ ∧ ψ)≡ (G φ) ∧ (G ψ)
        | (SyntaxTree::Globally(_), SyntaxTree::Globally(_)) => false,
        // (φ -> ψ_1) ∧ (φ -> ψ_2) ≡ φ -> (ψ_1 ∧ ψ_2)
        // (φ_1 -> ψ) ∧ (φ_2 -> ψ) ≡ (φ_1 ∨ φ_2) -> ψ
        (SyntaxTree::Implies(c_1), SyntaxTree::Implies(c_2)) if c_1.0 == c_2.0 || c_1.1 == c_2.1 => false,
        // (φ_1 U ψ) ∧ (φ_2 U ψ) ≡ (φ_1 ∧ φ_2) U ψ
        (SyntaxTree::Until(c_1), SyntaxTree::Until(c_2)) if c_1.1 == c_2.1 => false,
        // Absorption laws
        (SyntaxTree::Or(children), right_child) if children.0 == *right_child || children.1 == *right_child => false,
        (left_child, SyntaxTree::Or(children)) if children.0 == *left_child || children.1 == *left_child => false,
        // Distributive laws
        (SyntaxTree::Or(c_1), SyntaxTree::Or(c_2)) if c_1.0 == c_2.0 || c_1.0 == c_2.1 || c_1.1 == c_2.0 || c_1.1 == c_2.1 => false,
        // G φ ≡ φ ∧ X(G φ)
        (
            left_child,
            SyntaxTree::Next(child)
        ) => if let SyntaxTree::Globally(child) = child.as_ref() {
            child.as_ref() != left_child
        } else {
            true
        },
        // G φ ≡ X(G φ) ∧ φ
        (
            SyntaxTree::Next(child),
            right_child,
        ) => if let SyntaxTree::Globally(child) = child.as_ref() {
            child.as_ref() != right_child
        } else {
            true
        },
        _ => true,
    }
}

fn check_or((left_child, right_child): &(SyntaxTree, SyntaxTree)) -> bool {
    // Commutative law WARNING: CORRECTNESS OF COMM+ASSOC IS NOT PROVEN
    left_child < right_child
    // left_child != right_child
        && match (left_child, right_child) {
        //  Excluded middle
        (child, SyntaxTree::Not(neg_child))
        | (SyntaxTree::Not(neg_child), child) if child == neg_child.as_ref() => false,
        // // Identity law
        // (.., SyntaxTree::Zeroary { op: ZeroaryOp::False })
        // | (SyntaxTree::Zeroary { op: ZeroaryOp::False }, ..)
        // Associative laws
        | (SyntaxTree::Or(_), _)
        // // De Morgan's laws
        // | (SyntaxTree::Unary { op: UnaryOp::Not, .. }, SyntaxTree::Unary { op: UnaryOp::Not, .. })
        // ¬φ ∨ ψ ≡ φ -> ψ, subsumes De Morgan's laws
        | (SyntaxTree::Not(_), _)
        // X (φ ∨ ψ) ≡ (X φ) ∨ (X ψ)
        | (SyntaxTree::Next(_), SyntaxTree::Next(_))
        // F (φ ∨ ψ) ≡ (F φ) ∨ (F ψ)
        | (SyntaxTree::Finally(_), SyntaxTree::Finally(_)) => false,
        // (φ -> ψ_1) ∨ (φ -> ψ_2) ≡ φ -> (ψ_1 ∨ ψ_2)
        // (φ_1 -> ψ) ∨ (φ_2 -> ψ) ≡ (φ_1 ∧ φ_2) -> ψ
        (SyntaxTree::Implies(c_1), SyntaxTree::Implies(c_2)) if c_1.0 == c_2.0 || c_1.1 == c_2.1 => false,
        // (φ U ψ_1) ∨ (φ U ψ_2) ≡ φ U (ψ_1 ∨ ψ_2)
        (SyntaxTree::Until(c_1), SyntaxTree::Until(c_2)) if c_1.0 == c_2.0 => false,
        // Absorption laws
        (SyntaxTree::And(children), right_child) if children.0 == *right_child || children.1 == *right_child => false,
        (left_child, SyntaxTree::And(children)) if children.0 == *left_child || children.1 == *left_child => false,
        // Distributive laws
        (SyntaxTree::And(c_1), SyntaxTree::And(c_2)) if c_1.0 == c_2.0 || c_1.0 == c_2.1 || c_1.1 == c_2.0 || c_1.1 == c_2.1 => false,
        // F φ ≡ φ ∨ X(F φ)
        (
            left_child,
            SyntaxTree::Next(child)
        ) => if let SyntaxTree::Finally(child) = child.as_ref() {
            child.as_ref() != left_child
        } else {
            true
        },
        // F φ ≡ X(F φ) ∨ φ
        (
            SyntaxTree::Next(child),
            right_child,
        ) => if let SyntaxTree::Finally(child) = child.as_ref() {
            child.as_ref() != right_child
        } else {
            true
        },
        // φ U ψ ≡ ψ ∨ ( φ ∧ X(φ U ψ) )
        // φ U ψ ≡ ψ ∨ ( X(φ U ψ) ∧ φ )
        (
            left_child,
            SyntaxTree::And(c_1)
        ) => if let SyntaxTree::Next(child) = &c_1.1 {
                if let SyntaxTree::Until(c_2) = child.as_ref() {
                    !(*left_child == c_2.1 && c_1.0 == c_2.0)
            } else if let SyntaxTree::Next(child) = &c_1.0 {
                if let SyntaxTree::Until(c_2) = child.as_ref() {
                    !(*left_child == c_2.1 && c_1.1 == c_2.0)
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        }
        // φ U ψ ≡ ( φ ∧ X(φ U ψ) ) ∨ ψ
        // φ U ψ ≡ ( X(φ U ψ) ∧ φ ) ∨ ψ
        (
            SyntaxTree::And(c_1),
            right_child
        ) => if let SyntaxTree::Next(child) = &c_1.1 {
                if let SyntaxTree::Until(c_2) = child.as_ref() {
                    !(*right_child == c_2.1 && c_1.0 == c_2.0)
            } else if let SyntaxTree::Next(child) = &c_1.0 {
                if let SyntaxTree::Until(c_2) = child.as_ref() {
                    !(*right_child == c_2.1 && c_1.1 == c_2.0)
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        }
        _ => true,
    }
}

fn check_implies((left_child, right_child): &(SyntaxTree, SyntaxTree)) -> bool {
    left_child != right_child
        && !matches!(
            (left_child, right_child),
            // // Ex falso quodlibet (True defined as ¬False)
            // (
            //     SyntaxTree::Zeroary { op: ZeroaryOp::False },
            //     ..,
            // )
            // // φ -> False ≡ ¬φ
            // | (
            //     ..,
            //     SyntaxTree::Zeroary { op: ZeroaryOp::False },
            // )
            // // (SyntaxTree::Zeroary { op: ZeroaryOp::False, .. }, ..)
            // // φ -> ψ ≡ ¬ψ -> ¬φ // subsumed by following rule
            // (SyntaxTree::Unary { op: UnaryOp::Not, .. }, SyntaxTree::Unary { op: UnaryOp::Not, .. }) => false,
            // ¬φ -> ψ ≡ ψ ∨ φ
            (
            SyntaxTree::Not(_),
            _,
        )
        // φ -> ¬ψ ≡ ¬(ψ ∧ φ)
        | (
            _,
            SyntaxTree::Not(_),
        )
        // Currying
        // φ_1 -> (φ_2 -> ψ) ≡ (φ_1 ∧ φ_2) -> ψ
        | (
            _,
            SyntaxTree::Implies(_),
        )
        )
}

fn check_until((left_child, right_child): &(SyntaxTree, SyntaxTree)) -> bool {
    // φ U φ ≡ φ
    left_child != right_child
        && match (left_child, right_child) {
            // // φ U False ≡ G φ
            // (
            //     ..,
            //     SyntaxTree::Zeroary {
            //         op: ZeroaryOp::False
            //     },
            // )
            // // False U φ ≡ φ
            // | (
            //     SyntaxTree::Zeroary {
            //         op: ZeroaryOp::False
            //     },
            //     ..
            // )
            // X (φ U ψ) ≡ (X φ) U (X ψ)
            (SyntaxTree::Next(_), SyntaxTree::Next(_)) => false,
            // φ U ψ ≡ φ U (φ U ψ)
            (left_child, SyntaxTree::Until(children)) if *left_child == children.0 => false,
            _ => true,
        }
}

// TODO: write tests for checks

#[cfg(test)]
mod learn {
    use super::*;

    #[test]
    fn formulae() {
        for size in 1..=10 {
            let formulae = SkeletonTree::gen(size)
                .into_iter()
                .flat_map(|skeleton| skeleton.gen_formulae::<5>())
                .count();
            println!("formulae found (size {size}, vars 5): {formulae}");
        }
    }
}
