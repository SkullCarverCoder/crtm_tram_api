mod logic;
use std::env;
use std::sync::{ LazyLock };
use std::collections::HashMap;
use lambda_http::{ Error, Response, Request };
use lambda_http::ext::request::{ JsonPayloadError };
use crate::lambda_http::RequestPayloadExt;
use lambda_http::http::{ Method };
use serde::{ Serialize, Deserialize };

#[derive(Serialize, Deserialize)]
struct DataRequest {
    stopname: String,
    geolocation: logic::Entities::Geolocation,
}

#[derive(Serialize, Debug)]
struct DataRequestError {
    error: String,
}

pub trait Authorization {
    fn is_valid_request(&self) -> bool;
}

impl Authorization for Request {
    fn is_valid_request(&self) -> bool {
        let headers = self.headers();
        if headers.is_empty() {
            return false;
        }
        let api_key_header = headers.get("x-api-key");
        if api_key_header.is_none() {
            return false;
        }
        match env::var("API_KEY") {
            Ok(key) => {
                return *api_key_header.unwrap() == *key;
            }
            Err(_e) => {
                return false;
            }
        }
    }
}

const COMPATIBLE_STOPS: LazyLock<HashMap<String, logic::Entities::Stop>> = LazyLock::new(||
    HashMap::from([
        (
            "INFANTE_DON_LUIS".to_string(),
            logic::Entities::Stop::new(
                "10_37",
                "INFANTE DON LUIS",
                "Av.Infante Don Luis",
                28660,
                "10__ML3___",
                [
                    logic::Entities::Itinerary {
                        code: "10__ML3____2__IT_1".to_string(),
                        direction: logic::Entities::Direction::WestEast,
                        direction_int: 2,
                    },
                    logic::Entities::Itinerary {
                        code: "10__ML3____1__IT_1".to_string(),
                        direction: logic::Entities::Direction::EastWest,
                        direction_int: 1,
                    },
                ],
                logic::Entities::Geolocation {
                    latitude: Some(40.40595),
                    longitude: Some(-3.89741),
                }
            ),
        ),
        (
            "COLONIA_DE_JARDIN".to_string(),
            logic::Entities::Stop::new(
                "10_10",
                "COLONIA DE JARDIN",
                "COLONIA DE JARDIN",
                28024,
                "4__10___",
                [
                    logic::Entities::Itinerary {
                        code: "4__10___1".to_string(),
                        direction: logic::Entities::Direction::NorthSouth,
                        direction_int: 1,
                    },
                    logic::Entities::Itinerary {
                        code: "4__10___2".to_string(),
                        direction: logic::Entities::Direction::SouthNorth,
                        direction_int: 2,
                    },
                ],
                logic::Entities::Geolocation {
                    latitude: Some(40.39698),
                    longitude: Some(-3.77456),
                }
            ),
        ),
    ])
);

fn request_is_from_valid_method(event: &Request) -> bool {
    if *event.method() == Method::POST {
        return true;
    }
    return false;
}
fn request_to_data_request(event: &Request) -> Result<DataRequest, Error> {
    match event.json::<DataRequest>() {
        Ok(Some(data_request)) => Ok(data_request),
        Ok(None) => panic!("No payload"),
        Err(JsonPayloadError::Parsing(err)) => {
            if err.is_data() {
                panic!("payload does not match DataRequest schema: {err:?}");
            }
            if err.is_syntax() {
                panic!("payload is invalid json: {err:?}");
            }
            panic!("failed to parse json: {err:?}")
        }
        Err(_) => todo!(),
    }
}

async fn process_request(
    data: &DataRequest
) -> Result<logic::Entities::ResultCalculation, DataRequestError> {
    if
        !COMPATIBLE_STOPS.contains_key(&data.stopname.to_uppercase()) ||
        !COMPATIBLE_STOPS.contains_key(&data.stopname.to_uppercase())
    {
        return Err(DataRequestError {
            error: "Stop name couldn't be found".to_string(),
        });
    } else {
        if !data.geolocation.latitude.is_some() && !data.geolocation.longitude.is_some() {
            return Err(DataRequestError {
                error: "Latitude and longitude must be provided".to_string(),
            });
        }
        let result = logic
            ::get_configured_stop_data(
                COMPATIBLE_STOPS.get(&data.stopname.to_uppercase()),
                &data.geolocation
            ).await
            .unwrap();
        return Ok(result);
    }
}

pub(crate) async fn function_handler(event: Request) -> Result<Response<String>, Error> {
    if !request_is_from_valid_method(&event) {
        let error_body = serde_json::to_string(
            &(DataRequestError {
                error: "Method is not allowed".to_string(),
            })
        )?;
        let bad_resp = Response::builder()
            .status(405)
            .header("content-type", "application/json")
            .body(error_body)
            .map_err(Box::new)?;

        return Ok(bad_resp);
    }
    if !event.is_valid_request() {
        let error_body = serde_json::to_string(
            &(DataRequestError {
                error: "Request is invalid".to_string(),
            })
        )?;
        let bad_resp = Response::builder()
            .status(401)
            .header("content-type", "application/json")
            .body(error_body)
            .map_err(Box::new)?;
        return Ok(bad_resp);
    }

    let data_request = request_to_data_request(&event)?;

    let response = process_request(&data_request).await.unwrap();

    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(serde_json::to_string(&response)?)
        .map_err(Box::new)?;
    Ok(resp)
}
