use std::{collections::HashMap, fmt, rc::Rc, sync::Arc, time::Duration, usize};

use crate::{db::Record, ui::UiConfig};
use futures::future::join_all;
use reqwest::{header, StatusCode};
use serde_json;
use std::env;
use strfmt::strfmt;
use tokio::{sync::Semaphore, time::sleep};

const GOLEMIO_API_URL: &str = "https://api.golemio.cz/v2/vehiclepositions?offset=0&includeNotTracking=true&includeNotPublic=false&includePositions=false&preferredTimezone=Europe%2FPrague&routeShortName={route}";
const GOLEMIO_API_RATE_LIMIT: usize = 5;
const GOLEMIOAPI_COOL_OFF: Duration = Duration::from_millis(2000);

#[cfg(feature = "prague")]
#[derive(Debug, Clone, Copy)]
pub struct Additional {
    is_air_conditioned: Option<bool>,
    delay: Option<Duration>,
}

/// Formats additionals like: ❄ [+2 min]
#[cfg(feature = "prague")]
impl fmt::Display for Additional {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();

        // Air conditioned?
        if let Some(ac) = self.is_air_conditioned {
            if ac {
                output.push_str(" ❄");
            }
        };

        // Delayed more than 1 minute?
        if let Some(d) = self.delay {
            if d.as_secs() > 60 {
                output.push_str(format!(" [+{} min]", d.as_secs() / 60).as_str());
            };
        };

        write!(f, "{}", output)
    }
}

#[cfg(feature = "prague")]
pub async fn spice_up_departures(_config: Rc<UiConfig>, records: &mut [Record]) {
    let api_key =
        env::var("GOLEMIO_API_KEY").expect("Environment variable GOLEMIO_API_KEY is not set.");

    // HTTP client.
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "X-Access-Token",
        header::HeaderValue::from_str(&api_key).unwrap(),
    );
    headers.insert(
        "Accept",
        header::HeaderValue::from_static("application/json"),
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        // .proxy(Proxy::https("localhost:8080").unwrap())
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let client = Arc::new(client);

    // Tokio semaphore.
    let semaphore = Arc::new(Semaphore::new(GOLEMIO_API_RATE_LIMIT));

    // Spawn all at once.
    join_all(
        records
            .iter_mut()
            .map(|record| fetch_record_details(Arc::clone(&client), semaphore.clone(), record)),
    )
    .await;
}

#[cfg(feature = "prague")]
async fn fetch_record_details(
    client: Arc<reqwest::Client>,
    semaphore: Arc<Semaphore>,
    record: &mut Record,
) {
    // Give signal to semaphore.
    let _permit = semaphore.acquire().await.unwrap();

    // Fill in (to URL) the route name.
    let mut map: HashMap<String, String> = HashMap::new();
    map.insert("route".to_string(), record.route.clone());

    // Try until (with cool off) HTTP 200 is returned.
    loop {
        let result = client
            .get(strfmt(GOLEMIO_API_URL, &map).unwrap())
            .send()
            .await
            .unwrap();

        if let StatusCode::OK = result.status() {
            let json = result.json::<serde_json::Value>().await.unwrap();
            parse_record_details(record, json);

            break;
        }

        sleep(GOLEMIOAPI_COOL_OFF).await;
    }
}

#[cfg(feature = "prague")]
fn parse_record_details(record: &mut Record, json: serde_json::Value) {
    // Find suitable trip_id
    for feature in json["features"].as_array().unwrap() {
        if feature["properties"]["trip"]["gtfs"]["trip_id"] == record.trip_id {
            // Dig out A/C and delay info.
            let delay = feature["properties"]["last_position"]["delay"]["actual"].clone();
            let ac = feature["properties"]["trip"]["air_conditioned"].clone();

            // Create additional info for each
            record.additionals = Some(Additional {
                delay: delay.as_u64().map(Duration::from_secs),
                is_air_conditioned: ac.as_bool(),
            });

            break;
        }
    }
}
