# Task-specific memory retrieval

YMP-106 uses the existing SQLite FTS5 index. It changes the automatic context assembled for an invocation; it does not add embeddings or change knowledge promotion policy.

## Query and scope

For task execution and review, the query is the current task title and description, bound to the task ID and attempt. A missing or stale task reference is rejected. A conversation follow-up uses the latest actual user message in that session. Planning and other session-wide work use the captured session goal, with the stored original request as a legacy fallback. Empty requests and disabled memory do not retrieve arbitrary recent entries.

The existing query policy quotes the first twelve whitespace-separated terms and joins them with `OR`. `memory_search_query` supplies the same escaped expression to FTS5 and the journal. Results retain the existing relevance order and project-or-global scope. YMP-113 additionally resolves active entries against current supported evidence; an explicit inspection mode can include unconfirmed candidates and legacy entries. Generic role instructions are not search terms.

At most five entries contribute to the prompt. Their combined text, including separators, is limited to 8,000 Unicode characters. This bounds newly added memory context for one invocation; it is not a token cap or a limit on the native conversation retained by a provider. FTS matching is lexical and may still return a weakly relevant entry when task terms overlap; no semantic relevance or cost reduction is claimed.

## Retrieval evidence

Before a provider is started, the invocation's `memory_retrieval` event records:

- Assignment, invocation and optional task IDs; whether memory was enabled.
- The source query text and the exact effective FTS expression.
- The character allowance and actual total included characters.
- Each included entry's ID, source session, project scope, status, and SHA-256 version of its serialized record.
- The digest and character count of the actual included excerpt, also referenced by the assignment context.

The event is available through `ymp trace SESSION_ID`. Entries retrieved from SQLite but omitted by the context allowance are not claimed as included sources. Record versions distinguish a later edit of an entry from the version that informed the invocation. An included source is context, not evidence that its claims are confirmed. YMP-113 now adds [incremental accumulation and supported-source retrieval](incremental-knowledge.md); YMP-114 governs correction. Automatic context requires current supported evidence. Explicit candidate/legacy inspection retains its unconfirmed or unknown label. Replacement retrieval policies record their actual candidate source and implementation; inventory selection does not claim an FTS query.

## Verification

The invoice control invokes the production prompt assembly with a generic role prefix and project, foreign-project, retired and irrelevant memories. Before the fix it selected the generic entry and failed the assertion with exit 101; after the fix it selects the invoice entry and checks the actual excerpt digest, source version and allowance.

The original legacy-entry fixtures now use explicit unconfirmed inspection mode to retain their query/scope/excerpt checks; YMP-113 separately exercises supported retrieval from actual confirmed result records. Additional runtime cases cover a later conversation request, quoted FTS input, global knowledge, the captured session goal, legacy fallback, stale attempts, and disabled retrieval. These use the internal mock provider and temporary stores; they do not consume native-provider inference or establish model-quality gains.
