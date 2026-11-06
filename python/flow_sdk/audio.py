"""Audio of the program's own: tones, tunes written as note names, and songs
for a small band of instruments made in code (a plucked lute, a recorder, a
drone and a tabor), looped and faded one into the next. :class:`Audio`
plays them on the machine, in any terminal or none, its thread opening the
system's output with the first sound (CoreAudio on macOS, WASAPI on
Windows, ALSA's default device on Linux; :func:`plays`); :class:`Mixer` renders
them into the program's own buffers, for an output of its own or a test::

    from flow_sdk import audio

    dance = audio.Song(330, [
        audio.Line("lute", "A4/3 G4/1 F4/2 D5/2"),
        audio.Line("drone", "D3/8"),
        audio.Line("tabor", "X/3 x/2 x/1 r/2", gain=0.7),
    ])
    player = audio.Audio()
    player.play_song(dance)  # fades in, and the song before it out

Flow's own sounds (``flow_sdk.fx``) follow the user's settings for sound in
Flow; keep these for music and the like.

They come from the C library's ``audio`` feature, which it's built with
unless Cargo was given ``--no-default-features``. Where it wasn't,
:func:`available` is False and making anything here raises
:class:`Unavailable`, so a program can go on without music.
"""

import ctypes
from array import array
from dataclasses import dataclass
from typing import Iterable, Optional, Tuple, Union

from . import _native
from ._native import Handle, Reason, lib

#: A tone's shapes, in FLOW_WAVE_* order.
WAVES = ("sine", "triangle", "square", "saw", "noise")
#: The band, in FLOW_INSTRUMENT_* order.
INSTRUMENTS = ("lute", "recorder", "drone", "tabor")


class Unavailable(OSError):
    """Flow's C library was built without audio."""


def available() -> bool:
    """Whether the library has audio: songs and tunes can be made and mixed."""
    return bool(lib().flow_audio_built())


def plays() -> bool:
    """Whether :class:`Audio` plays anything here: a library with audio,
    on macOS, Windows or Linux."""
    return bool(lib().flow_audio_plays())


def _need() -> None:
    if not available():
        raise Unavailable(
            "Flow's C library was built without audio; build it with its `audio` feature, "
            "which `cargo build --release -p flow-sdk-c` has unless given --no-default-features"
        )


def _index(names, name: str, what: str) -> int:
    try:
        return names.index(name.lower())
    except (ValueError, AttributeError):
        raise ValueError("a %s is one of %s, not %r" % (what, ", ".join(names), name)) from None


def pitch(name: str) -> Optional[float]:
    """A note's pitch in hertz from its name ("A4" 440, "C#5", "Bb3"), or
    None for no such note."""
    _need()
    hz = lib().flow_pitch(name.encode("utf-8"))
    return hz if hz > 0 else None


@dataclass
class Tone:
    """One note: its pitch and how long it's held, how it sounds, and how
    it comes in and dies away. What's left as None is the SDK's: a sine at
    half volume in the middle, in over 5 ms and away over 80."""

    hz: float
    secs: float
    #: One of :data:`WAVES`.
    wave: Optional[str] = None
    #: 0 to 1.
    volume: Optional[float] = None
    #: Left (-1) to right (1).
    pan: Optional[float] = None
    #: Seconds to come in, and to die away after it's held.
    attack: Optional[float] = None
    release: Optional[float] = None

    def _c(self) -> _native.FlowTone:
        _need()
        tone = _native.FlowTone()
        lib().flow_tone_init(ctypes.byref(tone), float(self.hz), float(self.secs))
        if self.wave is not None:
            tone.wave = _index(WAVES, self.wave, "wave")
        for name in ("volume", "pan", "attack", "release"):
            value = getattr(self, name)
            if value is not None:
                setattr(tone, name, float(value))
        return tone


@dataclass(frozen=True)
class Line:
    """One instrument's part in a :class:`Song`: ``notes`` written as
    ``name/beats`` (``"D4/3 Bb3/2 r/1"``, ``r`` a rest; on the tabor
    ``X/1`` a full beat and ``x/1`` a light one), as loud as ``gain``
    against the instrument's own level."""

    instrument: str
    notes: str
    gain: float = 1.0


class Tune(Handle):
    """A line of notes played one after another on one sound, written as
    ``name/beats`` (``"C4/1 E4/1 G4/2 r/1"``) at ``tempo`` beats a minute,
    each note sounding as ``sound`` does but for its pitch and length, once
    or ``looped`` until stopped. Notes that don't read raise ValueError,
    saying why."""

    FREE = "flow_tune_free"
    WHAT = "tune"

    def __init__(self, notes: str, tempo: float, sound: Optional[Tone] = None, looped: bool = False):
        _need()
        reason = Reason()
        sound = None if sound is None else ctypes.byref(sound._c())
        handle = lib().flow_tune_new(_native.text(notes), float(tempo), sound, bool(looped), reason.ref())
        if not handle:
            raise ValueError(reason.take() or "Flow's library refused the tune")
        super().__init__(handle)

    @property
    def length(self) -> float:
        """Seconds once through."""
        return lib().flow_tune_length(self._live())


class Song(Handle):
    """A piece for several instruments at once, played looped: its lines
    side by side, each as many beats long as the others, at ``tempo`` beats
    a minute. ``lines`` are :class:`Line`\\ s, or (instrument, notes[, gain])
    tuples."""

    FREE = "flow_song_free"
    WHAT = "song"

    def __init__(self, tempo: float, lines: Iterable[Union[Line, Tuple]] = ()):
        _need()
        super().__init__(lib().flow_song_new(float(tempo)), "a song at %r beats a minute: its tempo is above 0" % (tempo,))
        for line in lines:
            if isinstance(line, Line):
                self.line(line.instrument, line.notes, line.gain)
            else:
                self.line(*line)

    def line(self, instrument: str, notes: str, gain: float = 1.0) -> "Song":
        """Adds ``instrument`` (one of :data:`INSTRUMENTS`) playing
        ``notes``; notes that don't read raise ValueError, saying why."""
        reason = Reason()
        if lib().flow_song_line(self._live(), _index(INSTRUMENTS, instrument, "instrument"), float(gain), _native.text(notes), reason.ref()) != 0:
            raise ValueError(reason.take() or "Flow's library refused the line")
        return self

    @property
    def length(self) -> float:
        """Seconds once through: its longest line's beats."""
        return lib().flow_song_length(self._live())


class _Player(Handle):
    """What :class:`Audio` and :class:`Mixer` both do, by their functions'
    prefix."""

    PREFIX = ""

    def _call(self, name: str, *args) -> None:
        if getattr(lib(), self.PREFIX + name)(self._live(), *args) != 0:
            raise ValueError("Flow's library refused it (a number that isn't finite?)")

    def play(self, tone: Tone) -> None:
        self._call("play", ctypes.byref(tone._c()))

    def play_tune(self, tune: Tune) -> None:
        self._call("play_tune", tune._live())

    def play_song(self, song: Song) -> None:
        """Plays ``song`` looped, fading it in over a second as the song
        before it fades out over a little more."""
        self._call("play_song", song._live())

    def stop_song(self) -> None:
        """The song playing fades out."""
        self._call("stop_song")

    def stop(self) -> None:
        """Everything falls silent at once, songs too."""
        self._call("stop")

    def set_volume(self, volume: float) -> None:
        """The whole mix's volume, 0 to 1 (1 unless set)."""
        self._call("set_volume", float(volume))


class Audio(_Player):
    """The program's sound on the machine: a thread of its own, started
    with the first sound, opens the system's output then and closes it
    after three seconds of silence. What it's given is copied. Closing it
    stops what's playing."""

    FREE = "flow_audio_free"
    WHAT = "audio"
    PREFIX = "flow_audio_"

    def __init__(self):
        _need()
        super().__init__(lib().flow_audio_new(), "the audio")


class Mixer(_Player):
    """The mix itself at ``rate`` samples a second, with no output: the
    voices, tunes and songs summed with a soft limit, rendered by
    :meth:`fill` for an output of the program's own."""

    FREE = "flow_mixer_free"
    WHAT = "mixer"
    PREFIX = "flow_mixer_"

    def __init__(self, rate: float = 48000.0):
        _need()
        super().__init__(lib().flow_mixer_new(float(rate)), "a mixer at %r samples a second: its rate is above 0" % (rate,))
        self.rate = float(rate)

    @property
    def busy(self) -> bool:
        """Whether anything still sounds or is to come: a song, until it
        has faded out."""
        return bool(lib().flow_mixer_busy(self._live()))

    def fill(self, frames: int, channels: int = 2) -> array:
        """The next ``frames`` frames of the mix, ``channels`` samples each
        (left and right first, any more silent, one alone both), as an
        array of floats."""
        count = int(frames) * int(channels)
        if count < 0 or int(channels) < 1:
            raise ValueError("a mix is frames of one channel or more")
        buffer = (ctypes.c_float * count)()
        if lib().flow_mixer_fill(self._live(), buffer, count, int(channels)) != 0:
            raise ValueError("Flow's library refused the mix")
        out = array("f")
        out.frombytes(bytes(buffer))
        return out
