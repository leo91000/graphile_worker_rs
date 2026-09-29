#!/usr/bin/env bash
set -euo pipefail

container="graphile-worker-tls-$$"
fixture_dir="$(mktemp -d)"
cleanup() {
  docker stop "$container" >/dev/null 2>&1 || true
  unlink "$fixture_dir/ca.crt" 2>/dev/null || true
  unlink "$fixture_dir/require-tls.sh" 2>/dev/null || true
  rmdir "$fixture_dir" 2>/dev/null || true
}
trap cleanup EXIT

cat > "$fixture_dir/require-tls.sh" <<'EOF'
#!/usr/bin/env bash
set -e
sed -i '1i hostnossl all all all reject' "$PGDATA/pg_hba.conf"
EOF
chmod +x "$fixture_dir/require-tls.sh"

docker run -d --rm --name "$container" -p 127.0.0.1::5432 \
  --tmpfs /var/lib/postgresql/data:rw \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres \
  -e TLS_MAX_PROTOCOL_VERSION="${TLS_MAX_PROTOCOL_VERSION:-TLSv1.3}" \
  -v "$fixture_dir/require-tls.sh:/docker-entrypoint-initdb.d/require-tls.sh:ro" \
  --entrypoint bash postgres:16 -ec '
    openssl req -x509 -newkey rsa:2048 -nodes \
      -keyout /tmp/ca.key -out /tmp/ca.crt -days 1 \
      -subj /CN=graphile-worker-test-ca \
      -addext basicConstraints=critical,CA:TRUE >/dev/null 2>&1
    openssl req -newkey rsa:2048 -nodes \
      -keyout /tmp/server.key -out /tmp/server.csr \
      -subj /CN=localhost >/dev/null 2>&1
    printf "%s\n" basicConstraints=critical,CA:FALSE \
      keyUsage=critical,digitalSignature,keyEncipherment \
      extendedKeyUsage=serverAuth subjectAltName=DNS:localhost \
      > /tmp/server.ext
    openssl x509 -req -in /tmp/server.csr -CA /tmp/ca.crt \
      -CAkey /tmp/ca.key -CAcreateserial -out /tmp/server.crt \
      -days 1 -extfile /tmp/server.ext >/dev/null 2>&1
    chown postgres:postgres /tmp/server.key /tmp/server.crt
    chmod 600 /tmp/server.key
    exec docker-entrypoint.sh postgres -c ssl=on \
      -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key \
      -c ssl_max_protocol_version="$TLS_MAX_PROTOCOL_VERSION"
  ' >/dev/null

for _ in $(seq 120); do
  if docker exec "$container" pg_isready -U postgres -h localhost >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! docker exec "$container" pg_isready -U postgres -h localhost >/dev/null; then
  docker logs "$container" >&2
  exit 1
fi

if docker exec -e PGPASSWORD=postgres "$container" psql \
  'host=localhost user=postgres dbname=postgres sslmode=disable' \
  -c 'SELECT 1' >/dev/null 2>&1; then
  echo "TLS fixture unexpectedly accepted a plaintext connection" >&2
  exit 1
fi

docker cp "$container:/tmp/ca.crt" "$fixture_dir/ca.crt" >/dev/null
port="$(docker port "$container" 5432/tcp | awk -F: '{print $NF}')"
export PGSSLROOTCERT="$fixture_dir/ca.crt"
export DATABASE_URL="postgres://postgres:postgres@localhost:$port/postgres?sslmode=require"

for feature in tls-rustls tls-native-tls tls-rustls,tls-native-tls; do
  cargo test -p graphile_worker_database --no-default-features \
    --features "runtime-tokio,driver-tokio-postgres,$feature" \
    --test driver_contract tokio_postgres_ -- --nocapture
done
