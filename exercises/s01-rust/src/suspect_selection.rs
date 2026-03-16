use anyhow::{Context, Result};
use crate::csv_service::CsvService;
use crate::llm_payload_service::{JsonSchemaFormat, LlmPayloadService};
use crate::openai_wrapper::{OpenAiWrapper, extract_response_text};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use tokio::fs;

const CURRENT_YEAR: i32 = 2026;
const AVAILABLE_TAGS: [&str; 7] = [
    "IT",
    "transport",
    "edukacja",
    "medycyna",
    "praca z ludźmi",
    "praca z pojazdami",
    "praca fizyczna",
];

#[derive(Debug, Clone, Deserialize)]
pub struct Person {
    pub name: String,
    pub surname: String,
    pub gender: String,
    #[serde(rename = "birthDate")]
    pub birth_date: String,
    #[serde(rename = "birthPlace")]
    pub birth_place: String,
    pub job: String,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub surname: String,
    pub gender: String,
    pub born: i32,
    pub city: String,
    pub job: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct JobClassificationInput {
    enriched_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationPerson {
    pub name: String,
    pub surname: String,
    pub gender: String,
    pub born: i32,
    pub city: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TaggingResponse {
    records: Vec<TaggingRecord>,
}

#[derive(Debug, Deserialize)]
struct TaggingRecord {
    index: usize,
    tags: Vec<String>,
}

pub fn parse_people(csv_text: &str) -> Result<Vec<Person>> {
    CsvService::read_records(csv_text)
}

pub fn filter_candidates(people: &[Person]) -> Result<Vec<Candidate>> {
    let mut candidates = Vec::new();

    for person in people {
        if person.gender != "M" || person.birth_place != "Grudziądz" {
            continue;
        }

        let birth_year = parse_birth_year(&person.birth_date)?;
        let age = CURRENT_YEAR - birth_year;
        if !(20..=40).contains(&age) {
            continue;
        }

        candidates.push(Candidate {
            name: person.name.clone(),
            surname: person.surname.clone(),
            gender: person.gender.clone(),
            born: birth_year,
            city: person.birth_place.clone(),
            job: person.job.clone(),
        });
    }

    Ok(candidates)
}

pub fn parse_birth_year(birth_date: &str) -> Result<i32> {
    birth_date
        .split('-')
        .next()
        .context("Missing birth year")?
        .parse::<i32>()
        .context("Invalid birth year")
}

pub async fn classify_jobs(
    openai: &OpenAiWrapper,
    cache_path: &Path,
    model: &str,
    candidates: &[Candidate],
) -> Result<Vec<VerificationPerson>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let mut grouped_jobs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for candidate in candidates {
        grouped_jobs
            .entry(candidate.job.clone())
            .or_default()
            .insert(format!("{} {}", candidate.name, candidate.surname));
    }

    let job_inputs = grouped_jobs
        .into_iter()
        .map(|(job, names)| JobClassificationInput {
            enriched_description: format!(
                "Opis stanowiska: {}\nPowiązane osoby: {}",
                job,
                names.into_iter().collect::<Vec<_>>().join(", ")
            ),
        })
        .collect::<Vec<_>>();

    let mut cache = load_job_tag_cache(cache_path).await?;
    let uncached_jobs = job_inputs
        .iter()
        .filter(|job| !cache.contains_key(&job.enriched_description))
        .cloned()
        .collect::<Vec<_>>();

    if !uncached_jobs.is_empty() {
        let tags_by_index = request_job_tags(openai, model, &uncached_jobs).await?;

        for (index, job) in uncached_jobs.iter().enumerate() {
            let tags = tags_by_index.get(&index).cloned().unwrap_or_default();
            cache.insert(job.enriched_description.clone(), CacheEntry { tags });
        }

        save_job_tag_cache(cache_path, &cache).await?;
    }

    let tags_by_enriched_description = cache
        .into_iter()
        .map(|(key, value)| (key, value.tags))
        .collect::<HashMap<_, _>>();

    let tags_by_job = job_inputs
        .into_iter()
        .map(|job| {
            let tags = tags_by_enriched_description
                .get(&job.enriched_description)
                .cloned()
                .unwrap_or_default();
            (job.enriched_description, tags)
        })
        .collect::<HashMap<_, _>>();

    let grouped_jobs_for_lookup = candidates
        .iter()
        .fold(BTreeMap::<String, BTreeSet<String>>::new(), |mut jobs, candidate| {
            jobs.entry(candidate.job.clone())
                .or_default()
                .insert(format!("{} {}", candidate.name, candidate.surname));
            jobs
        });

    let answer = candidates
        .iter()
        .map(|candidate| {
            let names = grouped_jobs_for_lookup
                .get(&candidate.job)
                .cloned()
                .unwrap_or_default();
            let enriched_description = format!(
                "Opis stanowiska: {}\nPowiązane osoby: {}",
                candidate.job,
                names.into_iter().collect::<Vec<_>>().join(", ")
            );
            let tags = tags_by_job
                .get(&enriched_description)
                .cloned()
                .unwrap_or_default();

            VerificationPerson {
                name: candidate.name.clone(),
                surname: candidate.surname.clone(),
                gender: candidate.gender.clone(),
                born: candidate.born,
                city: candidate.city.clone(),
                tags,
            }
        })
        .collect::<Vec<_>>();

    Ok(answer)
}

pub fn select_transport_people(tagged_candidates: &[VerificationPerson]) -> Vec<VerificationPerson> {
    tagged_candidates
        .iter()
        .filter(|candidate| {
            candidate
                .tags
                .iter()
                .any(|tag| tag == "transport" || tag == "praca z pojazdami")
        })
        .cloned()
        .collect()
}

pub async fn save_suspects(suspects_path: &Path, suspects: &[VerificationPerson]) -> Result<()> {
    let serialized = serde_json::to_string_pretty(suspects)?;
    fs::write(suspects_path, format!("{serialized}\n"))
        .await
        .with_context(|| format!("Failed to write {}", suspects_path.display()))
}

pub async fn load_tagged_suspects(suspects_path: &Path) -> Result<Option<Vec<VerificationPerson>>> {
    match fs::read_to_string(suspects_path).await {
        Ok(suspects_text) => {
            let suspects = serde_json::from_str::<Vec<VerificationPerson>>(&suspects_text)
                .with_context(|| format!("Failed to parse {}", suspects_path.display()))?;
            Ok(Some(suspects))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Failed to read {}", suspects_path.display())),
    }
}

async fn load_job_tag_cache(cache_path: &Path) -> Result<HashMap<String, CacheEntry>> {
    match fs::read_to_string(cache_path).await {
        Ok(cache_text) => {
            let parsed = serde_json::from_str::<HashMap<String, CacheEntry>>(&cache_text)
                .or_else(|_| serde_json::from_str::<HashMap<String, Vec<String>>>(&cache_text).map(|legacy| {
                    legacy
                        .into_iter()
                        .map(|(description, tags)| {
                            (
                                description,
                                CacheEntry {
                                    tags: normalize_tags(tags),
                                },
                            )
                        })
                        .collect()
                }))
                .context("Failed to parse job tag cache")?;
            Ok(parsed)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(error).with_context(|| format!("Failed to read {}", cache_path.display())),
    }
}

async fn save_job_tag_cache(cache_path: &Path, cache: &HashMap<String, CacheEntry>) -> Result<()> {
    let ordered = cache
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                CacheEntry {
                    tags: normalize_tags(value.tags.clone()),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let serialized = serde_json::to_string_pretty(&ordered)?;
    fs::write(cache_path, format!("{serialized}\n"))
        .await
        .with_context(|| format!("Failed to write {}", cache_path.display()))
}

async fn request_job_tags(
    openai: &OpenAiWrapper,
    model: &str,
    job_descriptions: &[JobClassificationInput],
) -> Result<HashMap<usize, Vec<String>>> {
    if job_descriptions.is_empty() {
        return Ok(HashMap::new());
    }

    let tag_descriptions = [
        (
            "IT",
            "prace związane z oprogramowaniem, systemami, danymi, algorytmami, elektroniką lub cyberbezpieczeństwem",
        ),
        (
            "transport",
            "prace związane z logistyką, przepływem towarów, planowaniem dostaw, dystrybucją, magazynowaniem lub organizacją przewozu",
        ),
        (
            "edukacja",
            "prace związane z nauczaniem, wychowaniem, szkoleniem i przekazywaniem wiedzy",
        ),
        (
            "medycyna",
            "prace związane ze zdrowiem, leczeniem, diagnostyką, terapią lub badaniami medycznymi",
        ),
        (
            "praca z ludźmi",
            "prace wymagające bezpośredniej pracy z ludźmi, opieki, wsparcia, terapii, obsługi lub komunikacji interpersonalnej",
        ),
        (
            "praca z pojazdami",
            "prace związane z pojazdami, ich naprawą, budową, obsługą lub systemami motoryzacyjnymi",
        ),
        (
            "praca fizyczna",
            "prace manualne, instalacyjne, remontowe, budowlane, rzemieślnicze lub wymagające pracy rękami",
        ),
    ]
    .iter()
    .map(|(tag, description)| format!("- {tag}: {description}"))
    .collect::<Vec<_>>()
    .join("\n");

    let jobs_list = job_descriptions
        .iter()
        .enumerate()
        .map(|(index, job)| format!("{index}: {}", job.enriched_description))
        .collect::<Vec<_>>()
        .join("\n\n");

    let system_prompt = concat!(
        "Przypisujesz tagi do opisów zawodów. ",
        "Używaj wyłącznie tagów z dozwolonej listy. ",
        "Możesz przypisać wiele tagów do jednego opisu. ",
        "Nie zgaduj ponad treść opisu. ",
        "Nazwiska i imiona osób traktuj jako dodatkowy kontekst opisu stanowiska. ",
        "Zwróć dokładnie jeden rekord wynikowy dla każdego wejściowego indeksu."
    );
    let user_prompt = format!(
        "Dostępne tagi i ich znaczenie:\n{}\n\nOpisy stanowisk do otagowania:\n{}",
        tag_descriptions,
        jobs_list
    );
    let schema = json!({
        "type": "object",
        "properties": {
            "records": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "index": {
                            "type": "integer"
                        },
                        "tags": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": AVAILABLE_TAGS,
                            }
                        }
                    },
                    "required": ["index", "tags"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["records"],
        "additionalProperties": false
    });
    let payload = LlmPayloadService::build_responses_payload(
        model,
        system_prompt,
        &user_prompt,
        JsonSchemaFormat {
            name: "job_tagging".to_owned(),
            schema,
            strict: true,
        },
    );

    let response = openai.responses(&payload).await?;
    let output_text = extract_response_text(&response).context("Missing text output in AI response")?;
    let parsed: TaggingResponse = serde_json::from_str(&output_text)
        .with_context(|| format!("Failed to parse tagging response: {output_text}"))?;

    let mut tags_by_index = HashMap::new();
    for record in parsed.records {
        tags_by_index.insert(record.index, normalize_tags(record.tags));
    }

    Ok(tags_by_index)
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for tag in tags {
        unique.insert(tag);
    }
    unique.into_iter().collect()
}
