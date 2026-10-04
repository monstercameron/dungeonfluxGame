-- UNAPPLIED SOURCE DRAFT: consumer contracts, limits, driver and scope binding pending.
-- No runtime policy grants access until the approved database scope resolver exists.
BEGIN;
CREATE SCHEMA df_game;
REVOKE ALL ON SCHEMA df_game FROM PUBLIC;

CREATE DOMAIN df_game.identity_bytes AS bytea
    CHECK (octet_length(VALUE) = 16 AND VALUE <> decode(repeat('00', 16), 'hex'));
CREATE DOMAIN df_game.epoch_number AS numeric(20, 0)
    CHECK (VALUE BETWEEN 1 AND 18446744073709551615);
CREATE DOMAIN df_game.sequence_number AS numeric(20, 0)
    CHECK (VALUE BETWEEN 0 AND 18446744073709551615);

CREATE TABLE df_game.sessions (
    tenant_id df_game.identity_bytes NOT NULL,
    session_id df_game.identity_bytes NOT NULL,
    run_id df_game.identity_bytes NOT NULL,
    recovery_epoch df_game.epoch_number NOT NULL,
    in_epoch_sequence df_game.sequence_number NOT NULL,
    owner_fence df_game.identity_bytes NOT NULL,
    lease_until timestamptz NOT NULL,
    PRIMARY KEY (tenant_id, session_id)
);
CREATE TABLE df_game.checkpoints (
    tenant_id df_game.identity_bytes NOT NULL,
    session_id df_game.identity_bytes NOT NULL,
    recovery_epoch df_game.epoch_number NOT NULL,
    in_epoch_sequence df_game.sequence_number NOT NULL,
    run_id df_game.identity_bytes NOT NULL,
    schema_version integer NOT NULL CHECK (schema_version > 0),
    codec_version integer NOT NULL CHECK (codec_version > 0),
    complete_envelope bytea NOT NULL,
    PRIMARY KEY (tenant_id, session_id, recovery_epoch, in_epoch_sequence),
    FOREIGN KEY (tenant_id, session_id) REFERENCES df_game.sessions
);
CREATE TABLE df_game.operations (
    tenant_id df_game.identity_bytes NOT NULL,
    session_id df_game.identity_bytes NOT NULL,
    principal_id df_game.identity_bytes NOT NULL,
    command_namespace bytea NOT NULL,
    recovery_epoch df_game.epoch_number NOT NULL,
    operation_id df_game.identity_bytes NOT NULL,
    fingerprint_version integer NOT NULL CHECK (fingerprint_version > 0),
    canonical_fingerprint bytea NOT NULL CHECK (octet_length(canonical_fingerprint) = 32),
    committed_epoch df_game.epoch_number NOT NULL,
    committed_sequence df_game.sequence_number NOT NULL,
    receipt_version integer NOT NULL CHECK (receipt_version > 0),
    receipt bytea,
    PRIMARY KEY (tenant_id, session_id, principal_id, command_namespace,
                 recovery_epoch, operation_id),
    FOREIGN KEY (tenant_id, session_id, committed_epoch, committed_sequence)
        REFERENCES df_game.checkpoints
);
CREATE TABLE df_game.facts (
    tenant_id df_game.identity_bytes NOT NULL,
    session_id df_game.identity_bytes NOT NULL,
    committed_epoch df_game.epoch_number NOT NULL,
    committed_sequence df_game.sequence_number NOT NULL,
    ordinal bigint NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
    fact_version integer NOT NULL CHECK (fact_version > 0),
    fact bytea NOT NULL,
    PRIMARY KEY (tenant_id, session_id, committed_epoch, committed_sequence, ordinal),
    FOREIGN KEY (tenant_id, session_id, committed_epoch, committed_sequence)
        REFERENCES df_game.checkpoints
);
CREATE TABLE df_game.intents (
    tenant_id df_game.identity_bytes NOT NULL,
    session_id df_game.identity_bytes NOT NULL,
    principal_id df_game.identity_bytes NOT NULL,
    command_namespace bytea NOT NULL,
    recovery_epoch df_game.epoch_number NOT NULL,
    operation_id df_game.identity_bytes NOT NULL,
    slot bigint NOT NULL CHECK (slot BETWEEN 0 AND 4294967295),
    effect_id df_game.identity_bytes NOT NULL,
    job_id df_game.identity_bytes,
    timer_id df_game.identity_bytes,
    run_id df_game.identity_bytes NOT NULL,
    committed_epoch df_game.epoch_number NOT NULL,
    committed_sequence df_game.sequence_number NOT NULL,
    process_generation df_game.epoch_number NOT NULL,
    execution_mode smallint NOT NULL CHECK (execution_mode BETWEEN 1 AND 3),
    intent_kind smallint NOT NULL CHECK (intent_kind BETWEEN 1 AND 7),
    intent_version integer NOT NULL CHECK (intent_version > 0),
    intent bytea NOT NULL,
    PRIMARY KEY (tenant_id, session_id, principal_id, command_namespace,
                 recovery_epoch, operation_id, slot),
    UNIQUE (tenant_id, effect_id),
    FOREIGN KEY (tenant_id, session_id, committed_epoch, committed_sequence)
        REFERENCES df_game.checkpoints,
    FOREIGN KEY (tenant_id, session_id, principal_id, command_namespace,
                 recovery_epoch, operation_id) REFERENCES df_game.operations
);
-- Cancellation intents reference existing identities and retain their own unique effect/slot.
-- Only initial dispatch/arming creates the globally stable job/timer identity.
CREATE UNIQUE INDEX intents_created_job ON df_game.intents (tenant_id, job_id)
    WHERE intent_kind IN (1, 2, 3) AND job_id IS NOT NULL;
CREATE UNIQUE INDEX intents_created_timer ON df_game.intents (tenant_id, timer_id)
    WHERE intent_kind = 4 AND timer_id IS NOT NULL;
CREATE TABLE df_game.retired_namespaces (
    tenant_id df_game.identity_bytes NOT NULL,
    session_id df_game.identity_bytes NOT NULL,
    principal_id df_game.identity_bytes NOT NULL,
    command_namespace bytea NOT NULL,
    recovery_epoch df_game.epoch_number NOT NULL,
    PRIMARY KEY (tenant_id, session_id, principal_id, command_namespace, recovery_epoch),
    FOREIGN KEY (tenant_id, session_id) REFERENCES df_game.sessions
);

ALTER TABLE df_game.sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE df_game.sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE df_game.checkpoints ENABLE ROW LEVEL SECURITY;
ALTER TABLE df_game.checkpoints FORCE ROW LEVEL SECURITY;
ALTER TABLE df_game.operations ENABLE ROW LEVEL SECURITY;
ALTER TABLE df_game.operations FORCE ROW LEVEL SECURITY;
ALTER TABLE df_game.facts ENABLE ROW LEVEL SECURITY;
ALTER TABLE df_game.facts FORCE ROW LEVEL SECURITY;
ALTER TABLE df_game.intents ENABLE ROW LEVEL SECURITY;
ALTER TABLE df_game.intents FORCE ROW LEVEL SECURITY;
ALTER TABLE df_game.retired_namespaces ENABLE ROW LEVEL SECURITY;
ALTER TABLE df_game.retired_namespaces FORCE ROW LEVEL SECURITY;
REVOKE ALL ON ALL TABLES IN SCHEMA df_game FROM PUBLIC;
COMMIT;
