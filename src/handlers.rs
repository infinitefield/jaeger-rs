//! # Jaeger API Handlers
//!
//! This module contains all HTTP request handlers for the Jaeger API endpoints.
//! The handlers interact with a Jaeger storage backend via gRPC to retrieve
//! tracing data and return it in Jaeger UI compatible JSON format.

use crate::rpc::jaeger::storage::v2::trace_reader_client::TraceReaderClient;
use crate::rpc::jaeger::storage::v2::{
    FindTracesRequest, GetOperationsRequest, GetServicesRequest, GetTraceParams, GetTracesRequest,
};
use axum::{
    Json,
    extract::State,
    extract::{Path, Query},
};
use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as};
use tonic::transport::Channel;

/// Application state containing the gRPC client for Jaeger storage backend.
///
/// This struct holds the connection to the Jaeger storage service and is
/// shared across all HTTP request handlers.
#[derive(Clone)]
pub struct AppState {
    /// gRPC client for communicating with Jaeger storage backend
    pub client: TraceReaderClient<Channel>,
}

impl AppState {
    /// Creates a new AppState with a gRPC connection to the specified URI.
    ///
    /// Uses lazy connection establishment and enables gzip compression for efficiency.
    ///
    /// # Arguments
    /// * `uri` - The URI of the Jaeger storage gRPC service (e.g., "http://localhost:4317")
    ///
    /// # Returns
    /// A new AppState instance with a lazy gRPC connection
    ///
    /// # Examples
    /// ```
    /// let state = AppState::new("http://localhost:4317".parse().unwrap());
    /// ```
    pub fn new(uri: http::Uri) -> Self {
        let channel = Channel::builder(uri.clone()).connect_lazy();
        let client = TraceReaderClient::new(channel)
            .send_compressed(tonic::codec::CompressionEncoding::Gzip);
        Self { client }
    }
}

/// Standard API response wrapper compatible with Jaeger UI expectations.
///
/// All Jaeger API endpoints return data in this consistent format with
/// pagination metadata and optional error information.
#[derive(Serialize)]
pub struct ApiResponse<T> {
    /// The actual response data
    data: T,
    /// Total number of items available (for pagination)
    total: usize,
    /// Maximum number of items requested (for pagination)
    limit: usize,
    /// Number of items skipped (for pagination)
    offset: usize,
    /// Optional list of errors that occurred during processing
    errors: Option<Vec<ApiError>>,
}

/// Represents an API error with HTTP status code and message.
#[derive(Serialize, Debug)]
pub struct ApiError {
    /// HTTP status code
    code: u16,
    /// Human-readable error message
    msg: String,
}

/// Represents a span reference in Jaeger format.
#[derive(Serialize, Clone)]
pub struct JaegerReference {
    /// Reference type ("CHILD_OF" or "FOLLOWS_FROM")
    #[serde(rename = "refType")]
    ref_type: String,
    /// Trace ID of the referenced span
    #[serde(rename = "traceID")]
    trace_id: String,
    /// Span ID of the referenced span
    #[serde(rename = "spanID")]
    span_id: String,
}

/// Represents a span in Jaeger UI format.
///
/// This struct converts OpenTelemetry span data into the format expected
/// by the Jaeger UI frontend.
#[derive(Serialize, Clone)]
pub struct JaegerSpan {
    /// Hex-encoded trace ID (16 bytes)
    #[serde(rename = "traceID")]
    trace_id: String,
    /// Hex-encoded span ID (8 bytes)
    #[serde(rename = "spanID")]
    span_id: String,
    /// Hex-encoded parent span ID (8 bytes), null if root span
    #[serde(rename = "parentSpanID")]
    parent_span_id: Option<String>,
    /// Flags field (usually 1 for sampled traces)
    flags: u32,
    /// Human-readable operation name
    #[serde(rename = "operationName")]
    operation_name: String,
    /// References to other spans (modern way to define relationships)
    ///
    /// https://github.com/jaegertracing/jaeger/blob/2f80e4e681e90a8488e84fcdb1a6e2455c48372c/internal/uimodel/converter/v1/json/from_domain.go#L90
    references: Vec<JaegerReference>,
    /// Start time in microseconds since Unix epoch
    #[serde(rename = "startTime")]
    start_time: u64,
    /// Duration in microseconds
    duration: u64,
    /// List of key-value tags/attributes on the span
    tags: Vec<JaegerTag>,
    /// List of log entries (not currently used)
    logs: Vec<serde_json::Value>,
    /// Process ID referencing the processes map in the trace
    #[serde(rename = "processID")]
    process_id: String,
}

/// Represents a key-value tag in Jaeger format.
#[derive(Serialize, Clone)]
pub struct JaegerTag {
    /// Tag key
    key: String,
    /// Tag value (converted to string)
    value: String,
}

/// Represents process information for a span (service context).
#[derive(Serialize, Clone)]
pub struct JaegerProcess {
    /// Name of the service that created this span
    #[serde(rename = "serviceName")]
    service_name: String,
    /// Process-level tags
    tags: Vec<JaegerTag>,
}

/// Represents a complete trace with all its spans.
#[derive(Serialize, Clone)]
pub struct JaegerTrace {
    /// Hex-encoded trace ID
    #[serde(rename = "traceID")]
    trace_id: String,
    /// All spans belonging to this trace
    spans: Vec<JaegerSpan>,
    /// Map of process IDs to process information
    processes: std::collections::HashMap<String, JaegerProcess>,
}

impl<T> ApiResponse<T> {
    /// Creates a new successful API response.
    ///
    /// # Arguments
    /// * `data` - The response data
    /// * `total` - Total number of items available
    /// * `limit` - Maximum number of items requested
    /// * `offset` - Number of items skipped
    fn new(data: T, total: usize, limit: usize, offset: usize) -> Self {
        Self {
            data,
            total,
            limit,
            offset,
            errors: None,
        }
    }

    /// Creates an error response from a gRPC error.
    ///
    /// # Arguments
    /// * `error` - The gRPC error
    /// * `context` - Additional context for logging
    fn from_grpc_error(error: tonic::Status, context: &str) -> Self
    where
        T: Default,
    {
        log::error!("{}: gRPC error: {}", context, error);
        Self {
            data: T::default(),
            total: 0,
            limit: 0,
            offset: 0,
            errors: Some(vec![ApiError {
                code: 500,
                msg: format!("gRPC error: {}", error),
            }]),
        }
    }

    /// Creates a not found error response.
    ///
    /// # Arguments
    /// * `message` - The error message
    fn not_found(message: String) -> Self
    where
        T: Default,
    {
        Self {
            data: T::default(),
            total: 0,
            limit: 0,
            offset: 0,
            errors: Some(vec![ApiError {
                code: 404,
                msg: message,
            }]),
        }
    }

    /// Creates a not implemented error response.
    ///
    /// # Arguments
    /// * `message` - The error message
    fn not_implemented(message: String) -> Self
    where
        T: Default,
    {
        Self {
            data: T::default(),
            total: 0,
            limit: 0,
            offset: 0,
            errors: Some(vec![ApiError {
                code: 501,
                msg: message,
            }]),
        }
    }

    /// Creates an error response with the given error.
    ///
    /// # Arguments
    /// * `error` - The API error
    fn errored(error: ApiError) -> Self
    where
        T: Default,
    {
        Self {
            data: T::default(),
            total: 0,
            limit: 0,
            offset: 0,
            errors: Some(vec![error]),
        }
    }
}

/// Extracts service name from resource attributes.
///
/// # Arguments
/// * `resource` - The OpenTelemetry resource
///
/// # Returns
/// Service name or "unknown-service" if not found
fn extract_service_name(
    resource: Option<&opentelemetry_proto::tonic::resource::v1::Resource>,
) -> String {
    resource
        .and_then(|resource| {
            resource.attributes.iter().find(|attr| attr.key == "service.name")
        })
        .and_then(|attr| attr.value.as_ref())
        .and_then(|value| {
            if let Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) = &value.value {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown-service".to_string())
}

/// Parses a trace ID from a hex string.
///
/// # Arguments
/// * `trace_id` - Hex-encoded trace ID
///
/// # Returns
/// Decoded bytes or UTF-8 bytes if hex parsing fails
fn parse_trace_id(trace_id: &str) -> Vec<u8> {
    hex::decode(trace_id).unwrap_or_else(|_| {
        log::warn!(
            "Failed to decode hex trace_id '{}', using as UTF-8 bytes",
            trace_id
        );
        trace_id.as_bytes().to_vec()
    })
}

/// Parses a timestamp from microseconds to protobuf Timestamp.
///
/// # Arguments
/// * `microseconds` - Timestamp in microseconds since Unix epoch
///
/// # Returns
/// Protobuf Timestamp
fn parse_timestamp(microseconds: i64) -> crate::rpc::google::protobuf::Timestamp {
    crate::rpc::google::protobuf::Timestamp {
        seconds: microseconds / 1_000_000,
        nanos: ((microseconds % 1_000_000) * 1000) as i32,
    }
}

/// Parses a duration from microseconds to protobuf Duration.
///
/// # Arguments
/// * `microseconds` - Duration in microseconds
///
/// # Returns
/// Protobuf Duration
fn parse_duration(microseconds: u64) -> crate::rpc::google::protobuf::Duration {
    crate::rpc::google::protobuf::Duration {
        seconds: (microseconds / 1_000_000) as i64,
        nanos: ((microseconds % 1_000_000) * 1000) as i32,
    }
}

/// Processes OpenTelemetry spans into Jaeger format.
///
/// # Arguments
/// * `resource_spans` - Iterator of resource spans from gRPC response
///
/// # Returns
/// Vector of (span, service_name) tuples
fn process_resource_spans<I>(
    resource_spans: I,
) -> Vec<(opentelemetry_proto::tonic::trace::v1::Span, String)>
where
    I: IntoIterator<Item = opentelemetry_proto::tonic::trace::v1::ResourceSpans>,
{
    let mut spans = Vec::new();
    
    for resource_span in resource_spans {
        let service_name = extract_service_name(resource_span.resource.as_ref());
        
        for scope_span in resource_span.scope_spans {
            for span in scope_span.spans {
                spans.push((span, service_name.clone()));
            }
        }
    }
    
    spans
}

/// Converts processed spans to Jaeger traces.
///
/// # Arguments
/// * `spans` - Vector of (span, service_name) tuples
///
/// # Returns
/// Vector of JaegerTrace objects
fn convert_to_jaeger_traces(
    spans: Vec<(opentelemetry_proto::tonic::trace::v1::Span, String)>,
) -> Vec<JaegerTrace> {
    let mut all_jaeger_spans = Vec::new();
    let mut all_processes = std::collections::HashMap::new();

    for (span, service_name) in &spans {
        let (jaeger_span, process_id, jaeger_process) =
            convert_span(span, service_name.clone());
        all_jaeger_spans.push(jaeger_span);
        all_processes.insert(process_id, jaeger_process);
    }

    log::debug!(
        "Converted {} spans to Jaeger format with {} unique processes",
        all_jaeger_spans.len(),
        all_processes.len()
    );

    // Group spans by trace ID to create proper trace objects
    let mut trace_map: std::collections::HashMap<String, Vec<JaegerSpan>> =
        std::collections::HashMap::new();
    for span in all_jaeger_spans {
        trace_map
            .entry(span.trace_id.clone())
            .or_default()
            .push(span);
    }

    trace_map
        .into_iter()
        .map(|(trace_id, spans)| JaegerTrace {
            trace_id,
            spans,
            processes: all_processes.clone(),
        })
        .collect()
}

/// Converts an OpenTelemetry span to Jaeger UI format.
///
/// This function transforms span data from the OpenTelemetry protobuf format
/// into the JSON structure expected by the Jaeger UI frontend. It handles:
/// - Converting nanosecond timestamps to microseconds
/// - Creating parent-child references 
/// - Transforming attributes to tags
/// - Generating process IDs based on service names
///
/// # Arguments
/// * `span` - OpenTelemetry span from the storage backend
/// * `service_name` - The service name for this span
///
/// # Returns
/// A tuple of (JaegerSpan, process_id, JaegerProcess) for building traces
fn convert_span(
    span: &opentelemetry_proto::tonic::trace::v1::Span,
    service_name: String,
) -> (JaegerSpan, String, JaegerProcess) {
    log::trace!(
        "convert_span: Converting span {} ({}) {}",
        hex::encode(&span.span_id),
        span.name,
        hex::encode(&span.parent_span_id)
    );

    // Generate a process ID based on service name (could be more sophisticated)
    let process_id = format!(
        "p{}",
        service_name
            .chars()
            .fold(0u32, |acc, c| acc.wrapping_add(c as u32))
    );

    let jaeger_process = JaegerProcess {
        service_name: service_name.clone(),
        tags: vec![],
    };

    // Create references array for parent-child relationship
    let mut references = Vec::new();
    if !span.parent_span_id.is_empty() {
        references.push(JaegerReference {
            ref_type: "CHILD_OF".to_string(),
            trace_id: hex::encode(&span.trace_id),
            span_id: hex::encode(&span.parent_span_id),
        });
    }

    let jaeger_span = JaegerSpan {
        trace_id: hex::encode(&span.trace_id),
        span_id: hex::encode(&span.span_id),
        parent_span_id: if span.parent_span_id.is_empty() {
            None
        } else {
            Some(hex::encode(&span.parent_span_id))
        },
        flags: 1, // Typically 1 for sampled traces
        operation_name: span.name.clone(),
        references,
        start_time: span.start_time_unix_nano / 1_000,
        duration: span
            .end_time_unix_nano
            .saturating_sub(span.start_time_unix_nano)
            / 1_000,
        tags: span
            .attributes
            .iter()
            .map(|attr| JaegerTag {
                key: attr.key.clone(),
                value: attr
                    .value
                    .as_ref()
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_default(),
            })
            .collect(),
        logs: Vec::new(), // No logs for now
        process_id: process_id.clone(),
    };

    log::trace!(
        "convert_span: Converted span {} with {} tags for service '{}'",
        jaeger_span.span_id,
        jaeger_span.tags.len(),
        service_name
    );
    (jaeger_span, process_id, jaeger_process)
}

/// Handler for GET /api/services
///
/// Returns a list of all services that have reported spans to the tracing backend.
/// This endpoint is used by the Jaeger UI to populate the service dropdown.
///
/// # Returns
/// JSON response containing:
/// - `data`: Array of service names (strings)
/// - Standard pagination metadata (total, limit, offset)
/// - Optional errors array if the request failed
///
/// # Example Response
/// ```json
/// {
///   "data": ["frontend", "backend", "database"],
///   "total": 3,
///   "limit": 0,
///   "offset": 0,
///   "errors": null
/// }
/// ```
pub async fn get_services(State(state): State<AppState>) -> Json<ApiResponse<Vec<String>>> {
    log::info!("get_services: Starting request");
    log::debug!("get_services: Input - No parameters");

    let mut client = state.client.clone();
    let request = tonic::Request::new(GetServicesRequest {});

    log::debug!("get_services: Sending gRPC request to storage backend");
    match client.get_services(request).await {
        Ok(response) => {
            let services = response.into_inner().services;
            log::info!("get_services: Success - Found {} services", services.len());
            log::debug!("get_services: Output - Services: {:?}", services);

            let response = ApiResponse::new(services.clone(), services.len(), 0, 0);
            Json(response)
        }
        Err(e) => {
            let response = ApiResponse::from_grpc_error(e, "get_services");
            log::debug!(
                "get_services: Output - Error response: {:?}",
                response.errors
            );
            Json(response)
        }
    }
}

/// Query parameters for the operations endpoint.
#[derive(Deserialize, Debug)]
pub struct OperationsQuery {
    /// Name of the service to get operations for
    service: String,
    /// Optional span kind filter (e.g., "server", "client")
    #[serde(rename = "spanKind")]
    span_kind: Option<String>,
}

/// Handler for GET /api/operations
///
/// Returns a list of operation names for a specified service.
/// Used by the Jaeger UI to populate the operation dropdown after a service is selected.
///
/// # Query Parameters
/// - `service`: Service name (required)
/// - `spanKind`: Optional span kind filter (e.g., "server", "client")
///
/// # Returns
/// JSON response containing operation names for the specified service
///
/// # Example Response
/// ```json
/// {
///   "data": ["GET /users", "POST /orders", "process_payment"],
///   "total": 3,
///   "limit": 0,
///   "offset": 0,
///   "errors": null
/// }
/// ```
pub async fn get_operations(
    State(state): State<AppState>,
    Query(params): Query<OperationsQuery>,
) -> Json<ApiResponse<Vec<String>>> {
    log::info!("get_operations: Starting request");
    log::debug!("get_operations: Input - {:?}", params);

    let mut client = state.client.clone();
    let request = tonic::Request::new(GetOperationsRequest {
        service: params.service.clone(),
        span_kind: params.span_kind.clone().unwrap_or_default(),
    });

    log::debug!(
        "get_operations: Sending gRPC request for service '{}'",
        params.service
    );
    match client.get_operations(request).await {
        Ok(response) => {
            let operations = response.into_inner().operations;
            let operation_names: Vec<String> = operations.into_iter().map(|op| op.name).collect();
            log::info!(
                "get_operations: Success - Found {} operations for service '{}'",
                operation_names.len(),
                params.service
            );
            log::debug!("get_operations: Output - Operations: {:?}", operation_names);

            let response = ApiResponse::new(operation_names.clone(), operation_names.len(), 0, 0);
            Json(response)
        }
        Err(e) => {
            let response = ApiResponse::from_grpc_error(
                e,
                &format!("get_operations for service '{}'", params.service),
            );
            log::debug!(
                "get_operations: Output - Error response: {:?}",
                response.errors
            );
            Json(response)
        }
    }
}

/// Query parameters for the service operations endpoint.
#[derive(Deserialize, Debug)]
pub struct ServiceOperationsQuery {
    /// Optional span kind filter (e.g., "server", "client")
    #[serde(rename = "spanKind")]
    span_kind: Option<String>,
}

/// Handler for GET /api/services/{service}/operations
///
/// Returns a list of operation names for the specified service.
/// This is an alternative REST-style endpoint to `/api/operations?service=name`.
///
/// # Path Parameters
/// - `service`: Service name
///
/// # Query Parameters
/// - `spanKind`: Optional span kind filter (e.g., "server", "client")
///
/// # Returns
/// JSON response containing operation names for the specified service
///
/// # Example Response
/// ```json
/// {
///   "data": ["GET /users", "POST /orders", "process_payment"],
///   "total": 3,
///   "limit": 0,
///   "offset": 0,
///   "errors": null
/// }
/// ```
pub async fn get_service_operations(
    State(state): State<AppState>,
    Path(service): Path<String>,
    Query(params): Query<ServiceOperationsQuery>,
) -> Json<ApiResponse<Vec<String>>> {
    log::info!("get_service_operations: Starting request");
    log::debug!(
        "get_service_operations: Input - service: '{}', params: {:?}",
        service,
        params
    );

    let mut client = state.client.clone();
    let request = tonic::Request::new(GetOperationsRequest {
        service: service.clone(),
        span_kind: params.span_kind.clone().unwrap_or_default(),
    });

    log::debug!(
        "get_service_operations: Sending gRPC request for service '{}'",
        service
    );
    match client.get_operations(request).await {
        Ok(response) => {
            let operations = response.into_inner().operations;
            let operation_names: Vec<String> = operations.into_iter().map(|op| op.name).collect();
            log::info!(
                "get_service_operations: Success - Found {} operations for service '{}'",
                operation_names.len(),
                service
            );
            log::debug!(
                "get_service_operations: Output - Operations: {:?}",
                operation_names
            );

            let response = ApiResponse::new(operation_names.clone(), operation_names.len(), 0, 0);
            Json(response)
        }
        Err(e) => {
            let response = ApiResponse::from_grpc_error(
                e,
                &format!("get_service_operations for service '{}'", service),
            );
            log::debug!(
                "get_service_operations: Output - Error response: {:?}",
                response.errors
            );
            Json(response)
        }
    }
}

/// Query parameters for the traces search endpoint.
#[serde_as]
#[derive(Deserialize, Debug)]
pub struct TracesQuery {
    /// Service name to filter traces by
    service: Option<String>,
    /// Operation name to filter traces by
    operation: Option<String>,
    /// Tag-based filters (not yet implemented)
    tags: Option<String>,
    /// Start time filter (microseconds since Unix epoch)
    start: Option<String>,
    /// End time filter (microseconds since Unix epoch)
    end: Option<String>,
    /// Minimum duration filter (microseconds)
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "minDuration")]
    min_duration: Option<String>,
    /// Maximum duration filter (microseconds)
    #[serde_as(as = "NoneAsEmptyString")]
    #[serde(rename = "maxDuration")]
    max_duration: Option<String>,
    /// Maximum number of traces to return (default: 1000)
    limit: Option<u32>,
}

/// Handler for GET /api/traces
///
/// Searches for traces based on the provided criteria.
/// This is the main endpoint used by Jaeger UI for trace search.
///
/// # Query Parameters
/// - `service`: Service name filter
/// - `operation`: Operation name filter
/// - `tags`: Tag-based filters (not yet implemented)
/// - `start`: Start time filter (microseconds since epoch)
/// - `end`: End time filter (microseconds since epoch)
/// - `minDuration`: Minimum duration filter (microseconds)
/// - `maxDuration`: Maximum duration filter (microseconds)
/// - `limit`: Maximum number of traces to return
///
/// # Returns
/// JSON response containing matching spans formatted for Jaeger UI
pub async fn get_traces(
    State(state): State<AppState>,
    Query(params): Query<TracesQuery>,
) -> Json<ApiResponse<Vec<JaegerTrace>>> {
    log::info!("get_traces: Starting request");
    log::debug!("get_traces: Input - {:?}", params);

    let mut client = state.client.clone();

    // Parse timestamps and durations
    let start_time = params
        .start
        .as_ref()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|ts| {
            log::debug!("get_traces: Parsed start time: {} microseconds", ts);
            parse_timestamp(ts)
        });

    let end_time = params
        .end
        .as_ref()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|ts| {
            log::debug!("get_traces: Parsed end time: {} microseconds", ts);
            parse_timestamp(ts)
        });

    let duration_min = params
        .min_duration
        .as_ref()
        .and_then(|d| d.parse().ok())
        .map(|d: u64| {
            log::debug!("get_traces: Parsed min duration: {} microseconds", d);
            parse_duration(d)
        });

    let duration_max = params
        .max_duration
        .as_ref()
        .and_then(|d| d.parse().ok())
        .map(|d: u64| {
            log::debug!("get_traces: Parsed max duration: {} microseconds", d);
            parse_duration(d)
        });

    let limit = params.limit.unwrap_or(1000);
    log::debug!("get_traces: Using limit: {}", limit);

    let request = tonic::Request::new(FindTracesRequest {
        query: Some(crate::rpc::jaeger::storage::v2::TraceQueryParameters {
            service_name: params.service.clone().unwrap_or_default(),
            operation_name: params.operation.clone().unwrap_or_default(),
            attributes: Vec::new(), // TODO: parse tags parameter
            start_time_min: start_time,
            start_time_max: end_time,
            duration_min,
            duration_max,
            search_depth: limit as i32,
        }),
    });

    log::debug!("get_traces: Sending gRPC find_traces request");
    match client.find_traces(request).await {
        Ok(response) => {
            log::debug!("get_traces: Received gRPC response, processing stream");
            let mut stream = response.into_inner();
            let mut all_resource_spans = Vec::new();

            while let Ok(Some(chunk)) = stream.message().await {
                log::trace!(
                    "get_traces: Processing chunk with {} resource spans",
                    chunk.resource_spans.len()
                );
                all_resource_spans.extend(chunk.resource_spans);
            }

            let traces = process_resource_spans(all_resource_spans);

            log::info!("get_traces: Success - Found {} spans", traces.len());

            let jaeger_traces = convert_to_jaeger_traces(traces);
            log::debug!(
                "get_traces: Grouped spans into {} traces",
                jaeger_traces.len()
            );

            let traces_len = jaeger_traces.len();
            let response = ApiResponse::new(jaeger_traces, traces_len, limit as usize, 0);
            Json(response)
        }
        Err(e) => {
            let response = ApiResponse::from_grpc_error(e, "get_traces");
            log::debug!("get_traces: Output - Error response: {:?}", response.errors);
            Json(response)
        }
    }
}

/// Handler for GET /api/traces/{trace_id}
///
/// Retrieves a specific trace by its ID.
/// Used by Jaeger UI when viewing trace details.
///
/// # Path Parameters
/// - `trace_id`: Hex-encoded trace ID
///
/// # Returns
/// JSON response containing the trace with all its spans
pub async fn get_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
) -> Json<ApiResponse<Vec<JaegerTrace>>> {
    log::info!("get_trace: Starting request");
    log::debug!("get_trace: Input - trace_id: '{}'", trace_id);

    let mut client = state.client.clone();

    let trace_id_bytes = parse_trace_id(&trace_id);
    log::debug!(
        "get_trace: Parsed trace_id to {} bytes",
        trace_id_bytes.len()
    );

    let request = tonic::Request::new(GetTracesRequest {
        query: vec![GetTraceParams {
            trace_id: trace_id_bytes,
            start_time: None,
            end_time: None,
        }],
    });

    log::debug!("get_trace: Sending gRPC get_traces request");
    match client.get_traces(request).await {
        Ok(response) => {
            log::debug!("get_trace: Received gRPC response, processing stream");
            let mut stream = response.into_inner();
            let mut all_resource_spans = Vec::new();

            while let Ok(Some(chunk)) = stream.message().await {
                log::trace!(
                    "get_trace: Processing chunk with {} resource spans",
                    chunk.resource_spans.len()
                );
                all_resource_spans.extend(chunk.resource_spans);
            }

            let spans = process_resource_spans(all_resource_spans);

            log::info!(
                "get_trace: Success - Found {} spans for trace '{}'",
                spans.len(),
                trace_id
            );

            let jaeger_traces = convert_to_jaeger_traces(spans);
            let trace = if let Some(mut trace) = jaeger_traces.into_iter().next() {
                // Use the original trace_id from the request to maintain consistency
                trace.trace_id = trace_id.clone();
                trace
            } else {
                JaegerTrace {
                    trace_id: trace_id.clone(),
                    spans: Vec::new(),
                    processes: std::collections::HashMap::new(),
                }
            };
            log::debug!(
                "get_trace: Created trace object with {} spans",
                trace.spans.len()
            );

            let response = ApiResponse::new(vec![trace], 1, 0, 0);
            Json(response)
        }
        Err(e) => {
            let response = ApiResponse::from_grpc_error(
                e,
                &format!("get_trace for trace_id '{}'", trace_id),
            );
            log::debug!("get_trace: Output - Error response: {:?}", response.errors);
            Json(response)
        }
    }
}

/// Handler for GET /api/dependencies
///
/// Returns service dependency information.
/// Currently not implemented as it requires a separate dependencies service.
///
/// # Returns
/// JSON response indicating the feature is not implemented
pub async fn get_dependencies(
    State(_state): State<AppState>,
    Query(params): Query<serde_json::Value>,
) -> Json<ApiResponse<Vec<()>>> {
    log::info!("get_dependencies: Starting request");
    log::debug!("get_dependencies: Input - params: {:?}", params);
    log::warn!(
        "get_dependencies: Dependencies API not implemented - returning not implemented response"
    );

    let response = ApiResponse::not_implemented("Dependencies API not implemented".to_string());
    log::debug!(
        "get_dependencies: Output - Error response: {:?}",
        response.errors
    );
    Json(response)
}

/// Handler for GET /api/archive/{trace_id}
///
/// Retrieves an archived trace by its ID.
/// Currently uses the same implementation as regular trace retrieval.
///
/// # Path Parameters
/// - `trace_id`: Hex-encoded trace ID
///
/// # Returns
/// JSON response containing the archived trace
pub async fn get_archived_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
) -> Json<ApiResponse<Vec<JaegerTrace>>> {
    log::info!("get_archived_trace: Starting request");
    log::debug!("get_archived_trace: Input - trace_id: '{}'", trace_id);
    log::debug!("get_archived_trace: Note - currently using same implementation as regular traces");

    let mut client = state.client.clone();

    let trace_id_bytes = parse_trace_id(&trace_id);
    log::debug!(
        "get_archived_trace: Parsed trace_id to {} bytes",
        trace_id_bytes.len()
    );

    let request = tonic::Request::new(GetTracesRequest {
        query: vec![GetTraceParams {
            trace_id: trace_id_bytes,
            start_time: None,
            end_time: None,
        }],
    });

    log::debug!("get_archived_trace: Sending gRPC get_traces request");
    match client.get_traces(request).await {
        Ok(response) => {
            log::debug!("get_archived_trace: Received gRPC response, processing stream");
            let mut stream = response.into_inner();
            let mut all_resource_spans = Vec::new();

            while let Ok(Some(chunk)) = stream.message().await {
                log::trace!(
                    "get_archived_trace: Processing chunk with {} resource spans",
                    chunk.resource_spans.len()
                );
                all_resource_spans.extend(chunk.resource_spans);
            }

            let spans = process_resource_spans(all_resource_spans);

            log::info!(
                "get_archived_trace: Success - Found {} spans for archived trace '{}'",
                spans.len(),
                trace_id
            );

            let jaeger_traces = convert_to_jaeger_traces(spans);
            let trace = if let Some(mut trace) = jaeger_traces.into_iter().next() {
                // Use the original trace_id from the request to maintain consistency
                trace.trace_id = trace_id.clone();
                trace
            } else {
                JaegerTrace {
                    trace_id: trace_id.clone(),
                    spans: Vec::new(),
                    processes: std::collections::HashMap::new(),
                }
            };
            log::debug!(
                "get_archived_trace: Created trace object with {} spans",
                trace.spans.len()
            );

            let response = ApiResponse::new(vec![trace], 1, 0, 0);
            Json(response)
        }
        Err(e) => {
            log::error!(
                "get_archived_trace: gRPC error for archived trace '{}': {}",
                trace_id,
                e
            );
            let response = ApiResponse::not_found("Trace not found in archive".to_string());
            log::debug!(
                "get_archived_trace: Output - Error response: {:?}",
                response.errors
            );
            Json(response)
        }
    }
}
