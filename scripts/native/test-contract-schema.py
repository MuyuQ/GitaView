"""Validate native contract examples with a draft-07 implementation."""
import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "contracts/native/v1"
SCHEMA = json.loads((CONTRACT / "schema.json").read_text(encoding="utf-8"))


def validator(definition=None):
    schema = copy.deepcopy(SCHEMA)
    if definition:
        schema.pop("oneOf", None)
        schema.pop("anyOf", None)
        schema["$ref"] = "#/definitions/" + definition
    return Draft7Validator(schema)


class ContractSchemaTests(unittest.TestCase):
    def test_schema_is_valid_draft7(self):
        Draft7Validator.check_schema(SCHEMA)

    def test_overlapping_directory_requests_are_valid_at_root_and_entry(self):
        request = {"path": "C:\\repos\\example"}
        validator().validate(request)
        for definition in ("probeRequest", "scanDirectoryInput", "repoAddInput"):
            with self.subTest(definition=definition):
                validator(definition).validate(request)

    def test_real_snapshot_fixtures_validate(self):
        for path in (CONTRACT / "fixtures").glob("snapshot-*.json"):
            with self.subTest(fixture=path.name):
                validator("appSnapshot").validate(json.loads(path.read_text(encoding="utf-8")))

    def test_nullable_refs_accept_null_and_defined_values_but_not_unknown_codes(self):
        for definition, field in (("repoStatus", "error"), ("probeResult", "error"), ("appSnapshot", "refreshError"),
                                  ("widgetRepo", "errorCode"), ("widgetSnapshot", "refreshErrorCode")):
            # Isolate each property while preserving the schema's local ref scope.
            schema = {"definitions": SCHEMA["definitions"],
                      "allOf": [SCHEMA["definitions"][definition]["properties"][field]]}
            check = Draft7Validator(schema)
            with self.subTest(definition=definition):
                check.validate(None)
                check.validate({"code": "GIT_FAILED"} if field in ("error", "refreshError") else "GIT_FAILED")
                self.assertFalse(check.is_valid({"code": "UNKNOWN"} if field in ("error", "refreshError") else "UNKNOWN"))

    def test_entry_still_rejects_unknown_fields(self):
        self.assertFalse(validator("repoAddInput").is_valid({"path": "C:\\repo", "unexpected": True}))

    def test_windows_ready_widget_shape_accepts_explicit_null_error(self):
        snapshot = {
            "schemaVersion": 1, "sessionId": "windows", "generation": 1,
            "stateRevision": 2, "settingsRevision": 1, "attemptedAtMs": 1000,
            "lastSuccessAtMs": 1000, "refreshState": "ready", "refreshErrorCode": None,
            "refreshPolicy": {"enabled": True, "intervalMinutes": 5},
            "counts": dict.fromkeys(("total", "synced", "localAhead", "remoteAhead", "diverged", "noRemote", "error", "pending"), 0),
            "compact": dict.fromkeys(("synced", "syncable", "needsAttention", "noRemote"), 0),
            "attention": [],
        }
        validator("widgetSnapshot").validate(snapshot)
        validator().validate(snapshot)


if __name__ == "__main__":
    unittest.main()
