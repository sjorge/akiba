# anidb-http

Blocking client for the AniDB HTTP API and the AniDB anime titles dump.

- `HttpApi` fetches one anime (`request=anime`) and `parse_anime` maps the XML
  into the typed `AniDbShow` structs. AniDB error replies become an `Error`.
- `fetch_titles_dump` (and `fetch_titles_dump_with`, over any `Transport`)
  downloads the gzipped anime-titles dump, and `parse_titles` maps it to the official and main titles of each anime id.
- `fetch_poster` and `poster_url` download a poster from the AniDB CDN by its
  `picture` file name.

The HTTP API needs a registered client name and version, sent as `client` and
`clientver`. Register one at https://anidb.net/software/add.

`UreqTransport` uses `ureq` with the platform's native TLS (`native-tls`). The
`Transport` trait lets callers and tests supply their own transport.

## TLS and the trust store

The TLS library and the trust store are both the system's. ureq is built with
`native-tls-no-default`, so the Mozilla root bundle is not compiled in, and every
agent is built by `tls::build_agent`, which sets `RootCerts::PlatformVerifier`
explicitly. (ureq's default roots are the bundled ones: asking for them without
the bundle panics at the first request, so nothing relies on the default.)

Automatic mode (`TlsOptions::automatic()`) looks in these places:

- macOS: the Security framework's trust evaluation (system and user keychains,
  admin and user trust settings).
- Linux, FreeBSD, illumos (OpenSSL): OpenSSL's compiled-in default file and
  directory (`SSL_CERT_FILE` and `SSL_CERT_DIR` override them), plus what
  `openssl-probe` finds. The bundle file is `SSL_CERT_FILE` when it names an
  existing file, otherwise the first existing file of the list below. Every
  existing directory of the list is loaded as well, plus `SSL_CERT_DIR` when it
  names an existing directory:
  - Linux: files `/etc/ssl/certs/ca-certificates.crt`,
    `/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem`,
    `/etc/pki/tls/certs/ca-bundle.crt`, `/etc/ssl/ca-bundle.pem`,
    `/etc/pki/tls/cacert.pem`, `/etc/ssl/cert.pem`,
    `/opt/etc/ssl/certs/ca-certificates.crt`, `/etc/ssl/certs/cacert.pem`;
    directories `/etc/ssl/certs`, `/etc/pki/tls/certs`,
    `/etc/security/certificates`.
  - FreeBSD: file `/usr/local/etc/ssl/cert.pem`; directories `/etc/ssl/certs`,
    `/usr/local/share/certs`.
  - illumos: files `/etc/ssl/cacert.pem` (OmniOS), `/etc/certs/ca-certificates.crt`
    (OpenIndiana), `/etc/ssl/certs/ca-certificates.crt` (SmartOS); directory
    `/etc/certs/CA`.

A minimal install without a CA bundle (or one in an unusual place) has nothing to
find. `TlsOptions::new(Some(path))` (config `tls.ca_file`) replaces the search:
only the certificates in that PEM file are trusted, and an unreadable file, a
file that is not PEM and a file without a certificate are errors when the agent
is built. A TLS failure at request time carries `tls::CERT_HINT`.
