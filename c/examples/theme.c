/*
 * theme.c: a theme package for Flow, from C. Builds a video terminal in
 * white phosphor, typed in at 9600 baud, and packs it as a .tar.gz, ready
 * for `flowapp install` in a Flow tab, which puts it in Settings' Theme
 * row and the new-tab picker. It needs no terminal; the package goes
 * where it's told (./theme out.tar.gz), else into the current folder.
 *
 *   cargo build -p flow-sdk-c
 *   cc c/examples/theme.c -I c/include -L target/debug -lflow_sdk_c \
 *      -Wl,-rpath,target/debug -o theme && ./theme
 */

#include <stdio.h>

#include "flow_sdk.h"

int main(int argc, char **argv) {
    if (flow_abi_version() != FLOW_SDK_ABI) {
        fprintf(stderr, "flow_sdk.h and the library it's linked with don't match.\n");
        return 1;
    }

    FlowTheme *vt52 = flow_theme_new("vt52");
    const char *fonts[] = {"VT323", "Glass TTY VT220"};
    const char *start[] = {"", "^{tall}VT52 DECscope", "^{tall}VT52 DECscope", "", "^READY", ""};
    /* Each setter says -1 for a name Flow hasn't; added up, any one shows. */
    int refused = flow_theme_name(vt52, "VT52")
        + flow_theme_name_in(vt52, "ja", "VT52 \xe7\xab\xaf\xe6\x9c\xab")
        + flow_theme_fonts(vt52, fonts, sizeof fonts / sizeof *fonts)
        + flow_theme_text(vt52, 0xE8F0F0)
        + flow_theme_cursor(vt52, 0xE8F0F0)
        + flow_theme_background(vt52, 0x0A0F0A)
        + flow_theme_phosphor(vt52, 0xE8F0F0)
        + flow_theme_cursor_shape(vt52, "block")
        + flow_theme_blink(vt52, true)
        + flow_theme_crt(vt52, "classic")
        + flow_theme_start(vt52, start, sizeof start / sizeof *start)
        + flow_theme_baud(vt52, 9600)
        + flow_theme_prompt(vt52, "$ ")
        + flow_theme_sounds(vt52, "terminal")
        + flow_theme_screen(vt52, 80, 24);
    if (refused != 0) {
        fprintf(stderr, "A setting was refused.\n");
        flow_theme_free(vt52);
        return 1;
    }

    FlowThemePackage *package = flow_theme_package_new("vt-themes", "1.0");
    flow_theme_package_add(package, vt52);
    flow_theme_free(vt52);

    /* An id Flow won't take is refused with the rest, saying why, and the
     * package is left as it was. */
    FlowThemePackage *wrong = flow_theme_package_new("vt-themes", "1.0");
    FlowTheme *capital = flow_theme_new("VT100");
    flow_theme_package_add(wrong, capital);
    char *why = NULL;
    if (flow_theme_package_check(wrong, &why) != 0) {
        printf("Not packed: %s\n", why);
        flow_free(why);
    }
    flow_theme_free(capital);
    flow_theme_package_free(wrong);

    char *named = flow_theme_package_file_name(package);
    const char *tarball = argc > 1 ? argv[1] : named;
    int status = 0;
    if (flow_theme_package_pack(package, tarball, &why) == 0) {
        printf("Packed %s: in a Flow tab, flowapp install %s\n", tarball, tarball);
    } else {
        fprintf(stderr, "The package wasn't made: %s\n", why);
        flow_free(why);
        status = 1;
    }
    flow_free(named);
    flow_theme_package_free(package);
    return status;
}
