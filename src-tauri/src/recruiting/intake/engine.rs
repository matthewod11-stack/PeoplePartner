//! Graph-based conversation engine. Ported from
//! `~/Projects/Sourcerer/packages/intake/src/conversation-engine.ts`.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use super::context::{append_message, merge_context_updates, IntakeContext};
use super::deps::{IntakeDeps, IntakeError};
use super::node::{IntakeNode, NodeId, ParsedResponse, StepResult};
use super::schemas::{Message, MessageRole};

pub type ConversationGraph = HashMap<NodeId, Box<dyn IntakeNode>>;

pub fn build_graph(nodes: Vec<Box<dyn IntakeNode>>) -> ConversationGraph {
    let mut g = HashMap::new();
    for node in nodes {
        let id = node.id();
        if g.insert(id, node).is_some() {
            panic!("duplicate node id in intake graph: {id:?}");
        }
    }
    g
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationState {
    pub current_node_id: NodeId,
    pub context: IntakeContext,
    pub completed_nodes: Vec<NodeId>,
    pub saved_at: String,
    pub version: u32,
}

pub struct ConversationEngine {
    graph: ConversationGraph,
    current: NodeId,
    context: IntakeContext,
    completed: HashSet<NodeId>,
}

impl ConversationEngine {
    pub fn new(graph: ConversationGraph, start: NodeId, initial: Option<IntakeContext>)
        -> Result<Self, IntakeError> {
        if start != NodeId::Done && !graph.contains_key(&start) {
            return Err(IntakeError::UnknownNode(start));
        }
        Ok(Self { graph, current: start, context: initial.unwrap_or_default(), completed: HashSet::new() })
    }

    pub fn is_done(&self) -> bool { self.current == NodeId::Done }
    pub fn context(&self) -> &IntakeContext { &self.context }
    pub fn completed_nodes(&self) -> Vec<NodeId> { self.completed.iter().copied().collect() }

    /// Resolves the skip_if chain (guarded against infinite loops), then
    /// returns the current node's prompt — or None if done.
    pub async fn get_prompt(&mut self) -> Option<String> {
        self.skip_eligible_nodes();
        if self.is_done() { return None; }
        let node = self.graph.get(&self.current)?;
        Some(node.prompt(&self.context).await)
    }

    pub async fn submit_response(&mut self, response: &str, deps: &IntakeDeps)
        -> Result<StepResult, IntakeError> {
        if self.is_done() { return Err(IntakeError::SubmitAfterDone); }

        // Record the user message first.
        self.context = append_message(
            std::mem::take(&mut self.context),
            Message { role: MessageRole::User, content: response.to_string() },
        );

        // Parse via the current node, then advance. We re-fetch the node from
        // the graph for each use to avoid holding a borrow across the context
        // mutations.
        let parsed = {
            let node = self.graph.get(&self.current).ok_or(IntakeError::UnknownNode(self.current))?;
            node.parse(response, &self.context, deps).await?
        };
        self.context = merge_context_updates(std::mem::take(&mut self.context), parsed.context_updates.clone());
        self.completed.insert(self.current);
        let next = {
            let node = self.graph.get(&self.current).ok_or(IntakeError::UnknownNode(self.current))?;
            node.next(&parsed, &self.context)
        };
        self.current = next;
        Ok(StepResult { parsed, next, done: next == NodeId::Done })
    }

    pub fn save_state(&self, now: String) -> ConversationState {
        // Sort for a deterministic serialized blob (the set's iteration order
        // is not stable across runs). Restore re-collects into a HashSet, so
        // ordering is cosmetic — but stable output enables snapshot testing.
        let mut completed_nodes: Vec<NodeId> = self.completed.iter().copied().collect();
        completed_nodes.sort();
        ConversationState {
            current_node_id: self.current,
            context: self.context.clone(),
            completed_nodes,
            saved_at: now,
            version: 1,
        }
    }

    /// Serializes with a fixed timestamp ("savedAt" is not load-bearing for
    /// resume). Callers needing a real time pass it via `save_state`.
    pub fn serialize_state(&self) -> String {
        serde_json::to_string_pretty(&self.save_state("1970-01-01T00:00:00Z".into()))
            .expect("ConversationState is serializable")
    }

    fn skip_eligible_nodes(&mut self) {
        let mut visited: HashSet<NodeId> = HashSet::new();
        while !self.is_done() {
            if !visited.insert(self.current) { break; }
            let Some(node) = self.graph.get(&self.current) else { break };
            if node.skip_if(&self.context) {
                self.completed.insert(self.current);
                let empty = ParsedResponse::default();
                let next = node.next(&empty, &self.context);
                self.current = next;
            } else {
                break;
            }
        }
    }
}

pub fn restore_conversation(graph: ConversationGraph, state_json: &str)
    -> Result<ConversationEngine, IntakeError> {
    let state: ConversationState = serde_json::from_str(state_json)
        .map_err(|e| IntakeError::Deserialize(e.to_string()))?;
    if state.version != 1 {
        return Err(IntakeError::UnsupportedStateVersion(state.version));
    }
    let mut engine = ConversationEngine::new(graph, state.current_node_id, Some(state.context))?;
    engine.completed = state.completed_nodes.into_iter().collect();
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::context::IntakeContext;
    use crate::recruiting::intake::deps::testing::{FakeProvider, FakeResearch, fake_deps};
    use crate::recruiting::intake::node::{IntakeNode, NodeId, ParsedResponse, Phase};
    use async_trait::async_trait;

    // Stub: A -> B -> Done, B skips if company_url is set.
    struct StubA;
    #[async_trait]
    impl IntakeNode for StubA {
        fn id(&self) -> NodeId { NodeId::RoleJdInput }
        fn phase(&self) -> Phase { Phase::Role }
        async fn prompt(&self, _c: &IntakeContext) -> String { "A?".into() }
        async fn parse(&self, _r: &str, _c: &IntakeContext, _d: &crate::recruiting::intake::deps::IntakeDeps)
            -> Result<ParsedResponse, crate::recruiting::intake::deps::IntakeError> {
            Ok(ParsedResponse::default())
        }
        fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::RoleParseConfirm }
    }
    struct StubB;
    #[async_trait]
    impl IntakeNode for StubB {
        fn id(&self) -> NodeId { NodeId::RoleParseConfirm }
        fn phase(&self) -> Phase { Phase::Role }
        async fn prompt(&self, _c: &IntakeContext) -> String { "B?".into() }
        async fn parse(&self, _r: &str, _c: &IntakeContext, _d: &crate::recruiting::intake::deps::IntakeDeps)
            -> Result<ParsedResponse, crate::recruiting::intake::deps::IntakeError> {
            Ok(ParsedResponse::default())
        }
        fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::Done }
        fn skip_if(&self, c: &IntakeContext) -> bool { c.company_url.is_some() }
    }

    fn graph() -> Vec<Box<dyn IntakeNode>> { vec![Box::new(StubA), Box::new(StubB)] }

    #[tokio::test]
    async fn walks_to_done() {
        let mut e = ConversationEngine::new(build_graph(graph()), NodeId::RoleJdInput, None).unwrap();
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        assert_eq!(e.get_prompt().await.unwrap(), "A?");
        let s = e.submit_response("x", &deps).await.unwrap();
        assert_eq!(s.next, NodeId::RoleParseConfirm);
        assert_eq!(e.get_prompt().await.unwrap(), "B?");
        let s = e.submit_response("y", &deps).await.unwrap();
        assert!(s.done);
        assert!(e.is_done());
    }

    #[tokio::test]
    async fn skip_if_advances_past_b() {
        let ctx = IntakeContext { company_url: Some("u".into()), ..Default::default() };
        let mut e = ConversationEngine::new(build_graph(graph()), NodeId::RoleParseConfirm, Some(ctx)).unwrap();
        assert_eq!(e.get_prompt().await, None);
        assert!(e.is_done());
    }

    #[tokio::test]
    async fn submit_after_done_errors() {
        let mut e = ConversationEngine::new(build_graph(graph()), NodeId::Done, None).unwrap();
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        assert!(e.submit_response("x", &deps).await.is_err());
    }

    #[tokio::test]
    async fn save_and_restore_round_trips() {
        let g = || build_graph(graph());
        let mut e = ConversationEngine::new(g(), NodeId::RoleJdInput, None).unwrap();
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        e.submit_response("x", &deps).await.unwrap();
        let json = e.serialize_state();
        let mut restored = restore_conversation(g(), &json).unwrap();
        assert_eq!(restored.get_prompt().await.unwrap(), "B?");
    }
}
