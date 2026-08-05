//! `GET /qr`: an SVG QR code for the guest join URL, so a phone can join the
//! party by scanning the TV. The join host is taken from the request's `Host`
//! header, so the QR always points at whatever address the TV itself was
//! reached on (LAN IP or hostname), no configuration needed.

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use qrcode::{render::svg, QrCode};

/// Build `http://<host>/party` from the Host header and render it as an SVG QR.
pub async fn handle_qr(headers: HeaderMap) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8080");

    // LAN-only, so plain http. HTTPS would be required for mic capture, but
    // joining the guest page does not need it.
    let url = format!("http://{host}/party");

    match QrCode::new(url.as_bytes()) {
        Ok(code) => {
            let svg = code
                .render::<svg::Color>()
                .min_dimensions(240, 240)
                .quiet_zone(true)
                .build();
            (
                [
                    (header::CONTENT_TYPE, "image/svg+xml"),
                    // The URL changes with the Host header, so don't cache.
                    (header::CACHE_CONTROL, "no-store"),
                ],
                svg,
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("qr error: {e}"),
        )
            .into_response(),
    }
}
