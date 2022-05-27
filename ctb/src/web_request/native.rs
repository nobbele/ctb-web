#[must_use]
pub struct WebRequest<T> {
    handle: Option<std::thread::JoinHandle<()>>,
    url: String,
    body: Option<String>,
    data: std::sync::Arc<std::lazy::SyncOnceCell<Result<T, String>>>,
}

// TODO Fix this generic mess.

impl<T> std::future::Future for WebRequest<T>
where
    T: for<'a> serde::Deserialize<'a> + std::fmt::Debug + Clone + Send + Sync + 'static,
{
    type Output = Result<T, String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.handle.is_none() {
            let req = ureq::post(&self.url);
            self.handle = Some(std::thread::spawn({
                let data = self.data.clone();
                let body = self.body.take().unwrap();
                move || {
                    let resp = match req
                        .send_json(serde_json::from_str::<serde_json::Value>(&body).unwrap())
                    {
                        Ok(o) => o,
                        Err(e) => {
                            data.set(Err(e.to_string())).unwrap();
                            return;
                        }
                    };

                    let status = resp.status();
                    let content = match resp.into_string() {
                        Ok(o) => o,
                        Err(e) => {
                            data.set(Err(e.to_string())).unwrap();
                            return;
                        }
                    };

                    data.set(if status == 200 {
                        Ok(match serde_json::from_str(&content) {
                            Ok(o) => o,
                            Err(e) => {
                                data.set(Err(e.to_string())).unwrap();
                                return;
                            }
                        })
                    } else {
                        Err(content)
                    })
                    .unwrap();
                }
            }));
        }

        match self.data.get() {
            Some(o) => std::task::Poll::Ready(o.clone()),
            None => std::task::Poll::Pending,
        }
    }
}

pub async fn post<B, T>(url: String, body: B) -> Result<T, String>
where
    B: serde::Serialize + Send + 'static,
    T: for<'a> serde::Deserialize<'a> + std::fmt::Debug + Clone + Send + Sync + 'static,
{
    let data = std::sync::Arc::new(std::lazy::SyncOnceCell::new());

    WebRequest {
        handle: None,
        url,
        body: Some(serde_json::to_string_pretty(&body).unwrap()),
        data,
    }
    .await
}
