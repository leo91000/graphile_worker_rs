# Graphile Worker Shutdown Signal

This crate provides cross-platform shutdown signal handling for [Graphile Worker](https://docs.rs/graphile_worker), enabling graceful shutdown on Unix (SIGINT, SIGTERM, etc.) and Windows (Ctrl+C, Ctrl+Close, etc.) systems.
