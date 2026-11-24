/*
 * settings.c: Flow's settings from C, as flow-settings has them. Asks Flow
 * which liquid style it shows and how big its text is, and given a style
 * (./settings rain) puts it on, which Flow applies, saves and says in a
 * toast. Outside Flow, or outside a terminal, nothing answers, and it says
 * so; without the pane's key (FLOW_TAB_KEY), which Flow 0.1.46 needs of
 * every request and the library sends, it says so at once and asks
 * nothing. The JSON is written with printf and the answers printed as they
 * came: a real program would read them with a JSON library of its own.
 *
 *   cargo build -p flow-sdk-c
 *   cc c/examples/settings.c -I c/include -L target/debug -lflow_sdk_c \
 *      -Wl,-rpath,target/debug -o settings && ./settings
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "flow_sdk.h"

/* Whether `name` is a style's name, letters and dashes, so it can go into
 * the JSON as it is. */
static bool plain_name(const char *name) {
    if (*name == '\0' || strlen(name) > 32) {
        return false;
    }
    for (const char *c = name; *c; c++) {
        if (!((*c >= 'a' && *c <= 'z') || *c == '-')) {
            return false;
        }
    }
    return true;
}

/* Asks Flow `request` and prints its answer; false if nothing answered. */
static bool ask(const char *request) {
    char *answer = flow_settings_ask(request, 0);
    if (answer == NULL) {
        return false;
    }
    printf("%s\n  %s\n", request, answer);
    flow_free(answer);
    return true;
}

int main(int argc, char **argv) {
    if (flow_abi_version() != FLOW_SDK_ABI) {
        fprintf(stderr, "flow_sdk.h and the library it's linked with don't match.\n");
        return 1;
    }
    if (argc > 1 && !plain_name(argv[1])) {
        fprintf(stderr, "A style is a name such as rain or oil.\n");
        return 2;
    }
    /* Another terminal says nothing, and the whole wait would pass. */
    if (!flow_in_flow()) {
        printf("This isn't a Flow tab, so there are no settings of Flow's to ask about.\n");
        return 0;
    }
    /* Nor does Flow answer a request without the pane's key, which
     * flow_settings_ask sends from here. */
    const char *key = getenv("FLOW_TAB_KEY");
    if (key == NULL || *key == '\0') {
        printf("There's no tab key (FLOW_TAB_KEY) here, so nothing was asked: Flow answers only programs run in its own tabs, which have one. sudo and env -i leave it out, and ssh doesn't carry it.\n");
        return 0;
    }

    const char *request = "{\"do\":\"get\",\"key\":\"fx.style\"}";
    char set[96];
    if (argc > 1) {
        snprintf(set, sizeof set, "{\"do\":\"set\",\"key\":\"fx.style\",\"value\":\"%s\"}", argv[1]);
        request = set;
    }
    if (!ask(request)) {
        printf("Nothing answered: outside a terminal nothing is asked, a Flow from before 0.1.30 doesn't answer, nor does tmux or screen pass the question on, and Flow answers only what carries this pane's own key, not another pane's or one an ssh session or a script brought along.\n");
        return 0;
    }
    ask("{\"do\":\"get\",\"key\":\"font.size\"}");
    return 0;
}
