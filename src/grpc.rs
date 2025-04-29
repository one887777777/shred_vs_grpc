use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use dotenvy::dotenv;
use futures_util::SinkExt;
use log::{error, info};
use tokio_stream::StreamExt;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeRequestPing,
    subscribe_update::UpdateOneof,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    pretty_env_logger::init();

    let url = std::env::var("GRPC_URL").expect("GRPC_URL must be set");
    
    // 创建 gRPC 客户端
    let mut client = GeyserGrpcClient::build_from_shared(url)?
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .connect()
        .await?;

    // 创建订阅请求
    let subscribe_request = SubscribeRequest {
        transactions: HashMap::from([(
            "client".to_string(),
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

    // 开始订阅
    let (mut subscribe_tx, mut stream) = client
        .subscribe_with_request(Some(subscribe_request))
        .await?;

    let mut last_slot = 0;
    while let Some(message) = stream.next().await {
        match message {
            Ok(msg) => {
                match msg.update_oneof {
                    Some(UpdateOneof::Transaction(sut)) => {
                        if sut.slot != last_slot {
                            last_slot = sut.slot;
                            let timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis();
                            info!("Slot: {}, Timestamp: {}", sut.slot, timestamp);
                        }
                    }
                    Some(UpdateOneof::Ping(_)) => {
                        // 发送 ping 保持连接
                        let _ = subscribe_tx
                            .send(SubscribeRequest {
                                ping: Some(SubscribeRequestPing { id: 1 }),
                                ..Default::default()
                            })
                            .await;
                    }
                    Some(UpdateOneof::Pong(_)) => {}
                    _ => {}
                }
            }
            Err(error) => {
                error!("Error: {:?}", error);
                break;
            }
        }
    }
    Ok(())
}
