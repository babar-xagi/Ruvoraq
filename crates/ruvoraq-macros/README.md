# Ruvoraq macros

Route attributes (`get`, `post`, `put`, `patch`, `delete`), schema metadata and
application bootstrap for the Ruvoraq framework. These procedural macros are
normally used through `ruvoraq::prelude::*`, not directly.

Attributed async handlers register automatically. The bootstrap macro loads
route-only `main.rs` from the application's settings entry point and supports
synchronous or asynchronous configure hooks. Models annotated with `schema`
contribute request/response metadata to OpenAPI.

This is an experimental 0.1.0 release. The framework is developed in verified
increments and is not yet a complete production platform.

Documentation: [user guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/user_guid.md),
[developer guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/developer_guid.md),
[testing guide](https://github.com/babar-xagi/Ruvoraq/blob/main/doc/testing_guid.md).

Licensed under Apache-2.0. Source: [Ruvoraq](https://github.com/babar-xagi/Ruvoraq).
