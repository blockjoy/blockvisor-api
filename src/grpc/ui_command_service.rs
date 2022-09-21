use super::helpers::{internal, required};
use crate::grpc::blockjoy_ui::command_service_server::CommandService;
use crate::grpc::blockjoy_ui::{CommandRequest, CommandResponse, Parameter, ResponseMeta};
use crate::grpc::notification::{ChannelNotification, ChannelNotifier, NotificationPayload};
use crate::models::{Command, CommandRequest as DbCommandRequest, HostCmd};
use crate::server::DbPool;
use crossbeam_channel::SendError;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub struct CommandServiceImpl {
    db: DbPool,
    notifier: ChannelNotifier,
}

impl CommandServiceImpl {
    pub fn new(db: DbPool, notifier: ChannelNotifier) -> Self {
        Self { db, notifier }
    }

    async fn create_command(
        &self,
        host_id: Uuid,
        cmd: HostCmd,
        sub_cmd: Option<String>,
        params: Vec<Parameter>,
    ) -> Result<Command, Status> {
        let resource_id = Self::get_resource_id_from_params(params)?;
        let req = DbCommandRequest {
            cmd,
            sub_cmd,
            resource_id,
        };
        Ok(Command::create(host_id, req, &self.db).await?)
    }

    fn send_notification(
        &self,
        notification: ChannelNotification,
    ) -> Result<(), SendError<ChannelNotification>> {
        tracing::debug!("Sending notification: {:?}", notification);
        self.notifier.commands_sender().send(notification)
    }

    fn get_resource_id_from_params(params: Vec<Parameter>) -> Result<Uuid, Status> {
        let bad_uuid = |_| Status::invalid_argument("Malformatted uuid");
        params
            .into_iter()
            .find(|p| p.name == "resource_id")
            .ok_or_else(|| Status::internal("Resource ID not available"))
            .and_then(|val| val.value.ok_or_else(required("val.value")))
            .and_then(|val| Uuid::from_slice(val.value.as_slice()).map_err(bad_uuid))
    }
}

macro_rules! create_command {
    ($obj:expr, $req:expr, $cmd:expr, $sub_cmd:expr) => {{
        let inner = $req.into_inner();

        let host_id = inner
            .id
            .ok_or_else(|| Status::not_found("No host ID provided"))?;
        let cmd = $obj
            .create_command(Uuid::from(host_id), $cmd, $sub_cmd, inner.params)
            .await?;

        let notification = ChannelNotification::Command(NotificationPayload::new(cmd.id));

        $obj.send_notification(notification).map_err(internal)?;
        let response = CommandResponse {
            meta: Some(ResponseMeta::from_meta(inner.meta).with_message(cmd.id)),
        };

        Ok(Response::new(response))
    }};
}

#[tonic::async_trait]
impl CommandService for CommandServiceImpl {
    async fn create_node(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::CreateNode, None }
    }

    async fn delete_node(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::DeleteNode, None }
    }

    async fn start_node(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::RestartNode, None }
    }

    async fn stop_node(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::ShutdownNode, None }
    }

    async fn restart_node(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::RestartNode, None }
    }

    async fn create_host(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::CreateBVS, None }
    }

    async fn delete_host(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::RemoveBVS, None }
    }

    async fn start_host(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::RestartBVS, None }
    }

    async fn stop_host(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::StopBVS, None }
    }

    async fn restart_host(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        create_command! { self, request, HostCmd::RestartBVS, None }
    }

    async fn execute_generic(
        &self,
        _request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        Err(Status::unimplemented(""))
    }
}
