use futures_util::{SinkExt, StreamExt, TryStreamExt};
use json::object;
use log::*;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{net::{TcpListener, TcpStream}, sync::{Mutex, mpsc::{UnboundedReceiver, UnboundedSender, error::SendError, unbounded_channel}}};
use tokio_tungstenite::{accept_async, WebSocketStream};
use tungstenite::Message;

#[derive(Debug)]
enum Error {
    Tungstenite(tungstenite::Error),
    SendError(SendError<Message>),
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
        current: &mut Player,
    ) -> std::result::Result<(), SendError<Message>> {
        current.do_not_send = true;

        for player in &mut self.clients {
            if !player.do_not_send {
                player.data_stream.send(data.clone())?;
            }
        }

        if self.host_connected {
            self.host.as_mut().unwrap().data_stream.send(data.clone())?;
        }

        Ok(())
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
        return handle_client_wrapper(data, peer, ws_stream).await;
    }
}

async fn handle_client_wrapper(
    data: Arc<Mutex<ServerData>>,
    peer: SocketAddr,
    stream: WebSocketStream<TcpStream>,
) -> Result<()> {
    if let Err(e) = handle_client(data /*.clone()*/, peer, stream).await {
        // client_cleanup(&data);

        return Err(e);
    }

    Ok(())
}

async fn handle_client(
    data: Arc<Mutex<ServerData>>,
    peer: SocketAddr,
    mut stream: WebSocketStream<TcpStream>,
) -> Result<()> {
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
        do_not_send: false,
    });

    data.lock()
        .await
        .host
        .as_ref()
        .unwrap()
        .data_stream
        .send(Message::text("Hello, World"))?;

    loop {
        tokio::select! {
            w = stream.next() => {
                if let Some(Ok(message)) = w {
                    info!("{:?}", message.to_text())
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

    Ok(())
}

async fn handle_host_wrapper(
    data: Arc<Mutex<ServerData>>,
    stream: WebSocketStream<TcpStream>,
    peer: SocketAddr,
    rx: UnboundedReceiver<Message>
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
    mut rx: UnboundedReceiver<Message>
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
                        let text: &str = message.to_text()?;

                        if text.trim() == "ping" {
                            data.lock().await.send_all_but_host(message).await?;
                        }
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
