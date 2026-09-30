CREATE ROLE fixture_owner NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
CREATE ROLE fixture_runtime LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
CREATE SCHEMA fixture AUTHORIZATION fixture_owner;
SET ROLE fixture_owner;
CREATE FUNCTION fixture.tenant() RETURNS bytea LANGUAGE sql STABLE AS $$
    SELECT CASE WHEN current_setting('dungeonflux.fixture_tenant', true) ~ '^[0-9a-f]{32}$'
        AND current_setting('dungeonflux.fixture_tenant', true) <> repeat('0', 32)
        THEN decode(current_setting('dungeonflux.fixture_tenant', true), 'hex') END
$$;
CREATE TABLE fixture.parent (
    tenant bytea NOT NULL CHECK (octet_length(tenant) = 16),
    id integer NOT NULL,
    label text NOT NULL,
    value integer NOT NULL DEFAULT 0,
    probe text UNIQUE,
    PRIMARY KEY (tenant, id)
);
CREATE TABLE fixture.child (
    tenant bytea NOT NULL CHECK (octet_length(tenant) = 16),
    id integer NOT NULL,
    parent_id integer NOT NULL,
    PRIMARY KEY (tenant, id),
    FOREIGN KEY (tenant, parent_id) REFERENCES fixture.parent (tenant, id)
);
INSERT INTO fixture.parent (tenant, id, label, probe)
    SELECT decode(t, 'hex'), n, 'record-' || lpad(n::text, 2, '0'), t || '-' || n
    FROM (VALUES (repeat('1', 32)), (repeat('2', 32))) AS tenants(t)
    CROSS JOIN generate_series(1, 32) AS rows(n);
INSERT INTO fixture.child SELECT tenant, id, id FROM fixture.parent;
ALTER TABLE fixture.parent ENABLE ROW LEVEL SECURITY;
ALTER TABLE fixture.parent FORCE ROW LEVEL SECURITY;
ALTER TABLE fixture.child ENABLE ROW LEVEL SECURITY;
ALTER TABLE fixture.child FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON fixture.parent
    USING (tenant = fixture.tenant()) WITH CHECK (tenant = fixture.tenant());
CREATE POLICY tenant_scope ON fixture.child
    USING (tenant = fixture.tenant()) WITH CHECK (tenant = fixture.tenant());
RESET ROLE;
GRANT USAGE ON SCHEMA fixture TO fixture_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON fixture.parent, fixture.child TO fixture_runtime;
-- An empty permanent runtime-owned target removes ownership/temporary-table
-- rejection as an earlier cause for the REFERENCES privilege probe.
CREATE TABLE fixture.reference_probe (tenant bytea, id integer);
ALTER TABLE fixture.reference_probe OWNER TO fixture_runtime;
SELECT json_build_object('case', 'paired_seed', 'pass',
    (SELECT count(*) = 64 FROM fixture.parent)
    AND (SELECT count(*) = 64 FROM fixture.child),
    'parents', (SELECT count(*) FROM fixture.parent),
    'children', (SELECT count(*) FROM fixture.child));
SELECT json_build_object('case', 'enforced_schema', 'pass',
    count(*) = 2 AND bool_and(relrowsecurity AND relforcerowsecurity)
    AND bool_and(pg_get_userbyid(relowner) = 'fixture_owner'),
    'tables', json_agg(json_build_object('name', relname, 'rls', relrowsecurity,
        'force', relforcerowsecurity, 'owner', pg_get_userbyid(relowner))))
    FROM pg_class WHERE oid IN ('fixture.parent'::regclass, 'fixture.child'::regclass);
SELECT json_build_object('case', 'runtime_attributes', 'pass',
    rolcanlogin AND NOT (rolsuper OR rolbypassrls OR rolcreatedb OR rolcreaterole OR rolreplication)
    AND NOT EXISTS (SELECT 1 FROM pg_auth_members WHERE member = pg_roles.oid),
    'role', rolname, 'superuser', rolsuper, 'bypassrls', rolbypassrls)
    FROM pg_roles WHERE rolname = 'fixture_runtime';
