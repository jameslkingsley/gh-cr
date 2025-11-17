use std::{
    error::Error,
    io::{Error as IoError, ErrorKind, Write},
    process::{Command, Stdio},
    task::{Context, Poll},
};

use bytes::Bytes;
use futures_util::future::BoxFuture;
use http::StatusCode;
use http_body::Body as HttpBody;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response};
use tower::Service;

type BoxError = Box<dyn Error + Send + Sync>;

pub struct GitHubCLI;

impl<B> Service<Request<B>> for GitHubCLI
where
    B: HttpBody<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<BoxError>,
{
    type Response = Response<Full<Bytes>>;
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let method = req.method().clone();
        let uri = req.uri().clone();

        // split so we can move the body into the async block
        let (_parts, body) = req.into_parts();

        Box::pin(async move {
            // Collect the request body into Bytes, regardless of the concrete body type.
            let collected = BodyExt::collect(body).await.map_err(|e| e.into())?;
            let body_bytes = collected.to_bytes();

            // gh api expects a path-only URL; Octocrab is responsible for the base URI.
            let path = uri.path_and_query().map(|x| x.as_str()).unwrap_or("/");

            let mut cmd = Command::new("gh");
            cmd.arg("api")
                .arg(path)
                .arg("--method")
                .arg(method.as_str())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            if !body_bytes.is_empty() {
                cmd.arg("--input").arg("-");
            }

            let mut child = cmd.spawn()?;

            if !body_bytes.is_empty() {
                // If gh's stdin isn't available, propagate a proper IO error.
                let mut stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| IoError::new(ErrorKind::BrokenPipe, "gh stdin unavailable"))?;
                stdin.write_all(&body_bytes)?;
            }

            let output = child.wait_with_output()?;

            let status = if output.status.success() {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            };

            let resp = Response::builder()
                .status(status)
                .body(Full::new(Bytes::from(output.stdout)))
                .map_err(|e| -> BoxError { Box::new(e) })?;

            Ok(resp)
        })
    }
}
