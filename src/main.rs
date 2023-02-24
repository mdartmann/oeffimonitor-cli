use serde::Deserialize;
use serde_json::Value;
use std::{thread::sleep, time::Duration, vec};
use thiserror::Error;

const STATION_IDS: &[i32] = &[
    252,  // Rathaus – 2 (Richtung Friedrich-Engels-Platz)
    269,  // Rathaus – 2 (Richtung Dornbach)
    4205, // Rathaus – U2 (gesperrt)
    4210, // Rathaus – U2 (gesperrt)
    1346, // Landesgerichtsstraße – 43, 44, N43 (stadtauswärts)
    1212, // Schottentor – 37, 38, 40, 41, 42 (stadtauswärts)
    1303, // Schottentor — 40A (stadtauswärts)
    3701, // Schottentor – N38 (stadtauswärts, nur am Wochenende)
    5568, // Schottentor – N41 (stadtauswärts)
    17, // Rathausplatz/Burgtheater – D, 1, 71, N25, N38, N60, N66 (Richtung Schottentor, Nachtbusse nur wochentags)
    48, // Stadiongasse/Parlament – D, 1, 71 (Richtung Volkstheater)
    16, // Stadiongasse/Parlament – D, 1, 2, 71 (Richtung Schottentor)
    1401, // Volkstheater – 48A (stadtauswärts)
    1440, // Volkstheater – 49 (stadtauswärts)
    4908, // Volkstheater – U3 (Richtung Ottakring)
    4909, // Volkstheater – U3 (Richtung Simmering)
    1376, // Auerspergstraße – 46 (stadtauswärts)
    5691, // Auerspergstraße – N46 (stadtauswärts)
];

const API_URL: &str = "http://www.wienerlinien.at/ogd_realtime/monitor/";

#[derive(Error, Debug)]
enum ApiRequestError {
    #[error("API request failed: {0}")]
    ApiReqFailed(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    JsonParsingFailed(#[from] serde_json::Error),

    #[error("Missing response field: {0}")]
    MissingField(String),
}

struct WienerLinienAPIRequest {
    traffic_info: String,
    stop_id: Vec<i32>,
}

impl WienerLinienAPIRequest {
    fn to_req_url(&self) -> String {
        format!(
            "{}?activateTrafficInfo={}{}",
            API_URL,
            self.traffic_info,
            self.stop_id
                .iter()
                .map(|x| "&stopId=".to_string() + &x.to_string())
                .collect::<String>()
        )
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
struct WienerLinienMonitor {
    locationStop: WienerLinienLocationStop,
    lines: Vec<WienerLinienLine>,
}

#[derive(Debug, Clone, Deserialize)]
struct WienerLinienLocationStop {
    #[serde(rename = "geometry")]
    geometry: StopGeometry,
    #[serde(rename = "properties")]
    properties: StopProperties,
}
#[derive(Debug, Clone, Deserialize)]
struct StopGeometry {
    coordinates: [f32; 2],
}

#[derive(Debug, Clone, Deserialize)]
struct StopProperties {
    title: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WienerLinienLine {
    name: String,
    #[serde(rename = "towards")]
    destination: String,
    #[serde(rename = "type")]
    vehicle_type: String,
    departures: WienerLinienLineDepartures,
}

#[derive(Debug, Clone, Deserialize)]
struct WienerLinienLineDepartures {
    departure: Vec<WienerLinienLineDeparture>,
}

#[derive(Debug, Clone, Deserialize)]
struct WienerLinienLineDeparture {
    #[serde(rename = "departureTime")]
    departure_time: WienerLinienLineDepartureTime,
}

#[derive(Debug, Clone, Deserialize)]
struct WienerLinienLineDepartureTime {
    #[serde(rename = "timePlanned")]
    time_planned: String,
    #[serde(rename = "timeReal")]
    time_real: Option<String>,
    countdown: i64,
}

#[derive(Clone, Deserialize)]
struct WienerLinienTrafficInfo {
    priority: String,
    title: String,
    description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WienerLinienVehicleType {
    Tram,
    Metro,
    CityBus,
    NightBus,
}

#[derive(Clone, Eq)]
struct Line {
    vehicle_type: WienerLinienVehicleType,
    name: String,
}

#[derive(Clone, Eq)]
struct Departure {
    time_planned: String,
    time_real: Option<String>,
    countdown: i64,
    station_name: String,
    destination_name: String,
    line: Line,
}

impl Line {
    fn from_wiener_linien_line(input: &WienerLinienLine) -> Self {
        Self {
            name: input.name.to_owned(),
            vehicle_type: match input.vehicle_type.as_str() {
                "ptTram" => WienerLinienVehicleType::Tram,
                "ptMetro" => WienerLinienVehicleType::Metro,
                "ptBusCity" => WienerLinienVehicleType::CityBus,
                "ptBusNight" => WienerLinienVehicleType::NightBus,
                _ => panic!("Unknown vehicle type!"),
            },
        }
    }
}

impl Departure {
    fn from_wiener_linien_api(
        t_line: &WienerLinienLine,
        t_time_planned: &str,
        t_time_real: &Option<String>,
        t_countdown: &i64,
        t_station_name: &str,
    ) -> Self {
        Departure {
            line: Line::from_wiener_linien_line(t_line),
            time_planned: t_time_planned.to_owned(),
            time_real: t_time_real.clone(),
            countdown: *t_countdown,
            destination_name: t_line.destination.clone(),
            station_name: t_station_name.to_owned(),
        }
    }
}

impl Ord for Departure {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.countdown.cmp(&other.countdown)
    }
}

impl PartialOrd for Departure {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Departure {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line
            && self.destination_name == other.destination_name
            && self.station_name == other.station_name
            && self.time_planned == other.time_planned
    }
}

impl PartialEq for Line {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.vehicle_type == other.vehicle_type
    }
}

async fn get_data_from_api(req: &WienerLinienAPIRequest) -> Result<String, reqwest::Error> {
    let res = reqwest::get(req.to_req_url()).await;

    return res?.text().await;
}

async fn make_api_request(
) -> Result<(Vec<Departure>, Option<Vec<WienerLinienTrafficInfo>>), ApiRequestError> {
    let reqobj = WienerLinienAPIRequest {
        traffic_info: "stoerunglang".to_string(),
        stop_id: STATION_IDS.to_vec(),
    };

    let response_text = get_data_from_api(&reqobj)
        .await
        .map_err(ApiRequestError::ApiReqFailed)?;

    let response_json: Value =
        serde_json::from_str(&response_text).map_err(ApiRequestError::JsonParsingFailed)?;

    let response_trafficinfo_json = response_json["data"]["trafficInfos"].as_array();

    let response_monitors_json = response_json["data"]["monitors"]
        .as_array()
        .ok_or(ApiRequestError::MissingField("monitors".to_string()))?;

    let mut wl_monitors: Vec<WienerLinienMonitor> = vec![];
    response_monitors_json.iter().try_for_each(|monitor| {
        let stop_json = monitor["locationStop"].clone();

        let station =
            serde_json::from_value(stop_json).map_err(ApiRequestError::JsonParsingFailed)?;

        let mut v_lines: Vec<WienerLinienLine> = vec![];

        if let Some(arr_lines) = monitor["lines"].as_array() {
            arr_lines
                .iter()
                .for_each(|line| v_lines.push(serde_json::from_value(line.to_owned()).unwrap()));
            wl_monitors.push(WienerLinienMonitor {
                lines: v_lines,
                locationStop: station,
            });
            Ok(())
        } else {
            Err(ApiRequestError::MissingField(
                "lines missing or of wrong type".to_string(),
            ))
        }
    })?;

    let mut departures: Vec<Departure> = vec![];
    wl_monitors.iter().for_each(|monitor| {
        let t_lines: Vec<WienerLinienLine> = monitor.lines.to_vec();

        for t_line in t_lines.iter() {
            for dep in t_line.departures.departure.iter() {
                departures.push(Departure::from_wiener_linien_api(
                    t_line,
                    &dep.departure_time.time_planned,
                    &dep.departure_time.time_real,
                    &dep.departure_time.countdown,
                    &monitor.locationStop.properties.title,
                ))
            }
        }
    });

    let traffic_info = response_trafficinfo_json.and_then(|traffic_info_json| {
        traffic_info_json
            .iter()
            .map(|traffic_info_value| serde_json::from_value(traffic_info_value.to_owned()))
            .collect::<Result<Vec<WienerLinienTrafficInfo>, _>>()
            .ok()
    });

    departures.sort();

    Ok((departures, traffic_info))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (departures, traffic_info) = make_api_request().await.unwrap();
        if traffic_info.is_some() {
            traffic_info.unwrap().iter().for_each(|info| {
                println!(
                    "TRAFFIC INFO! TITLE: {} DESCRIPTION:{} PRIORITY:{}",
                    info.title, info.description, info.priority
                );
            })
        }

        let mut iter = departures.iter();
        for _ in 0..5 {
            let odep = iter.next();
            if odep.is_some() {
                let dep = odep.unwrap();
                println!(
                    "Dep: in {} min  line {} type {:?} to {} from station {}",
                    dep.countdown,
                    dep.line.name,
                    dep.line.vehicle_type,
                    dep.destination_name,
                    dep.station_name
                );
            }
        }
        sleep(Duration::from_secs(10));
    }
}
