-- Fixture-only faults for ROOT's registered isolated database, never production migration.
-- Installing one AFTER INSERT trigger exercises rollback after that family was written.
-- Trigger function creation and trigger installation are performed only by fixture admin.
CREATE FUNCTION df_fixture_authority.fail_inserted_family()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog AS $$
BEGIN
    RAISE LOG 'registered fixture family insertion fault: %', TG_TABLE_NAME;
    RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'registered fixture family insertion fault';
END;
$$;
REVOKE ALL ON FUNCTION df_fixture_authority.fail_inserted_family() FROM PUBLIC;
-- The fixture caller installs/removes one of these exact whitelisted statements per case:
-- CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.checkpoints
--     FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family();
-- CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.facts
--     FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family();
-- CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.operations
--     FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family();
-- CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.intents
--     FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family();

-- Refuse the actual last revision update only after all four candidate families exist.
-- This fixture-owned trigger never grants membership or issues a production capability.
CREATE FUNCTION df_fixture_authority.refuse_final_revision_cas()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
BEGIN
    IF NEW.in_epoch_sequence <> OLD.in_epoch_sequence + 1
        OR (SELECT count(*) FROM df_game.checkpoints
            WHERE tenant_id = NEW.tenant_id AND session_id = NEW.session_id
              AND recovery_epoch = NEW.recovery_epoch
              AND in_epoch_sequence = NEW.in_epoch_sequence) <> 1
        OR (SELECT count(*) FROM df_game.facts
            WHERE tenant_id = NEW.tenant_id AND session_id = NEW.session_id
              AND committed_epoch = NEW.recovery_epoch
              AND committed_sequence = NEW.in_epoch_sequence) <> 1
        OR (SELECT count(*) FROM df_game.operations
            WHERE tenant_id = NEW.tenant_id AND session_id = NEW.session_id
              AND committed_epoch = NEW.recovery_epoch
              AND committed_sequence = NEW.in_epoch_sequence) <> 1
        OR (SELECT count(*) FROM df_game.intents
            WHERE tenant_id = NEW.tenant_id AND session_id = NEW.session_id
              AND committed_epoch = NEW.recovery_epoch
              AND committed_sequence = NEW.in_epoch_sequence) <> 1 THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001',
            MESSAGE = 'registered final CAS fixture did not observe all four decision families';
    END IF;
    RAISE LOG 'registered final revision CAS refusal after all four decision families';
    RETURN NULL;
END;
$$;
REVOKE ALL ON FUNCTION df_fixture_authority.refuse_final_revision_cas() FROM PUBLIC;


-- A deterministic write-boundary barrier expires the actual database lease after
-- all four families exist, before the adapter evaluates its final strict-clock CAS.
CREATE FUNCTION df_fixture_authority.expire_lease_before_cas()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
    IF (SELECT count(*) FROM df_game.checkpoints WHERE tenant_id=NEW.tenant_id
        AND session_id=NEW.session_id AND recovery_epoch=NEW.committed_epoch
        AND in_epoch_sequence=NEW.committed_sequence)<>1
       OR (SELECT count(*) FROM df_game.facts WHERE tenant_id=NEW.tenant_id
        AND session_id=NEW.session_id AND committed_epoch=NEW.committed_epoch
        AND committed_sequence=NEW.committed_sequence)<>1
       OR (SELECT count(*) FROM df_game.operations WHERE tenant_id=NEW.tenant_id
        AND session_id=NEW.session_id AND committed_epoch=NEW.committed_epoch
        AND committed_sequence=NEW.committed_sequence)<>1
       OR (SELECT count(*) FROM df_game.intents WHERE tenant_id=NEW.tenant_id
        AND session_id=NEW.session_id AND committed_epoch=NEW.committed_epoch
        AND committed_sequence=NEW.committed_sequence)<>1 THEN
        RAISE EXCEPTION 'registered lease barrier missing a decision family';
    END IF;
    UPDATE df_game.sessions SET lease_until=clock_timestamp()
        WHERE tenant_id=NEW.tenant_id AND session_id=NEW.session_id;
    IF NOT FOUND THEN RAISE EXCEPTION 'registered lease barrier missing session'; END IF;
    RAISE LOG 'registered lease expiry after four families before final CAS';
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION df_fixture_authority.expire_lease_before_cas() FROM PUBLIC;

-- Exercise actual schema constraints, not a synthetic SQLSTATE. The depth-two
-- insertion creates a valid sibling projection and the outer insertion must hit
-- the selected actual primary key or partial unique index. Both roll back.
CREATE FUNCTION df_fixture_authority.collide_projection_identity()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE
    duplicate_slot bigint;
    duplicate_effect bytea;
BEGIN
    IF pg_trigger_depth()<>1 THEN RETURN NEW; END IF;
    IF TG_TABLE_NAME='facts' AND TG_ARGV[0]='fact-ordinal' THEN
        RAISE LOG 'registered actual uniqueness collision: fact-ordinal';
        INSERT INTO df_game.facts VALUES (NEW.tenant_id,NEW.session_id,NEW.committed_epoch,
            NEW.committed_sequence,NEW.ordinal,NEW.fact_version,NEW.fact);
    ELSIF TG_TABLE_NAME='intents' AND TG_ARGV[0] IN ('intent-slot','effect-id','created-job','created-timer') THEN
        IF TG_ARGV[0]='created-job' AND (NEW.job_id IS NULL OR NEW.intent_kind NOT IN (1,2,3))
            OR TG_ARGV[0]='created-timer' AND (NEW.timer_id IS NULL OR NEW.intent_kind<>4) THEN
            RAISE EXCEPTION 'registered uniqueness fixture missing creation identity';
        END IF;
        duplicate_slot=CASE WHEN TG_ARGV[0]='intent-slot' THEN NEW.slot ELSE NEW.slot+1 END;
        duplicate_effect=CASE WHEN TG_ARGV[0]='effect-id' THEN NEW.effect_id
            ELSE decode(repeat('5a',16),'hex') END;
        RAISE LOG 'registered actual uniqueness collision: %',TG_ARGV[0];
        INSERT INTO df_game.intents VALUES (NEW.tenant_id,NEW.session_id,NEW.principal_id,
            NEW.command_namespace,NEW.recovery_epoch,NEW.operation_id,duplicate_slot,
            duplicate_effect,NEW.job_id,NEW.timer_id,NEW.run_id,NEW.committed_epoch,
            NEW.committed_sequence,NEW.process_generation,NEW.execution_mode,
            NEW.intent_kind,NEW.intent_version,NEW.intent);
    ELSE
        RAISE EXCEPTION 'unregistered uniqueness fixture selector';
    END IF;
    RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION df_fixture_authority.collide_projection_identity() FROM PUBLIC;
