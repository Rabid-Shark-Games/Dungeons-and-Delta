use futures_util::{SinkExt, StreamExt};
use json::{JsonValue, object};
use log::*;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
};
use tokio_tungstenite::{accept_async, WebSocketStream};
use tungstenite::Message;
use uuid::Uuid;

#[derive(Debug)]
enum Error {
    Tungstenite(tungstenite::Error),
    SendError(SendError<Message>),
    Json(json::Error),
}

impl From<tungstenite::Error> for Error {
    fn from(e: tungstenite::Error) -> Self {
        Self::Tungstenite(e)
    }
}

impl From<SendError<Message>> for Error {
    fn from(e: SendError<Message>) -> Self {
        Self::SendError(e)
    }
}

impl From<json::Error> for Error {
    fn from(e: json::Error) -> Self {
        Self::Json(e)
    }
}

type Result<T> = std::result::Result<T, Error>;

struct ServerData {
    host_connected: bool,
    host: Option<HostPlayer>,
    clients: Vec<Player>,
    client_options: Vec<PlayerStat>,
}

impl ServerData {
    async fn send_all_but_host(
        &mut self,
        data: Message,
    ) -> std::result::Result<(), SendError<Message>> {
        for player in &mut self.clients {
            player.data_stream.send(data.clone())?;
        }

        Ok(())
    }

    async fn send_all_but_self(
        &mut self,
        data: Message,
        uuid: Uuid,
    ) -> std::result::Result<(), SendError<Message>> {
        for player in &mut self.clients {
            if player.uuid != uuid {
                player.data_stream.send(data.clone())?;
            }
        }

        if self.host_connected {
            self.host.as_mut().unwrap().data_stream.send(data.clone())?;
        }

        Ok(())
    }

    fn get_player(&mut self, uuid: Uuid) -> Option<&mut Player> {
        for player in &mut self.clients {
            if player.uuid == uuid {
                return Some(player);
            }
        }
        return None;
    }
}

struct HostPlayer {
    data_stream: UnboundedSender<Message>,
    peer: SocketAddr,
}

struct Player {
    data_stream: UnboundedSender<Message>,
    peer: SocketAddr,
    stat: Option<PlayerStat>,
    last_item: Option<Item>,
    uuid: Uuid,
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
            return format!(
                "You ate the {}.\n{} HP restored.",
                player.last_item.as_ref().unwrap().name,
                amount
            );
        }
        return "You aren't playing a character!".to_string();
    };

    closure
}

async fn accept_connection(data: Arc<Mutex<ServerData>>, peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(data, peer, stream).await {
        match e {
            Error::Tungstenite(
                tungstenite::Error::ConnectionClosed
                | tungstenite::Error::Protocol(_)
                | tungstenite::Error::Utf8,
            ) => (),
            err => error!("Error processing connection: {:?}", err),
        }
    }
}

async fn handle_connection(
    data: Arc<Mutex<ServerData>>,
    peer: SocketAddr,
    stream: TcpStream,
) -> Result<()> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");

    info!("New WebSocket connection: {}", peer);

    if !data.lock().await.host_connected {
        let rx = {
            let (tx, rx) = unbounded_channel();

            let mut lock = data.lock().await;
            lock.host = Some(HostPlayer {
                data_stream: tx,
                peer,
            });
            lock.host_connected = true;

            rx
        };

        return handle_host_wrapper(data, ws_stream, peer, rx).await;
    } else {
        return handle_client_wrapper(data, peer, ws_stream, uuid::Uuid::new_v4()).await;
    }
}

async fn handle_client_wrapper(
    data: Arc<Mutex<ServerData>>,
    peer: SocketAddr,
    stream: WebSocketStream<TcpStream>,
    uuid: Uuid
) -> Result<()> {
    if let Err(e) = handle_client(data.clone(), peer, stream, uuid).await {
        client_cleanup(&data, uuid).await;

        return Err(e);
    }

    Ok(())
}

async fn handle_client(
    data: Arc<Mutex<ServerData>>,
    peer: SocketAddr,
    mut stream: WebSocketStream<TcpStream>,
    uuid: Uuid
) -> Result<()> {
    info!("WHAT?");

    stream.send(Message::Text(json::stringify(object! {
        event_type: "player_status",
        status: "client",
        possible_players: data.lock().await.client_options.iter().map(|x: &PlayerStat| x.name.clone()).collect::<Vec<String>>()
    }))).await?;

    let (tx, mut rx) = unbounded_channel();

    data.lock().await.clients.push(Player {
        data_stream: tx,
        peer,
        stat: None,
        last_item: None,
        uuid: uuid.clone(),
    });

    let mut state = ClientState::SelectWho;

    loop {
        tokio::select! {
            w = stream.next() => {
                if let Some(Ok(message)) = w {
                    info!("{:?}", message.to_text());
                    client_on_message(message, &data, uuid, &mut stream, &mut state).await?;
                } else {
                    println!("w");
                    break;
                }
            }
            Some(message) = rx.recv() => {
                stream.send(message).await?;
            }
        }
    }

    client_cleanup(&data, uuid).await;

    Ok(())
}

async fn client_cleanup(data: &Arc<Mutex<ServerData>>, uuid: Uuid) {
    let mut lock = data.lock().await;

    let x = lock.get_player(uuid).unwrap();

    if x.stat.is_some() {
        let y = x.stat.take();

        lock.client_options.push(y.unwrap());
    }

    let idx = lock.clients.iter().position(|x| x.uuid == uuid).unwrap();
    lock.clients.remove(idx);
}

enum ClientState {
    SelectWho,
    Idle,
    Battle,
    Grid,
}

async fn client_on_message(
    message: Message,
    data: &Arc<Mutex<ServerData>>,
    uuid: Uuid,
    stream: &mut WebSocketStream<TcpStream>,
    state: &mut ClientState,
) -> Result<()> {

    if !message.is_text() && !message.is_binary() {
        return Ok(());
    }
    let text = message.to_text()?;

    let json = json::parse(text)?;

    //warn!("{:?}", json);

    match *state {
        ClientState::SelectWho => {
            if json.has_key("event_type") {
                info!("{:?}", json["event_type"]);
                match &json["event_type"] {
                    JsonValue::Short(str) => {
                        match str.as_str() {
                            "select_player" => {
                                if json.has_key("char") && json["char"].is_string() {
                                    let mut lock = data.lock().await;
                                    if lock.client_options.iter().any(|x| x.name == json["char"].as_str().unwrap() ) {
                                        let idx = lock.client_options.iter().position(|x| x.name == json["char"].as_str().unwrap()).unwrap();
                                        let x = lock.client_options.swap_remove(idx);
                                        lock.get_player(uuid).unwrap().stat = Some(x);

                                        stream.send(Message::Text(json::stringify(object! {
                                            event_type: "player_selected",
                                            successful: true
                                        }))).await?;

                                        let name = lock.get_player(uuid).unwrap().stat.as_ref().unwrap().name.clone();

                                        lock
                                            .send_all_but_self(
                                                Message::Text(json::stringify(object! {
                                                    event_type: "player_join",
                                                    uuid: uuid.to_string(),
                                                    stat_name: name,
                                                })),
                                                uuid,
                                            )
                                            .await?;
                                    } else {
                                        stream.send(Message::Text(json::stringify(object! {
                                            event_type: "player_selected",
                                            successful: false
                                        }))).await?;
                                    }
                                } else {
                                    stream.send(Message::text("invalid.")).await?;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        ClientState::Idle => {}
        ClientState::Battle => {}
        ClientState::Grid => {}
    }

    Ok(())
}

async fn handle_host_wrapper(
    data: Arc<Mutex<ServerData>>,
    stream: WebSocketStream<TcpStream>,
    peer: SocketAddr,
    rx: UnboundedReceiver<Message>,
) -> Result<()> {
    if let Err(e) = handle_host(data.clone(), stream, peer, rx).await {
        host_cleanup(&data).await;

        return Err(e);
    }

    Ok(())
}

async fn handle_host(
    data: Arc<Mutex<ServerData>>,
    mut stream: WebSocketStream<TcpStream>,
    peer: SocketAddr,
    mut rx: UnboundedReceiver<Message>,
) -> Result<()> {
    stream
        .send(Message::Text(json::stringify(object! {
            event_type: "player_status",
            status: "host",
        })))
        .await?;

    loop {
        tokio::select! {
            w = stream.next() => {
                if let Some(Ok(message)) = w {
                    info!("{:?}", message.to_text().unwrap().trim());

                    if message.is_text() {
                        // let text: &str = message.to_text()?;

                        // if text.trim() == "ping" {
                        //     data.lock().await.send_all_but_host(message).await?;
                        // }
                    }
                } else {
                    println!("w");
                    break;
                }
            }
            Some(message) = rx.recv() => {
                stream.send(message).await?;
            }
        }
    }

    host_cleanup(&data).await;

    Ok(())
}

async fn host_cleanup(data: &Arc<Mutex<ServerData>>) {
    let mut lock = data.lock().await;
    lock.host_connected = false;
    lock.host = None;
}

#[tokio::main]
async fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let addr = "0.0.0.0:8123";
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
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        let server_data = data.clone();
        tokio::spawn(accept_connection(server_data, peer, stream));
    }
}
