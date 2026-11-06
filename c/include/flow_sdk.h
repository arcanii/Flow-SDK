/*
 * flow_sdk.h: Flow's SDK for C (the flow-sdk-c crate, c/ in the Flow-SDK
 * repository). What Flow adds over a plain console, for any program that
 * runs in it: pictures (the kitty graphics protocol), 3D models (the Ratty
 * graphics protocol), meshes made in code, whole scenes worked out each
 * frame (with the vector maths to work them out) and fitted to the cells,
 * screens of cells sent as their difference from the last, with pictures
 * among the letters, the liquid's effects and Flow's sounds (OSC 7731),
 * DECPS notes, Flow's settings seen and changed (OSC 7732), and audio of
 * the program's own (songs for a small band, played on the machine).
 * README.md explains each escape.
 *
 * Build the library with `cargo build --release -p flow-sdk-c`, which
 * leaves libflow_sdk_c.dylib (macOS), libflow_sdk_c.so (Linux) or
 * flow_sdk_c.dll (Windows), and the static libflow_sdk_c.a, in
 * target/release; `--no-default-features` builds it without audio. Then,
 * on macOS or Linux:
 *
 *     cc hello.c -I c/include -L target/release -lflow_sdk_c \
 *        -Wl,-rpath,target/release -o hello
 *
 * Ownership, for every function here:
 *
 *   - A `char *` or `uint8_t *` that a function returns is the caller's,
 *     and goes back to flow_free() once used. Strings are NUL-terminated
 *     UTF-8 escapes, ready to write; a buffer's length comes back through
 *     a `size_t *`. NULL comes back for arguments refused (a NULL handle, a
 *     format there's none of, pixels that don't fit their size).
 *   - Handles (FlowTerminal, FlowMesh, FlowEffect, FlowBudget, FlowScene,
 *     FlowCamera, FlowStage, FlowScreen, FlowTune, FlowSong, FlowAudio,
 *     FlowMixer) come
 *     from their `_new` (or flow_detect, flow_camera_orbit,
 *     flow_stage_effect) and go back to their own `_free`, never
 *     flow_free(). Each `_free` takes NULL and does nothing.
 *   - Pointers passed in are read during the call only and never kept. A
 *     pointer with a length may be NULL when the length is 0.
 *   - Functions that return `int` give 0 for done and -1 for refused or
 *     failed, unless they say otherwise.
 *   - Nothing unwinds out of the library: a failure inside comes back as
 *     NULL or -1.
 *
 * The library writes to standard output through its own buffer and flushes
 * it each time, so fflush(stdout) first when printf's text has to come
 * before it. None of it is safe to call from two threads at once on the
 * same handle.
 */

#ifndef FLOW_SDK_H
#define FLOW_SDK_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The interface's version. flow_abi_version() returns the library's, which
 * must match the header it's used with. */
#define FLOW_SDK_ABI 1

uint32_t flow_abi_version(void);

/* Frees a string or buffer a flow_ function returned. NULL does nothing. */
void flow_free(void *ptr);

/* ------------------------------------------------------------------------
 * The terminal
 * ------------------------------------------------------------------------ */

/* Whether the program runs in Flow, by the name Flow gives its shells
 * (TERM_PROGRAM=flow-terminal). */
bool flow_in_flow(void);

/* Whether standard input and output are both a terminal. Outside one, the
 * functions below that need one do nothing and return -1, and flow_detect
 * asks nothing (on macOS and Linux). */
bool flow_is_terminal(void);

/* Raw keys on or off, keeping the screen as it is: for asking the terminal
 * below the prompt. -1 outside a terminal. */
int flow_raw_mode(bool on);

/* Raw keys, the alternate screen, no cursor or line wrapping, a clear
 * screen, and with `mouse` the mouse's clicks, drags and moves. -1 outside
 * a terminal. flow_term_leave() hands it all back as it was. */
int flow_term_enter(bool mouse);
void flow_term_leave(void);

/* The terminal's size in cells, and a cell's size in pixels where the
 * terminal tells (0 where it doesn't: ConPTY on Windows never does). */
typedef struct FlowSize {
    uint16_t cols;
    uint16_t rows;
    float cell_width;
    float cell_height;
} FlowSize;

int flow_term_size(FlowSize *out);

/* Which questions flow_detect asks; DA1 always goes last, so a terminal
 * that knows none of them still ends the wait at once. */
#define FLOW_ASK_PICTURES   1u  /* kitty's query: does it show pictures? */
#define FLOW_ASK_MODELS     2u  /* Ratty's `s`: does it show 3D models? */
#define FLOW_ASK_CELL       4u  /* CSI 16 t: a cell's whole pixels */
#define FLOW_ASK_AREA       8u  /* CSI 14 t: the text area's pixels */
#define FLOW_ASK_VERSION   16u  /* XTVERSION: which terminal it is */
#define FLOW_ASK_NO_PIPE   32u  /* on Windows, ask nothing down Flow's pipe */
#define FLOW_ASK_BACKGROUND 64u  /* OSC 11: its background color */
#define FLOW_ASK_EVERYTHING 95u

/* The terminal as asked: what it can do, and where pictures go. */
typedef struct FlowTerminal FlowTerminal;

/* Asks the terminal once, waiting `wait_ms` at most for one that answers
 * nothing (0 for a second). Turn raw mode on first (flow_raw_mode or
 * flow_term_enter), or the terminal echoes its answers, and ask before
 * anything else reads the keys. Outside a terminal on macOS and Linux it
 * asks nothing. Never NULL but for want of memory; free it with
 * flow_terminal_free. */
FlowTerminal *flow_detect(uint32_t ask, uint32_t wait_ms);

/* A terminal that can do nothing, as one that answers nothing. */
FlowTerminal *flow_terminal_plain(void);
void flow_terminal_free(FlowTerminal *terminal);

/* What the terminal said it can do. */
typedef struct FlowCaps {
    bool pictures;          /* kitty pictures */
    bool models;            /* Ratty models */
    bool flow;              /* it's Flow, by XTVERSION or by name */
    bool bare;              /* picture cells go bare (flow_picture_bare_row) */
    bool piped;             /* pictures go down Flow's pipe (Windows) */
    uint16_t cell_width;    /* a cell's whole pixels (CSI 16 t), 0 if not said */
    uint16_t cell_height;
    uint32_t area_width;    /* the text area's pixels (CSI 14 t), 0 if not said */
    uint32_t area_height;
} FlowCaps;

int flow_terminal_caps(const FlowTerminal *terminal, FlowCaps *out);

/* XTVERSION's answer ("Flow 0.1.24", say), or NULL if it gave none.
 * Free it with flow_free. */
char *flow_terminal_version(const FlowTerminal *terminal);

/* The background OSC 11 gave (FLOW_ASK_BACKGROUND), as 0xRRGGBB in `rgb`;
 * -1 if it gave none, as Flow's pipe on Windows didn't before 0.1.28. A
 * program that sets its own colors can lay a ground of its own only where
 * it's light, or unknown (flow_screen_ground). */
int flow_terminal_background(const FlowTerminal *terminal, uint32_t *rgb);

/* Whether 0xRRGGBB is light: past the middle of the eye's brightness, so
 * dark letters suit it better than light ones. */
bool flow_is_light(uint32_t rgb);

/* A cell's size in pixels now (after a resize, say), to a fraction of a
 * pixel where the terminal tells; -1 if it doesn't. */
int flow_terminal_cell_size(FlowTerminal *terminal, float *width, float *height);

/* Sends graphics commands (pictures and models): down Flow's pipe on
 * Windows, waiting until Flow has them, so they're in before the cells
 * that show them; elsewhere to standard output. */
int flow_terminal_send(FlowTerminal *terminal, const void *bytes, size_t len);

/* Sends effects and sounds: down Flow's pipe on Windows, else to standard
 * output. Send them after the screen that shows what set them off. */
int flow_terminal_send_effects(FlowTerminal *terminal, const void *bytes, size_t len);

/* Writes a string to standard output and flushes it. */
int flow_write(const char *text);

/* flow_write, then flow_free: for a string just made, as in
 * flow_write_free(flow_picture_delete(id)). -1 for NULL. */
int flow_write_free(char *text);

/* ------------------------------------------------------------------------
 * Keys, clicks and resizes
 * ------------------------------------------------------------------------ */

#define FLOW_EVENT_KEY    1  /* a key went down or repeated */
#define FLOW_EVENT_CLICK  2  /* a mouse button went down */
#define FLOW_EVENT_RESIZE 3  /* the terminal's new size is col × row */
#define FLOW_EVENT_OTHER  4  /* anything else: moves, releases, pastes, focus */

/* Keys as a terminal sends them, or past Unicode for those without a
 * character. */
#define FLOW_KEY_TAB        9u
#define FLOW_KEY_ENTER     13u
#define FLOW_KEY_ESCAPE    27u
#define FLOW_KEY_BACKSPACE 127u
#define FLOW_KEY_OTHER     0x110000u
#define FLOW_KEY_LEFT      0x110001u
#define FLOW_KEY_RIGHT     0x110002u
#define FLOW_KEY_UP        0x110003u
#define FLOW_KEY_DOWN      0x110004u
#define FLOW_KEY_HOME      0x110005u
#define FLOW_KEY_END       0x110006u
#define FLOW_KEY_PAGE_UP   0x110007u
#define FLOW_KEY_PAGE_DOWN 0x110008u
#define FLOW_KEY_DELETE    0x110009u
#define FLOW_KEY_INSERT    0x11000Au
#define FLOW_KEY_BACKTAB   0x11000Bu
#define FLOW_KEY_F1        0x110101u  /* F2 to F24 follow on */

#define FLOW_MOD_SHIFT 1u
#define FLOW_MOD_CTRL  2u
#define FLOW_MOD_ALT   4u

typedef struct FlowEvent {
    int32_t kind;      /* FLOW_EVENT_* */
    uint32_t key;      /* a key's character as a code point, or FLOW_KEY_* */
    uint32_t mods;     /* FLOW_MOD_* held with the key */
    uint16_t col;      /* where a click went down, from 0; a resize's cells */
    uint16_t row;
    uint32_t button;   /* a click's: 0 left, 1 right, 2 middle */
} FlowEvent;

/* The next event, waiting `timeout_ms` at most: 1 when `out` holds one, 0
 * when the time ran out, -1 outside a terminal. Ctrl+C in raw mode comes as
 * the key 'c' with FLOW_MOD_CTRL. */
int flow_next_event(uint32_t timeout_ms, FlowEvent *out);

/* ------------------------------------------------------------------------
 * Pictures (the kitty graphics protocol)
 *
 * A picture is sent once as a virtual placement and shown by placeholder
 * cells: U+10EEEE in the picture's id as a color, which scroll and clear as
 * text does. Ids name pictures in the whole tab, so pick them with
 * flow_picture_id; cells number 297 rows and columns at most.
 * ------------------------------------------------------------------------ */

#define FLOW_PIXELS_RGB   24  /* 3 bytes a pixel */
#define FLOW_PIXELS_RGBA  32  /* 4 bytes a pixel, straight alpha */
#define FLOW_PIXELS_PNG  100  /* a PNG file's bytes */

/* The placement a picture gets unless the program picks another. */
#define FLOW_PLACEMENT 1u

typedef struct FlowImage {
    int32_t format;        /* FLOW_PIXELS_* */
    uint32_t width;        /* in pixels; read from the file for a PNG */
    uint32_t height;
    const uint8_t *data;   /* rows top first; len is width × height × 3 or 4 */
    size_t len;
} FlowImage;

/* An id to start the program's pictures from, picked by `seed` (the
 * process's id, say) in the 24 bits a cell's color holds. Count up from it
 * for more, staying at or under 0xFFFFFF. */
uint32_t flow_picture_id(uint32_t seed);

/* The commands that send `image` as picture `id`, shown by placement
 * `placement` over `cols` × `rows` cells. Sending an id again replaces it.
 * RGBA that's opaque throughout goes as RGB. */
char *flow_picture_upload(uint32_t id, const FlowImage *image, uint32_t placement, uint16_t cols, uint16_t rows);

/* The commands that send `image` and only keep it (a=t), for a program
 * that places it with its own commands. */
char *flow_picture_transmit(uint32_t id, const FlowImage *image);

/* Forgets picture `id` and its pixels. */
char *flow_picture_delete(uint32_t id);

/* Takes away every picture the screen in use shows, keeping the
 * scrollback's and the other screen's. */
char *flow_picture_delete_shown(void);

/* Row `row` of a picture's cells, `cols` wide from its column `from`: write
 * it where the row goes. */
char *flow_picture_placeholder_row(uint32_t id, uint32_t placement, uint16_t row, uint16_t from, uint16_t cols);

/* All of a picture's cells, `cols` × `rows` with its top-left at screen
 * cell (`col`, `row`), counting from 0, each row placed by the cursor. */
char *flow_picture_placeholders(uint32_t id, uint32_t placement, uint16_t col, uint16_t row, uint16_t cols, uint16_t rows);

/* A row of bare cells (FlowCaps.bare, through ConPTY on Windows): the id's
 * color and the character alone. Each row of a taller picture is then a
 * picture of its own. */
char *flow_picture_bare_row(uint32_t id, uint16_t cols);

/* All of it: sends `image` as picture `id` where `terminal` takes pictures
 * and writes the cells that show it over `cols` × `rows` cells, with its
 * top-left at (`col`, `row`), or at the cursor, a line each, if either is
 * negative. On bare cells each row is a picture of its own, ids `id` to
 * `id + rows - 1` (and a PNG is refused). Returns how many ids it took, or
 * -1. Check FlowCaps.pictures first. */
int flow_picture_show(FlowTerminal *terminal, uint32_t id, const FlowImage *image, uint16_t cols, uint16_t rows, int32_t col, int32_t row);

/* ------------------------------------------------------------------------
 * 3D models (the Ratty graphics protocol)
 *
 * An object is registered by an id, then placed centered on a cell and
 * fitted to a block of cells by its box, with a tenth's margin. Placing an
 * id again moves it; registering it again draws the new model in its place.
 * Flow takes models up to 64 MB, placements up to 200 cells a side.
 * ------------------------------------------------------------------------ */

#define FLOW_MODEL_OBJ 0
#define FLOW_MODEL_GLB 1

/* The scale that undoes the tenth's margin, so a model's box is its cells. */
#define FLOW_MODEL_FILL (1.0f / 0.9f)

/* How a placed object looks. flow_look_init sets every field to "leave as
 * it was" (-1 or NAN); set those wanted. */
typedef struct FlowLook {
    int32_t animate;     /* 1 spins, tumbles and bobs it; 0 holds it still */
    int32_t color;       /* 0xRRGGBB tints an OBJ (a glTF has its own) */
    float scale;
    float depth;         /* thickness for flat artwork */
    float brightness;
    float offset[3];     /* pixels, x right, y up; z nearer or further */
    float rotation[3];   /* degrees about x, y and z, turned in that order */
    float scale3[3];
} FlowLook;

void flow_look_init(FlowLook *look);

/* Registers a model's bytes (FLOW_MODEL_*) as object `id`, in one command.
 * `name` is the file's name for the terminal's messages, or NULL; a name or
 * path holding ';' or a control character is refused. */
char *flow_model_register(uint32_t id, int32_t format, const uint8_t *bytes, size_t len, const char *name);

/* The same in commands of `chunk` bytes at most (0 for one; 3072 is 4 KB
 * of base64), each saying whether more follow. */
char *flow_model_register_chunks(uint32_t id, int32_t format, const uint8_t *bytes, size_t len, const char *name, size_t chunk);

/* Registers the file at `path`, which Flow reads once the user allows it
 * (a relative path is from the shell's folder). */
char *flow_model_register_path(uint32_t id, int32_t format, const char *path);

/* Places object `id` centered on screen cell (`row`, `col`), counting from
 * 0, over `cols` × `rows` cells. `look` may be NULL. */
char *flow_model_place(uint32_t id, uint16_t row, uint16_t col, uint16_t cols, uint16_t rows, const FlowLook *look);

/* Places it over the `cols` × `rows` cells whose top-left is (`top`,
 * `left`): centered on the middle cell, or the one past the middle. */
char *flow_model_place_over(uint32_t id, uint16_t top, uint16_t left, uint16_t cols, uint16_t rows, const FlowLook *look);

/* Changes how placed object `id` looks, without placing it again. */
char *flow_model_update(uint32_t id, const FlowLook *look);

/* Deletes object `id` and takes its placement away; or every object the
 * program's tab holds. */
char *flow_model_delete(uint32_t id);
char *flow_model_delete_all(void);

/* ------------------------------------------------------------------------
 * Meshes: objects built in code, for the terminal to turn and light
 *
 * Points are x, y, z with y up, in any units: the terminal fits the object
 * to its cells. Each face is flat and convex and faces away from `inside`
 * (the object's middle, for one that's solid), whichever way its points
 * go round. Colors are red, green, blue and alpha, 0 to 1.
 *
 * A face (or a scene's polygon, below) is in a color, matte and lit, or in
 * a FlowMaterial: the color polished, metal, glowing, under a clear coat,
 * or unlit, as glTF's materials have them and Flow draws them. A material
 * of a color alone is written exactly as the color is.
 * ------------------------------------------------------------------------ */

typedef struct FlowMesh FlowMesh;
typedef struct FlowEffect FlowEffect;  /* the liquid's effects, below */

typedef struct FlowMaterial {
    float color[4];       /* red, green, blue and alpha, 0 to 1 as shown */
    float gloss;          /* 0 matte (glTF's own default) to 1 polished */
    float metal;          /* 0 to 1: a metal's reflections take its color */
    float glow[3];        /* the light it gives off, as shown; black for none */
    float glow_strength;  /* 1 as given (glTF's emissive strength) */
    float coat;           /* a clear coat over it, 0 (none) to 1 */
    float coat_gloss;     /* how polished the coat is, 0 to 1 */
    bool unlit;           /* its color alone, no light falling on it */
} FlowMaterial;

/* Makes `material` the color `color` alone, matte and lit; set the rest
 * after. Every number has to be finite where a material is used. */
int flow_material_init(FlowMaterial *material, const float color[4]);

FlowMesh *flow_mesh_new(void);
void flow_mesh_free(FlowMesh *mesh);

/* A face of `count` points (3 floats each), 3 at least. */
int flow_mesh_face(FlowMesh *mesh, const float color[4], const float *points, size_t count, const float inside[3]);
int flow_mesh_face_in(FlowMesh *mesh, const FlowMaterial *material, const float *points, size_t count, const float inside[3]);
int flow_mesh_triangle(FlowMesh *mesh, const float color[4], const float a[3], const float b[3], const float c[3], const float inside[3]);
size_t flow_mesh_triangles(const FlowMesh *mesh);

/* The object as GLB bytes for flow_model_register, `*len` of them. Free
 * them with flow_free. */
uint8_t *flow_mesh_glb(const FlowMesh *mesh, size_t *len);

/* The same with its points and normals packed into whole numbers (glTF's
 * KHR_mesh_quantization, which the file requires): half the size, for
 * Flow (0.1.27 on), which reads them, where not every terminal does. */
uint8_t *flow_mesh_glb_packed(const FlowMesh *mesh, size_t *len);

/* ------------------------------------------------------------------------
 * Scenes: a picture worked out afresh each frame, for the terminal to draw
 *
 * A scene is in the picture's pixels as the screen shows them, from its
 * middle, x right and y up; z only orders what's in front, nearer being
 * more. Polygons are flat and convex, cut to the picture, and wound as
 * Flow lights them whichever way their corners go round. Placements can't
 * overlap, so a scene of several things moving about is one object, sent
 * anew each frame: a stage, below, does that. Colors are red, green, blue
 * and alpha, 0 to 1; a polygon of alpha 0 isn't drawn.
 * ------------------------------------------------------------------------ */

typedef struct FlowScene FlowScene;

/* A picture `width` × `height` pixels (a stage's FlowStageLayout gives
 * them); NULL unless both are above 0. */
FlowScene *flow_scene_new(float width, float height);
void flow_scene_free(FlowScene *scene);

/* A polygon of `count` corners (3 at least), 6 floats each: where the
 * corner is (x, y, z) and which way the surface faces there (x, y, z, z
 * toward the viewer), so a curved surface shades smoothly. */
int flow_scene_polygon(FlowScene *scene, const float color[4], const float *corners, size_t count);
int flow_scene_polygon_in(FlowScene *scene, const FlowMaterial *material, const float *corners, size_t count);

/* A polygon of `count` points (3 floats each) facing `normal` all over. */
int flow_scene_flat(FlowScene *scene, const float color[4], const float *points, size_t count, const float normal[3]);
int flow_scene_flat_in(FlowScene *scene, const FlowMaterial *material, const float *points, size_t count, const float normal[3]);
size_t flow_scene_triangles(const FlowScene *scene);

/* The scene as GLB bytes, `*len` of them, its points stretched by
 * (`squash_x`, `squash_y`), 1 and 1 to leave them, and named as
 * `generator` made it (NULL for "flow-sdk"). A stage does this itself;
 * free them with flow_free. */
uint8_t *flow_scene_glb(const FlowScene *scene, float squash_x, float squash_y, const char *generator, size_t *len);

/* The same packed into whole numbers, as flow_mesh_glb_packed is. */
uint8_t *flow_scene_glb_packed(const FlowScene *scene, float squash_x, float squash_y, const char *generator, size_t *len);

/* A camera in a world whose y is up, seeing it in perspective: looking at
 * `target` from `distance` away, turned `yaw` radians to the right of
 * looking from in front (+z) and `pitch` radians above it, seeing `fov`
 * radians up and down in a picture `height` pixels tall. Make one each
 * frame it moves. */
typedef struct FlowCamera FlowCamera;

FlowCamera *flow_camera_orbit(const float target[3], float yaw, float pitch, float distance, float fov, float height);
void flow_camera_free(FlowCamera *camera);

/* Where point `p` of the world shows in the picture, into `out`: x, y and
 * a depth that grows as one over the distance does. -1 if it's behind the
 * eye. */
int flow_camera_project(const FlowCamera *camera, const float p[3], float out[3]);

/* Direction `n` of the world as the picture has it, into `out`: x right,
 * y up, z toward the eye. */
int flow_camera_turn(const FlowCamera *camera, const float n[3], float out[3]);

/* A flat face of the world, `count` points (3 floats each) turned
 * `normal` way, drawn into `scene` if it faces the eye and is all in front
 * of it; 0 whether or not it was. */
int flow_camera_face(const FlowCamera *camera, FlowScene *scene, const float color[4], const float *points, size_t count, const float normal[3]);
int flow_camera_face_in(const FlowCamera *camera, FlowScene *scene, const FlowMaterial *material, const float *points, size_t count, const float normal[3]);

/* The color to give a face turned `normal` way (x right, y up, z toward
 * the viewer) for it to show as `color` once Flow's light (from above
 * left) falls on it, into `out`, which may be `color` itself. Other
 * terminals may light models otherwise. */
int flow_lit_as(const float color[4], const float normal[3], float out[4]);

/* The vector maths a scene is worked out with. Points and directions are
 * three floats, x, y and z; a rotation is nine, by rows; a color four.
 * Each result goes into `out`, which may be one of the arguments: they're
 * read before it's written. NULL is refused, -1 (NAN for a number). */

int flow_v3_add(const float a[3], const float b[3], float out[3]);
int flow_v3_sub(const float a[3], const float b[3], float out[3]);       /* a less b */
int flow_v3_mul(const float a[3], float k, float out[3]);                /* a times k */
float flow_v3_dot(const float a[3], const float b[3]);
int flow_v3_cross(const float a[3], const float b[3], float out[3]);     /* the right-hand way */

/* `a` made a length of 1; (0, 0, 1) for one too short to have a way. */
int flow_v3_normalize(const float a[3], float out[3]);

/* The point `t` of the way from `a` to `b`. */
int flow_v3_lerp(const float a[3], const float b[3], float t, float out[3]);

/* Eases from 0 to 1 as `t` goes from `a` to `b`, still at both ends. */
float flow_smooth(float a, float b, float t);

/* No turn at all: float m[9] = FLOW_M3_IDENTITY; */
#define FLOW_M3_IDENTITY {1.0f, 0.0f, 0.0f, 0.0f, 1.0f, 0.0f, 0.0f, 0.0f, 1.0f}

/* Turning by `angle` radians about `axis`, the right-hand way. */
int flow_m3_rotation(const float axis[3], float angle, float out[9]);

/* Rotation `m` turning `v`. */
int flow_m3_apply(const float m[9], const float v[3], float out[3]);

/* Turning by `b` and then by `a`, as one rotation. */
int flow_m3_compose(const float a[9], const float b[9], float out[9]);

/* 0xRRGGBB as red, green, blue and alpha (1), 0 to 1. */
int flow_rgb(uint32_t rgb, float out[4]);

/* The color `t` of the way from `a` to `b`, alpha too. */
int flow_mix(const float a[4], const float b[4], float t, float out[4]);

/* How much of Flow's light a face turned `normal` way gets (x right, y up,
 * z toward the viewer): what flow_lit_as divides by. */
float flow_shade(const float normal[3]);

/* ------------------------------------------------------------------------
 * Stages: a scene over the cells, sent each frame
 *
 * A stage takes as many of the terminal's cells as one placement may cover
 * in Flow (200 a side, 4096 pixels a side, about 8 million pixels),
 * centered above the rows the program keeps for its words. Each frame's
 * scene goes as the same object, which Flow draws in place of the last;
 * the first is placed too, scaled by FLOW_MODEL_FILL, and a frame like the
 * last isn't sent at all. The scene is squashed from the screen's pixels
 * into the terminal's picture, a cell's whole pixels per cell.
 * ------------------------------------------------------------------------ */

typedef struct FlowStage FlowStage;

/* Where a stage is, in cells counted from 0, and its picture's size in
 * the screen's pixels: a scene for it is `width` × `height`. */
typedef struct FlowStageLayout {
    uint16_t left;
    uint16_t top;
    uint16_t cols;
    uint16_t rows;
    uint16_t below;     /* the first row below the picture, for words */
    float width;
    float height;
} FlowStageLayout;

/* The stage for a terminal of `size` (flow_term_size's, with a cell's
 * pixels from flow_terminal_cell_size where it gave none) whose cells have
 * `cell_width` × `cell_height` whole pixels (FlowCaps' cell, 0 if not
 * said), keeping `words` rows below it. Its object is id 1. NULL if it
 * would be under 16 × 6 cells, or no cell size is known at all: make a
 * new one after a resize. */
FlowStage *flow_stage_new(const FlowSize *size, uint16_t cell_width, uint16_t cell_height, uint16_t words);
void flow_stage_free(FlowStage *stage);

/* Its object's id in place of 1, and the program's name in each frame's
 * GLB in place of "flow-sdk". */
int flow_stage_id(FlowStage *stage, uint32_t id);
int flow_stage_made_by(FlowStage *stage, const char *generator);

/* Sends each frame packed into whole numbers (flow_scene_glb_packed), half
 * the size, for Flow; false sends them as floats again, as made. */
int flow_stage_packed(FlowStage *stage, bool packed);

int flow_stage_layout(const FlowStage *stage, FlowStageLayout *out);

/* The screen cell a point of the picture is in (the nearest on it), and
 * the point at the middle of a screen cell, -1 if the cell isn't on the
 * picture: for clicks. */
int flow_stage_cell_at(const FlowStage *stage, float x, float y, uint16_t *col, uint16_t *row);
int flow_stage_point_at(const FlowStage *stage, uint16_t col, uint16_t row, float *x, float *y);

/* What shows `scene`: the object registered again, and the first time
 * where it goes. "" when it's the last frame again. Send it with
 * flow_terminal_send, and free it with flow_free. */
char *flow_stage_show(FlowStage *stage, const FlowScene *scene);

/* Takes the picture away: before a new stage after a resize, and on the
 * way out. */
char *flow_stage_clear(const FlowStage *stage);

/* An effect for Flow's liquid (FLOW_FX_*) at point `at` of the picture
 * (x, y), reaching `size` pixels, as hard as `strength` (0 to 1), put in
 * the screen's cells. `to` is where a bolt ends and `dir` which way a stir
 * pushes (y up), each NULL for none. Its color and sound are set after,
 * as any effect's; free it with flow_effect_free. NULL for a kind there's
 * none of or a number that isn't finite. */
FlowEffect *flow_stage_effect(const FlowStage *stage, int32_t kind, const float at[2], const float *to, const float *dir, float size, float strength);

/* ------------------------------------------------------------------------
 * Screens: cells drawn each frame, sent as their difference from the last
 *
 * A screen is letters in their colors, with pieces of pictures among them
 * (each sent first, at FLOW_PLACEMENT), drawn into each frame and sent as
 * the difference from the one shown before it: only the cells that
 * changed, with the colors set only where they change. Cells count from 0
 * at the top left; what falls off the screen isn't drawn. A letter with no
 * background keeps the terminal's own, which in Flow is the glass and the
 * liquid. Colors are 0xRRGGBB; a background that's negative is the
 * terminal's own.
 * ------------------------------------------------------------------------ */

typedef struct FlowScreen FlowScreen;

/* A blank screen `cols` × `rows` cells (flow_term_size's), nothing shown
 * yet; NULL for none of either. Make a new one after a resize. */
FlowScreen *flow_screen_new(uint16_t cols, uint16_t rows);
void flow_screen_free(FlowScreen *screen);

/* Writes `text` (UTF-8) from cell (`col`, `row`), a cell a character, in
 * `fg` on `bg`, bold or not. */
int flow_screen_text(FlowScreen *screen, int32_t col, int32_t row, const char *text, uint32_t fg, int32_t bg, bool bold);

/* Picture `id` over `cols` × `rows` cells from (`col`, `row`), each on
 * the background of the cell it covers, which shows through the picture's
 * clear pixels where the terminal draws it (Flow doesn't): send it first
 * (flow_picture_upload, at FLOW_PLACEMENT, over as many cells). */
int flow_screen_image(FlowScreen *screen, int32_t col, int32_t row, uint16_t cols, uint16_t rows, uint32_t id);

/* Blanks `cols` × `rows` cells from (`col`, `row`), on `bg`: the whole
 * screen, to draw the next frame afresh. */
int flow_screen_clear(FlowScreen *screen, int32_t col, int32_t row, int32_t cols, int32_t rows, int32_t bg);

/* Lays `bg` (0xRRGGBB, never the terminal's own) under every cell of the
 * `cols` × `rows` from (`col`, `row`) that has no background of its own:
 * over the whole screen after drawing it, it leaves nothing to the
 * terminal's background, so a light one can't wash the screen out. */
int flow_screen_ground(FlowScreen *screen, int32_t col, int32_t row, int32_t cols, int32_t rows, uint32_t bg);

/* Row `row`'s letters, a picture's cells as spaces; NULL off the screen.
 * Free it with flow_free. */
char *flow_screen_row(const FlowScreen *screen, uint16_t row);

/* The escapes that bring the terminal from the screen shown last (a
 * cleared one the first time) to this one, picture cells bare if `bare`
 * (FlowCaps.bare: then each row of a taller picture has to be a picture of
 * its own). The screen is shown from then on, and stays to draw over.
 * Write it with flow_write_free, after the pictures it shows are sent. */
char *flow_screen_show(FlowScreen *screen, bool bare);

/* Has the next flow_screen_show draw everything, as after the terminal was
 * cleared or another program drew on it. */
int flow_screen_forget(FlowScreen *screen);

/* ------------------------------------------------------------------------
 * The liquid and sound (OSC 7731, DECPS)
 *
 * Flow takes effects from the tab in front only, about six a second after
 * a burst of eight, at cells on the screen shown; another terminal ignores
 * them, but send them only in Flow to be tidy (FlowCaps.flow). Each sounds
 * as its kind does while Flow's sound is on, or as flow_effect_sound says.
 * ------------------------------------------------------------------------ */

#define FLOW_FX_BOLT   0  /* lightning to flow_effect_to's cell, which it needs */
#define FLOW_FX_BURST  1  /* a splash, in its color where the liquid takes color */
#define FLOW_FX_RIPPLE 2  /* a ring spreading */
#define FLOW_FX_STIR   3  /* the liquid pushed along flow_effect_dir, which it needs */

#define FLOW_SOUND_DROP     0
#define FLOW_SOUND_SPLASH   1
#define FLOW_SOUND_THUNDER  2
#define FLOW_SOUND_WHOOSH   3
#define FLOW_SOUND_CHIME    4
#define FLOW_SOUND_LOW      5
#define FLOW_SOUND_DING     6
#define FLOW_SOUND_THUD     7
#define FLOW_SOUND_CLICK    8
#define FLOW_SOUND_BOING    9
#define FLOW_SOUND_BELL    10

/* An effect at screen cell (`col`, `row`), counting from 0; NULL for a
 * kind there's none of. */
FlowEffect *flow_effect_new(int32_t kind, uint16_t col, uint16_t row);
void flow_effect_free(FlowEffect *effect);

/* Each changes the effect, or returns -1 and leaves it as it was for a
 * value refused (a sound there's none of, a float that isn't finite). */
int flow_effect_to(FlowEffect *effect, uint16_t col, uint16_t row);
int flow_effect_color(FlowEffect *effect, uint32_t rgb);       /* 0xRRGGBB */
int flow_effect_dir(FlowEffect *effect, float dx, float dy);    /* y down */
int flow_effect_size(FlowEffect *effect, float columns);        /* 0.5 to 40 */
int flow_effect_strength(FlowEffect *effect, float strength);   /* 0 to 1 */
int flow_effect_volume(FlowEffect *effect, float volume);       /* its sound's, 0 to 1 */
int flow_effect_sound(FlowEffect *effect, int32_t sound);       /* FLOW_SOUND_* */
int flow_effect_quiet(FlowEffect *effect);                      /* no sound */

/* The effect's escape. */
char *flow_effect_escape(const FlowEffect *effect);

/* One of Flow's sounds alone, placed left or right by `col` (leave `col`
 * or `row` negative for the middle), as strong as `strength` (NAN for 0.5)
 * and at `volume` of what Flow would make it (NAN or 1 for full). */
char *flow_sound(int32_t sound, int32_t col, int32_t row, float strength, float volume);

/* FLOW_SOUND_* for a sound's name, in any case ("chime"), or -1. */
int flow_sound_named(const char *name);

/* Notes as DECPS plays them: `volume` 0 (silent) to 7, each note `length`
 * 32nds of a second, `count` notes from 1 (C5) up a semitone at a time to
 * 25 (C7), 0 a rest. NULL for no notes. */
char *flow_notes(uint8_t volume, uint8_t length, const uint8_t *notes, size_t count);

/* DECPS's number for a note by name ("C5", "C#5", "Db5", "r" for a rest),
 * or -1 past its two octaves. */
int flow_note(const char *name);

/* Washes Flow's glass in `rgb` (0xRRGGBB) as far as `strength` (0 to 1)
 * says while the program holds the alternate screen, as a game's night; a
 * strength of 0 washes it off. Send it when it changes, not each frame. */
char *flow_tint(uint32_t rgb, float strength);

/* A budget that keeps a program under Flow's allowance, so nothing it
 * sends is dropped unseen: a full `burst` of effects and named sounds,
 * coming back at `per_second` a second (NAN for either takes 7, then 5 a
 * second, a little under Flow's 8 and 6). Time is the program's own clock
 * in seconds; notes and the tint don't count. */
typedef struct FlowBudget FlowBudget;

FlowBudget *flow_budget_new(float per_second, float burst);
void flow_budget_free(FlowBudget *budget);

/* Spends one at `now` if `keep` would still be left after it, for what
 * matters more; false, and nothing spent, if not. */
bool flow_budget_take(FlowBudget *budget, float now, float keep);

/* How many are left at `now`; NAN for NULL. */
float flow_budget_left(const FlowBudget *budget, float now);

/* ------------------------------------------------------------------------
 * Flow's settings (OSC 7732)
 *
 * A program can see Flow's settings and change them, as flow-settings
 * does. A request is a JSON object, {"do": ...} with a "key" (a dotted
 * name from Flow's config.toml, as "fx.style") and a "value" (any JSON)
 * where it needs them, and Flow answers with another:
 *
 *   "list"            every setting a program may see, as
 *                     {"ok": true, "settings": {"fx.style": "rain", ...}}
 *   "get"             one, as {"ok": true, "key": ..., "value": ...}
 *   "set"             changes one, answered with what it is now; Flow
 *                     applies it, saves it and says so in a toast
 *   "reset"           puts one back to its default, or with "key": "all"
 *                     every one a program may change
 *   "reset-terminal"  starts the asking tab afresh
 *   "restart"         restarts Flow as Settings' Restart does: at once
 *                     while every other tab waits at its prompt, else
 *                     asking the user first
 *
 * What fails is answered {"ok": false, "error": "..."}, the reason in
 * Flow's language. Flow decides what a program may see and change: never
 * the assistant's settings, nor the user's own theme. The JSON is the
 * program's to write and to read: these pass it on as UTF-8 text. Flow
 * answers from 0.1.30 on.
 * ------------------------------------------------------------------------ */

/* The bytes that ask Flow `request`: OSC 7732 with the request in base64.
 * flow_settings_ask writes them and reads the answer; this is for a program
 * that reads its own input. NULL for text that isn't UTF-8. */
char *flow_settings_request(const char *request);

/* The first whole answer in the `len` bytes at `bytes` (what came in after
 * asking), as JSON text; NULL until one has come whole, or if it isn't one
 * Flow wrote. */
char *flow_settings_answer(const void *bytes, size_t len);

/* Asks Flow `request` and waits `wait_ms` at most (0 for three seconds, as
 * flow-settings waits) for its answer, as JSON text, which may be one that
 * isn't ok; NULL when nothing answered: a terminal that isn't Flow, a Flow
 * from before 0.1.30, or no terminal at all. A terminal that isn't Flow
 * says nothing, so the whole wait passes: ask where flow_in_flow() or
 * FlowCaps.flow says it's Flow.
 *
 * On macOS and Linux it asks the program's terminal itself, opened by
 * name, so it reaches Flow however standard input and output are piped or
 * redirected. The answer comes in as keys: raw mode is turned on for the
 * wait where it isn't on already (flow_raw_mode, flow_term_enter) and off
 * again after, so it can be called between lines read from the terminal;
 * keys waiting to be read or typed meanwhile are read with the answer and
 * lost, and nothing else may be reading the keys. Where none of standard
 * input, output and error is a terminal it asks nothing and returns NULL
 * at once.
 *
 * An answer that comes after its wait has run out lands in the terminal's
 * input: the next call throws away what waits there before it asks, but
 * meanwhile a terminal that isn't raw echoes it, and a program reading
 * lines reads it. So don't cut the wait short (a Flow behind other windows
 * or without a GPU answers slower), and check that an answer to "get",
 * "set" or "reset" names the "key" asked about, as Python's functions do:
 * one that came late enough could still be taken for the next's.
 *
 * On Windows the question goes down Flow's pipe instead, which needs no
 * raw mode, and wait_ms isn't used: the pipe can't be read against a
 * clock, so Flow's own wait, three seconds or so, bounds it, and a Flow
 * that couldn't answer in time, or one from before 0.1.30, gives NULL. */
char *flow_settings_ask(const char *request, uint32_t wait_ms);

/* ------------------------------------------------------------------------
 * Audio of the program's own
 *
 * Tones, tunes and songs made in code, played on the machine in any
 * terminal or none: through the system's output (CoreAudio on macOS,
 * WASAPI on Windows, ALSA's default device on Linux) by a FlowAudio, or
 * rendered into the program's own buffers by a FlowMixer, for an output of
 * its own. Flow's sounds, above, follow the user's settings for sound in
 * Flow; keep these for music and the like.
 *
 * A library built without its `audio` feature (cargo's
 * --no-default-features) still has these functions, but each refuses:
 * NULL, -1, 0 or false, and flow_audio_built() says so.
 * ------------------------------------------------------------------------ */

/* Whether this library was built with audio, and whether a FlowAudio
 * plays anything here (built with it, on macOS, Windows or Linux). */
bool flow_audio_built(void);
bool flow_audio_plays(void);

/* A note's pitch in hertz from its name: a letter, a sharp (#) or flat
 * (b), and the octave, "C4" middle C and "A4" 440. 0 for no such note. */
float flow_pitch(const char *name);

#define FLOW_WAVE_SINE      0
#define FLOW_WAVE_TRIANGLE  1
#define FLOW_WAVE_SQUARE    2
#define FLOW_WAVE_SAW       3
#define FLOW_WAVE_NOISE     4  /* white noise, for drums and wind */

/* One note: its pitch and how long it's held, how it sounds, and how it
 * comes in and dies away. flow_tone_init gives the SDK's: a sine at half
 * volume in the middle, in over 5 ms and away over 80. A tone with a
 * number that isn't finite is refused. */
typedef struct FlowTone {
    float hz;
    float secs;
    int32_t wave;       /* FLOW_WAVE_* */
    float volume;       /* 0 to 1 */
    float pan;          /* left (-1) to right (1) */
    float attack;       /* seconds to come in */
    float release;      /* seconds to die away after it's held */
} FlowTone;

int flow_tone_init(FlowTone *tone, float hz, float secs);

/* A line of notes played one after another on one sound, written as
 * name/beats ("C4/1 E4/1 G4/2 r/1", r a rest) at `tempo` beats a minute,
 * each note sounding as `sound` does but for its pitch and length (NULL
 * for the SDK's tone), once or `looped` until stopped.
 *
 * Where notes don't read, the function returns NULL or -1 and, if `error`
 * isn't NULL, puts the reason in `*error` ("A4: a note needs its length"),
 * to free with flow_free; `*error` is NULL when it did. */
typedef struct FlowTune FlowTune;

FlowTune *flow_tune_new(const char *notes, float tempo, const FlowTone *sound, bool looped, char **error);
float flow_tune_length(const FlowTune *tune);   /* seconds once through */
void flow_tune_free(FlowTune *tune);

/* A song: lines for several instruments played together and looped, each
 * as many beats long as the others, at `tempo` beats a minute. */
#define FLOW_INSTRUMENT_LUTE      0  /* a plucked string, ringing on */
#define FLOW_INSTRUMENT_RECORDER  1  /* a breathy pipe, with a vibrato */
#define FLOW_INSTRUMENT_DRONE     2  /* a note and its fifth, held under a tune */
#define FLOW_INSTRUMENT_TABOR     3  /* a drum: X/1 a full beat, x/1 a light one */

typedef struct FlowSong FlowSong;

FlowSong *flow_song_new(float tempo);

/* Adds `instrument` (FLOW_INSTRUMENT_*) playing `notes`, written as a
 * tune's are ("D4/3 Bb3/2 r/1"), as loud as `gain` (1 for the instrument's
 * own level). `error` as for flow_tune_new. */
int flow_song_line(FlowSong *song, int32_t instrument, float gain, const char *notes, char **error);
float flow_song_length(const FlowSong *song);   /* seconds once through */
void flow_song_free(FlowSong *song);

/* The program's sound on the machine: a thread of its own, started with
 * the first sound, opens the system's output then and closes it after
 * three seconds of silence. What it's given is copied, so a tune or song
 * may be freed once handed over. Freeing it stops what's playing. */
typedef struct FlowAudio FlowAudio;

FlowAudio *flow_audio_new(void);
void flow_audio_free(FlowAudio *audio);
int flow_audio_play(FlowAudio *audio, const FlowTone *tone);
int flow_audio_play_tune(FlowAudio *audio, const FlowTune *tune);

/* Plays `song` looped, fading it in over a second as the song before it
 * fades out over a little more, as a game's music goes from place to
 * place; flow_audio_stop_song fades it out. */
int flow_audio_play_song(FlowAudio *audio, const FlowSong *song);
int flow_audio_stop_song(FlowAudio *audio);

/* Everything falls silent at once, songs too. */
int flow_audio_stop(FlowAudio *audio);

/* The whole mix's volume, 0 to 1 (1 unless set). */
int flow_audio_set_volume(FlowAudio *audio, float volume);

/* The mix itself at `rate` samples a second, with no output: the voices,
 * tunes and songs summed with a soft limit. Lock it yourself if your
 * output's callback runs on another thread; a song's voices are made by
 * flow_mixer_play_song, so call that outside the callback. */
typedef struct FlowMixer FlowMixer;

FlowMixer *flow_mixer_new(float rate);
void flow_mixer_free(FlowMixer *mixer);
int flow_mixer_play(FlowMixer *mixer, const FlowTone *tone);
int flow_mixer_play_tune(FlowMixer *mixer, const FlowTune *tune);
int flow_mixer_play_song(FlowMixer *mixer, const FlowSong *song);
int flow_mixer_stop_song(FlowMixer *mixer);
int flow_mixer_stop(FlowMixer *mixer);
int flow_mixer_set_volume(FlowMixer *mixer, float volume);

/* Whether anything still sounds or is to come: a song, until it has faded
 * out. */
bool flow_mixer_busy(const FlowMixer *mixer);

/* Fills `len` floats at `out` with what comes next, `channels` to a frame:
 * left and right first, any more left silent, and one alone gets both. */
int flow_mixer_fill(FlowMixer *mixer, float *out, size_t len, uint32_t channels);

#ifdef __cplusplus
}
#endif

#endif /* FLOW_SDK_H */
