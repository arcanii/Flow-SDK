/*
 * scene.c: a scene from C. A cube turning about two axes is drawn each
 * frame through a camera that stays put, and sent over the window's cells
 * as one model (a stage), while a dance for a lute, a drone and a tabor
 * plays where the machine plays sound. A click ripples Flow's liquid where
 * it lands; any key ends it. Outside a terminal, or in one that shows no 3D
 * models, it says what goes unseen (and unheard) and stops.
 *
 *   cargo build -p flow-sdk-c
 *   cc c/examples/scene.c -I c/include -L target/debug -lflow_sdk_c \
 *      -Wl,-rpath,target/debug -o scene && ./scene
 */

#include <stdio.h>
#include <string.h>

#include "flow_sdk.h"

/* Each face: which way it faces and the two ways across it, and its color
 * as 0xRRGGBB. */
static const float FACES[6][3][3] = {
    {{1.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {0.0f, 1.0f, 0.0f}},
    {{-1.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {0.0f, 1.0f, 0.0f}},
    {{0.0f, 1.0f, 0.0f}, {1.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}},
    {{0.0f, -1.0f, 0.0f}, {1.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}},
    {{0.0f, 0.0f, 1.0f}, {1.0f, 0.0f, 0.0f}, {0.0f, 1.0f, 0.0f}},
    {{0.0f, 0.0f, -1.0f}, {1.0f, 0.0f, 0.0f}, {0.0f, 1.0f, 0.0f}},
};
static const unsigned COLORS[6] = {0xe8231c, 0x2a9df4, 0xf4f4f4, 0xf2c12e, 0x7ce7ff, 0xb04ad0};

/* Where the cube is turned to when it starts: three of its faces in view. */
#define START 1.0f

/* The cube as it's turned `t` seconds in, in a picture `width` × `height`
 * pixels: each corner and each face's way turned as the cube is, for the
 * camera to turn into the picture's pixels and leave out the faces turned
 * away from it. */
static FlowScene *cube(float t, float width, float height) {
    FlowScene *scene = flow_scene_new(width, height);
    const float target[3] = {0.0f, 0.0f, 0.0f};
    FlowCamera *camera = flow_camera_orbit(target, 0.0f, 0.35f, 6.0f, 0.7f, height);
    /* About y, after about x. */
    const float up[3] = {0.0f, 1.0f, 0.0f}, across[3] = {1.0f, 0.0f, 0.0f};
    float spin[9], turn[9];
    flow_m3_rotation(up, t * 0.8f, spin);
    flow_m3_rotation(across, t * 0.5f, turn);
    flow_m3_compose(spin, turn, turn);
    const float square[4][2] = {{-1.0f, -1.0f}, {1.0f, -1.0f}, {1.0f, 1.0f}, {-1.0f, 1.0f}};
    for (int f = 0; f < 6; f++) {
        const float *n = FACES[f][0], *u = FACES[f][1], *v = FACES[f][2];
        float points[12];
        for (int i = 0; i < 4; i++) {
            /* n + u·a + v·b, turned. */
            float along[3], corner[3];
            flow_v3_mul(u, square[i][0], along);
            flow_v3_mul(v, square[i][1], corner);
            flow_v3_add(along, corner, corner);
            flow_v3_add(n, corner, corner);
            flow_m3_apply(turn, corner, &points[i * 3]);
        }
        float normal[3];
        flow_m3_apply(turn, n, normal);
        float color[4];
        flow_rgb(COLORS[f], color);
        /* Each face shows its color as given, whichever way it's turned
         * under Flow's light. */
        float turned[3];
        if (flow_camera_turn(camera, normal, turned) == 0) {
            flow_lit_as(color, turned, color);
        }
        flow_camera_face(camera, scene, color, points, 4, normal);
    }
    flow_camera_free(camera);
    return scene;
}

/* Adds a line to the song, saying why if its notes don't read. */
static bool add_line(FlowSong *song, int32_t instrument, float gain, const char *notes) {
    char *error = NULL;
    if (flow_song_line(song, instrument, gain, notes, &error) == 0) {
        return true;
    }
    fprintf(stderr, "The dance doesn't read: %s\n", error != NULL ? error : "refused");
    flow_free(error);
    return false;
}

/* The dance: eight beats at 150 a minute, or NULL in a build without audio. */
static FlowSong *dance(void) {
    FlowSong *song = flow_song_new(150.0f);
    if (song == NULL) {
        return NULL;
    }
    if (!add_line(song, FLOW_INSTRUMENT_LUTE, 1.0f, "D4/1 F4/1 A4/2 G4/1 F4/1 E4/2") || !add_line(song, FLOW_INSTRUMENT_DRONE, 1.0f, "D3/8")
        || !add_line(song, FLOW_INSTRUMENT_TABOR, 0.7f, "X/2 x/1 x/1 X/2 x/2")) {
        flow_song_free(song);
        return NULL;
    }
    return song;
}

/* Sends a string just made as graphics, or as effects, and frees it. */
static void send(FlowTerminal *terminal, char *escape, bool effects) {
    if (escape == NULL) {
        return;
    }
    if (effects) {
        flow_terminal_send_effects(terminal, escape, strlen(escape));
    } else {
        flow_terminal_send(terminal, escape, strlen(escape));
    }
    flow_free(escape);
}

/* A stage for the window as it is now, with the words below it; NULL if
 * the window's too small. */
static FlowStage *stage_now(FlowTerminal *terminal, const FlowCaps *caps) {
    FlowSize size = {0};
    if (flow_term_size(&size) != 0) {
        return NULL;
    }
    if (size.cell_width <= 0.0f) {
        flow_terminal_cell_size(terminal, &size.cell_width, &size.cell_height);
    }
    FlowStage *stage = flow_stage_new(&size, caps->cell_width, caps->cell_height, 1);
    flow_write("\x1b[0m\x1b[2J");
    FlowStageLayout layout;
    if (stage != NULL && flow_stage_layout(stage, &layout) == 0) {
        char words[96];
        snprintf(words, sizeof words, "\x1b[%u;1Hclick to ripple the liquid, any key ends it", (unsigned)layout.below + 1);
        flow_write(words);
    }
    return stage;
}

int main(void) {
    if (flow_abi_version() != FLOW_SDK_ABI) {
        fprintf(stderr, "flow_sdk.h and the library it's linked with don't match.\n");
        return 1;
    }
    FlowSong *song = dance();

    /* Raw keys on the alternate screen before asking, so the answers
     * aren't echoed, and the mouse's clicks; it fails quietly outside a
     * terminal. */
    bool screen = flow_term_enter(true) == 0;
    /* Clicks are wanted, not every move of the mouse: ?1003 off leaves
     * ?1000's clicks and ?1002's drags. */
    if (screen) {
        flow_write("\x1b[?1003l");
    }
    /* Models only through the terminal: on Windows, Flow writes a model's
     * cells where ConPTY doesn't know of them. */
    FlowTerminal *terminal = flow_detect(FLOW_ASK_MODELS | FLOW_ASK_CELL | FLOW_ASK_NO_PIPE, 0);
    FlowCaps caps = {0};
    flow_terminal_caps(terminal, &caps);
    FlowStage *stage = screen && caps.models ? stage_now(terminal, &caps) : NULL;

    if (stage == NULL) {
        if (screen) {
            flow_term_leave();
        }
        FlowScene *first = cube(START, 800.0f, 480.0f);
        printf("This terminal shows no 3D models here, so the cube (%zu triangles a frame) goes unseen", flow_scene_triangles(first));
        if (song != NULL) {
            printf(", and its dance (%.1f s) unheard.\n", (double)flow_song_length(song));
        } else {
            printf(", with no audio in this build.\n");
        }
        flow_scene_free(first);
        flow_song_free(song);
        flow_terminal_free(terminal);
        return 0;
    }

    FlowAudio *audio = song != NULL && flow_audio_plays() ? flow_audio_new() : NULL;
    if (audio != NULL) {
        flow_audio_set_volume(audio, 0.6f);
        flow_audio_play_song(audio, song);
    }

    float t = START;
    for (;;) {
        FlowStageLayout layout;
        if (stage != NULL && flow_stage_layout(stage, &layout) == 0) {
            FlowScene *scene = cube(t, layout.width, layout.height);
            send(terminal, flow_stage_show(stage, scene), false);
            flow_scene_free(scene);
        }
        /* A frame every 33 ms, sooner for a click. */
        FlowEvent event;
        int got = flow_next_event(33, &event);
        if (got < 0 || (got == 1 && event.kind == FLOW_EVENT_KEY)) {
            break;
        }
        if (got == 1 && event.kind == FLOW_EVENT_RESIZE) {
            if (stage != NULL) {
                send(terminal, flow_stage_clear(stage), false);
                flow_stage_free(stage);
            }
            stage = stage_now(terminal, &caps);
        }
        float at[2];
        if (got == 1 && event.kind == FLOW_EVENT_CLICK && caps.flow && stage != NULL && flow_stage_point_at(stage, event.col, event.row, &at[0], &at[1]) == 0) {
            FlowEffect *ripple = flow_stage_effect(stage, FLOW_FX_RIPPLE, at, NULL, NULL, 160.0f, 0.6f);
            flow_effect_sound(ripple, FLOW_SOUND_DROP);
            send(terminal, flow_effect_escape(ripple), true);
            flow_effect_free(ripple);
        }
        t += 0.033f;
    }

    if (stage != NULL) {
        send(terminal, flow_stage_clear(stage), false);
        flow_stage_free(stage);
    }
    flow_term_leave();
    flow_audio_free(audio);
    flow_song_free(song);
    flow_terminal_free(terminal);
    return 0;
}
