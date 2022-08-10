use app::App;
use tokio::net::TcpListener;

pub mod app;
pub mod client;

#[tokio::main]
async fn main() {
    println!("Starting Azusa..");
    dotenv::dotenv().ok();
    let app = App::new().await;
    let socket = TcpListener::bind("0.0.0.0:3012").await.unwrap();

    while let Ok((stream, _)) = socket.accept().await {
        app.accept(stream);
    }
}
