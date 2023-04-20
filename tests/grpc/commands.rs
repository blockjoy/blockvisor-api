use blockvisor_api::auth::FindableById;
use blockvisor_api::grpc::api;
use blockvisor_api::models;

type Service = api::commands_client::CommandsClient<super::Channel>;

async fn create_command(
    tester: &super::Tester,
    node_id: uuid::Uuid,
    cmd_type: models::CommandType,
) -> models::Command {
    let host = tester.host().await;
    let mut conn = tester.conn().await;
    let new_cmd = models::NewCommand {
        host_id: host.id,
        cmd: cmd_type,
        sub_cmd: None,
        node_id: Some(node_id),
    };

    new_cmd.create(&mut conn).await.unwrap()
}

#[tokio::test]
async fn can_create_each_variant() {
    use api::create_command_request::Command::*;

    let tester = super::Tester::new().await;
    let node = tester.node().await;
    let host = tester.host().await;
    let variants = [
        StartNode(api::StartNodeCommand {
            node_id: node.id.to_string(),
        }),
        StopNode(api::StopNodeCommand {
            node_id: node.id.to_string(),
        }),
        RestartNode(api::RestartNodeCommand {
            node_id: node.id.to_string(),
        }),
        StartHost(api::StartHostCommand {
            host_id: host.id.to_string(),
        }),
        StopHost(api::StopHostCommand {
            host_id: host.id.to_string(),
        }),
        RestartHost(api::RestartHostCommand {
            host_id: host.id.to_string(),
        }),
    ];
    for command in variants {
        let req = api::CreateCommandRequest {
            command: Some(command),
        };
        tester.send_admin(Service::create, req).await.unwrap();
    }
}

#[tokio::test]
async fn responds_ok_with_single_get() {
    let tester = super::Tester::new().await;
    let mut conn = tester.conn().await;
    let node = tester.node().await;
    let update = models::UpdateNode {
        id: node.id,
        name: None,
        version: None,
        ip_addr: Some("123.123.123.123"),
        block_height: None,
        node_data: None,
        chain_status: None,
        sync_status: None,
        staking_status: None,
        container_status: None,
        self_update: None,
        address: None,
    };
    update.update(&mut conn).await.unwrap();

    let cmd = create_command(&tester, node.id, models::CommandType::CreateNode).await;
    let req = api::GetCommandRequest {
        id: cmd.id.to_string(),
    };

    let cmd = tester.send_admin(Service::get, req).await.unwrap();
    assert!(matches!(
        cmd.command.unwrap().command.unwrap(),
        api::command::Command::Node(_),
    ));
}

#[tokio::test]
async fn responds_ok_for_update() {
    let tester = super::Tester::new().await;
    let mut conn = tester.conn().await;
    let node = tester.node().await;
    let cmd = create_command(&tester, node.id, models::CommandType::CreateNode).await;
    let host = models::Host::find_by_id(cmd.host_id, &mut conn)
        .await
        .unwrap();
    let token = tester.host_token(&host);
    let refresh = tester.refresh_for(&token);
    let req = api::UpdateCommandRequest {
        id: cmd.id.to_string(),
        response: Some("hugo boss".to_string()),
        exit_code: Some(98),
    };

    tester
        .send_with(Service::update, req, token, refresh)
        .await
        .unwrap();

    let cmd = models::Command::find_by_id(cmd.id, &mut conn)
        .await
        .unwrap();

    assert_eq!(cmd.response.unwrap(), "hugo boss");
    assert_eq!(cmd.exit_status.unwrap(), 98);
}

#[tokio::test]
async fn responds_ok_for_pending() {
    let tester = super::Tester::new().await;
    let mut conn = tester.conn().await;
    let node = tester.node().await;
    let update = models::UpdateNode {
        id: node.id,
        name: None,
        version: None,
        ip_addr: Some("123.123.123.123"),
        block_height: None,
        node_data: None,
        chain_status: None,
        sync_status: None,
        staking_status: None,
        container_status: None,
        self_update: None,
        address: None,
    };
    update.update(&mut conn).await.unwrap();
    let cmd = create_command(&tester, node.id, models::CommandType::CreateNode).await;
    let host = models::Host::find_by_id(cmd.host_id, &mut conn)
        .await
        .unwrap();
    let token = tester.host_token(&host);
    let refresh = tester.refresh_for(&token);
    let req = api::PendingCommandsRequest {
        host_id: host.id.to_string(),
        filter_type: None,
    };

    tester
        .send_with(Service::pending, req, token, refresh)
        .await
        .unwrap();
}
