#!/usr/bin/env python3
"""Keep application primitives behind toolkit-security's crypto facade.

Cargo metadata resolves workspace inheritance, renamed dependencies, target
sections and dev dependencies. This checks direct dependencies, not the
presence of cryptography transitively linked by unrelated libraries.
"""
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PRIMITIVES = {
    "aws-lc-rs", "ring", "sha1", "sha2", "md-5", "hmac", "hkdf", "aes",
    "aes-gcm", "chacha20", "chacha20poly1305", "rsa", "p256", "p384", "p521",
    "ed25519-dalek", "blake2", "blake3", "openssl", "openssl-sys",
}
FACADE = "libs/toolkit-security"
BACKEND = "libs/rustls-corecrypto-provider"
TEST_ORACLES = {
    (BACKEND, "rsa"), (BACKEND, "sha2"),
    ("gears/system/authn-resolver/plugins/firebase-authn-plugin", "rsa"),
}
RAW_TLS = re.compile(r"\b(?:rustls\s*::\s*crypto\s*::\s*(?:aws_lc_rs|default_fips_provider)|rustls_(?:corecrypto_provider|cng_crypto)\s*::)")
RAW_JWT = re.compile(r"\bjsonwebtoken\s*::\s*(?:crypto\s*::\s*verify|decode\s*(?:\(|::<))")
JWT_IMPORT = re.compile(r"\buse\s+jsonwebtoken\s*::\s*(?:decode\b|\{[^;]*\bdecode\b)", re.S)
BACKEND_IMPORT = re.compile(
    r"\buse\s+(?:jsonwebtoken\s*(?:as\b|::\s*\*|::\s*\{[^;]*\bcrypto\b)|"
    r"rustls\s*::\s*(?:crypto\s+as\b|\{[^;]*\bcrypto\b|crypto\s*::\s*\{[^;]*\b(?:aws_lc_rs|default_fips_provider)\b))", re.S)
# Match strings before comments, so // inside an issuer URL is not a comment.
RUST_TOKENS = re.compile(
    r'r(?P<hashes>#{0,255})".*?"(?P=hashes)|"(?:\\.|[^"\\])*"|(?P<comment>//[^\n]*|/\*.*?\*/)', re.S)


def dependency_errors(metadata, root):
    errors = []
    for package in metadata["packages"]:
        if package["id"] not in metadata["workspace_members"]:
            continue
        path = Path(package["manifest_path"]).parent.relative_to(root).as_posix()
        for dependency in package["dependencies"]:
            name = dependency["name"]
            if name not in PRIMITIVES:
                continue
            if path == FACADE and name == "aws-lc-rs":
                continue
            if dependency["kind"] == "dev" and (path, name) in TEST_ORACLES:
                continue
            errors.append(f"{path}/Cargo.toml: use toolkit-security::crypto instead of {name}")
    return errors


def source_errors(path, source):
    # Backend implementation and deliberate wrong-provider test are boundaries.
    if path.startswith(FACADE + "/src/crypto/") or path.startswith(BACKEND + "/"):
        return []
    if path == "libs/toolkit/tests/crypto_init_fips.rs":
        return []
    # Ignore comments, so architecture explanations are not policy violations.
    code = RUST_TOKENS.sub(lambda match: " " if match.group("comment") else '"literal"', source)
    if RAW_TLS.search(code) or RAW_JWT.search(code) or JWT_IMPORT.search(code) or BACKEND_IMPORT.search(code):
        return [f"{path}: route crypto operations/provider selection through toolkit-security::crypto"]
    return []


def main():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    errors = dependency_errors(metadata, ROOT)
    paths = subprocess.check_output(
        ["git", "ls-files", "-co", "--exclude-standard", "--", "*.rs"], cwd=ROOT,
        text=True).splitlines()
    for path in sorted(set(paths)):
        errors.extend(source_errors(path, (ROOT / path).read_text()))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Crypto facade dependency and source checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
