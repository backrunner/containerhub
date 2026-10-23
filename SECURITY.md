# Security

ContainerHub is pre-release software. The remote protocol and local queue have
automated tests, but production app attestation/enrollment, independent security
review and real public-network acceptance are incomplete. See
[the remote development notes](docs/REMOTE-DEVELOPMENT.md).

Report vulnerabilities privately to **dev@backrunner.top**. Include the affected
commit, platform, reproduction steps and impact. Do not post credentials, private
keys, enrollment profiles or working exploit details in public issues. No fixed
response time or supported release branch is promised at this stage.

The development feature creates loopback-only fake environments. It is not a
production trust issuer. Normal remote clients require authenticated encrypted
transport and pinned Agent identity after initial binding; initial ownership is
still trust-on-first-use. TLS does not replace application authorization.

Local container management runs with the current user's runtime permissions.
Application logs can contain workload output; review exported logs before
sharing. Generated state, keys and local diagnostics must remain outside Git.
