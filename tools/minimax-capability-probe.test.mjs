import assert from "node:assert/strict";
import test from "node:test";

import {
  inferMimeType,
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
