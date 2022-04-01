use app::App;
use tokio::net::TcpListener;

pub mod app;
pub mod client;

#[tokio::main]
async fn main() {
    println!("Starting Azusa..");
    dotenv::dotenv().unwrap();
    let app = App::new().await;
    let socket = TcpListener::bind("127.0.0.1:3012").await.unwrap();

    while let Ok((stream, _)) = socket.accept().await {
        app.accept(stream);
    }
}
