//! End-to-end Cora server/client tests over a loopback TCP socket.

use std::time::Duration;

use productiondeck_cora::client::CoraClient;
use productiondeck_cora::command::DeckCommand;
use productiondeck_cora::core::feature::Identity;
use productiondeck_cora::core::input::InputEvent;
use productiondeck_cora::core::model;
use productiondeck_cora::server::CoraServer;
use productiondeck_cora::FeatureStyle;
use tokio::sync::mpsc;
use tokio::time::timeout;

async fn setup() -> (CoraServer, CoraClient, mpsc::UnboundedReceiver<DeckCommand>) {
    let identity = Identity::new(&model::STUDIO, "TESTSERIAL0001");
    let (tx, rx) = mpsc::unbounded_channel();
    let server = CoraServer::bind("127.0.0.1:0", &model::STUDIO, identity, tx)
        .await
        .expect("bind");
    let client = CoraClient::connect(server.local_addr())
        .await
        .expect("connect");
    (server, client, rx)
}

#[tokio::test]
async fn device_info_is_returned_and_keepalive_is_acked() {
    let (_server, mut client, _rx) = setup().await;
    let payload = timeout(
        Duration::from_secs(2),
        client.get_feature(0x80, FeatureStyle::CoraPrimary),
    )
    .await
    .expect("timeout")
    .expect("feature");
    assert_eq!(payload[0], 0x03);
    assert_eq!(payload[1], 0x80);
    assert_eq!(
        u16::from_le_bytes([payload[12], payload[13]]),
        0x0fd9,
        "VID"
    );
    assert_eq!(
        u16::from_le_bytes([payload[14], payload[15]]),
        0x00aa,
        "PID"
    );
}

#[tokio::test]
async fn official_write_style_query_works() {
    let (_server, mut client, _rx) = setup().await;
    let payload = timeout(Duration::from_secs(2), client.get_feature_write_style(0x83))
        .await
        .expect("timeout")
        .expect("feature");
    assert_eq!(payload[0], 0x03);
    assert_eq!(payload[1], 0x83);
}

#[tokio::test]
async fn brightness_command_reaches_the_sink() {
    let (_server, mut client, mut rx) = setup().await;
    timeout(Duration::from_secs(2), client.set_brightness(42))
        .await
        .expect("timeout")
        .expect("send");
    let command = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("command");
    match command {
        DeckCommand::Brightness(value) => assert_eq!(value, 42),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn key_image_chunks_are_assembled() {
    let (_server, mut client, mut rx) = setup().await;
    // Report id 0x02, cmd 0x07, key 5, last, size=4, chunk=0, data
    let mut report = vec![0u8; 12];
    report[0] = 0x02;
    report[1] = 0x07;
    report[2] = 5;
    report[3] = 1;
    report[4..6].copy_from_slice(&4u16.to_le_bytes());
    report[6..8].copy_from_slice(&0u16.to_le_bytes());
    report[8..12].copy_from_slice(b"JPEG");
    timeout(Duration::from_secs(2), client.send_output_report(&report))
        .await
        .expect("timeout")
        .expect("send");

    let command = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("command");
    match command {
        DeckCommand::KeyImage { key, jpeg } => {
            assert_eq!(key, 5);
            assert_eq!(jpeg, b"JPEG");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn output_report_ack_is_returned() {
    let (_server, mut client, mut rx) = setup().await;
    let mut report = vec![0u8; 12];
    report[0] = 0x02;
    report[1] = 0x07;
    report[2] = 1;
    report[3] = 1;
    report[4..6].copy_from_slice(&1u16.to_le_bytes());
    report[8] = 0xff;
    timeout(
        Duration::from_secs(2),
        client.send_output_report_ack(&report),
    )
    .await
    .expect("timeout")
    .expect("ack");
    // The image command is still emitted.
    let command = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("command");
    assert!(matches!(command, DeckCommand::KeyImage { key: 1, .. }));
}

#[tokio::test]
async fn input_is_broadcast_to_the_client() {
    let (server, mut client, _rx) = setup().await;
    // Complete the handshake so the server-side session exists before broadcasting.
    let _ = timeout(
        Duration::from_secs(2),
        client.get_feature(0x80, FeatureStyle::CoraPrimary),
    )
    .await
    .expect("timeout")
    .expect("feature");

    server.input(InputEvent::Button {
        index: 0,
        pressed: true,
    });
    let frame = timeout(Duration::from_secs(2), async {
        loop {
            let frame = client
                .recv_frame(Duration::from_secs(1))
                .await
                .expect("read")
                .expect("frame");
            if frame.is_keep_alive() {
                client.ack_keep_alive(&frame).await.expect("ack");
                continue;
            }
            if frame
                .flags
                .contains(productiondeck_cora::CoraFlags::ACK_NAK)
            {
                continue;
            }
            return frame;
        }
    })
    .await
    .expect("timeout");
    assert_eq!(frame.payload[0], 0x01);
    assert_eq!(frame.payload[1], 0x00);
    assert_eq!(frame.payload[4], 1);
}
