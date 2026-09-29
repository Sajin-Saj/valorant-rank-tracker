import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parseEnv } from 'node:util';
const root = fileURLToPath(new URL('../', import.meta.url));
const vars = parseEnv(await readFile(`${root}.dev.vars`, 'utf8'));
if (!vars.HENRIK_API_KEY) throw new Error('Configure HENRIK_API_KEY in .dev.vars');
// Account comes from wrangler.toml [vars]; .dev.vars can override it, like the Worker.
const toml = await readFile(`${root}wrangler.toml`, 'utf8');
const setting = key => vars[key] ?? toml.match(new RegExp(String.raw`^${key}\s*=\s*"([^"]*)"`, 'm'))?.[1];
const [name, tag, region, platform] = ['RIOT_NAME', 'RIOT_TAG', 'REGION', 'PLATFORM'].map(setting);
if (!name || !tag || !region || !platform) throw new Error('Set RIOT_NAME, RIOT_TAG, REGION and PLATFORM in wrangler.toml');
const e = encodeURIComponent;
const dir = `${root}crates/core/tests/fixtures/real`;
await mkdir(dir, { recursive: true });
async function get(path, filename) {
  const r = await fetch(`https://api.henrikdev.xyz/valorant/${path}`, { headers: { Authorization: vars.HENRIK_API_KEY }, signal: AbortSignal.timeout(30000) });
  if (!r.ok) throw new Error(`Upstream HTTP ${r.status} for ${filename}`);
  const data = await r.json();
  await writeFile(`${dir}/${filename}.json`, JSON.stringify(data, null, 2));
  console.log(`Captured ${filename}.json`);
  return data.data;
}
const mmr = await get(`v3/mmr/${e(region)}/${e(platform)}/${e(name)}/${e(tag)}`, 'mmr');
const history = await get(`v2/by-puuid/mmr-history/${e(region)}/${e(platform)}/${e(mmr.account.puuid)}`, 'history');
if (history.history?.[0]) await get(`v4/match/${e(region)}/${e(history.history[0].match_id)}`, 'match');
console.log(`Rank: ${mmr.current.tier.name}, ${mmr.current.rr} RR; ${history.history?.length ?? 0} history entries`);
