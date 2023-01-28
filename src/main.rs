use serde_json::to_string_pretty;
use serde_json::Value;

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

struct WienerLinienAPIRequest {
    url: String,
    traffic_info: String,
    stop_id: Vec<i32>,
}

impl WienerLinienAPIRequest {
    fn to_req_url(self) -> String {
        return format!(
            "{}?activeTrafficInfo={}{}",
            self.url,
            self.traffic_info,
            self.stop_id
                .iter()
                .map(|x| "&stopId=".to_string() + &x.to_string())
                .collect::<String>()
        );
    }
}

#[allow(non_snake_case)]
struct WienerLinienMonitor {
    locationStop: WienerLinienLocationStop,
    lines: Vec<WienerLinienLine>,
}

struct WienerLinienLocationStop {
    coordinates: [f64; 2],
    title: String,
}

struct WienerLinienLine {
    name: String,
    vehicle_type: String,
    departures: Vec<WienerLinienLineDeparture>,
}

#[allow(non_snake_case)]
struct WienerLinienLineDeparture {
    timePlanned: iso8601::DateTime,
    timeReal: iso8601::DateTime,
    countdown: i64,
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
            coordinates: [coords_value[0].as_f64().unwrap(), coords_value[1].as_f64().unwrap()],
            title: location_stop_value["properties"]["title"].as_str().unwrap().to_string(),
        }
    }
}

impl WienerLinienLineDeparture {
    /// - Assemble WienerLinienLineDeparture object from JSON API output
    /// - Expects Index from `lines.departures.departure` array
    fn assemble_from_json(input: &serde_json::Value) -> Self {
        let departure_value = input["departureTime"].clone();
        Self {
            timePlanned: iso8601::datetime(departure_value["timePlanned"].as_str().unwrap())
                .unwrap(),
            timeReal: iso8601::datetime(departure_value["timeReal"].as_str().unwrap()).unwrap(),
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
        let departure_raw_array = input["departures"]["departure"].as_array().unwrap();
        let mut departure_parsed_array: Vec<WienerLinienLineDeparture> = vec![];
        departure_raw_array.iter().for_each(|x| {
            departure_parsed_array.push(WienerLinienLineDeparture::assemble_from_json(&x));
        });

        Self {
            name: t_name.to_string(),
            vehicle_type: t_vtype.to_string(),
            departures: departure_parsed_array,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exampleobj = WienerLinienAPIRequest {
        url: "http://www.wienerlinien.at/ogd_realtime/monitor".to_string(),
        traffic_info: "stoerunglang".to_string(),
        stop_id: STATION_IDS.to_vec(),
    };

    let exampleobj_url = exampleobj.to_req_url();

    let response_txt = reqwest::get(exampleobj_url).await?.text().await?;
    let response_json: Value = serde_json::from_str(&response_txt).expect("JSON PARSING FAILED");

    println!("{}", to_string_pretty(&response_json)?);

    let monitors = response_json["data"]["monitors"].clone();

    let example_station = WienerLinienLocationStop::assemble_from_json(&monitors[0]);
    let example_line = WienerLinienLine::assemble_from_json(&monitors[0]["lines"][0]);
    let example_monitor = WienerLinienMonitor {
        lines: vec![example_line],
        locationStop: example_station,
    };

    println!(
        "Monitor: \n Station: \n\tName: {}\n\tCoordinates: X {} Y {} \nLines: \n\tName: {}\n\tType: {}\n\tNext departure: \n\t\tPlanned departure: {}\n\t\tActual departure: {}\n\t\tCountdown: {}",
        example_monitor.locationStop.title, example_monitor.locationStop.coordinates[0], example_monitor.locationStop.coordinates[1], example_monitor.lines[0].name, example_monitor.lines[0].vehicle_type, example_monitor.lines[0].departures[0].timePlanned.to_string(), example_monitor.lines[0].departures[0].timeReal.to_string(), example_monitor.lines[0].departures[0].countdown);
    return Ok(());
}
