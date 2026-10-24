//! Entrega del Admin y del Player compilados por Vite (`web/dist`).
//!
//! En la compilación de producción (`--release`) los archivos quedan dentro del binario: Kinora
//! no depende de dónde esté el proyecto. En desarrollo se leen del disco en cada pedido, así
//! que alcanza con `npm run build` y recargar la página.

use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
// Sin la web compilada el servidor igual compila (y la API anda): avisa al arrancar.
#[allow_missing = true]
struct Dist;

/// Sirve `/` (Player) y `/admin/` (Admin). Cada carpeta entrega su `index.html`.
pub async fn serve(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let wanted = if path.is_empty() || path.ends_with('/') {
        format!("{path}index.html")
    } else {
        path.to_string()
    };
    if let Some(file) = Dist::get(&wanted) {
        return respond(&wanted, file, &headers);
    }
    // "/admin" sin barra: como un servidor de archivos, se redirige a la carpeta.
    if !path.is_empty()
        && !path.ends_with('/')
        && Dist::get(&format!("{path}/index.html")).is_some()
    {
        let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
        return Redirect::temporary(&format!("/{path}/{query}")).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

/// Lo que arma Vite en `assets/` lleva el contenido en el nombre: no cambia nunca, se guarda
/// para siempre. El resto (los `index.html`) lleva `no-cache`: el navegador pregunta siempre
/// si cambió algo, y si no cambió el servidor responde "304 Not Modified" sin reenviarlo.
fn respond(path: &str, file: EmbeddedFile, headers: &HeaderMap) -> Response {
    let hash: String = file
        .metadata
        .sha256_hash()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let etag = format!("\"{hash}\"");
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let unchanged = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|tag| tag.trim() == etag));
    let mut response = if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (
            [(header::CONTENT_TYPE, file.metadata.mimetype().to_string())],
            file.data,
        )
            .into_response()
    };
    let response_headers = response.headers_mut();
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Ok(value) = HeaderValue::from_str(&etag) {
        response_headers.insert(header::ETAG, value);
    }
    response
}

pub fn is_built() -> bool {
    Dist::get("index.html").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_each_app_and_redirects_admin_without_slash() {
        if !is_built() {
            return;
        }
        let get = |path: &str| serve(path.parse().unwrap(), HeaderMap::new());

        let player = get("/").await;
        assert_eq!(player.status(), StatusCode::OK);
        assert_eq!(player.headers()[header::CACHE_CONTROL], "no-cache");

        let admin = get("/admin/").await;
        assert_eq!(admin.status(), StatusCode::OK);

        let redirect = get("/admin?x=1").await;
        assert_eq!(redirect.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(redirect.headers()[header::LOCATION], "/admin/?x=1");

        assert_eq!(get("/no-existe.js").await.status(), StatusCode::NOT_FOUND);

        // Con el mismo ETag, "no cambió".
        let etag = player.headers()[header::ETAG].clone();
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag);
        let again = serve("/".parse().unwrap(), headers).await;
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    }
}
