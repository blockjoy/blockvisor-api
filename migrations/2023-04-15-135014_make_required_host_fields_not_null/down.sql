ALTER TABLE hosts ALTER COLUMN version DROP NOT NULL;
ALTER TABLE hosts ALTER COLUMN cpu_count DROP NOT NULL;
ALTER TABLE hosts ALTER COLUMN mem_size DROP NOT NULL;
ALTER TABLE hosts ALTER COLUMN disk_size DROP NOT NULL;
ALTER TABLE hosts ALTER COLUMN os DROP NOT NULL;
ALTER TABLE hosts ALTER COLUMN os_version DROP NOT NULL;
