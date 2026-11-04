CREATE ROLE shogi LOGIN PASSWORD 'shogi';
CREATE ROLE shogi_reader LOGIN PASSWORD 'shogi_reader';
CREATE DATABASE shogi OWNER shogi;
CREATE DATABASE users OWNER shogi;
REVOKE CONNECT ON DATABASE users FROM PUBLIC;
GRANT CONNECT ON DATABASE users TO shogi;
REVOKE CONNECT ON DATABASE shogi FROM PUBLIC;
GRANT CONNECT ON DATABASE shogi TO shogi, shogi_reader;

\connect shogi
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE ON SCHEMA public TO shogi_reader;
ALTER DEFAULT PRIVILEGES FOR ROLE shogi IN SCHEMA public
    GRANT SELECT ON TABLES TO shogi_reader;
ALTER ROLE shogi_reader IN DATABASE shogi SET default_transaction_read_only = on;
ALTER ROLE shogi_reader IN DATABASE shogi SET statement_timeout = '2s';
