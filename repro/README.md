# Reproductions

A reproduction is one ordinary sqlx program in a single Rust file, posted as a comment on an issue or pull request. The runner prints the file, runs it against sqlx-oldapi 0.6.59, the release SQLPage uses, then runs the same file against a revision of sqlx-odbc. Each run happens once on DuckDB and once on PostgreSQL. Comparing the outputs shows whether a behavior difference is real, and whether a change fixes it.

Reproductions are not committed. Once a difference is understood, a unit test or an integration test in `tests/` guards the behavior instead.

## Run one from a comment

Copy the whole block from the comment, paste it into a terminal, and press Enter. A reproduction comment looks like this:

```sh
nix run github:lovasoa/sqlx-odbc#repro <<'EOF'
---cargo
[package]
edition = "2024"

[dependencies]
sqlx = { package = "sqlx-repro", git = "https://github.com/lovasoa/sqlx-odbc" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
---

use sqlx::{Connection, OdbcConnection, Row};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = OdbcConnection::connect(&std::env::var("DATABASE_URL")?).await?;
    let row = sqlx::query("SELECT 41 + 1").fetch_one(&mut connection).await?;

    println!("{:?}", row.try_get::<i64, _>(0));
    Ok(())
}
EOF
```

It prints:

```text
==> cat reproduction.rs
…the file…

==> sqlx-oldapi 0.6.59 on DuckDB ODBC 1.5.5.0
Ok(42)

==> sqlx-oldapi 0.6.59 on PostgreSQL 17.12, psqlODBC 18.00.0005
Ok(42)

==> sqlx-odbc <commit> on DuckDB ODBC 1.5.5.0
Ok(42)

==> sqlx-odbc <commit> on PostgreSQL 17.12, psqlODBC 18.00.0005
Ok(42)
```

The runner starts a throwaway PostgreSQL server and an in-memory DuckDB database with the drivers pinned in the flake. The first run downloads and compiles both drivers, which takes a few minutes. Later runs reuse them from `~/.cache/sqlx-odbc-repro`.

The revision after the repository decides which sqlx-odbc runs, and each heading names that commit. `github:lovasoa/sqlx-odbc#repro` runs `main`, so running an old comment again shows whether the difference is fixed. To check a pull request, name its branch or commit, such as `github:lovasoa/sqlx-odbc/<commit>#repro`. If the commit exists only on a fork, use the fork's owner and name instead.

To run against a local checkout, use `nix run .#repro` with the same block, or pass a file: `nix run .#repro reproduction.rs`. Nix builds sqlx-odbc from the files Git tracks in the checkout, including uncommitted changes to them.

## Write one

Start the file with the `---cargo` header from the example above. Then write a `main` that reads its connection string from `DATABASE_URL` and prints what you want to compare. An error returned from `main` is printed too, so `?` is fine for steps that are not the point of the reproduction.

The runner points `sqlx` at [`sqlx-oldapi`](sqlx-oldapi) for the first runs and at [`sqlx-odbc`](sqlx-odbc) for the others. Both crates export the same names, so the file builds unchanged against either. Where the two sqlx versions differ, both crates add a helper with one name:

- `sqlx::connect_any` opens an `AnyConnection` from an ODBC connection string.
- `sqlx::sql` turns SQL text into what `Executor::prepare` takes.

If a reproduction needs a name that only one crate exports, add it to both.

Post the block, then the output inside `<details>`, so readers see the steps first.

## Without Nix

Cargo scripts still need nightly Cargo ([rust-lang/cargo#12207](https://github.com/rust-lang/cargo/issues/12207)), so the flake pins one. Without Nix, install the drivers yourself, then run the file once per driver crate from a checkout:

```sh
DATABASE_URL='Driver=DuckDB;Database=:memory:' cargo +nightly -Zscript \
    --config 'patch."https://github.com/lovasoa/sqlx-odbc".sqlx-repro.path="repro/sqlx-oldapi"' \
    reproduction.rs
```

The sqlx-oldapi features in [`sqlx-oldapi/Cargo.toml`](sqlx-oldapi/Cargo.toml) match SQLPage's, because they change how `Any` decodes values. Keep them in sync when SQLPage changes them.
