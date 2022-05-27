use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Request, RequestInit, Response};

pub async fn post<B, T>(url: String, body: B) -> Result<T, String>
where
    B: serde::Serialize + Send + 'static,
    T: for<'a> serde::Deserialize<'a>,
{
    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.body(Some(&JsValue::from_str(
        &serde_json::to_string_pretty(&body).unwrap(),
    )));

    let request = Request::new_with_str_and_init(&url, &opts).map_err(|js| format!("{:?}", js))?;

    request
        .headers()
        .set("Accept", "application/json")
        .map_err(|js| format!("{:?}", js))?;

    request.headers().delete("content-type").unwrap();
    request
        .headers()
        .set("Content-Type", "application/json; charset=utf-8")
        .map_err(|js| format!("{:?}", js))?;

    let window = window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|js| format!("{:?}", js))?;

    let resp: Response = resp_value.dyn_into().unwrap();
    let json = JsFuture::from(resp.json().map_err(|js| format!("{:?}", js))?)
        .await
        .map_err(|js| format!("{:?}", js))?;

    Ok(json.into_serde().unwrap())
}
