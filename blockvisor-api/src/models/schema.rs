// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "blockchain_property_ui_type"))]
    pub struct BlockchainPropertyUiType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_blockchain_visibility"))]
    pub struct EnumBlockchainVisibility;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_command_exit_code"))]
    pub struct EnumCommandExitCode;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_conn_status"))]
    pub struct EnumConnStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_container_status"))]
    pub struct EnumContainerStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_host_cmd"))]
    pub struct EnumHostCmd;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_host_type"))]
    pub struct EnumHostType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_managed_by"))]
    pub struct EnumManagedBy;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_log_event"))]
    pub struct EnumNodeLogEvent;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_resource_affinity"))]
    pub struct EnumNodeResourceAffinity;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_similarity_affinity"))]
    pub struct EnumNodeSimilarityAffinity;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_staking_status"))]
    pub struct EnumNodeStakingStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_status"))]
    pub struct EnumNodeStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_sync_status"))]
    pub struct EnumNodeSyncStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_node_type"))]
    pub struct EnumNodeType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "enum_resource_type"))]
    pub struct EnumResourceType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "token_type"))]
    pub struct TokenType;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumResourceType;

    api_keys (id) {
        id -> Uuid,
        user_id -> Uuid,
        label -> Text,
        key_hash -> Text,
        key_salt -> Text,
        resource -> EnumResourceType,
        resource_id -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumNodeType;
    use super::sql_types::EnumBlockchainVisibility;

    blockchain_node_types (id) {
        id -> Uuid,
        blockchain_id -> Uuid,
        description -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        node_type -> EnumNodeType,
        visibility -> EnumBlockchainVisibility,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::BlockchainPropertyUiType;

    blockchain_properties (id) {
        id -> Uuid,
        blockchain_id -> Uuid,
        name -> Text,
        default -> Nullable<Text>,
        ui_type -> BlockchainPropertyUiType,
        disabled -> Bool,
        required -> Bool,
        blockchain_node_type_id -> Uuid,
        blockchain_version_id -> Uuid,
        display_name -> Text,
    }
}

diesel::table! {
    blockchain_versions (id) {
        id -> Uuid,
        blockchain_id -> Uuid,
        blockchain_node_type_id -> Uuid,
        version -> Text,
        description -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumBlockchainVisibility;

    blockchains (id) {
        id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        project_url -> Nullable<Text>,
        repo_url -> Nullable<Text>,
        version -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        visibility -> EnumBlockchainVisibility,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumHostCmd;
    use super::sql_types::EnumCommandExitCode;

    commands (id) {
        id -> Uuid,
        host_id -> Uuid,
        cmd -> EnumHostCmd,
        exit_message -> Nullable<Text>,
        created_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
        node_id -> Nullable<Uuid>,
        acked_at -> Nullable<Timestamptz>,
        retry_hint_seconds -> Nullable<Int8>,
        exit_code -> Nullable<EnumCommandExitCode>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumConnStatus;
    use super::sql_types::EnumHostType;
    use super::sql_types::EnumManagedBy;

    hosts (id) {
        id -> Uuid,
        version -> Text,
        name -> Text,
        ip_addr -> Text,
        status -> EnumConnStatus,
        created_at -> Timestamptz,
        cpu_count -> Int8,
        mem_size_bytes -> Int8,
        disk_size_bytes -> Int8,
        os -> Text,
        os_version -> Text,
        ip_range_from -> Inet,
        ip_range_to -> Inet,
        ip_gateway -> Inet,
        used_cpu -> Nullable<Int4>,
        used_memory -> Nullable<Int8>,
        used_disk_space -> Nullable<Int8>,
        load_one -> Nullable<Float8>,
        load_five -> Nullable<Float8>,
        load_fifteen -> Nullable<Float8>,
        network_received -> Nullable<Int8>,
        network_sent -> Nullable<Int8>,
        uptime -> Nullable<Int8>,
        host_type -> Nullable<EnumHostType>,
        org_id -> Uuid,
        created_by -> Nullable<Uuid>,
        region_id -> Nullable<Uuid>,
        monthly_cost_in_usd -> Nullable<Int8>,
        vmm_mountpoint -> Nullable<Text>,
        deleted_at -> Nullable<Timestamptz>,
        managed_by -> EnumManagedBy,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumResourceType;

    invitations (id) {
        id -> Uuid,
        invited_by -> Uuid,
        org_id -> Uuid,
        invitee_email -> Text,
        created_at -> Timestamptz,
        accepted_at -> Nullable<Timestamptz>,
        declined_at -> Nullable<Timestamptz>,
        invited_by_resource -> EnumResourceType,
    }
}

diesel::table! {
    ip_addresses (id) {
        id -> Uuid,
        ip -> Inet,
        host_id -> Nullable<Uuid>,
        is_assigned -> Bool,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumNodeLogEvent;
    use super::sql_types::EnumNodeType;

    node_logs (id) {
        id -> Uuid,
        host_id -> Uuid,
        node_id -> Uuid,
        event -> EnumNodeLogEvent,
        blockchain_name -> Text,
        #[max_length = 32]
        version -> Varchar,
        created_at -> Timestamptz,
        node_type -> EnumNodeType,
    }
}

diesel::table! {
    node_properties (id) {
        id -> Uuid,
        node_id -> Uuid,
        blockchain_property_id -> Uuid,
        value -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumResourceType;

    node_reports (id) {
        id -> Uuid,
        node_id -> Uuid,
        created_by_resource -> EnumResourceType,
        created_by -> Uuid,
        message -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::EnumNodeSimilarityAffinity;
    use super::sql_types::EnumNodeResourceAffinity;
    use super::sql_types::EnumResourceType;
    use super::sql_types::EnumNodeType;
    use super::sql_types::EnumNodeStatus;
    use super::sql_types::EnumContainerStatus;
    use super::sql_types::EnumNodeSyncStatus;
    use super::sql_types::EnumNodeStakingStatus;

    nodes (id) {
        id -> Uuid,
        org_id -> Uuid,
        host_id -> Uuid,
        name -> Text,
        version -> Text,
        ip_addr -> Text,
        address -> Nullable<Text>,
        wallet_address -> Nullable<Text>,
        block_height -> Nullable<Int8>,
        node_data -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        blockchain_id -> Uuid,
        ip_gateway -> Text,
        self_update -> Bool,
        block_age -> Nullable<Int8>,
        consensus -> Nullable<Bool>,
        vcpu_count -> Int8,
        mem_size_bytes -> Int8,
        disk_size_bytes -> Int8,
        network -> Text,
        created_by -> Nullable<Uuid>,
        #[max_length = 50]
        dns_record_id -> Varchar,
        allow_ips -> Jsonb,
        deny_ips -> Jsonb,
        scheduler_similarity -> Nullable<EnumNodeSimilarityAffinity>,
        scheduler_resource -> Nullable<EnumNodeResourceAffinity>,
        scheduler_region -> Nullable<Uuid>,
        data_directory_mountpoint -> Nullable<Text>,
        jobs -> Jsonb,
        created_by_resource -> Nullable<EnumResourceType>,
        deleted_at -> Nullable<Timestamptz>,
        node_type -> EnumNodeType,
        node_status -> EnumNodeStatus,
        container_status -> EnumContainerStatus,
        sync_status -> EnumNodeSyncStatus,
        staking_status -> Nullable<EnumNodeStakingStatus>,
    }
}

diesel::table! {
    orgs (id) {
        id -> Uuid,
        name -> Text,
        is_personal -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    orgs_users (org_id, user_id) {
        org_id -> Uuid,
        user_id -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        #[max_length = 32]
        host_provision_token -> Varchar,
    }
}

diesel::table! {
    permissions (name) {
        name -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    regions (id) {
        id -> Uuid,
        name -> Text,
        pricing_tier -> Nullable<Text>,
    }
}

diesel::table! {
    role_permissions (role, permission) {
        role -> Text,
        permission -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    roles (name) {
        name -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    subscriptions (id) {
        id -> Uuid,
        org_id -> Uuid,
        user_id -> Uuid,
        external_id -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::TokenType;

    token_blacklist (token) {
        token -> Text,
        token_type -> TokenType,
    }
}

diesel::table! {
    user_roles (user_id, org_id, role) {
        user_id -> Uuid,
        org_id -> Uuid,
        role -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        email -> Text,
        hashword -> Text,
        salt -> Text,
        created_at -> Timestamptz,
        #[max_length = 64]
        first_name -> Varchar,
        #[max_length = 64]
        last_name -> Varchar,
        confirmed_at -> Nullable<Timestamptz>,
        deleted_at -> Nullable<Timestamptz>,
        billing_id -> Nullable<Text>,
    }
}

diesel::joinable!(api_keys -> users (user_id));
diesel::joinable!(blockchain_node_types -> blockchains (blockchain_id));
diesel::joinable!(blockchain_properties -> blockchain_node_types (blockchain_node_type_id));
diesel::joinable!(blockchain_properties -> blockchain_versions (blockchain_version_id));
diesel::joinable!(blockchain_properties -> blockchains (blockchain_id));
diesel::joinable!(blockchain_versions -> blockchain_node_types (blockchain_node_type_id));
diesel::joinable!(blockchain_versions -> blockchains (blockchain_id));
diesel::joinable!(commands -> hosts (host_id));
diesel::joinable!(commands -> nodes (node_id));
diesel::joinable!(hosts -> orgs (org_id));
diesel::joinable!(hosts -> regions (region_id));
diesel::joinable!(hosts -> users (created_by));
diesel::joinable!(invitations -> orgs (org_id));
diesel::joinable!(invitations -> users (invited_by));
diesel::joinable!(ip_addresses -> hosts (host_id));
diesel::joinable!(node_properties -> blockchain_properties (blockchain_property_id));
diesel::joinable!(node_properties -> nodes (node_id));
diesel::joinable!(node_reports -> nodes (node_id));
diesel::joinable!(nodes -> blockchains (blockchain_id));
diesel::joinable!(nodes -> hosts (host_id));
diesel::joinable!(nodes -> orgs (org_id));
diesel::joinable!(nodes -> regions (scheduler_region));
diesel::joinable!(orgs_users -> orgs (org_id));
diesel::joinable!(orgs_users -> users (user_id));
diesel::joinable!(role_permissions -> permissions (permission));
diesel::joinable!(role_permissions -> roles (role));
diesel::joinable!(subscriptions -> orgs (org_id));
diesel::joinable!(subscriptions -> users (user_id));
diesel::joinable!(user_roles -> orgs (org_id));
diesel::joinable!(user_roles -> roles (role));
diesel::joinable!(user_roles -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    api_keys,
    blockchain_node_types,
    blockchain_properties,
    blockchain_versions,
    blockchains,
    commands,
    hosts,
    invitations,
    ip_addresses,
    node_logs,
    node_properties,
    node_reports,
    nodes,
    orgs,
    orgs_users,
    permissions,
    regions,
    role_permissions,
    roles,
    subscriptions,
    token_blacklist,
    user_roles,
    users,
);
