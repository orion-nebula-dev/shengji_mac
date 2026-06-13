#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_API_BASE = "https://api.minimax.io/v1";
const DEFAULT_DOCS_INDEX = "https://platform.minimax.io/docs/llms.txt";
const DEFAULT_OPENAPI_URL =
  "https://platform.minimax.io/docs/zh/api-reference/openapi.json";

const COMMANDS = new Set(["docs", "models", "chat", "tts", "asr", "all", "help"]);

export function parseArgs(argv) {
  const args = [...argv];
  let command = "all";
  const flags = {};
  const positional = [];

  if (args[0] && !args[0].startsWith("-") && COMMANDS.has(args[0])) {
    command = args.shift();
  }

  for (let index = 0; index < args.length; index += 1) {
    const item = args[index];

    if (!item.startsWith("--")) {
      positional.push(item);
      continue;
    }

    const rawName = item.slice(2);
    const equalIndex = rawName.indexOf("=");
    const name = toCamelCase(equalIndex >= 0 ? rawName.slice(0, equalIndex) : rawName);

    if (equalIndex >= 0) {
      flags[name] = rawName.slice(equalIndex + 1);
      continue;
    }

    const next = args[index + 1];
    if (!next || next.startsWith("--")) {
      flags[name] = true;
      continue;
    }

    flags[name] = next;
    index += 1;
  }

  return { command, flags, positional };
}

export function parseCsv(value) {
  if (!value) return [];
  return String(value)
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function inferMimeType(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  switch (extension) {
    case ".wav":
      return "audio/wav";
    case ".mp3":
      return "audio/mpeg";
    case ".m4a":
      return "audio/mp4";
    case ".flac":
      return "audio/flac";
    case ".ogg":
      return "audio/ogg";
    case ".webm":
      return "audio/webm";
    default:
      return "application/octet-stream";
  }
}

export function redactForLog(value) {
  if (Array.isArray(value)) {
    return value.map((item) => redactForLog(item));
  }

  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [
        key,
        isSecretKey(key) ? "<redacted>" : redactForLog(item),
      ]),
    );
  }

  if (typeof value === "string" && /^Bearer\s+/i.test(value)) {
    return "<redacted>";
  }

  return value;
}

async function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.command === "help" || parsed.flags.help) {
    printHelp();
    return;
  }

  const context = {
    apiKey: process.env.MINIMAX_API_KEY || "",
    apiBase: parsed.flags.apiBase || process.env.MINIMAX_API_BASE || DEFAULT_API_BASE,
    docsIndex: parsed.flags.docsIndex || DEFAULT_DOCS_INDEX,
    openapiUrl: parsed.flags.openapiUrl || DEFAULT_OPENAPI_URL,
    printResponse: Boolean(parsed.flags.printResponse),
    json: Boolean(parsed.flags.json),
    flags: parsed.flags,
  };

  const results = [];

  if (parsed.command === "docs" || parsed.command === "all") {
    results.push(await probeDocs(context));
  }

  if (parsed.command === "models" || parsed.command === "all") {
    results.push(await probeModels(context));
  }

  if (parsed.command === "chat" || parsed.command === "all") {
    results.push(await probeChat(context));
  }

  if (parsed.command === "tts") {
    results.push(await probeTts(context));
  }

  if (parsed.command === "asr") {
    results.push(await probeAsr(context));
  }

  if (parsed.command === "all" && parsed.flags.audio) {
    results.push(await probeAsr(context));
  }

  if (parsed.command === "all" && parsed.flags.tts) {
    results.push(await probeTts(context));
  }

  if (context.json) {
    console.log(JSON.stringify(redactForLog(results), null, 2));
    return;
  }

  for (const result of results) {
    printResult(result, context.printResponse);
  }
}

async function probeDocs(context) {
  const result = makeResult("docs", "MiniMax official documentation index");

  try {
    const [indexText, openapi] = await Promise.all([
      fetchText(context.docsIndex),
      fetchJson(context.openapiUrl),
    ]);

    const docLines = indexText
      .split("\n")
      .filter((line) => /asr|stt|speech-to-text|transcri/i.test(line));
    const paths = Object.keys(openapi.paths || {});
    const audioRelatedPaths = paths.filter((item) =>
      /audio|speech|transcription|asr|t2a|voice|file|model/i.test(item),
    );

    result.ok = true;
    result.summary = {
      docsIndex: context.docsIndex,
      openapiUrl: context.openapiUrl,
      hasOfficialAsrDocHit: docLines.length > 0,
      asrDocHits: docLines,
      audioRelatedPaths,
      conclusion:
        docLines.length > 0 || audioRelatedPaths.some((item) => /transcription|asr/i.test(item))
          ? "Found possible ASR documentation references. Inspect manually before relying on them."
          : "No official ASR/STT documentation page or OpenAPI path was found. Treat ASR as unconfirmed.",
    };
  } catch (error) {
    result.error = normalizeError(error);
  }

  return result;
}

async function probeModels(context) {
  const result = makeResult("models", "GET /v1/models");

  if (!context.apiKey) {
    result.skipped = true;
    result.error = "Set MINIMAX_API_KEY to call the live API.";
    return result;
  }

  try {
    const response = await requestJson(`${trimSlash(context.apiBase)}/models`, {
      headers: authHeaders(context.apiKey),
    });
    result.ok = response.ok;
    result.status = response.status;
    result.summary = summarizeModels(response.body);
    result.response = response.body;
  } catch (error) {
    result.error = normalizeError(error);
  }

  return result;
}

async function probeChat(context) {
  const model = context.flags.model || "MiniMax-M3";
  const result = makeResult("chat", `POST /v1/chat/completions with ${model}`);

  if (!context.apiKey) {
    result.skipped = true;
    result.error = "Set MINIMAX_API_KEY to call the live API.";
    return result;
  }

  try {
    const response = await requestJson(`${trimSlash(context.apiBase)}/chat/completions`, {
      method: "POST",
      headers: {
        ...authHeaders(context.apiKey),
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model,
        messages: [
          {
            role: "user",
            content:
              "Return exactly this JSON object and nothing else: {\"ok\":true,\"task\":\"semantic_validation\"}",
          },
        ],
        temperature: 0,
      }),
    });

    result.ok = response.ok;
    result.status = response.status;
    result.summary = {
      model,
      content: extractChatContent(response.body),
      usage: response.body?.usage,
    };
    result.response = response.body;
  } catch (error) {
    result.error = normalizeError(error);
  }

  return result;
}

async function probeTts(context) {
  const model = context.flags.ttsModel || "speech-2.8-turbo";
  const voiceId = context.flags.voiceId || "English_expressive_narrator";
  const result = makeResult("tts", `POST /v1/t2a_v2 with ${model}`);

  if (!context.apiKey) {
    result.skipped = true;
    result.error = "Set MINIMAX_API_KEY to call the live API.";
    return result;
  }

  try {
    const response = await requestJson(`${trimSlash(context.apiBase)}/t2a_v2`, {
      method: "POST",
      headers: {
        ...authHeaders(context.apiKey),
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model,
        text: context.flags.text || "MiniMax speech capability validation.",
        stream: false,
        language_boost: "auto",
        output_format: "url",
        voice_setting: {
          voice_id: voiceId,
          speed: 1,
          vol: 1,
          pitch: 0,
        },
        audio_setting: {
          sample_rate: 32000,
          bitrate: 128000,
          format: "mp3",
          channel: 1,
        },
      }),
    });

    result.ok = response.ok;
    result.status = response.status;
    result.summary = {
      model,
      voiceId,
      baseResp: response.body?.base_resp,
      extraInfo: response.body?.extra_info,
      hasAudio: Boolean(response.body?.data?.audio),
    };
    result.response = response.body;
  } catch (error) {
    result.error = normalizeError(error);
  }

  return result;
}

async function probeAsr(context) {
  const audioPath = context.flags.audio;
  const endpoint =
    context.flags.asrEndpoint || `${trimSlash(context.apiBase)}/audio/transcriptions`;
  const models = parseCsv(context.flags.asrModels || "speech-2.8-turbo,speech-2.8-hd,MiniMax-M3");
  const result = makeResult("asr", `POST ${endpoint}`);

  if (!context.apiKey) {
    result.skipped = true;
    result.error = "Set MINIMAX_API_KEY to call the live API.";
    return result;
  }

  if (!audioPath) {
    result.skipped = true;
    result.error = "Pass --audio /path/to/sample.wav to probe ASR.";
    return result;
  }

  try {
    const fileBytes = await readFile(audioPath);
    const mimeType = context.flags.mimeType || inferMimeType(audioPath);
    result.summary = {
      endpoint,
      audioPath,
      mimeType,
      candidateModels: models,
      note: "This endpoint is not present in MiniMax official OpenAPI docs as of this probe. A 404/405 means ASR is not exposed through this path for this key/region.",
    };

    result.attempts = [];
    for (const model of models) {
      const form = new FormData();
      form.append("file", new Blob([fileBytes], { type: mimeType }), path.basename(audioPath));
      form.append("model", model);
      form.append("response_format", context.flags.responseFormat || "json");

      const startedAt = Date.now();
      const response = await requestJson(endpoint, {
        method: "POST",
        headers: authHeaders(context.apiKey),
        body: form,
      });

      const attempt = {
        model,
        status: response.status,
        ok: response.ok,
        durationMs: Date.now() - startedAt,
        summary: summarizeAsrResponse(response.body),
        response: response.body,
      };
      result.attempts.push(attempt);

      if (response.ok) {
        result.ok = true;
        result.status = response.status;
        break;
      }
    }
  } catch (error) {
    result.error = normalizeError(error);
  }

  return result;
}

async function fetchText(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`GET ${url} failed with ${response.status}`);
  }
  return response.text();
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`GET ${url} failed with ${response.status}`);
  }
  return response.json();
}

async function requestJson(url, init = {}) {
  const response = await fetch(url, init);
  const text = await response.text();
  return {
    ok: response.ok,
    status: response.status,
    body: parseResponseBody(text),
  };
}

function parseResponseBody(text) {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text.slice(0, 4000) };
  }
}

function makeResult(name, title) {
  return {
    name,
    title,
    ok: false,
    skipped: false,
  };
}

function authHeaders(apiKey) {
  return {
    Authorization: `Bearer ${apiKey}`,
  };
}

function summarizeModels(body) {
  const data = Array.isArray(body?.data) ? body.data : [];
  return {
    count: data.length,
    ids: data.map((item) => item.id).filter(Boolean),
  };
}

function extractChatContent(body) {
  return body?.choices?.[0]?.message?.content ?? body?.choices?.[0]?.text ?? null;
}

function summarizeAsrResponse(body) {
  if (!body) return null;
  return {
    text: body.text || body.transcript || body.result?.text || null,
    hasSegments: Array.isArray(body.segments) || Array.isArray(body.result?.segments),
    error: body.error || body.base_resp || body.message || null,
  };
}

function printResult(result, printResponse) {
  console.log(`\n## ${result.title}`);
  console.log(`status: ${result.skipped ? "skipped" : result.ok ? "ok" : "failed"}`);

  if (result.status) {
    console.log(`http: ${result.status}`);
  }

  if (result.error) {
    console.log(`error: ${result.error}`);
  }

  if (result.summary) {
    console.log("summary:");
    console.log(JSON.stringify(redactForLog(result.summary), null, 2));
  }

  if (result.attempts) {
    console.log("attempts:");
    console.log(JSON.stringify(redactForLog(result.attempts.map(stripLargeResponse)), null, 2));
  }

  if (printResponse && result.response) {
    console.log("response:");
    console.log(JSON.stringify(redactForLog(result.response), null, 2));
  }
}

function stripLargeResponse(attempt) {
  if (attempt.ok) return attempt;
  return {
    model: attempt.model,
    status: attempt.status,
    ok: attempt.ok,
    durationMs: attempt.durationMs,
    summary: attempt.summary,
  };
}

function printHelp() {
  console.log(`
MiniMax capability probe

Usage:
  npm run probe:minimax -- docs
  MINIMAX_API_KEY=... npm run probe:minimax -- models
  MINIMAX_API_KEY=... npm run probe:minimax -- chat --model MiniMax-M3
  MINIMAX_API_KEY=... npm run probe:minimax -- tts --tts-model speech-2.8-turbo
  MINIMAX_API_KEY=... npm run probe:minimax -- asr --audio ./sample.wav

Commands:
  docs    Check official docs index and OpenAPI paths for ASR/STT evidence.
  models  Call GET /v1/models.
  chat    Validate MiniMax-M3 semantic/chat access.
  tts     Validate Speech 2.x text-to-speech. This may incur a small cost.
  asr     Probe /v1/audio/transcriptions. This is not official docs-confirmed.
  all     Run docs + models + chat. Add --audio to also probe ASR.

Environment:
  MINIMAX_API_KEY      Required for live API calls.
  MINIMAX_API_BASE     Optional, defaults to ${DEFAULT_API_BASE}.

Useful flags:
  --json
  --print-response
  --audio ./sample.wav
  --asr-models speech-2.8-turbo,speech-2.8-hd,MiniMax-M3
  --asr-endpoint https://api.minimax.io/v1/audio/transcriptions
  --model MiniMax-M3
  --tts-model speech-2.8-turbo
  --voice-id English_expressive_narrator
`);
}

function normalizeError(error) {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function trimSlash(value) {
  return String(value).replace(/\/+$/, "");
}

function toCamelCase(value) {
  return value.replace(/-([a-z])/g, (_, char) => char.toUpperCase());
}

function isSecretKey(key) {
  return /authorization|api[_-]?key|token|secret/i.test(key);
}

const isCli = process.argv[1]
  ? fileURLToPath(import.meta.url) === path.resolve(process.argv[1])
  : false;

if (isCli) {
  main().catch((error) => {
    console.error(normalizeError(error));
    process.exitCode = 1;
  });
}
