async-graphql-extension-sentry
====

<div align="center">
  <!-- CI -->
  <img src="https://github.com/Miaxos/async_graphql_sentry_extension/actions/workflows/ci.yml/badge.svg" />
  <!-- Crates version -->
  <a href="https://crates.io/crates/async-graphql-extension-sentry">
    <img src="https://img.shields.io/crates/v/async-graphql-extension-sentry.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <!-- Documentation -->
  <a href="https://docs.rs/async-graphql-extension-sentry/">
    <img src="https://docs.rs/async-graphql-extension-sentry/badge.svg?style=flat-square"
      alt="Documentation" />
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/async-graphql-extension-sentry">
    <img src="https://img.shields.io/crates/d/async-graphql-extension-sentry.svg?style=flat-square"
      alt="Download" />
  </a>
</div>
<br />
<br />


async-graphql-extension-sentry is an open-source extension for the crates [async_graphql](https://github.com/async-graphql/async-graphql). The purpose of this extension is to provide a simple way to monitorr your requests through (Sentry)[https://sentry.io].

_Tested at Rust version: `rustc 1.56.0 (09c42c458 2021-10-18)`_

## Features

* Runtime agnostic (tokio / async-std)
* Fully support transaction breakdown

## Crate features

This crate offers the following features, all of which are not activated by default:

- `tokio-comp`: Enable the Tokio compatibility  when you have a tokio-runtime
- `async-std-comp`: Enable the async-std compatibility  when you have a async-std-runtime

## References

* [GraphQL](https://graphql.org)
* [Async Graphql Crates](https://github.com/async-graphql/async-graphql)
