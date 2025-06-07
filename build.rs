//! Build script for generating Rust code from Protocol Buffer definitions.
//!
//! This script generates Rust structs and gRPC client code from the Jaeger storage
//! service protobuf definitions. It configures tonic-build to:
//!
//! - Generate only client code (no server implementations)
//! - Map OpenTelemetry TracesData to the opentelemetry-proto crate types
//! - Include Google well-known types (Timestamp, Duration, etc.)
//!
//! The generated code allows the application to communicate with the Jaeger
//! storage backend using strongly-typed Rust structs.

/// Main build function that generates Rust code from protobuf definitions.
///
/// This function configures tonic-build to generate gRPC client code for the
/// Jaeger storage service. It maps external OpenTelemetry types to use the
/// opentelemetry-proto crate for consistency.
///
/// # Generated Code
/// The build process generates:
/// - gRPC client code for TraceReader service
/// - Rust structs for all protobuf messages
/// - Type mappings for OpenTelemetry data structures
///
/// # Returns
/// - `Ok(())` if code generation succeeds
/// - `Err(Box<dyn std::error::Error>)` if compilation fails
///
/// # Panics
/// This function will panic if:
/// - The proto files cannot be found in the `proto/` directory
/// - The protobuf definitions contain syntax errors
/// - The opentelemetry-proto crate is not available
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-changed=build.rs");
    
    tonic_build::configure()
        // Only generate client code, not server implementations
        .build_server(false)
        // Map OpenTelemetry TracesData to use the opentelemetry-proto crate
        // This ensures compatibility with existing OpenTelemetry ecosystem
        .extern_path(
            ".opentelemetry.proto.trace.v1.TracesData",
            "::opentelemetry_proto::tonic::trace::v1::TracesData",
        )
        // Include Google well-known types (Timestamp, Duration, Any, etc.)
        .compile_well_known_types(true)
        // Compile the Jaeger storage service protobuf definition
        .compile(&["proto/trace_storage.proto"], &["proto"])?;
    
    Ok(())
}
