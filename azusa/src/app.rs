use crate::client::Client;
use ctb_web::azusa::{ClientPacket, ServerPacket};
use futures::{SinkExt, StreamExt};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::{collections::HashMap, env, ops::DerefMut, sync::Arc, time::Duration};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_tungstenite::WebSocketStream;

pub type Wss = WebSocketStream<TcpStream>;

#[derive(Debug)]
pub enum Target {
    Everyone,
    User(String),
}

pub struct App {
    tx: flume::Sender<(Target, ServerPacket)>,
    rx: flume::Receiver<(Target, ServerPacket)>,
    clients: RwLock<HashMap<String, Arc<RwLock<(Wss, Client)>>>>,
    pub pool: Pool<Postgres>,
}

impl App {
    pub async fn new() -> &'static Self {
        let (tx, rx) = flume::unbounded();
        let db_addr = env::var("DATABASE_URL").unwrap();
        println!("Connecting to '{}'", db_addr);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&db_addr)
            .await
            .unwrap();
        /*sqlx::query("DROP TABLE IF EXISTS users, scores")
        .execute(&pool)
        .await
        .unwrap();*/
        sqlx::query(
            "
CREATE TABLE IF NOT EXISTS users (
    user_id INT GENERATED ALWAYS AS IDENTITY NOT NULL,
    username TEXT NOT NULL,
    PRIMARY KEY(user_id)
);
        ",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "
CREATE TABLE IF NOT EXISTS scores (
    user_id INT NOT NULL,
    diff_id INTEGER NOT NULL,
    hit_count INTEGER NOT NULL,
    miss_count INTEGER NOT NULL,
    score INTEGER NOT NULL,
    top_combo INTEGER NOT NULL,

    CONSTRAINT fk_user
      FOREIGN KEY(user_id)
	  REFERENCES users(user_id)
);
        ",
        )
        .execute(&pool)
        .await
        .unwrap();

        let app: &'static App = Box::leak(Box::new(App {
            tx,
            rx,
            clients: RwLock::new(HashMap::new()),
            pool,
        }));

        println!("Spinning up.");

        tokio::task::spawn(async move {
            loop {
                for (target, packet) in app.rx.drain() {
                    println!("Sending packet '{:?}' to {:?}", packet, target);
                    let data = bincode::serialize(&packet).unwrap();
                    let clients = app.clients.read().await;
                    match target {
                        Target::Everyone => {
                            for lock in clients.values() {
                                let mut guard = lock.write().await;
                                guard.0.feed(data.clone().into()).await.unwrap();
                            }
                        }
                        Target::User(username) => {
                            clients[&username]
                                .write()
                                .await
                                .0
                                .feed(data.into())
                                .await
                                .unwrap();
                        }
                    }
                }
                for lock in app.clients.read().await.values() {
                    let _ = lock.write().await.0.flush().await;
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        });

        app
    }

    pub fn send(&self, target: Target, packet: ServerPacket) {
        self.tx.send((target, packet)).unwrap();
    }

    pub fn accept(&'static self, stream: tokio::net::TcpStream) {
        tokio::task::spawn(async move {
            println!("Received TCP stream.");
            let mut ws = tokio_tungstenite::accept_async(stream)
                .await
                .expect("WebSocket handshake failed.");
            println!("WebSocket handshake complete.");
            let conn_msg = match tokio::time::timeout_at(
                tokio::time::Instant::now() + tokio::time::Duration::from_millis(3000),
                ws.next(),
            )
            .await
            {
                Ok(o) => o.unwrap().unwrap(),
                Err(_) => {
                    println!("Client failed to login in time.");
                    return;
                }
            };
            let packet: ClientPacket = bincode::deserialize(&conn_msg.into_data()).unwrap();
            let username = match packet {
                ClientPacket::Login => {
                    format!(
                        "TEMP-{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros()
                    )
                }
                _ => panic!(),
            };

            let (user_id,): (i32,) =
                sqlx::query_as("INSERT INTO users(username) VALUES ($1) RETURNING user_id")
                    .bind(username.clone())
                    .fetch_one(&self.pool)
                    .await
                    .unwrap();
            let user_id = u32::try_from(user_id).unwrap();

            let client = Client::new(username.clone(), user_id, self);
            let tup = Arc::new(RwLock::new((ws, client)));
            self.clients
                .write()
                .await
                .insert(username.clone(), tup.clone());
            self.tx
                .send((Target::User(username.clone()), ServerPacket::Connected))
                .unwrap();
            println!("Client login sucessful. Username: {}", username);

            loop {
                let mut guard = tup.write().await;
                let (wss, client) = guard.deref_mut();
                match tokio::time::timeout_at(
                    tokio::time::Instant::now() + tokio::time::Duration::from_millis(10),
                    wss.next(),
                )
                .await
                {
                    Ok(o) => {
                        let msg = o.unwrap().unwrap();
                        let packet: ClientPacket =
                            bincode::deserialize(msg.into_data().as_slice()).unwrap();
                        println!("Server got packet '{:?}'", packet);
                        client.handle(packet).await;
                    }
                    Err(_) => (),
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    }
}
