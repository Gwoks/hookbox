-- migrations/0003_trace_counter_trigger.sql ---------------------------------
-- AC-73(b)/AC-S20 regression fix (hookbox-mun.30): the endpoint lifetime
-- counter bump (request_count/last_hit) that hookbox-mun.26 added as a
-- standalone `UPDATE endpoints ...` statement inside `insert_trace` made every
-- traced mock request issue THREE SQLite statements, where the frozen
-- baseline (§4.8 AC-73(b), §5.5.4 note for QA AC-S20) is TWO: insert + prune.
--
-- Move the bump into an AFTER INSERT trigger on request_logs so it rides
-- along with the INSERT statement itself (SQLite fires triggers as part of
-- the same statement, not as a separate round trip) instead of costing the
-- app a fourth->third-turned-second explicit statement. `insert_trace` goes
-- back to issuing exactly two statements: the INSERT (which now also bumps
-- the counters via this trigger) and the prune DELETE.
CREATE TRIGGER trg_request_logs_bump_counters
AFTER INSERT ON request_logs
BEGIN
    UPDATE endpoints
       SET request_count = request_count + 1,
           last_hit = datetime('now')
     WHERE token = NEW.token;
END;
