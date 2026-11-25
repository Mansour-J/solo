#!/bin/sh
# Runs once, when the PostgreSQL data volume is first initialised (docker-entrypoint-initdb.d).
# Creates the API's login role (solo_app, which the migrations grant to) and the schema Flyway
# migrates. Tables and grants come from Flyway. Also creates `<db>_test` for integration tests.
set -eu

: "${APP_DB_PASSWORD:?APP_DB_PASSWORD must be set}"

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
  -v app_password="$APP_DB_PASSWORD" -v test_db="${POSTGRES_DB}_test" <<-'EOSQL'
  CREATE ROLE solo_app LOGIN PASSWORD :'app_password';
  CREATE SCHEMA IF NOT EXISTS solo;
  CREATE DATABASE :"test_db";
EOSQL

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "${POSTGRES_DB}_test" <<-'EOSQL'
  CREATE SCHEMA IF NOT EXISTS solo;
EOSQL
