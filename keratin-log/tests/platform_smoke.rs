use keratin_log::{KDurability, Keratin, KeratinConfig, Message};

fn message(index: u64) -> Message {
    let mut payload = vec![0x5a; 1024];
    payload[..8].copy_from_slice(&index.to_le_bytes());
    Message {
        payload,
        flags: 0,
        headers: vec![],
    }
}

#[tokio::test]
async fn durable_append_reopen_and_continue() {
    let dir = keratin_log::test_dir!("platform_smoke");
    let config = KeratinConfig {
        segment_preallocate_bytes: 8 << 20,
        ..KeratinConfig::test_default()
    };
    let log = Keratin::open(&dir.root, config.clone()).await.unwrap();
    log.append_batch(
        (0..128).map(message).collect(),
        Some(KDurability::AfterFsync),
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    log.append_batch(
        (128..256).map(message).collect(),
        Some(KDurability::AfterFsync),
    )
    .await
    .unwrap();
    log.force_close().await.unwrap();
    let log = Keratin::open(&dir.root, config).await.unwrap();
    log.append_batch(
        (256..384).map(message).collect(),
        Some(KDurability::AfterFsync),
    )
    .await
    .unwrap();
    let records = log.reader().scan_from(0, 400).unwrap();
    assert_eq!(records.len(), 384);
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.offset, index as u64);
        assert_eq!(record.payload, message(index as u64).payload);
    }
    log.shutdown().await.unwrap();
}
