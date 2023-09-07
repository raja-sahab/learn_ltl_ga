use learn_ltl::*;
use std::fs::File;
use std::io::{Write};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short = 's', long, default_value_t = 3)]
    size: usize, //taking command line argument for size

    #[clap(short = 'v', long)]
    vars: String, //taking command line argument for propositional variables
}

fn main() {
    let args = Args::parse();
    const N: usize = 3; // number of propositional variables
    let size = args.size; //size of the formula
    // converting string argument into Vec<string>
    let vars: Vec<String> = args.vars.split_whitespace().map(String::from).collect();

    // Convert the Vec<String> into a Vec<u8> by encoding the strings as UTF-8
    let vars_as_bytes: Vec<u8> = vars
    .iter()
    .flat_map(|s| s.as_bytes().to_owned())
    .collect();

    // Convert the Vec<u8> into a &[u8] slice
    let vars_slice: &[u8] = &vars_as_bytes;

    // start a new vector
    let mut formulas: Vec<SyntaxTree> = Vec::new();

    // using learn module function
    for skeleton in SkeletonTree::gen(size) {
        let generated_formulas = skeleton.gen_formulae::<N>(vars_slice);
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
    println!("size of the formula is {}", args.size);
    println!("propositional variables are {:?}", vars);
    println!("Total number of formulas generated: {}", total_formulas);
}
