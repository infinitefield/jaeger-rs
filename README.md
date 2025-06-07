# Jaeger RS

A Rust implementation of the Jaeger frontend that provides a web interface and HTTP API for distributed tracing data.

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

- **Static File Server**: Serves the Jaeger UI frontend from `./actual/` directory. You can probably get it by running `make` in jaeger.
- **HTTP API Server**: Provides REST endpoints compatible with Jaeger UI
- **gRPC Client**: Communicates with storage backend using Jaeger storage v2 protocol
- **Data Conversion**: Transforms OpenTelemetry protobuf data to Jaeger JSON format

## Usage

### Build and Run

```bash
# Run with default settings (connects to localhost:4317)
cargo run

# Run with custom backend
cargo run -- --host http://your-backend:4317

# Run on different port
cargo run -- --listen 127.0.0.1:8080
```

The server will start on `http://0.0.0.0:3000`. Open this URL in your browser to access the Jaeger UI.

### Command Line Options

```bash
cargo run -- --help
```

- `--host` - gRPC backend URL (default: `http://localhost:4317`)
- `--listen` - Listen address for HTTP server (default: `0.0.0.0:3000`)

### Systemd Service

Create `/etc/systemd/system/jaeger-rs.service`:

```ini
[Unit]
Description=Jaeger RS - Rust Jaeger Frontend
After=network.target

[Service]
Type=simple
User=jaeger
Group=jaeger
ExecStart=/usr/local/bin/jaeger-rs --host http://your-backend:4317
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable jaeger-rs
sudo systemctl start jaeger-rs
sudo systemctl status jaeger-rs
```

## Contributing

PRs are welcome, just make it compatible with the UI.
