#include <gio/gdesktopappinfo.h>
#include <gio/gio.h>
#include <string.h>

typedef void (*AppCallback)(void *, const char *, const char *, const char *, const char *,
                            const char *const *, int);

static void emit_app(GAppInfo *app, void *context, AppCallback callback, int current, const char *custom_path) {
    const char *id = custom_path ? custom_path : g_app_info_get_id(app);
    if (id == NULL || (G_IS_DESKTOP_APP_INFO(app)
            && g_desktop_app_info_get_is_hidden(G_DESKTOP_APP_INFO(app))))
        return;

    const char *path = custom_path ? custom_path : G_IS_DESKTOP_APP_INFO(app)
        ? g_desktop_app_info_get_filename(G_DESKTOP_APP_INFO(app)) : NULL;
    GIcon *icon = g_app_info_get_icon(app);
    char *icon_path = NULL;
    const char *file_icons[] = {NULL, NULL};
    const char *const *icons = file_icons;
    if (icon && G_IS_THEMED_ICON(icon)) {
        icons = g_themed_icon_get_names(G_THEMED_ICON(icon));
    } else if (icon && G_IS_FILE_ICON(icon)) {
        icon_path = g_file_get_path(g_file_icon_get_file(G_FILE_ICON(icon)));
        file_icons[0] = icon_path;
    }
    char *source = G_IS_DESKTOP_APP_INFO(app)
        ? g_desktop_app_info_get_string(G_DESKTOP_APP_INFO(app), "X-DefaultHelper-Source") : NULL;
    callback(context, id, g_app_info_get_display_name(app), path, source, icons, current);
    g_free(source);
    g_free(icon_path);
}

void dh_linux_lookup(const char *mime, void *context, AppCallback callback) {
    GAppInfo *current = g_app_info_get_default_for_type(mime, FALSE);
    if (current) {
        emit_app(current, context, callback, 1, NULL);
        g_object_unref(current);
    }
    GList *apps = g_app_info_get_all_for_type(mime);
    for (GList *item = apps; item; item = item->next)
        emit_app(G_APP_INFO(item->data), context, callback, 0, NULL);
    g_list_free_full(apps, g_object_unref);

    // File managers do not always advertise inode/directory in MimeType.
    // Include visible desktop entries in the standard FileManager category.
    if (strcmp(mime, "inode/directory") == 0) {
        apps = g_app_info_get_all();
        for (GList *item = apps; item; item = item->next) {
            GAppInfo *app = G_APP_INFO(item->data);
            if (!G_IS_DESKTOP_APP_INFO(app) || !g_app_info_should_show(app))
                continue;
            const char *categories = g_desktop_app_info_get_categories(G_DESKTOP_APP_INFO(app));
            char **parts = g_strsplit(categories ? categories : "", ";", -1);
            if (g_strv_contains((const char *const *)parts, "FileManager"))
                emit_app(app, context, callback, 0, NULL);
            g_strfreev(parts);
        }
        g_list_free_full(apps, g_object_unref);
    }
}

char *dh_linux_extension_type(const char *extension) {
    char *filename = g_strconcat("file.", extension, NULL);
    char *mime = g_content_type_guess(filename, NULL, 0, NULL);
    g_free(filename);
    if (mime && strcmp(mime, "application/octet-stream") != 0)
        return mime;
    g_free(mime);
    char *hash = g_compute_checksum_for_string(G_CHECKSUM_SHA256, extension, -1);
    char *custom = g_strconcat("application/x-default-helper-ext-", hash, NULL);
    g_free(hash);
    return custom;
}

// Only called when applying an unknown extension. Lookup remains read-only.
char *dh_linux_register_extension(const char *extension, const char *mime) {
    if (!g_str_has_prefix(mime, "application/x-default-helper-ext-"))
        return NULL;
    char *tool = g_find_program_in_path("update-mime-database");
    if (!tool)
        return g_strdup("注册新扩展名需要 update-mime-database，请安装 shared-mime-info。");
    char *directory = g_build_filename(g_get_user_data_dir(), "mime", NULL);
    char *packages = g_build_filename(directory, "packages", NULL);
    char *name = g_strconcat(mime + strlen("application/"), ".xml", NULL);
    char *path = g_build_filename(packages, name, NULL);
    char *escaped = g_markup_escape_text(extension, -1);
    char *xml = g_strdup_printf("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        "<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">"
        "<mime-type type=\"%s\"><comment>Custom .%s file</comment>"
        "<glob pattern=\"*.%s\"/></mime-type></mime-info>\n", mime, escaped, escaped);
    GError *error = NULL;
    char *message = NULL;
    if (g_mkdir_with_parents(packages, 0700) != 0) {
        message = g_strdup("无法创建当前用户的 MIME 数据目录。");
    } else if (!g_file_set_contents(path, xml, -1, &error)) {
        message = g_strdup(error->message);
    } else {
        char *argv[] = {tool, directory, NULL};
        char *stderr_text = NULL;
        int status = 0;
        if (!g_spawn_sync(NULL, argv, NULL, 0, NULL, NULL, NULL, &stderr_text, &status, &error)
                || !g_spawn_check_wait_status(status, &error))
            message = g_strdup_printf("更新 MIME 数据库失败：%s %s", error->message,
                stderr_text ? stderr_text : "");
        g_free(stderr_text);
    }
    g_clear_error(&error);
    g_free(xml);
    g_free(escaped);
    g_free(path);
    g_free(name);
    g_free(packages);
    g_free(directory);
    g_free(tool);
    return message;
}

static void append_exec_argument(GString *command, const char *argument) {
    g_string_append_c(command, '"');
    for (const char *c = argument; *c; c++) {
        if (strchr("\\\"`$", *c))
            g_string_append_c(command, '\\');
        if (*c == '%')
            g_string_append_c(command, '%');
        g_string_append_c(command, *c);
    }
    g_string_append_c(command, '"');
}

static GKeyFile *custom_key_file(const char *path, gboolean scheme, GError **error) {
    GKeyFile *file = g_key_file_new();
    if (g_str_has_suffix(path, ".desktop")) {
        if (!g_key_file_load_from_file(file, path,
                G_KEY_FILE_KEEP_COMMENTS | G_KEY_FILE_KEEP_TRANSLATIONS, error)) {
            g_key_file_unref(file);
            return NULL;
        }
    } else {
        if (!g_file_test(path, G_FILE_TEST_IS_REGULAR)
                || !g_file_test(path, G_FILE_TEST_IS_EXECUTABLE)) {
            g_set_error_literal(error, G_IO_ERROR, G_IO_ERROR_INVALID_ARGUMENT,
                "请选择有效的 .desktop 文件或具有执行权限的程序。");
            g_key_file_unref(file);
            return NULL;
        }
        // Desktop Entry Exec quoting is distinct from shell quoting. Percent
        // field codes and reserved characters in the literal path must escape.
        GString *command = g_string_new(NULL);
        if (strchr(path, '%')) {
            // GIO checks argv[0] before expanding %% field escapes. Use env as
            // argv[0] for literal percent paths; no shell is involved.
            char *env = g_find_program_in_path("env");
            if (!env) {
                g_set_error_literal(error, G_IO_ERROR, G_IO_ERROR_NOT_FOUND,
                    "包含百分号的程序路径需要 env 命令。");
                g_string_free(command, TRUE);
                g_key_file_unref(file);
                return NULL;
            }
            append_exec_argument(command, env);
            g_string_append(command, " -- ");
            g_free(env);
        }
        append_exec_argument(command, path);
        g_string_append(command, scheme ? " %u" : " %f");
        char *name = g_path_get_basename(path);
        g_key_file_set_string(file, "Desktop Entry", "Type", "Application");
        g_key_file_set_string(file, "Desktop Entry", "Name", name);
        g_key_file_set_string(file, "Desktop Entry", "Exec", command->str);
        g_key_file_set_boolean(file, "Desktop Entry", "NoDisplay", TRUE);
        g_string_free(command, TRUE);
        g_free(name);
    }
    GDesktopAppInfo *app = g_desktop_app_info_new_from_keyfile(file);
    const char *commandline = app ? g_app_info_get_commandline(G_APP_INFO(app)) : NULL;
    if (!app || g_desktop_app_info_get_is_hidden(app)
            || ((!commandline || !*commandline)
                && !g_key_file_get_boolean(file, "Desktop Entry", "DBusActivatable", NULL))) {
        g_set_error_literal(error, G_IO_ERROR, G_IO_ERROR_INVALID_ARGUMENT,
            "应用的 desktop 文件无效、已被屏蔽或找不到可执行程序。");
        g_key_file_unref(file);
        file = NULL;
    }
    g_clear_object(&app);
    return file;
}

char *dh_linux_custom(const char *path, void *context, AppCallback callback) {
    GError *error = NULL;
    GKeyFile *file = custom_key_file(path, FALSE, &error);
    if (!file) {
        char *message = g_strdup(error->message);
        g_clear_error(&error);
        return message;
    }
    GDesktopAppInfo *app = g_desktop_app_info_new_from_keyfile(file);
    if (!app) {
        g_key_file_unref(file);
        return g_strdup("应用已变化或被移除，请重新添加。");
    }
    emit_app(G_APP_INFO(app), context, callback, 0, path);
    g_object_unref(app);
    g_key_file_unref(file);
    return NULL;
}

// Desktop-specific user defaults precede mimeapps.list. Update an existing
// override for this type, keeping comments and every unrelated setting.
static gboolean update_desktop_overrides(const char *mime, const char *id, GError **error) {
    const char *desktop = g_getenv("XDG_CURRENT_DESKTOP");
    char **desktops = g_strsplit(desktop ? desktop : "", ":", -1);
    gboolean success = TRUE;
    for (char **entry = desktops; *entry && success; entry++) {
        if (!**entry)
            continue;
        gboolean valid = TRUE;
        for (const char *c = *entry; *c; c++) {
            if (!g_ascii_isalnum(*c) && *c != '_' && *c != '-')
                valid = FALSE;
        }
        if (!valid)
            continue;
        char *lower = g_ascii_strdown(*entry, -1);
        char *name = g_strconcat(lower, "-mimeapps.list", NULL);
        char *path = g_build_filename(g_get_user_config_dir(), name, NULL);
        GKeyFile *file = g_key_file_new();
        GError *load_error = NULL;
        if (g_key_file_load_from_file(file, path,
                G_KEY_FILE_KEEP_COMMENTS | G_KEY_FILE_KEEP_TRANSLATIONS, &load_error)) {
            if (g_key_file_has_key(file, "Default Applications", mime, NULL)) {
                const char *ids[] = {id};
                g_key_file_set_string_list(file, "Default Applications", mime, ids, 1);
                success = g_key_file_save_to_file(file, path, error);
            }
        } else if (!g_error_matches(load_error, G_FILE_ERROR, G_FILE_ERROR_NOENT)) {
            g_propagate_error(error, load_error);
            load_error = NULL;
            success = FALSE;
        }
        g_clear_error(&load_error);
        g_key_file_unref(file);
        g_free(path);
        g_free(name);
        g_free(lower);
    }
    g_strfreev(desktops);
    return success;
}

static char *validate_user_mimeapps(void) {
    GError *error = NULL;
    char *path = g_build_filename(g_get_user_config_dir(), "mimeapps.list", NULL);
    GKeyFile *file = g_key_file_new();
    gboolean loaded = g_key_file_load_from_file(file, path, G_KEY_FILE_NONE, &error);
    g_key_file_unref(file);
    g_free(path);
    char *message = NULL;
    if (!loaded && !g_error_matches(error, G_FILE_ERROR, G_FILE_ERROR_NOENT))
        message = g_strdup(error->message);
    g_clear_error(&error);
    return message;
}

// Returns an owned error message, or NULL on success. Called on a worker thread.
char *dh_linux_set(const char *mime, const char *id) {
    GDesktopAppInfo *app = g_desktop_app_info_new(id);
    if (!app || g_desktop_app_info_get_is_hidden(app)) {
        g_clear_object(&app);
        return g_strdup("应用已被移除或 desktop 文件无效，请重新查询。");
    }
    GError *error = NULL;
    // Never replace malformed user configuration and discard unrelated values.
    char *validation = validate_user_mimeapps();
    if (validation) {
        g_object_unref(app);
        return validation;
    }
    // GIO invalidates its user-config cache when setting the generic default,
    // so update higher-priority user overrides first.
    gboolean success = update_desktop_overrides(mime, id, &error)
        && g_app_info_set_as_default_for_type(G_APP_INFO(app), mime, &error);
    g_object_unref(app);
    if (!success) {
        char *message = g_strdup(error ? error->message : "无法写入默认应用设置。");
        g_clear_error(&error);
        return message;
    }
    GAppInfo *current = g_app_info_get_default_for_type(mime, FALSE);
    gboolean matches = current && g_strcmp0(g_app_info_get_id(current), id) == 0;
    g_clear_object(&current);
    return matches ? NULL : g_strdup("设置已写入，但被更高优先级的应用关联覆盖，请检查 mimeapps.list。");
}

char *dh_linux_set_custom(const char *mime, const char *path, int scheme) {
    char *validation = validate_user_mimeapps();
    if (validation) return validation;
    GError *error = NULL;
    GKeyFile *file = custom_key_file(path, scheme, &error);
    if (!file) {
        char *message = g_strdup(error->message);
        g_clear_error(&error);
        return message;
    }
    GList *installed = g_app_info_get_all();
    char *id = NULL;
    char *destination = NULL;
    for (GList *item = installed; item; item = item->next) {
        if (!G_IS_DESKTOP_APP_INFO(item->data))
            continue;
        GDesktopAppInfo *app = item->data;
        if (g_strcmp0(path, g_desktop_app_info_get_filename(app)) == 0) {
            id = g_strdup(g_app_info_get_id(G_APP_INFO(app)));
            break;
        }
        char *source = g_desktop_app_info_get_string(app, "X-DefaultHelper-Source");
        gboolean same_mode = g_desktop_app_info_get_boolean(app, "X-DefaultHelper-URIs") == !!scheme;
        if (g_strcmp0(source, path) == 0 && same_mode) {
            id = g_strdup(g_app_info_get_id(G_APP_INFO(app)));
            destination = g_strdup(g_desktop_app_info_get_filename(app));
        }
        g_free(source);
        if (id)
            break;
    }
    g_list_free_full(installed, g_object_unref);
    if (id && !destination) {
        // Preserve the ID of an already installed app, including D-Bus details.
        g_key_file_unref(file);
        char *message = dh_linux_set(mime, id);
        g_free(id);
        return message;
    }
    char *exec = g_key_file_get_string(file, "Desktop Entry", "Exec", NULL);
    gboolean has_exec = exec && *exec;
    g_free(exec);
    if (!has_exec) {
        g_key_file_unref(file);
        g_free(destination);
        g_free(id);
        return g_strdup("未安装的 desktop 应用需要有效的 Exec 启动命令。");
    }
    g_key_file_set_boolean(file, "Desktop Entry", "DBusActivatable", FALSE);
    g_key_file_set_boolean(file, "Desktop Entry", "NoDisplay", TRUE);
    g_key_file_set_string(file, "Desktop Entry", "X-DefaultHelper-Source", path);
    g_key_file_set_boolean(file, "Desktop Entry", "X-DefaultHelper-URIs", scheme);
    char *message = NULL;
    if (!destination) {
        // Let GIO save/register the new entry and invalidate its application
        // cache. Saving a new file directly leaves GIO's cached ID list stale.
        GDesktopAppInfo *app = g_desktop_app_info_new_from_keyfile(file);
        if (!app) {
            message = g_strdup("应用已变化或被移除，请重新添加。");
        } else if (!g_app_info_add_supports_type(G_APP_INFO(app), mime, &error)) {
            message = g_strdup(error ? error->message : "无法注册此应用。");
        } else {
            id = g_strdup(g_app_info_get_id(G_APP_INFO(app)));
            destination = g_strdup(g_desktop_app_info_get_filename(app));
        }
        g_clear_object(&app);
    }
    if (!message) {
        // Restore all source desktop fields and our reuse metadata after GIO's
        // initial save, which keeps only its standard application fields.
        if (!g_key_file_save_to_file(file, destination, &error))
            message = g_strdup(error->message);
        else
            message = dh_linux_set(mime, id);
    }
    g_clear_error(&error);
    g_key_file_unref(file);
    g_free(destination);
    g_free(id);
    return message;
}

void dh_linux_free(char *message) {
    g_free(message);
}
