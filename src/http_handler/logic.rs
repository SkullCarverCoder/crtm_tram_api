use chrono::prelude::*;
use geo::Point;
use std::error::Error;
use std::fmt;
use std::string::ParseError;

pub mod Entities {
    use geo::{Distance, Haversine, Point};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fmt;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct Geolocation {
        pub latitude: Option<f64>,
        pub longitude: Option<f64>,
    }

    #[derive(Serialize, Clone)]
    pub enum Direction {
        EastWest,
        WestEast,
        NorthSouth,
        SouthNorth,
    }
    impl fmt::Display for Direction {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "{}",
                match self {
                    Self::EastWest => "Westbound",
                    Self::WestEast => "Eastbound",
                    Self::SouthNorth => "Northbound",
                    Self::NorthSouth => "Southbound",
                }
            )
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
            coordinates: Geolocation,
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
                self.coordinates.latitude.expect("latitude must be set"),
            );

            return Haversine.distance(another_point, p2);
        }
    }

    #[derive(Serialize, Clone)]
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

    #[derive(Serialize, Deserialize, Clone)]
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
    #[derive(Serialize, Deserialize, Clone)]
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

    #[derive(Serialize, Deserialize, Clone)]
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

    #[derive(Serialize, Deserialize, Clone)]
    pub struct TimeResult {
        #[serde(default, alias = "Time")]
        pub time: Vec<LineResult>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct StopTime {
        #[serde(alias = "actualDate")]
        pub actual_date: String,
        stop: ResultStop,
        pub times: Option<TimeResult>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct CRTMResult {
        #[serde(alias = "stopTimes")]
        pub stop_times: StopTime,
    }
}

pub fn get_result_direction_from_itineraries(
    stop_instance: &Entities::Stop,
    response: &Entities::CRTMResult,
    result_times: &mut Vec<Entities::ResultDirection>,
) -> Result<(), ParseError> {
    for itinerary in &stop_instance.itineraries {
        let times_result = match &response.stop_times.times {
            None => continue,
            Some(t) => t,
        };

        if times_result.time.is_empty() {
            continue;
        }

        let mut earliest_time_in_direction: Option<DateTime<FixedOffset>> = Some(
            response
                .stop_times
                .actual_date
                .parse::<DateTime<FixedOffset>>()
                .unwrap(),
        );
        let mut next_train_time_in_direction: Vec<DateTime<FixedOffset>> = Vec::new();
        let mut destiny: Option<String> = None;

        for time_direction in &times_result.time {
            if itinerary.direction_int == time_direction.direction {
                destiny = Some(time_direction.destination.clone());
                let time_direction_datetime =
                    match time_direction.time.clone().parse::<DateTime<FixedOffset>>() {
                        Ok(line_result) => line_result,
                        Err(_) => continue,
                    };
                if let Some(earliest) = earliest_time_in_direction.as_ref() {
                    if time_direction_datetime.timestamp() < earliest.timestamp() {
                        earliest_time_in_direction = Some(time_direction_datetime);
                    } else {
                        next_train_time_in_direction.push(time_direction_datetime);
                    }
                } else {
                    earliest_time_in_direction = Some(time_direction_datetime);
                }
            }
        }

        let current = match earliest_time_in_direction {
            Some(t) => t.to_rfc2822(),
            None => continue,
        };

        result_times.push(Entities::ResultDirection {
            direction: itinerary.direction.clone(),
            next_tram_time: next_train_time_in_direction.first().map(|t| t.to_rfc2822()),
            current_tram_time: Some(current),
            last_stop: destiny.unwrap_or_default(),
        });
    }
    Ok(())
}

#[derive(Debug)]
struct NoTimesInAPIError;

impl fmt::Display for NoTimesInAPIError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No times data returned for any itinerary")
    }
}

impl Error for NoTimesInAPIError {}

pub async fn get_configured_stop_data(
    stop_instance: Option<&Entities::Stop>,
    current_geo: &Entities::Geolocation,
) -> Result<Entities::ResultCalculation, Box<dyn std::error::Error>> {
    let p1 = Point::new(
        current_geo.longitude.unwrap(),
        current_geo.latitude.unwrap(),
    );
    let stop = stop_instance.unwrap();

    let distance = stop.distance_from_point(p1);

    let mut resp: Option<Entities::CRTMResult> = None;
    for itinerary in &stop.itineraries {
        let attempt: Entities::CRTMResult = reqwest
            ::get(
                format!(
                    "https://www.crtm.es/widgets/api/GetStopsTimes.php?codStop={}&type={}&orderBy={}&stopTimesByIti={}",
                    stop.code,
                    "0",
                    "2",
                    itinerary.code
                )
            ).await?
            .json().await?;

        let has_times = match &attempt.stop_times.times {
            Some(t) => !t.time.is_empty(),
            None => false,
        };

        if has_times {
            resp = Some(attempt);
            break;
        }
    }

    let resp = match resp {
        Some(r) => r,
        None => return Err(Box::new(NoTimesInAPIError)),
    };

    let mut result_times: Vec<Entities::ResultDirection> = Vec::new();

    get_result_direction_from_itineraries(stop, &resp, &mut result_times);

    let timestamp = match resp
        .stop_times
        .actual_date
        .clone()
        .parse::<DateTime<FixedOffset>>()
    {
        Ok(parse_result) => parse_result.to_rfc2822(),
        Err(_) => todo!(),
    };

    let result = Entities::ResultCalculation {
        distance_to_stop: format!("{}", distance),
        times: result_times.clone(),
        timestamp: timestamp, //timestamp of result
    };

    return Ok(result);
}
