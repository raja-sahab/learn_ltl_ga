use learn_ltl::*;
use clap::Parser;
use ron::de::from_reader;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;
//use serde_derive::Deserialize;
use ron;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short = 's', long, default_value_t = 3)]
    size: usize, //taking command line argument for size

    #[clap(short = 'f', long, default_value = "sample.ron")]
    sample_file: String, //taking command line argument for the sample file

    #[arg(short, long, default_value_t = false)]
    multithread: bool,
}

const N: usize = 2; // number of propositional variables

fn evaluate_formulas(
    contents: &[u8],
    multithread: bool,
    formulas: &[SyntaxTree],
    sample: &Sample<N>,
) -> Option<String> {
    for (i, formula) in formulas.iter().enumerate() {
        let mut positive_count = 0;
        let mut negative_count = 0;

        for content in contents.chunks(10) {
            // Attempt to deserialize the content
            if let Ok(deserialized_sample) = ron::de::from_bytes::<Sample<N>>(content) {
                // Check if the deserialized sample is consistent with the formula
                if deserialized_sample.is_consistent(formula) {
                    // If consistent, increment the positive count
                    positive_count += 1;
                } else {
                    // Otherwise, increment the negative count
                    negative_count += 1;
                }
            }
        }

        let total_traces = sample.positive_traces.len() + sample.negative_traces.len();
        let fitness = (positive_count as f32) / (total_traces as f32) * 100.0;

        println!(
            "Formula {} satisfies {} positive traces and {} negative traces, fitness is {:.2}%",
            i, positive_count, negative_count, fitness
        );
    }

    // Return
    Some("Result string".to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let multithread: bool = true; // Initialize multithread with a value
    let size = args.size; // size of the formula

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
    //let path = Path::new(&args.sample);
    let sample_filename = &args.sample_file;
    let file = File::open(sample_filename)?;
    let mut buf_reader = BufReader::new(file);
    let mut content = Vec::new();
    buf_reader.read_to_end(&mut content)?;

    let sample: Sample<N> = from_reader(&content[..])?;

    // Evaluate formulas
    evaluate_formulas(&content, multithread, &formulas, &sample);

    // Saving the list of formulas in a txt file
    let filename = "formulas.txt";
    let mut file = File::create(filename)?;

    for formula in &formulas {
        writeln!(file, "{:?}", formula)?;
    }

    // Count the total number of formulas and print
    let total_formulas = formulas.len();
    println!("size of the formula is {}", args.size);
    println!("propositional variables are {:?}", vars);
    println!("Total number of formulas generated: {}", total_formulas);

    Ok(())
}
