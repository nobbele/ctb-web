#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

// TODO Fix this generic mess (native).

pub async fn post<B, T>(url: String, body: B) -> Result<T, String>
where
    B: serde::Serialize + Send + 'static,
    T: for<'a> serde::Deserialize<'a> + std::fmt::Debug + Clone + Send + Sync + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::post(url, body).await
    }
    #[cfg(target_arch = "wasm32")]
    {
        web::post(url, body).await
    }
}
