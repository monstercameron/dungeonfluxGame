SELECT json_build_object('case', 'direct_login', 'pass',
    session_user = 'fixture_runtime' AND current_user = session_user,
    'session_user', session_user, 'current_user', current_user, 'backend', pg_backend_pid());
SELECT json_build_object('case', 'missing_scope', 'pass', count(*) = 0, 'rows', count(*))
    FROM fixture.parent;
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = 'invalid';
SELECT json_build_object('case', 'invalid_scope', 'pass', count(*) = 0, 'rows', count(*))
    FROM fixture.parent;
ROLLBACK;
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '00000000000000000000000000000000';
SELECT json_build_object('case', 'zero_scope', 'pass', count(*) = 0, 'rows', count(*))
    FROM fixture.parent;
ROLLBACK;
-- Trusted fixture setter clears inherited SESSION state BEFORE BEGIN, then uses LOCAL.
SET dungeonflux.fixture_tenant = '';
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
SELECT json_build_object('case', 'tenant_a', 'pass', count(*) = 32 AND
    bool_and(tenant = decode(repeat('1', 32), 'hex')), 'rows', count(*)) FROM fixture.parent;
SELECT json_build_object('case', 'explicit_predicate', 'pass', count(*) = 32, 'rows', count(*))
    FROM fixture.parent WHERE tenant = fixture.tenant();
SELECT json_build_object('case', 'join', 'pass', count(*) = 32 AND
    bool_and(p.tenant = fixture.tenant() AND c.tenant = fixture.tenant()), 'rows', count(*))
    FROM fixture.parent p JOIN fixture.child c ON (p.tenant, p.id) = (c.tenant, c.parent_id)
    WHERE p.tenant = fixture.tenant() AND c.tenant = fixture.tenant();
SELECT json_build_object('case', 'search', 'pass', count(*) = 10, 'rows', count(*))
    FROM fixture.parent WHERE tenant = fixture.tenant() AND label LIKE 'record-1%';
SELECT json_build_object('case', 'keyset_page', 'pass', array_agg(id ORDER BY id) = ARRAY[9,10,11,12,13,14,15,16],
    'ids', array_agg(id ORDER BY id)) FROM
    (SELECT id FROM fixture.parent WHERE tenant = fixture.tenant() AND id > 8 ORDER BY id LIMIT 8) page;
SELECT json_build_object('case', 'foreign_read', 'pass', count(*) = 0, 'rows', count(*))
    FROM fixture.parent WHERE tenant = decode(repeat('2',32), 'hex') AND id = 1;
WITH changed AS (UPDATE fixture.parent SET value = 9
    WHERE tenant = decode(repeat('2',32), 'hex') AND id = 1 RETURNING id)
    SELECT json_build_object('case', 'foreign_update', 'pass', count(*) = 0, 'rows', count(*)) FROM changed;
WITH changed AS (DELETE FROM fixture.child
    WHERE tenant = decode(repeat('2',32), 'hex') AND id = 1 RETURNING id)
    SELECT json_build_object('case', 'foreign_delete', 'pass', count(*) = 0, 'rows', count(*)) FROM changed;
WITH changed AS (UPDATE fixture.parent SET value = 1 WHERE tenant = fixture.tenant() AND id = 1 RETURNING id)
    SELECT json_build_object('case', 'own_update', 'pass', count(*) = 1, 'rows', count(*)) FROM changed;
INSERT INTO fixture.parent VALUES (fixture.tenant(), 33, 'temporary', 0);
INSERT INTO fixture.child VALUES (fixture.tenant(), 33, 33);
WITH changed AS (DELETE FROM fixture.child WHERE tenant = fixture.tenant() AND id = 33 RETURNING id)
    SELECT json_build_object('case', 'own_child_delete', 'pass', count(*) = 1) FROM changed;
WITH changed AS (DELETE FROM fixture.parent WHERE tenant = fixture.tenant() AND id = 33 RETURNING id)
    SELECT json_build_object('case', 'own_parent_delete', 'pass', count(*) = 1) FROM changed;
COMMIT;
SELECT json_build_object('case', 'commit_reset', 'pass', count(*) = 0 AND
    current_setting('dungeonflux.fixture_tenant', true) = '', 'rows', count(*), 'backend', pg_backend_pid())
    FROM fixture.parent;
SET dungeonflux.fixture_tenant = '';
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '22222222222222222222222222222222';
SELECT json_build_object('case', 'tenant_b', 'pass', count(*) = 32 AND bool_and(value = 0),
    'rows', count(*), 'backend', pg_backend_pid()) FROM fixture.parent WHERE tenant = fixture.tenant();
ROLLBACK;
SELECT json_build_object('case', 'rollback_reset', 'pass', count(*) = 0, 'rows', count(*),
    'backend', pg_backend_pid()) FROM fixture.parent;
SET dungeonflux.fixture_tenant = '';
BEGIN;
SAVEPOINT before_scope;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
SELECT json_build_object('case', 'savepoint_scoped', 'pass', count(*) = 32) FROM fixture.parent;
ROLLBACK TO before_scope;
SELECT json_build_object('case', 'savepoint_reset', 'pass', count(*) = 0, 'rows', count(*)) FROM fixture.parent;
SET LOCAL dungeonflux.fixture_tenant = '22222222222222222222222222222222';
COMMIT;
SET dungeonflux.fixture_tenant = '';
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
SAVEPOINT scoped_a;
SET LOCAL dungeonflux.fixture_tenant = '22222222222222222222222222222222';
ROLLBACK TO scoped_a;
SELECT json_build_object('case', 'savepoint_restores_prior_local', 'pass', count(*) = 32 AND
    bool_and(tenant = decode(repeat('1',32),'hex')), 'rows', count(*)) FROM fixture.parent;
ROLLBACK;
-- Demonstrate why LOCAL alone cannot sanitize inherited SESSION context.
SET dungeonflux.fixture_tenant = '22222222222222222222222222222222';
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
COMMIT;
SELECT json_build_object('case', 'inherited_contamination', 'pass', count(*) = 32 AND
    bool_and(tenant = decode(repeat('2',32),'hex')), 'rows', count(*),
    'setting', current_setting('dungeonflux.fixture_tenant')) FROM fixture.parent;
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
ROLLBACK;
SELECT json_build_object('case', 'inherited_rollback_contamination', 'pass', count(*) = 32 AND
    bool_and(tenant = decode(repeat('2',32),'hex')), 'rows', count(*)) FROM fixture.parent;
SET dungeonflux.fixture_tenant = '';
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
COMMIT;
SELECT json_build_object('case', 'sanitized_reuse', 'pass', count(*) = 0, 'rows', count(*),
    'backend', pg_backend_pid()) FROM fixture.parent;
-- Expected errors stay SQLSTATE evidence; no public error adapter is invented.
\set ON_ERROR_STOP off
SET dungeonflux.fixture_tenant = '';
BEGIN;
SET LOCAL dungeonflux.fixture_tenant = '11111111111111111111111111111111';
SELECT 1 / 0;
\echo EXPECTED failed_statement :SQLSTATE
SELECT count(*) FROM fixture.parent;
\echo EXPECTED aborted_transaction :SQLSTATE
ROLLBACK;
\set ON_ERROR_STOP on
SELECT json_build_object('case', 'error_reset', 'pass', count(*) = 0, 'rows', count(*),
    'backend', pg_backend_pid()) FROM fixture.parent;
