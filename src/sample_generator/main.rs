use learn_ltl::*;
use clap::Parser;
use ron::de::from_reader;
use std::fs::File;
use std::io::Write;
use std::io::{BufReader, Read};
use learn_ltl::SyntaxTree as ImportedSyntaxTree;
use ron;
use rand::Rng;
use std::sync::Arc;
use rand::seq::SliceRandom;
use rand::prelude::*;


#[derive(Parser, Debug)]
struct Args {
    #[clap(short = 's', long, default_value_t = 3)]
    size: usize, //taking command line argument for size

    #[clap(short = 'f', long, default_value = "sample.ron")]
    sample_file: String, //taking command line argument for the sample file

    #[arg(short, long, default_value_t = false)]
    multithread: bool,

    #[clap(short = 'i', long, default_value_t = 10)]
    iterations: usize, // taking command line argument for number of iterations

}

const N: usize = 2; // number of propositional variables

fn calculate_formula_size(tree: &SyntaxTree) -> usize {
    match tree {
        SyntaxTree::Atom(_) => 1,
        SyntaxTree::Not(subtree) => 1 + calculate_formula_size(subtree),
        SyntaxTree::Next(subtree) => 1 + calculate_formula_size(subtree),
        SyntaxTree::Globally(subtree) => 1 + calculate_formula_size(subtree),
        SyntaxTree::Finally(subtree) => 1 + calculate_formula_size(subtree),
        SyntaxTree::And(left, right)
        | SyntaxTree::Or(left, right)
        | SyntaxTree::Implies(left, right)
        | SyntaxTree::Until(left, right) => 1 + calculate_formula_size(left) + calculate_formula_size(right),
    }
}

fn calculate_fitness(positive_count: usize, negative_count: usize, size: usize) -> i32 {
    // Calculate the net gain in positive traces and net loss in negative traces
    let net_fitness = (positive_count as i32) - (negative_count as i32);
    // Introduce a penalty for the size of the formula
    let size_penalty = size as i32;
    // Calculate the final fitness by subtracting the size penalty
    net_fitness - size_penalty
}

fn evaluate_formulas(
    contents: &[u8],
    multithread: bool,
    formulas: &[SyntaxTree],
    sample: &Sample<N>,
) -> Option<(usize, usize)> {
    let mut total_positive_count = 0;
    let mut total_negative_count = 0;

    for formula in formulas {
        let mut positive_count = 0;
        let mut negative_count = 0;

        for content in contents.chunks(10000000) {
            // Deserialize the content
            if let Ok(deserialized_sample) = ron::de::from_bytes::<Sample<N>>(content) {
                // Count the number of satisfied positive traces
                positive_count += deserialized_sample.positive_traces
                    .iter()
                    .filter(|&trace| formula.eval(trace.as_slice()))
                    .count();

                // Count the number of satisfied negative traces
                negative_count += deserialized_sample.negative_traces
                    .iter()
                    .skip(deserialized_sample.negative_traces.len() - deserialized_sample.positive_traces.len())
                    .filter(|&trace| formula.eval(trace.as_slice()))
                    .count();
            }
        }

        total_positive_count += positive_count;
        total_negative_count += negative_count;
    }

    Some((total_positive_count, total_negative_count))
}

// Define a trait to handle operations on SyntaxTree
trait SyntaxTreeOperations {
    fn replace_branch(&self, new_branch: Arc<SyntaxTree>) -> SyntaxTree;
    fn combine_branches(branch1: Arc<SyntaxTree>, branch2: Arc<SyntaxTree>) -> SyntaxTree;
}

impl SyntaxTreeOperations for SyntaxTree {
    fn replace_branch(&self, new_branch: Arc<SyntaxTree>) -> SyntaxTree {
        match self {
            SyntaxTree::And(_, _) => SyntaxTree::And(new_branch.clone(), new_branch.clone()),
            SyntaxTree::Or(_, _) => SyntaxTree::Or(new_branch.clone(), new_branch.clone()),
            SyntaxTree::Implies(_, _) => SyntaxTree::Implies(new_branch.clone(), new_branch.clone()),
            SyntaxTree::Until(_, _) => SyntaxTree::Until(new_branch.clone(), new_branch.clone()),
            _ => self.clone(),
        }
    }

    fn combine_branches(branch1: Arc<SyntaxTree>, branch2: Arc<SyntaxTree>) -> SyntaxTree {
        match (&*branch1, &*branch2) {
            (SyntaxTree::Finally(left), SyntaxTree::Atom(right)) => SyntaxTree::Until(branch1, branch2),
            (SyntaxTree::Finally(left), _) => SyntaxTree::Implies(branch1, branch2),
            (_, SyntaxTree::Atom(right)) => SyntaxTree::Implies(branch1, branch2),
            (_, _) => SyntaxTree::Or(branch1, branch2),
        }
    }
}

fn get_branches(tree: &SyntaxTree) -> (Option<Arc<SyntaxTree>>, Option<Arc<SyntaxTree>>) {
    match tree {
        SyntaxTree::And(left, right)
        | SyntaxTree::Or(left, right)
        | SyntaxTree::Implies(left, right)
        | SyntaxTree::Until(left, right) => (Some(left.clone()), Some(right.clone())),
        _ => (None, None),
    }
}

fn crossover(parent1: &SyntaxTree, parent2: &SyntaxTree) -> Option<(SyntaxTree, SyntaxTree)> {
    //println!("Formula is {} {}", parent1, parent2); // Print the parents

    // Check if both parents have exactly two branches
    if let (Some(branch1_p1), Some(branch2_p1)) = get_branches(parent1) {
        if let (Some(branch1_p2), Some(branch2_p2)) = get_branches(parent2) {

            // println!("Formula is {} {}", parent1, parent2);

            let mut offspring1 = None;
            let mut offspring2 = None;

            // Randomly select a crossover method
            let crossover_method = rand::thread_rng().gen_range(0..=2);

            match crossover_method {
                // Method 1: Swap subtrees between parents
                0 => {
                    offspring1 = Some(parent1.replace_branch(branch2_p2.clone()));
                    offspring2 = Some(parent2.replace_branch(branch1_p1.clone()));
                }
                // Method 2: Combine branches from both parents
                1 => {
                    offspring1 = Some(SyntaxTree::combine_branches(branch1_p1.clone(), branch2_p2.clone()));
                    offspring2 = Some(SyntaxTree::combine_branches(branch1_p2.clone(), branch2_p1.clone()));
                }
                // Method 3: Randomly select a branch from each parent
                2 => {
                    let random_branch_parent1 = if rand::random() { branch1_p1.clone() } else { branch2_p1.clone() };
                    let random_branch_parent2 = if rand::random() { branch1_p2.clone() } else { branch2_p2.clone() };
                    offspring1 = Some(parent1.replace_branch(random_branch_parent2));
                    offspring2 = Some(parent2.replace_branch(random_branch_parent1));
                }
                _ => {}
            }

            // If both offspring are successfully created, return them
            if let (Some(off1), Some(off2)) = (offspring1, offspring2) {
                return Some((off1, off2));
            }
        }
    }

    // If parents do not meet the criteria, return None
    None
}

fn mutate_formula(formula: &SyntaxTree) -> SyntaxTree {
    match formula {
        SyntaxTree::Atom(_) => formula.clone(),
        SyntaxTree::Not(subtree) => SyntaxTree::Not(subtree.clone()),
        SyntaxTree::Next(subtree) => SyntaxTree::Next(subtree.clone()),
        SyntaxTree::Globally(subtree) => SyntaxTree::Globally(subtree.clone()),
        SyntaxTree::Finally(subtree) => SyntaxTree::Finally(subtree.clone()),
        SyntaxTree::And(left, right) => {
            match (rand::random::<usize>() % 3) {
                0 => SyntaxTree::Or(left.clone(), right.clone()),
                1 => SyntaxTree::Implies(left.clone(), right.clone()),
                2 => SyntaxTree::Until(left.clone(), right.clone()),
                _ => unreachable!("Unexpected random value for And mutation"),
            }
        }
        SyntaxTree::Or(left, right) => {
            match (rand::random::<usize>() % 3) {
                0 => SyntaxTree::And(left.clone(), right.clone()),
                1 => SyntaxTree::Implies(right.clone(), left.clone()),
                2 => SyntaxTree::Until(left.clone(), right.clone()),
                _ => unreachable!("Unexpected random value for Or mutation"),
            }
        }
        SyntaxTree::Implies(left, right) => {
            match (rand::random::<usize>() % 3) {
                0 => SyntaxTree::And(left.clone(), right.clone()),
                1 => SyntaxTree::Or(left.clone(), right.clone()),
                2 => SyntaxTree::Until(left.clone(), right.clone()),
                _ => unreachable!("Unexpected random value for Implies mutation"),
            }
        }
        SyntaxTree::Until(left, right) => {
            match (rand::random::<usize>() % 3) {
                0 => SyntaxTree::And(left.clone(), right.clone()),
                1 => SyntaxTree::Or(left.clone(), right.clone()),
                2 => SyntaxTree::Implies(left.clone(), right.clone()),
                _ => unreachable!("Unexpected random value for Until mutation"),
            }
        }
    }
}

fn save_formulas_to_file(formulas: &[SyntaxTree], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(filename)?;

    for formula in formulas {
        writeln!(file, "{:?}", formula)?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let multithread: bool = true; // Initialize multithread with a value
    let size = args.size; // size of the formula
    let iterations = args.iterations; // number of iterations

    let vars = vec![0, N-1];

    // Convert Vec<i32> into Vec<u8>
    let vars_vec: Vec<u8> = vars.iter().map(|&x| x as u8).collect();

    // Convert Vec<u8> into &[u8] slice
    let vars_slice: &[u8] = &vars_vec;

    // Start a new vector
    let mut formulas: Vec<SyntaxTree> = Vec::new();

    // Using learn module function
    for skeleton in SkeletonTree::gen(size) {
        let generated_formulas = skeleton.gen_formulae::<N>(vars_slice);
        formulas.extend(generated_formulas);
    }

    // Deserialize the sample of traces from a .ron file
    let sample_filename = &args.sample_file;
    let file = File::open(sample_filename)?;
    let mut buf_reader = BufReader::new(file);
    let mut content = Vec::new();
    buf_reader.read_to_end(&mut content)?;

    let sample: Sample<N> = from_reader(&content[..])?;

    // Evaluate formulas
    let (positive_count, negative_count) = evaluate_formulas(&content, multithread, &formulas, &sample)
        .expect("Evaluation failed");

    // Saving the list of formulas in a txt file
    let filename = "formulas.txt";
    let mut file = File::create(filename)?;
    //println!("Generated Formula: {:?}", formulas);

    for formula in &formulas {
        //println!(" PARENTTTTTTTTTTTTTTTTT 1111111111111111111 isssssssssssss {}", formula);
        writeln!(file, "{:?}", formula)?;
    }

    // Count the total number of formulas and print
    let total_formulas = formulas.len();
    println!("size of the formula is {}", args.size);
    println!("propositional variables are {:?}", vars);
    println!("Total number of formulas generated: {}", total_formulas);

    let mut rng = rand::thread_rng();

    for iteration in 0..iterations {
        println!("\nIteration {}", iteration + 1);
    let total_formulas = formulas.len();
        println!("Total number of initial formulas: {}", total_formulas);

    // Perform crossover
    let mut new_population: Vec<SyntaxTree> = Vec::new(); // Declare and initialize new_population

    // Combine initial formulas with crossover and mutated formulas
    let mut combined_formulas = formulas.clone();

    // Assuming you have some parent1, parent2, and crossover_point values
    // let mut parent1; // Accessing the first formula as parent1 for example
    // println!("size of the parent1 is {}", parent1);
    // let mut parent2; // Accessing the second formula as parent2 for example
    // println!("size of the parent2 is {}", parent2);
    // let crossover_point = 5; // Example crossover point

    let mut crossoverFormulas: Vec<SyntaxTree> = Vec::new();

    for i in 1..total_formulas {

        let parent1_index = rng.gen_range(0..total_formulas);
        let parent2_index = rng.gen_range(0..total_formulas);

        let parent1 = &formulas[parent1_index];
        let parent2 = &formulas[parent2_index];
        // println!("Number: {}", i);
        // parent1 = &formulas[i - 1];
        // parent2 = &formulas[i];
        // println!(" parents are {} {}", parent1, parent2);
        //println!(" PARENTTTTTTTTTTTTTTTTT 1111111111111111111 isssssssssssss {}", parent1);
        if let Some((mut offspring1, mut offspring2)) = crossover(parent1, parent2) {
            //println!(" offspring1 is {}", offspring1);
            //println!(" offspring2 is {}", offspring2);
            let offspring_vec1 = vec![offspring1.clone()]; // Wrap offspring1 in a vector
            let offspring_vec2 = vec![offspring2.clone()]; // Wrap offspring2 in a vector

            if !crossoverFormulas.contains(&offspring1) {
                crossoverFormulas.extend(offspring_vec1);
            }

            if !crossoverFormulas.contains(&offspring2) {
                crossoverFormulas.extend(offspring_vec2);
            }

        }
    }

    // Add crossover formulas to combined formulas
    combined_formulas.extend(crossoverFormulas.clone());

    //println!("After Applying corssover on valid expressions");
    //for formula in &crossoverFormulas {

    //    println!(" formula is {}", formula);
    //}

    // Perform mutation on all formulas with 10% probability
    let mut mutated_formulas: Vec<SyntaxTree> = Vec::new();
    for formula in &mut formulas {
        // Apply mutation with 20% probability
        if rand::thread_rng().gen_range(0..=99) < 20 {
            let mutated_formula = mutate_formula(formula);
            mutated_formulas.push(mutated_formula);
        }
    }

    // Add mutated formulas to combined formulas
    combined_formulas.extend(mutated_formulas.clone());

    // Save the combined set of formulas to a new file
    let combined_filename = "combined_formulas.txt";
    save_formulas_to_file(&combined_formulas, combined_filename)?;

    // Print the combined formulas after crossover and mutation
    //println!("Combined formulas after crossover and mutation: {:?}", combined_formulas);

    // Calculate the fitness scores for all formulas
    let mut formula_fitness: Vec<(SyntaxTree, i32)> = Vec::new();
    for (i, formula) in combined_formulas.iter().enumerate() {
        let (positive_count, negative_count) = evaluate_formulas(&content, multithread, &[formula.clone()], &sample)
            .expect("Evaluation failed");
        let size = calculate_formula_size(formula);
        let fitness = calculate_fitness(positive_count, negative_count, size);
        formula_fitness.push((formula.clone(), fitness));

        /* Print the evaluation results for the current formula
        println!(
            "Formula {} satisfied {} positive traces and {} negative traces, fitness is {:.2}",
            i + 1, positive_count, negative_count, fitness
        ); */
    }

    // Evaluate formulas
    let (positive_count, negative_count) = evaluate_formulas(&content, multithread, &formulas, &sample)
        .expect("Evaluation failed");

    // Calculate and print the size of each formula in combined_formulas
    for formula in &combined_formulas {
        let size = calculate_formula_size(formula);
        // println!("Formula: {:?}, Size: {}", formula, size);
    }

    // Sort the formulas based on fitness score in descending order
    formula_fitness.sort_by(|a, b| b.1.cmp(&a.1));

    // Print the formulas with their fitness for the sorted formulas
    println!("Formulas sorted by fitness:");
    for (i, (formula, fitness)) in formula_fitness.iter().enumerate() {
        let (positive_count, negative_count) = evaluate_formulas(&content, multithread, &[formula.clone()], &sample)
            .expect("Evaluation failed");
        println!(
            "Formula {} satisfied {} positive traces and {} negative traces, fitness is {:.2}",
            i + 1, positive_count, negative_count, fitness
        );
    }

    // Extract the sorted formulas from the tuples
    let sorted_formulas: Vec<SyntaxTree> = formula_fitness.iter().map(|(formula, _)| formula.clone()).collect();

    // Save the sorted formulas to a new file
    let sorted_filename = "sorted_formulas.txt";
    save_formulas_to_file(&sorted_formulas, sorted_filename)?;

    // Extract the top 100 sorted formulas
    let top_n = 100;
    let sorted_formulas: Vec<SyntaxTree> = formula_fitness
        .iter()
        .take(top_n.min(formula_fitness.len()))
        .map(|(formula, _)| formula.clone())
        .collect();

    println!("Iteration {} completed", iteration + 1);

    // Update formulas with the combined formulas
    formulas.clear();
    formulas.extend(sorted_formulas);
    }

    Ok(())
}
