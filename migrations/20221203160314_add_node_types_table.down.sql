DROP TABLE IF EXISTS node_type_properties;
DROP TABLE IF EXISTS node_type_requirements;
DROP TABLE IF EXISTS node_types;
DROP TABLE IF EXISTS node_type_settings;
DROP TYPE IF EXISTS enum_node_property_field_type;

ALTER TABLE blockchains
    ADD column supported_node_types jsonb default '[]'::jsonb;

ALTER TABLE nodes
    ADD column node_data jsonb default '{}'::jsonb;
