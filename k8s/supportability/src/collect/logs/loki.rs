use crate::{collect::utils::write_to_log_file, log};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};

/// Loki endpoint to query for logs
const ENDPOINT: &str = "/loki/api/v1/query_range";

const SERVICE_NAME: &str = "loki";

/// Possible errors can occur while interacting with Loki service
#[derive(Debug)]
pub(crate) enum LokiError {
    ReqError(reqwest::Error),
    IOError(std::io::Error),
}

impl From<reqwest::Error> for LokiError {
    fn from(e: reqwest::Error) -> LokiError {
        LokiError::ReqError(e)
    }
}

impl From<std::io::Error> for LokiError {
    fn from(e: std::io::Error) -> LokiError {
        LokiError::IOError(e)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StreamMetaData {
    #[serde(rename = "hostname")]
    host_name: String,
    #[serde(rename = "pod")]
    pod_name: String,
    #[serde(rename = "container")]
    container_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StreamContent {
    #[serde(rename = "stream")]
    stream_metadata: StreamMetaData,
    values: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Data {
    result: Vec<StreamContent>,
}

// Response structure obtained from Loki after making http request
#[derive(Serialize, Deserialize, Clone, Debug)]
struct LokiResponse {
    status: String,
    data: Data,
}

type SinceTime = u128;

impl LokiResponse {
    // fetch last stream log epoch timestamp in nanoseconds
    fn get_last_stream_unix_time(&self) -> SinceTime {
        let unix_time = match self.data.result.last() {
            Some(last_stream) => last_stream
                .values
                .last()
                .unwrap_or(&vec![])
                .get(0)
                .unwrap_or(&"0".to_string())
                .parse::<SinceTime>()
                .unwrap_or(0),
            None => {
                return 0;
            }
        };
        unix_time
    }
}

// Determines the sort order of logs
#[derive(Debug, Clone)]
enum LogDirection {
    Forward,
}

impl LogDirection {
    fn as_string(&self) -> String {
        match self {
            LogDirection::Forward => "forward".to_string(),
        }
    }
}

/// Http client to interact with Loki (a log management system)
/// to fetch historical log information
#[derive(Debug, Clone)]
pub(crate) struct LokiClient {
    // Address of Loki service
    uri: String,
    // Endpoint of Loki logs service
    logs_endpoint: String,
    // Defines period from which logs needs to collect
    since: SinceTime,
    // Determines the sort order of logs. Supported values are "forward" or "backward".
    // Defaults to forward
    direction: LogDirection,
    // maximum number of entries to return on one http call
    limit: u64,
    // specifies the timeout value to interact with Loki service
    timeout: humantime::Duration,
}

impl LokiClient {
    /// Instantiate new instance of Http Loki client
    pub(crate) fn new(
        uri: String,
        since: humantime::Duration,
        timeout: humantime::Duration,
    ) -> Self {
        LokiClient {
            uri,
            since: get_epoch_unix_time(since),
            logs_endpoint: ENDPOINT.to_string(),
            direction: LogDirection::Forward,
            limit: 3000,
            timeout,
        }
    }

    /// fetch_and_dump_logs will do the following steps:
    /// 1. Creates poller to interact with Loki service based on provided arguments
    ///     1.1. Use poller to fetch all available logs
    ///     1.2. Write fetched logs into file
    ///     Continue above steps till extraction all logs
    pub(crate) async fn fetch_and_dump_logs(
        &self,
        label_selector: String,
        container_name: String,
        host_name: Option<String>,
        service_dir: PathBuf,
    ) -> Result<(), LokiError> {
        // Build query params: Convert label selector into Loki supported query field
        // Below snippet convert app=mayastor,openebs.io/storage=mayastor into
        //  app="mayastor",openebs_io_storage="mayastor"(Loki supported values)
        let mut label_filters: String = label_selector
            .split(',')
            .into_iter()
            .map(|key_value_pair| {
                let pairs = key_value_pair.split('=').collect::<Vec<&str>>();
                format!("{}=\"{}\",", pairs[0], pairs[1])
                    .replace(".", "_")
                    .replace("/", "_")
            })
            .collect::<String>();
        if !label_filters.is_empty() {
            label_filters.pop();
        }
        let (file_name, new_query_field) = match host_name {
            Some(host_name) => {
                let file_name = format!("{}-{}-{}.log", host_name, SERVICE_NAME, container_name);
                let new_query_field = format!(
                    "{{{},container=\"{}\",hostname=~\"{}.*\"}}",
                    label_filters, container_name, host_name
                );
                (file_name, new_query_field)
            }
            None => {
                let file_name = format!("{}-{}.log", SERVICE_NAME, container_name);
                let new_query_field =
                    format!("{{{},container=\"{}\"}}", label_filters, container_name);
                (file_name, new_query_field)
            }
        };
        let encoded_query = urlencoding::encode(&new_query_field);
        let query_params = format!(
            "?query={}&limit={}&direction={}",
            encoded_query,
            self.limit,
            self.direction.as_string()
        );

        let mut poller = LokiPoll {
            uri: self.uri.clone(),
            endpoint: self.logs_endpoint.clone(),
            since: self.since,
            query_params,
            next_start_epoch_timestamp: 0,
            timeout: self.timeout,
        };
        let mut is_written = false;
        let file_path = service_dir.join(file_name.clone());
        let mut log_file: std::fs::File = std::fs::File::create(file_path.clone())?;

        loop {
            let result = match poller.poll_next().await {
                Ok(value) => match value {
                    Some(v) => v,
                    None => {
                        break;
                    }
                },
                Err(e) => {
                    if !is_written {
                        if let Err(e) = std::fs::remove_file(file_path) {
                            log(format!(
                                "[Warning] Failed to remove empty historic log file {}",
                                e
                            ));
                        }
                    }
                    write_to_log_file(format!("[Warning] While fetching logs from Loki {:?}", e))?;
                    return Err(e);
                }
            };
            is_written = true;
            for msg in result.iter() {
                write!(log_file, "{}", msg)?;
            }
        }
        Ok(())
    }
}

fn get_epoch_unix_time(since: humantime::Duration) -> SinceTime {
    Utc::now().timestamp_nanos() as SinceTime - since.as_nanos()
}

struct LokiPoll {
    uri: String,
    endpoint: String,
    since: SinceTime,
    timeout: humantime::Duration,
    query_params: String,
    next_start_epoch_timestamp: SinceTime,
}

impl LokiPoll {
    // poll_next will extract response from Loki service and perform following actions:
    // 1. Get last log epoch timestamp
    // 2. Extract logs from response
    async fn poll_next(&mut self) -> Result<Option<Vec<String>>, LokiError> {
        let mut start_time = self.since;
        if self.next_start_epoch_timestamp != 0 {
            start_time = self.since;
        }
        let request_str = format!(
            "{}{}{}&start={}",
            self.uri, self.endpoint, self.query_params, start_time
        );

        // Build client & make a request to Loki
        // TODO: Test timeouts when Loki service is dropped unexpectedly
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(self.timeout.as_secs()))
            .build()?;
        let loki_response: LokiResponse = client.get(request_str).send().await?.json().await?;
        if loki_response.status == "success" && loki_response.data.result.is_empty() {
            return Ok(None);
        }
        let last_unix_time = loki_response.get_last_stream_unix_time();
        if last_unix_time == 0 {
            return Ok(None);
        }
        // Next time when poll_next is invoked it will continue to fetch logs after last timestamp
        // TODO: Do we need to just add 1 nanosecond instead of 1 mill second?
        self.since = last_unix_time + (1000000);
        let logs = loki_response
            .data
            .result
            .iter()
            .map(|stream| -> Vec<String> {
                stream
                    .values
                    .iter()
                    .map(|value| value.get(1).unwrap_or(&"".to_string()).to_owned())
                    .filter(|val| !val.is_empty())
                    .collect::<Vec<String>>()
            })
            .flatten()
            .collect::<Vec<String>>();
        Ok(Some(logs))
    }
}
