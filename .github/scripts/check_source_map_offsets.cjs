const assert = require('node:assert/strict');
const path = require('node:path');
const { createRequire } = require('node:module');
const projectRequire = createRequire(path.resolve(process.argv[2] || path.resolve(__dirname, '../../crates/graphile-worker-admin-ui'), 'package.json'));
const { SourceMapConsumer } = projectRequire('source-map-js');
const basic = { version: 3, sources: ['fixture.js'], names: [], mappings: 'AAAA' };
const indexed = line => ({ version: 3, sections: [{offset:{line,column:0},map:basic}] });
// Construction alone is bounded: do not expand a malicious map into a generator.
assert.throws(() => new SourceMapConsumer(indexed(10000001)), /must not exceed/);
const consumer = new SourceMapConsumer(indexed(1));
assert.equal(consumer.originalPositionFor({line:2,column:1}).source, 'fixture.js');
assert.equal(consumer.originalPositionFor({line:2,column:1}).line, 1);
console.log('oversized indexed map rejected; valid indexed map resolves correctly');
