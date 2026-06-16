use reqwest::{ Error };
use std::collections::HashMap;
use chrono::prelude::*;
use geo::{ Point };
use std::string::ParseError;

pub mod Entities {
    use serde::{ Serialize, Deserialize };
    use geo::{ Distance, Haversine, Point };
    use std::fmt;
    use std::collections::HashMap;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct Geolocation {
        pub latitude: Option<f64>,
        pub longitude: Option<f64>,
    }

    #[derive(Serialize)]
    #[derive(Clone)]
    pub enum Direction {
        EastWest,
        WestEast,
        NorthSouth,
        SouthNorth,
    }
    impl fmt::Display for Direction {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", match self {
                Self::EastWest => "Westbound",
                Self::WestEast => "Eastbound",
                Self::SouthNorth => "Northbound",
                Self::NorthSouth => "Southbound",
            })
        }
    }
    pub struct Itinerary {
        pub direction: Direction,
        pub code: String,
        pub direction_int: i32,
    }
    pub struct Stop {
        pub code: String,
        name: String,
        address: String,
        postal_code: i32,
        line_code: String,
        pub itineraries: [Itinerary; 2],
        pub coordinates: Geolocation,
    }
    impl Stop {
        pub fn new(
            code: &str,
            name: &str,
            address: &str,
            postal_code: i32,
            line_code: &str,
            itineraries: [Itinerary; 2],
            coordinates: Geolocation
        ) -> Stop {
            Stop {
                code: code.to_string(),
                name: name.to_string(),
                address: address.to_string(),
                postal_code: postal_code,
                line_code: line_code.to_string(),
                itineraries: itineraries,
                coordinates: coordinates,
            }
        }
        pub fn distance_from_point(&self, another_point: Point) -> f64 {
            // We first get distance between stop and geolocation
            let p2 = Point::new(
                self.coordinates.longitude.expect("longitude must be set"),
                self.coordinates.latitude.expect("latitude must be set")
            );

            return Haversine.distance(another_point, p2);
        }
    }

    #[derive(Serialize)]
    #[derive(Clone)]
    pub struct ResultDirection {
        pub direction: Direction,
        pub next_tram_time: Option<String>,
        pub current_tram_time: Option<String>,
        pub last_stop: String,
    }

    #[derive(Serialize)]
    pub struct ResultCalculation {
        pub distance_to_stop: String, // walking distance to the stop
        pub times: Vec<ResultDirection>,
        pub timestamp: String, //timestamp of result
    }

    #[derive(Serialize)]
    #[derive(Deserialize)]
    pub struct ResultStop {
        #[serde(alias = "codStop")]
        code: String,
        #[serde(alias = "shortCodStop")]
        short_code: String,
        name: String,
        park: i32,
        #[serde(alias = "nightLinesService")]
        night_line_service: i32,
    }
    #[derive(Serialize)]
    #[derive(Deserialize)]
    pub struct Line {
        #[serde(alias = "codLine")]
        pub code_line: String,
        #[serde(alias = "shortDescription")]
        short_description: String,
        #[serde(alias = "codMode")]
        code_mode: String,
        #[serde(alias = "updateDate")]
        update_date: String,
        #[serde(alias = "updateKmlDate")]
        update_kml_date: String,
        #[serde(alias = "nightService")]
        night_service: i32,
        #[serde(alias = "shortItinerary")]
        short_itinerary: HashMap<String, u32>,
        #[serde(alias = "companyCode")]
        company_code: String,
    }

    #[derive(Serialize)]
    #[derive(Deserialize)]
    pub struct LineResult {
        line: Line,
        pub direction: i32,
        pub destination: String,
        #[serde(alias = "destinationStop")]
        destination_stop: ResultStop,
        pub time: String, // this is the train arriving to the stop time
        #[serde(alias = "codVehicle")]
        code_vehicle: String,
        #[serde(alias = "codIssue")]
        code_issue: String,
    }

    #[derive(Serialize)]
    #[derive(Deserialize)]
    pub struct TimeResult {
        #[serde(alias = "Time")]
        pub time: Vec<LineResult>,
    }

    #[derive(Serialize)]
    #[derive(Deserialize)]
    pub struct StopTime {
        #[serde(alias = "actualDate")]
        pub actual_date: String,
        stop: ResultStop,
        pub times: TimeResult,
    }

    #[derive(Serialize)]
    #[derive(Deserialize)]
    pub struct CRTMResult {
        #[serde(alias = "stopTimes")]
        pub stop_times: StopTime,
    }
}

pub fn get_result_direction_from_itineraries(
    stop_instance: &Entities::Stop,
    response: &Entities::CRTMResult,
    result_times: &mut Vec<Entities::ResultDirection>
) -> Result<(), ParseError> {
    let mut earliest_time_in_direction: Option<DateTime<FixedOffset>> = None;
    let mut next_train_time_in_direction: Vec<DateTime<FixedOffset>> = Vec::new();
    let mut destiny: Option<String> = None;
    for itinerary in &stop_instance.itineraries {
        for time_direction in &response.stop_times.times.time {
            if itinerary.direction_int == time_direction.direction {
                destiny = Some(time_direction.destination.clone());
                let time_direction_parse_result = time_direction.time
                    .clone()
                    .parse::<DateTime<FixedOffset>>();
                let time_direction_datetime = match time_direction_parse_result {
                    Ok(line_result) => line_result,
                    Err(_) => todo!(),
                };
                if earliest_time_in_direction.is_none() {
                    earliest_time_in_direction = Some(time_direction_datetime.clone());
                } else {
                    if
                        time_direction_datetime.timestamp() <
                        earliest_time_in_direction.unwrap().timestamp()
                    {
                        earliest_time_in_direction = Some(time_direction_datetime.clone());
                    } else {
                        next_train_time_in_direction.push(time_direction_datetime);
                    }
                }
            }
        }
        result_times.push(Entities::ResultDirection {
            direction: itinerary.direction.clone(),
            next_tram_time: Some(next_train_time_in_direction[0].to_rfc2822()),
            current_tram_time: Some(earliest_time_in_direction.unwrap().to_rfc2822()),
            last_stop: destiny.clone().unwrap(),
        });
    }
    Ok(())
}

pub async fn get_configured_stop_data(
    stop_instance: Option<&Entities::Stop>,
    current_geo: &Entities::Geolocation
) -> Result<Entities::ResultCalculation, Error> {
    let p1 = Point::new(current_geo.longitude.unwrap(), current_geo.latitude.unwrap());
    let stop = stop_instance.unwrap();

    let distance = stop.distance_from_point(p1);

    let resp: Entities::CRTMResult = reqwest
        ::get(
            format!(
                "https://www.crtm.es/widgets/api/GetStopsTimes.php?codStop={}&type={}&orderBy={}&stopTimesByIti={}",
                stop.code,
                "0",
                "2",
                stop.itineraries[0].code
            )
        ).await?
        .json().await?;

    let mut result_times: Vec<Entities::ResultDirection> = Vec::new();

    get_result_direction_from_itineraries(&stop_instance.unwrap(), &resp, &mut result_times);

    let timestamp = match resp.stop_times.actual_date.clone()
                    .parse::<DateTime<FixedOffset>>(){
        Ok(parse_result) => parse_result.to_rfc2822(),
        Err(_) => todo!(),
    };

    let result = Entities::ResultCalculation {
        distance_to_stop: format!("{}", distance),
        times: result_times.clone(),
        timestamp: timestamp, //timestamp of result
    };
    

    println!(
        "{}",
        format!(
            "TEST Direction: {}  current_tram_time: {}  next_tram_time: {} distance_to_stop: {}",
            &result.times[0].direction,
            &result.times[0].current_tram_time.clone().unwrap(),
            &result.times[0].next_tram_time.clone().unwrap(),
            &distance
        )
    );

    return Ok(result);
}
