// use z3::ast::Bool;

use crate::syntax::*;
use crate::trace::*;
use itertools::Itertools;

use std::sync::Arc;

// pub fn learn<const N: usize>(sample: Sample<N>) -> Option<SyntaxTree> {
//     unimplemented!();
// }

// pub fn learn_size<const N: usize>(sample: Sample<N>, size: usize) -> Option<SyntaxTree> {
//     unimplemented!();
// }

#[derive(Debug, Clone)]
enum SkeletonTree {
    Zeroary,
    Unary(Arc<SkeletonTree>),
    Binary((Arc<SkeletonTree>, Arc<SkeletonTree>)),
}

impl SkeletonTree {
    fn gen_formulae<const N: usize>(&self) -> Vec<SyntaxTree> {
        match self {
            SkeletonTree::Zeroary => {
                let mut trees = (0..N)
                    .map(|n| {
                        SyntaxTree::Zeroary {
                            op: ZeroaryOp::AtomicProp(n as Var),
                        }
                    })
                    .collect::<Vec<SyntaxTree>>();
                trees.push(SyntaxTree::Zeroary {
                    op: ZeroaryOp::False,
                });
                trees
            }
            SkeletonTree::Unary(child) => {
                let mut trees = Vec::new();
                let children = child.gen_formulae::<N>();
    
                for child in children {
                    let a_child = Arc::new(child.clone());
    
                    if check_globally(&child) {
                        trees.push(SyntaxTree::Unary {
                            op: UnaryOp::Globally,
                            child: a_child.clone(),
                        });
                    }
                    if check_finally(&child) {
                        trees.push(SyntaxTree::Unary {
                            op: UnaryOp::Finally,
                            child: a_child.clone(),
                        });
                    }
    
                    if check_not(&child) {
                        trees.push(SyntaxTree::Unary {
                            op: UnaryOp::Not,
                            child: a_child.clone(),
                        });
                    }
    
                    if check_next(&child) {
                        trees.push(SyntaxTree::Unary {
                            op: UnaryOp::Next,
                            child: a_child,
                        });
                    }
                }
    
                trees
            }
            SkeletonTree::Binary(child) => {
                let mut trees = Vec::new();
                let left_children = child.0.gen_formulae::<N>();
                let right_children = child.1.gen_formulae::<N>();
                let children = left_children
                    .into_iter()
                    .cartesian_product(right_children.into_iter());
    
                for (left_child, right_child) in children {
                    let a_left_child = Arc::new(left_child.clone());
                    let a_right_child = Arc::new(right_child.clone());
    
                    if check_and(&left_child, &right_child) {
                        trees.push(SyntaxTree::Binary {
                            op: BinaryOp::And,
                            left_child: a_left_child.clone(),
                            right_child: a_right_child.clone(),
                        });
                    }
    
                    if check_or(&left_child, &right_child) {
                        trees.push(SyntaxTree::Binary {
                            op: BinaryOp::Or,
                            left_child: a_left_child.clone(),
                            right_child: a_right_child.clone(),
                        });
                    }
    
                    if check_implies(&left_child, &right_child) {
                        trees.push(SyntaxTree::Binary {
                            op: BinaryOp::Implies,
                            left_child: a_left_child.clone(),
                            right_child: a_right_child.clone(),
                        });
                    }
    
                    if check_until(&left_child, &right_child) {
                        trees.push(SyntaxTree::Binary {
                            op: BinaryOp::Until,
                            left_child: a_left_child,
                            right_child: a_right_child,
                        });
                    }
                }

                trees
            }
        }
    }
    

    // fn depth(&self) -> u8 {
    //     match self {
    //         SkeletonTree::Zeroary => 1,
    //         SkeletonTree::Unary(child) => child.depth() + 1,
    //         SkeletonTree::Binary((left_child, right_child)) => {
    //             left_child.depth().max(right_child.depth()) + 1
    //         }
    //     }
    // }
}

pub fn brute_solve<const N: usize>(sample: &Sample<N>, log: bool) -> Option<SyntaxTree> {
    (0..).into_iter().find_map(|size| {
        if log {
            println!("Searching formulae of size {}", size);
        }
        gen_skeleton_trees(size)
            .into_iter()
            .flat_map(|skeleton| skeleton.gen_formulae::<N>())
            .find(|formula| sample.is_consistent(formula))
    })
}

pub fn par_brute_solve<const N: usize>(sample: &Sample<N>, log: bool) -> Option<SyntaxTree> {
    use rayon::prelude::*;

    (0..).into_iter().find_map(|size| {
        if log {
            println!("Generating formulae of size {}", size);
        }

        gen_skeleton_trees(size)
            .into_iter()
            .flat_map(|skeleton| skeleton.gen_formulae::<N>())
            .par_bridge()
            .find_any(|formula| sample.is_consistent(formula))
    })
}

// Should be possible to compute skeleton trees at compile time
fn gen_skeleton_trees(size: usize) -> Vec<SkeletonTree> {
    if size == 0 {
        vec![SkeletonTree::Zeroary]
    } else {
        let smaller_skeletons = gen_skeleton_trees(size - 1);
        let mut skeletons: Vec<SkeletonTree> = smaller_skeletons
            .into_iter()
            .map(|child| SkeletonTree::Unary(Arc::new(child)))
            .collect();
        for left_size in 0..size {
            let left_smaller_skeletons = gen_skeleton_trees(left_size);
            let right_smaller_skeletons = gen_skeleton_trees(size - 1 - left_size);

            skeletons.extend(
                left_smaller_skeletons
                    .into_iter()
                    .cartesian_product(right_smaller_skeletons.into_iter())
                    .map(|(left_child, right_child)| {
                        SkeletonTree::Binary((
                            Arc::new(left_child),
                            Arc::new(right_child),
                        ))
                    }),
            );
        }
        skeletons
    }
}

fn check_not(child: &SyntaxTree) -> bool {
    match *child {
        // ¬¬φ ≡ φ
        SyntaxTree::Unary { op: UnaryOp::Not, .. }
        // ¬(φ -> ψ) ≡ φ ∧ ¬ψ
        | SyntaxTree::Binary { op: BinaryOp::Implies, .. } => false,
        _ => true,
    }
}

fn check_next(child: &SyntaxTree) -> bool {
    match *child {
        // ¬ X φ ≡ X ¬ φ
        SyntaxTree::Unary {
            op: UnaryOp::Next, ..
        } => false,
        _ => true,
    }
}

fn check_globally(child: &SyntaxTree) -> bool {
    match *child {
        // G G φ <=> G φ
        SyntaxTree::Unary { op: UnaryOp::Globally, .. }
        // ¬ F φ ≡ G ¬ φ
        | SyntaxTree::Unary { op: UnaryOp::Finally, .. } => false,
        _ => true,
    }
}

fn check_finally(child: &SyntaxTree) -> bool {
    match *child {
        // F F φ <=> F φ
        SyntaxTree::Unary {
            op: UnaryOp::Finally,
            ..
        } => false,
        _ => true,
    }
}

fn check_and(left_child: &SyntaxTree, right_child: &SyntaxTree) -> bool {
    // Commutative law
    left_child < right_child
        && match (left_child, right_child) {
        // Domination law
        (.., SyntaxTree::Zeroary { op: ZeroaryOp::False })
        | (SyntaxTree::Zeroary { op: ZeroaryOp::False }, ..)
        // Associative laws
        | (SyntaxTree::Binary { op: BinaryOp::And, .. }, ..)
        // De Morgan's laws
        | (SyntaxTree::Unary { op: UnaryOp::Not, .. }, SyntaxTree::Unary { op: UnaryOp::Not, .. })
        // X (φ ∧ ψ) ≡ (X φ) ∧ (X ψ)
        | (SyntaxTree::Unary { op: UnaryOp::Next, .. }, SyntaxTree::Unary { op: UnaryOp::Next, .. })
        // G (φ ∧ ψ)≡ (G φ) ∧ (G ψ)
        | (SyntaxTree::Unary { op: UnaryOp::Globally, .. }, SyntaxTree::Unary { op: UnaryOp::Globally, .. }) => false,
        // (φ -> ψ_1) ∧ (φ -> ψ_2) ≡ φ -> (ψ_1 ∧ ψ_2)
        // (φ_1 -> ψ) ∧ (φ_2 -> ψ) ≡ (φ_1 ∨ φ_2) -> ψ
        (SyntaxTree::Binary { op: BinaryOp::Implies, left_child: l_1, right_child: r_1 }, SyntaxTree::Binary { op: BinaryOp::Implies, left_child: l_2, right_child: r_2 }) if *l_1 == *l_2 || *r_1 == *r_2 => false,
        // (φ_1 U ψ) ∧ (φ_2 U ψ) ≡ (φ_1 ∧ φ_2) U ψleft_child: l_1
        (SyntaxTree::Binary { op: BinaryOp::Until, right_child: r_1, .. }, SyntaxTree::Binary { op: BinaryOp::Until, right_child: r_2, .. }) if *r_1 == *r_2 => false,
        // Absorption laws
        (SyntaxTree::Binary { op: BinaryOp::Or, left_child: l_1, right_child: r_1 }, right_child) if *(l_1.as_ref()) == *right_child || *(r_1.as_ref()) == *right_child => false,
        (left_child, SyntaxTree::Binary { op: BinaryOp::Or, left_child: l_1, right_child: r_1 }) if *(l_1.as_ref()) == *left_child || *(r_1.as_ref()) == *left_child => false,
        // Distributive laws
        (SyntaxTree::Binary { op: BinaryOp::Or, left_child: l_1, right_child: r_1 }, SyntaxTree::Binary { op: BinaryOp::Or, left_child: l_2, right_child: r_2 }) if *l_1 == *l_2 || *l_1 == *r_2 || *r_1 == *l_2 || *r_1 == *r_2 => false,
        // G φ ≡ φ ∧ X(G φ)
        (
            left_child,
            SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            }
        ) => if let SyntaxTree::Unary { op: UnaryOp::Globally, child } = child.as_ref() {
            child.as_ref() != left_child
        } else {
            true
        },
        // G φ ≡ X(G φ) ∧ φ
        (
            SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            },
            right_child,
        ) => if let SyntaxTree::Unary { op: UnaryOp::Globally, child } = child.as_ref() {
            child.as_ref() != right_child
        } else {
            true
        },
        _ => true,
    }
}

fn check_or(left_child: &SyntaxTree, right_child: &SyntaxTree) -> bool {
    // Commutative law
    left_child < right_child
        && match (left_child, right_child) {
        // Identity law
        (.., SyntaxTree::Zeroary { op: ZeroaryOp::False })
        | (SyntaxTree::Zeroary { op: ZeroaryOp::False }, ..)
        // Associative laws
        | (SyntaxTree::Binary { op: BinaryOp::Or, .. }, ..)
        // // De Morgan's laws
        // | (SyntaxTree::Unary { op: UnaryOp::Not, .. }, SyntaxTree::Unary { op: UnaryOp::Not, .. })
        // ¬φ ∨ ψ ≡ φ -> ψ, subsumes De Morgan's laws
        | (SyntaxTree::Unary { op: UnaryOp::Not, .. }, ..)
        // X (φ ∨ ψ) ≡ (X φ) ∨ (X ψ)
        | (SyntaxTree::Unary { op: UnaryOp::Next, .. }, SyntaxTree::Unary { op: UnaryOp::Next, .. })
        // F (φ ∨ ψ) ≡ (F φ) ∨ (F ψ)
        | (SyntaxTree::Unary { op: UnaryOp::Finally, .. }, SyntaxTree::Unary { op: UnaryOp::Finally, .. }) => false,
        // (φ -> ψ_1) ∨ (φ -> ψ_2) ≡ φ -> (ψ_1 ∨ ψ_2)
        // (φ_1 -> ψ) ∨ (φ_2 -> ψ) ≡ (φ_1 ∧ φ_2) -> ψ
        (SyntaxTree::Binary { op: BinaryOp::Implies, left_child: l_1, right_child: r_1 }, SyntaxTree::Binary { op: BinaryOp::Implies, left_child: l_2, right_child: r_2 }) if l_1 == l_2 || r_1 == r_2 => false,
        // (φ U ψ_1) ∨ (φ U ψ_2) ≡ φ U (ψ_1 ∨ ψ_2)
        (SyntaxTree::Binary { op: BinaryOp::Until, left_child: l_1, .. }, SyntaxTree::Binary { op: BinaryOp::Until, left_child: l_2, .. }) if l_1 == l_2 => false,
        // Absorption laws
        (SyntaxTree::Binary { op: BinaryOp::And, left_child: l_1, right_child: r_1 }, right_child) if l_1.as_ref() == right_child || r_1.as_ref() == right_child => false,
        (left_child, SyntaxTree::Binary { op: BinaryOp::And, left_child: l_1, right_child: r_1 }) if l_1.as_ref() == left_child || r_1.as_ref() == left_child => false,
        // Distributive laws
        (SyntaxTree::Binary { op: BinaryOp::And, left_child: l_1, right_child: r_1 }, SyntaxTree::Binary { op: BinaryOp::And, left_child: l_2, right_child: r_2 }) if l_1 == l_2 || l_1 == r_2 || r_1 == l_2 || r_1 == r_2 => false,
        // F φ ≡ φ ∨ X(F φ)
        (
            left_child,
            SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            }
        ) => if let SyntaxTree::Unary { op: UnaryOp::Finally, child } = child.as_ref() {
            child.as_ref() != left_child
        } else {
            true
        },
        // F φ ≡ X(F φ) ∨ φ
        (
            SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            },
            right_child,
        ) => if let SyntaxTree::Unary { op: UnaryOp::Finally, child } = child.as_ref() {
            child.as_ref() != right_child
        } else {
            true
        },
        // φ U ψ ≡ ψ ∨ ( φ ∧ X(φ U ψ) )
        // φ U ψ ≡ ψ ∨ ( X(φ U ψ) ∧ φ )
        (
            left_child,
            SyntaxTree::Binary {
                op: BinaryOp::And,
                left_child: l_1,
                right_child: r_1,
            }
        ) => if let SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            } = r_1.as_ref() {
                if let SyntaxTree::Binary {
                    op: BinaryOp::Until,
                    left_child: l_2,
                    right_child: r_2,
                } = child.as_ref() {
                    !(left_child == r_2.as_ref() && l_1 == l_2)
            } else if let SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            } = l_1.as_ref() {
                if let SyntaxTree::Binary {
                    op: BinaryOp::Until,
                    left_child: l_2,
                    right_child: r_2,
                } = child.as_ref() {
                    !(left_child == r_2.as_ref() && r_1 == l_2)
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
            SyntaxTree::Binary {
                op: BinaryOp::And,
                left_child: l_1,
                right_child: r_1,
            },
            right_child
        ) => if let SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            } = r_1.as_ref() {
                if let SyntaxTree::Binary {
                    op: BinaryOp::Until,
                    left_child: l_2,
                    right_child: r_2,
                } = child.as_ref() {
                    !(right_child == r_2.as_ref() && l_1 == l_2)
            // // Made useless by commutativity optimization on ∧
            } else if let SyntaxTree::Unary {
                op: UnaryOp::Next,
                child,
            } = l_1.as_ref() {
                if let SyntaxTree::Binary {
                    op: BinaryOp::Until,
                    left_child: l_2,
                    right_child: r_2,
                } = child.as_ref() {
                    !(right_child == r_2.as_ref() && r_1 == l_2)
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

fn check_implies(left_child: &SyntaxTree, right_child: &SyntaxTree) -> bool {
    match (left_child, right_child) {
        // Ex falso quodlibet (need to define True)
        // (SyntaxTree::Zeroary { op: ZeroaryOp::False, .. }, ..)
        // // φ -> ψ ≡ ¬ψ -> ¬φ
        // (SyntaxTree::Unary { op: UnaryOp::Not, .. }, SyntaxTree::Unary { op: UnaryOp::Not, .. }) => false,
        // ¬φ -> ψ ≡ ψ ∨ φ
        (
            SyntaxTree::Unary {
                op: UnaryOp::Not, ..
            },
            ..,
        ) => false,
        _ => true,
    }
}

fn check_until(left_child: &SyntaxTree, right_child: &SyntaxTree) -> bool {
    match (left_child, right_child) {
        // X (φ U ψ) ≡ (X φ) U (X ψ)
        (
            SyntaxTree::Unary {
                op: UnaryOp::Next, ..
            },
            SyntaxTree::Unary {
                op: UnaryOp::Next, ..
            },
        ) => false,
        (
            left_child,
            SyntaxTree::Binary {
                op: BinaryOp::Until,
                left_child: l_1,
                ..
            },
        ) if left_child == l_1.as_ref() => false,
        _ => true,
    }
}
