use serde_json::Value;
use std::vec;
use thiserror::Error;

const STATION_IDS: [i32; 18] = [
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
    url: String,
    traffic_info: String,
    stop_id: Vec<i32>,
}

impl WienerLinienAPIRequest {
    fn to_req_url(&self) -> String {
       format!(
            "{}?activateTrafficInfo={}{}",
            self.url,
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

#[derive(Debug, Clone)]
struct WienerLinienLocationStop {
    _coordinates: [f64; 2],
    title: String,
}

#[derive(Debug, Clone)]
struct WienerLinienLine {
    name: String,
    destination: String,
    vehicle_type: String,
    departures: Vec<WienerLinienLineDeparture>,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
struct WienerLinienLineDeparture {
    timePlanned: iso8601::DateTime,
    timeReal: iso8601::DateTime,
    timeRealIsPresent: bool,
    countdown: i64,
}

#[derive(Clone)]
struct WienerLinienTrafficInfo {
    priority: String,
    title: String,
    description: String,
}

impl WienerLinienTrafficInfo {
    /// - Assemble WienerLinienTrafficInfo object from JSON API output
    /// - Expects an element from the "trafficInfos" array
    fn assemble_from_json(input: &serde_json::Value) -> Self {
        Self {
            priority: input["priority"].as_str().unwrap().to_string(),
            description: input["description"].as_str().unwrap().to_string(),
            title: input["title"].as_str().unwrap().to_string(),
        }
    }
}

impl WienerLinienLocationStop {
    /// - Assemble WienerLinienLocationStop object from JSON API output
    /// - Expects an index from the monitors array
    fn assemble_from_json(input: &serde_json::Value) -> Self {
        let location_stop_value = input["locationStop"].clone();
        let coords_value = location_stop_value["geometry"]["coordinates"]
            .as_array()
            .unwrap();
        Self {
            _coordinates: [
                coords_value[0].as_f64().unwrap(),
                coords_value[1].as_f64().unwrap(),
            ],
            title: location_stop_value["properties"]["title"]
                .as_str()
                .unwrap()
                .to_string(),
        }
    }
}

impl WienerLinienLineDeparture {
    /// - Assemble WienerLinienLineDeparture object from JSON API output
    /// - Expects Index from `lines.departures.departure` array
    fn assemble_from_json(input: &serde_json::Value) -> Self {
        let departure_value = input["departureTime"].clone();
        let departure_time_real: iso8601::DateTime;
        let departure_time_real_present: bool;
        if departure_value["timeReal"].is_string() {
            departure_time_real =
                iso8601::datetime(departure_value["timeReal"].as_str().unwrap()).unwrap();
            departure_time_real_present = true;
        } else {
            departure_time_real = iso8601::datetime("2000-01-01T00:00").unwrap();
            departure_time_real_present = false;
        }
        Self {
            timePlanned: iso8601::datetime(departure_value["timePlanned"].as_str().unwrap())
                .unwrap(),
            timeReal: departure_time_real,
            timeRealIsPresent: departure_time_real_present,
            countdown: departure_value["countdown"].as_i64().unwrap(),
        }
    }
}

impl WienerLinienLine {
    /// - Assemble WienerLinienLine object from JSON API output
    /// - Expects element from `monitors[].lines` array
    fn assemble_from_json(input: &serde_json::Value) -> Self {
        let t_name = input["name"].as_str().unwrap();
        let t_vtype = input["type"].as_str().unwrap();
        let t_destination = input["towards"].as_str().unwrap();
        let departure_raw_array = input["departures"]["departure"].as_array().unwrap();
        let mut departure_parsed_array: Vec<WienerLinienLineDeparture> = vec![];
        departure_raw_array.iter().for_each(|x| {
            departure_parsed_array.push(WienerLinienLineDeparture::assemble_from_json(x));
        });

        departure_parsed_array.sort_by(|a, b| a.countdown.cmp(&b.countdown));
        Self {
            name: t_name.to_string(),
            vehicle_type: t_vtype.to_string(),
            departures: departure_parsed_array,
            destination: t_destination.to_string(),
        }
    }
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
    time: iso8601::DateTime,
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
        t_line: WienerLinienLine,
        t_time: iso8601::DateTime,
        t_countdown: i64,
        t_station_name: String,
    ) -> Self {
        Departure {
            line: Line::from_wiener_linien_line(&t_line),
            time: t_time,
            countdown: t_countdown,
            destination_name: t_line.destination,
            station_name: t_station_name,
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
            && self.time == other.time
    }
}

impl PartialEq for Line {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.vehicle_type == other.vehicle_type
    }
}

async fn get_data_from_api(req: &WienerLinienAPIRequest) -> Result<String, reqwest::Error> {
    let res = reqwest::get(req.to_req_url()).await;

    if res.is_err() {
        return Err(res.err().unwrap());
    }

    return res?.text().await;
}

async fn make_api_request(
) -> Result<(Vec<Departure>, Option<Vec<WienerLinienTrafficInfo>>), ApiRequestError> {
    let reqobj = WienerLinienAPIRequest {
        url: "http://www.wienerlinien.at/ogd_realtime/monitor/".to_string(),
        traffic_info: "stoerunglang".to_string(),
        stop_id: STATION_IDS.to_vec(),
    };

    let response_text = match get_data_from_api(&reqobj).await {
        Ok(text) => text,
        Err(e) => return Err(ApiRequestError::ApiReqFailed(e)),
    };

    let response_json: Value = match serde_json::from_str(&response_text) {
        Ok(json) => json,
        Err(e) => return Err(ApiRequestError::JsonParsingFailed(e)),
    };

    let response_trafficinfo_json = response_json["data"]["trafficInfos"].as_array();

    let response_monitors_json = response_json["data"]["monitors"]
        .as_array()
        .ok_or(ApiRequestError::MissingField("monitors".to_string()))?;

    let mut wl_monitors: Vec<WienerLinienMonitor> = vec![];
    response_monitors_json.iter().try_for_each(|monitor| {
        let station = WienerLinienLocationStop::assemble_from_json(monitor);
        let mut v_lines: Vec<WienerLinienLine> = vec![];
        if let Some(arr_lines) = monitor["lines"].as_array() {
            arr_lines
                .iter()
                .for_each(|line| v_lines.push(WienerLinienLine::assemble_from_json(line)));
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
        let mut t_lines: Vec<WienerLinienLine> = vec![];
        monitor.lines.iter().for_each(|line| {
            t_lines.push(line.to_owned());
        });
        t_lines.iter().for_each(|t_line| {
            t_line.to_owned().departures.iter().for_each(|dep| {
                departures.push(Departure::from_wiener_linien_api(
                    t_line.to_owned(),
                    {
                        if dep.timeRealIsPresent {
                            dep.timeReal
                        } else {
                            dep.timePlanned
                        }
                    },
                    dep.countdown,
                    monitor.locationStop.title.to_owned(),
                ))
            })
        })
    });

    let mut traffic_info: Option<Vec<WienerLinienTrafficInfo>> = None;
    if let Some(response_trafficinfo) = response_trafficinfo_json {
        traffic_info = Some(
            response_trafficinfo
            .iter()
            .map(WienerLinienTrafficInfo::assemble_from_json)
            .collect() 
        );
    }
    departures.sort();

    Ok((departures, traffic_info))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // clear
        print!("\x1B[2J\x1B[1;1H");
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
    }
}
