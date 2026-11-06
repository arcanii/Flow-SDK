"""Flow's settings, seen and changed by the program as ``flow-settings``
does (the SDK's ``flow_sdk::settings``, OSC 7732)::

    import flow_sdk as flow
    from flow_sdk import settings

    if flow.in_flow():
        try:
            print(settings.get("fx.style"))   # "rain", say
            settings.set("font.size", 16)     # Flow applies it, saves it and says so
        except settings.NoAnswer:
            pass                              # not Flow after all, or one from before 0.1.30
        except settings.Refused as e:
            print(e)                          # why, in Flow's language

A request is a dict, ``{"do": ...}`` with a ``"key"`` (a dotted name from
Flow's config.toml, as ``"fx.style"``) and a ``"value"`` (anything JSON
holds) where it needs them, and Flow answers with another: ``{"ok": True,
...}``, or ``{"ok": False, "error": ...}`` with the reason in Flow's
language. ``"do"`` is ``"list"``, ``"get"``, ``"set"``, ``"reset"`` (a key,
or ``"all"``), ``"reset-terminal"`` or ``"restart"``, and a function below
asks each. Flow decides what a program may see and change: never the
assistant's settings, nor the user's own theme.

:func:`ask` gives Flow's answer as it came, ok or not, and None when
nothing answered. The others raise :class:`Refused`, carrying Flow's
reason, for an answer that isn't ok, and :class:`NoAnswer` when nothing
answered: a terminal that isn't Flow, a Flow from before 0.1.30, or no
terminal at all, where nothing is asked and nothing waited for. Both are
a :class:`SettingsError`. A terminal that isn't Flow says nothing, so the
whole wait passes first: ask where :func:`flow_sdk.in_flow` (or
``Caps.flow``) says it's Flow.

On macOS and Linux the question goes to the program's terminal itself,
so it reaches Flow however standard input and output are piped or
redirected; where none of the three standard streams is a terminal,
nothing is asked. The answer comes in as keys, so raw mode is turned on
for the wait where it isn't on already (:func:`flow_sdk.raw_mode`,
:func:`flow_sdk.screen`) and off again after: a program reading its input
a line at a time can ask between lines, and keys waiting to be read or
typed meanwhile go with the answer. An answer that comes after its wait
has run out lands in the terminal's input: the next request throws away
what waits there before it asks, but meanwhile a terminal that isn't raw
echoes it, and ``input()`` reads it with the next line. So don't cut the
wait short: a Flow behind other windows or without a GPU answers slower.
The functions below also check that an answer names what they asked,
raising :class:`NoAnswer` for an earlier request's that came in its
place; :func:`ask` gives what came, whatever it answers.

On Windows the question goes down Flow's pipe instead, which needs no raw
mode, and the pipe can't be read against a clock, so ``wait`` isn't used
there: Flow's own wait, three seconds or so, bounds it, and a Flow that
couldn't answer in time, or one from before 0.1.30, is no answer.

The module's ``list`` and ``set`` are its own, not the builtins': call them
as ``settings.list()`` and ``settings.set(...)``.
"""

import json
from typing import Any, Dict, Optional

from . import _native
from ._native import lib
from .term import _flush

#: How long Flow is given to answer, in seconds: as long as
#: ``flow-settings`` gives it.
WAIT = 3.0


class SettingsError(Exception):
    """A request about Flow's settings that came to nothing, or was refused."""


class Refused(SettingsError):
    """Flow refused the request: ``str()`` of it is Flow's reason, in Flow's
    language."""

    def __init__(self, message: str, answer: Optional[dict] = None):
        super().__init__(message)
        #: Flow's reason.
        self.message = message
        #: Flow's whole answer.
        self.answer = answer


class NoAnswer(SettingsError):
    """Nothing answered within the wait: a terminal that isn't Flow, a Flow
    from before 0.1.30, or no terminal at all; or what came was an earlier
    request's answer, too late for its own wait."""


def _json(asked: dict) -> bytes:
    """A request as the UTF-8 JSON text C takes, written as compactly as the
    Rust SDK's serde_json writes it."""
    if not isinstance(asked, dict):
        raise TypeError("a request is a dict, not %s" % type(asked).__name__)
    return json.dumps(asked, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _parsed(found: Optional[str]) -> Optional[dict]:
    """An answer's JSON as a dict; None for none, or for one that isn't an
    object, which Flow never writes."""
    if found is None:
        return None
    try:
        parsed = json.loads(found)
    except ValueError:
        return None
    return parsed if isinstance(parsed, dict) else None


def request(asked: dict) -> str:
    """The escape that asks Flow ``asked``: OSC 7732 with its JSON in
    base64. :func:`ask` writes it and reads the answer; this is for a
    program that reads its own input."""
    return _native.made(lib().flow_settings_request(_json(asked)), "the request")


def answer(data) -> Optional[dict]:
    """The first whole answer in ``data`` (what came in after asking, as
    ``bytes`` or ``str``); None until one has come whole, or if it isn't one
    Flow wrote."""
    raw = _native.text(data)
    return _parsed(_native.take(lib().flow_settings_answer(raw, len(raw))))


def ask(asked: dict, wait: float = WAIT) -> Optional[dict]:
    """Asks Flow ``asked`` and waits ``wait`` seconds at most (on Windows,
    Flow's own few seconds) for its answer, ok or not; None when nothing
    answered."""
    request = _json(asked)
    wait_ms = max(1, min(int(wait * 1000), 0xFFFFFFFF))
    _flush()
    return _parsed(_native.take(lib().flow_settings_ask(request, wait_ms)))


def _answers(asked: dict, got: dict) -> bool:
    """Whether ``got`` can be Flow's answer to ``asked``, by what it names
    and holds: an answer to ``get``, ``set`` or ``reset`` names its key
    (``"all"`` for every one), ``list``'s holds the settings, and the rest's
    neither. An earlier request's answer, come too late for its own wait,
    may be taken in this one's place and fails this; a refusal names no key,
    so it can't be told apart and is taken."""
    if got.get("ok") is not True:
        return True
    what = asked["do"]
    if what in ("get", "set", "reset"):
        return got.get("key") == asked.get("key")
    if what == "list":
        return isinstance(got.get("settings"), dict)
    return "key" not in got and "settings" not in got


def _ok(asked: dict, wait: float) -> dict:
    """Flow's answer to ``asked``, which was ok; else :class:`NoAnswer` or
    :class:`Refused`."""
    got = ask(asked, wait)
    if got is None:
        raise NoAnswer("nothing answered %r within %g s: not a Flow tab, a Flow from before 0.1.30, or no terminal" % (asked["do"], wait))
    if not _answers(asked, got):
        what = " ".join(str(asked[k]) for k in ("do", "key") if k in asked)
        raise NoAnswer("an earlier request's late answer came in place of the answer to %r, which didn't come within %g s: ask again" % (what, wait))
    if got.get("ok") is not True:
        reason = got.get("error")
        raise Refused(reason if isinstance(reason, str) and reason else "Flow refused %r" % asked["do"], got)
    return got


def _key(key: str) -> str:
    if not isinstance(key, str):
        raise TypeError("a setting is named by a str, as \"fx.style\", not %s" % type(key).__name__)
    return key


def list(wait: float = WAIT) -> Dict[str, Any]:
    """Every setting a program may see, by its dotted name, with its value."""
    return dict(_ok({"do": "list"}, wait).get("settings") or {})


def get(key: str, wait: float = WAIT) -> Any:
    """One setting's value: a str, a number, a bool, a list or None, as
    config.toml holds it."""
    return _ok({"do": "get", "key": _key(key)}, wait).get("value")


def set(key: str, value: Any, wait: float = WAIT) -> Any:
    """Changes one setting to ``value``, of the kind it already is, and
    gives what it is now. Flow applies it at once, saves it and says so in
    a toast."""
    return _ok({"do": "set", "key": _key(key), "value": value}, wait).get("value")


def reset(key: str, wait: float = WAIT) -> Any:
    """Puts one setting back to its default and gives what it is now; with
    ``"all"`` puts back every one a program may change, and gives None."""
    return _ok({"do": "reset", "key": _key(key)}, wait).get("value")


def reset_terminal(wait: float = WAIT) -> None:
    """Starts this tab afresh: its screen, scrollback and modes."""
    _ok({"do": "reset-terminal"}, wait)


def restart(wait: float = WAIT) -> None:
    """Restarts Flow, as Settings' Restart does: at once while every other
    tab waits at its prompt, else asking the user first."""
    _ok({"do": "restart"}, wait)
