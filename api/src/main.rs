use actix_cors::Cors;
use actix_web::{
    dev, error::ErrorBadRequest, guard, middleware, web, App, FromRequest, HttpMessage,
    HttpRequest, HttpServer,
};
use actix_web_httpauth::middleware::HttpAuthentication;
use sqlx::types::Uuid;

mod db;
mod routes;

async fn index() -> String {
    "Welcome to CTB-Web API".into()
}

pub struct UserIdFromToken(u32);

impl FromRequest for UserIdFromToken {
    type Error = actix_web::Error;
    type Future = std::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        std::future::ready(match req.extensions().get::<UserIdFromToken>() {
            Some(UserIdFromToken(user_id)) => Ok(UserIdFromToken(*user_id)),
            None => Err(actix_web::error::ErrorBadRequest(
                "Unable to get UserIdFromToken",
            )),
        })
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().unwrap();
    std::env::set_var("RUST_LOG", "actix_web=debug");
    env_logger::init();

    println!("Starting server at http://127.0.0.1:8080");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::init_pool(&database_url)
        .await
        .expect("Failed to create pool");

    HttpServer::new(move || {
        let auth = HttpAuthentication::bearer({
            let pool = pool.clone();
            move |req, cred| {
                let pool = pool.clone();
                async move {
                    let user_id: i32 =
                        match sqlx::query_as("SELECT user_id FROM sessions WHERE token = $1;")
                            .bind(cred.token().parse::<Uuid>().unwrap())
                            .fetch_optional(&pool)
                            .await
                            .unwrap()
                        {
                            Some((user_id,)) => user_id,
                            None => return Err((ErrorBadRequest("Invalid Token"), req)),
                        };
                    req.extensions_mut()
                        .insert(UserIdFromToken(user_id.try_into().unwrap()));
                    Ok(req)
                }
            }
        });
        App::new()
            .wrap(Cors::permissive())
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .route("/", web::get().to(index))
            .service(
                web::resource("/login")
                    .guard(guard::Post())
                    .to(routes::login::login),
            )
            .service(
                web::resource("/register")
                    .guard(guard::Post())
                    .to(routes::register::register),
            )
            .service(
                web::resource("/users/me")
                    .wrap(auth)
                    .guard(guard::Get())
                    .to(routes::get_me::get_me),
            )
            .service(
                web::resource("/users/by-id/{user_id}")
                    .guard(guard::Get())
                    .to(routes::get_user_by_id::get_user_by_id),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
