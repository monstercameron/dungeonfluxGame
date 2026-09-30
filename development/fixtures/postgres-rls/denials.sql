SET dungeonflux.fixture_tenant = '11111111111111111111111111111111';
\set ON_ERROR_STOP off
INSERT INTO fixture.parent (tenant,id,label) VALUES (decode(repeat('2',32),'hex'),33,'foreign');
\echo EXPECTED foreign_insert :SQLSTATE
UPDATE fixture.parent SET tenant = decode(repeat('2',32),'hex') WHERE tenant = fixture.tenant() AND id = 1;
\echo EXPECTED tenant_move :SQLSTATE
INSERT INTO fixture.parent (tenant,id,label) VALUES (fixture.tenant(),1,'duplicate');
\echo EXPECTED unique_own :SQLSTATE
INSERT INTO fixture.child VALUES (fixture.tenant(),33,33);
\echo EXPECTED composite_fk_missing :SQLSTATE
INSERT INTO fixture.child VALUES (decode(repeat('2',32),'hex'),33,1);
\echo EXPECTED foreign_child :SQLSTATE
-- Deliberately unsafe global uniqueness models the constraint existence channel.
INSERT INTO fixture.parent (tenant,id,label,probe)
    VALUES (fixture.tenant(),33,'hidden-probe',repeat('2',32)||'-1');
\echo EXPECTED hidden_unique_channel :SQLSTATE
ALTER TABLE fixture.parent DISABLE ROW LEVEL SECURITY;
\echo EXPECTED disable_rls :SQLSTATE
ALTER TABLE fixture.parent NO FORCE ROW LEVEL SECURITY;
\echo EXPECTED unforce_rls :SQLSTATE
DROP POLICY tenant_scope ON fixture.parent;
\echo EXPECTED drop_policy :SQLSTATE
SET ROLE fixture_owner;
\echo EXPECTED assume_owner :SQLSTATE
SET ROLE fixture_admin;
\echo EXPECTED assume_admin :SQLSTATE
ALTER ROLE fixture_runtime BYPASSRLS;
\echo EXPECTED grant_bypass :SQLSTATE
CREATE ROLE fixture_attacker SUPERUSER;
\echo EXPECTED create_admin :SQLSTATE
TRUNCATE fixture.parent, fixture.child;
\echo EXPECTED truncate :SQLSTATE
ALTER TABLE fixture.reference_probe ADD CONSTRAINT forbidden_reference
    FOREIGN KEY (tenant,id) REFERENCES fixture.parent(tenant,id);
\echo EXPECTED references :SQLSTATE
SET row_security = off;
SELECT count(*) FROM fixture.parent;
\echo EXPECTED row_security_off :SQLSTATE
SET row_security = on;
\set ON_ERROR_STOP on
SELECT json_build_object('case', 'whole_table_privileges', 'pass',
    NOT has_table_privilege(current_user,'fixture.parent','REFERENCES')
    AND NOT has_table_privilege(current_user,'fixture.parent','TRUNCATE')
    AND NOT has_table_privilege(current_user,'fixture.child','REFERENCES')
    AND NOT has_table_privilege(current_user,'fixture.child','TRUNCATE'),
    'references',has_table_privilege(current_user,'fixture.parent','REFERENCES'),
    'truncate',has_table_privilege(current_user,'fixture.parent','TRUNCATE'));
SELECT json_build_object('case', 'reference_probe_preconditions', 'pass',
    pg_get_userbyid(relowner) = current_user AND relpersistence = 'p'
    AND has_schema_privilege(current_user, 'fixture', 'USAGE')
    AND NOT has_table_privilege(current_user,'fixture.parent','REFERENCES')
    AND (SELECT relpersistence = 'p' FROM pg_class WHERE oid = 'fixture.parent'::regclass)
    AND NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'fixture.reference_probe'::regclass),
    'target_owner',pg_get_userbyid(relowner),'target_persistence',relpersistence,
    'schema_usage',has_schema_privilege(current_user,'fixture','USAGE'))
    FROM pg_class WHERE oid = 'fixture.reference_probe'::regclass;
BEGIN;
INSERT INTO fixture.parent (tenant,id,label,probe) VALUES (fixture.tenant(),33,'absent-probe','absent');
SELECT json_build_object('case', 'absent_unique_probe', 'pass', count(*) = 1)
    FROM fixture.parent WHERE tenant = fixture.tenant() AND id = 33;
ROLLBACK;
SET dungeonflux.fixture_tenant = '';
SELECT json_build_object('case', 'post_denials', 'pass', session_user = current_user AND count(*) = 0)
    FROM fixture.parent;
