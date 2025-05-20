use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Local;
use dotenvy::dotenv;
use futures_util::StreamExt;
use futures_util::SinkExt;
use tokio::sync::mpsc;
use tokio::time::Duration;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeRequestPing,
    subscribe_update::UpdateOneof,
};
use jito_protos::shredstream::shredstream_proxy_client::ShredstreamProxyClient;
use jito_protos::shredstream::SubscribeEntriesRequest;

// 定义共享结构来存储最新的 slot 信息
struct SlotTracker {
    grpc_latest_slot: u64,
    shred_latest_slot: u64,
}

impl SlotTracker {
    fn new() -> Self {
        Self {
            grpc_latest_slot: 0,
            shred_latest_slot: 0,
        }
    }

    // 更新 GRPC 的最新 slot 并显示对比
    fn update_grpc_slot(&mut self, slot: u64) -> bool {
        let is_new = slot > self.grpc_latest_slot;
        if is_new {
            self.grpc_latest_slot = slot;
            self.print_comparison();
        }
        is_new
    }

    // 更新 SHRED 的最新 slot 并显示对比
    fn update_shred_slot(&mut self, slot: u64) -> bool {
        let is_new = slot > self.shred_latest_slot;
        if is_new {
            self.shred_latest_slot = slot;
            self.print_comparison();
        }
        is_new
    }

    // 打印两者的 slot 差距对比
    fn print_comparison(&self) {
        let diff = self.grpc_latest_slot as i64 - self.shred_latest_slot as i64;
        
        println!("┌─────────────────────────────────────────────────┐");
        println!("│ [{}] 实时 Slot 对比                       │", 
            Local::now().format("%H:%M:%S"));
        println!("├─────────────────────────────────────────────────┤");
        println!("│ GRPCS 最新 Slot: {:12}                   │", self.grpc_latest_slot);
        println!("│ SHRED 最新 Slot: {:12}                   │", self.shred_latest_slot);
        println!("├─────────────────────────────────────────────────┤");
        
        if diff > 0 {
            println!("│ 🔵 GRPC 领先 SHRED: {} 个 slot                   │", diff);
        } else if diff < 0 {
            println!("│ 🟢 SHRED 领先 GRPC: {} 个 slot                   │", -diff);
        } else {
            println!("│ 🟡 两者完全同步: 差距 0 个 slot                 │");
        }
        println!("└─────────────────────────────────────────────────┘");
    }
}

// GRPC 订阅函数
async fn subscribe_grpc(tx: mpsc::Sender<u64>) {
    dotenv().ok();
    let url = std::env::var("GRPC_URL").expect("GRPC_URL must be set");
    println!("正在连接 GRPC 服务: {}", url);
    
    let mut client = GeyserGrpcClient::build_from_shared(url)
        .unwrap()
        .tls_config(ClientTlsConfig::new().with_native_roots())
        .unwrap()
        .connect()
        .await
        .unwrap();

    let subscribe_request = SubscribeRequest {
        transactions: std::collections::HashMap::from([(
            "speed_test".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include: vec![],
                account_exclude: vec![],
                account_required: vec![],
            },
        )]),
        commitment: Some(CommitmentLevel::Processed.into()),
        ..Default::default()
    };

    let (mut subscribe_tx, mut stream) = client
        .subscribe_with_request(Some(subscribe_request))
        .await
        .unwrap();

    let mut last_slot = 0;
    println!("GRPC 服务连接成功，开始接收数据...");
    
    while let Some(message) = stream.next().await {
        match message {
            Ok(msg) => match msg.update_oneof {
                Some(UpdateOneof::Transaction(sut)) => {
                    if sut.slot != last_slot {
                        last_slot = sut.slot;
                        let _ = tx.send(sut.slot).await;
                    }
                }
                Some(UpdateOneof::Ping(_)) => {
                    let _ = subscribe_tx
                        .send(SubscribeRequest {
                            ping: Some(SubscribeRequestPing { id: 1 }),
                            ..Default::default()
                        })
                        .await;
                }
                _ => {}
            },
            Err(e) => {
                println!("GRPC 错误: {:?}", e);
                break;
            }
        }
    }
}

// SHRED 订阅函数
async fn subscribe_shred(tx: mpsc::Sender<u64>) {
    dotenv().ok();
    let url = std::env::var("SHRED_URL").expect("SHRED_URL must be set");
    println!("正在连接 SHRED 服务: {}", url);
    
    let mut client = ShredstreamProxyClient::connect(url)
        .await
        .unwrap();
    let mut stream = client
        .subscribe_entries(SubscribeEntriesRequest {})
        .await
        .unwrap()
        .into_inner();

    let mut last_slot = 0;
    println!("SHRED 服务连接成功，开始接收数据...");
    
    while let Some(slot_entry) = stream.message().await.unwrap() {
        if slot_entry.slot != last_slot {
            last_slot = slot_entry.slot;
            let _ = tx.send(slot_entry.slot).await;
        }
    }
}

#[tokio::main]
async fn main() {
    println!("⭐ 启动 GRPC 与 SHRED slot 对比监控 ⭐");
    dotenv().ok();
    println!("目标端点: ");
    println!("  GRPC:  {}", std::env::var("GRPC_URL").unwrap_or_else(|_| "未设置".to_string()));
    println!("  SHRED: {}", std::env::var("SHRED_URL").unwrap_or_else(|_| "未设置".to_string()));
    
    // 创建通道用于接收 slot 数据
    let (grpc_tx, mut grpc_rx) = mpsc::channel::<u64>(100);
    let (shred_tx, mut shred_rx) = mpsc::channel::<u64>(100);
    
    // 启动订阅任务
    tokio::spawn(subscribe_grpc(grpc_tx));
    tokio::spawn(subscribe_shred(shred_tx));
    
    // 创建 slot 跟踪器
    let mut tracker = SlotTracker::new();
    
    // 监控持续时间
    let monitor_duration = Duration::from_secs(3600); // 默认监控1小时
    let start_time = tokio::time::Instant::now();
    
    // 处理接收到的 slot 数据
    loop {
        tokio::select! {
            Some(slot) = grpc_rx.recv() => {
                tracker.update_grpc_slot(slot);
            }
            Some(slot) = shred_rx.recv() => {
                tracker.update_shred_slot(slot);
            }
            _ = tokio::time::sleep_until(start_time + monitor_duration) => {
                println!("监控时间结束");
                break;
            }
        }
    }
}