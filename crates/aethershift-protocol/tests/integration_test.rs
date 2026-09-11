use aethershift_protocol::{
    ClientStream, ProtocolError, Request, Response, ServerStream, StatusInfo, call,
    new_length_delimited_codec, read_request, read_response, send_request, send_response,
};
use bytes::Bytes;
use futures::SinkExt;
use tokio::io::duplex;
use tokio::net::UnixListener;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_full_request_response_cycle() {
    let (client_io, server_io) = duplex(4096);
    let mut client = ClientStream::new(client_io);
    let mut server = ServerStream::new(server_io);

    // ListProfiles
    client
        .send(&Request::ListProfiles)
        .await
        .expect("send ListProfiles");
    let req = server.recv().await.expect("recv ListProfiles");
    assert_eq!(req, Request::ListProfiles);
    server
        .send(&Response::ok("Profiles listed"))
        .await
        .expect("send Response");
    let resp = client.recv().await.expect("recv Response");
    assert_eq!(resp, Response::ok("Profiles listed"));

    // Switch
    let switch_req = Request::Switch {
        profile: "cyberpunk".into(),
        force: true,
    };
    client.send(&switch_req).await.expect("send Switch");
    let req = server.recv().await.expect("recv Switch");
    assert_eq!(req, switch_req);
    server
        .send(&Response::ok("Switched to cyberpunk"))
        .await
        .expect("send Response");
    let resp = client.recv().await.expect("recv Response");
    assert_eq!(resp, Response::ok("Switched to cyberpunk"));

    // Status
    client.send(&Request::Status).await.expect("send Status");
    let req = server.recv().await.expect("recv Status");
    assert_eq!(req, Request::Status);
    let status_info = StatusInfo::new("cyberpunk", 4, 1024, "0.1.0");
    let status_resp = Response::ok_with_data("Current status", &status_info).expect("status resp");
    server.send(&status_resp).await.expect("send status resp");
    let resp = client.recv().await.expect("recv status resp");
    assert_eq!(resp, status_resp);

    // Cycle
    client.send(&Request::Cycle).await.expect("send Cycle");
    let req = server.recv().await.expect("recv Cycle");
    assert_eq!(req, Request::Cycle);

    // Restore
    client.send(&Request::Restore).await.expect("send Restore");
    let req = server.recv().await.expect("recv Restore");
    assert_eq!(req, Request::Restore);

    // Shutdown
    client
        .send(&Request::Shutdown)
        .await
        .expect("send Shutdown");
    let req = server.recv().await.expect("recv Shutdown");
    assert_eq!(req, Request::Shutdown);
}

#[tokio::test]
async fn test_convenience_read_send_functions() {
    let (client_io, server_io) = duplex(4096);
    let mut client_framed = Framed::new(client_io, new_length_delimited_codec());
    let mut server_framed = Framed::new(server_io, new_length_delimited_codec());

    let req = Request::Cycle;
    send_request(&mut client_framed, &req)
        .await
        .expect("send_request");

    let received_req = read_request(&mut server_framed)
        .await
        .expect("read_request");
    assert_eq!(received_req, req);

    let resp = Response::ok("Cycled");
    send_response(&mut server_framed, &resp)
        .await
        .expect("send_response");

    let received_resp = read_response(&mut client_framed)
        .await
        .expect("read_response");
    assert_eq!(received_resp, resp);
}

#[tokio::test]
async fn test_framing_prevents_sticky_packets() {
    let (client_io, server_io) = duplex(4096);
    let mut client = ClientStream::new(client_io);
    let mut server = ServerStream::new(server_io);

    // Send 3 requests back to back without waiting
    client.send(&Request::Status).await.expect("send 1");
    client.send(&Request::Cycle).await.expect("send 2");
    client.send(&Request::Restore).await.expect("send 3");

    // Server should receive all 3 cleanly separated
    let r1 = server.recv().await.expect("recv 1");
    assert_eq!(r1, Request::Status);

    let r2 = server.recv().await.expect("recv 2");
    assert_eq!(r2, Request::Cycle);

    let r3 = server.recv().await.expect("recv 3");
    assert_eq!(r3, Request::Restore);
}

#[tokio::test]
async fn test_invalid_json_frame() {
    let (client_io, server_io) = duplex(1024);
    let mut raw_framed_client = Framed::new(client_io, new_length_delimited_codec());
    let mut server = ServerStream::new(server_io);

    // Send valid frame with invalid JSON
    raw_framed_client
        .send(Bytes::from_static(b"{invalid json}"))
        .await
        .expect("send raw");

    let err = server.recv().await.expect_err("should fail json parsing");
    match err {
        ProtocolError::Json(_) => {}
        other => panic!("expected ProtocolError::Json, got {:?}", other),
    }
}

#[tokio::test]
async fn test_unix_domain_socket_and_call_helper() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("aethershift_test_{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path).expect("bind unix socket");

    let server_task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut server = ServerStream::new(stream);
        let req = server.recv().await.expect("server recv");
        if let Request::Status = req {
            let status = StatusInfo::new("test_profile", 0, 10, "0.1.0");
            let resp = Response::ok_with_data("OK", &status).expect("ok_with_data");
            server.send(&resp).await.expect("server send");
        } else {
            panic!("unexpected request: {:?}", req);
        }
    });

    let resp = call(&socket_path, &Request::Status)
        .await
        .expect("call helper");

    server_task.await.expect("server task");

    let _ = std::fs::remove_file(&socket_path);

    match resp {
        Response::Success { message, data } => {
            assert_eq!(message, "OK");
            let data = data.expect("data present");
            let status: StatusInfo = serde_json::from_value(data).expect("parse status");
            assert_eq!(status.active_profile, "test_profile");
        }
        Response::Error { .. } => panic!("expected success"),
        Response::Event { .. } => panic!("expected success"),
    }
}

#[tokio::test]
async fn test_phase3_requests_cycle() {
    use aethershift_protocol::{Recommendation, SnapLayout, UsageStats};

    let (client_io, server_io) = duplex(8192);
    let mut client = ClientStream::new(client_io);
    let mut server = ServerStream::new(server_io);

    // ApplyLayout
    let layout_req = Request::ApplyLayout {
        layout: SnapLayout::TwoThirdsLeft,
        preview: false,
    };
    client.send(&layout_req).await.expect("send ApplyLayout");
    let req = server.recv().await.expect("recv ApplyLayout");
    assert_eq!(req, layout_req);
    server
        .send(&Response::ok("Layout applied"))
        .await
        .expect("send ok");
    let resp = client.recv().await.expect("recv ok");
    assert_eq!(resp, Response::ok("Layout applied"));

    // MoveWindowToMonitor
    let move_req = Request::MoveWindowToMonitor {
        direction: "left".into(),
    };
    client
        .send(&move_req)
        .await
        .expect("send MoveWindowToMonitor");
    let req = server.recv().await.expect("recv MoveWindowToMonitor");
    assert_eq!(req, move_req);

    // CreateProfile
    let create_req = Request::CreateProfile {
        name: "dev".into(),
        description: Some("Developer layout".into()),
        copy_from: Some("windows".into()),
    };
    client.send(&create_req).await.expect("send CreateProfile");
    let req = server.recv().await.expect("recv CreateProfile");
    assert_eq!(req, create_req);

    // UpdateBinding
    let update_req = Request::UpdateBinding {
        profile: "dev".into(),
        key_combo: "Super+Alt+T".into(),
        action: "spawn_alacritty".into(),
        description: Some("Alacritty terminal".into()),
    };
    client.send(&update_req).await.expect("send UpdateBinding");
    let req = server.recv().await.expect("recv UpdateBinding");
    assert_eq!(req, update_req);

    // RemoveBinding
    let remove_req = Request::RemoveBinding {
        profile: "dev".into(),
        key_combo: "Super+Alt+T".into(),
    };
    client.send(&remove_req).await.expect("send RemoveBinding");
    let req = server.recv().await.expect("recv RemoveBinding");
    assert_eq!(req, remove_req);

    // SaveProfile
    let save_req = Request::SaveProfile {
        profile: "dev".into(),
    };
    client.send(&save_req).await.expect("send SaveProfile");
    let req = server.recv().await.expect("recv SaveProfile");
    assert_eq!(req, save_req);

    // DeleteProfile
    let delete_req = Request::DeleteProfile {
        profile: "dev".into(),
    };
    client.send(&delete_req).await.expect("send DeleteProfile");
    let req = server.recv().await.expect("recv DeleteProfile");
    assert_eq!(req, delete_req);

    // GetStats
    client
        .send(&Request::GetStats)
        .await
        .expect("send GetStats");
    let req = server.recv().await.expect("recv GetStats");
    assert_eq!(req, Request::GetStats);
    let mut stats = UsageStats::default();
    stats.total_switches = 42;
    stats.profile_usage.insert("dev".into(), 10);
    let stats_resp = Response::ok_with_data("Current stats", &stats).expect("stats resp");
    server.send(&stats_resp).await.expect("send stats resp");
    let resp = client.recv().await.expect("recv stats resp");
    assert_eq!(resp, stats_resp);

    // GetRecommendations
    client
        .send(&Request::GetRecommendations)
        .await
        .expect("send GetRecommendations");
    let req = server.recv().await.expect("recv GetRecommendations");
    assert_eq!(req, Request::GetRecommendations);
    let recs = vec![Recommendation {
        id: "rec-1".into(),
        title: "Test".into(),
        message: "Test message".into(),
        suggestion_type: "test".into(),
        suggested_action: None,
    }];
    let rec_resp = Response::ok_with_data("Recommendations", &recs).expect("rec resp");
    server.send(&rec_resp).await.expect("send rec resp");
    let resp = client.recv().await.expect("recv rec resp");
    assert_eq!(resp, rec_resp);

    // ExportStats
    let export_req = Request::ExportStats {
        path: Some("/tmp/stats_export.json".into()),
    };
    client.send(&export_req).await.expect("send ExportStats");
    let req = server.recv().await.expect("recv ExportStats");
    assert_eq!(req, export_req);
}
