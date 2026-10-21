"""UI state and reversible decision batches, independent of Qt and workbook I/O."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field

from ..models import AnalysisResult, Decision, ProjectConfig


def decision_stamp(decisions):
    payload = {k: v.model_dump(mode="json") for k, v in decisions.items()}
    return hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()


@dataclass
class SessionController:
    config: ProjectConfig = field(default_factory=ProjectConfig)
    decisions: dict[str, Decision] = field(default_factory=dict)
    history: list[dict] = field(default_factory=list)
    redo_history: list[dict] = field(default_factory=list)
    audit: list[dict] = field(default_factory=list)
    result: AnalysisResult | None = None
    analyzed_stamp: str = ""
    advanced: bool = False
    dirty: bool = False

    @property
    def current(self):
        return bool(
            self.result
            and self.result.config_hash == self.config.fingerprint()
            and self.analyzed_stamp == decision_stamp(self.decisions)
        )

    def snapshot(self):
        return {k: v.model_dump(mode="json") for k, v in self.decisions.items()}

    def queue(self, updated, label):
        if updated == self.decisions:
            return False
        self.history.append(self.snapshot())
        self.redo_history.clear()
        keys = [k for k, value in updated.items() if self.decisions.get(k) != value]
        self.decisions = updated
        self.audit.append({"label": label, "cells": len(keys), "keys": keys})
        self.dirty = True
        return True

    def undo(self):
        if not self.history:
            return False
        self.redo_history.append(self.snapshot())
        self.decisions = {k: Decision.model_validate(v) for k, v in self.history.pop().items()}
        self.audit.append({"label": "Décision annulée", "cells": 0})
        self.dirty = True
        return True

    def redo(self):
        if not self.redo_history:
            return False
        self.history.append(self.snapshot())
        self.decisions = {k: Decision.model_validate(v) for k, v in self.redo_history.pop().items()}
        self.audit.append({"label": "Décision rétablie", "cells": 0})
        self.dirty = True
        return True

    def accept(self, result, stamp):
        if result.config_hash != self.config.fingerprint() or stamp != decision_stamp(self.decisions):
            return False
        self.result, self.analyzed_stamp = result, stamp
        return True
