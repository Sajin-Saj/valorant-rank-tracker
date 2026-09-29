import test from 'node:test';
import assert from 'node:assert/strict';
globalThis.location = { search: '' };
globalThis.window = {};
globalThis.document = { hidden:false };
const { TrackerFeed, signed, safeImage } = await import('../public/shared/client.js');
const state = events => ({rank:{tier_name:'Gold 2'},events});
test('feed suppresses initial, quiet, repeated, and reset event IDs', () => {
  const feed=new TrackerFeed();const fired=[];feed.addEventListener('event',e=>fired.push(e.detail.id));
  feed.ingest(state([{id:8,quiet:false}]));
  feed.ingest(state([{id:8,quiet:false},{id:9,quiet:true},{id:10,quiet:false}]));
  feed.ingest(state([{id:8,quiet:false},{id:9,quiet:true},{id:10,quiet:false}]));
  feed.ingest(state([{id:1,quiet:false}]));
  feed.ingest(state([{id:1,quiet:false},{id:2,quiet:false}]));
  assert.deepEqual(fired,[10,2]);
});
test('OBS idle/live cadence and last good state on network failure', async () => {
  const feed=new TrackerFeed({fetcher:async()=>{throw Error('offline');}});feed.ingest(state([]));
  window.obsstudio={};feed.live=false;assert.equal(feed.interval(),300000);feed.live=true;assert.equal(feed.interval(),30000);
  await feed.poll();clearTimeout(feed.timer);assert.equal(feed.state.rank.tier_name,'Gold 2');window.obsstudio=undefined;
});
test('image URLs and deltas are safe and explicit', () => {
  assert.equal(signed(-16),'−16');assert.equal(signed(21),'+21');assert.equal(signed(0),'0');
  assert.equal(safeImage('javascript:alert(1)'),'');assert.equal(safeImage('https://evil.example/icon.png'),'');
  assert.equal(safeImage('https://media.valorant-api.com/icon.png'),'https://media.valorant-api.com/icon.png');
});
