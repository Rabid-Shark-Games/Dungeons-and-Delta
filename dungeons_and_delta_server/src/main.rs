use futures_util::{SinkExt, StreamExt};
use json::object;
use log::*;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{net::{TcpListener, TcpStream}, sync::Mutex};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Error};
use tungstenite::{Message, Result, protocol::CloseFrame};

struct ServerData {
    host_connected: bool,
    host: Option<HostPlayer>,
    clients: Vec<Player>,
    client_options: Vec<PlayerStat>,
}

impl ServerData {
    async fn send_all_but_self(&mut self, data: Message, current: &mut Player) -> Result<()> {
        current.do_not_send = true;

        for player in &mut self.clients {
            if !player.do_not_send {
                player.data_stream.lock().await.send(data.clone()).await?;
            }
        }

        if self.host_connected {
            self.host.as_mut().unwrap().data_stream.lock().await.send(data.clone()).await?;
        }

        Ok(())
    }
}

struct HostPlayer {
    data_stream: Arc<Mutex<WebSocketStream<TcpStream>>>,
    peer: SocketAddr,
}

struct Player {
    data_stream: Arc<Mutex<WebSocketStream<TcpStream>>>,
    peer: SocketAddr,
    stat: Option<PlayerStat>,
    last_item: Option<Item>,
    do_not_send: bool,
}

struct PlayerStat {
    name: String,
    weapons: Vec<Weapon>,
    spells: Vec<Spell>,
    items: Vec<Item>,
    health: f32,
    max_health: f32,
    modifiers: HashMap<DamageType, f32>,
    global_modifier: f32,
    /// This only applies to healing *spells*, and not healing *items*.
    heal_mod: f32,
    max_spell_slots: i32,
    current_spell_slots: i32,
}

impl PlayerStat {
    /// Returns the amount of damage *actually* taken.
    /// (including modifiers)
    ///
    /// This shouldn't be shown, but could be useful.
    fn take_damage(&mut self, amount: f32, damage: DamageType) -> f32 {
        let damage = amount * self.modifiers.get(&damage).unwrap_or(&1.0);

        self.health -= damage;

        return damage;
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

struct Weapon;
struct Spell;
struct Item {
    name: String,
    function: Box<dyn Fn(&mut Player) -> String + Send>,
}

fn healing_item(amount: f32) -> impl Fn(&mut Player) -> String {
    let closure = move |player: &mut Player| -> String {
        if let Some(stat) = &mut player.stat {
            stat.health += amount;
            return format!("You ate the {}.\n{} HP restored.", player.last_item.as_ref().unwrap().name, amount);
        }
        return "You aren't playing a character!".to_string();
    };

    closure
}

async fn accept_connection(data: Arc<Mutex<ServerData>>, peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(data, peer, stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(data: Arc<Mutex<ServerData>>, peer: SocketAddr, stream: TcpStream) -> Result<()> {
    let ws_stream = Arc::new(Mutex::new(accept_async(stream).await.expect("Failed to accept")));

    info!("New WebSocket connection: {}", peer);

    if !data.lock().await.host_connected {
        {
            let mut lock = data.lock().await;
            lock.host = Some(HostPlayer {
                data_stream: ws_stream.clone(),
                peer,
            });
            lock.host_connected = true;
        }

        return handle_host(data.clone(), ws_stream, peer).await;
    } else {
        return handle_client(data, peer, ws_stream).await;
    }
}

async fn handle_client(data: Arc<Mutex<ServerData>>, peer: SocketAddr, stream: Arc<Mutex<WebSocketStream<TcpStream>>>) -> Result<()> {
    stream.lock().await.send(Message::Text(json::stringify(object! {
        event_type: "player_status",
        status: "client",
        possible_players: data.lock().await.client_options.iter().map(|x: &PlayerStat| x.name.clone()).collect::<Vec<String>>()
    }))).await?;

    Ok(())
}

async fn client_error<T>(x: Result<T>, data: Arc<Mutex<ServerData>>) -> Result<T> {
    if x.is_err() {
        data.lock().await.host_connected = false;
    }
    x
}

async fn handle_host(data: Arc<Mutex<ServerData>>, stream: Arc<Mutex<WebSocketStream<TcpStream>>>, peer: SocketAddr) -> Result<()> {
    host_error(stream.lock().await.send(Message::Text(json::stringify(object! {
        event_type: "player_status",
        status: "host",
    }))).await, &data).await?;

    let _ = stream.lock().await.next().await;

    host_cleanup(&data).await;

    Ok(())
}

async fn host_error<T>(x: Result<T>, data: &Arc<Mutex<ServerData>>) -> Result<T> {
    if x.is_err() {
        host_cleanup(data).await;
    }
    x
}

async fn host_cleanup(data: &Arc<Mutex<ServerData>>) {
        let mut lock = data.lock().await;
        lock.host_connected = false;
        lock.host = None;
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .init();

    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    let data = Arc::new(Mutex::new(ServerData {
        host_connected: false,
        host: None,
        clients: vec![],
        client_options: vec![PlayerStat {
            name: "Test".to_string(),
            weapons: vec![],
            spells: vec![],
            items: vec![Item {
                name: "heal".to_string(),
                function: Box::new(healing_item(1000.0)),
            }],
            health: 20.0,
            max_health: 20.0,
            modifiers: HashMap::new(),
            global_modifier: 1.0,
            heal_mod: 1.0,
            current_spell_slots: 0,
            max_spell_slots: 0,
        }],
    }));

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        let server_data = data.clone();
        tokio::spawn(accept_connection(server_data, peer, stream));
    }
}