# Fixture provenance

mmr.json, history.json and match.json are synthetic, trimmed examples following HenrikDev's official v3/v2/v4 schemas. They are NOT captured ISHQ#tejo2 account responses. Mock values use the current Ascendant rank schema (Diamond 1=18, Diamond 2=19, Diamond 3=20), correcting the illustrative older tier IDs in PLAN.md.

tiers.json is captured from https://valorant-api.com/v1/competitivetiers by scripts/assets.ps1. Use scripts/capture.mjs with a local HENRIK_API_KEY to capture actual account responses into the separate real/ directory.

Schema sources: https://docs.henrikdev.xyz/api-reference/valorant/get-mmr-history-by-puuid-v2.md and https://docs.henrikdev.xyz/api-reference/valorant/get-match-details-v4.md.
