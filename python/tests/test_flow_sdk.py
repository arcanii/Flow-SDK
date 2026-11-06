"""The Python package against Flow's C library: where the library is looked
for, every function in flow_sdk.h declared for ctypes, each wrapper writing
the very escapes the Rust SDK's own tests expect, scenes read back as glTF,
and the program's own audio mixed without a sound played. Run them with

    python3 -I -B -m unittest discover -s python/tests

after `cargo build -p flow-sdk-c` (or with FLOW_SDK_LIB naming the
library); `cargo test -p flow-sdk-c` runs them too. The tests that need the
library are skipped, saying so, where it isn't built and isn't named.
"""

import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

PACKAGE_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PACKAGE_ROOT))

import flow_sdk as flow  # noqa: E402
from flow_sdk import _native, audio, fx, model, picture, scene, settings  # noqa: E402

HEADER = PACKAGE_ROOT.parent / "c" / "include" / "flow_sdk.h"


def library_or_skip(test):
    """Loads the library, or skips the test where it isn't built and isn't
    named. One named in FLOW_SDK_LIB that won't load is a failure."""
    try:
        _native.lib()
    except OSError as e:
        if os.environ.get("FLOW_SDK_LIB"):
            raise
        test.skipTest("Flow's C library isn't built: %s" % e)


class WhereTheLibraryIsLookedFor(unittest.TestCase):
    def test_each_os_names_it_as_cargo_builds_it(self):
        self.assertEqual(_native.library_name("darwin"), "libflow_sdk_c.dylib")
        self.assertEqual(_native.library_name("linux"), "libflow_sdk_c.so")
        self.assertEqual(_native.library_name("win32"), "flow_sdk_c.dll")

    def test_flow_sdk_lib_is_the_only_place_looked_once_its_set(self):
        named = _native.candidates({"FLOW_SDK_LIB": "/opt/flow/libflow_sdk_c.so"}, here="/x/y", platform="linux")
        self.assertEqual(named, [Path("/opt/flow/libflow_sdk_c.so")])
        with tempfile.TemporaryDirectory() as folder:
            self.assertEqual(_native.candidates({"FLOW_SDK_LIB": folder}, platform="linux"), [Path(folder) / "libflow_sdk_c.so"])

    def test_cargos_targets_are_looked_in_release_first_then_upward(self):
        found = _native.candidates({"CARGO_TARGET_DIR": "/build"}, here="/repo/python/flow_sdk", platform="darwin")
        name = "libflow_sdk_c.dylib"
        self.assertEqual(found[:2], [Path("/build/release") / name, Path("/build/debug") / name])
        self.assertIn(Path("/repo/target/release") / name, found)
        self.assertLess(found.index(Path("/repo/target/release") / name), found.index(Path("/repo/target/debug") / name))

    def test_a_library_in_the_package_itself_comes_before_the_workspaces(self):
        name = "libflow_sdk_c.so"
        found = _native.candidates({"CARGO_TARGET_DIR": "/build"}, here="/site/flow_sdk", platform="linux")
        own = found.index(Path("/site/flow_sdk") / name)
        self.assertEqual(own, 2, "after CARGO_TARGET_DIR's, which is asked for")
        self.assertLess(own, found.index(Path("/site/flow_sdk/target/release") / name))
        self.assertEqual(_native.candidates({}, here="/site/flow_sdk", platform="linux")[0], Path("/site/flow_sdk") / name)

    def test_every_function_in_the_header_is_declared_for_ctypes(self):
        if not HEADER.is_file():
            self.skipTest("flow_sdk.h isn't beside the package here")
        code = re.sub(r"/\*.*?\*/", "", HEADER.read_text(), flags=re.S)
        declared = set(re.findall(r"\b(flow_\w+)\s*\(", code))
        self.assertGreater(len(declared), 50)
        self.assertEqual(declared, set(_native.SIGNATURES))

    def test_the_header_and_the_package_speak_one_version(self):
        if not HEADER.is_file():
            self.skipTest("flow_sdk.h isn't beside the package here")
        abi = re.search(r"#define FLOW_SDK_ABI (\d+)", HEADER.read_text())
        self.assertEqual(int(abi.group(1)), _native.ABI)


class Effects(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)

    def test_an_effect_is_written_as_flow_reads_it(self):
        bolt = flow.Effect.bolt((3, 4), (10, 4)).color(0xFF5A1A).size(2.0).strength(0.8)
        self.assertEqual(bolt.escape(), "\x1b]7731;fx=bolt;at=3,4;to=10,4;color=ff5a1a;size=2.0;strength=0.80\x07")
        stir = flow.Effect.stir((4, 7), (1.0, -1.0)).color(0xE02020).sound("thud").size(10.0)
        self.assertEqual(str(stir), "\x1b]7731;fx=stir;at=4,7;color=e02020;dir=1.00,-1.00;sound=thud;size=10.0;strength=0.50\x07")
        ripple = flow.Effect.ripple((0, 0)).quiet().size(900.0)
        self.assertEqual(ripple.escape(), "\x1b]7731;fx=ripple;at=0,0;sound=none;size=40.0;strength=0.50\x07")
        self.assertIn(";volume=0.40;", flow.Effect.burst((3, 4)).volume(0.4).escape())

    def test_what_flow_doesnt_have_is_refused(self):
        with self.assertRaises(ValueError):
            flow.Effect("splash", (0, 0))
        with self.assertRaises(ValueError):
            flow.Effect.burst((0, 0)).sound("kazoo")
        with self.assertRaises(ValueError):
            flow.Effect.burst((0, 0)).size(float("nan"))
        with self.assertRaises(ValueError):
            flow.Effect.burst((70000, 0))

    def test_a_budget_spends_a_burst_and_keeps_some_back(self):
        budget = flow.Budget()
        self.assertEqual(sum(budget.take(0.0, keep=2) for _ in range(9)), 5)
        self.assertTrue(budget.take(0.0) and budget.take(0.0) and not budget.take(0.0))
        self.assertAlmostEqual(budget.left(0.1), 0.5, places=5)
        own = flow.Budget(per_second=1, burst=2)
        self.assertEqual(sum(own.take(3.0) for _ in range(4)), 2)
        own.close()
        with self.assertRaises(ValueError):
            own.take(4.0)

    def test_sounds_notes_and_the_tint(self):
        self.assertEqual(fx.sound("chime", at=(30, 4), strength=0.8), "\x1b]7731;fx=sound;sound=chime;at=30,4;strength=0.80\x07")
        self.assertEqual(fx.sound("CLICK"), "\x1b]7731;fx=sound;sound=click\x07")
        self.assertEqual(fx.sound("ding", volume=0.25), "\x1b]7731;fx=sound;sound=ding;volume=0.25\x07")
        self.assertEqual(fx.notes(9, 8, ["C5", "E5", "G5", "r", "C6", "C7"]), "\x1b[7;8;1;5;8;0;13;25,~")
        self.assertEqual(fx.notes(3, 16, [30]), "\x1b[3;16;0,~")
        self.assertEqual(fx.notes(3, 16, []), "")
        self.assertEqual((fx.note("Db5"), fx.note("C#5"), fx.note("B4"), fx.note("H5")), (2, 2, None, None))
        self.assertEqual(fx.tint(0x0C1838, 0.55), "\x1b]7731;fx=tint;color=0c1838;strength=0.55\x07")
        self.assertEqual(fx.tint_off(), "\x1b]7731;fx=tint;strength=0\x07")


class Models(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)

    def test_a_model_is_registered_whole_or_in_chunks(self):
        self.assertEqual(model.register(1, b"abc", "glb", name="scene.glb"), "\x1b_ratty;g;r;id=1;fmt=glb;source=payload;more=0;name=scene.glb;YWJj\x1b\\")
        chunks = model.register(3, b"abcdefg", "obj", chunk=3)
        self.assertEqual(chunks.count("more=1"), 2)
        self.assertTrue(chunks.endswith("more=0;Zw==\x1b\\"))
        self.assertEqual(model.register_path(2, "rat.obj"), "\x1b_ratty;g;r;id=2;fmt=obj;path=rat.obj\x1b\\")
        with self.assertRaises(ValueError):
            model.register(1, b"abc", name="a;b.glb")
        with self.assertRaises(ValueError):
            model.register(1, b"abc", "stl")

    def test_a_placement_writes_only_what_its_look_sets(self):
        placed = model.place_over(1, 2, 4, 80, 23, flow.Look(scale=model.FILL))
        self.assertEqual(placed, "\x1b_ratty;g;p;id=1;row=13;col=44;w=80;h=23;scale=1.1111112\x1b\\")
        self.assertEqual(model.place(1, 13, 44, 80, 23), "\x1b_ratty;g;p;id=1;row=13;col=44;w=80;h=23\x1b\\")
        look = flow.Look(animate=True, color=0x7CE7FF, rotation=(None, 30, None))
        self.assertEqual(model.update(4, look), "\x1b_ratty;g;u;id=4;animate=1;color=7ce7ff;ry=30\x1b\\")
        self.assertEqual(model.delete(4), "\x1b_ratty;g;d;id=4\x1b\\")
        self.assertEqual(model.delete_all(), "\x1b_ratty;g;d\x1b\\")


class Meshes(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)

    def test_a_mesh_becomes_a_glb_whose_header_says_its_length(self):
        with flow.Mesh() as mesh:
            gold = (0.9, 0.7, 0.2, 1.0)
            mesh.face(gold, [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)], inside=(0.5, 0.5, 0))
            mesh.triangle(0xE0A030, (0, 0, -1), (1, 0, -1), (0, 1, -1), inside=(0.5, 0.5, 0))
            self.assertEqual(mesh.triangles, 3)
            glb = mesh.glb()
        self.assertEqual(glb[:4], b"glTF")
        self.assertEqual(int.from_bytes(glb[8:12], "little"), len(glb))
        self.assertIn(b'"generator":"flow-sdk"', glb)

    def test_a_face_in_a_material_shines_and_a_packed_glb_is_smaller(self):
        hull = flow.Material(0x101014, gloss=0.85, metal=0.3, coat=0.7, coat_gloss=0.9)
        lamp = flow.Material((0.1, 0.1, 0.1), glow=0xFF8000, glow_strength=3)
        with flow.Mesh() as mesh:
            for i in range(40):
                mesh.face(hull, [(i, 0, 1), (i + 1, 0, 1), (i + 1, 1, 1), (i, 1, 1)], inside=(0.5, 0.5, 0))
            mesh.triangle(lamp, (0, 0, -1), (1, 0, -1), (0, 1, -1), inside=(0.5, 0.5, 0))
            mesh.face(0xE0A030, [(0, 0, 2), (1, 0, 2), (0, 1, 2)], inside=(0.5, 0.5, 0))
            glb, packed = mesh.glb(), mesh.glb(packed=True)
        doc = gltf(glb)
        shiny, glowing, plain = doc["materials"]
        self.assertEqual(shiny["pbrMetallicRoughness"]["metallicFactor"], 0.3)
        self.assertAlmostEqual(shiny["pbrMetallicRoughness"]["roughnessFactor"], 0.15, places=6)
        self.assertEqual(shiny["extensions"]["KHR_materials_clearcoat"]["clearcoatFactor"], 0.7)
        self.assertEqual(glowing["emissiveFactor"][0], 1)
        self.assertEqual(glowing["extensions"]["KHR_materials_emissive_strength"]["emissiveStrength"], 3)
        self.assertEqual(plain, {"pbrMetallicRoughness": {"baseColorFactor": plain["pbrMetallicRoughness"]["baseColorFactor"], "metallicFactor": 0}, "alphaMode": "OPAQUE", "doubleSided": True}, "a color alone, as it always was")
        self.assertEqual(gltf(packed)["extensionsRequired"], ["KHR_mesh_quantization"])
        self.assertLess(len(packed), len(glb))
        self.assertEqual(flow.Material(0x336699), flow.Material((0x33 / 255, 0x66 / 255, 0x99 / 255)))
        with self.assertRaises(ValueError):
            flow.Mesh().face(flow.Material(0xFFFFFF, gloss=float("nan")), [(0, 0, 0), (1, 0, 0), (0, 1, 0)])

    def test_a_face_needs_three_points_of_three_numbers(self):
        mesh = flow.Mesh()
        with self.assertRaises(ValueError):
            mesh.face(0xFFFFFF, [(0, 0, 0), (1, 0, 0)])
        with self.assertRaises(ValueError):
            mesh.face(0xFFFFFF, [(0, 0), (1, 0), (0, 1)])
        mesh.close()
        with self.assertRaises(ValueError):
            mesh.glb()
        self.assertEqual(flow.rgb(0x336699), (0x33 / 255, 0x66 / 255, 0x99 / 255, 1.0))


def gltf(glb):
    """A GLB's JSON chunk, read."""
    length = int.from_bytes(glb[12:16], "little")
    assert glb[16:20] == b"JSON"
    return json.loads(glb[20:20 + length])


def rms(samples):
    return math.sqrt(sum(x * x for x in samples) / max(1, len(samples)))


class Scenes(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)

    def test_a_frame_reads_back_as_gltf_with_its_colors_and_the_pictures_corners(self):
        with flow.Scene(200, 100) as scene:
            scene.flat(0xFF0000, [(0, 0, 5), (10, 0, 5), (10, 10, 5)])
            scene.polygon((0, 0, 0, 0.4), [((0, 0, -5), (0, 0, 1)), ((-10, 0, -5), (0, 1, 0)), ((-10, -10, -5), (0, 0, 1))])
            self.assertEqual(scene.triangles, 2)
            glb = scene.glb((0.5, 1.0))
            named = scene.glb(generator='my "game"')
        self.assertEqual(int.from_bytes(glb[8:12], "little"), len(glb))
        doc = gltf(glb)
        self.assertEqual(len(doc["meshes"][0]["primitives"]), 3, "red, the shadow and the invisible corners")
        self.assertEqual(doc["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"], [1, 0, 0, 1])
        self.assertEqual(doc["materials"][2]["pbrMetallicRoughness"]["baseColorFactor"][3], 0)
        corners = doc["accessors"][6]
        self.assertEqual((corners["min"], corners["max"]), ([-50, -50, 0], [50, 50, 0]), "the picture's corners, squashed as the scene is")
        self.assertEqual(doc["asset"]["generator"], "flow-sdk")
        self.assertEqual(gltf(named)["asset"]["generator"], "my game")

    def test_a_scene_in_materials_reads_back_and_packs(self):
        beam = flow.Material((1.0, 0.3, 0.2, 0.5), unlit=True)
        with flow.Scene(200, 100) as scene, flow.Camera((0, 0, 0), 0.0, 0.0, 6.0, 0.7, 100) as camera:
            scene.flat(flow.Material(0x101014, gloss=0.9), [(0, 0, 5), (10, 0, 5), (10, 10, 5)])
            scene.polygon(beam, [((0, 0, -5), (0, 0, 1)), ((-10, 0, -5), (0, 1, 0)), ((-10, -10, -5), (0, 0, 1))])
            camera.face(scene, beam, [(-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)], (0, 0, 1))
            self.assertEqual(scene.triangles, 4)
            glb, packed = scene.glb(), scene.glb(packed=True)
        doc = gltf(glb)
        self.assertEqual(len(doc["materials"]), 3, "the hull, the beam and the corners")
        self.assertIn("KHR_materials_unlit", doc["materials"][1]["extensions"])
        self.assertEqual(doc["extensionsUsed"], ["KHR_materials_unlit"])
        node = gltf(packed)["nodes"][0]
        self.assertEqual(len(set(node["scale"])), 1, "one scale for every axis")

    def test_what_a_scene_cant_take_is_refused(self):
        with self.assertRaises(ValueError):
            flow.Scene(0, 100)
        scene = flow.Scene(100, 100)
        with self.assertRaises(ValueError):
            scene.flat(0xFFFFFF, [(0, 0, 0), (1, 0, 0)])
        with self.assertRaises(ValueError):
            scene.polygon(0xFFFFFF, [((0, 0, 0), (0, 0, 1))] * 2)
        with self.assertRaises(ValueError):
            scene.glb((0, 1))
        scene.close()
        with self.assertRaises(ValueError):
            scene.triangles

    def test_the_camera_sees_nearer_things_deeper_and_turns_away_from_the_back_of_a_face(self):
        with flow.Camera((0, 0, 0), 0.0, 0.0, 10.0, 0.6, 400) as camera:
            near, far = camera.project((0, 0, 2)), camera.project((0, 0, -2))
            self.assertGreater(near[2], far[2])
            self.assertLess(abs(near[0]), 1e-4)
            self.assertGreater(camera.project((1, 0, 0))[0], 0, "x is to the right")
            self.assertIsNone(camera.project((0, 0, 20)), "behind the eye")
            self.assertEqual(camera.turn((0, 0, 1)), (0, 0, 1))
            scene = flow.Scene(400, 400)
            square = [(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0)]
            camera.face(scene, 0xFFFFFF, square, (0, 0, -1))
            self.assertEqual(scene.triangles, 0, "its back is to the eye")
            camera.face(scene, 0xFFFFFF, square, (0, 0, 1))
            self.assertEqual(scene.triangles, 2)
        with self.assertRaises(ValueError):
            flow.Camera((0, 0, 0), 0, float("inf"), 10, 0.6, 400)

    def test_a_face_is_given_the_color_that_shows_once_lit(self):
        lit = flow.lit_as(0x8040C0, (0, 0, 1))
        for given, made in zip(flow.rgb(0x8040C0), lit[:3]):
            self.assertGreater(made, given, "Flow's light from in front is under full")
        self.assertEqual(lit[3], 1.0)
        self.assertEqual(flow.lit_as((1, 1, 1, 0.5), (0, 0, 1)), (1, 1, 1, 0.5), "no channel past full")


def close(test, got, want, places=5):
    """Each number of ``got`` within ``places`` of ``want``'s, nested."""
    if isinstance(want, (tuple, list)):
        test.assertEqual(len(got), len(want))
        for g, w in zip(got, want):
            close(test, g, w, places)
    else:
        test.assertAlmostEqual(got, want, places=places)


class VectorMaths(unittest.TestCase):
    """The scene's vector maths, as the Rust SDK works it out."""

    def setUp(self):
        library_or_skip(self)

    def test_points_add_scale_and_cross(self):
        a, b = (1, 2, 3), (-4, 0.5, 2)
        self.assertEqual(scene.add(a, b), (-3.0, 2.5, 5.0))
        self.assertEqual(scene.sub(a, b), (5.0, 1.5, 1.0))
        self.assertEqual(scene.mul(a, 2.5), (2.5, 5.0, 7.5))
        self.assertEqual(scene.dot(a, b), 3.0)
        self.assertEqual(scene.cross((1, 0, 0), (0, 1, 0)), (0.0, 0.0, 1.0))
        close(self, scene.normalize((3, 0, 4)), (0.6, 0.0, 0.8))
        self.assertEqual(scene.normalize((0, 0, 0)), (0.0, 0.0, 1.0), "too short to have a way")
        self.assertEqual(scene.lerp(a, b, 0.5), (-1.5, 1.25, 2.5))
        self.assertEqual((scene.smooth(0, 2, -1), scene.smooth(0, 2, 1), scene.smooth(0, 2, 5)), (0.0, 0.5, 1.0))
        with self.assertRaises(ValueError):
            scene.add((1, 2), b)

    def test_a_quarter_turn_about_y_takes_x_to_minus_z(self):
        quarter = scene.rotation((0, 1, 0), math.pi / 2)
        close(self, scene.apply(quarter, (1, 0, 0)), (0, 0, -1))
        close(self, scene.compose(scene.rotation((0, 1, 0), 0.3), scene.rotation((0, 1, 0), 0.4)), scene.rotation((0, 1, 0), 0.7))
        self.assertEqual(scene.rotation((0, 0, 5), 0), scene.IDENTITY, "no turn, whatever the axis")
        self.assertEqual(scene.apply(scene.IDENTITY, (1, 2, 3)), (1.0, 2.0, 3.0))
        # Turning by the second first: about x, then about y.
        both = scene.compose(scene.rotation((0, 1, 0), math.pi / 2), scene.rotation((1, 0, 0), math.pi / 2))
        close(self, scene.apply(both, (0, 1, 0)), (1, 0, 0))
        with self.assertRaises(ValueError):
            scene.apply(((1, 0), (0, 1)), (1, 2, 3))

    def test_colors_mix_and_take_flows_light(self):
        close(self, scene.mix(0x000000, (1, 1, 1, 0), 0.25), (0.25, 0.25, 0.25, 0.75))
        n = (0.3, 0.5, 0.8)
        lit = flow.lit_as(0x404040, n)
        close(self, [c * scene.shade(n) for c in lit[:3]], flow.rgb(0x404040)[:3])
        self.assertIs(scene.rgb, flow.rgb)


class Screens(unittest.TestCase):
    """Screens of cells, sent as the Rust SDK's own tests expect."""

    def setUp(self):
        library_or_skip(self)

    def test_only_changed_cells_are_sent(self):
        with flow.Screen(10, 3) as cells:
            self.assertEqual(cells.text(0, 0, "hp", 0xFF0000), 2)
            self.assertEqual(cells.show(), "\x1b[0m\x1b[2J\x1b[1;1H\x1b[38;2;255;0;0mhp")
            cells.text(5, 2, "x", 0xFF0000)
            self.assertEqual(cells.show(), "\x1b[0m\x1b[3;6H\x1b[38;2;255;0;0mx")
            self.assertEqual(cells.show(), "\x1b[0m", "nothing changed")
            self.assertEqual(cells.row(2), "     x    ")
            cells.forget()
            self.assertTrue(cells.show().startswith("\x1b[0m\x1b[2J"), "all of it again")

    def test_colors_and_weight_are_set_only_as_they_change_and_pictures_go_among_them(self):
        cells = flow.Screen(6, 1)
        cells.text(0, 0, "ab", 0x010203, bg=0x040506, bold=True)
        self.assertEqual(cells.text(2, 0, "c", 0x010203), 3)
        cells.image(3, 0, 2, 1, 0x010203)
        mark = "\u0305"
        self.assertEqual(
            cells.show(),
            "\x1b[0m\x1b[2J\x1b[1;1H\x1b[1m\x1b[38;2;1;2;3m\x1b[48;2;4;5;6mab\x1b[22m\x1b[49mc"
            "\x1b[58;2;0;0;1m\U0010EEEE" + mark + mark + "\U0010EEEE" + mark + "\u030d",
        )
        self.assertEqual(cells.row(0), "abc   ")
        cells.clear(bg=0x090909)
        self.assertEqual(cells.show(bare=True), "\x1b[0m\x1b[1;1H\x1b[38;2;200;200;200m\x1b[48;2;9;9;9m      ", "each show starts from a reset")

    def test_a_ground_goes_under_what_has_none_pictures_too(self):
        cells = flow.Screen(4, 1)
        cells.text(0, 0, "a", 0x010203, bg=0x040506)
        cells.image(2, 0, 2, 1, 0x010203)
        cells.ground(0x090909)
        mark = "\u0305"
        self.assertEqual(
            cells.show(),
            "\x1b[0m\x1b[2J\x1b[1;1H\x1b[38;2;1;2;3m\x1b[48;2;4;5;6ma\x1b[38;2;200;200;200m\x1b[48;2;9;9;9m "
            "\x1b[38;2;1;2;3m\x1b[58;2;0;0;1m\U0010EEEE" + mark + mark + "\U0010EEEE" + mark + "\u030d",
        )
        with self.assertRaises(ValueError):
            cells.ground(None)

    def test_what_a_screen_cant_take_is_refused(self):
        with self.assertRaises(ValueError):
            flow.Screen(0, 10)
        cells = flow.Screen(4, 2)
        cells.text(-3, 9, "off the screen")
        with self.assertRaises(ValueError):
            cells.text(0, 0, "x", bg=0x1000000)
        with self.assertRaises(ValueError):
            cells.row(2)
        cells.close()
        with self.assertRaises(ValueError):
            cells.show()


class Stages(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)
        self.stage = flow.Stage.fit(flow.Size(80, 24, (9.5, 19.0)), (9, 19), words=1)

    def test_a_stage_keeps_the_words_row_free_and_centers(self):
        stage = self.stage
        self.assertEqual((stage.cols, stage.left), (80, 0))
        self.assertLessEqual(stage.below, 23, "the words' row stays free")
        self.assertEqual((stage.width, stage.height), (stage.cols * 9.5, stage.rows * 19.0))
        self.assertIsNone(flow.Stage.fit(flow.Size(12, 5, (9.0, 18.0)), (9, 18)), "too small")
        self.assertIsNone(flow.Stage.fit(flow.Size(80, 24, None)), "no cell size known at all")
        self.assertIsNone(flow.Stage.fit(None))

    def test_a_scene_is_sent_whole_each_frame_and_placed_once(self):
        stage = self.stage.made_by("a-test")
        scene = flow.Scene(stage.width, stage.height).flat(0xFF0000, [(0, 0, 0), (50, 0, 0), (0, 50, 0)])
        first = stage.show(scene)
        self.assertTrue(first.startswith("\x1b_ratty;g;r;id=1;fmt=glb;source=payload;more=0;name=scene.glb;"))
        placed = "\x1b_ratty;g;p;id=1;row=%d;col=%d;w=%d;h=%d;scale=1.1111112\x1b\\" % (stage.top + 11, stage.left + 40, stage.cols, stage.rows)
        self.assertTrue(first.endswith(placed), first[-80:])
        self.assertEqual(stage.show(scene), "", "the same frame isn't sent again")
        scene.flat(0x00FF00, [(0, 0, 0), (-50, 0, 0), (0, -50, 0)])
        again = stage.show(scene)
        self.assertTrue(again.startswith("\x1b_ratty;g;r;id=1;") and "ratty;g;p" not in again, "a new frame takes the last one's place")
        self.assertEqual(stage.id(7).clear(), "\x1b_ratty;g;d;id=7\x1b\\")

    def test_a_packed_stage_sends_its_frames_packed(self):
        stage = self.stage.packed()
        scene = flow.Scene(stage.width, stage.height).flat(0xFF0000, [(0, 0, 0), (50, 0, 0), (0, 50, 0)])
        packed = stage.show(scene)
        unpacked = stage.packed(False).show(scene)
        self.assertTrue(unpacked.startswith("\x1b_ratty;g;r;id=1;") and unpacked != packed, "unpacked, the same scene is a new frame")
        self.assertEqual(stage.show(scene), "")

    def test_a_click_and_an_effect_land_in_the_same_cell(self):
        stage = self.stage
        at = stage.point_at(30, 10)
        self.assertEqual(stage.cell_at(at), (30, 10))
        self.assertIsNone(stage.point_at(30, 23), "the words' row isn't the picture")
        stir = stage.effect("stir", at, 90.0, 0.5, dir=(1.0, 1.0)).color(0xE02020).sound("thud")
        self.assertEqual(stir.escape(), "\x1b]7731;fx=stir;at=30,10;color=e02020;dir=1.00,-1.00;sound=thud;size=9.5;strength=0.50\x07")
        bolt = stage.effect("bolt", at, 30.0, 0.8, to=stage.point_at(40, 10)).escape()
        self.assertIn(";at=30,10;to=40,10;", bolt)
        with self.assertRaises(ValueError):
            stage.effect("splash", at, 10)
        with self.assertRaises(ValueError):
            stage.effect("bolt", at, 10, to=(float("nan"), 0))


class Audio(unittest.TestCase):
    """The program's own audio: the mixing, without a sound played (no
    output is opened), or, from a library built without audio, each part
    refusing as it should."""

    RATE = 48000

    def setUp(self):
        library_or_skip(self)

    def render(self, mixer, secs):
        return mixer.fill(int(self.RATE * secs))

    def test_without_audio_each_part_says_so(self):
        if not audio.available():
            self.assertFalse(audio.plays())
        # As a library built without audio has it, whichever this one is.
        with mock.patch.object(audio, "available", lambda: False):
            for make in (lambda: audio.Song(120), lambda: audio.Tune("A4/1", 120), audio.Mixer, audio.Audio, lambda: audio.pitch("A4"), audio.Tone(440, 1)._c):
                with self.assertRaises(audio.Unavailable):
                    make()

    def test_notes_are_named_as_written_and_refused_saying_why(self):
        if not audio.available():
            self.skipTest("the library has no audio")
        self.assertAlmostEqual(audio.pitch("A4"), 440.0, places=2)
        self.assertAlmostEqual(audio.pitch("Bb3"), 233.08, places=2)
        self.assertIsNone(audio.pitch("H4"))
        with self.assertRaisesRegex(ValueError, "A4: a note needs its length"):
            audio.Song(120, [("lute", "A4")])
        with self.assertRaisesRegex(ValueError, "X/2: no such note"):
            audio.Song(120).line("lute", "X/2")
        with self.assertRaisesRegex(ValueError, "its length isn't a number"):
            audio.Tune("A4/x", 120)
        with self.assertRaises(ValueError):
            audio.Song(120).line("kazoo", "A4/1")
        with self.assertRaises(ValueError):
            audio.Song(0)
        with self.assertRaises(ValueError):
            audio.Tone(440, 1, wave="cello")._c()

    def test_a_tone_sounds_panned_and_falls_silent(self):
        if not audio.available():
            self.skipTest("the library has no audio")
        with audio.Mixer(self.RATE) as mixer:
            mixer.play(audio.Tone(440, 0.2, wave="square", volume=1.0, pan=0.5))
            out = self.render(mixer, 0.2)
            self.assertLess(max(abs(x) for x in out), 1.0)
            self.assertGreater(rms(out[1::2]), rms(out[0::2]), "panned right, it's louder there")
            self.render(mixer, 0.1)
            self.assertFalse(mixer.busy)
            self.assertTrue(all(x == 0 for x in self.render(mixer, 0.05)))
            tune = audio.Tune("A4/1 r/1 A4/1", 120, sound=audio.Tone(0, 0, release=0.01), looped=True)
            self.assertAlmostEqual(tune.length, 1.5, places=5)
            mixer.play_tune(tune)
            self.render(mixer, 3.0)
            self.assertTrue(mixer.busy, "round again")
            mixer.stop()
            self.assertFalse(mixer.busy)
            with self.assertRaises(ValueError):
                mixer.set_volume(float("nan"))

    def test_a_song_fades_into_the_next_and_out_when_stopped(self):
        if not audio.available():
            self.skipTest("the library has no audio")
        band = audio.Song(330, [
            audio.Line("lute", "A4/3 G4/1 F4/2 D5/2"),
            audio.Line("recorder", "D5/4 C5/2 A4/2", gain=0.9),
            audio.Line("drone", "D3/8"),
            audio.Line("tabor", "X/3 x/2 x/1 r/2", gain=0.7),
        ])
        self.assertAlmostEqual(band.length, 8 * 60 / 330, places=5)
        mixer = audio.Mixer(self.RATE)
        mixer.play_song(band)
        first = self.render(mixer, 1.0)
        self.assertLess(rms(first[:4800]), rms(first[-9600:]), "fading in")
        mixer.play_song(audio.Song(200, [("recorder", "E5/2 G5/2")]))
        self.render(mixer, 1.3)
        self.assertTrue(mixer.busy)
        self.assertGreater(rms(self.render(mixer, 0.5)), 0.01, "the second loops on")
        mixer.stop_song()
        self.render(mixer, 1.3)
        self.assertFalse(mixer.busy)
        self.assertTrue(all(abs(x) < 1e-6 for x in self.render(mixer, 0.1)), "silent")
        mixer.close()

    def test_the_player_opens_nothing_until_it_plays(self):
        if not audio.available():
            self.skipTest("the library has no audio")
        # Stopping before anything's played starts no thread and opens no
        # output; playing would, so the tests don't.
        with audio.Audio() as player:
            player.stop_song()
            player.stop()
        with self.assertRaises(ValueError):
            player.stop()
        self.assertIsInstance(audio.plays(), bool)



class Pictures(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)

    def test_a_picture_is_sent_once_as_a_virtual_placement(self):
        image = flow.Image.rgba(20, 20, b"\xff" * (20 * 20 * 4))
        sent = picture.upload(0x123456, image, 2, 1)
        self.assertTrue(sent.startswith("\x1b_Ga=T,U=1,f=24,o=z,s=20,v=20,i=1193046,p=1,c=2,r=1,q=2,m=0;"), sent[:80])
        self.assertTrue(sent.endswith("\x1b\\"))
        png = flow.Image.png(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x01\0\0\0\0\x80")
        self.assertTrue(picture.transmit(7, png).startswith("\x1b_Ga=t,f=100,i=7,q=2,m=0;"))
        self.assertEqual(picture.delete(7), "\x1b_Ga=d,d=I,i=7,q=2\x1b\\")
        self.assertEqual(picture.delete_shown(), "\x1b_Ga=d,d=a,q=2\x1b\\")
        with self.assertRaises(ValueError):
            picture.upload(1, flow.Image.rgb(2, 2, b"\0" * 10), 1, 1)

    def test_cells_carry_the_id_the_placement_and_their_place(self):
        cell = "\U0010EEEE"
        row = picture.placeholder_row(0x010203, 2, 2)
        self.assertEqual(row, "\x1b[38;2;1;2;3m\x1b[58;2;0;0;1m" + cell + "̎̅" + cell + "̎̍" + "\x1b[39;59m")
        self.assertEqual(picture.bare_row(0x010203, 2), "\x1b[38;2;1;2;3m" + cell * 2 + "\x1b[39m")
        self.assertTrue(picture.placeholders(5, (3, 4), 1, 2).startswith("\x1b[5;4H"))
        self.assertTrue(0x100000 <= picture.first_id(42) <= 0xFFFFFF)


class Settings(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)

    def test_a_request_goes_as_base64_of_its_json_as_the_sdk_writes_it(self):
        self.assertEqual(settings.request({"do": "get", "key": "fx.style"}), "\x1b]7732;eyJkbyI6ImdldCIsImtleSI6ImZ4LnN0eWxlIn0=\x07")
        with self.assertRaises(TypeError):
            settings.request('{"do":"list"}')

    def test_an_answer_is_read_whole_among_other_input(self):
        reply = b"x\x1b]7732;eyJvayI6dHJ1ZX0=\x1b\\y"
        self.assertEqual(settings.answer(reply), {"ok": True})
        self.assertEqual(settings.answer(reply.decode("ascii")), {"ok": True})
        self.assertIsNone(settings.answer(reply[:12]), "not come whole yet")
        self.assertIsNone(settings.answer(b"\x1b]11;rgb:0/0/0\x07"))
        self.assertIsNone(settings.answer(b""))
        # An escape whose base64 isn't a JSON object isn't Flow's.
        self.assertIsNone(settings.answer(b"\x1b]7732;WzEsMl0=\x07"), "[1,2]")

    def test_each_convenience_asks_what_flow_settings_asks_and_gives_the_answers_part(self):
        asked = []
        answers = {
            "list": {"ok": True, "settings": {"fx.style": "rain", "font.size": 14}},
            "get": {"ok": True, "key": "fx.style", "value": "rain"},
            "set": {"ok": True, "key": "font.size", "value": 16},
            "reset": {"ok": True, "key": "all"},
            "reset-terminal": {"ok": True},
            "restart": {"ok": True},
        }

        def fake(request, wait=settings.WAIT):
            asked.append(request)
            return answers[request["do"]]

        with mock.patch.object(settings, "ask", fake):
            self.assertEqual(settings.list(), {"fx.style": "rain", "font.size": 14})
            self.assertEqual(settings.get("fx.style"), "rain")
            self.assertEqual(settings.set("font.size", 16), 16)
            self.assertIsNone(settings.reset("all"))
            self.assertIsNone(settings.reset_terminal())
            self.assertIsNone(settings.restart())
            with self.assertRaises(TypeError):
                settings.get(3)
        self.assertEqual(
            asked,
            [
                {"do": "list"},
                {"do": "get", "key": "fx.style"},
                {"do": "set", "key": "font.size", "value": 16},
                {"do": "reset", "key": "all"},
                {"do": "reset-terminal"},
                {"do": "restart"},
            ],
        )

    def test_a_refusal_carries_flows_reason_and_no_answer_says_so(self):
        refusal = {"ok": False, "error": "ai.model: Flow doesn't let programs change that"}
        with mock.patch.object(settings, "ask", lambda request, wait=settings.WAIT: refusal):
            with self.assertRaises(settings.Refused) as caught:
                settings.set("ai.model", "x")
        self.assertEqual(str(caught.exception), refusal["error"])
        self.assertEqual((caught.exception.message, caught.exception.answer), (refusal["error"], refusal))
        with mock.patch.object(settings, "ask", lambda request, wait=settings.WAIT: {"ok": False}):
            with self.assertRaises(settings.Refused):
                settings.get("fx.style")
        with mock.patch.object(settings, "ask", lambda request, wait=settings.WAIT: None):
            with self.assertRaises(settings.NoAnswer) as caught:
                settings.list()
        self.assertIsInstance(caught.exception, settings.SettingsError)
        self.assertIn("nothing answered", str(caught.exception))

    def test_an_answer_naming_another_request_is_no_answer_to_this_one(self):
        rain = {"ok": True, "key": "fx.style", "value": "rain"}
        self.assertTrue(settings._answers({"do": "get", "key": "fx.style"}, rain))
        self.assertFalse(settings._answers({"do": "get", "key": "font.size"}, rain))
        self.assertFalse(settings._answers({"do": "set", "key": "font.size", "value": 16}, rain))
        self.assertTrue(settings._answers({"do": "reset", "key": "all"}, {"ok": True, "key": "all"}))
        self.assertFalse(settings._answers({"do": "reset", "key": "all"}, rain))
        self.assertTrue(settings._answers({"do": "list"}, {"ok": True, "settings": {"fx.style": "rain"}}))
        self.assertFalse(settings._answers({"do": "list"}, rain))
        self.assertFalse(settings._answers({"do": "list"}, {"ok": True}))
        self.assertTrue(settings._answers({"do": "reset-terminal"}, {"ok": True}))
        self.assertFalse(settings._answers({"do": "restart"}, rain))
        # A refusal names no key, so it can't be told apart: it's taken.
        self.assertTrue(settings._answers({"do": "set", "key": "ai.model", "value": "x"}, {"ok": False, "error": "closed"}))
        with mock.patch.object(settings, "ask", lambda request, wait=settings.WAIT: rain):
            with self.assertRaises(settings.NoAnswer) as caught:
                settings.get("font.size")
            self.assertEqual(settings.get("fx.style"), "rain")
        self.assertIn("late answer", str(caught.exception))


class SettingsInATerminal(unittest.TestCase):
    """Flow's settings asked from a terminal of the test's own (a PTY) that
    answers as Flow does, one answer too late for its wait."""

    # Asks with a short wait, waits to be told its answer has come late,
    # then asks about another setting.
    CHILD = """
import os, sys
sys.path.insert(0, sys.argv[1])
from flow_sdk import settings
try:
    settings.get("fx.style", wait=0.3)
    print("first: answered", flush=True)
except settings.NoAnswer:
    print("first: none", flush=True)
os.read(int(sys.argv[2]), 1)
try:
    print("second:", settings.get("font.size", wait=5), flush=True)
except settings.SettingsError as e:
    print("second: failed:", e, flush=True)
"""

    def setUp(self):
        library_or_skip(self)
        if os.name != "posix":
            self.skipTest("on Windows the answer comes down Flow's pipe, not as keys")

    def test_an_answer_too_late_for_its_wait_is_thrown_away_not_taken_for_the_next_ones(self):
        import base64
        import pty
        import select
        import signal

        def reply(asked):
            told = {"ok": True, "key": asked["key"], "value": "value of " + asked["key"]}
            encoded = base64.b64encode(json.dumps(told).encode())
            os.write(fd, b"\x1b]7732;" + encoded + b"\x07")
            return encoded

        late, tell = os.pipe()
        os.set_inheritable(late, True)
        pid, fd = pty.fork()
        if pid == 0:
            try:
                os.execv(sys.executable, [sys.executable, "-I", "-B", "-c", self.CHILD, str(PACKAGE_ROOT), str(late)])
            finally:
                os._exit(127)
        os.close(late)
        seen, answered, echo, told_at = b"", 0, None, None
        deadline = time.monotonic() + 20
        try:
            while time.monotonic() < deadline:
                ready, _, _ = select.select([fd], [], [], 0.05)
                if ready:
                    try:
                        got = os.read(fd, 4096)
                    except OSError:
                        break  # the child has gone (EIO on Linux)
                    if not got:
                        break
                    seen += got
                # The requests the child wrote (the late answer's echo, if
                # the terminal writes ESC as itself, has no "do").
                asks = [json.loads(base64.b64decode(m)) for m in re.findall(rb"\x1b\]7732;([A-Za-z0-9+/=]*)\x07", seen)]
                asks = [a for a in asks if "do" in a]
                if answered == 0 and asks and b"first: none" in seen:
                    echo, told_at, answered = reply(asks[0]), time.monotonic(), 1
                elif answered == 1 and (echo in seen or time.monotonic() - told_at > 1):
                    # Echoed, so it waits in the child's input.
                    os.write(tell, b"x")
                    answered = 2
                elif answered == 2 and len(asks) > 1:
                    reply(asks[1])
                    answered = 3
        finally:
            os.close(tell)
            os.close(fd)
            # Gone, or going; else stopped, so the test never hangs.
            for _ in range(200):
                done, status = os.waitpid(pid, os.WNOHANG)
                if done:
                    break
                time.sleep(0.01)
            else:
                os.kill(pid, signal.SIGKILL)
                _, status = os.waitpid(pid, 0)
        shown = seen.decode("utf-8", "replace")
        self.assertIn("first: none", shown)
        self.assertIn("second: value of font.size", shown)
        self.assertEqual(status, 0, shown)

    # Its output to a file and its input from nothing, as `> file`, a pipe
    # or the shell's $(...) leave a program, its error still the terminal.
    REDIRECTED = """
import os, sys
sys.path.insert(0, sys.argv[1])
os.dup2(os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_TRUNC), 1)
os.dup2(os.open(os.devnull, os.O_RDONLY), 0)
from flow_sdk import settings
print(settings.get("fx.style", wait=3), flush=True)
"""

    def test_a_program_whose_output_and_input_are_redirected_still_asks_its_terminal(self):
        import base64
        import pty
        import select
        import signal

        with tempfile.TemporaryDirectory() as folder:
            out = os.path.join(folder, "out.txt")
            pid, fd = pty.fork()
            if pid == 0:
                try:
                    os.execv(sys.executable, [sys.executable, "-I", "-B", "-c", self.REDIRECTED, str(PACKAGE_ROOT), out])
                finally:
                    os._exit(127)
            seen, answered = b"", False
            deadline = time.monotonic() + 20
            try:
                while time.monotonic() < deadline:
                    ready, _, _ = select.select([fd], [], [], 0.05)
                    if ready:
                        try:
                            got = os.read(fd, 4096)
                        except OSError:
                            break  # the child has gone (EIO on Linux)
                        if not got:
                            break
                        seen += got
                    asked = re.search(rb"\x1b\]7732;([A-Za-z0-9+/=]*)\x07", seen)
                    if asked and not answered:
                        request = json.loads(base64.b64decode(asked.group(1)))
                        told = {"ok": True, "key": request["key"], "value": "rain"}
                        os.write(fd, b"\x1b]7732;" + base64.b64encode(json.dumps(told).encode()) + b"\x07")
                        answered = True
            finally:
                os.close(fd)
                for _ in range(200):
                    done, status = os.waitpid(pid, os.WNOHANG)
                    if done:
                        break
                    time.sleep(0.01)
                else:
                    os.kill(pid, signal.SIGKILL)
                    _, status = os.waitpid(pid, 0)
            shown = seen.decode("utf-8", "replace")
            self.assertTrue(answered, "the question reached the terminal, not the file: " + shown)
            self.assertEqual(status, 0, shown)
            with open(out) as written:
                self.assertEqual(written.read(), "rain\n")


class OutsideATerminal(unittest.TestCase):
    def setUp(self):
        library_or_skip(self)
        if flow.is_terminal():
            self.skipTest("run in a terminal, where asking would ask it")

    def test_nothing_is_asked_and_a_plain_terminal_comes_back(self):
        terminal = flow.detect(pictures=True, models=True, version=True, wait=0.05)
        self.assertEqual((terminal.caps.pictures, terminal.caps.models, terminal.caps.version), (False, False, None))
        self.assertEqual(terminal.caps.flow, flow.in_flow())
        self.assertIsNone(flow.size())
        with flow.screen() as entered:
            self.assertFalse(entered)
        if not sys.stdin.isatty():
            self.assertIsNone(flow.next_event(0))
        terminal.close()
        with self.assertRaises(ValueError):
            terminal.send("x")

    def test_flows_settings_are_asked_nothing_and_waited_for_not_at_all(self):
        if os.environ.get("FLOW_IMAGE_PIPE"):
            self.skipTest("in a Flow tab on Windows, whose pipe would answer")
        if os.name == "posix" and any(os.isatty(stream) for stream in (0, 1, 2)):
            self.skipTest("a terminal is at hand, which asking would ask")
        started = time.monotonic()
        self.assertIsNone(settings.ask({"do": "list"}, wait=5))
        with self.assertRaises(settings.NoAnswer):
            settings.get("fx.style", wait=5)
        self.assertLess(time.monotonic() - started, 1.0)

    def test_a_plain_terminal_can_do_nothing(self):
        with flow.Terminal.plain() as terminal:
            self.assertEqual(terminal.caps, flow.Caps())
            self.assertIsNone(terminal.caps.light_background)

    def test_a_background_is_judged_light_or_dark_as_the_sdk_does(self):
        self.assertTrue(flow.Caps(background=0xD6D2C4).light_background)
        self.assertFalse(flow.Caps(background=0x080C16).light_background)
        self.assertTrue(flow.is_light(0xFFFFFF))

    def run_example(self, name):
        env = {k: v for k, v in os.environ.items() if k != "TERM_PROGRAM"}
        example = PACKAGE_ROOT / "examples" / name
        ran = subprocess.run([sys.executable, "-I", "-B", str(example)], stdin=subprocess.DEVNULL, capture_output=True, env=env, timeout=60)
        said = ran.stdout.decode("utf-8", "replace")
        self.assertEqual(ran.returncode, 0, said + ran.stderr.decode("utf-8", "replace"))
        self.assertNotIn("\x1b", said)
        return said

    def test_the_example_falls_back_quietly_and_exits_0(self):
        self.assertIn("the gem (8 triangles,", self.run_example("hello.py"))

    def test_the_scene_example_says_what_goes_unseen_and_unheard(self):
        said = self.run_example("scene.py")
        self.assertIn("the cube (6 triangles a frame) goes unseen", said)
        self.assertIn("its dance (3.2 s) unheard" if audio.available() else "no audio in this build", said)


class EventsReadAsPythonHasThem(unittest.TestCase):
    def test_keys_clicks_and_resizes(self):
        raw = _native.FlowEvent(kind=1, key=ord("c"), mods=2)
        self.assertEqual(flow.term._event(raw), flow.Event("key", key="c", mods=frozenset({"ctrl"})))
        self.assertTrue(flow.term._event(raw).is_quit)
        self.assertEqual(flow.term._event(_native.FlowEvent(kind=1, key=0x110101 + 11)).key, "f12")
        self.assertEqual(flow.term._event(_native.FlowEvent(kind=1, key=13)).key, "enter")
        click = flow.term._event(_native.FlowEvent(kind=2, col=7, row=3, button=1))
        self.assertEqual((click.kind, click.col, click.row, click.button, click.is_quit), ("click", 7, 3, "right", False))
        self.assertEqual(flow.term._event(_native.FlowEvent(kind=3, col=120, row=40)), flow.Event("resize", col=120, row=40))


if __name__ == "__main__":
    unittest.main()
