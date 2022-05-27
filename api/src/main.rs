use actix_cors::Cors;
use actix_web::{guard, middleware, web, App, HttpServer};

mod db;
mod routes;

async fn index() -> String {
    "Welcome to CTB-Web API".into()
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
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(pool.clone()))
            .route("/", web::get().to(index))
            .service(
                web::resource("/login")
                    .wrap(middleware::Logger::default())
                    .guard(guard::Post())
                    .to(routes::login),
            )
            .service(
                web::resource("/register")
                    .wrap(middleware::Logger::default())
                    .guard(guard::Post())
                    .to(routes::register),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
