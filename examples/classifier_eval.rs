use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;

use aetheris_cortex::{ClassifyInput, RegexClassifier, RequestClassifier};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct EvalItem {
    id: String,
    query: String,
    context: Option<String>,
}

#[derive(Debug, Serialize)]
struct EvalOutput {
    id: String,
    request_type: String,
    complexity: u8,
    confidence: f32,
    latency_us: u128,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_path = if Path::new("classifier-eval.json").exists() {
        "classifier-eval.json"
    } else {
        "../classifier-eval.json"
    };

    println!("Loading evaluation dataset from: {}", dataset_path);
    let file = File::open(dataset_path)?;
    let reader = BufReader::new(file);

    let items: Vec<EvalItem> = if dataset_path.ends_with(".jsonl") {
        let mut list = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                list.push(serde_json::from_str(&line)?);
            }
        }
        list
    } else {
        serde_json::from_reader(reader)?
    };

    println!("Loaded {} test queries. Running classification...", items.len());
    let classifier = RegexClassifier;
    let mut outputs = Vec::new();

    let mut out_file = File::create("classifier-out.jsonl")?;

    for item in items {
        let start = Instant::now();
        let classification = classifier
            .classify(ClassifyInput {
                query: item.query.clone(),
                context: item.context.clone(),
            })
            .await;
        let latency_us = start.elapsed().as_micros();

        let out = EvalOutput {
            id: item.id,
            request_type: classification.request_type.as_str().to_string(),
            complexity: classification.complexity,
            confidence: classification.confidence,
            latency_us,
        };

        let json_line = serde_json::to_string(&out)?;
        writeln!(out_file, "{}", json_line)?;
        outputs.push(out);
    }

    println!(
        "Evaluation complete! Wrote {} records to classifier-out.jsonl",
        outputs.len()
    );

    Ok(())
}
