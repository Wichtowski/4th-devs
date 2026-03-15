use anyhow::{Context, Result};
use dotenvy::from_filename_override;
use exercises::aidevs_verification::AiDevsVerification;
use exercises::openai_wrapper::{OpenAiWrapper, resolve_model_for_provider};
use exercises::suspect_selection::{
    classify_jobs, filter_candidates, parse_people, save_suspects, select_transport_people,
};
use std::env;
use std::path::PathBuf;
use tokio::fs;

const TASK_NAME: &str = "people";
const BASE_MODEL: &str = "gpt-5.4";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    load_environment()?;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let exercise_dir = base_dir.join("S01E01");
    let csv_path = exercise_dir.join("people.csv");
    let cache_path = exercise_dir.join("job-tags-cache.json");
    let suspects_path = exercise_dir.join("suspects.json");

    let csv_text = fs::read_to_string(&csv_path)
        .await
        .with_context(|| format!("Failed to read {}", csv_path.display()))?;
    let people = parse_people(&csv_text)?;
    let candidates = filter_candidates(&people)?;

    let openai = OpenAiWrapper::from_env()?;
    let verifier = AiDevsVerification::from_env()?;
    let model = resolve_model_for_provider(BASE_MODEL)?;

    let tagged_candidates = classify_jobs(&openai, &cache_path, &model, &candidates).await?;
    let answer = select_transport_people(&tagged_candidates);
    save_suspects(&suspects_path, &answer).await?;

    println!("Candidates after hard filters: {}", candidates.len());
    println!("{}", serde_json::to_string_pretty(&answer)?);

    let verification_result = verifier.verify(TASK_NAME, &answer).await?;
    println!("Verification response:");
    println!("{}", serde_json::to_string_pretty(&verification_result)?);

    Ok(())
}

fn load_environment() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_env_path = manifest_dir.join(".env");

    if root_env_path.exists() {
        from_filename_override(&root_env_path)
            .with_context(|| format!("Failed to load {}", root_env_path.display()))?;
    }

    Ok(())
}
