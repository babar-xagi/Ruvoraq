# Ruvoraq database adapters

SQLx-based SQLite and PostgreSQL adapters with shared connection pools,
parameterized queries, transactions and checked forward-only migrations.

```toml
# SQLite is the default adapter feature.
ruvoraq-db = "0.1.0"

# For PostgreSQL alone, use instead:
# ruvoraq-db = { version = "0.1.0", default-features = false, features = ["postgres"] }
```

`Database` is the SQLite adapter; `PostgresDatabase` is available with the
`postgres` feature. Both expose `connect`, `pool`, `begin`, `close`, `migrate`
and `migration_status`. The PostgreSQL adapter does not provision databases.

Migration files use `<positive_version>_<lowercase_description>.sql`.
Applied versions/checksums are validated before changes; each file is
transactional. PostgreSQL migrations use advisory locks. SQLite requires a
single migrator at a time. There is no ORM or reversible migration system.

This is an experimental 0.1.0 release. The framework is developed in verified
increments and is not yet a complete production platform.

Documentation: [user guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/user_guid.md),
[developer guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/developer_guid.md),
[testing guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/testing_guid.md).

Licensed under Apache-2.0. Source: [Ruvoraq](https://github.com/babar-xagi/Ruvoraq).
