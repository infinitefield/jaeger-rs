# Jaeger RS

A Rust implementation of the Jaeger frontend that provides a web interface and HTTP API for distributed tracing data. This application serves the Jaeger UI and proxies API requests to a gRPC-based tracing storage backend.

## Features

- ✅ **Complete Jaeger UI compatibility** - Serves the official Jaeger UI
- ✅ **HTTP API endpoints** - All essential Jaeger API endpoints implemented
- ✅ **gRPC backend integration** - Connects to Jaeger storage via gRPC
- ✅ **OpenTelemetry support** - Handles OpenTelemetry trace data format
- ✅ **Comprehensive logging** - Detailed request/response logging
- ✅ **Error handling** - Graceful error responses with proper HTTP status codes

## Architecture

```
┌─────────────────┐    HTTP/JSON    ┌───────────────┐    gRPC/Proto    ┌─────────────────┐
│   Jaeger UI     │ ◄──────────────► │  Jaeger RS    │ ◄───────────────► │ Storage Backend │
│  (JavaScript)   │                 │   (Rust)      │                  │   (Any gRPC)    │
└─────────────────┘                 └───────────────┘                  └─────────────────┘
```

### Components

- **Static File Server**: Serves the Jaeger UI frontend from `./actual/` directory
- **HTTP API Server**: Provides REST endpoints compatible with Jaeger UI
- **gRPC Client**: Communicates with storage backend using Jaeger storage v2 protocol
- **Data Conversion**: Transforms OpenTelemetry protobuf data to Jaeger JSON format

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/services` | List all services that have reported traces |
| `GET` | `/api/operations?service={name}` | List operations for a specific service |
| `GET` | `/api/services/{service}/operations` | Alternative endpoint for operations |
| `GET` | `/api/traces` | Search traces with various filters |
| `GET` | `/api/traces/{trace_id}` | Retrieve a specific trace by ID |
| `GET` | `/api/dependencies` | Get service dependencies (not implemented) |
| `GET` | `/api/archive/{trace_id}` | Retrieve archived trace by ID |

### Query Parameters for `/api/traces`

- `service` - Filter by service name
- `operation` - Filter by operation name  
- `tags` - Filter by tags (not yet implemented)
- `start` - Start time filter (microseconds since epoch)
- `end` - End time filter (microseconds since epoch)
- `minDuration` - Minimum duration filter (microseconds)
- `maxDuration` - Maximum duration filter (microseconds)
- `limit` - Maximum number of traces to return (default: 100)

## Quick Start

### Prerequisites

- Rust 1.70+ with Cargo
- A running Jaeger storage backend with gRPC interface
- Jaeger UI static files in `./actual/` directory

### Building

```bash
# Clone the repository
git clone <repository-url>
cd jaeger-rs

# Build the application
cargo build --release
```

### Configuration

The gRPC backend URL can be configured via command line:

```bash
# Default: connects to localhost:4317
cargo run

# Custom backend
cargo run -- --host http://your-jaeger-backend:4317
```

### Running

```bash
# Run with default backend (localhost:4317)
cargo run

# Run with custom backend
cargo run -- --host http://your-backend:4317

# Run with debug logging
RUST_LOG=debug cargo run

# Combined: custom backend with debug logging
RUST_LOG=debug cargo run -- --host http://your-backend:4317
```

The server will start on `http://0.0.0.0:3000`.

## Dependencies

### Runtime Dependencies

- **axum** - Modern web framework for HTTP server
- **tonic** - gRPC client for backend communication
- **serde** - JSON serialization/deserialization
- **opentelemetry-proto** - OpenTelemetry protobuf definitions
- **hex** - Hex encoding/decoding for trace IDs
- **env_logger** - Logging implementation
- **tower-http** - HTTP middleware for static file serving

### Build Dependencies

- **tonic-build** - Code generation from protobuf definitions

## Logging

The application provides comprehensive logging at multiple levels:

### Log Levels

- **ERROR** - gRPC errors, connection failures
- **WARN** - Invalid trace ID formats, missing data
- **INFO** - Request lifecycle, connection status, response summaries
- **DEBUG** - Request parameters, response data, detailed processing steps
- **TRACE** - Individual span processing, low-level details

### Log Format

Each log entry includes:
- Function name prefix (e.g., `get_services:`, `get_traces:`)
- Action description (e.g., `Starting request`, `Success - Found 5 services`)
- Input/output data at DEBUG level

### Examples

```
INFO  jaeger_rs::handlers - get_services: Starting request
DEBUG jaeger_rs::handlers - get_services: Input - No parameters  
INFO  jaeger_rs::handlers - get_services: Success - Found 3 services
DEBUG jaeger_rs::handlers - get_services: Output - Services: ["frontend", "backend", "database"]
```

## Error Handling

All API endpoints return consistent error responses:

```json
{
  "data": [],
  "total": 0,
  "limit": 0,
  "offset": 0,
  "errors": [
    {
      "code": 500,
      "msg": "gRPC error: transport error"
    }
  ]
}
```

### Error Codes

- **500** - Internal server error (gRPC failures)
- **501** - Not implemented (dependencies endpoint)
- **404** - Not found (archived traces)

## Development

### Project Structure

```
jaeger-rs/
├── src/
│   ├── main.rs          # Application entry point and routing
│   └── handlers.rs      # HTTP request handlers
├── proto/               # Protocol buffer definitions
│   └── trace_storage.proto
├── build.rs            # Build script for protobuf generation
├── Cargo.toml          # Rust dependencies
└── actual/             # Jaeger UI static files
```

### Adding New Endpoints

1. Add route in `src/main.rs`
2. Implement handler function in `src/handlers.rs`
3. Add comprehensive logging and documentation
4. Update this README

### Protocol Buffer Updates

If you modify `proto/trace_storage.proto`:

1. Update the protobuf definitions
2. Run `cargo build` to regenerate Rust code
3. Update handler implementations if needed

## Testing

```bash
# Check compilation
cargo check

# Run with debug logging for testing
RUST_LOG=debug cargo run

# Test API endpoints
curl http://localhost:3000/api/services
curl "http://localhost:3000/api/operations?service=my-service"
curl "http://localhost:3000/api/traces?service=my-service&limit=10"

# View help for command line options
cargo run -- --help
```

## Performance

The application is designed for high performance:

- **Async/await** - Non-blocking request handling
- **Connection pooling** - Reuses gRPC connections
- **Streaming** - Efficiently handles large trace responses
- **Static file caching** - Serves UI files efficiently

## Limitations

1. **Tags filtering** - Not yet implemented in trace search
2. **Dependencies endpoint** - Returns "not implemented" response
3. **Authentication** - No built-in authentication/authorization

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add comprehensive documentation and logging
4. Test your changes thoroughly
5. Submit a pull request

## License

[Add your license information here]