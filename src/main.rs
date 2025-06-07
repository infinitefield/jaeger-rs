//! # Jaeger RS - A Rust Implementation of Jaeger Frontend
//!
//! This application provides a Jaeger-compatible frontend that serves the Jaeger UI
//! and exposes HTTP API endpoints for retrieving distributed tracing data.
//! It acts as a bridge between the Jaeger UI and a gRPC-based tracing storage backend.
//!
//! ## Architecture
//!
//! The application consists of:
//! - Static file serving for the Jaeger UI frontend
//! - HTTP API endpoints that proxy requests to a gRPC storage backend
//! - Protocol buffer definitions for communication with the storage service
//!
//! ## API Endpoints
//!
//! - `GET /api/services` - List all services
//! - `GET /api/operations` - List operations for a service (query param)
//! - `GET /api/services/{service}/operations` - List operations for a service (path param)
//! - `GET /api/traces` - Search traces with filters
//! - `GET /api/traces/{trace_id}` - Get specific trace by ID
//! - `GET /api/dependencies` - Get service dependencies (not implemented)
//! - `GET /api/archive/{trace_id}` - Get archived trace by ID
//!
//! ## Configuration
//!
//! The application connects to a gRPC storage backend. The connection URI is currently
//! hardcoded but can be made configurable via environment variables.

use axum::{Router, routing::get};
use clap::Parser;
use simple_logger::SimpleLogger;
use tower_http::services::{ServeDir, ServeFile};

mod handlers;

/// Protocol buffer definitions for communicating with the Jaeger storage backend.
///
/// This module contains generated code from protobuf definitions for:
/// - Google protobuf well-known types (Timestamp, Duration, etc.)
/// - Jaeger storage service v2 API
/// - OpenTelemetry trace data structures
pub mod rpc {
    /// Google protobuf well-known types.
    pub mod google {
        /// Standard protobuf types like Timestamp and Duration.
        pub mod protobuf {
            tonic::include_proto!("google.protobuf");
        }
    }

    /// Jaeger storage service definitions.
    pub mod jaeger {
        /// Storage service API definitions.
        pub mod storage {
            /// Version 2 of the Jaeger storage service API.
            pub mod v2 {
                tonic::include_proto!("jaeger.storage.v2");
            }
        }
    }

    // Re-export OpenTelemetry types for easier access
    pub use opentelemetry_proto::tonic::{
        common::v1 as opentelemetry_common, resource::v1 as opentelemetry_resource,
        trace::v1 as opentelemetry_trace,
    };
}

/// Command line arguments for the Jaeger RS frontend server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// gRPC storage backend host URI
    #[arg(long, default_value = "http://127.0.0.1:4317")]
    host: http::Uri,
}

/// Main application entry point.
///
/// This function sets up the HTTP server, configures routing, and starts serving
/// both the Jaeger UI static files and the API endpoints.
///
/// # Server Configuration
/// - Binds to `0.0.0.0:3000` to accept connections from any interface
/// - Serves static files from `./actual/` directory (Jaeger UI)
/// - Configures API routes under `/api/` prefix
/// - Establishes gRPC connection to storage backend
///
/// # Panics
/// - If the gRPC connection to the storage backend fails
/// - If the HTTP server cannot bind to the specified address
/// - If the server encounters an unrecoverable error during startup
#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging - supports RUST_LOG environment variable
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    // Create application state with gRPC connection to storage backend
    let state = handlers::AppState::new(args.host);

    // Configure HTTP router with API endpoints and static file serving
    let app = Router::new()
        // Core Jaeger API endpoints
        .route("/api/services", get(handlers::get_services))
        .route(
            "/api/services/{service}/operations",
            get(handlers::get_service_operations),
        )
        .route("/api/operations", get(handlers::get_operations))
        .route("/api/traces", get(handlers::get_traces))
        .route("/api/traces/{trace_id}", get(handlers::get_trace))
        .route("/api/dependencies", get(handlers::get_dependencies))
        .route("/api/archive/{trace_id}", get(handlers::get_archived_trace))
        // Static file serving for Jaeger UI
        .fallback_service(
            ServeDir::new("./actual/").not_found_service(ServeFile::new("./actual/index.html")),
        )
        // .fallback_service(ServeFile::new("./actual/index.html"))
        // Attach application state (gRPC client)
        .with_state(state);

    // Start HTTP server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    log::info!("Listening on 0.0.0.0:3000");
    // Serve requests indefinitely
    axum::serve(listener, app).await.unwrap();
}
