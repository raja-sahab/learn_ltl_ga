use learn_ltl::*;
use std::fs::File;
use std::io::{Write};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short = 's', long, default_value_t = 3)]
    size: usize, //taking command line argument for size
}

fn main() {
    let args = Args::parse();
    println!("size of the formula is {}!", args.size);

    let size = args.size; //size of the formula
    const N: usize = 5; // number of propositional variables
    let vars: &[Idx] = &[1, 2, 3]; // list of propositional variables

    // start a new vector
    let mut formulas: Vec<SyntaxTree> = Vec::new();

    // using learn module function
    for skeleton in SkeletonTree::gen(size) {
        let generated_formulas = skeleton.gen_formulae::<N>(vars);
        formulas.extend(generated_formulas);
    }

    // saving the list of formulas in txt file
    let filename = "formulas.txt";
    let mut file = File::create(filename).expect("Failed to create file");

    for formula in &formulas {
        writeln!(file, "{:?}", formula).expect("Failed to write to file");
    }

    // count the total number of formulas and print
    let total_formulas = formulas.len();
    println!("Total number of formulas generated: {}", total_formulas);
}
