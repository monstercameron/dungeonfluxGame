-- Unexecuted fixture-only authority. Apply only inside ROOT-registered owned PostgreSQL.
-- This exercises actual current grants/role-bound proofs/FORCE-RLS, not a production issuer.
BEGIN;
CREATE ROLE df_persistence_fixture_issuer NOLOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
CREATE ROLE df_persistence_fixture_runtime_a LOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
CREATE ROLE df_persistence_fixture_runtime_b LOGIN NOSUPERUSER NOBYPASSRLS NOINHERIT;
CREATE SCHEMA df_fixture_authority AUTHORIZATION df_persistence_fixture_issuer;
REVOKE ALL ON SCHEMA df_fixture_authority FROM PUBLIC;

CREATE TABLE df_fixture_authority.grants (
    service_role name NOT NULL,
    tenant_id bytea NOT NULL CHECK (octet_length(tenant_id) = 16),
    principal_id bytea NOT NULL CHECK (octet_length(principal_id) = 16),
    campaign_id bytea NOT NULL CHECK (octet_length(campaign_id) = 16),
    effective_role bytea NOT NULL CHECK (octet_length(effective_role) BETWEEN 1 AND 128),
    access_revision bytea NOT NULL CHECK (octet_length(access_revision) BETWEEN 1 AND 128),
    active boolean NOT NULL,
    expires_at timestamptz NOT NULL,
    PRIMARY KEY (service_role, tenant_id, principal_id, campaign_id)
);
CREATE TABLE df_fixture_authority.scope_proofs (
    -- Two core cryptographic UUIDs provide an opaque 32-byte fixture registry token.
    binding bytea PRIMARY KEY DEFAULT (
        decode(replace(gen_random_uuid()::text, '-', ''), 'hex') ||
        decode(replace(gen_random_uuid()::text, '-', ''), 'hex'))
        CHECK (octet_length(binding) = 32),
    service_role name NOT NULL,
    tenant_id bytea NOT NULL CHECK (octet_length(tenant_id) = 16),
    principal_id bytea NOT NULL CHECK (octet_length(principal_id) = 16),
    campaign_id bytea NOT NULL CHECK (octet_length(campaign_id) = 16),
    effective_role bytea NOT NULL CHECK (octet_length(effective_role) BETWEEN 1 AND 128),
    access_revision bytea NOT NULL CHECK (octet_length(access_revision) BETWEEN 1 AND 128),
    session_id bytea NOT NULL CHECK (octet_length(session_id) = 16),
    operation_id bytea NOT NULL CHECK (octet_length(operation_id) = 16),
    command_namespace bytea NOT NULL CHECK (octet_length(command_namespace) BETWEEN 1 AND 128),
    recovery_epoch numeric(20, 0) NOT NULL CHECK (recovery_epoch BETWEEN 1 AND 18446744073709551615),
    fingerprint_version integer NOT NULL CHECK (fingerprint_version = 1),
    canonical_fingerprint bytea NOT NULL CHECK (octet_length(canonical_fingerprint) = 32),
    owner_fence bytea NOT NULL CHECK (octet_length(owner_fence) = 16),
    execution_mode smallint NOT NULL CHECK (execution_mode BETWEEN 1 AND 3),
    lookup_only boolean NOT NULL,
    expires_at timestamptz NOT NULL,
    FOREIGN KEY (service_role, tenant_id, principal_id, campaign_id)
        REFERENCES df_fixture_authority.grants
);
CREATE TABLE df_fixture_authority.transaction_bindings (
    backend_pid integer NOT NULL,
    transaction_id xid8 NOT NULL,
    binding bytea NOT NULL REFERENCES df_fixture_authority.scope_proofs,
    PRIMARY KEY (backend_pid, transaction_id)
);
ALTER TABLE df_fixture_authority.grants OWNER TO df_persistence_fixture_issuer;
ALTER TABLE df_fixture_authority.scope_proofs OWNER TO df_persistence_fixture_issuer;
ALTER TABLE df_fixture_authority.transaction_bindings OWNER TO df_persistence_fixture_issuer;
REVOKE ALL ON ALL TABLES IN SCHEMA df_fixture_authority FROM PUBLIC;

-- Actual df-auth membership snapshot reads this exact current source under the authenticated
-- fixture connection role. Native transaction verification below rereads and locks its revision.
CREATE FUNCTION df_fixture_authority.read_membership(
    p_tenant bytea, p_principal bytea, p_campaign bytea, p_role bytea)
RETURNS TABLE (tenant_id bytea, principal_id bytea, campaign_id bytea,
    effective_role bytea, access_revision bytea, active boolean, expires_unix bigint)
LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT g.tenant_id, g.principal_id, g.campaign_id, g.effective_role, g.access_revision,
           g.active, floor(extract(epoch FROM g.expires_at))::bigint
    FROM df_fixture_authority.grants AS g
    WHERE g.service_role = session_user AND g.tenant_id = p_tenant
      AND g.principal_id = p_principal AND g.campaign_id = p_campaign
      AND g.effective_role = p_role;
$$;

-- Prepared verifier ABI matches NativeScope/actual MembershipCapability projection exactly.
-- It authenticates access independently of the current session run/fence/epoch, allowing old
-- retained receipt lookup. New-write owner/run/epoch/lease predicates remain the native CAS.
CREATE FUNCTION df_fixture_authority.bind_scope(p_binding bytea, p_session bytea)
RETURNS TABLE (tenant_id bytea, principal_id bytea, campaign_id bytea, effective_role bytea,
    access_revision bytea, permission_current boolean, operation_id bytea,
    command_namespace bytea, recovery_epoch text, fingerprint_version integer,
    canonical_fingerprint bytea, owner_fence bytea, execution_mode smallint, lookup_only boolean)
LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    proof df_fixture_authority.scope_proofs%ROWTYPE;
    current_grant df_fixture_authority.grants%ROWTYPE;
BEGIN
    IF octet_length(p_binding) <> 32 OR octet_length(p_session) <> 16 THEN RETURN; END IF;
    SELECT p.* INTO proof FROM df_fixture_authority.scope_proofs AS p
    WHERE p.binding = p_binding AND p.session_id = p_session AND p.service_role = session_user
      AND p.expires_at > clock_timestamp() FOR UPDATE;
    IF NOT FOUND THEN RETURN; END IF;
    SELECT g.* INTO current_grant FROM df_fixture_authority.grants AS g
    WHERE g.service_role = session_user AND g.tenant_id = proof.tenant_id
      AND g.principal_id = proof.principal_id AND g.campaign_id = proof.campaign_id FOR UPDATE;
    IF NOT FOUND THEN RETURN; END IF;
    IF NOT current_grant.active OR current_grant.expires_at <= clock_timestamp()
        OR current_grant.access_revision <> proof.access_revision
        OR current_grant.effective_role <> proof.effective_role THEN RETURN; END IF;
    -- A role cannot replace its verified scope midway through a transaction.
    INSERT INTO df_fixture_authority.transaction_bindings (backend_pid, transaction_id, binding)
    VALUES (pg_backend_pid(), pg_current_xact_id(), p_binding)
    ON CONFLICT (backend_pid, transaction_id) DO NOTHING;
    IF NOT EXISTS (SELECT 1 FROM df_fixture_authority.transaction_bindings AS b
        WHERE b.backend_pid = pg_backend_pid() AND b.transaction_id = pg_current_xact_id()
          AND b.binding = p_binding) THEN RETURN; END IF;
    RETURN QUERY SELECT proof.tenant_id, proof.principal_id, proof.campaign_id, proof.effective_role,
        proof.access_revision,
        current_grant.active AND current_grant.expires_at > clock_timestamp()
            AND proof.expires_at > clock_timestamp(),
        proof.operation_id, proof.command_namespace, proof.recovery_epoch::text,
        proof.fingerprint_version, proof.canonical_fingerprint, proof.owner_fence,
        proof.execution_mode, proof.lookup_only;
END;
$$;

-- SQL-visible scope derives only from the private transaction registry and current authority.
-- SET/RESET of unsigned tenant/principal GUCs never changes these results.
CREATE FUNCTION df_fixture_authority.current_scope()
RETURNS TABLE (tenant_id bytea, session_id bytea)
LANGUAGE sql VOLATILE SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT p.tenant_id, p.session_id
    FROM df_fixture_authority.transaction_bindings AS b
    JOIN df_fixture_authority.scope_proofs AS p ON p.binding = b.binding
    JOIN df_fixture_authority.grants AS g ON g.service_role = p.service_role
      AND g.tenant_id = p.tenant_id AND g.principal_id = p.principal_id AND g.campaign_id = p.campaign_id
    WHERE b.backend_pid = pg_backend_pid() AND b.transaction_id = pg_current_xact_id()
      AND p.service_role = session_user AND g.active
      AND g.access_revision = p.access_revision AND g.effective_role = p.effective_role
      AND p.expires_at > clock_timestamp() AND g.expires_at > clock_timestamp();
$$;
ALTER FUNCTION df_fixture_authority.read_membership(bytea, bytea, bytea, bytea) OWNER TO df_persistence_fixture_issuer;
ALTER FUNCTION df_fixture_authority.bind_scope(bytea, bytea) OWNER TO df_persistence_fixture_issuer;
ALTER FUNCTION df_fixture_authority.current_scope() OWNER TO df_persistence_fixture_issuer;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA df_fixture_authority FROM PUBLIC;
GRANT USAGE ON SCHEMA df_fixture_authority, df_game TO df_persistence_fixture_runtime_a, df_persistence_fixture_runtime_b;
GRANT EXECUTE ON FUNCTION df_fixture_authority.read_membership(bytea, bytea, bytea, bytea),
    df_fixture_authority.bind_scope(bytea, bytea), df_fixture_authority.current_scope()
    TO df_persistence_fixture_runtime_a, df_persistence_fixture_runtime_b;
-- The runtime can append committed families but cannot rewrite their fingerprints/results.
GRANT SELECT ON df_game.sessions, df_game.retired_namespaces
    TO df_persistence_fixture_runtime_a, df_persistence_fixture_runtime_b;
GRANT UPDATE (in_epoch_sequence) ON df_game.sessions
    TO df_persistence_fixture_runtime_a, df_persistence_fixture_runtime_b;
GRANT SELECT, INSERT ON df_game.checkpoints, df_game.operations, df_game.facts, df_game.intents
    TO df_persistence_fixture_runtime_a, df_persistence_fixture_runtime_b;

CREATE POLICY fixture_verified_scope ON df_game.sessions
USING (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = sessions.tenant_id::bytea AND scope.session_id = sessions.session_id::bytea))
WITH CHECK (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = sessions.tenant_id::bytea AND scope.session_id = sessions.session_id::bytea));
CREATE POLICY fixture_verified_scope ON df_game.checkpoints
USING (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = checkpoints.tenant_id::bytea AND scope.session_id = checkpoints.session_id::bytea))
WITH CHECK (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = checkpoints.tenant_id::bytea AND scope.session_id = checkpoints.session_id::bytea));
CREATE POLICY fixture_verified_scope ON df_game.operations
USING (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = operations.tenant_id::bytea AND scope.session_id = operations.session_id::bytea))
WITH CHECK (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = operations.tenant_id::bytea AND scope.session_id = operations.session_id::bytea));
CREATE POLICY fixture_verified_scope ON df_game.facts
USING (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = facts.tenant_id::bytea AND scope.session_id = facts.session_id::bytea))
WITH CHECK (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = facts.tenant_id::bytea AND scope.session_id = facts.session_id::bytea));
CREATE POLICY fixture_verified_scope ON df_game.intents
USING (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = intents.tenant_id::bytea AND scope.session_id = intents.session_id::bytea))
WITH CHECK (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = intents.tenant_id::bytea AND scope.session_id = intents.session_id::bytea));
CREATE POLICY fixture_verified_scope ON df_game.retired_namespaces
USING (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = retired_namespaces.tenant_id::bytea AND scope.session_id = retired_namespaces.session_id::bytea))
WITH CHECK (EXISTS (SELECT 1 FROM df_fixture_authority.current_scope() AS scope
    WHERE scope.tenant_id = retired_namespaces.tenant_id::bytea AND scope.session_id = retired_namespaces.session_id::bytea));
COMMIT;
