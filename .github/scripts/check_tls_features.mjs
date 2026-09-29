import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

const connectors = ['graphile_worker_postgres_tls', 'tokio-postgres-rustls', 'postgres-native-tls'];
const cases = [
  { features: ['driver-sqlx', 'tls-rustls'], absent: connectors },
  { features: ['driver-sqlx', 'tls-native-tls'], absent: connectors },
  { features: ['driver-tokio-postgres'], absent: ['rustls', 'native-tls', ...connectors.slice(1)] },
  { features: ['driver-tokio-postgres', 'tls-rustls'], absent: ['native-tls', 'postgres-native-tls', 'openssl-sys'], present: ['tokio-postgres-rustls'] },
  { features: ['driver-tokio-postgres', 'tls-native-tls'], absent: ['rustls', 'tokio-postgres-rustls'], present: ['postgres-native-tls'] },
  { features: ['driver-tokio-postgres', 'tls-rustls', 'tls-native-tls'], absent: [], present: connectors },
];

for (const { features, absent, present = [] } of cases) {
  const tree = execFileSync('cargo', [
    'tree', '-p', 'graphile_worker_database', '--no-default-features',
    '--features', ['runtime-tokio', ...features].join(','),
    '-e', 'normal', '--prefix', 'none',
  ], { encoding: 'utf8' });
  const packages = new Set(tree.split('\n').map(line => line.split(' ')[0]));
  for (const name of absent) {
    assert(!packages.has(name), `${features.join(',')} unexpectedly includes ${name}`);
  }
  for (const name of present) {
    assert(packages.has(name), `${features.join(',')} is missing ${name}`);
  }
  console.log(`Dependency isolation passed: ${features.join(',')}`);
}
