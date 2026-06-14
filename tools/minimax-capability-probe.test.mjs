import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_API_BASE,
  inferMimeType,
  isSuccessfulMiniMaxResponse,
  parseArgs,
  parseCsv,
  redactForLog,
} from "./minimax-capability-probe.mjs";

test("parseArgs reads command, flags, and positional values", () => {
  const parsed = parseArgs([
    "asr",
    "--audio",
    "sample.wav",
    "--asr-models",
    "speech-2.8-turbo,MiniMax-M3",
    "--print-response",
  ]);

  assert.equal(parsed.command, "asr");
  assert.equal(parsed.flags.audio, "sample.wav");
  assert.equal(parsed.flags.asrModels, "speech-2.8-turbo,MiniMax-M3");
  assert.equal(parsed.flags.printResponse, true);
});

test("parseCsv trims values and removes empty entries", () => {
  assert.deepEqual(parseCsv(" speech-2.8-turbo, ,MiniMax-M3 "), [
    "speech-2.8-turbo",
    "MiniMax-M3",
  ]);
});

test("redactForLog hides bearer tokens recursively", () => {
  assert.deepEqual(
    redactForLog({
      headers: {
        Authorization: "Bearer sk-live-secret",
        "Content-Type": "application/json",
      },
      nested: {
        api_key: "another-secret",
      },
    }),
    {
      headers: {
        Authorization: "<redacted>",
        "Content-Type": "application/json",
      },
      nested: {
        api_key: "<redacted>",
      },
    },
  );
});

test("inferMimeType maps common audio file extensions", () => {
  assert.equal(inferMimeType("demo.wav"), "audio/wav");
  assert.equal(inferMimeType("demo.mp3"), "audio/mpeg");
  assert.equal(inferMimeType("demo.m4a"), "audio/mp4");
  assert.equal(inferMimeType("demo.unknown"), "application/octet-stream");
});

test("token-plan probe defaults to China OpenAI-compatible chat endpoint", () => {
  assert.equal(DEFAULT_API_BASE, "https://api.minimaxi.com/v1");
});

test("isSuccessfulMiniMaxResponse treats non-zero base_resp as failure", () => {
  assert.equal(
    isSuccessfulMiniMaxResponse({
      ok: true,
      body: { base_resp: { status_code: 2049, status_msg: "invalid api key" } },
    }),
    false,
  );
  assert.equal(
    isSuccessfulMiniMaxResponse({
      ok: true,
      body: { base_resp: { status_code: 0, status_msg: "success" } },
    }),
    true,
  );
});
