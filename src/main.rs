
use futures::{stream::StreamExt, sink::SinkExt, channel::mpsc};

#[derive(serde::Deserialize)]
pub struct GatewayUrlResult {
    url: String
}
#[derive(serde::Deserialize, serde::Serialize)]
pub struct GatewayMessageFrame {
    op: u32,
    d: Option<serde_json::Value>,
    s: Option<u32>,
    t: Option<String>
}
impl GatewayMessageFrame {
    pub fn new(op: u32) -> Self {
        GatewayMessageFrame {
            op,
            d: None,
            s: None,
            t: None
        }
    }

    pub fn data<T: serde::Serialize>(mut self, data: T) -> Self {
        self.d = Some(serde_json::to_value(data).expect("Failed to serialize data"));
        self
    }
}
const GATEWAY_OP_EVENT: u32 = 0;
const GATEWAY_OP_HEARTBEAT: u32 = 1;
const GATEWAY_OP_IDENTITY: u32 = 2;
const GATEWAY_OP_HELLO: u32 = 10;
const GATEWAY_OP_HEARTBEAT_ACK: u32 = 11;

#[derive(serde::Deserialize)]
pub struct GatewayHelloData {
    pub heartbeat_interval: u64
}

#[derive(serde::Deserialize)]
pub struct GatewayReadyData {
    pub user: User
}

#[derive(serde::Serialize)]
pub struct GatewayIdentityData<'s> {
    pub token: &'s str,
    pub properties: GatewayIdentityProperties<'s>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compress: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_threshold: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shard: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<GatewayStatusUpdateData<'s>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_subscriptions: Option<bool>,
    pub intents: u32
}
impl<'s> GatewayIdentityData<'s> {
    pub fn new(token: &'s str, properties: GatewayIdentityProperties<'s>, intents: u32) -> Self {
        GatewayIdentityData {
            token, properties, intents,
            compress: None,
            large_threshold: None,
            shard: None,
            presence: None,
            guild_subscriptions: None
        }
    }

    pub fn compress(mut self, enable: bool) -> Self {
        self.compress = Some(enable);
        self
    }
    pub fn large_threshold(mut self, thres: u32) -> Self {
        self.large_threshold = Some(thres);
        self
    }
    pub fn shard(mut self, id: u32, num: u32) -> Self {
        self.shard = Some([id, num]);
        self
    }
    pub fn presence(mut self, status: GatewayStatusUpdateData<'s>) -> Self {
        self.presence = Some(status);
        self
    }
    pub fn guild_subscription(mut self, enable: bool) -> Self {
        self.guild_subscriptions = Some(enable);
        self
    }
}
#[derive(serde::Serialize)]
pub struct GatewayIdentityProperties<'s> {
    #[serde(rename = "$os")]
    pub os: &'s str,
    #[serde(rename = "$browser")]
    pub browser: &'s str,
    #[serde(rename = "$device")]
    pub device: &'s str
}
const GATEWAY_INTENT_BIT_GUILDS: u32 = 1 << 0;
const GATEWAY_INTENT_BIT_GUILD_MEMBERS: u32 = 1 << 1;
const GATEWAY_INTENT_BIT_GUILD_BANS: u32 = 1 << 2;
const GATEWAY_INTENT_BIT_GUILD_EMOJIS: u32 = 1 << 3;
const GATEWAY_INTENT_BIT_GUILD_INTEGRATIONS: u32 = 1 << 4;
const GATEWAY_INTENT_BIT_GUILD_WEBHOOKS: u32 = 1 << 5;
const GATEWAY_INTENT_BIT_GUILD_INVITES: u32 = 1 << 6;
const GATEWAY_INTENT_BIT_GUILD_VOICE_STATES: u32 = 1 << 7;
const GATEWAY_INTENT_BIT_GUILD_PRESENCES: u32 = 1 << 8;
const GATEWAY_INTENT_BIT_GUILD_MESSAGES: u32 = 1 << 9;
const GATEWAY_INTENT_BIT_GUILD_MESSAGE_REACTIONS: u32 = 1 << 10;
const GATEWAY_INTENT_BIT_GUILD_MESSAGE_TYPING: u32 = 1 << 11;
const GATEWAY_INTENT_BIT_DIRECT_MESSAGES: u32 = 1 << 12;
const GATEWAY_INTENT_BIT_DIRECT_MESSAGE_REACTIONS: u32 = 1 << 13;
const GATEWAY_INTENT_BIT_DIRECT_MESSAGE_TYPING: u32 = 1 << 14;

#[derive(serde::Serialize)]
pub struct GatewayStatusUpdateData<'s> {
    pub since: Option<u64>,
    pub status: &'s str,
    pub afk: bool
}

type Snowflake = String;

#[derive(serde::Deserialize)]
pub struct User {
    pub id: Snowflake,
    pub username: String
}

#[derive(serde::Deserialize)]
pub struct Message {
    pub content: String,
    pub author: User,
    pub channel_id: Snowflake
}

#[derive(serde::Serialize)]
pub struct CreateMessagePayload<'s> {
    pub content: &'s str
}

const DISCORD_TOKEN: &'static str = include_str!("../.discord_token");

#[async_std::main]
async fn main() {
    let ws_endpoint = surf::get("https://discord.com/api/gateway")
        .recv_json::<GatewayUrlResult>().await
        .expect("Failed to get gateway url");
    println!("url: {:?}", ws_endpoint.url);
    let ep_url = format!("{}{}?v=8&encoding=json", ws_endpoint.url, if ws_endpoint.url.ends_with("/") { "" } else { "/" });
    let ep_domain = ws_endpoint.url.splitn(2, "//").nth(1).expect("no domain?");
    let tcp_stream = async_std::net::TcpStream::connect(format!("{}:443", ep_domain)).await
        .expect("Failed to connect remote server");
    let enc_stream = async_tls::TlsConnector::default().connect(ep_domain, tcp_stream).await
        .expect("Failed to connect encrypted stream");
    let (mut c, _) = async_tungstenite::client_async(ep_url, enc_stream).await
        .expect("Failed to connect Discord Gateway");
    let heartbeat_interval = loop {
        let x = c.next().await.expect("No data received").expect("Failed to decode incoming messages");
        match x {
            tungstenite::Message::Text(s) => {
                println!("text: {:?}", s);
                let xd: GatewayMessageFrame = serde_json::from_str(&s).expect("Failed to deserialize message");
                if xd.op == GATEWAY_OP_HELLO {
                    let hd: GatewayHelloData = serde_json::from_value(xd.d.expect("no hello data?"))
                        .expect("Failed to deserialize hello data");
                    break hd.heartbeat_interval;
                }
            },
            _ => ()
        }
    };
    let (w, mut r) = c.split();
    let w = async_std::sync::Arc::new(async_std::sync::RwLock::new(w));
    let w2 = w.clone();
    let (mut heartbeat_updates_sender, mut heartbeat_updates_receiver) = mpsc::channel::<u32>(1);
    async_std::task::spawn(async move {
        let mut last_sequence_id = None::<u32>;
        loop {
            let u = async_std::future::timeout(
                std::time::Duration::from_millis(heartbeat_interval),
                heartbeat_updates_receiver.next()
            ).await;
            match u {
                Ok(Some(v)) => {
                    last_sequence_id = Some(v);
                },
                // channel vanished
                Ok(None) => break,
                Err(_) => 
                    w2.write().await.send(tungstenite::Message::Text(serde_json::to_string(
                        &GatewayMessageFrame::new(GATEWAY_OP_HEARTBEAT).data(last_sequence_id)
                    ).expect("Failed to serialize"))).await.expect("Failed to send heartbeat message")
            }
        }
    });
    w.write().await.send(tungstenite::Message::Text(serde_json::to_string(
        &GatewayMessageFrame::new(GATEWAY_OP_IDENTITY).data(
            GatewayIdentityData::new(
                DISCORD_TOKEN, GatewayIdentityProperties {
                    os: "windows",
                    browser: "koyuki bot test",
                    device: "pc"
                },
                GATEWAY_INTENT_BIT_GUILD_MESSAGES
            )
        )
    ).expect("Failed to serialize"))).await.expect("Failed to send identify message");
    let mut bot_user_id = None;
    while let Some(x) = r.next().await {
        match x.expect("receive failed") {
            tungstenite::Message::Text(t) => {
                let f: GatewayMessageFrame = serde_json::from_str(&t).expect("Failed to deserialize text frame");
                if let Some(s) = f.s {
                    heartbeat_updates_sender.send(s).await.expect("Failed to send heartbeat updates");
                }
                println!("data: {:?}", t);

                if f.op == GATEWAY_OP_EVENT {
                    // generic event
                    if f.t.as_deref().map_or(false, |s| s == "READY") {
                        let d: GatewayReadyData = serde_json::from_value(f.d.expect("no message data?"))
                            .expect("Failed to deserialize message data");
                        bot_user_id = Some(d.user.id);
                    } else if f.t.as_deref().map_or(false, |s| s == "MESSAGE_CREATE") {
                        let d: Message = serde_json::from_value(f.d.expect("no message data?"))
                            .expect("Failed to deserialize message data");
                        if let Some(bot_uid) = bot_user_id.as_deref() {
                            // 自分自身には反応しない
                            if bot_uid == d.author.id { continue; }

                            let resp_content = format!("<@{}> にゃーん！", d.author.id);
                            let resp = CreateMessagePayload {
                                content: &resp_content
                            };
                            let resp = surf::post(format!("https://discord.com/api/channels/{}/messages", d.channel_id))
                                .body(serde_json::to_string(&resp).expect("Failed to serialize"))
                                .header("Content-Type", "application/json")
                                .header("Authorization", concat!("Bot ", include_str!("../.discord_token")))
                                .recv_string().await
                                .expect("Failed to send CreateMessage");
                            println!("CreateMessage resp: {}", resp);
                        }
                    }
                }
            },
            e => println!("non-text event: {:?}", e)
        }
    }
}
