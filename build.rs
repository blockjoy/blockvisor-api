//! Build file generating gRPC stubs

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        // needed for integration tests
        .build_client(true)
        .compile(
            &[
                // Backend API
                "command_flow.proto",
                "host_service.proto",
                // UI API
                "authentication_service.proto",
                "billing_service.proto",
                "blockchain_service.proto",
                "command_service.proto",
                "dashboard_service.proto",
                "host_provision_service.proto",
                "fe_host_service.proto",
                "node_service.proto",
                "organization_service.proto",
                "update_service.proto",
                "user_service.proto",
            ],
            &["proto/blockjoy/api/v1", "proto/blockjoy/api/ui_v1"],
        )?;

    Ok(())
}
