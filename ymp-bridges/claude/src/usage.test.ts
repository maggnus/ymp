import { test } from "node:test";
import assert from "node:assert/strict";
import { UsageTracker } from "./usage.js";

test("stream counters are cumulative per API response and repeated blocks do not add", () => {
  const tracker = new UsageTracker();
  const start = {type:"stream_event",session_id:"s",event:{type:"message_start",message:{id:"m1",usage:{input_tokens:10,cache_read_input_tokens:80,cache_creation_input_tokens:10,output_tokens:1}}}};
  const first=tracker.ingest(start)!;
  assert.equal(first.counts.input,100);
  assert.equal(first.counts.output,null);
  const delta=(n:number)=>({type:"stream_event",session_id:"s",event:{type:"message_delta",usage:{output_tokens:n}}});
  tracker.ingest(delta(12));tracker.ingest(delta(20));
  const repeated=tracker.ingest({type:"assistant",message:{id:"m1",usage:{input_tokens:10,cache_read_input_tokens:80,cache_creation_input_tokens:10,output_tokens:1}}})!;
  assert.equal(repeated.counts.input,100);assert.equal(repeated.counts.output,20);
  const final=tracker.ingest({type:"result",modelUsage:{main:{inputTokens:20,outputTokens:30,cacheReadInputTokens:100,cacheCreationInputTokens:10}}})!;
  assert.equal(final.counts.input,130);assert.equal(final.counts.output,30);assert.equal(final.partial,false);
});

test("parallel streams and final whole-query totals replace partial counts", () => {
  const tracker=new UsageTracker();
  for (const [lane,id] of [["a","m1"],["b","m2"]]) {
    tracker.ingest({type:"stream_event",session_id:"s",parent_tool_use_id:lane,event:{type:"message_start",message:{id,usage:{input_tokens:5}}}});
    tracker.ingest({type:"stream_event",session_id:"s",parent_tool_use_id:lane,event:{type:"message_delta",usage:{output_tokens:8}}});
  }
  const final=tracker.ingest({type:"result",modelUsage:{main:{inputTokens:20,outputTokens:30,cacheReadInputTokens:0,cacheCreationInputTokens:0},child:{inputTokens:10,outputTokens:10,cacheReadInputTokens:0,cacheCreationInputTokens:0}}})!;
  assert.equal(final.counts.input,30);assert.equal(final.counts.output,40);
});

test("a zeroed crash result does not erase observed usage", () => {
  const tracker=new UsageTracker();
  tracker.ingest({type:"assistant",message:{id:"m1",usage:{input_tokens:50,cache_read_input_tokens:100,output_tokens:1}}});
  const final=tracker.ingest({type:"result",subtype:"error_during_execution",modelUsage:{},usage:{input_tokens:0,output_tokens:0}})!;
  assert.equal(final.counts.input,150);assert.equal(final.counts.output,null);assert(final.partial);
});

test("a missing counter in one model is partial even if another model reports it", () => {
  const tracker = new UsageTracker();
  const final = tracker.ingest({type:"result",modelUsage:{main:{inputTokens:20,outputTokens:30},child:{inputTokens:10}}})!;
  assert.equal(final.counts.input,30);
  assert.equal(final.counts.output,30);
  assert(final.partial);
});
