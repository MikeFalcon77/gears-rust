"""Policy regressions: dependency aliases/targets and new call-site paths."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("crypto_guard", Path(__file__).parents[1] / "check_crypto_facade.py")
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class CryptoFacadePolicyTests(unittest.TestCase):
    def metadata(self, path, dependency):
        return {"workspace_members": ["app"], "packages": [{"id": "app",
            "manifest_path": str(Path("/repo") / path / "Cargo.toml"),
            "dependencies": [dependency]}]}

    def test_alias_and_target_do_not_bypass_dependency_policy(self):
        for kind in [None, "build", "dev"]:
            metadata = self.metadata("gears/new-app", {"name": "sha2", "rename": "checksum",
                "kind": kind, "target": 'cfg(target_os = "windows")'})
            self.assertTrue(guard.dependency_errors(metadata, Path("/repo")))

    def test_oracle_exception_is_dev_only_and_path_specific(self):
        dependency = {"name": "rsa", "kind": "dev"}
        self.assertFalse(guard.dependency_errors(self.metadata(guard.BACKEND, dependency), Path("/repo")))
        self.assertTrue(guard.dependency_errors(self.metadata("libs/new-provider", dependency), Path("/repo")))
        dependency["kind"] = None
        self.assertTrue(guard.dependency_errors(self.metadata(guard.BACKEND, dependency), Path("/repo")))

    def test_new_file_cannot_select_a_tls_backend_or_verify_jwt_directly(self):
        for source in ["rustls::crypto::aws_lc_rs::default_provider();",
                       "rustls_cng_crypto::fips_provider();",
                       "jsonwebtoken::decode::<Claims>(token, key, validation);",
                       "use jsonwebtoken::{Header, decode as parse};"]:
            self.assertTrue(guard.source_errors("gears/new/src/lib.rs", source))
        self.assertFalse(guard.source_errors("gears/new/src/lib.rs", "// rustls::crypto::aws_lc_rs\nuse toolkit_security::crypto::hash::Sha256;"))

    def test_aliases_groups_and_url_strings_do_not_hide_calls(self):
        for source in ["use jsonwebtoken as jwt; jwt::decode::<Claims>(token, key, validation);",
                       "use jsonwebtoken::{crypto}; crypto::verify(sig, msg, key, alg);",
                       "use rustls::crypto as backend; backend::aws_lc_rs::default_provider();",
                       "use rustls::crypto::{aws_lc_rs};",
                       'let url = "https://issuer"; jsonwebtoken::decode::<Claims>(token, key, validation);']:
            self.assertTrue(guard.source_errors("gears/new/src/lib.rs", source))


if __name__ == "__main__":
    unittest.main()
