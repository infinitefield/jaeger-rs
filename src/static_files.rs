//! # Embedded Static File Server
//!
//! This module provides a custom tower Service implementation for serving
//! embedded static files using the include_dir crate. This allows the Jaeger UI
//! to be bundled directly into the binary without requiring external file dependencies.

use axum::{
    body::Body,
    http::{Request, Response, StatusCode, Uri},
};
use include_dir::{Dir, include_dir};
use mime_guess::MimeGuess;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::Service;

/// Embedded static files directory.
///
/// This includes all files from the "./actual/" directory at compile time,
/// making them available as embedded resources in the binary.
static STATIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/actual");

/// A tower Service that serves embedded static files.
///
/// This service handles HTTP requests for static files by looking them up
/// in the embedded directory structure. It supports:
/// - MIME type detection based on file extensions
/// - Fallback to index.html for SPA routing
/// - Proper HTTP status codes (200, 404)
/// - ETag-like caching headers
#[derive(Clone)]
pub struct StaticFileService;

impl StaticFileService {
    /// Creates a new StaticFileService instance.
    pub fn new() -> Self {
        Self
    }

    /// Handles a request for a static file.
    ///
    /// # Arguments
    /// * `uri` - The requested URI path
    ///
    /// # Returns
    /// HTTP response with the file content or 404 if not found
    fn handle_request(&self, uri: &Uri) -> Response<Body> {
        let path = uri.path().trim_start_matches('/');

        // Try to get the requested file
        if let Some(file) = STATIC_DIR.get_file(path) {
            return self.serve_file(file, path);
        }

        // For SPA routing, try common variations
        if !path.is_empty() && !path.contains('.') {
            // This might be a route, try to serve index.html
            if let Some(index_file) = STATIC_DIR.get_file("index.html") {
                return self.serve_file(index_file, "index.html");
            }
        }

        // If path is empty or just "/", serve index.html
        if path.is_empty() || path == "/" {
            if let Some(index_file) = STATIC_DIR.get_file("index.html") {
                return self.serve_file(index_file, "index.html");
            }
        }

        // File not found
        self.not_found_response()
    }

    /// Serves a file with appropriate headers.
    ///
    /// # Arguments
    /// * `file` - The embedded file to serve
    /// * `path` - The file path for MIME type detection
    ///
    /// # Returns
    /// HTTP response with the file content and appropriate headers
    fn serve_file(&self, file: &include_dir::File, path: &str) -> Response<Body> {
        let mime_type = MimeGuess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", mime_type)
            .body(Body::from(file.contents().to_vec()))
            .unwrap_or_else(|_| self.internal_error_response())
    }

    fn not_found_response(&self) -> Response<Body> {
        if let Some(index_file) = STATIC_DIR.get_file("index.html") {
            self.serve_file(index_file, "index.html")
        } else {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("content-type", "text/plain")
                .body(Body::from("File not found"))
                .unwrap()
        }
    }

    /// Creates a 500 Internal Server Error response.
    fn internal_error_response(&self) -> Response<Body> {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("content-type", "text/plain")
            .body(Body::from("Internal server error"))
            .unwrap()
    }
}

impl Default for StaticFileService {
    fn default() -> Self {
        Self::new()
    }
}

impl<ReqBody> Service<Request<ReqBody>> for StaticFileService {
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // This service is always ready to handle requests
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let uri = req.uri().clone();
        let response = self.handle_request(&uri);

        Box::pin(async move { Ok(response) })
    }
}

/// Helper function to create the static file service for use in axum routing.
///
/// # Returns
/// A configured StaticFileService that can be used as a fallback service
///
/// # Example
/// ```rust
/// use axum::Router;
///
/// let app = Router::new()
///     .route("/api/health", get(health_check))
///     .fallback_service(crate::static_files::static_file_service());
/// ```
pub fn static_file_service() -> StaticFileService {
    StaticFileService::new()
}
