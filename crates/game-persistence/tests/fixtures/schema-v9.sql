


SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;


CREATE TABLE public.games (
    id text NOT NULL COLLATE pg_catalog."C",
    black text NOT NULL,
    white text NOT NULL,
    version bigint NOT NULL,
    winner integer,
    reason integer,
    rule text NOT NULL,
    visibility text NOT NULL,
    finished_at bigint,
    final_move_ply bigint NOT NULL,
    initial_main_ms bigint NOT NULL,
    byoyomi_ms bigint NOT NULL,
    created_at bigint NOT NULL,
    clock_black bigint NOT NULL,
    clock_white bigint NOT NULL,
    clock_started bigint,
    clock_deadline bigint,
    clock_stopped bigint,
    byoyomi_black bigint,
    byoyomi_white bigint,
    CONSTRAINT games_byoyomi_black_check CHECK (((byoyomi_black IS NULL) OR (byoyomi_black >= 0))),
    CONSTRAINT games_byoyomi_ms_check CHECK ((byoyomi_ms = 30000)),
    CONSTRAINT games_byoyomi_white_check CHECK (((byoyomi_white IS NULL) OR (byoyomi_white >= 0))),
    CONSTRAINT games_check CHECK ((black <> white)),
    CONSTRAINT games_check1 CHECK ((((winner IS NULL) AND (reason IS NULL) AND (finished_at IS NULL)) OR ((winner IS NOT NULL) AND (reason IS NOT NULL) AND (finished_at IS NOT NULL) AND (winner = ANY (ARRAY[1, 2])) AND (reason = ANY (ARRAY[1, 2, 3]))))),
    CONSTRAINT games_check2 CHECK ((((clock_started IS NOT NULL) AND (clock_deadline IS NOT NULL) AND (clock_stopped IS NULL) AND (byoyomi_black IS NULL) AND (byoyomi_white IS NULL)) OR ((clock_started IS NULL) AND (clock_deadline IS NULL) AND (clock_stopped IS NOT NULL) AND (byoyomi_black IS NOT NULL) AND (byoyomi_white IS NOT NULL)))),
    CONSTRAINT games_clock_black_check CHECK ((clock_black >= 0)),
    CONSTRAINT games_clock_deadline_check CHECK (((clock_deadline IS NULL) OR (clock_deadline >= 0))),
    CONSTRAINT games_clock_started_check CHECK (((clock_started IS NULL) OR (clock_started >= 0))),
    CONSTRAINT games_clock_stopped_check CHECK (((clock_stopped IS NULL) OR (clock_stopped >= 0))),
    CONSTRAINT games_clock_white_check CHECK ((clock_white >= 0)),
    CONSTRAINT games_created_at_check CHECK ((created_at >= 0)),
    CONSTRAINT games_final_move_ply_check CHECK ((final_move_ply >= 0)),
    CONSTRAINT games_finished_at_check CHECK (((finished_at IS NULL) OR (finished_at >= 0))),
    CONSTRAINT games_initial_main_ms_check CHECK ((initial_main_ms = ANY (ARRAY[(0)::bigint, (600000)::bigint]))),
    CONSTRAINT games_rule_check CHECK ((rule = 'standard'::text)),
    CONSTRAINT games_version_check CHECK ((version >= 0)),
    CONSTRAINT games_visibility_check CHECK ((visibility = ANY (ARRAY['public'::text, 'private'::text])))
);



CREATE TABLE public.journal (
    game_id text NOT NULL,
    seq bigint NOT NULL,
    request_id text,
    user_id text,
    expected_seq bigint NOT NULL,
    action integer NOT NULL,
    usi text,
    in_check boolean NOT NULL,
    winner integer,
    reason integer,
    evaluated_at bigint NOT NULL,
    loser integer,
    deadline_at bigint,
    clock_black bigint NOT NULL,
    clock_white bigint NOT NULL,
    clock_started bigint,
    clock_deadline bigint,
    clock_stopped bigint,
    byoyomi_black bigint,
    byoyomi_white bigint,
    CONSTRAINT journal_byoyomi_black_check CHECK (((byoyomi_black IS NULL) OR (byoyomi_black >= 0))),
    CONSTRAINT journal_byoyomi_white_check CHECK (((byoyomi_white IS NULL) OR (byoyomi_white >= 0))),
    CONSTRAINT journal_check CHECK ((expected_seq = (seq - 1))),
    CONSTRAINT journal_check1 CHECK ((((action = 1) AND (usi IS NOT NULL)) OR ((action = ANY (ARRAY[2, 3])) AND (usi IS NULL)))),
    CONSTRAINT journal_check2 CHECK ((((winner IS NULL) AND (reason IS NULL)) OR ((winner IS NOT NULL) AND (reason IS NOT NULL) AND (winner = ANY (ARRAY[1, 2])) AND (reason = ANY (ARRAY[1, 2, 3]))))),
    CONSTRAINT journal_check3 CHECK ((((action = ANY (ARRAY[1, 2])) AND (request_id IS NOT NULL) AND (user_id IS NOT NULL) AND (loser IS NULL) AND (deadline_at IS NULL)) OR ((action = 3) AND (request_id IS NULL) AND (user_id IS NULL) AND (loser = ANY (ARRAY[1, 2])) AND (deadline_at IS NOT NULL) AND (reason = 3)))),
    CONSTRAINT journal_check4 CHECK ((((clock_started IS NOT NULL) AND (clock_deadline IS NOT NULL) AND (clock_stopped IS NULL) AND (byoyomi_black IS NULL) AND (byoyomi_white IS NULL)) OR ((clock_started IS NULL) AND (clock_deadline IS NULL) AND (clock_stopped IS NOT NULL) AND (byoyomi_black IS NOT NULL) AND (byoyomi_white IS NOT NULL)))),
    CONSTRAINT journal_clock_black_check CHECK ((clock_black >= 0)),
    CONSTRAINT journal_clock_deadline_check CHECK (((clock_deadline IS NULL) OR (clock_deadline >= 0))),
    CONSTRAINT journal_clock_started_check CHECK (((clock_started IS NULL) OR (clock_started >= 0))),
    CONSTRAINT journal_clock_stopped_check CHECK (((clock_stopped IS NULL) OR (clock_stopped >= 0))),
    CONSTRAINT journal_clock_white_check CHECK ((clock_white >= 0)),
    CONSTRAINT journal_evaluated_at_check CHECK ((evaluated_at >= 0)),
    CONSTRAINT journal_expected_seq_check CHECK ((expected_seq >= 0)),
    CONSTRAINT journal_seq_check CHECK ((seq > 0))
);



CREATE TABLE public.matchmaking_tickets (
    user_id text NOT NULL,
    ticket_id text NOT NULL,
    pool text,
    state text NOT NULL,
    game_id text,
    prospective_game_id text,
    refusal_reason text,
    enqueued_at bigint,
    expires_at bigint,
    rating bigint,
    completed bigint,
    heartbeat_seq bigint,
    ready_deadline bigint,
    side integer,
    matched_side integer,
    ready boolean,
    CONSTRAINT matchmaking_tickets_check CHECK (((pool IS NOT NULL) OR (state = 'cancelled'::text))),
    CONSTRAINT matchmaking_tickets_check1 CHECK ((((enqueued_at IS NULL) AND (expires_at IS NULL) AND (rating IS NULL) AND (completed IS NULL) AND (heartbeat_seq IS NULL)) OR ((enqueued_at IS NOT NULL) AND (expires_at IS NOT NULL) AND (expires_at > enqueued_at) AND (completed IS NOT NULL) AND (heartbeat_seq IS NOT NULL) AND (heartbeat_seq >= 0) AND (((rating IS NULL) AND (completed = 0)) OR ((rating IS NOT NULL) AND (completed > 0)))))),
    CONSTRAINT matchmaking_tickets_check2 CHECK (((state <> 'waiting'::text) OR ((enqueued_at IS NOT NULL) AND (game_id IS NULL)))),
    CONSTRAINT matchmaking_tickets_check3 CHECK ((((state = ANY (ARRAY['playing'::text, 'finished'::text])) AND (game_id IS NOT NULL) AND (side IS NOT NULL) AND (side = ANY (ARRAY[1, 2])) AND (game_id = prospective_game_id) AND (side = matched_side) AND (ready = true) AND (enqueued_at IS NOT NULL)) OR ((state <> ALL (ARRAY['playing'::text, 'finished'::text])) AND (game_id IS NULL) AND (side IS NULL)))),
    CONSTRAINT matchmaking_tickets_check4 CHECK ((((state = ANY (ARRAY['matched'::text, 'playing'::text, 'finished'::text, 'voided'::text])) AND (prospective_game_id IS NOT NULL) AND (matched_side IS NOT NULL) AND (matched_side = ANY (ARRAY[1, 2])) AND (ready IS NOT NULL) AND (ready_deadline IS NOT NULL) AND (enqueued_at IS NOT NULL)) OR ((state <> ALL (ARRAY['matched'::text, 'playing'::text, 'finished'::text, 'voided'::text])) AND (prospective_game_id IS NULL) AND (matched_side IS NULL) AND (ready IS NULL) AND (ready_deadline IS NULL)))),
    CONSTRAINT matchmaking_tickets_check5 CHECK ((((state = 'refused'::text) AND (refusal_reason IS NOT NULL) AND (refusal_reason = ANY (ARRAY['already_waiting'::text, 'already_matched'::text, 'already_playing'::text, 'rating_pending'::text])) AND (enqueued_at IS NULL)) OR ((state <> 'refused'::text) AND (refusal_reason IS NULL)))),
    CONSTRAINT matchmaking_tickets_pool_check CHECK (((pool IS NULL) OR (pool = ANY (ARRAY['ten_minute_30'::text, 'byoyomi_30'::text])))),
    CONSTRAINT matchmaking_tickets_state_check CHECK ((state = ANY (ARRAY['waiting'::text, 'matched'::text, 'playing'::text, 'finished'::text, 'voided'::text, 'cancelled'::text, 'expired'::text, 'refused'::text])))
);



CREATE TABLE public.outbox_events (
    id bigint NOT NULL,
    game_id text NOT NULL,
    seq bigint NOT NULL,
    event_kind integer NOT NULL,
    encoding_version integer NOT NULL,
    request_id text,
    user_id text,
    usi text,
    in_check boolean,
    winner integer,
    reason integer,
    published boolean DEFAULT false NOT NULL,
    evaluated_at bigint NOT NULL,
    clock_black bigint NOT NULL,
    clock_white bigint NOT NULL,
    clock_started bigint,
    clock_deadline bigint,
    clock_stopped bigint,
    byoyomi_black bigint,
    byoyomi_white bigint,
    CONSTRAINT outbox_events_byoyomi_black_check CHECK (((byoyomi_black IS NULL) OR (byoyomi_black >= 0))),
    CONSTRAINT outbox_events_byoyomi_white_check CHECK (((byoyomi_white IS NULL) OR (byoyomi_white >= 0))),
    CONSTRAINT outbox_events_check CHECK ((((winner IS NULL) AND (reason IS NULL)) OR ((winner IS NOT NULL) AND (reason IS NOT NULL) AND (winner = ANY (ARRAY[1, 2])) AND (reason = ANY (ARRAY[1, 2, 3]))))),
    CONSTRAINT outbox_events_check1 CHECK ((((request_id IS NULL) AND (user_id IS NULL) AND (reason = 3)) OR ((request_id IS NOT NULL) AND (user_id IS NOT NULL) AND ((reason IS NULL) OR (reason <> 3))))),
    CONSTRAINT outbox_events_check2 CHECK ((((event_kind = 0) AND (usi IS NOT NULL) AND (in_check IS NOT NULL) AND ((reason IS NULL) OR (reason = 1))) OR ((event_kind = 1) AND (usi IS NULL) AND (in_check IS NULL) AND (winner IS NOT NULL)))),
    CONSTRAINT outbox_events_check3 CHECK ((((clock_started IS NOT NULL) AND (clock_deadline IS NOT NULL) AND (clock_stopped IS NULL) AND (byoyomi_black IS NULL) AND (byoyomi_white IS NULL)) OR ((clock_started IS NULL) AND (clock_deadline IS NULL) AND (clock_stopped IS NOT NULL) AND (byoyomi_black IS NOT NULL) AND (byoyomi_white IS NOT NULL)))),
    CONSTRAINT outbox_events_clock_black_check CHECK ((clock_black >= 0)),
    CONSTRAINT outbox_events_clock_deadline_check CHECK (((clock_deadline IS NULL) OR (clock_deadline >= 0))),
    CONSTRAINT outbox_events_clock_started_check CHECK (((clock_started IS NULL) OR (clock_started >= 0))),
    CONSTRAINT outbox_events_clock_stopped_check CHECK (((clock_stopped IS NULL) OR (clock_stopped >= 0))),
    CONSTRAINT outbox_events_clock_white_check CHECK ((clock_white >= 0)),
    CONSTRAINT outbox_events_evaluated_at_check CHECK ((evaluated_at >= 0)),
    CONSTRAINT outbox_events_event_kind_check CHECK ((event_kind = ANY (ARRAY[0, 1])))
);



ALTER TABLE public.outbox_events ALTER COLUMN id ADD GENERATED BY DEFAULT AS IDENTITY (
    SEQUENCE NAME public.outbox_events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);



CREATE TABLE public.rated_games (
    game_id text NOT NULL,
    pool text NOT NULL,
    black_start_rating bigint,
    black_start_count bigint NOT NULL,
    white_start_rating bigint,
    white_start_count bigint NOT NULL,
    CONSTRAINT rated_games_check CHECK ((((black_start_rating IS NULL) AND (black_start_count = 0)) OR ((black_start_rating IS NOT NULL) AND (black_start_count > 0)))),
    CONSTRAINT rated_games_check1 CHECK ((((white_start_rating IS NULL) AND (white_start_count = 0)) OR ((white_start_rating IS NOT NULL) AND (white_start_count > 0)))),
    CONSTRAINT rated_games_pool_check CHECK ((pool = ANY (ARRAY['ten_minute_30'::text, 'byoyomi_30'::text])))
);



CREATE TABLE public.rating_applications (
    game_id text NOT NULL,
    pool text NOT NULL,
    final_seq bigint NOT NULL,
    delta_black bigint NOT NULL,
    black_before_rating bigint,
    black_before_count bigint NOT NULL,
    white_before_rating bigint,
    white_before_count bigint NOT NULL,
    black_after_rating bigint NOT NULL,
    black_after_count bigint NOT NULL,
    white_after_rating bigint NOT NULL,
    white_after_count bigint NOT NULL,
    CONSTRAINT rating_applications_black_after_count_check CHECK ((black_after_count > 0)),
    CONSTRAINT rating_applications_check CHECK ((((black_before_rating IS NULL) AND (black_before_count = 0)) OR ((black_before_rating IS NOT NULL) AND (black_before_count > 0)))),
    CONSTRAINT rating_applications_check1 CHECK ((((white_before_rating IS NULL) AND (white_before_count = 0)) OR ((white_before_rating IS NOT NULL) AND (white_before_count > 0)))),
    CONSTRAINT rating_applications_delta_black_check CHECK (((delta_black >= '-32'::integer) AND (delta_black <= 32))),
    CONSTRAINT rating_applications_final_seq_check CHECK ((final_seq > 0)),
    CONSTRAINT rating_applications_pool_check CHECK ((pool = ANY (ARRAY['ten_minute_30'::text, 'byoyomi_30'::text]))),
    CONSTRAINT rating_applications_white_after_count_check CHECK ((white_after_count > 0))
);



CREATE TABLE public.rating_jobs (
    game_id text NOT NULL,
    state text NOT NULL,
    owner text,
    generation bigint NOT NULL,
    lease_until bigint,
    available_at bigint NOT NULL,
    attempts bigint NOT NULL,
    last_error text,
    CONSTRAINT rating_jobs_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT rating_jobs_check CHECK ((((state = 'claimed'::text) AND (owner IS NOT NULL) AND (lease_until IS NOT NULL)) OR ((state <> 'claimed'::text) AND (owner IS NULL) AND (lease_until IS NULL)))),
    CONSTRAINT rating_jobs_generation_check CHECK ((generation >= 0)),
    CONSTRAINT rating_jobs_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'claimed'::text, 'quarantined'::text, 'completed'::text])))
);



CREATE TABLE public.rating_slots (
    user_id text NOT NULL,
    pool text NOT NULL,
    game_id text NOT NULL,
    CONSTRAINT rating_slots_pool_check CHECK ((pool = ANY (ARRAY['ten_minute_30'::text, 'byoyomi_30'::text])))
);



CREATE TABLE public.rating_states (
    user_id text NOT NULL,
    pool text NOT NULL,
    rating bigint,
    completed bigint NOT NULL,
    CONSTRAINT rating_states_check CHECK ((((rating IS NULL) AND (completed = 0)) OR ((rating IS NOT NULL) AND (completed > 0)))),
    CONSTRAINT rating_states_pool_check CHECK ((pool = ANY (ARRAY['ten_minute_30'::text, 'byoyomi_30'::text])))
);



CREATE TABLE public.schema_version (
    id bigint NOT NULL,
    version bigint NOT NULL
);



ALTER TABLE ONLY public.games
    ADD CONSTRAINT games_pkey PRIMARY KEY (id);



ALTER TABLE ONLY public.journal
    ADD CONSTRAINT journal_game_id_request_id_key UNIQUE (game_id, request_id);



ALTER TABLE ONLY public.journal
    ADD CONSTRAINT journal_pkey PRIMARY KEY (game_id, seq);



ALTER TABLE ONLY public.matchmaking_tickets
    ADD CONSTRAINT matchmaking_tickets_game_id_side_key UNIQUE (game_id, side);



ALTER TABLE ONLY public.matchmaking_tickets
    ADD CONSTRAINT matchmaking_tickets_pkey PRIMARY KEY (user_id, ticket_id);



ALTER TABLE ONLY public.matchmaking_tickets
    ADD CONSTRAINT matchmaking_tickets_prospective_game_id_matched_side_key UNIQUE (prospective_game_id, matched_side);



ALTER TABLE ONLY public.outbox_events
    ADD CONSTRAINT outbox_events_game_id_seq_event_kind_key UNIQUE (game_id, seq, event_kind);



ALTER TABLE ONLY public.outbox_events
    ADD CONSTRAINT outbox_events_pkey PRIMARY KEY (id);



ALTER TABLE ONLY public.rated_games
    ADD CONSTRAINT rated_games_pkey PRIMARY KEY (game_id);



ALTER TABLE ONLY public.rating_applications
    ADD CONSTRAINT rating_applications_pkey PRIMARY KEY (game_id);



ALTER TABLE ONLY public.rating_jobs
    ADD CONSTRAINT rating_jobs_pkey PRIMARY KEY (game_id);



ALTER TABLE ONLY public.rating_slots
    ADD CONSTRAINT rating_slots_pkey PRIMARY KEY (user_id, pool);



ALTER TABLE ONLY public.rating_states
    ADD CONSTRAINT rating_states_pkey PRIMARY KEY (user_id, pool);



ALTER TABLE ONLY public.schema_version
    ADD CONSTRAINT schema_version_pkey PRIMARY KEY (id);



CREATE INDEX games_deadlines ON public.games USING btree (clock_deadline) WHERE (winner IS NULL);



CREATE UNIQUE INDEX matchmaking_active_user ON public.matchmaking_tickets USING btree (user_id) WHERE (state = ANY (ARRAY['waiting'::text, 'matched'::text, 'playing'::text]));



CREATE INDEX matchmaking_matched_deadline ON public.matchmaking_tickets USING btree (ready_deadline) WHERE (state = 'matched'::text);



CREATE INDEX matchmaking_waiting_order ON public.matchmaking_tickets USING btree (pool, enqueued_at, ticket_id) WHERE (state = 'waiting'::text);



CREATE INDEX matchmaking_waiting_rating ON public.matchmaking_tickets USING btree (pool, rating, ticket_id) WHERE (state = 'waiting'::text);



CREATE INDEX outbox_pending ON public.outbox_events USING btree (published, id);



CREATE INDEX rating_jobs_expired ON public.rating_jobs USING btree (state, lease_until, game_id);



CREATE INDEX rating_jobs_pending ON public.rating_jobs USING btree (state, available_at, game_id);



CREATE INDEX records_black ON public.games USING btree (black, finished_at DESC, id DESC) WHERE ((visibility = 'public'::text) AND (finished_at IS NOT NULL));



CREATE INDEX records_finished ON public.games USING btree (finished_at DESC, id DESC) WHERE ((visibility = 'public'::text) AND (finished_at IS NOT NULL));



CREATE INDEX records_white ON public.games USING btree (white, finished_at DESC, id DESC) WHERE ((visibility = 'public'::text) AND (finished_at IS NOT NULL));



ALTER TABLE ONLY public.journal
    ADD CONSTRAINT journal_game_id_fkey FOREIGN KEY (game_id) REFERENCES public.games(id);



ALTER TABLE ONLY public.matchmaking_tickets
    ADD CONSTRAINT matchmaking_tickets_game_id_fkey FOREIGN KEY (game_id) REFERENCES public.games(id);



ALTER TABLE ONLY public.outbox_events
    ADD CONSTRAINT outbox_events_game_id_seq_fkey FOREIGN KEY (game_id, seq) REFERENCES public.journal(game_id, seq);



ALTER TABLE ONLY public.rated_games
    ADD CONSTRAINT rated_games_game_id_fkey FOREIGN KEY (game_id) REFERENCES public.games(id);



ALTER TABLE ONLY public.rating_applications
    ADD CONSTRAINT rating_applications_game_id_fkey FOREIGN KEY (game_id) REFERENCES public.rating_jobs(game_id);



ALTER TABLE ONLY public.rating_jobs
    ADD CONSTRAINT rating_jobs_game_id_fkey FOREIGN KEY (game_id) REFERENCES public.rated_games(game_id);



ALTER TABLE ONLY public.rating_slots
    ADD CONSTRAINT rating_slots_game_id_fkey FOREIGN KEY (game_id) REFERENCES public.rated_games(game_id);



ALTER TABLE ONLY public.rating_slots
    ADD CONSTRAINT rating_slots_user_id_pool_fkey FOREIGN KEY (user_id, pool) REFERENCES public.rating_states(user_id, pool);




INSERT INTO public.schema_version VALUES (0,9);
