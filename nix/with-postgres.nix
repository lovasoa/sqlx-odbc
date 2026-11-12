{
  postgresql,
  writeShellApplication,
}:

writeShellApplication {
  name = "with-postgres";
  runtimeInputs = [ postgresql ];
  text = ''
    if [[ $# -eq 0 ]]; then
        echo "usage: with-postgres <command> [args...]" >&2
        echo "Starts a throwaway PostgreSQL ${postgresql.version} server, runs the command, then stops the server." >&2
        exit 2
    fi

    server_dir="$(mktemp -d)"
    stop_server() {
        pg_ctl --pgdata="$server_dir/data" --mode=immediate stop >/dev/null 2>&1 || true
        rm -rf "$server_dir"
    }
    trap stop_server EXIT

    initdb --pgdata="$server_dir/data" --username=postgres --auth=trust >/dev/null
    pg_ctl --pgdata="$server_dir/data" --log="$server_dir/server.log" --wait \
        --options="-c listen_addresses= -k $server_dir" start >/dev/null

    export POSTGRES_HOST="$server_dir"
    export POSTGRES_PORT=5432
    export POSTGRES_DATABASE=postgres
    export POSTGRES_USER=postgres
    export POSTGRES_PASSWORD=postgres

    "$@"
  '';
}
