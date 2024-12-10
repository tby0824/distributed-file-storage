// src/node_discovery.rs
use libp2p::{
    identity, PeerId, Multiaddr,
    core::upgrade,
    tcp::TcpTransport,
    noise::{NoiseConfig, Keypair, X25519Spec},
    yamux::YamuxConfig,

    gossipsub::{Gossipsub, GossipsubEvent, GossipsubConfig, IdentTopic, MessageAuthenticity},
    mdns::{async_io::Mdns, MdnsConfig, MdnsEvent},
    identify::{Identify, IdentifyConfig},
    swarm::{SwarmBuilder, SwarmEvent},
    NetworkBehaviour, Swarm, Transport,
};
use tokio::sync::mpsc;
use crate::node::communication::{NodeStorage, ChunkMessage, CommunicationController, ArcMutexReceiver};
use serde_json;
use log::{info, error};

// 定义 NetworkBehaviour 的组合类型
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "MyBehaviourEvent")]
struct MyBehaviour {
    gossipsub: Gossipsub,
    mdns: Mdns,
    identify: Identify,
}

#[allow(clippy::large_enum_variant)]
enum MyBehaviourEvent {
    Gossipsub(GossipsubEvent),
    Mdns(MdnsEvent),
    Identify(libp2p::identify::Event),
}

impl From<GossipsubEvent> for MyBehaviourEvent {
    fn from(e: GossipsubEvent) -> Self {
        MyBehaviourEvent::Gossipsub(e)
    }
}

impl From<MdnsEvent> for MyBehaviourEvent {
    fn from(e: MdnsEvent) -> Self {
        MyBehaviourEvent::Mdns(e)
    }
}

impl From<libp2p::identify::Event> for MyBehaviourEvent {
    fn from(e: libp2p::identify::Event) -> Self {
        MyBehaviourEvent::Identify(e)
    }
}

pub async fn start_node_discovery() -> anyhow::Result<CommunicationController> {
    // 生成本地密钥对
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    // 构建 Noise 安全传输层
    let noise_keys = Keypair::<X25519Spec>::new()
        .into_authentic(&local_key)
        .expect("Signing libp2p-noise static keypair failed.");
    let noise = NoiseConfig::xx(noise_keys).into_authenticated();

    // 构建传输层：TCP + Noise + Yamux
    let transport = TcpTransport::new(libp2p::tcp::TokioTcpConfig::new())
        .upgrade(upgrade::Version::V1)
        .authenticate(noise)
        .multiplex(YamuxConfig::default())
        .boxed();

    // 构建Gossipsub, Mdns, Identify行为
    let gossipsub = Gossipsub::new(
        MessageAuthenticity::Signed(local_key.clone()),
        GossipsubConfig::default()
    )?;

    let mdns = Mdns::new(MdnsConfig::default()).await?;
    let identify = Identify::new(IdentifyConfig::new("my-protocol/1.0.0".to_string(), local_key.public()));

    let mut behaviour = MyBehaviour {
        gossipsub,
        mdns,
        identify,
    };

    let topic = IdentTopic::new("chunk-transfer");
    behaviour.gossipsub.subscribe(&topic)?;

    // 构建 Swarm
    let mut swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build();

    // 监听地址
    let addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse().unwrap();
    Swarm::listen_on(&mut swarm, addr)?;

    // 创建消息通道
    let (msg_sender, mut msg_receiver) = mpsc::channel::<ChunkMessage>(100);
    let (resp_sender, resp_receiver) = mpsc::channel::<ChunkMessage>(100);
    let resp_receiver = ArcMutexReceiver::new(resp_receiver);

    // 初始化本地存储
    let local_storage = NodeStorage::new("./data/chunks");

    // 启动处理 Swarm 事件的任务
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("Node listening on {:?}", address);
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(g_event)) => {
                            if let GossipsubEvent::Message { message, .. } = g_event {
                                if let Ok(msg) = serde_json::from_slice::<ChunkMessage>(&message.data) {
                                    match msg {
                                        ChunkMessage::StoreChunk { chunk_id, chunk_data } => {
                                            if let Err(e) = local_storage.store_chunk(chunk_id, &chunk_data) {
                                                error!("Failed to store chunk {}: {:?}", chunk_id, e);
                                            } else {
                                                info!("Stored chunk {}", chunk_id);
                                            }
                                        },
                                        ChunkMessage::GetChunk { chunk_id } => {
                                            if let Some(data) = local_storage.get_chunk(&chunk_id) {
                                                let response = ChunkMessage::ChunkResponse { chunk_id, chunk_data: data };
                                                let data = serde_json::to_vec(&response).unwrap();
                                                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), data) {
                                                    error!("Failed to publish response: {:?}", e);
                                                }
                                            } else {
                                                error!("Requested chunk {} not found locally", chunk_id);
                                            }
                                        },
                                        ChunkMessage::ChunkResponse { chunk_id, chunk_data } => {
                                            let _ = resp_sender.send(ChunkMessage::ChunkResponse { chunk_id, chunk_data }).await;
                                        }
                                    }
                                } else {
                                    error!("Failed to deserialize incoming message");
                                }
                            }
                        },
                        _ => {}
                    }
                },
                outbound = msg_receiver.recv() => {
                    if let Some(msg) = outbound {
                        let data = serde_json::to_vec(&msg).unwrap();
                        if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), data) {
                            error!("Failed to publish message: {:?}", e);
                        }
                    }
                }
            }
        }
    });

    Ok(CommunicationController {
        request_sender: msg_sender,
        response_receiver: resp_receiver,
    })
}
