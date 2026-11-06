/*
 * hello.c: Flow's SDK from C. Builds a cut gem as a mesh, asks the terminal
 * what it can do, and where it shows 3D models places the gem turning in
 * the middle of the screen; in Flow it also ripples the liquid round it
 * with a chime. Any key, a click or a few seconds ends it. Outside a
 * terminal, or in one that shows none of this, it says so and stops.
 *
 *   cargo build -p flow-sdk-c
 *   cc c/examples/hello.c -I c/include -L target/debug -lflow_sdk_c \
 *      -Wl,-rpath,target/debug -o hello && ./hello
 */

#include <stdio.h>
#include <string.h>

#include "flow_sdk.h"

/* The gem: an octahedron, its upper faces gold and its lower ones amber. */
static void cut_gem(FlowMesh *gem) {
    const float gold[4] = {0.98f, 0.80f, 0.30f, 1.0f};
    const float amber[4] = {0.85f, 0.45f, 0.10f, 1.0f};
    const float middle[3] = {0.0f, 0.0f, 0.0f};
    const float top[3] = {0.0f, 1.0f, 0.0f};
    const float bottom[3] = {0.0f, -1.0f, 0.0f};
    const float ring[4][3] = {{1.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 1.0f}, {-1.0f, 0.0f, 0.0f}, {0.0f, 0.0f, -1.0f}};
    for (int i = 0; i < 4; i++) {
        const float *a = ring[i];
        const float *b = ring[(i + 1) % 4];
        flow_mesh_triangle(gem, gold, a, b, top, middle);
        flow_mesh_triangle(gem, amber, b, a, bottom, middle);
    }
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

int main(void) {
    if (flow_abi_version() != FLOW_SDK_ABI) {
        fprintf(stderr, "flow_sdk.h and the library it's linked with don't match.\n");
        return 1;
    }

    FlowMesh *gem = flow_mesh_new();
    cut_gem(gem);
    size_t glb_len = 0;
    uint8_t *glb = flow_mesh_glb(gem, &glb_len);
    size_t triangles = flow_mesh_triangles(gem);
    flow_mesh_free(gem);

    /* Raw keys on the alternate screen before asking, so the answers
     * aren't echoed; it fails quietly outside a terminal. */
    bool screen = flow_term_enter(false) == 0;
    FlowTerminal *terminal = flow_detect(FLOW_ASK_MODELS | FLOW_ASK_VERSION, 0);
    FlowCaps caps = {0};
    flow_terminal_caps(terminal, &caps);
    FlowSize size = {0};
    bool sized = flow_term_size(&size) == 0 && size.cols >= 20 && size.rows >= 8;

    if (!screen || !sized || (!caps.models && !caps.flow)) {
        if (screen) {
            flow_term_leave();
        }
        printf("This terminal shows no 3D models and isn't Flow, so the gem (%zu triangles, %zu bytes of glTF) goes unseen.\n", triangles, glb_len);
        flow_free(glb);
        flow_terminal_free(terminal);
        return 0;
    }

    uint16_t col = size.cols / 2, row = size.rows / 2;
    if (caps.models) {
        send(terminal, flow_model_register(1, FLOW_MODEL_GLB, glb, glb_len, "gem.glb"), false);
        FlowLook look;
        flow_look_init(&look);
        look.animate = 1;
        send(terminal, flow_model_place(1, row, col, 16, 8, &look), false);
    }
    if (caps.flow) {
        FlowEffect *ripple = flow_effect_new(FLOW_FX_RIPPLE, col, row);
        flow_effect_size(ripple, 10.0f);
        flow_effect_sound(ripple, FLOW_SOUND_CHIME);
        send(terminal, flow_effect_escape(ripple), true);
        flow_effect_free(ripple);
    }

    FlowEvent event;
    flow_next_event(4000, &event);

    if (caps.models) {
        send(terminal, flow_model_delete(1), false);
    }
    flow_term_leave();
    flow_free(glb);
    flow_terminal_free(terminal);
    return 0;
}
