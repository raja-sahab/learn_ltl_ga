use learn_ltl::*;
use clap::Parser;
use ron::de::from_reader;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short = 's', long, default_value_t = 3)]
    size: usize, //taking command line argument for size

    #[clap(short = 'f', long, default_value = "sample.ron")]
    sample_file: String, //taking command line argument for the sample file
}
const N: usize = 2; // number of propositional variables

fn evaluate_formulas(
    formulas: &[SyntaxTree],
    sample: &Sample<N>,
)
{
    for (i, formula) in formulas.iter().enumerate() {
        let mut positive_count = 0;
        let mut negative_count = 0;

        // Count how many positive traces each formula satisfies
        for trace in &sample.positive_traces {
            if sample.is_consistent(formula) {
                positive_count += 1;
            }
        }

        // Count how many negative traces each formula satisfies
        for trace in &sample.negative_traces {
            if !sample.is_consistent(formula) {
                negative_count += 1;
            }
        }

        let total_traces = sample.positive_traces.len() + sample.negative_traces.len();
        let fitness = (positive_count as f32) / (total_traces as f32) * 100.0;

        println!(
            "Formula {} satisfies {} positive traces and {} negative traces, fitness is {:.2}%",
            i, positive_count, negative_count, fitness
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    //const N: usize = 2; // number of propositional variables
    let size = args.size; // size of the formula

    let vars = vec![0; N - 1];

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
    let mut sample_content = String::new();
    buf_reader.read_to_string(&mut sample_content)?;

    let sample: Sample<N> = from_reader(sample_content.as_bytes())?;

    // Evaluate formulas
    evaluate_formulas(&formulas, &sample);

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
