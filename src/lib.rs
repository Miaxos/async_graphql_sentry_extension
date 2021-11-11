//! # Sentry Extensions for async_graphql
//!
//!  <div align="center">
//!  <!-- CI -->
//!  <img src="https://github.com/Miaxos/async_graphql_sentry_extension/actions/workflows/ci.yml/badge.svg" />
//!  <!-- Crates version -->
//!  <a href="https://crates.io/crates/async-graphql-extension-sentry">
//!    <img src="https://img.shields.io/crates/v/async-graphql-extension-sentry.svg?style=flat-square"
//!    alt="Crates.io version" />
//!  </a>
//!  <!-- Downloads -->
//!  <a href="https://crates.io/crates/async-graphql-extension-sentry">
//!    <img src="https://img.shields.io/crates/d/async-graphql-extension-sentry.svg?style=flat-square"
//!      alt="Download" />
//!  </a>
//! </div>
//!
//! TODO:
//!   - Sentry trace header.
//!   - Additional data.
//!
mod runtime;

#[macro_use]
extern crate tracing;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use chrono::Utc;
use sentry::{Envelope, Hub};

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextParseQuery, NextResolve, ResolveInfo,
};

use async_graphql::parser::types::ExecutableDocument;
use async_graphql::{Response, ServerResult, Value, Variables};
use runtime::RwLock;
use sentry::protocol::{ClientSdkInfo, Span, SpanId, SpanStatus, TraceContext, Transaction};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sentry Extension to send transaction to Sentry
/// The extension to include to you `async_graphql` instance to connect with Sentry.
///
/// Sentry works by creating traces from GraphQL calls, which contains extra data about the
/// request being processed. These traces are then sent to Sentry.
///
/// To add additional data to your metrics, you should add a ApolloTracingDataExt to your
/// query_data when you process a query with async_graphql.
pub struct SentryExtensionFactory {
    sdk: Arc<ClientSdkInfo>,
}

impl SentryExtensionFactory {
    pub fn new() -> Self {
        SentryExtensionFactory {
            sdk: Arc::new(ClientSdkInfo {
                name: "async_graphql".to_string(),
                version: VERSION.to_string(),
                packages: vec![],
                integrations: vec![],
            }),
        }
    }
}

/// The structure where you can add additional context for Sentry.
/// This structure must be added to your query data.
#[derive(Debug, Clone)]
pub struct SentryAdditionalData {}

impl Default for SentryAdditionalData {
    fn default() -> Self {
        SentryAdditionalData {}
    }
}

impl ExtensionFactory for SentryExtensionFactory {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(SentryExtension {
            transaction: RwLock::new(Transaction::new()),
            sdk: self.sdk.clone(),
            trace: RwLock::new(TraceContext::default()),
            nodes: RwLock::new(HashMap::new()),
            query: RwLock::new("".to_string()),
        })
    }
}

struct SentryExtension {
    transaction: RwLock<Transaction<'static>>,
    sdk: Arc<ClientSdkInfo>,
    /// Actual trace
    trace: RwLock<TraceContext>,
    /// Resolvers spans
    nodes: RwLock<HashMap<String, Span>>,
    query: RwLock<String>,
}

#[async_graphql::async_trait::async_trait]
impl Extension for SentryExtension {
    async fn request(
        &self,
        ctx: &ExtensionContext<'_>,
        next: async_graphql::extensions::NextRequest<'_>,
    ) -> Response {
        self.transaction.write().await.platform = Cow::Borrowed("async_graphql");
        self.transaction.write().await.sdk = Some(Cow::Owned((*self.sdk).clone()));
        {
            let trace = self.trace.read().await;
            let trace_id = trace.trace_id;
            let span_id = trace.span_id;
            self.nodes.write().await.insert(
                "".to_string(),
                Span {
                    trace_id,
                    span_id,
                    parent_span_id: None,
                    same_process_as_parent: None,
                    op: Some("request".to_string()),
                    description: None,
                    timestamp: None,
                    start_timestamp: Utc::now(),
                    status: None,
                    tags: BTreeMap::new(),
                    data: BTreeMap::new(),
                },
            );
        }

        let result = next.run(ctx).await;

        let mut trace = self.trace.write().await;
        let mut trace = std::mem::take(&mut *trace);
        match result.is_err() {
            true => trace.status = Some(SpanStatus::UnknownError),
            false => trace.status = Some(SpanStatus::Ok),
        };

        let mut transaction = self.transaction.write().await;
        let mut transaction = std::mem::take(&mut *transaction);
        transaction
            .contexts
            .insert("trace".to_string(), trace.into());

        if let Some(mut parent_span) = self.nodes.write().await.get_mut(&"".to_string()) {
            parent_span.data.insert(
                "query".to_string(),
                serde_json::json!(*self.query.read().await),
            );
            if result.is_err() {
                parent_span.status = Some(SpanStatus::UnknownError);
            };
            parent_span.finish();
        };

        let mut nodes = self.nodes.write().await;
        let nodes = std::mem::take(&mut *nodes);
        nodes.into_values().for_each(|x| transaction.spans.push(x));
        transaction.finish();

        // To Send the transaction
        Hub::with_active(move |hub| {
            let client = match hub.client() {
                Some(client) => client,
                None => {
                    error!("client not found");
                    return;
                }
            };

            // Only available on master
            /*
            if !client.sample_should_send() {
                return;
            }
            */

            let envelope = Envelope::from(transaction);
            client.send_envelope(envelope);
        });
        result
    }

    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: async_graphql::Request,
        next: async_graphql::extensions::NextPrepareRequest<'_>,
    ) -> ServerResult<async_graphql::Request> {
        let mut span = Span::default();
        self.transaction.write().await.name = request.operation_name.clone();
        (*self.trace.write().await).op = request.operation_name.clone();
        span.parent_span_id = self.nodes.read().await.get("").map(|x| x.span_id);
        span.op = Some("prepare_request".to_string());
        let res = next.run(ctx, request).await;
        span.finish();
        self.transaction.write().await.spans.push(span);
        res
    }

    async fn validation(
        &self,
        ctx: &ExtensionContext<'_>,
        next: async_graphql::extensions::NextValidation<'_>,
    ) -> async_graphql::Result<async_graphql::ValidationResult, Vec<async_graphql::ServerError>>
    {
        let mut span = Span::default();
        span.parent_span_id = self.nodes.read().await.get("").map(|x| x.span_id);
        span.op = Some("validation".to_string());
        let res = next.run(ctx).await;
        span.finish();
        self.transaction.write().await.spans.push(span);
        res
    }

    async fn parse_query(
        &self,
        ctx: &ExtensionContext<'_>,
        query: &str,
        variables: &Variables,
        next: NextParseQuery<'_>,
    ) -> ServerResult<ExecutableDocument> {
        let mut span = Span::default();
        *self.query.write().await = query.to_string();
        span.parent_span_id = self.nodes.read().await.get("").map(|x| x.span_id);
        span.op = Some("parse_query".to_string());
        let document = match next.run(ctx, query, variables).await {
            Ok(document) => {
                span.status = Some(SpanStatus::Ok);
                document
            }
            Err(err) => {
                span.status = Some(SpanStatus::UnknownError);
                span.data
                    .insert("error".to_string(), serde_json::json!(err.message));
                span.data
                    .insert("path".to_string(), serde_json::json!(err.path));
                span.finish();
                self.transaction.write().await.spans.push(span);
                return Err(err);
            }
        };
        span.finish();
        self.transaction.write().await.spans.push(span);
        Ok(document)
    }

    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        // We do create a node when it's invoked which we insert at the right place inside the
        // struct.

        let start_time = Utc::now();

        let path = info.path_node.to_string_vec().join(".");
        let path_node = info.path_node;

        // If ParentNode = None -> Parent.
        let parent_node = path_node.parent.map(|x| x.to_string_vec().join("."));
        let parent_span = self
            .nodes
            .read()
            .await
            .get(&parent_node.unwrap_or("".to_string()))
            .map(|x| x.span_id);

        let mut span = Span {
            timestamp: None,
            trace_id: self.trace.read().await.trace_id,
            data: BTreeMap::new(),
            op: Some(format!("resolve-{}", path)),
            tags: BTreeMap::new(),
            status: Some(SpanStatus::Ok),
            span_id: SpanId::default(),
            parent_span_id: parent_span,
            description: None,
            start_timestamp: start_time,
            same_process_as_parent: None,
        };

        let res = match next.run(ctx, info).await {
            Ok(res) => Ok(res),
            Err(e) => {
                span.status = Some(SpanStatus::UnknownError);
                span.data
                    .insert("error".to_string(), serde_json::json!(e.message));
                span.data
                    .insert("path".to_string(), serde_json::json!(e.path));
                Err(e)
            }
        };

        span.finish();
        self.nodes.write().await.insert(path, span);
        res
    }
}
