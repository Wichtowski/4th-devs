use anyhow::{Context, Result, anyhow};
use dotenvy::from_filename_override;
use exercises::aidevs_verification::AiDevsVerification;
use exercises::openai_wrapper::{OpenAiWrapper, resolve_model_for_provider};
use exercises::suspect_selection::{
    classify_jobs, filter_candidates, load_tagged_suspects, parse_people, select_transport_people,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::f64::consts::PI;
use std::path::PathBuf;
use tokio::fs;

const TASK_NAME: &str = "findhim";
const EARTH_RADIUS_KM: f64 = 6371.0;
const BASE_MODEL: &str = "gpt-5.4";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Suspect {
    name: String,
    surname: String,
    birth_year: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct PowerPlant {
    code: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct LocationResponse {
    code: Option<i32>,
    message: Option<String>,
    locations: Vec<Location>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Location {
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct AccessLevelResponse {
    code: Option<i32>,
    message: Option<String>,
    #[serde(rename = "accessLevel")]
    access_level: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FinalAnswer {
    name: String,
    surname: String,
    #[serde(rename = "accessLevel")]
    access_level: i32,
    #[serde(rename = "powerPlant")]
    power_plant: String,
}

#[derive(Debug, Clone)]
struct BestMatch {
    suspect: Suspect,
    power_plant: String,
    distance_km: f64,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    load_environment()?;
    let _forwarded_args = env::args().skip(1).collect::<Vec<_>>();

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let exercise_s01e01_dir = base_dir.join("S01E01");
    let csv_path = base_dir.join("S01E01/people.csv");
    let cache_path = exercise_s01e01_dir.join("job-tags-cache.json");
    let suspects_path = exercise_s01e01_dir.join("suspects.json");

    let csv_text = fs::read_to_string(&csv_path)
        .await
        .with_context(|| format!("Failed to read {}", csv_path.display()))?;
    let people = parse_people(&csv_text)?;
    let suspects = load_tagged_suspects(&suspects_path).await?;
    let suspects = suspects
        .map(|tagged_people| select_transport_people(&tagged_people))
        .unwrap_or_default();
    let suspects: Vec<Suspect> = if suspects.is_empty() {
       let candidates = filter_candidates(&people)?;
        let openai = OpenAiWrapper::from_env()?;
        let model = resolve_model_for_provider(BASE_MODEL)?;
        let tagged_candidates = classify_jobs(&openai, &cache_path, &model, &candidates).await?;
        select_transport_people(&tagged_candidates)
            .into_iter()
            .map(|candidate| Suspect {
                name: candidate.name,
                surname: candidate.surname,
                birth_year: candidate.born,
            })
            .collect()
    } else {
        suspects
            .into_iter()
            .map(|candidate| Suspect {
                name: candidate.name,
                surname: candidate.surname,
                birth_year: candidate.born,
            })
            .collect()
    };

    println!("Found {} suspects from S01E01 results", suspects.len());

    let api_key = env::var("DEVS_KEY").context("Missing DEVS_KEY")?;
    let client = Client::new();

    let power_plants = fetch_power_plants(&client, &api_key).await?;
    println!("Loaded {} power plants", power_plants.len());

    let verifier = AiDevsVerification::from_env()?;
    let answer = solve_findhim(&client, &api_key, &suspects, &power_plants).await?;
    println!("\nFinal answer:");
    println!("{}", serde_json::to_string_pretty(&answer)?);

    let verification_result = verifier.verify(TASK_NAME, &answer).await?;
    println!("\nVerification response:");
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

async fn fetch_power_plants(client: &Client, api_key: &str) -> Result<Vec<PowerPlant>> {
    let url = format!(
        "https://hub.ag3nts.org/data/{}/findhim_locations.json",
        api_key
    );
    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch power plants")?;
    let body = response
        .text()
        .await
        .context("Failed to read power plants response body")?;
    let value = serde_json::from_str::<Value>(&body)
        .with_context(|| format!("Failed to parse power plants JSON body: {body}"))?;

    parse_power_plants(&value)
        .with_context(|| format!("Unsupported power plants payload format: {body}"))
}

fn parse_power_plants(value: &Value) -> Result<Vec<PowerPlant>> {
    if let Some(power_plants) = value.get("power_plants") {
        return parse_power_plants(power_plants);
    }

    if let Ok(plants) = serde_json::from_value::<Vec<PowerPlant>>(value.clone()) {
        if !plants.is_empty() {
            return Ok(plants);
        }
    }

    if let Some(array) = value.as_array() {
        let plants = array
            .iter()
            .filter_map(parse_power_plant_entry)
            .collect::<Vec<_>>();
        if !plants.is_empty() {
            return Ok(plants);
        }
    }

    if let Some(object) = value.as_object() {
        let mut plants = Vec::new();
        for (key, entry) in object {
            if let Some(plant) = parse_power_plant_entry_with_fallback_code(entry, Some(key)) {
                plants.push(plant);
            }
        }
        if !plants.is_empty() {
            return Ok(plants);
        }
    }

    Err(anyhow!("No supported power plant entries found"))
}

fn parse_power_plant_entry(value: &Value) -> Option<PowerPlant> {
    parse_power_plant_entry_with_fallback_code(value, None)
}

fn parse_power_plant_entry_with_fallback_code(
    value: &Value,
    fallback_code: Option<&str>,
) -> Option<PowerPlant> {
    let object = value.as_object()?;
    let code = get_string_field(object, &["code", "powerPlant", "id"])
        .or_else(|| fallback_code.map(str::to_owned))?;
    let coordinates = get_number_field(object, &["latitude", "lat", "y"])
        .zip(get_number_field(object, &["longitude", "lon", "lng", "x"]))
        .or_else(|| fallback_code.and_then(lookup_city_coordinates))?;

    Some(PowerPlant {
        code,
        latitude: coordinates.0,
        longitude: coordinates.1,
    })
}

fn lookup_city_coordinates(city_name: &str) -> Option<(f64, f64)> {
    match city_name.to_lowercase().as_str() {
        "zabrze" => Some((50.3249, 18.7857)),
        "piotrków trybunalski" => Some((51.4052, 19.7030)),
        "grudziądz" => Some((53.4837, 18.7536)),
        "tczew" => Some((54.0924, 18.7779)),
        "radom" => Some((51.4027, 21.1471)),
        "chelmno" => Some((53.3486, 18.4251)),
        "chełmno" => Some((53.3486, 18.4251)),
        "żarnowiec" => Some((54.7446, 18.0822)),
        "zarnowiec" => Some((54.7446, 18.0822)),
        _ => None,
    }
}

fn get_string_field(
    object: &serde_json::Map<String, Value>,
    field_names: &[&str],
) -> Option<String> {
    field_names.iter().find_map(|field_name| {
        object
            .get(*field_name)
            .and_then(Value::as_str)
            .map(str::to_owned)
    })
}

fn get_number_field(object: &serde_json::Map<String, Value>, field_names: &[&str]) -> Option<f64> {
    field_names.iter().find_map(|field_name| {
        object.get(*field_name).and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
    })
}

fn get_i32_field(object: &serde_json::Map<String, Value>, field_names: &[&str]) -> Option<i32> {
    field_names.iter().find_map(|field_name| {
        object.get(*field_name).and_then(|value| {
            value
                .as_i64()
                .and_then(|number| i32::try_from(number).ok())
                .or_else(|| value.as_str().and_then(|text| text.parse::<i32>().ok()))
        })
    })
}

fn parse_location_value(value: &Value) -> Option<Location> {
    let object = value.as_object()?;
    let latitude = get_number_field(object, &["latitude", "lat", "y"])?;
    let longitude = get_number_field(object, &["longitude", "lon", "lng", "x"])?;

    Some(Location {
        latitude,
        longitude,
    })
}

fn parse_location_response(value: &Value) -> Result<LocationResponse> {
    if let Some(array) = value.as_array() {
        let locations = array
            .iter()
            .filter_map(parse_location_value)
            .collect::<Vec<_>>();

        return Ok(LocationResponse {
            code: Some(0),
            message: None,
            locations,
        });
    }

    let object = value
        .as_object()
        .context("Location response is not a JSON object")?;

    let locations = object
        .get("locations")
        .or_else(|| object.get("location"))
        .map(|locations_value| {
            if let Some(array) = locations_value.as_array() {
                array
                    .iter()
                    .filter_map(parse_location_value)
                    .collect::<Vec<_>>()
            } else {
                parse_location_value(locations_value)
                    .into_iter()
                    .collect::<Vec<_>>()
            }
        })
        .unwrap_or_default();

    Ok(LocationResponse {
        code: get_i32_field(object, &["code"]),
        message: get_string_field(object, &["message", "error"]),
        locations,
    })
}

fn parse_access_level_response(value: &Value) -> Result<AccessLevelResponse> {
    let object = value
        .as_object()
        .context("Access level response is not a JSON object")?;

    let access_level = get_i32_field(object, &["accessLevel", "access_level", "level"])
        .context("Missing access level in response")?;

    Ok(AccessLevelResponse {
        code: get_i32_field(object, &["code"]),
        message: get_string_field(object, &["message", "error"]),
        access_level,
    })
}

async fn solve_findhim(
    client: &Client,
    api_key: &str,
    suspects: &[Suspect],
    power_plants: &[PowerPlant],
) -> Result<FinalAnswer> {
    let best_match = find_best_match(client, api_key, suspects, power_plants).await?;
    let access_level = fetch_access_level(client, api_key, &best_match.suspect).await?;

    println!(
        "Nearest suspect: {} {} at {:.2} km from {}",
        best_match.suspect.name,
        best_match.suspect.surname,
        best_match.distance_km,
        best_match.power_plant
    );

    Ok(FinalAnswer {
        name: best_match.suspect.name,
        surname: best_match.suspect.surname,
        access_level,
        power_plant: best_match.power_plant,
    })
}

async fn find_best_match(
    client: &Client,
    api_key: &str,
    suspects: &[Suspect],
    power_plants: &[PowerPlant],
) -> Result<BestMatch> {
    let mut best_match: Option<BestMatch> = None;

    for suspect in suspects {
        let locations = fetch_suspect_locations(client, api_key, suspect).await?;
        if locations.is_empty() {
            continue;
        }

        for location in locations {
            if let Some((power_plant, distance_km)) =
                find_nearest_power_plant(&location, power_plants)
            {
                let should_replace = match &best_match {
                    Some(current_best) => distance_km < current_best.distance_km,
                    None => true,
                };

                if should_replace {
                    best_match = Some(BestMatch {
                        suspect: suspect.clone(),
                        power_plant,
                        distance_km,
                    });
                }
            }
        }
    }

    best_match.context("No suspect locations matched any power plant")
}

async fn fetch_suspect_locations(
    client: &Client,
    api_key: &str,
    suspect: &Suspect,
) -> Result<Vec<Location>> {
    let payload = json!({
        "apikey": api_key,
        "name": suspect.name,
        "surname": suspect.surname
    });

    let response = client
        .post("https://hub.ag3nts.org/api/location")
        .json(&payload)
        .send()
        .await
        .with_context(|| {
            format!(
                "Failed to call location API for {} {}",
                suspect.name, suspect.surname
            )
        })?;

    let body_text = response.text().await.with_context(|| {
        format!(
            "Failed to read location response for {} {}",
            suspect.name, suspect.surname
        )
    })?;

    let body_value = serde_json::from_str::<Value>(&body_text).with_context(|| {
        format!(
            "Failed to parse location response for {} {}: {}",
            suspect.name, suspect.surname, body_text
        )
    })?;

    let body = parse_location_response(&body_value).with_context(|| {
        format!(
            "Unsupported location response for {} {}: {}",
            suspect.name, suspect.surname, body_text
        )
    })?;

    if body.code.unwrap_or(0) != 0 {
        let message = body
            .message
            .unwrap_or_else(|| "Unknown location API error".to_owned());
        return Err(anyhow!(
            "Location API error for {} {}: {}",
            suspect.name,
            suspect.surname,
            message
        ));
    }

    Ok(body.locations)
}

async fn fetch_access_level(client: &Client, api_key: &str, suspect: &Suspect) -> Result<i32> {
    let payload = json!({
        "apikey": api_key,
        "name": suspect.name,
        "surname": suspect.surname,
        "birthYear": suspect.birth_year
    });

    let response = client
        .post("https://hub.ag3nts.org/api/accesslevel")
        .json(&payload)
        .send()
        .await
        .with_context(|| {
            format!(
                "Failed to call access level API for {} {}",
                suspect.name, suspect.surname
            )
        })?;

    let body_text = response.text().await.with_context(|| {
        format!(
            "Failed to read access level response for {} {}",
            suspect.name, suspect.surname
        )
    })?;

    let body_value = serde_json::from_str::<Value>(&body_text).with_context(|| {
        format!(
            "Failed to parse access level response for {} {}: {}",
            suspect.name, suspect.surname, body_text
        )
    })?;

    let body = parse_access_level_response(&body_value).with_context(|| {
        format!(
            "Unsupported access level response for {} {}: {}",
            suspect.name, suspect.surname, body_text
        )
    })?;

    if body.code.unwrap_or(0) != 0 {
        let message = body
            .message
            .unwrap_or_else(|| "Unknown access level API error".to_owned());
        return Err(anyhow!(
            "Access level API error for {} {}: {}",
            suspect.name,
            suspect.surname,
            message
        ));
    }

    Ok(body.access_level)
}

fn find_nearest_power_plant(
    location: &Location,
    power_plants: &[PowerPlant],
) -> Option<(String, f64)> {
    power_plants
        .iter()
        .map(|power_plant| {
            (
                power_plant.code.clone(),
                haversine_distance(
                    location.latitude,
                    location.longitude,
                    power_plant.latitude,
                    power_plant.longitude,
                ),
            )
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let lat1_rad = lat1 * PI / 180.0;
    let lat2_rad = lat2 * PI / 180.0;
    let delta_lat = (lat2 - lat1) * PI / 180.0;
    let delta_lon = (lon2 - lon1) * PI / 180.0;

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}
